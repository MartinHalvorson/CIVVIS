#!/usr/bin/env python3
"""Validate and count an all-seats continuous gene-screen ledger.

The JSONL is deliberately *not* a one-line-per-game log.  Every segment has
one ``kind: header`` record and every completed game has one ``kind: game``
record for each major seat.  Counting nonblank lines as games therefore turns a
six-player run into an approximately sixfold overstatement.  This tool is the
only supported live-status reader: it groups rows by the durable game key
(``seed``, ``arm``), proves that every group has exactly one row for every
seat and exactly one winner, then reports games and seats separately.

It rejects an in-flight partial write, duplicate seat, inconsistent winner,
undeclared seed, or malformed pre-registration rather than printing a
plausible but false number.  ``--analysis`` additionally makes the frozen
``gene_screen --analyze --json`` artefact agree with the validated ledger.
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any


class LedgerError(ValueError):
    """A continuous-screen ledger cannot support a trustworthy status."""


@dataclass(frozen=True)
class Segment:
    """One unique, pre-registered all-seats seed window."""

    players: int
    seed_first: int
    seed_last: int
    target_games: int
    target_seats: int


def _integer(value: Any, *, name: str, line: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise LedgerError(f"line {line}: {name} must be an integer")
    return value


def _segment(record: dict[str, Any], line: int) -> Segment:
    players = _integer(record.get("players"), name="header players", line=line)
    if players < 2:
        raise LedgerError(f"line {line}: header players must be at least two")
    if record.get("all_seats") is not True:
        raise LedgerError(
            f"line {line}: continuous status requires an all-seats header")
    if record.get("design") != "independent":
        raise LedgerError(
            f"line {line}: continuous status requires the independent design")
    batch = record.get("batch")
    if not isinstance(batch, dict):
        raise LedgerError(f"line {line}: header has no pre-registered batch")
    target_games = _integer(batch.get("target_games"), name="target_games", line=line)
    target_seats = _integer(batch.get("target_seats"), name="target_seats", line=line)
    seed_first = _integer(batch.get("seed_first"), name="seed_first", line=line)
    seed_last = _integer(batch.get("seed_last"), name="seed_last", line=line)
    if target_games < 1:
        raise LedgerError(f"line {line}: target_games must be positive")
    if seed_last - seed_first + 1 != target_games:
        raise LedgerError(
            f"line {line}: seed window {seed_first}..{seed_last} does not reserve "
            f"target_games={target_games}")
    if target_seats != target_games * players:
        raise LedgerError(
            f"line {line}: target_seats={target_seats} is not "
            f"target_games × players ({target_games * players})")
    return Segment(players, seed_first, seed_last, target_games, target_seats)


def _read(path: Path) -> tuple[list[Segment], list[tuple[int, dict[str, Any]]], int]:
    """Read the current file, retaining physical record count separately."""
    segments: dict[tuple[int, int], Segment] = {}
    rows: list[tuple[int, dict[str, Any]]] = []
    records = 0
    try:
        source = path.open(encoding="utf-8")
    except OSError as error:
        raise LedgerError(f"cannot open {path}: {error}") from error
    with source:
        for line_number, text in enumerate(source, 1):
            if not text.strip():
                continue
            records += 1
            try:
                record = json.loads(text)
            except json.JSONDecodeError as error:
                raise LedgerError(f"line {line_number}: invalid JSON: {error.msg}") from error
            if not isinstance(record, dict):
                raise LedgerError(f"line {line_number}: JSON record must be an object")
            kind = record.get("kind")
            if kind == "header":
                segment = _segment(record, line_number)
                key = (segment.seed_first, segment.seed_last)
                previous = segments.get(key)
                if previous is not None and previous != segment:
                    raise LedgerError(
                        f"line {line_number}: duplicate seed window {key[0]}..{key[1]} "
                        "has a different declared shape")
                segments[key] = segment
            elif kind == "game":
                rows.append((line_number, record))
            else:
                raise LedgerError(
                    f"line {line_number}: expected kind 'header' or 'game', got {kind!r}")
    if not segments:
        raise LedgerError("no continuous-screen header records found")
    ordered = sorted(segments.values(), key=lambda segment: segment.seed_first)
    for prior, current in zip(ordered, ordered[1:]):
        if current.seed_first <= prior.seed_last:
            raise LedgerError(
                f"overlapping seed windows {prior.seed_first}..{prior.seed_last} and "
                f"{current.seed_first}..{current.seed_last}")
    return ordered, rows, records


def _segment_for(seed: int, segments: list[Segment], line: int) -> Segment:
    for segment in segments:
        if segment.seed_first <= seed <= segment.seed_last:
            return segment
    raise LedgerError(f"line {line}: game seed {seed} is outside every declared window")


def summarize(path: Path) -> dict[str, Any]:
    """Return a validated status; do not return a number for malformed data."""
    segments, rows, records = _read(path)
    games: dict[tuple[int, int], list[tuple[int, dict[str, Any]]]] = defaultdict(list)
    for line, row in rows:
        seed = _integer(row.get("seed"), name="game seed", line=line)
        arm = _integer(row.get("arm", 0), name="game arm", line=line)
        _segment_for(seed, segments, line)
        games[(seed, arm)].append((line, row))

    for (seed, arm), group in games.items():
        segment = _segment_for(seed, segments, group[0][0])
        if len(group) != segment.players:
            raise LedgerError(
                f"game seed={seed} arm={arm} has {len(group)} seat rows; "
                f"expected exactly {segment.players}. The ledger may be mid-write.")
        seats = [_integer(row.get("seat"), name="seat", line=line) for line, row in group]
        if sorted(seats) != list(range(segment.players)):
            raise LedgerError(
                f"game seed={seed} arm={arm} has seats {sorted(seats)!r}; "
                f"expected {list(range(segment.players))!r}")
        game_ids = {_integer(row.get("game"), name="game", line=line) for line, row in group}
        if len(game_ids) != 1:
            raise LedgerError(
                f"game seed={seed} arm={arm} has inconsistent game ordinals {sorted(game_ids)!r}")
        winners = [row for _, row in group if row.get("win") is True]
        if len(winners) != 1:
            raise LedgerError(
                f"game seed={seed} arm={arm} has {len(winners)} winning seats; expected one")
        winner = winners[0]["seat"]
        if any(row.get("winner") != winner for _, row in group):
            raise LedgerError(
                f"game seed={seed} arm={arm} does not agree on winner seat {winner}")

    game_count = len(games)
    seat_count = len(rows)
    target_games = sum(segment.target_games for segment in segments)
    target_seats = sum(segment.target_seats for segment in segments)
    played_seeds = sorted(seed for seed, _arm in games)
    return {
        "schema": "continuous_gene_screen_status/v1",
        "complete_games": game_count,
        "complete_seats": seat_count,
        "wins": game_count,
        "records": records,
        "header_records": records - seat_count,
        "seat_records": seat_count,
        "target_games": target_games,
        "target_seats": target_seats,
        "partial": seat_count < target_seats,
        "reserved_seed_windows": [
            {
                "seed_first": segment.seed_first,
                "seed_last": segment.seed_last,
                "target_games": segment.target_games,
                "target_seats": segment.target_seats,
                "players": segment.players,
            }
            for segment in segments
        ],
        "played_seed_window": ([played_seeds[0], played_seeds[-1]] if played_seeds else None),
    }


def validate_analysis(summary: dict[str, Any], path: Path) -> None:
    """Require the frozen analyzer output to state the same completed batch."""
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise LedgerError(f"cannot open analysis {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise LedgerError(f"analysis {path}: invalid JSON: {error.msg}") from error
    if not isinstance(data, dict) or data.get("kind") != "gene_screen_analysis":
        raise LedgerError(f"analysis {path}: not a gene_screen analysis")
    batch = data.get("batch")
    if not isinstance(batch, dict):
        raise LedgerError(f"analysis {path}: missing batch object")
    checks = {
        "games": summary["complete_games"],
        "seats": summary["complete_seats"],
        "batch.complete_games": summary["complete_games"],
        "batch.complete_seats": summary["complete_seats"],
        "batch.target_games": summary["target_games"],
        "batch.target_seats": summary["target_seats"],
    }
    actual = {
        "games": data.get("games"),
        "seats": data.get("seats"),
        "batch.complete_games": batch.get("complete_games"),
        "batch.complete_seats": batch.get("complete_seats"),
        "batch.target_games": batch.get("target_games"),
        "batch.target_seats": batch.get("target_seats"),
    }
    for field, expected in checks.items():
        if actual[field] != expected:
            raise LedgerError(
                f"analysis {path}: {field}={actual[field]!r}, expected {expected!r} "
                "from the validated JSONL")


def render(summary: dict[str, Any]) -> str:
    windows = ", ".join(
        f"{window['seed_first']}..{window['seed_last']}"
        for window in summary["reserved_seed_windows"])
    state = "partial" if summary["partial"] else "complete"
    return "\n".join((
        "continuous gene-screen status",
        f"  complete: {summary['complete_games']:,} games / {summary['complete_seats']:,} seats "
        f"({summary['wins']:,} wins)",
        f"  target:   {summary['target_games']:,} games / {summary['target_seats']:,} seats ({state})",
        f"  records:  {summary['records']:,} = {summary['header_records']:,} header records + "
        f"{summary['seat_records']:,} seat records (never a game count)",
        f"  reserved seeds: {windows}",
    ))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rows", type=Path, help="continuous rows-continuous.jsonl")
    parser.add_argument("--analysis", type=Path,
                        help="frozen gene_screen --analyze --json output to cross-check")
    parser.add_argument("--json", action="store_true", help="write the validated status JSON")
    args = parser.parse_args(argv)
    try:
        status = summarize(args.rows)
        if args.analysis is not None:
            validate_analysis(status, args.analysis)
    except LedgerError as error:
        print(f"continuous gene-screen status: {error}", file=sys.stderr)
        return 2
    print(json.dumps(status, indent=2, sort_keys=True) if args.json else render(status))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
