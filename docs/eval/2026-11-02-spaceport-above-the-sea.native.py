"""Captured-read Spaceport queries. Never issues commands to a running host."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def records(raw):
    for line in raw.splitlines():
        try:
            yield json.loads(line)
        except (ValueError, UnicodeDecodeError):
            continue


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["repo", "binary", "root", "profile", "out"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    sys.path.insert(0, str(args.repo / "tools"))
    from civ6_decision_trace import captured_input
    from civ6_ladder import decider_genome
    tag = "spaceport-above-the-sea"
    profile = args.profile.read_bytes()
    bundle = profile.decode().strip()
    assert tag not in bundle.split(",")
    (args.out / "profile.txt").write_bytes(profile)
    cases = [("civvis-20261102T171007Z", t) for t in [139, 140, 176]]
    cases.append(("civvis-20261102T185903Z", 166))
    binary_hash = hashlib.sha256(args.binary.read_bytes()).hexdigest()
    summary = []
    for run, turn in cases:
        matches = [r for r in records((args.root / run / "decisions.jsonl").read_bytes())
                   if r["decision"]["turn"] == turn and r["decision"]["frame"] == 0]
        assert len(matches) == 1
        record = matches[0]
        capture = record["input_capture"]
        assert capture["complete"]
        raw = None
        for read in capture["reads"]:
            candidate = captured_input(capture, read["snapshot_id"])
            states = [r for r in records(candidate) if r.get("kind") == "state"]
            if states and (states[-1]["turn"], states[-1].get("frame", 0)) == (turn, 0):
                raw = candidate
                snapshot = read["snapshot_id"]
                state = states[-1]
                break
        assert raw is not None, "no actual captured read matches the decision identity"
        plots = {}
        for r in records(raw):
            if r.get("kind") == "tiles" and isinstance(r.get("plots"), list):
                for plot in r["plots"]:
                    plots[(plot["x"], plot["y"])] = plot
        directory = args.out / f"{run}-{turn}"
        directory.mkdir()
        (directory / "captured-events.jsonl").write_bytes(raw)
        result = {"complete": False, "run": run, "turn": turn, "frame": 0,
                  "scope": "One-shot Science queries on one actual captured read; no original process replay, host execution, or strength claim",
                  "source_decision_sha256": record["sha256"], "source_capture": capture,
                  "snapshot_id": snapshot, "binary_sha256": binary_hash,
                  "profile_sha256": hashlib.sha256(profile).hexdigest(),
                  "reads": [{"complete": True, "bytes": len(raw),
                             "sha256": hashlib.sha256(raw).hexdigest(), "arms": []}]}
        for arm in ["off", "on", "off-repeat"]:
            output = directory / arm
            output.mkdir()
            (output / "events.jsonl").write_bytes(raw)
            argv = [str(args.binary.resolve()), "--mirror", str(output.resolve()),
                    "--turn", str(turn), "--victory", "science", "--explain"]
            seated = bundle.split(",") if bundle else []
            if arm == "on":
                seated.append(tag)
            for gene in seated:
                argv += ["--with", gene]
            done = subprocess.run(argv, capture_output=True, timeout=180)
            (output / "stdout").write_bytes(done.stdout)
            (output / "stderr").write_bytes(done.stderr)
            response = {"arm": arm, "argv": argv, "exit": done.returncode,
                        "genome": decider_genome(output / "stderr")}
            if done.returncode == 0:
                response["payload"] = json.loads(done.stdout)
            if arm == "off-repeat":
                result["repeat"] = response
            else:
                result["reads"][0]["arms"].append(response)
            (directory / "result.json").write_text(json.dumps(result, indent=2) + "\n")
            (output / "events.jsonl").unlink()
            assert done.returncode == 0 and response["genome"], "query failed"
            assert (tag in response["genome"]["treatments"]) == (arm == "on"), "wrong played arm"
        off, on = result["reads"][0]["arms"]
        result["repeat_orders_equal"] = off["payload"]["orders"] == result["repeat"]["payload"]["orders"]
        assert result["repeat_orders_equal"]
        for key in ["treatments", "forced", "ledger_withheld"]:
            assert sorted(x for x in off["genome"][key] if x != tag) == sorted(
                x for x in on["genome"][key] if x != tag), "another gene changed"
        def without_positions(response):
            return [{**o, "x": None, "y": None}
                    if o.get("verb") == "DISTRICT_SPACEPORT" and o["kind"] in ["produce", "produce_next"]
                    else o for o in response["payload"]["orders"]]
        result["all_other_order_content_equal"] = without_positions(off) == without_positions(on)
        assert result["all_other_order_content_equal"]
        changes = []
        for old, new in zip(off["payload"]["orders"], on["payload"]["orders"]):
            if old == new:
                continue
            city = next(c for c in state["cities"] if c["id"] == new["subject"])
            offer = next(o for o in city["buildable"] if o["t"] == "DISTRICT_SPACEPORT")
            pos = (new["x"], new["y"])
            assert pos in [(p["x"], p["y"]) for p in offer["s"]]
            plot = plots[pos]
            assert plot.get("cl") in [-1, 0] and plot.get("flooded") is False and plot.get("submerged") is False
            changes.append({"before": old, "after": new, "native_plot": plot})
        result["site_changes"] = changes
        result["complete"] = True
        (directory / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        row = {"run": run, "turn": turn, "site_changes": len(changes),
               "orders_otherwise_equal": result["all_other_order_content_equal"]}
        summary.append(row)
        print(json.dumps(row), flush=True)
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")


if __name__ == "__main__":
    main()
