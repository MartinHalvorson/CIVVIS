#!/usr/bin/env python3
"""Which saved fields does the CIVVIS transition function itself read?

Drives `civvis core-probe` (src/core_probe.rs) over seeded games: record each,
blank every field of its turn-T save one at a time, replay the recorded
actions with no agent in the loop, and classify each field by what blanking it
did:

  core       something else in the end state changed, an action was refused,
             a seat's set of legal actions changed, or the engine could not
             load or replay without it
  derived    nothing else changed and the field was rebuilt by the next turn
             boundary (a cache, a per-turn total, a turn-start snapshot)
  transient  nothing else changed and the field came back later (a timer
             that expired inside the window); evidence of a short memory
  inert      nothing else changed and the field stayed different: the
             transition never read it in these windows (a ledger, a log, an
             agent's notes, a mirror of the real game)
  untested   already empty in every save, so blanking it tested nothing

Then blanks every non-core field at once (`strip`) to check that what is left
is sufficient on its own, prices each class in bytes, separates fields fixed
at setup from fields play moves, and times a clone with and without them.

  python3 tools/core_probe.py --civvis target/ci/civvis [--budget-minutes 30]
"""

import argparse
import concurrent.futures as cf
import gzip
import json
import os
import subprocess
import sys
import tempfile
import time

# (players, seed, T, K, fog). Online speed (250 turns); T is a target, the
# record step stops earlier on a slow host. `fog` forces last-seen memory on.
CONFIGS = [
    (4, 201, 60, 40, False),
    (4, 202, 120, 40, False),
    (6, 203, 100, 40, True),
    (4, 204, 180, 40, False),
    (8, 205, 90, 40, False),
    (6, 206, 160, 40, True),
    (4, 207, 205, 40, False),
    (2, 208, 120, 40, False),
]

PREFIX = "CPROBE "
RANK = {"core": 4, "inert": 3, "transient": 2, "derived": 1, "untested": 0}


def emit(obj):
    print(PREFIX + json.dumps(obj, separators=(",", ":")), flush=True)


def run(cmd, timeout):
    t = time.time()
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    return p.returncode, p.stdout, p.stderr[-2000:], time.time() - t


def items_of(save, path):
    outer, _ = path.split("[*].", 1)
    if "." in outer:
        k, s = outer.split(".", 1)
        return (save.get(k) or {}).get(s) or []
    return save.get(outer) or []


def size_of(save, path):
    if "[*]." in path:
        leaf = path.split("[*].", 1)[1]
        vals = [i.get(leaf) for i in items_of(save, path) if isinstance(i, dict)]
    else:
        vals = [save.get(path)]
    return sum(len(json.dumps(v, separators=(",", ":"))) for v in vals if v is not None)


def keyed(save, path):
    """The field's values keyed by record identity (id, else position)."""
    if "[*]." not in path:
        return {"": save.get(path)}
    leaf = path.split("[*].", 1)[1]
    out = {}
    for i, item in enumerate(items_of(save, path)):
        if isinstance(item, dict):
            out[json.dumps(item.get("id", item.get("pos", i)))] = item.get(leaf)
    return out


def gz(path):
    with open(path, "rb") as f:
        return len(gzip.compress(f.read(), 6))


