#!/usr/bin/env python3
"""Preserve real probe inputs without normal turn logs or model predictions."""
import argparse
import hashlib
import json
from pathlib import Path


def extract(source: Path, destination: Path, count: int) -> dict:
    if count < 1:
        raise ValueError("count must be positive")
    records, completed = [], 0
    digest = hashlib.sha256()
    with source.open("rb") as stream:
        for raw in stream:
            digest.update(raw)
            event = json.loads(raw)
            kind = event.get("kind", "")
            if completed < count and (kind in ("seat", "tiles") or kind.startswith("action_transition_")):
                records.append(event)
                if kind == "action_transition_end":
                    if not (event.get("settled") and event.get("isolated")):
                        raise ValueError("fixture contains an incomplete/non-isolated probe")
                    completed += 1
    if completed != count:
        raise ValueError("not enough completed probes")
    metadata = {"source_run": source.parent.name, "source_sha256": digest.hexdigest(),
                "probes": completed, "selection": "first consecutive isolated probes; no outcome selection",
                "strength_evidence": False}
    # Exclusive creation prevents overwriting existing evidence.
    with destination.open("x") as stream:
        stream.write(json.dumps({"kind": "fixture_provenance", **metadata}) + "\n")
        for event in records:
            stream.write(json.dumps(event, separators=(",", ":")) + "\n")
    return metadata


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--count", type=int, default=10)
    args = parser.parse_args()
    print(json.dumps(extract(args.source, args.destination, args.count), indent=2))
