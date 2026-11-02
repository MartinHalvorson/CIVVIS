"""Check candidate regression on the separately qualified Domination history.

This tests another lane, not Religion efficacy. No host orders are published.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("repo", "context", "qualification", "binary", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    sys.path.insert(0, str(args.repo / "tools"))
    import civ6_captured_read_replay as replay

    proof = json.loads((args.qualification / "qualification.json").read_text())
    assert proof["passed"] and len(proof["controls"]) == 2
    context = json.loads(args.context.read_text())
    assert context == json.loads((args.qualification / "observed-context.json").read_text())
    raw = (args.qualification / "native-decisions.jsonl").read_bytes()
    assert sha(raw) == proof["native_decisions_sha256"]
    records = [json.loads(line) for line in raw.splitlines()]
    command = context["processes"][0]["argv"].copy()
    assert command[command.index("--victory") + 1] == "domination"
    command[0] = str(args.binary.resolve())
    digest = sha(args.binary.read_bytes())
    scope = "Candidate regression on a separately qualified Domination history; not a Religion efficacy test, original-executable qualification, native execution or strength estimate."
    variant = {"binary_sha256": digest,
               "provenance": {"sha256": digest, "source": "candidate bytes, not an observed host executable"},
               "processes": [{"argv": command, "cwd": context["processes"][0]["cwd"]}]}
    mapping = {}
    for capture in proof["sealed_captures"]:
        portable = args.qualification / Path(capture["sealed_directory"]).name
        directory = portable if portable.is_dir() else Path(capture["sealed_directory"])
        archive = (directory / "bytes.bin").read_bytes()
        assert len(archive) == capture["archive_bytes"]
        assert sha(archive) == capture["archive_sha256"]
        assert sha((directory / "snapshots.jsonl").read_bytes()) == capture["index_sha256"]
        mapping[capture["source"]] = directory
    args.out.mkdir(parents=True, exist_ok=False)
    results = []
    for number in range(2):
        result = replay.run_control(variant, records, mapping, args.out / f"candidate-{number}", 60)
        results.append(result)
        report = {"scope": scope, "source_run": context["run"],
                  "original_context_sha256": sha(args.context.read_bytes()),
                  "native_decisions_sha256": proof["native_decisions_sha256"],
                  "original_binary_sha256": context["binary_sha256"],
                  "candidate_binary_sha256": digest,
                  "candidate_git_head": subprocess.check_output(
                      ["git", "rev-parse", "HEAD"], cwd=args.repo, text=True).strip(),
                  "passed": len(results) == 2 and all(r["passed"] for r in results),
                  "results": results}
        (args.out / "result.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps({"control": number, "passed": result["passed"],
                          "requests": len(result["requests"]), "error": result.get("error")}), flush=True)
        if not result["passed"]:
            raise SystemExit(2)


if __name__ == "__main__":
    main()