def classify(row, base_refused):
    if row.get("trivial"):
        return "untested"
    if "load_error" in row or row.get("load_panic") or "replay_panic_at" in row:
        return "core"
    if row.get("diverged") or row.get("refused", 0) != base_refused or row.get("legal_differs"):
        return "core"
    if row.get("restored_at_checkpoint"):
        return "derived"
    if row.get("restored"):
        return "transient"
    return "inert"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--civvis", required=True)
    ap.add_argument("--budget-minutes", type=float, default=30.0)
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    ap.add_argument("--work", default=None)
    a = ap.parse_args()
    start = time.time()
    budget = a.budget_minutes * 60
    work = a.work or tempfile.mkdtemp(prefix="core-probe-")
    emit({"phase": "start", "work": work, "jobs": a.jobs, "budget_s": budget, "configs": CONFIGS})

    # 1. Record.
    record_budget = 0.25 * budget
    dirs = {}

    def record(cfg):
        players, seed, t, k, fog = cfg
        d = os.path.join(work, f"p{players}-s{seed}")
        cmd = [a.civvis, "core-probe", "record", str(players), str(seed), str(t), str(k), d, str(record_budget), "fog" if fog else "-"]
        rc, out, err, secs = run(cmd, timeout=record_budget + 0.15 * budget)
        return cfg, d, rc, out, err, secs

    with cf.ThreadPoolExecutor(a.jobs) as ex:
        for cfg, d, rc, out, err, secs in ex.map(record, CONFIGS):
            meta = json.loads(out.strip().splitlines()[-1]) if rc == 0 and out.strip() else None
            if meta:
                dirs[d] = meta
            emit({"phase": "record", "cfg": cfg, "rc": rc, "secs": round(secs, 1), "meta": meta, "err": err if rc else ""})

    # 2. Ablate, one process per game, streamed so a job killed at its time
    # cap still leaves every row it reached in the log.
    ablate_deadline = max(60.0, budget - (time.time() - start) - 0.2 * budget)
    rows = {}

    def ablate(d):
        t = time.time()
        p = subprocess.Popen([a.civvis, "core-probe", "ablate", d, str(ablate_deadline)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        lines = []
        for raw in p.stdout:
            if raw.startswith("{"):
                l = json.loads(raw)
                l["game"] = os.path.basename(d)
                lines.append(l)
                emit(l)
        err = p.stderr.read()[-2000:]
        rc = p.wait()
        rows[d] = lines
        emit({"phase": "ablate", "game": os.path.basename(d), "rc": rc, "secs": round(time.time() - t, 1), "rows": len(lines), "err": err if rc else ""})

    with cf.ThreadPoolExecutor(max(1, min(a.jobs, len(dirs)))) as ex:
        list(ex.map(ablate, list(dirs)))

    # 3. Classify across games.
    verdict, modes, samples = {}, {}, {}
    for d, lines in rows.items():
        base = next((l for l in lines if l.get("baseline")), {})
        for l in lines:
            p = l.get("path")
            if not p or p.startswith("<"):
                continue
            c = classify(l, base.get("refused", 0))
            verdict.setdefault(p, []).append(c)
            if l.get("mode") == "zero":
                modes[p] = "zero"
            if c == "core" and p not in samples:
                samples[p] = {k: l.get(k) for k in ("diverged", "diverged_count", "legal_differs", "first_refusal", "load_error", "load_panic", "replay_panic_at") if l.get(k) not in (None, [], False)}
    final = {}
    for p, cs in verdict.items():
        tested = [c for c in cs if c != "untested"]
        final[p] = max(tested, key=lambda c: RANK[c]) if tested else "untested"

    # 4. Static or dynamic, sizes, and how much moves in the window.
    sizes, static, moved = {}, {}, {}
    for d in dirs:
        s0, st, sf = (json.load(open(os.path.join(d, n))) for n in ("state_0.json", "state_t.json", "final.json"))
        for p in final:
            sizes.setdefault(p, []).append(size_of(st, p))
            k0, kt, kf = keyed(s0, p), keyed(st, p), keyed(sf, p)
            static.setdefault(p, []).append(k0 == kt == kf)
            keys = set(kt) | set(kf)
            ch = sum(1 for k in keys if kt.get(k) != kf.get(k))
            moved.setdefault(p, []).append([ch, len(keys)])
    for p in sorted(final, key=lambda p: (-RANK[final[p]], p)):
        emit({"field": p, "class": final[p], "per_game": verdict[p], "bytes": sizes.get(p, []), "static": static.get(p, []), "moved": moved.get(p, []), "mode": modes.get(p, "empty"), "sample": samples.get(p)})

    # Bytes per class, counted at the finest depth so nothing is counted
    # twice: a list of records is priced by its fields.
    leaves = [p for p in final if not any(q != p and (q.startswith(p + "[*].") or q.startswith(p + ".")) for q in final)]
    per_class = {}
    for p in leaves:
        n = sum(sizes.get(p, [])) / max(1, len(sizes.get(p, [])))
        per_class[final[p]] = per_class.get(final[p], 0) + n
    emit({"phase": "class_bytes_mean_per_game", "bytes": {k: round(v) for k, v in per_class.items()}, "leaves": len(leaves)})

    # 5. Sufficiency: blank everything that was not core, all at once.
    strip_paths = [p for p, c in final.items() if c != "core"]
    listfile = os.path.join(work, "strip.txt")
    with open(listfile, "w") as f:
        f.write("\n".join(f"{p}\t{modes.get(p, 'empty')}" for p in strip_paths))

    def strip(d):
        rc, out, err, secs = run([a.civvis, "core-probe", "strip", d, listfile], timeout=900)
        return d, rc, out, err, secs

    with cf.ThreadPoolExecutor(max(1, min(a.jobs, len(dirs)))) as ex:
        for d, rc, out, err, secs in ex.map(strip, list(dirs)):
            line = json.loads(out.strip().splitlines()[-1]) if rc == 0 and out.strip() else None
            emit(
                {
                    "phase": "strip",
                    "game": os.path.basename(d),
                    "rc": rc,
                    "secs": round(secs, 1),
                    "result": line,
                    "gzip": {
                        name: gz(os.path.join(d, name))
                        for name in ("state_0.json", "state_t.json", "final.json", "actions_all.json", "actions.json", "stripped.json")
                        if os.path.exists(os.path.join(d, name))
                    },
                    "err": err if rc else "",
                }
            )

    # 6. What holding a position costs, with and without the non-core fields.
    for d in list(dirs)[:3]:
        for name in ("state_t.json", "stripped.json"):
            path = os.path.join(d, name)
            if os.path.exists(path):
                rc, out, err, secs = run([a.civvis, "core-probe", "bench", path, "300"], timeout=300)
                emit({"phase": "bench", "game": os.path.basename(d), "save": name, "rc": rc, "result": json.loads(out.strip().splitlines()[-1]) if rc == 0 and out.strip() else None, "err": err if rc else ""})
    emit({"phase": "done", "secs": round(time.time() - start, 1)})
    return 0


if __name__ == "__main__":
    sys.exit(main())
