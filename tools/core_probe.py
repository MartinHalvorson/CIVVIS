#!/usr/bin/env python3
"""Which saved fields does the CIVVIS transition function itself read?

Drives `civvis core-probe` (src/core_probe.rs) over a few seeded games:
record each, blank every field of its turn-T save one at a time, replay the
recorded actions, and classify each field by what blanking it did:

  core         something else in the end state changed, or actions were
               refused, or the engine could not load/replay without it
  regenerated  nothing else changed and the field came back by itself
               (a cache or a value recomputed from the rest)
  inert        nothing else changed and the field stayed different: the
               transition never reads it (a ledger, a log, an agent's memory,
               a mirror of the real game)
  untested     already empty in every save, so blanking it tested nothing

Then blanks every non-core field at once (`strip`) to check that what is left
is sufficient on its own.

  python3 tools/core_probe.py --civvis target/ci/civvis [--budget-minutes 26]
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

# (players, seed, T, K). Online speed; T is a target, the record step may
# stop earlier on a slow host.
CONFIGS = [(4, 101, 110, 15), (4, 102, 90, 15), (6, 103, 80, 15), (4, 104, 140, 15)]

PREFIX = "CPROBE "


def emit(obj):
    print(PREFIX + json.dumps(obj, separators=(",", ":")), flush=True)


def run(cmd, timeout):
    t = time.time()
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    return p.returncode, p.stdout, p.stderr[-2000:], time.time() - t


def path_values(save, path):
    if "[*]." in path:
        outer, leaf = path.split("[*].", 1)
        if "." in outer:
            k, s = outer.split(".", 1)
            items = (save.get(k) or {}).get(s) or []
        else:
            items = save.get(outer) or []
        return [i.get(leaf) for i in items if isinstance(i, dict)]
    return [save.get(path)]


def size_of(save, path):
    return sum(len(json.dumps(v, separators=(",", ":"))) for v in path_values(save, path) if v is not None)


def gz(path):
    with open(path, "rb") as f:
        return len(gzip.compress(f.read(), 6))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--civvis", required=True)
    ap.add_argument("--budget-minutes", type=float, default=26.0)
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    ap.add_argument("--work", default=None)
    a = ap.parse_args()
    start = time.time()
    budget = a.budget_minutes * 60
    work = a.work or tempfile.mkdtemp(prefix="core-probe-")
    emit({"phase": "start", "work": work, "jobs": a.jobs, "budget_s": budget})

    # 1. Record. Each game gets 40% of the budget to reach its T.
    record_budget = 0.40 * budget
    dirs = {}

    def record(cfg):
        players, seed, t, k = cfg
        d = os.path.join(work, f"p{players}-s{seed}")
        rc, out, err, secs = run(
            [a.civvis, "core-probe", "record", str(players), str(seed), str(t), str(k), d, str(record_budget)],
            timeout=record_budget + 0.25 * budget,
        )
        return cfg, d, rc, out, err, secs

    with cf.ThreadPoolExecutor(min(a.jobs, len(CONFIGS))) as ex:
        for cfg, d, rc, out, err, secs in ex.map(record, CONFIGS):
            meta = None
            if rc == 0 and out.strip():
                meta = json.loads(out.strip().splitlines()[-1])
                dirs[d] = meta
            emit({"phase": "record", "cfg": cfg, "rc": rc, "secs": round(secs, 1), "meta": meta, "err": err if rc else ""})

    # 2. Ablate, one process per game, all in parallel.
    elapsed = time.time() - start
    ablate_deadline = max(60.0, budget - elapsed - 0.15 * budget)
    rows = {}

    def ablate(d):
        # Streamed: a job killed at its time cap still leaves every row it
        # reached in the log.
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

    # 3. Classify across games, and price each field in bytes.
    sizes = {}
    for d in dirs:
        save = json.load(open(os.path.join(d, "state_t.json")))
        for l in rows.get(d, []):
            p = l.get("path")
            if p and not p.startswith("<"):
                sizes.setdefault(p, []).append(size_of(save, p))
    verdict = {}
    for d, lines in rows.items():
        for l in lines:
            p = l.get("path")
            if not p or p.startswith("<"):
                continue
            if l.get("trivial"):
                c = "untested"
            elif "load_error" in l or l.get("load_panic") or "replay_panic_at" in l:
                c = "core"
            elif l.get("diverged") or l.get("refused", 0) != base_refused(lines):
                c = "core"
            elif l.get("restored"):
                c = "regenerated"
            else:
                c = "inert"
            verdict.setdefault(p, []).append(c)
    rank = {"core": 3, "inert": 2, "regenerated": 1, "untested": 0}
    final = {}
    for p, cs in verdict.items():
        tested = [c for c in cs if c != "untested"]
        if not tested:
            final[p] = "untested"
        elif "core" in tested:
            final[p] = "core"
        elif "inert" in tested:
            final[p] = "inert"
        else:
            final[p] = "regenerated"
    for p in sorted(final, key=lambda p: (-rank[final[p]], p)):
        emit({"field": p, "class": final[p], "per_game": verdict[p], "bytes": sizes.get(p, [])})
    totals = {}
    for p, c in final.items():
        if "[*]" in p or "." in p:
            continue  # top-level rows already include their children
        totals.setdefault(c, 0)
        totals[c] += sum(sizes.get(p, [])) // max(1, len(sizes.get(p, [])))
    emit({"phase": "totals_top_level_mean_bytes", "totals": totals})

    # 4. Sufficiency: blank everything that was not core, all at once.
    strip_paths = [p for p, c in final.items() if c in ("inert", "regenerated", "untested")]
    # A top-level field that is core may still contain non-core children;
    # those are listed as children and blanked individually.
    listfile = os.path.join(work, "strip.txt")
    with open(listfile, "w") as f:
        f.write("\n".join(strip_paths))

    def strip(d):
        rc, out, err, secs = run([a.civvis, "core-probe", "strip", d, listfile], timeout=600)
        return d, rc, out, err, secs

    with cf.ThreadPoolExecutor(max(1, min(a.jobs, len(dirs)))) as ex:
        for d, rc, out, err, secs in ex.map(strip, list(dirs)):
            line = json.loads(out.strip().splitlines()[-1]) if rc == 0 and out.strip() else None
            sp = os.path.join(d, "stripped.json")
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
    emit({"phase": "done", "secs": round(time.time() - start, 1)})
    return 0


def base_refused(lines):
    for l in lines:
        if l.get("baseline"):
            return l.get("refused", 0)
    return 0


if __name__ == "__main__":
    sys.exit(main())
