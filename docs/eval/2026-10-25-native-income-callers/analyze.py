"""Join qualified root-Game caller receipts to the fixed native income audit."""
import argparse
import collections
import gzip
import hashlib
import json
from pathlib import Path
import re


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qualification", required=True, type=Path)
    parser.add_argument("--audit", required=True, type=Path)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    qualified = json.loads(args.qualification.read_text())
    assert qualified["passed"] and len(qualified["controls"]) == 2
    assert all(c["passed"] for c in qualified["controls"])
    traces = []
    for number in range(2):
        path = args.qualification.parent / f"control-{number}" / "stderr.log"
        traces.append([json.loads(line.removeprefix("[income-probe] "))
                       for line in path.read_text().splitlines() if line.startswith("[income-probe] ")])
    assert traces[0] == traces[1], "the two qualified diagnostic traces differ"
    trace = traces[0]
    assert trace, "no root-Game diagnostic receipts were captured"
    audit = json.loads(gzip.decompress(args.audit.read_bytes()))
    run = next(r for r in audit["runs"] if r["run"] == "civvis-20261025T174828Z")
    cases = [row for row in run["opportunities"] if row["turn"] <= qualified["through_turn"]]
    frames = collections.defaultdict(list)
    for row in trace:
        frames[row["turn"], row["frame"]].append(row)
    requests = {(q["turn"], q["frame"]): q for q in run["qualified_requests_with_offered_idle_hubs"]}

    def source_receipt(writer):
        path = args.source / writer["caller_file"]
        lines = path.read_text().splitlines()
        number = writer["caller_line"]
        assert 1 <= number <= len(lines)
        function = next((line.strip() for line in reversed(lines[:number])
                         if re.match(r"\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+", line)), None)
        return {"file": writer["caller_file"], "line": number, "file_sha256": sha(path),
                "function": function,
                "excerpt": [{"line": i + 1, "text": lines[i]}
                            for i in range(max(0, number - 10), min(len(lines), number + 8))]}

    results = []
    for case in cases:
        key = case["turn"], case["frame"]
        rows = frames[key]
        writers = [r for r in rows if r["event"] == "produce" and r["city"] == case["model_city_id"]]
        successful = [r for r in writers if r["ok"]]
        planned = case["planned_model_productions"]
        matches = bool(successful and planned and successful[-1]["item"] == planned[-1]["item"])
        root = [r for r in rows if r["event"] == "root_begin"]
        result = dict(case)
        result.update(native_gold_per_turn=requests[key]["native_state"].get("gold_per_turn"),
                      root_begin=root, production_calls=writers,
                      last_successful_writer_matches_last_recorded_model_produce=matches,
                      last_writer_source=source_receipt(successful[-1]) if successful else None,
                      commercial_calls=[r for r in rows if r["event"].startswith("commercial_") and r.get("city") == case["model_city_id"]],
                      strategic_hub_gates=[r for r in rows if r["event"] == "strategic_hub_reached" and r.get("city") == case["model_city_id"]])
        results.append(result)
    report = {"scope": "Post-hoc actual caller diagnosis for one fixed historical prefix. Both complete instrumented planning/read controls and both diagnostic traces match. No counterfactual game or strength estimate.",
              "qualification_sha256": sha(args.qualification), "original_audit_sha256": sha(args.audit),
              "through_turn": qualified["through_turn"], "run": run["run"],
              "trace_event_counts": dict(collections.Counter(r["event"] for r in trace)),
              "qualified_cases": len(results),
              "matched_last_successful_writers": sum(r["last_successful_writer_matches_last_recorded_model_produce"] for r in results),
              "writer_functions": dict(collections.Counter((r["last_writer_source"] or {}).get("function") for r in results)),
              "cases": results, "complete_root_trace": trace}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(gzip.compress(json.dumps(report, sort_keys=True).encode(), mtime=0))
    assert json.loads(gzip.decompress(args.out.read_bytes())) == report
    print(json.dumps({k: v for k, v in report.items() if k not in ("cases", "complete_root_trace")}))
    for case in results:
        source = case["last_writer_source"] or {}
        writer = next((w for w in reversed(case["production_calls"]) if w["ok"]), {})
        print(json.dumps({"turn": case["turn"], "frame": case["frame"], "city": case["city_name"],
                          "choice": case["classification"], "native_income": case["native_gold_per_turn"],
                          "writer_model_income": writer.get("model_gold_per_turn_before"),
                          "caller": [source.get("file"), source.get("line"), source.get("function")],
                          "matched": case["last_successful_writer_matches_last_recorded_model_produce"],
                          "commercial_calls": len(case["commercial_calls"])}))


if __name__ == "__main__":
    main()
