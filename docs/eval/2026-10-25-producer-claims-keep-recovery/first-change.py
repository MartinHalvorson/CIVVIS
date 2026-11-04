"""Measure the registered first planning change on unchanged native history."""
import argparse
import copy
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--harness", type=Path, required=True)
    parser.add_argument("--context", type=Path, required=True)
    parser.add_argument("--qualification", type=Path, required=True)
    parser.add_argument("--decisions", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--registration", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    spec = importlib.util.spec_from_file_location("constant_read_control", args.harness)
    replay = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(replay)
    report = {"scope": "Two local first-change witnesses on fixed recorded native observations. No later world reaction, continuation or native strength estimate.",
              "qualified": False, "comparisons": []}
    try:
        registration = json.loads(args.registration.read_text())
        expected = tuple(registration["expected_first_changed_request"])
        original = json.loads(args.qualification.read_text())
        assert original["passed"] and len(original["controls"]) == 2
        assert all(c["passed"] for c in original["controls"])
        context = json.loads(args.context.read_text())
        records = replay.select_records(args.decisions, original["through_turn"], context)
        native = next(q for q in records if replay.frame_key(q) == expected)
        assert any(a.get("type") == "produce" and a.get("city") == 33
                   and a.get("item", {}).get("district") == "industrial_zone"
                   for a in native["decision"]["native_actions"])
        candidate = copy.deepcopy(context)
        candidate["binary_sha256"] = sha(args.binary)
        candidate["processes"][0]["argv"][0] = str(args.binary.resolve())
        candidate["provenance"] = {"kind": "private_historical_recovery_backport", "sha256": candidate["binary_sha256"]}
        (args.out / "candidate-context.json").write_text(json.dumps(candidate, indent=2) + "\n")
        report.update(registration=registration, registration_sha256=sha(args.registration),
                      original_qualification_sha256=sha(args.qualification),
                      original_decisions_sha256=sha(args.decisions),
                      candidate_binary_sha256=candidate["binary_sha256"])
        mapping, report["sealed_captures"] = replay.seal_captures(records, args.out)
        changed = []
        for number in range(2):
            directory = args.out / f"comparison-{number}"
            result = replay.run_control(candidate, records, mapping, directory, 120)
            report["comparisons"].append(result)
            assert not result["passed"] and tuple(result["failed_request"]) == expected
            checks = result["requests"]
            assert checks and tuple(checks[-1]["request"]) == expected
            assert all(not q["identity_errors"] and q["read_sequence_matches"] for q in checks)
            assert all(q["native_payload_matches"] for q in checks[:-1])
            assert not checks[-1]["native_payload_matches"]
            reply = json.loads((directory / "replies.jsonl").read_text().splitlines()[-1])
            assert not any(a.get("type") == "produce" and a.get("city") == 33
                           and a.get("item", {}).get("district") == "industrial_zone"
                           for a in reply["decision"]["native_actions"])
            changed.append(replay.control_payload(reply))
        assert changed[0] == changed[1], "the changed complete payloads differ"
        report.update(qualified=True, first_changed_request=expected,
                      identical_complete_requests_before_change=len(report["comparisons"][0]["requests"]) - 1,
                      original_changed_payload=replay.control_payload(native), candidate_changed_payload=changed[0])
    except Exception as error:
        report["error"] = f"{type(error).__name__}: {error}"
    data = json.dumps(report, sort_keys=True).encode()
    (args.out / "comparison.json.gz").write_bytes(gzip.compress(data, mtime=0))
    (args.out / "summary.json").write_text(json.dumps({k: v for k, v in report.items()
        if k not in ("comparisons", "sealed_captures", "original_changed_payload", "candidate_changed_payload")}, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k in
        ("qualified", "first_changed_request", "identical_complete_requests_before_change", "error")}), flush=True)
    if report["qualified"]:
        for name in ("original_changed_payload", "candidate_changed_payload"):
            print(name, json.dumps([a for a in report[name]["decision"]["native_actions"]
                                   if a.get("type") == "produce"]))
    return 0 if report["qualified"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
