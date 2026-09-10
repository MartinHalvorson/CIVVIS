"""Lossless planning/transport evidence. This is not an execution receipt.

The brain records before publishing orders. A trace proves what was planned
and emitted, never that Firaxis accepted or executed it. Verification must join
the corresponding host frame and actual outcome separately.
"""
from __future__ import annotations

import hashlib
import argparse
import json
import math
import os
from functools import lru_cache
from pathlib import Path


@lru_cache(maxsize=8)
def _binary_digest(filename: str, identity: tuple) -> str:
    # Cache by file identity, not path alone: the brain can refresh its binary.
    digest = hashlib.sha256()
    with open(filename, "rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
        status = os.fstat(stream.fileno())
    if identity != (status.st_dev, status.st_ino, status.st_size, status.st_mtime_ns, status.st_ctime_ns):
        raise ValueError("decision binary changed while recording provenance")
    return digest.hexdigest()


def record_decision(run_dir: Path, payload: dict, binary: str) -> dict:
    decision = payload["decision"]
    if decision.get("schema") != 1 or decision.get("turn") != payload.get("turn"):
        raise ValueError("decision trace schema or turn mismatch")
    if not isinstance(decision.get("frame"), int) or decision["frame"] < 0:
        raise ValueError("decision trace requires a nonnegative frame")
    if not isinstance(decision.get("native_actions"), list) or not isinstance(payload.get("orders"), list):
        raise ValueError("decision trace requires actions and emitted orders")
    filename = str(Path(binary).resolve(strict=True))
    status = os.stat(filename)
    identity = (status.st_dev, status.st_ino, status.st_size, status.st_mtime_ns, status.st_ctime_ns)
    record = {"run": Path(run_dir).name, "binary": filename,
              # This identifies disk bytes at recording time, not the loaded
              # process image if an external actor replaced the executable.
              "binary_on_disk_sha256": _binary_digest(filename, identity),
              "decision": decision, "orders": payload["orders"],
              "execution_status": "not_observed"}
    encoded = json.dumps(record, sort_keys=True, separators=(",", ":"), allow_nan=False)
    record["sha256"] = hashlib.sha256(encoded.encode()).hexdigest()
    with (Path(run_dir) / "decisions.jsonl").open("a", encoding="utf-8") as stream:
        stream.write(json.dumps(record, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n")
        stream.flush()
        os.fsync(stream.fileno())
    return record


def compare_transition(case: dict) -> dict:
    """Check a causally isolated action against independently recorded facts.

    `predictions` are model point values or {low, high} bounds, not fitted to
    these observations. Missing facts and non-isolated frame intervals are
    coverage gaps. A caller cannot obtain a passing report with no comparison.
    """
    gaps, differences, compared = [], [], 0
    if case.get("same_turn") is not True or case.get("intervening_actions") != 0:
        return {"status": "unverifiable", "compared": 0,
                "gaps": ["transition is not an isolated same-turn action"], "differences": []}
    predictions, observed = case.get("predictions", {}), case.get("observed", {})
    for key, expected in predictions.items():
        actual = observed.get(key)
        if actual is None:
            gaps.append(key)
            continue
        if isinstance(expected, dict):
            low, high = expected.get("low"), expected.get("high")
            values = (low, high, actual)
            if not all(isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v) for v in values) or low > high:
                raise ValueError(f"invalid numeric prediction or observation: {key}")
            matches = low <= actual <= high
        else:
            if isinstance(actual, float) and not math.isfinite(actual):
                raise ValueError(f"non-finite observation: {key}")
            matches = type(actual) is type(expected) and actual == expected
        compared += 1
        if not matches:
            differences.append({"key": key, "predicted": expected, "observed": actual})
    if not predictions:
        gaps.append("no model predictions")
    return {"status": "mismatch" if differences else "unverifiable" if gaps or not compared else "match",
            "compared": compared, "gaps": gaps, "differences": differences}


def main():
    parser = argparse.ArgumentParser(description="Check isolated action-transition fixtures; missing coverage fails closed.")
    parser.add_argument("cases", type=Path, help="JSONL action cases with predictions and independently observed outcomes")
    args = parser.parse_args()
    cases = [json.loads(line) for line in args.cases.read_text().splitlines() if line.strip()]
    reports = [dict(case=case.get("id", index), **compare_transition(case)) for index, case in enumerate(cases)]
    print(json.dumps({"cases": reports, "compared": sum(r["compared"] for r in reports)}, indent=2, allow_nan=False))
    return 0 if reports and all(r["status"] == "match" for r in reports) else 1


if __name__ == "__main__":
    raise SystemExit(main())
