"""Qualify a private diagnostic binary against the unchanged native history."""
import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--context", type=Path, required=True)
    parser.add_argument("--original-qualification", type=Path, required=True)
    parser.add_argument("--decisions", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--through-turn", type=int, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location("constant_read_control", Path(__file__).with_name("constant-read-control.py"))
    replay = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(replay)
    args.out.mkdir(parents=True, exist_ok=False)
    report = {"scope": "Diagnostic log qualification only; unchanged native history and full original planning payloads. No treatment or win-rate estimate.",
              "passed": False, "through_turn": args.through_turn, "controls": []}
    try:
        original = json.loads(args.original_qualification.read_text())
        assert original["passed"] and original["through_turn"] == args.through_turn
        assert len(original["controls"]) == 2 and all(c["passed"] for c in original["controls"])
        context = json.loads(args.context.read_text())
        records = replay.select_records(args.decisions, args.through_turn, context)
        assert all(len(c["requests"]) == len(records) for c in original["controls"])
        diagnostic = copy.deepcopy(context)
        diagnostic["evidence_scope"] = "Private diagnostic executable; original reconstructed arguments retained except executable and input/output destinations."
        diagnostic["processes"][0]["argv"][0] = str(args.binary.resolve())
        diagnostic["binary_sha256"] = digest(args.binary)
        diagnostic["provenance"] = {"kind": "private_diagnostic_build", "sha256": diagnostic["binary_sha256"]}
        (args.out / "diagnostic-context.json").write_text(json.dumps(diagnostic, indent=2) + "\n")
        report.update(original_qualification_sha256=digest(args.original_qualification),
                      original_context_sha256=digest(args.context),
                      original_binary_sha256=context["binary_sha256"],
                      diagnostic_binary_sha256=diagnostic["binary_sha256"],
                      original_decisions_sha256=digest(args.decisions))
        mapping, report["sealed_captures"] = replay.seal_captures(records, args.out)
        for number in range(2):
            result = replay.run_control(diagnostic, records, mapping, args.out / f"control-{number}", 120)
            report["controls"].append(result)
            if not result["passed"]:
                break
        report["passed"] = len(report["controls"]) == 2 and all(c["passed"] for c in report["controls"])
    except Exception as error:
        report["error"] = f"{type(error).__name__}: {error}"
    (args.out / "qualification.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"passed": report["passed"], "requests": [len(c["requests"]) for c in report["controls"]],
                      "error": report.get("error"), "out": str(args.out)}), flush=True)
    return 0 if report["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
