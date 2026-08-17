#!/usr/bin/env python3
"""Compare two CIVVIS replay traces and stop at the first semantic drift.

The real-game bridge and the headless engine both produce JSON, but a JSONL
file is not a replay contract by itself.  A missing frame, a reordered phase,
or a state field that changed after a refactor can otherwise look like a healthy
run because the final score still agrees.  This tool turns the trace into an
ordered sequence of ``(turn, phase, occurrence)`` frames, canonicalises each
payload, hashes it, and reports the first mismatch with a JSON-pointer path.

It is deliberately independent of Civilization VI and of the Rust binary, so
it can run in CI against a checked-in golden trace or against an exported
``events.jsonl`` without installing the game::

    python3 tools/civ6_differential.py \
        --oracle traces/stock.jsonl --candidate traces/new.jsonl

The default stream is the transition spine emitted by the control bridge:
``state``, ``turn`` and ``orders`` records.  Other event kinds (including an
explicit action stream) can
be selected with ``--kinds``.  ``run``/``ctx`` and similar envelope metadata are
ignored by default; all state fields remain strict.  List order remains strict
unless a path is explicitly named with ``--unordered``.  Exit status is 0 for
an equal trace, 1 for a semantic or structural difference, and 2 for an input
or contract error.

This is the dynamic companion to ``civ6_yield_drift.py``.  The latter asks
whether one derived economy is right at a single frame; this asks whether two
implementations stayed on the same ordered state machine and identifies the
first frame where they stopped doing so.
"""

from __future__ import annotations

import argparse
from collections import Counter
from dataclasses import dataclass
import fnmatch
import hashlib
import json
from pathlib import Path
import sys
from typing import Any, Iterable, Sequence


DEFAULT_KINDS = ("state", "turn", "orders")
# These are transport/session fields, not game state.  They are ignored at
# every nesting level only when they occur at the event root; a real state field
# with one of these names remains visible when it is nested under ``state``.
DEFAULT_IGNORES = (
    "/kind",
    "/event",
    "/ctx",
    "/run",
    "/revision",
    "/timestamp",
    "/utc",
)
# These fields are mathematical sets in the Civ VI state schema.  Their order
# is not a transition; the records themselves remain ordered by phase/turn.
DEFAULT_UNORDERED = (
    "/techs",
    "/civics",
    "/policies",
    "/founded_religions",
    "/religion_beliefs",
    "/taken_religion_beliefs",
)


class TraceError(ValueError):
    """The trace cannot satisfy the differential contract."""


@dataclass(frozen=True)
class Frame:
    """One selected transition record in source order."""

    line: int
    kind: str
    turn: int
    phase: str
    occurrence: int
    payload: Any

    @property
    def key(self) -> tuple[int, str, int]:
        return (self.turn, self.phase, self.occurrence)


@dataclass(frozen=True)
class Trace:
    path: str
    frames: tuple[Frame, ...]

    @property
    def kinds(self) -> Counter[str]:
        return Counter(frame.kind for frame in self.frames)

    @property
    def turns(self) -> tuple[int, ...]:
        return tuple(sorted({frame.turn for frame in self.frames}))


def _reject_constant(value: str) -> Any:
    raise TraceError(f"non-finite JSON number {value!r} is not a game-state value")


def _object_no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise TraceError(f"duplicate JSON object key {key!r}")
        result[key] = value
    return result


