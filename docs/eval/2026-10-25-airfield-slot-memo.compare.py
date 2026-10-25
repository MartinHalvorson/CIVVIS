"""Fixed-board cost/equivalence probe registered beside this experiment.

This is a cold one-shot planner comparison, not a native-history replay or a
full-game benchmark. The preparation manifest fixes every case before timing.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import statistics
import subprocess
import time

MARKER = "last district slot waits for the airfield"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load():
    return list(os.getloadavg())


def fingerprint(binary):
    matches = list((binary.parent / ".fingerprint").glob("civvis-*/bin-civvis_orders.json"))
    if len(matches) != 1:
        raise ValueError("candidate needs exactly one default-feature Cargo fingerprint")
    return json.loads(matches[0].read_text())


def command(binary, case):
    return ([str(binary), "--mirror", case["directory"], "--fresh-board", "--explain",
             "--turn", str(case["turn"]), "--victory", "domination",
             "--civ", "CIVILIZATION_GRAN_COLOMBIA"]
            + [word for tag in case["forced"] for word in ("--with", tag)]
            + [word for tag in case["withheld"] for word in ("--without", tag)])


def run(binary, case, directory, label):
    mirror = Path(case["directory"])
    why = mirror / "why.log"
    if why.exists():
        why.unlink()  # Only the experiment's disposable mirror, never a live run.
    before = resource.getrusage(resource.RUSAGE_CHILDREN).ru_utime
    start = time.monotonic()
    done = subprocess.run(command(binary, case), capture_output=True, timeout=120)
    wall = time.monotonic() - start
    cpu = resource.getrusage(resource.RUSAGE_CHILDREN).ru_utime - before
    if done.returncode:
        (directory / (label + ".stdout")).write_bytes(done.stdout)
        (directory / (label + ".stderr")).write_bytes(done.stderr)
        raise ValueError(f"{label}: decider exited {done.returncode}")
    json.loads(done.stdout)
    digest = sha(done.stdout)
    output = directory / (digest + ".reply.json")
    if not output.exists():
        output.write_bytes(done.stdout)
    # The one-shot CLI prints its reasoning on stderr; the native brain is
    # what normally writes those lines into why.log.
    journal = done.stderr.decode("utf-8", errors="replace")
    if why.exists():
        journal += why.read_text()
    if label.startswith("qualification"):
        (directory / (label + ".why.log")).write_text(journal)
        (directory / (label + ".stderr")).write_bytes(done.stderr)
    return {"reply_sha256": digest, "cpu_seconds": cpu, "wall_seconds": wall,
            "reservation_active": MARKER in journal, "load": load()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preparation", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--candidate", type=Path)
    args = parser.parse_args()
    manifest = json.loads(args.preparation.read_text())
    baseline = Path(manifest["baseline"]["frozen"])
    assert sha(baseline.read_bytes()) == manifest["baseline"]["sha256"]
    args.out.mkdir(exist_ok=False)
    result = {"scope": "fixed captured boards, fresh one-shot planners; no replay/strength claim",
              "baseline": manifest["baseline"], "cases": [], "measurements": [],
              "started_at": time.time(), "load_start": load(), "status": "qualifying"}

    def save():
        temporary = args.out / "result.json.tmp"
        temporary.write_text(json.dumps(result, indent=2) + "\n")
        os.replace(temporary, args.out / "result.json")

    save()
    try:
        binaries = {"baseline": baseline}
        if args.candidate:
            candidate = args.candidate.resolve()
            candidate_fp = fingerprint(candidate)
            parent_fp = manifest["baseline"]["fingerprint"]
            for key in ("rustc", "features", "profile", "target", "rustflags"):
                if candidate_fp[key] != parent_fp[key]:
                    raise ValueError(f"build fingerprints differ on {key}")
            result["candidate"] = {"path": str(candidate), "sha256": sha(candidate.read_bytes()),
                                   "fingerprint": candidate_fp}
            binaries["candidate"] = candidate
        for index, case in enumerate(manifest["cases"]):
            entry = {"tag": case["tag"], "turn": case["turn"],
                     "input_status": case["status"]}
            result["cases"].append(entry)
            if case["status"] != "input_verified":
                save()
                continue
            if sha((Path(case["directory"]) / "events.jsonl").read_bytes()) != case["input_sha256"]:
                raise ValueError("fixed input changed")
            entry["input_sha256"] = case["input_sha256"]
            entry["qualification"] = {
                arm: run(binary, case, args.out, f"qualification-{index}-{arm}")
                for arm, binary in binaries.items()
            }
            receipts = list(entry["qualification"].values())
            if len({r["reply_sha256"] for r in receipts}) != 1:
                raise ValueError(f"case {index}: complete replies differ before timing")
            entry["active_in_both"] = all(r["reservation_active"] for r in receipts)
            print("qualified", index, case["tag"], case["turn"],
                  "active", entry["active_in_both"], flush=True)
            save()
        if not args.candidate:
            result["status"] = "baseline_qualification_complete"
            save()
            return
        if not any(case.get("active_in_both") for case in result["cases"]):
            raise ValueError("no active reservation case; no optimization measurement")
        for phase, rounds in (("same_binary_control", 3), ("comparison", 5)):
            for repetition in range(rounds):
                for index, case in enumerate(manifest["cases"]):
                    if case["status"] != "input_verified":
                        continue
                    order = ["baseline", "candidate"]
                    if (repetition + index) % 2:
                        order.reverse()
                    readings = {}
                    for arm in order:
                        binary = baseline if phase == "same_binary_control" else binaries[arm]
                        readings[arm] = run(binary, case, args.out,
                                            f"{phase}-{repetition}-{index}-{arm}")
                    record = {"phase": phase, "round": repetition, "case": index,
                              "order": order, **readings}
                    result["measurements"].append(record)
                    expected = result["cases"][index]["qualification"]["baseline"]["reply_sha256"]
                    if any(reading["reply_sha256"] != expected for reading in readings.values()):
                        raise ValueError(f"case {index}: complete reply changed during {phase}")
                    if readings["baseline"]["reservation_active"] != readings["candidate"]["reservation_active"]:
                        raise ValueError("reservation exposure differs")
                    save()
                print(phase, "round", repetition + 1, "complete", flush=True)
        result["summary"] = {}
        for phase in ("same_binary_control", "comparison"):
            totals = []
            for repetition in sorted({r["round"] for r in result["measurements"] if r["phase"] == phase}):
                entries = [r for r in result["measurements"] if r["phase"] == phase and r["round"] == repetition]
                parent = sum(r["baseline"]["cpu_seconds"] for r in entries)
                candidate = sum(r["candidate"]["cpu_seconds"] for r in entries)
                totals.append({"baseline_cpu": parent, "candidate_cpu": candidate,
                               "change_pct": 100 * (candidate / parent - 1)})
            per_case = {}
            for index in range(len(manifest["cases"])):
                entries = [r for r in result["measurements"] if r["phase"] == phase and r["case"] == index]
                if entries:
                    per_case[str(index)] = {"active": result["cases"][index]["active_in_both"],
                        "median_cpu_change_pct": statistics.median(
                            100 * (r["candidate"]["cpu_seconds"] / r["baseline"]["cpu_seconds"] - 1)
                            for r in entries)}
            result["summary"][phase] = {"rounds": totals, "per_case": per_case,
                "median_cpu_change_pct": statistics.median(t["change_pct"] for t in totals)}
        if sha(baseline.read_bytes()) != result["baseline"]["sha256"]:
            raise ValueError("baseline binary changed during the comparison")
        if sha(binaries["candidate"].read_bytes()) != result["candidate"]["sha256"]:
            raise ValueError("candidate binary changed during the comparison")
        result["status"] = "complete_identical_replies"
        result["load_end"] = load()
        save()
        print(json.dumps(result["summary"], indent=2), flush=True)
    except Exception as error:
        result["status"] = "stopped_needs_review"
        result["error"] = repr(error)
        save()
        raise


if __name__ == "__main__":
    main()
