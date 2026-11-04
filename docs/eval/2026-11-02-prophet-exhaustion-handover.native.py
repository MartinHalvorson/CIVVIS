"""Query captured native reads with the original and repaired executables.

Disposable mirrors only. Does not connect to or issue commands to a host.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def rows(raw):
    for line in raw.splitlines():
        yield json.loads(line)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("repo", "root", "before", "after", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    sys.path.insert(0, str(args.repo / "tools"))
    from civ6_decision_trace import captured_input
    from civ6_ladder import decider_genome

    run = args.root / "civvis-20261102T203602Z"
    genome = decider_genome(run / "why.log")
    native_civ = json.loads((run / "summary.json").read_text())["seat"]["civ"]
    tag = "religion-lane-hands-over"
    assert tag in genome["treatments"]
    profile = sorted(genome["forced"])
    assert tag in profile
    (args.out / "observed-genome.json").write_text(json.dumps(genome, indent=2) + "\n")
    decisions_raw = (run / "decisions.jsonl").read_bytes()
    decisions = list(rows(decisions_raw))
    evidence = {"run": run.name, "decisions_sha256": sha(decisions_raw),
                "source_genome_sha256": sha((args.out / "observed-genome.json").read_bytes()),
                "binaries": {name: {"path": str(path.resolve()), "sha256": sha(path.read_bytes())}
                             for name, path in (("before", args.before), ("after", args.after))},
                "scope": "One-shot Religion queries on exact captured reads, not original persistent-process replay, native execution, a race-win claim or strength evidence.",
                "cases": []}
    for turn, frame in ((38, 0), (39, 0), (42, 0), (57, 0)):
        matches = [r for r in decisions if (r["decision"]["turn"], r["decision"]["frame"]) == (turn, frame)]
        assert len(matches) == 1
        decision = matches[0]
        capture = decision["input_capture"]
        assert capture["complete"]
        selected = None
        for read in capture["reads"]:
            raw = captured_input(capture, read["snapshot_id"])
            states = [r for r in rows(raw) if r.get("kind") == "state"]
            if states and (states[-1]["turn"], states[-1].get("frame", 0)) == (turn, frame):
                selected = (raw, read["snapshot_id"], states[-1])
                break
        assert selected is not None
        raw, snapshot, state = selected
        case_dir = args.out / f"t{turn}-frame{frame}"
        case_dir.mkdir()
        (case_dir / "captured-events.jsonl").write_bytes(raw)
        case = {"turn": turn, "frame": frame, "snapshot_id": snapshot,
                "input_sha256": sha(raw), "input_bytes": len(raw),
                "source_decision_sha256": decision["sha256"], "capture": capture,
                "host_exhausted": state.get("great_person_exhausted"),
                "founded_religions": state.get("founded_religions"),
                "prophet_pending": state.get("prophet_pending"),
                "cities": [{"id": c["id"], "name": c["name"], "producing": c.get("producing"),
                            "holy_sites": [d for d in c.get("districts", [])
                                           if d.get("type") == "DISTRICT_HOLY_SITE"]}
                           for c in state["cities"]], "responses": []}
        for label, binary, enabled in (("before-on", args.before, True),
                                       ("after-on", args.after, True),
                                       ("before-on-repeat", args.before, True),
                                       ("before-off", args.before, False),
                                       ("after-off", args.after, False)):
            directory = case_dir / label
            directory.mkdir()
            (directory / "events.jsonl").write_bytes(raw)
            seated = [t for t in profile if enabled or t != tag]
            argv = [str(binary.resolve()), "--mirror", str(directory.resolve()),
                    "--turn", str(turn), "--victory", "religious", "--civ", native_civ, "--fresh-board", "--explain"]
            for treatment in seated:
                argv += ["--with", treatment]
            process = subprocess.run(argv, capture_output=True, timeout=180)
            (directory / "stdout").write_bytes(process.stdout)
            (directory / "stderr").write_bytes(process.stderr)
            response = {"label": label, "argv": argv, "exit_code": process.returncode,
                        "genome": decider_genome(directory / "stderr")}
            if process.returncode == 0:
                response["payload"] = json.loads(process.stdout)
                response["primary"] = response["payload"]["decision"]["victory_portfolio"]["primary"]
            case["responses"].append(response)
            (case_dir / "result.json").write_text(json.dumps(case, indent=2) + "\n")
            assert process.returncode == 0 and response["genome"]
            expected = sorted(t for t in genome["treatments"] if enabled or t != tag)
            assert sorted(response["genome"]["treatments"]) == expected, "observed profile did not reproduce"
            (directory / "events.jsonl").unlink()
        before, after, repeated, before_off, after_off = case["responses"]
        assert before["genome"] == after["genome"]
        assert before_off["genome"] == after_off["genome"]
        assert before["payload"]["orders"] == repeated["payload"]["orders"], "control is unstable"
        assert before_off["payload"]["orders"] == after_off["payload"]["orders"], "off behavior changed"
        old_orders, new_orders = before["payload"]["orders"], after["payload"]["orders"]
        case["removed_orders"] = [o for o in old_orders if o not in new_orders]
        case["added_orders"] = [o for o in new_orders if o not in old_orders]
        case["orders_changed"] = old_orders != new_orders
        case["before_primary"], case["after_primary"] = before["primary"], after["primary"]
        expected = {38: ("religious", "religious"), 39: ("religious", "science"),
                    42: ("religious", "science"), 57: ("science", "science")}
        assert (case["before_primary"], case["after_primary"]) == expected[turn]
        assert before_off["primary"] == after_off["primary"] == "religious"
        case["complete"] = True
        (case_dir / "result.json").write_text(json.dumps(case, indent=2) + "\n")
        evidence["cases"].append(case)
        print(json.dumps({k: case[k] for k in ("turn", "frame", "before_primary", "after_primary", "orders_changed", "removed_orders", "added_orders")}), flush=True)
    (args.out / "summary.json").write_text(json.dumps(evidence, indent=2) + "\n")


if __name__ == "__main__":
    main()
