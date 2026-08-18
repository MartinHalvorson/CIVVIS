#!/usr/bin/env python3
"""Run the checked-in differential corpus as a reviewable fidelity ratchet.

The comparator's unit tests prove individual rules.  This corpus proves that a
small, sanitized replay remains an ordered transition stream: transport
metadata and mathematical-set order may vary, while state, action order, and
frame boundaries may not.  The manifest is intentionally committed so a
fixture or canonicalisation change requires an explicit review update.
"""

from __future__ import annotations

import copy
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / "tools"
sys.path.insert(0, str(TOOLS))

import civ6_differential as differential  # noqa: E402


ORACLE_PATH = ROOT / "tests/fixtures/differential/golden.jsonl"
CANDIDATE_PATH = ROOT / "tests/fixtures/differential/candidate.jsonl"
MANIFEST_PATH = ROOT / "tests/fixtures/differential/manifest.json"


def _canonical_hashes(trace: differential.Trace) -> list[dict[str, object]]:
    return [
        {
            "key": list(frame.key),
            "kind": frame.kind,
            "sha256": differential.state_hash(
                differential.canonical_payload(frame.payload)
            ),
        }
        for frame in trace.frames
    ]


def _fail(message: str) -> int:
    print(f"differential golden ratchet: {message}", file=sys.stderr)
    return 1


def main() -> int:
    try:
        oracle = differential.load_trace(ORACLE_PATH, require_contiguous=True)
        candidate = differential.load_trace(CANDIDATE_PATH, require_contiguous=True)
    except differential.TraceError as exc:
        return _fail(str(exc))

    expected = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    actual = {
        "records": len(oracle.frames),
        "turns": list(oracle.turns),
        "frames": _canonical_hashes(oracle),
    }
    if actual != expected:
        return _fail(
            "oracle fixture or canonicalisation changed; update the manifest only "
            "with a reviewed fidelity change\n"
            f"expected={json.dumps(expected, sort_keys=True)}\n"
            f"actual={json.dumps(actual, sort_keys=True)}"
        )

    equal = differential.compare_traces(oracle, candidate)
    if not equal["equal"]:
        return _fail(f"candidate fixture drifted: {equal['first_divergence']}")

    # Exercise the negative path against a copy of one state frame.  This is a
    # mutation test for the gate itself: a ratchet that only ever proves equal
    # traces can silently stop comparing payloads after a refactor.
    mutated = copy.deepcopy(candidate.frames[0].payload)
    mutated["score"] = 1
    if differential.state_hash(mutated) == differential.state_hash(oracle.frames[0].payload):
        return _fail("mutation did not change the state hash")
    path, _, _ = differential.first_difference(
        differential.canonical_payload(oracle.frames[0].payload),
        differential.canonical_payload(mutated),
    )
    if path != "/score":
        return _fail(f"mutation was not localized to /score: {path}")

    print(
        "differential golden ratchet: PASS "
        f"({actual['records']} frames, turns {actual['turns'][0]}–{actual['turns'][-1]})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
