"""One-call hoisted-scan planner diagnostic; no whole-game or strength claim."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import time


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    preparation = args.root / "sim-preparation.json"
    manifest = json.loads(preparation.read_text())
    binaries = {"parent": args.root / "parent-sim-probe-observed",
                "candidate": args.root / "candidate-hoisted-sim-probe"}
    names = {"parent": "parent-sim-probe-observed-build.json",
             "candidate": "candidate-hoisted-sim-probe-build.json"}
    builds = {arm: json.loads((args.root / names[arm]).read_text()) for arm in binaries}
    parent_fps = list((args.root / "parent-source/target/ci/.fingerprint").glob(
        "civvis-*/lib-civvis.json"))
    assert len(parent_fps) == 1
    parent_fp = json.loads(parent_fps[0].read_text())
    library_build = json.loads((args.root / "hoisted-library-build.json").read_text())
    assert builds["candidate"]["library_sha256"] == library_build["library_sha256"]
    for key in ("rustc", "features", "profile", "target", "rustflags"):
        if parent_fp[key] != library_build["fingerprint"][key]:
            raise ValueError(f"library fingerprints differ on {key}")
    if len({build["source_sha256"] for build in builds.values()}) != 1:
        raise ValueError("probe source differs")
    for arm, binary in binaries.items():
        if sha(binary.read_bytes()) != builds[arm]["binary_sha256"]:
            raise ValueError(f"{arm} binary changed")
    args.out.mkdir(exist_ok=False)
    result = {"scope": __doc__, "builds": builds, "cases": [], "measurements": [],
              "started_at": time.time(), "load_start": os.getloadavg(), "status": "qualifying"}

    def save():
        temp = args.out / "result.json.tmp"
        temp.write_text(json.dumps(result, indent=2) + "\n")
        os.replace(temp, args.out / "result.json")

    def run(arm, index, label):
        try:
            done = subprocess.run([str(binaries[arm]), str(preparation), str(index), "1"],
                                  capture_output=True, timeout=120)
        except subprocess.TimeoutExpired as error:
            (args.out / f"{label}.stdout").write_bytes(error.stdout or b"")
            (args.out / f"{label}.stderr").write_bytes(error.stderr or b"")
            raise
        if done.returncode:
            (args.out / f"{label}.stdout").write_bytes(done.stdout)
            (args.out / f"{label}.stderr").write_bytes(done.stderr)
            raise ValueError(f"{label}: exit {done.returncode}")
        output = json.loads(done.stdout)
        assert output["case"] == index and len(output["samples"]) == 1
        semantic = json.dumps(output["semantic"], sort_keys=True, separators=(",", ":")).encode()
        digest = sha(semantic)
        path = args.out / f"{digest}.semantic.json"
        if not path.exists():
            path.write_bytes(semantic)
        return {"semantic_sha256": digest, "samples": output["samples"],
                "load": os.getloadavg(), "binary_arm": arm}

    def ticks(reading):
        return sum(sample["process_cpu_ticks"] for sample in reading["samples"])

    save()
    try:
        for index, case in enumerate(manifest["cases"]):
            if case["status"] != "input_verified":
                result["cases"].append({"tag": case["tag"], "turn": case["turn"],
                                        "status": case["status"], "active": False})
                save()
                continue
            if sha(Path(case["input_file"]).read_bytes()) != case["input_sha256"]:
                raise ValueError(f"fixed case {index} input changed")
            readings = {arm: run(arm, index, f"qualification-{index}-{arm}") for arm in binaries}
            if len({r["semantic_sha256"] for r in readings.values()}) != 1:
                raise ValueError(f"case {index}: parent/candidate semantics differ")
            exposure = {arm: [s["reservation_active"] for s in r["samples"]]
                        for arm, r in readings.items()}
            if exposure["parent"] != exposure["candidate"]:
                raise ValueError(f"case {index}: exposure differs")
            result["cases"].append({"tag": case["tag"], "turn": case["turn"],
                                    "input_sha256": case["input_sha256"],
                                    "active": all(exposure["parent"]), "qualification": readings})
            save()
            print("qualified", index, "active", result["cases"][-1]["active"], flush=True)
        if not any(case["active"] for case in result["cases"]):
            raise ValueError("no active reservation case")
        result["status"] = "measuring"
        save()
        for phase, rounds in (("same_binary_control", 3), ("comparison", 5)):
            for repetition in range(rounds):
                for index in range(len(manifest["cases"])):
                    if manifest["cases"][index]["status"] != "input_verified":
                        continue
                    order = ["parent", "candidate"]
                    if (index + repetition) % 2:
                        order.reverse()
                    readings = {arm: run("parent" if phase == "same_binary_control" else arm,
                                         index, f"{phase}-{repetition}-{index}-{arm}") for arm in order}
                    record = {"phase": phase, "round": repetition, "case": index,
                              "order": order, **readings}
                    result["measurements"].append(record)
                    expected = result["cases"][index]["qualification"]["parent"]
                    for reading in readings.values():
                        if reading["semantic_sha256"] != expected["semantic_sha256"]:
                            raise ValueError(f"case {index}: semantics changed during {phase}")
                        if [s["reservation_active"] for s in reading["samples"]] != [
                                s["reservation_active"] for s in expected["samples"]]:
                            raise ValueError(f"case {index}: exposure changed during {phase}")
                    save()
                print(phase, "round", repetition + 1, "complete", flush=True)
        result["summary"] = {}
        for phase in ("same_binary_control", "comparison"):
            entries = [r for r in result["measurements"] if r["phase"] == phase]
            rounds = []
            for repetition in sorted({r["round"] for r in entries}):
                rows = [r for r in entries if r["round"] == repetition]
                parent = sum(ticks(r["parent"]) for r in rows)
                candidate = sum(ticks(r["candidate"]) for r in rows)
                rounds.append({"parent_ticks": parent, "candidate_ticks": candidate,
                               "change_pct": 100 * (candidate / parent - 1)})
            per_case = {}
            for index, case in enumerate(result["cases"]):
                rows = [r for r in entries if r["case"] == index]
                if not rows:
                    continue
                per_case[str(index)] = {"active": case["active"], "median_cpu_change_pct":
                    statistics.median(100 * (ticks(r["candidate"]) / ticks(r["parent"]) - 1)
                                      for r in rows)}
            result["summary"][phase] = {"rounds": rounds, "per_case": per_case,
                "median_cpu_change_pct": statistics.median(r["change_pct"] for r in rounds)}
        for phase in ("same_binary_control", "comparison"):
            strata = {}
            for active in (False, True):
                entries = [r for r in result["measurements"] if r["phase"] == phase
                           and result["cases"][r["case"]]["active"] == active]
                rounds = []
                for repetition in sorted({r["round"] for r in entries}):
                    rows = [r for r in entries if r["round"] == repetition]
                    parent = sum(ticks(r["parent"]) for r in rows)
                    candidate = sum(ticks(r["candidate"]) for r in rows)
                    rounds.append({"parent_ticks": parent, "candidate_ticks": candidate,
                                   "change_pct": 100 * (candidate / parent - 1)})
                strata["active" if active else "inactive"] = {
                    "cases": len({r["case"] for r in entries}), "rounds": rounds,
                    "median_cpu_change_pct": statistics.median(r["change_pct"] for r in rounds)
                                             if rounds else None}
            result["summary"][phase]["exposure_strata"] = strata
        for arm, binary in binaries.items():
            if sha(binary.read_bytes()) != builds[arm]["binary_sha256"]:
                raise ValueError(f"{arm} binary changed during comparison")
        result["status"] = "complete_identical_semantics"
        result["load_end"] = os.getloadavg()
        save()
        print(json.dumps(result["summary"], indent=2), flush=True)
    except Exception as error:
        result.update(status="stopped_needs_review", error=repr(error))
        save()
        raise


if __name__ == "__main__":
    main()
