"""Bound the registered forty-eight-start trial; never stop or restart a game.

Run from a CIVVIS worktree after an unarmed delivered-feature game has started:
  python3 <this-file> --repo <worktree> --repair <merged-feature-sha> \
      --qualification-run <tag> --out <new-evidence-directory>

The frozen attempt directory is the durable assignment witness, including a
start that fails before producing a native state or ledger row. Continuations
share their stem. A missing played-arm readback is retained, never replaced.
This observer ends at the start cutoff; outcome collection is separate work.
"""
from __future__ import annotations

import argparse
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time
import tempfile

GENE = "forward-airfield-project"
BASELINE_GENE = "capital-walls-before-the-surprise"
LIMIT = 48
TAG = re.compile(r"^(civvis-\d{8}T\d{6}Z)(?:-cont\d+)?$")


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def stem(tag):
    match = TAG.fullmatch(tag)
    return match[1] if match else None


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.replace(temporary, path)


def rows(path):
    if not path.exists():
        return []
    result = []
    with path.open() as stream:
        for line in stream:
            try:
                result.append(json.loads(line))
            except ValueError:
                continue  # A live writer may still be appending the final row.
    return result


def drivers():
    result = []
    output = subprocess.check_output(["ps", "-axo", "pid=,command="], text=True)
    for line in output.splitlines():
        parts = line.strip().split(None, 1)
        if len(parts) != 2:
            continue
        try:
            words = shlex.split(parts[1])
        except ValueError:
            continue
        if not any(Path(word).name == "civ6_play.py" for word in words):
            continue
        if "--tag" not in words:
            continue
        tag = words[words.index("--tag") + 1]
        if stem(tag):
            result.append({"pid": int(parts[0]), "tag": tag,
                           "command": parts[1], "argv": words})
    return result


def verify_forceable(binary):
    """Probe an empty disposable mirror; never open the real orders channel."""
    from civ6_ladder import decider_genome
    with tempfile.TemporaryDirectory(prefix="civvis-empty-forward-field-arm-") as tmp:
        directory = Path(tmp)
        command = [str(binary), "--mirror", str(directory), "--with", GENE,
                   "--victory", "domination", "--turn", "1"]
        done = subprocess.run(command, capture_output=True, text=True, timeout=30)
        log = directory / "probe.stderr.log"
        log.write_text(done.stderr)
        played = decider_genome(log)
        if done.returncode != 0 or played is None or GENE not in played.get("treatments", []):
            raise ValueError("delivered binary cannot seat the candidate on an empty mirror: " +
                             json.dumps({"command": command, "exit_code": done.returncode,
                                         "stdout": done.stdout, "stderr": done.stderr}))
        return {"scope": "empty disposable mirror capability probe, not native execution",
                "command": command, "exit_code": done.returncode,
                "stdout": done.stdout, "stderr": done.stderr, "played_genome": played}


def qualify(repo, root, tag, repair):
    sys.path.insert(0, str(repo / "tools"))
    from civ6_decision_trace import captured_input
    from civ6_ladder import decider_genome

    actual = [d for d in drivers() if d["tag"] == tag]
    if len(actual) != 1:
        raise ValueError("qualification needs exactly one actual unarmed driver")
    words = actual[0]["argv"]
    if "--civvis-capture-inputs" not in words or "--screen-gene" in words:
        raise ValueError("qualification driver must capture inputs and be unarmed")
    run = root / "control" / tag
    updates = rows(run / "runtime_updates.jsonl")
    starts = [r for r in updates if r.get("status") == "start"]
    if len(starts) != 1:
        raise ValueError("qualification lacks a unique frozen runtime receipt")
    runtime = starts[0]
    revision = runtime["binary_revision"]
    subprocess.run(["git", "-C", str(repo), "merge-base", "--is-ancestor",
                    repair, revision], check=True)
    binary = Path(runtime["binary"])
    if hashlib.sha256(binary.read_bytes()).hexdigest() != runtime["binary_sha256"]:
        raise ValueError("qualification binary differs from its runtime receipt")
    decisions = rows(run / "decisions.jsonl")
    if not decisions:
        raise ValueError("qualification has no decision yet")
    first = decisions[0]
    if first["binary_on_disk_sha256"] != runtime["binary_sha256"]:
        raise ValueError("qualification decision uses a different binary")
    genome = decider_genome(run / "why.log")
    if (genome is None or BASELINE_GENE not in genome.get("treatments", [])
            or GENE in genome.get("treatments", [])):
        raise ValueError("qualification must retain the baseline watch with the candidate treatment absent")
    forceable = verify_forceable(binary)
    capture = first["input_capture"]
    if not capture["complete"] or not capture["reads"]:
        raise ValueError("qualification needs a complete first request")
    if any("error" in read for read in capture["reads"]):
        raise ValueError("qualification contains a failed read")
    snapshots = {}
    for read in capture["reads"]:
        sid = read["snapshot_id"]
        data = captured_input(capture, sid)
        snapshots[str(sid)] = {"bytes": len(data),
                               "sha256": hashlib.sha256(data).hexdigest()}
    # The installed bytes can be observed now, but the loader's version=2
    # event does not attest which bytes an already-running game loaded.
    try:
        from civ6_control import install
        mod = install.install_dir() / "CivvisControlAgent.lua"
        data = mod.read_bytes()
        mod_snapshot = {"path": str(mod), "read_utc": utc(), "bytes": len(data),
                        "sha256": hashlib.sha256(data).hexdigest(),
                        "text": data.decode(), "loaded_identity_attested": False}
    except (OSError, ValueError) as error:
        mod_snapshot = {"error": str(error), "loaded_identity_attested": False}
    return {"utc": utc(), "driver": actual[0], "runtime": runtime,
            "first_decision": first, "reconstructed_reads": snapshots,
            "played_genome": genome, "candidate_capability_probe": forceable,
            "installed_mod_on_disk": mod_snapshot,
            "loaded_events": [r for r in rows(run / "events.jsonl") if r.get("kind") == "loaded"]}


