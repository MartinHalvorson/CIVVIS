#!/usr/bin/env python3
"""Manually reproduce the fixed opening-prefix experiment with a supplied CLI.

This runs only the planner against captured bytes in a new output directory.
It never launches Civilization VI or continues beyond the first changed order.
"""
from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
import subprocess
import sys

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "tools"))
import civ6_captured_read_replay as replay
from civ6_ladder import decider_genome


def run_on(context, records, mapping, directory):
    directory.mkdir()
    command, cwd = replay.replay_command(context, directory)
    rows, process, feeder = [], None, None
    with (directory / "stderr.log").open("w") as stderr, \
            (directory / "replies.jsonl").open("w") as replies:
        try:
            process = subprocess.Popen(
                command, cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=stderr, text=True, bufsize=1,
            )
            for record in records:
                key = replay.frame_key(record)
                capture = dict(record["input_capture"], directory=str(
                    mapping[record["input_capture"]["directory"]]))
                expected = replay.expanded_reads(capture)
                assert expected and all(data == expected[0] for data in expected)
                path = directory / "events.jsonl"
                if path.exists():
                    path.unlink()
                feeder = replay.deliver_reads(path, expected, 30, "hybrid")
                process.stdin.write(f"{key[0]}\n")
                process.stdin.flush()
                reply = replay.request_reply(process, 30)
                replies.write(json.dumps(reply, sort_keys=True) + "\n")
                replies.flush()
                feeder.finish()
                feeder = None
                actual = replay.expanded_reads(reply["input_capture"])
                assert actual and all(data == expected[0] for data in actual)
                assert not replay.request_identity_errors(reply, *key)
                row = {
                    "request": key,
                    "native_orders_match": record["orders"] == reply["orders"],
                    "expected_read_count": len(expected),
                    "actual_read_count": len(actual),
                    "input_sha256": replay.sha(expected[0]),
                    "payload": replay.control_payload(reply),
                }
                rows.append(row)
                if not row["native_orders_match"]:
                    changed = copy.deepcopy(record["orders"])
                    count = 0
                    for order in changed:
                        if order["kind"] == "produce" and order["subject"] == 65536:
                            assert order["verb"] == "UNIT_SCOUT"
                            order["verb"] = "UNIT_SLINGER"
                            count += 1
                    assert count == 1 and changed == reply["orders"]
                    break
            assert rows[-1]["request"] == (1, 2)
            assert not rows[-1]["native_orders_match"]
            process.stdin.close()
            assert process.wait(timeout=5) == 0
        finally:
            if feeder is not None:
                feeder.close()
            if process is not None and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            if process is not None:
                for stream in (process.stdin, process.stdout):
                    if stream is not None and not stream.closed:
                        stream.close()
    return {"rows": rows, "genome": decider_genome(directory / "stderr.log")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    fixture = Path(__file__).with_name("fixture")
    manifest = json.loads((fixture / "manifest.json").read_text())
    for name, field in [("bytes.bin", "archive_sha256"),
                        ("snapshots.jsonl", "index_sha256"),
                        ("native-decisions.jsonl", "native_decisions_sha256")]:
        assert replay.sha((fixture / name).read_bytes()) == manifest[field]
    records = [json.loads(line) for line in
               (fixture / "native-decisions.jsonl").read_text().splitlines()]
    assert [replay.frame_key(r) for r in records] == [(1, 0), (1, 1), (1, 2)]
    for record in records:
        unsigned = {key: value for key, value in record.items() if key != "sha256"}
        assert replay.sha(json.dumps(unsigned, sort_keys=True,
                          separators=(",", ":"), allow_nan=False).encode()) == record["sha256"]
    binary = args.binary.resolve()
    digest = replay.sha(binary.read_bytes())
    context = {
        "scope": "Candidate replay command adapted from historical Native arguments.",
        "processes": [{"argv": [str(binary), *manifest["argv_without_binary"]],
                       "cwd": str(REPO)}],
        "binary_sha256": digest,
        "provenance": {"sha256": digest},
    }
    mapping = {r["input_capture"]["directory"]: fixture for r in records}
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    report = {"scope": "Fixed prefix through first changed host order; no outcome test.",
              "binary_sha256": digest, "off": [], "on": [], "complete": False}
    try:
        for repeat in range(2):
            result = replay.run_control(context, records, mapping,
                                        out / f"off-{repeat + 1}", 30, "hybrid")
            report["off"].append(result)
            assert result["passed"], result
            played = decider_genome(out / f"off-{repeat + 1}" / "stderr.log")
            assert sorted(played["treatments"]) == sorted(manifest["native_treatments"])
        context["processes"][0]["argv"] += ["--with", "opening-ranged-support"]
        for repeat in range(2):
            result = run_on(context, records, mapping, out / f"on-{repeat + 1}")
            report["on"].append(result)
            assert sorted(result["genome"]["treatments"]) == sorted(
                manifest["native_treatments"] + ["opening-ranged-support"])
        assert replay.exact_fact(report["on"][0]["rows"], report["on"][1]["rows"])
        report["complete"] = True
    finally:
        (out / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"complete": True, "off_repeats": 2, "on_repeats": 2,
                      "requests_each": 3, "first_change": [1, 2],
                      "change": "UNIT_SCOUT -> UNIT_SLINGER"}))


if __name__ == "__main__":
    main()
