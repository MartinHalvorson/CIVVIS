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
    if type(decision.get("schema")) is not int or decision["schema"] != 1 or type(payload.get("turn")) is not int or payload["turn"] < 0 or type(decision.get("turn")) is not int or decision["turn"] != payload["turn"]:
        raise ValueError("decision trace schema or turn mismatch")
    if type(decision.get("frame")) is not int or decision["frame"] < 0:
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
    if "input_capture" in payload:
        validate_input_capture(payload["input_capture"])
        # Preserve the ordered read frontiers, including failures. The receipt
        # hashes this descriptor, not the external archive's contents.
        record["input_capture"] = payload["input_capture"]
    encoded = json.dumps(record, sort_keys=True, separators=(",", ":"), allow_nan=False)
    record["sha256"] = hashlib.sha256(encoded.encode()).hexdigest()
    with (Path(run_dir) / "decisions.jsonl").open("a", encoding="utf-8") as stream:
        stream.write(json.dumps(record, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n")
        stream.flush()
        os.fsync(stream.fileno())
    return record


def validate_input_capture(capture: dict) -> None:
    """Validate the opt-in descriptor without treating capture as execution."""
    if not isinstance(capture, dict) or type(capture.get("schema")) is not int or capture["schema"] != 1:
        raise ValueError("invalid input capture schema")
    if type(capture.get("complete")) is not bool or not isinstance(capture.get("reads"), list):
        raise ValueError("invalid input capture completeness or reads")
    if capture.get("archive") != "bytes.bin" or capture.get("index") != "snapshots.jsonl":
        raise ValueError("invalid input capture filenames")
    for key in ("directory", "source"):
        if not isinstance(capture.get(key), str) or not Path(capture[key]).is_absolute():
            raise ValueError("input capture paths must be absolute")
    if (capture["complete"] and capture.get("error") is not None) or (not capture["complete"] and not isinstance(capture.get("error"), str)):
        raise ValueError("input capture error disagrees with completeness")
    for read in capture["reads"]:
        if not isinstance(read, dict) or type(read.get("count")) is not int or read["count"] < 1:
            raise ValueError("invalid input capture read count")
        if set(read) == {"snapshot_id", "count"}:
            if type(read["snapshot_id"]) is not int or read["snapshot_id"] < 0:
                raise ValueError("invalid input capture snapshot id")
        elif set(read) != {"error", "count"} or not isinstance(read["error"], str):
            raise ValueError("invalid input capture read")


def captured_input(capture: dict, snapshot_id: int) -> bytes:
    """Reconstruct one actual read, not a guessed decision-wide file prefix.

    Structural checks detect missing/truncated evidence, not same-length byte
    tampering. Archive integrity hashes can be sealed separately after a run.
    Later index rows can still be appending; only read through the requested ID.
    """
    validate_input_capture(capture)
    if not capture["complete"]:
        raise ValueError("input capture is incomplete")
    if type(snapshot_id) is not int or snapshot_id < 0 or not any(read.get("snapshot_id") == snapshot_id for read in capture["reads"]):
        raise ValueError("snapshot is not a read in this decision")
    directory = Path(capture["directory"])
    rows, offset = [], 0
    with (directory / capture["index"]).open(encoding="utf-8") as index:
        for line in index:
            row = json.loads(line)
            if not isinstance(row, dict) or set(row) != {"id", "parent", "offset", "append_bytes", "total_bytes"}:
                raise ValueError("invalid input snapshot row")
            for key in ("id", "offset", "append_bytes", "total_bytes"):
                if type(row[key]) is not int or row[key] < 0:
                    raise ValueError("invalid input snapshot integer")
            parent = row["parent"]
            if row["id"] != len(rows) or row["offset"] != offset or (parent is not None and (type(parent) is not int or not 0 <= parent < len(rows))):
                raise ValueError("invalid input snapshot sequence")
            before = 0 if parent is None else rows[parent]["total_bytes"]
            if row["total_bytes"] != before + row["append_bytes"]:
                raise ValueError("invalid input snapshot length")
            offset += row["append_bytes"]
            rows.append(row)
            if row["id"] == snapshot_id:
                break
    if len(rows) <= snapshot_id:
        raise ValueError("missing input snapshot")
    chain, current = [], snapshot_id
    while current is not None:
        chain.append(rows[current])
        current = rows[current]["parent"]
    chunks = []
    with (directory / capture["archive"]).open("rb") as archive:
        if os.fstat(archive.fileno()).st_size < offset:
            raise ValueError("truncated input archive")
        for row in reversed(chain):
            archive.seek(row["offset"])
            chunk = archive.read(row["append_bytes"])
            if len(chunk) != row["append_bytes"]:
                raise ValueError("truncated input archive")
            chunks.append(chunk)
    return b"".join(chunks)


def exact_fact(expected, actual):
    """JSON shape and scalar types are part of an exact deterministic fact."""
    json.dumps([expected, actual], allow_nan=False)
    for value in (expected, actual):
        if isinstance(value, float) and not math.isfinite(value):
            raise ValueError("non-finite deterministic fact")
    if type(expected) is not type(actual):
        return False
    if isinstance(expected, list):
        # Check every entry even after a mismatch so a later NaN is not hidden.
        matches = [exact_fact(a, b) for a, b in zip(expected, actual)]
        return len(expected) == len(actual) and all(matches)
    if isinstance(expected, dict):
        return expected.keys() == actual.keys() and all(exact_fact(value, actual[key]) for key, value in expected.items())
    return expected == actual


def compare_transition(case: dict) -> dict:
    """Check a causally isolated action against independently recorded facts.

    `predictions` are model point values or {low, high} bounds, not fitted to
    these observations. Missing facts and non-isolated frame intervals are
    coverage gaps. A caller cannot obtain a passing report with no comparison.
    """
    gaps, differences, compared = [], [], 0
    if case.get("phase") != "settled" or case.get("coverage_gap"):
        return {"status": "unverifiable", "compared": 0,
                "gaps": ["explicit settled evidence without coverage gaps is required"], "differences": []}
    if case.get("same_turn") is not True or type(case.get("intervening_actions")) is not int or case["intervening_actions"] != 0:
        return {"status": "unverifiable", "compared": 0,
                "gaps": ["transition is not an isolated same-turn action"], "differences": []}
    predictions, observed = case.get("predictions", {}), case.get("observed", {})
    if not isinstance(predictions, dict) or not isinstance(observed, dict):
        raise ValueError("predictions and observed must be fact objects")
    for key, expected in predictions.items():
        actual = observed.get(key)
        if actual is None:
            gaps.append(key)
            continue
        if isinstance(expected, dict):
            if set(expected) != {"low", "high"}:
                raise ValueError(f"numeric prediction requires exactly low/high bounds: {key}")
            low, high = expected.get("low"), expected.get("high")
            values = (low, high, actual)
            if not all(isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v) for v in values) or low > high:
                raise ValueError(f"invalid numeric prediction or observation: {key}")
            matches = low <= actual <= high
        else:
            matches = exact_fact(expected, actual)
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