def identity(path):
    stat = path.stat()
    return [stat.st_dev, stat.st_ino, stat.st_mtime_ns, stat.st_ctime_ns]


def cut_off(screen, expected_identity):
    if not screen.exists():
        return "already_absent"
    if identity(screen) != expected_identity or screen.read_text().strip() != GENE:
        return "foreign_or_changed_screen_preserved"
    screen.unlink()
    return "removed_owned_screen"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--repair", required=True)
    parser.add_argument("--qualification-run", required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    root = Path.home() / "civvis-civ6-runs"
    snapshots = root / "decider-snapshots"
    screen = Path.home() / ".civvis-live-screen-gene"
    args.out.mkdir(parents=True, exist_ok=True)
    with (args.out / "observer.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        state_path = args.out / "state.json"
        if state_path.exists():
            state = json.loads(state_path.read_text())
            if (state["repair"] != args.repair or state.get("gene") != GENE
                    or state.get("limit") != LIMIT):
                raise ValueError("resumed observer has a different source, gene or cutoff")
        else:
            qualification = qualify(args.repo.resolve(), root, args.qualification_run,
                                    args.repair)
            write_json(args.out / "qualification.json", qualification)
            baseline = sorted({stem(p.name) for p in snapshots.iterdir() if stem(p.name)})
            state = {"repair": args.repair, "gene": GENE, "limit": LIMIT,
                     "qualification_run": args.qualification_run,
                     "baseline": baseline, "assigned": [], "overruns": [],
                     "activation_utc": utc(), "cutoff": None}
            # Exclusive creation never replaces another owner's screen.
            with screen.open("x") as stream:
                stream.write(GENE + "\n")
                stream.flush()
                os.fsync(stream.fileno())
            state["screen_identity"] = identity(screen)
            write_json(state_path, state)
            print("armed", state["activation_utc"], flush=True)
        known = set(state["baseline"]) | set(state["assigned"]) | set(state["overruns"])
        seen_drivers = set()
        while state["cutoff"] is None:
            current = sorted({stem(p.name) for p in snapshots.iterdir() if stem(p.name)})
            new = [tag for tag in current if tag not in known]
            for tag in new:
                known.add(tag)
                destination = "assigned" if len(state["assigned"]) < LIMIT else "overruns"
                state[destination].append(tag)
                print(destination, tag, flush=True)
            # Persist assignments before any transient process inspection.
            write_json(state_path, state)
            if len(state["assigned"]) == LIMIT:
                state["cutoff"] = {"utc": utc(),
                                   "action": cut_off(screen, state["screen_identity"])}
                write_json(state_path, state)
                print("start cutoff", state["cutoff"], flush=True)
            try:
                actual = drivers()
            except (OSError, subprocess.SubprocessError) as error:
                actual = []
                print("process observation failed", repr(error), flush=True)
            for driver in actual:
                key = (driver["pid"], driver["tag"])
                if stem(driver["tag"]) not in state["assigned"] or key in seen_drivers:
                    continue
                seen_drivers.add(key)
                with (args.out / "drivers.jsonl").open("a") as stream:
                    stream.write(json.dumps({"utc": utc(), **driver}) + "\n")
            if state["cutoff"] is None:
                time.sleep(1)
        print("Forty-eight starts retained; collect every continuation and terminal outcome.", flush=True)


if __name__ == "__main__":
    main()