def parse_json_line(raw: str, source: str, line: int) -> dict[str, Any]:
    """Parse one strict JSON object and annotate errors with its source line."""
    try:
        value = json.loads(
            raw,
            object_pairs_hook=_object_no_duplicates,
            parse_constant=_reject_constant,
        )
    except (json.JSONDecodeError, TraceError) as exc:
        raise TraceError(f"{source}:{line}: invalid JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise TraceError(f"{source}:{line}: trace records must be JSON objects")
    return value


def _integer_turn(value: Any) -> int | None:
    # bool is an int subclass in Python, but it is never a valid turn number.
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    return None


def _selected_kinds(kinds: Sequence[str] | None) -> set[str] | None:
    if kinds is None:
        return set(DEFAULT_KINDS)
    selected = {str(kind).strip() for kind in kinds if str(kind).strip()}
    # An empty --kinds means every record.  It is useful when validating an
    # entire bridge log, but those records still need integer turns.
    return selected or None


def _payload(event: dict[str, Any]) -> Any:
    # A future logger may put state below an envelope.  Accept both that shape
    # and today's flat ``kind: state`` shape without weakening comparison.
    nested = event.get("state")
    if isinstance(nested, dict):
        return nested
    return event


def load_trace(
    path: str | Path,
    *,
    kinds: Sequence[str] | None = None,
    from_turn: int | None = None,
    to_turn: int | None = None,
    allow_trailing_partial: bool = False,
    require_contiguous: bool = False,
) -> Trace:
    """Load and validate a selected, ordered JSONL transition trace.

    Records are never sorted.  Replay order is part of the contract, so a
    backwards turn or a missing selected turn is an input failure rather than a
    convenience to be silently repaired by the differ.
    """
    source = str(path)
    selected = _selected_kinds(kinds)
    try:
        raw_text = Path(path).read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        raise TraceError(f"cannot read {source}: {exc}") from exc

    frames: list[Frame] = []
    occurrences: Counter[tuple[int, str]] = Counter()
    previous_turn: int | None = None
    lines = raw_text.splitlines(keepends=True)
    for line_number, raw_line in enumerate(lines, 1):
        text = raw_line.strip()
        if not text:
            continue
        try:
            event = parse_json_line(text, source, line_number)
        except TraceError:
            # A live Automation.log relay can leave one unterminated JSON line
            # while the game is still writing it.  Strict golden traces must
            # reject it; the explicit opt-in only tolerates the final line.
            is_last = line_number == len(lines)
            if allow_trailing_partial and is_last and not raw_line.endswith("\n"):
                break
            raise
        kind_value = event.get("kind", event.get("event"))
        kind = str(kind_value or "").strip()
        if selected is not None and kind not in selected:
            continue
        turn = _integer_turn(event.get("turn"))
        if turn is None:
            raise TraceError(
                f"{source}:{line_number}: selected {kind or 'record'!r} "
                "record has no integer turn"
            )
        if from_turn is not None and turn < from_turn:
            continue
        if to_turn is not None and turn > to_turn:
            continue
        if previous_turn is not None and turn < previous_turn:
            raise TraceError(
                f"{source}:{line_number}: turn moved backwards "
                f"from {previous_turn} to {turn}"
            )
        previous_turn = turn
        phase_value = event.get("phase", kind)
        phase = str(phase_value or kind or "event")
        occurrence_key = (turn, phase)
        occurrence = occurrences[occurrence_key]
        occurrences[occurrence_key] += 1
        frames.append(Frame(
            line=line_number,
            kind=kind,
            turn=turn,
            phase=phase,
            occurrence=occurrence,
            payload=_payload(event),
        ))

    if not frames:
        raise TraceError(f"{source}: no selected transition records")
    trace = Trace(source, tuple(frames))
    if require_contiguous and trace.turns:
        expected = set(range(trace.turns[0], trace.turns[-1] + 1))
        missing = sorted(expected - set(trace.turns))
        if missing:
            preview = ", ".join(str(turn) for turn in missing[:12])
            suffix = "..." if len(missing) > 12 else ""
            raise TraceError(
                f"{source}: selected turns are not contiguous; missing "
                f"{preview}{suffix}"
            )
    return trace


def _segments(pattern: str) -> tuple[str, ...]:
    pattern = pattern.strip()
    if not pattern:
        return ()
    if pattern.startswith("/"):
        pattern = pattern[1:]
    # Accept both JSON pointers and a compact dotted spelling.  JSON pointer is
    # preferred because it can represent keys containing dots unambiguously.
    if "/" not in pattern and "." in pattern:
        return tuple(part for part in pattern.split(".") if part)
    return tuple(part for part in pattern.split("/") if part)


def _path_matches(pattern: str, path: tuple[str, ...]) -> bool:
    wanted = _segments(pattern)

    def walk(wi: int, pi: int) -> bool:
        if wi == len(wanted):
            return pi == len(path)
        token = wanted[wi]
        if token == "**":
            return walk(wi + 1, pi) or (pi < len(path) and walk(wi, pi + 1))
        return pi < len(path) and (token == "*" or fnmatch.fnmatchcase(path[pi], token)) \
            and walk(wi + 1, pi + 1)

    return walk(0, 0)


def _matches_any(patterns: Sequence[str], path: tuple[str, ...]) -> bool:
    return any(_path_matches(pattern, path) for pattern in patterns)


_OMIT = object()


def _canonical(
    value: Any,
    path: tuple[str, ...],
    ignores: Sequence[str],
    unordered: Sequence[str],
) -> Any:
    if _matches_any(ignores, path):
        return _OMIT
    if isinstance(value, dict):
        result: dict[str, Any] = {}
        for key in sorted(value):
            child = _canonical(value[key], path + (str(key),), ignores, unordered)
            if child is not _OMIT:
                result[str(key)] = child
        return result
    if isinstance(value, list):
        items = []
        for index, item in enumerate(value):
            child = _canonical(item, path + (str(index),), ignores, unordered)
            if child is not _OMIT:
                items.append(child)
        if _matches_any(unordered, path):
            # Sort by canonical JSON rather than Python's heterogeneous value
            # ordering.  Objects, strings and numbers can safely share a list.
            items.sort(key=lambda item: json.dumps(
                item, sort_keys=True, separators=(",", ":"), ensure_ascii=False
            ))
        return items
    if isinstance(value, float) and (value != value or value in (float("inf"), -float("inf"))):
        raise TraceError(f"non-finite number at /{'/'.join(path)}")
    return value


def canonical_payload(
    payload: Any,
    *,
    ignores: Sequence[str] = DEFAULT_IGNORES,
    unordered: Sequence[str] = DEFAULT_UNORDERED,
) -> Any:
    """Return a JSON-serialisable canonical copy of a frame payload."""
    result = _canonical(payload, (), ignores, unordered)
    return {} if result is _OMIT else result


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False,
    ).encode("utf-8")


