#!/usr/bin/env python3
"""Extract a concrete repeating-unit-blocker signal from a CIVVIS event log.

The GUI controller emits a ``blocked`` event every time Civ VI refuses to end a
turn. Most such notifications clear on the next controller pass. A repeated
unit notification on the same turn is different: the game is waiting for an
order while the controller keeps reporting that it has already made its
complete pass. No turn can advance from that state.

This dependency-free reader is separate from the shell watchdog so its
interpretation of the live JSONL evidence is unit-tested.
"""

from __future__ import annotations

import argparse
import json
from collections.abc import Iterable
from pathlib import Path
from typing import Any


UNIT_BLOCKERS = frozenset({
    "ENDTURN_BLOCKING_UNITS",
    "ENDTURN_BLOCKING_UNIT_NEEDS_ORDERS",
    "ENDTURN_BLOCKING_STACKED_UNITS",
})
TERMINAL_KINDS = frozenset({"outcome", "summary", "finished"})


def _turn(event: dict[str, Any]) -> int | None:
    try:
        return int(event["turn"])
    except (KeyError, TypeError, ValueError):
        return None


def repeating_unit_blocker(
    events: Iterable[dict[str, Any]],
) -> tuple[int, str, int] | None:
    """Return the newest unresolved unit-blocker turn and its sightings.

    The count is scoped to one ``(turn, blocker)`` tuple. A normal turn may
    report one or two unit reminders; repeated notifications on that same turn
    prove that the controller's recovery has failed. A later turn or terminal
    event makes an older notification irrelevant.
    """

    latest: tuple[int, str] | None = None
    counts: dict[tuple[int, str], int] = {}
    latest_turn = -1
    terminal_turns: set[int] = set()

    for event in events:
        kind = event.get("kind")
        turn = _turn(event)
        if turn is not None and kind == "turn":
            latest_turn = max(latest_turn, turn)
        if turn is not None and kind in TERMINAL_KINDS:
            terminal_turns.add(turn)
        if kind != "blocked" or turn is None:
            continue
        blocker = event.get("blocker")
        if blocker not in UNIT_BLOCKERS:
            continue
        key = (turn, str(blocker))
        counts[key] = counts.get(key, 0) + 1
        latest = key

    if latest is None:
        return None
    turn, blocker = latest
    if latest_turn > turn or any(done_turn >= turn for done_turn in terminal_turns):
        return None
    return turn, blocker, counts[latest]


def events_from(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    try:
        with path.open(encoding="utf-8") as handle:
            for line in handle:
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if isinstance(event, dict):
                    events.append(event)
    except OSError:
        return []
    return events


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Print the newest repeating Civ VI unit blocker, if any."
    )
    parser.add_argument("events", type=Path)
    args = parser.parse_args()
    result = repeating_unit_blocker(events_from(args.events))
    if result is not None:
        print(*result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
