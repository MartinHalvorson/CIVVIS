#!/usr/bin/env python3
"""What live games recorded, read from their event logs.

Every real finding about the live agent has come from the same two moves: count
an event across recent games, then check it against the next `state` frame to
see what actually happened. This is those two moves, so they are not rewritten
each time.

Usage::

    python3 tools/civ6_events.py kinds              # every event kind, and its fields
    python3 tools/civ6_events.py failures           # failed orders, by type and reason
    python3 tools/civ6_events.py noops              # "the unit did not move", checked

Pass ``--runs N`` for more games, or ``--root DIR`` for another directory.
Line numbers in Lua errors refer to the installed agent; see civ6_agent_errors.py.
"""

from __future__ import annotations

import argparse
import collections
import json
import re
from pathlib import Path
from typing import Iterable, Iterator

RUN_ROOT = Path.home() / "civvis-civ6-runs" / "control"

# Fragments of a run's log so small they cannot hold a game.
MIN_GAME_BYTES = 1_000_000


def recent_games(root: Path, runs: int) -> list[Path]:
    """The newest event logs that hold a game, newest first."""
    logs = [path for path in root.glob("*/events.jsonl") if path.stat().st_size >= MIN_GAME_BYTES]
    return sorted(logs, key=lambda path: path.stat().st_mtime, reverse=True)[:runs]


def events(path: Path, needles: Iterable[str] = ()) -> Iterator[dict]:
    """Parsed events from one log. `needles` skip the JSON parse on other lines."""
    wanted = tuple(needles)
    with path.open(errors="replace") as handle:
        for line in handle:
            if wanted and not any(needle in line for needle in wanted):
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue  # a torn final line
            if isinstance(event, dict):
                yield event


def kinds(logs: list[Path]) -> dict[str, dict]:
    """kind -> {count, games, fields}."""
    table: dict[str, dict] = {}
    for path in logs:
        for event in events(path):
            kind = str(event.get("kind"))
            row = table.setdefault(kind, {"count": 0, "games": set(), "fields": set()})
            row["count"] += 1
            row["games"].add(path.parent.name)
            row["fields"].update(event)
    return table


def _verb_head(verb: object) -> str:
    return re.split(r"[:=,]", str(verb or ""))[0][:24]


def failures(logs: list[Path]) -> list[dict]:
    """Failed orders per (kind, verb), worst rate first."""
    verified: collections.Counter = collections.Counter()
    failed: dict[tuple, collections.Counter] = collections.defaultdict(collections.Counter)
    for path in logs:
        for event in events(path, ('"order_failed"', '"order_verified"')):
            key = (event.get("order_kind"), _verb_head(event.get("verb")))
            if event.get("kind") == "order_verified":
                verified[key] += 1
            elif event.get("kind") == "order_failed":
                failed[key][re.sub(r"\d+", "N", str(event.get("reason")))] += 1
    rows = []
    for key in set(verified) | set(failed):
        bad = sum(failed[key].values())
        total = verified[key] + bad
        rows.append(
            {"kind": key[0], "verb": key[1], "total": total, "failed": bad,
             "rate": bad / total, "reasons": failed[key].most_common(3)}
        )
    return sorted(rows, key=lambda row: (-row["rate"], -row["total"]))


def noop_outcomes(logs: list[Path]) -> dict[str, collections.Counter]:
    """For each `move_noop` reason, where the unit stood at the next state frame."""
    table: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for path in logs:
        pending: list[dict] = []
        for event in events(path, ('"move_noop"', '"kind": "state"')):
            if event.get("kind") == "move_noop":
                pending.append(event)
            elif event.get("kind") == "state" and pending:
                where = {unit.get("id"): (unit.get("x"), unit.get("y")) for unit in event.get("units") or []}
                for noop in pending:
                    now = where.get(noop.get("unit"))
                    if now is None:
                        outcome = "gone"
                    elif now == tuple(noop["want"]):
                        outcome = "reached"
                    elif now == tuple(noop["from"]):
                        outcome = "stayed"
                    else:
                        outcome = "elsewhere"
                    table[str(noop.get("why"))][outcome] += 1
                pending = []
    return table


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("report", choices=("kinds", "failures", "noops"))
    parser.add_argument("--runs", type=int, default=8, help="newest games to read")
    parser.add_argument("--root", type=Path, default=RUN_ROOT)
    args = parser.parse_args()
    logs = recent_games(args.root, args.runs)
    print(f"{len(logs)} games read")
    if args.report == "kinds":
        for kind, row in sorted(kinds(logs).items(), key=lambda item: -item[1]["count"]):
            fields = ",".join(sorted(row["fields"] - {"kind", "ctx", "run", "t", "utc"}))
            print(f"{row['count']:8} in {len(row['games']):3} games  {kind:28} {fields[:110]}")
    elif args.report == "failures":
        for row in failures(logs):
            if row["total"] >= 8:
                reasons = ", ".join(f"{name} {count}" for name, count in row["reasons"])
                print(f"{row['rate']:4.0%} of {row['total']:6}  {str(row['kind']):14} {row['verb']:24} {reasons}")
    else:
        for why, outcomes in sorted(noop_outcomes(logs).items(), key=lambda item: -sum(item[1].values())):
            print(f"{why:18} {dict(outcomes)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