def state_hash(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def _pointer(path: tuple[str, ...]) -> str:
    if not path:
        return "/"
    return "".join("/" + str(part).replace("~", "~0").replace("/", "~1")
                   for part in path)


def first_difference(left: Any, right: Any, path: tuple[str, ...] = ()) -> tuple[str, Any, Any]:
    """Return the first deterministic JSON-pointer difference."""
    if isinstance(left, dict) and isinstance(right, dict):
        keys = sorted(set(left) | set(right))
        for key in keys:
            if key not in left:
                return _pointer(path + (str(key),)), None, right[key]
            if key not in right:
                return _pointer(path + (str(key),)), left[key], None
            found = first_difference(left[key], right[key], path + (str(key),))
            if found[0] != "":
                return found
        return "", None, None
    if isinstance(left, list) and isinstance(right, list):
        if len(left) != len(right):
            return _pointer(path + ("length",)), len(left), len(right)
        for index, (a, b) in enumerate(zip(left, right)):
            found = first_difference(a, b, path + (str(index),))
            if found[0] != "":
                return found
        return "", None, None
    if left != right or type(left) is not type(right):
        return _pointer(path), left, right
    return "", None, None


def _preview(value: Any, limit: int = 1600) -> Any:
    """Keep a missing whole-state frame from flooding a CI log."""
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    if len(encoded) <= limit:
        return value
    return {
        "truncated": True,
        "sha256": hashlib.sha256(encoded.encode("utf-8")).hexdigest(),
        "bytes": len(encoded.encode("utf-8")),
        "prefix": encoded[:limit] + "...",
    }


def compare_traces(
    oracle: Trace,
    candidate: Trace,
    *,
    ignores: Sequence[str] = DEFAULT_IGNORES,
    unordered: Sequence[str] = DEFAULT_UNORDERED,
) -> dict[str, Any]:
    """Compare two traces and return a stable, JSON-serialisable report."""
    first: dict[str, Any] | None = None
    matched = 0
    for index, (left, right) in enumerate(zip(oracle.frames, candidate.frames)):
        if left.key != right.key:
            first = {
                "type": "frame",
                "index": index,
                "oracle": {"line": left.line, "kind": left.kind, "key": list(left.key)},
                "candidate": {"line": right.line, "kind": right.kind, "key": list(right.key)},
                "path": "/frame",
                "oracle_value": list(left.key),
                "candidate_value": list(right.key),
            }
            break
        left_value = canonical_payload(left.payload, ignores=ignores, unordered=unordered)
        right_value = canonical_payload(right.payload, ignores=ignores, unordered=unordered)
        left_hash = state_hash(left_value)
        right_hash = state_hash(right_value)
        if left_hash != right_hash:
            path, left_leaf, right_leaf = first_difference(left_value, right_value)
            first = {
                "type": "state",
                "index": index,
                "frame": {"turn": left.turn, "phase": left.phase,
                          "occurrence": left.occurrence},
                "oracle": {"line": left.line, "kind": left.kind, "hash": left_hash},
                "candidate": {"line": right.line, "kind": right.kind, "hash": right_hash},
                "path": path or "/",
                "oracle_value": _preview(left_leaf),
                "candidate_value": _preview(right_leaf),
            }
            break
        matched += 1

    if first is None and len(oracle.frames) != len(candidate.frames):
        index = min(len(oracle.frames), len(candidate.frames))
        missing = oracle.frames[index:] or candidate.frames[index:]
        frame = missing[0]
        side = "oracle" if len(oracle.frames) > len(candidate.frames) else "candidate"
        first = {
            "type": "frame_count",
            "index": index,
            "side": side,
            "path": "/frame",
            "frame": {"turn": frame.turn, "phase": frame.phase,
                      "occurrence": frame.occurrence},
            "value": _preview(frame.payload),
        }

    return {
        "equal": first is None,
        "oracle": oracle.path,
        "candidate": candidate.path,
        "oracle_records": len(oracle.frames),
        "candidate_records": len(candidate.frames),
        "matched_records": matched,
        "oracle_turns": list(oracle.turns),
        "candidate_turns": list(candidate.turns),
        "oracle_kinds": dict(sorted(oracle.kinds.items())),
        "candidate_kinds": dict(sorted(candidate.kinds.items())),
        "first_divergence": first,
    }


def _format_frame(frame: dict[str, Any]) -> str:
    if "frame" in frame:
        f = frame["frame"]
        return f"turn {f['turn']} phase {f['phase']} occurrence {f['occurrence']}"
    if "oracle" in frame and isinstance(frame["oracle"], dict) and "key" in frame["oracle"]:
        return f"oracle key {frame['oracle']['key']} / candidate key {frame['candidate']['key']}"
    return "frame boundary"


def render_report(report: dict[str, Any]) -> str:
    lines = [
        "TRACE DIFFERENTIAL",
        f"oracle      : {report['oracle']}",
        f"candidate   : {report['candidate']}",
        f"records     : {report['oracle_records']} / {report['candidate_records']}",
        f"turns       : {report['oracle_turns']} / {report['candidate_turns']}",
        f"kinds       : {report['oracle_kinds']} / {report['candidate_kinds']}",
    ]
    divergence = report.get("first_divergence")
    if divergence is None:
        lines.append("status      : EQUAL (canonical state hashes match)")
        return "\n".join(lines)
    lines.append("status      : DIFFERENT")
    lines.append(f"first drift : {_format_frame(divergence)}")
    lines.append(f"path        : {divergence.get('path', '/')}")
    if divergence.get("type") == "state":
        lines.append(f"oracle      : {divergence.get('oracle_value')!r}")
        lines.append(f"candidate   : {divergence.get('candidate_value')!r}")
        lines.append(f"hashes      : {divergence['oracle']['hash']} / "
                     f"{divergence['candidate']['hash']}")
    elif divergence.get("type") == "frame_count":
        lines.append(f"extra side  : {divergence.get('side')}")
        lines.append(f"frame       : {divergence.get('value')!r}")
    else:
        lines.append(f"oracle key  : {divergence.get('oracle_value')!r}")
        lines.append(f"candidate   : {divergence.get('candidate_value')!r}")
    return "\n".join(lines)


def _parse_patterns(values: Iterable[str]) -> tuple[str, ...]:
    return tuple(value.strip() for value in values if value.strip())


def _write_json(report: dict[str, Any], destination: str) -> None:
    encoded = json.dumps(report, sort_keys=True, indent=2, ensure_ascii=False) + "\n"
    if destination == "-":
        sys.stdout.write(encoded)
        return
    try:
        Path(destination).write_text(encoded, encoding="utf-8")
    except OSError as exc:
        raise TraceError(f"cannot write JSON report {destination}: {exc}") from exc


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--oracle", required=True, help="authoritative JSONL trace")
    parser.add_argument("--candidate", required=True, help="trace to compare")
    parser.add_argument(
        "--kinds", default=",".join(DEFAULT_KINDS),
        help="comma-separated event kinds (empty selects every kind)",
    )
    parser.add_argument("--from-turn", type=int, default=None)
    parser.add_argument("--to-turn", type=int, default=None)
    parser.add_argument(
        "--ignore", action="append", default=[], metavar="JSON_POINTER",
        help="field path to ignore; repeatable, supports * and **",
    )
    parser.add_argument(
        "--unordered", action="append", default=[], metavar="JSON_POINTER",
        help="list path whose elements are a mathematical set; repeatable",
    )
    parser.add_argument(
        "--require-contiguous", action="store_true",
        help="fail if selected turn numbers have a gap in either trace",
    )
    parser.add_argument(
        "--allow-trailing-partial", action="store_true",
        help="ignore one unterminated final JSON line (live-tail use only)",
    )
    parser.add_argument(
        "--json", nargs="?", const="-", metavar="PATH",
        help="write the machine-readable report to PATH, or stdout when omitted",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        kinds = [kind for kind in args.kinds.split(",") if kind.strip()]
        ignores = DEFAULT_IGNORES + _parse_patterns(args.ignore)
        unordered = DEFAULT_UNORDERED + _parse_patterns(args.unordered)
        oracle = load_trace(
            args.oracle, kinds=kinds, from_turn=args.from_turn, to_turn=args.to_turn,
            allow_trailing_partial=args.allow_trailing_partial,
            require_contiguous=args.require_contiguous,
        )
        candidate = load_trace(
            args.candidate, kinds=kinds, from_turn=args.from_turn, to_turn=args.to_turn,
            allow_trailing_partial=args.allow_trailing_partial,
            require_contiguous=args.require_contiguous,
        )
        report = compare_traces(oracle, candidate, ignores=ignores, unordered=unordered)
        if args.json is not None:
            _write_json(report, args.json)
        else:
            print(render_report(report))
        return 0 if report["equal"] else 1
    except TraceError as exc:
        print(f"trace contract error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
