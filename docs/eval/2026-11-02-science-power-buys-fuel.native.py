"""Same-input native purchase proposals, never orders sent to a running host."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    sys.path.insert(0, str(args.repo / "tools"))
    from civ6_decision_trace import captured_input
    from civ6_ladder import decider_genome
    tag = "science-power-buys-fuel"
    turns = [150, 152, 155, 161, 184]
    decisions = {}
    for line in (args.run / "decisions.jsonl").open():
        record = json.loads(line)
        decision = record["decision"]
        if decision["turn"] in turns and decision["frame"] == 0:
            if decision["turn"] in decisions:
                raise ValueError("duplicate native frame identity")
            decisions[decision["turn"]] = record
    summary = []
    for turn in turns:
        record = decisions[turn]
        capture = record["input_capture"]
        if not capture["complete"]:
            raise ValueError("incomplete native capture")
        snapshot = capture["reads"][0]["snapshot_id"]
        raw = captured_input(capture, snapshot)
        root = args.out / f"case-{turn}"
        root.mkdir()
        (root / "captured-events.jsonl").write_bytes(raw)
        result = {"complete": False, "turn": turn,
                  "scope": "One-shot proposals on one actual captured read; not original process replay, host execution, or strength",
                  "source_decision_sha256": record["sha256"],
                  "source_capture": capture, "snapshot_id": snapshot,
                  "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                  "reads": [{"complete": True, "bytes": len(raw),
                             "sha256": hashlib.sha256(raw).hexdigest(), "arms": []}]}
        (root / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        for arm in ["off", "on", "off-repeat"]:
            directory = root / arm
            directory.mkdir()
            (directory / "events.jsonl").write_bytes(raw)
            argv = [str(args.binary.resolve()), "--mirror", str(directory.resolve()),
                    "--turn", str(turn), "--victory", "science", "--explain"]
            if arm == "on":
                argv += ["--with", tag]
            done = subprocess.run(argv, capture_output=True, timeout=120)
            (directory / "stdout").write_bytes(done.stdout)
            (directory / "stderr").write_bytes(done.stderr)
            response = {"arm": arm, "argv": argv, "exit": done.returncode,
                        "genome": decider_genome(directory / "stderr")}
            if done.returncode == 0:
                response["payload"] = json.loads(done.stdout)
            if arm == "off-repeat":
                result["repeat"] = response
            else:
                result["reads"][0]["arms"].append(response)
            (directory / "events.jsonl").unlink()
            (root / "result.json").write_text(json.dumps(result, indent=2) + "\n")
            if done.returncode != 0 or not response["genome"]:
                raise RuntimeError(f"query failed at {turn}/{arm}")
        off, on = result["reads"][0]["arms"]
        result["repeat_orders_equal"] = off["payload"]["orders"] == result["repeat"]["payload"]["orders"]
        result["complete"] = result["repeat_orders_equal"]
        (root / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        if not result["complete"]:
            raise RuntimeError("unstable control proposals")
        row = {"turn": turn, "changed_orders": off["payload"]["orders"] != on["payload"]["orders"],
               "buys": {a["arm"]: [order for order in a["payload"]["orders"] if order["kind"] == "buy"] for a in (off, on)}}
        summary.append(row)
        print(json.dumps(row), flush=True)
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
