#!/usr/bin/env python3
"""Extract concrete unattended-game recovery signals from CIVVIS state.

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
import sys
import time
from collections.abc import Iterable
from datetime import datetime
from pathlib import Path
from typing import Any


UNIT_BLOCKERS = frozenset({
    "ENDTURN_BLOCKING_UNITS",
    "ENDTURN_BLOCKING_UNIT_NEEDS_ORDERS",
    "ENDTURN_BLOCKING_STACKED_UNITS",
})
TERMINAL_KINDS = frozenset({"outcome", "summary", "finished"})
# ⚠⚠ SIGHTINGS ARE NOT TIME. Six sightings once took about two minutes, because
# the mod re-raised a stuck blocker every few seconds. Once the mod answered
# within 0.1 s (2026-10-04), six sightings fit inside one healthy turn. Run
# `civvis-20261004T192700Z` logged seven `blocked ENDTURN_BLOCKING_UNITS` at
# t105 in 0.7 s, the end-turn force cleared them, t106 landed 3 s later, and
# the watchdog still killed the game four seconds after that and reloaded its
# autosave. A wedge is a blocker that is still there a minute after it was
# first seen, so the CLI requires that age as well as the count.
BLOCKER_MIN_AGE_S = 60.0
GAME_TERMINAL_KINDS = TERMINAL_KINDS | frozenset({"victory"})


def _turn(event: dict[str, Any]) -> int | None:
    try:
        return int(event["turn"])
    except (KeyError, TypeError, ValueError):
        return None


def _utc_seconds(event: dict[str, Any]) -> float | None:
    """Return the relay's receipt stamp as epoch seconds, if it has one."""
    stamp = event.get("utc")
    if not isinstance(stamp, str):
        return None
    try:
        return datetime.fromisoformat(stamp.replace("Z", "+00:00")).timestamp()
    except ValueError:
        return None


def repeating_unit_blocker(
    events: Iterable[dict[str, Any]],
    *,
    now: float | None = None,
    min_age_s: float = 0.0,
) -> tuple[int, str, int] | None:
    """Return the newest unresolved unit-blocker turn and its sightings.

    The count is scoped to one ``(turn, blocker)`` tuple. A normal turn may
    report one or two unit reminders; repeated notifications on that same turn
    prove that the controller's recovery has failed. A later turn or terminal
    event makes an older notification irrelevant.

    With ``now`` and ``min_age_s``, a blocker whose first sighting on its turn
    is younger than ``min_age_s`` is not reported yet. A sighting without a
    ``utc`` stamp cannot be aged and counts as old.
    """

    latest: tuple[int, str] | None = None
    counts: dict[tuple[int, str], int] = {}
    first_seen: dict[tuple[int, str], float | None] = {}
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
        if key not in first_seen:
            first_seen[key] = _utc_seconds(event)
        latest = key

    if latest is None:
        return None
    turn, blocker = latest
    if latest_turn > turn or any(done_turn >= turn for done_turn in terminal_turns):
        return None
    seen = first_seen.get(latest)
    if now is not None and seen is not None and now - seen < min_age_s:
        return None
    return turn, blocker, counts[latest]


def _nonnegative_int(value: Any) -> int | None:
    """Return an integer status field, excluding bools and negative values."""

    if isinstance(value, bool):
        return None
    try:
        number = int(value)
    except (TypeError, ValueError):
        return None
    return number if number >= 0 else None


def _terminal_game_event(event: dict[str, Any]) -> bool:
    """Whether this event ends our current game rather than a rival's seat."""

    kind = event.get("kind")
    return (kind in GAME_TERMINAL_KINDS
            or (kind == "defeat" and bool(event.get("ours"))))


def synchronized_progress_token(
    status: dict[str, Any],
    events: Iterable[dict[str, Any]],
    *,
    max_turn_skew: int = 1,
) -> tuple[int, int, int, int] | None:
    """Return a stable progress token only for the current, live game.

    A native ``/status`` response can belong to a mirror whose follower died
    before a new game began. Restarting the game from that stale page would
    throw away a healthy attempt. Therefore the no-progress watchdog is armed
    only after its mirror turn agrees (within one turn) with a ``turn`` event
    in *this* run's JSONL. The token changes for a new mirror process, turn,
    published frame, or locally recorded turn.

    Non-turn events are deliberately not part of the token. A modal can emit
    the same blocked notice forever while no turn or frame advances; that is
    exactly the frozen state this guard must recover. Terminal events disarm
    it so a result screen is left to the normal harness teardown.
    """

    if max_turn_skew < 0:
        return None
    mirror_turn = _nonnegative_int(status.get("turn"))
    frame_sequence = _nonnegative_int(status.get("frame_sequence"))
    server_instance = _nonnegative_int(status.get("server_instance"))
    if (mirror_turn is None or frame_sequence is None
            or server_instance is None):
        return None

    latest_turn: int | None = None
    for event in events:
        if _terminal_game_event(event):
            return None
        kind = event.get("kind")
        if kind != "turn":
            continue
        turn = _turn(event)
        if turn is not None:
            latest_turn = max(latest_turn, turn) if latest_turn is not None else turn

    # A game has not started merely because a staged mirror reports turn zero.
    # Requiring a local turn also prevents a previous game visible at :8610
    # from being mistaken for the brand-new run that is still in setup.
    if latest_turn is None or latest_turn < 1 or mirror_turn < 1:
        return None
    if abs(mirror_turn - latest_turn) > max_turn_skew:
        return None
    return server_instance, mirror_turn, frame_sequence, latest_turn


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
        description="Print a watchdog signal from a CIVVIS event log."
    )
    parser.add_argument("events", type=Path, nargs="?",
                        help="event log for the repeating-unit-blocker signal")
    parser.add_argument("--progress", type=Path, metavar="EVENTS",
                        help="read /status JSON from stdin and print a synchronized progress token")
    parser.add_argument("--blocker-min-age", type=float, default=BLOCKER_MIN_AGE_S,
                        help="seconds a unit blocker must have persisted on its turn")
    parser.add_argument("--max-turn-skew", type=int, default=1,
                        help="largest allowed mirror/local turn difference for --progress")
    args = parser.parse_args()
    if args.progress is not None:
        try:
            status = json.load(sys.stdin)
        except (json.JSONDecodeError, OSError, TypeError, ValueError):
            return 0
        if not isinstance(status, dict):
            return 0
        token = synchronized_progress_token(
            status,
            events_from(args.progress),
            max_turn_skew=args.max_turn_skew,
        )
        if token is not None:
            print(*token)
        return 0
    if args.events is None:
        parser.error("events is required unless --progress is used")
    result = repeating_unit_blocker(events_from(args.events), now=time.time(),
                                    min_age_s=args.blocker_min_age)
    if result is not None:
        print(*result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
