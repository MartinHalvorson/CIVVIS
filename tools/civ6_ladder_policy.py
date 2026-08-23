#!/usr/bin/env python3
"""Choose the next real-Civ VI ladder rung from measured live evidence.

The game supervisor used to run one Settler attempt per source revision and
never consulted the ladder.  That made a rung look active without ever defining
when it was repeatable, and it gave the supervisor no path to Chieftain.  This
module is deliberately read-only: it derives the next target from the live
ledger, and the supervisor passes that target into the harness explicitly.

The policy is conservative by design.  A rung is not advanced merely because
it has one historical win.  The trailing comparable window must contain the
configured rung, at least ``min_attempts`` valid outcomes, and
``repeat_wins`` wins.  A game the harness could not confirm was the game it
asked for -- ``configured`` read back from inside the running session -- is not
evidence for repeatability.

Usage::

    python tools/civ6_ladder_policy.py --runs ~/civvis-civ6-runs/control target
    python tools/civ6_ladder_policy.py --runs ~/civvis-civ6-runs/control explain
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
from typing import Any

import civ6_ladder


RUNS_DEFAULT = Path.home() / "civvis-civ6-runs" / "control"
DEFAULT_WINDOW = 8
DEFAULT_REPEAT_WINS = 2
DEFAULT_MIN_ATTEMPTS = 3


def _positive_env(name: str, default: int) -> int:
    raw = os.environ.get(name, "").strip()
    if not raw:
        return default
    try:
        value = int(raw)
    except ValueError:
        return default
    return value if value > 0 else default


def policy_defaults() -> tuple[int, int, int]:
    """Return window, required wins, and minimum outcomes from the environment."""
    return (
        _positive_env("CIVVIS_LADDER_WINDOW", DEFAULT_WINDOW),
        _positive_env("CIVVIS_LADDER_REPEAT_WINS", DEFAULT_REPEAT_WINS),
        _positive_env("CIVVIS_LADDER_MIN_ATTEMPTS", DEFAULT_MIN_ATTEMPTS),
    )


def comparable_attempt(attempt: dict[str, Any], difficulty: str) -> bool:
    """Whether one ledger row is safe to compare with this rung's batch.

    ⚠⚠ EVERY KEY READ HERE MUST BE A KEY `civ6_ladder.entry_from` WRITES.
    This predicate used to add `and not attempt.get("settings_mismatch")` and
    `and not attempt.get("blocked")`, and `entry_from` writes neither: measured
    across the 325 rows of the live ledger, **0 carried either key**. They read
    as two extra safety conditions and were two guards that could not fire —
    this repository's own "a claim is not a check" defect, in the file that
    decides when a difficulty rung has been beaten. Both were removed rather
    than populated, because on this file neither can ever have a value:

    * `blocked` is `civ6_civvis_climb.py`'s field on its own `civvis_ladder.jsonl`
      row, and a start that produced no game produces no `summary.json` either —
      `civ6_play._play` returns before writing one — so a blocked start never
      becomes a ledger row at all;
    * `settings_mismatch` is the same file's asked-versus-dealt comparison of
      the launcher's own echo. The ledger's equivalent is `configured`, which
      `civ6_play.py` reads back from INSIDE the running game (difficulty, size,
      speed, map script, leader, modes, ruleset) and which this predicate
      already requires — strictly stronger evidence than comparing a request
      with the harness's memory of it.

    `tools/test_civ6_ladder_policy.py` now fails if a key appears here that no
    ledger row carries, so the deletion cannot quietly come back.
    """
    return (
        attempt.get("configured") is True
        and attempt.get("difficulty") == difficulty
        and isinstance(attempt.get("won"), bool)
    )


def rung_status(
    state: dict[str, Any],
    difficulty: str,
    *,
    window: int = DEFAULT_WINDOW,
    repeat_wins: int = DEFAULT_REPEAT_WINS,
    min_attempts: int = DEFAULT_MIN_ATTEMPTS,
) -> dict[str, Any]:
    """Summarize the evidence needed before moving past ``difficulty``."""
    attempts = [
        attempt
        for attempt in state.get("attempts", [])
        if isinstance(attempt, dict) and comparable_attempt(attempt, difficulty)
    ]
    wins = [attempt for attempt in attempts if attempt.get("won") is True]
    # ⚠⚠ THE TRAILING WINDOW IS THE NEWEST GAMES, NOT THE LAST ROWS APPENDED,
    # AND THOSE STOPPED BEING THE SAME THING THE DAY `sync` WAS WRITTEN.
    # `civ6_ladder.apply` already carries this correction one file over, in
    # its own words: insertion order wearing chronology's name. It fixed the
    # rung MILESTONE and left the rung GATE reading arrival order, so a
    # backfill of week-old games -- which is exactly what `sync` and a merged
    # publish produce -- redefines "the last eight attempts" as those old
    # games and can hand the supervisor a rung on evidence that predates
    # everything it has played since. `utc` is an ISO-8601 Z stamp, so a
    # string compare is a time compare; a row with no stamp cannot claim to be
    # recent and sorts oldest, and the sort is stable so same-stamp rows keep
    # the order they were recorded in.
    tail = sorted(attempts, key=lambda row: row.get("utc") or "")[-window:]
    tail_wins = sum(attempt.get("won") is True for attempt in tail)
    claimed = difficulty in (state.get("wins") or {}) or bool(wins)
    repeatable = claimed and len(tail) >= min_attempts and tail_wins >= repeat_wins
    return {
        "difficulty": difficulty,
        "claimed": claimed,
        "comparable_attempts": len(attempts),
        "wins": len(wins),
        "window": window,
        "window_attempts": len(tail),
        "window_wins": tail_wins,
        "repeat_wins_required": repeat_wins,
        "min_attempts_required": min_attempts,
        "repeatable": repeatable,
    }


def next_target(
    state: dict[str, Any],
    *,
    window: int = DEFAULT_WINDOW,
    repeat_wins: int = DEFAULT_REPEAT_WINS,
    min_attempts: int = DEFAULT_MIN_ATTEMPTS,
) -> tuple[str, list[dict[str, Any]]]:
    """Return the lowest rung that still needs evidence or a first win."""
    statuses: list[dict[str, Any]] = []
    for difficulty, _label in civ6_ladder.LADDER:
        status = rung_status(
            state,
            difficulty,
            window=window,
            repeat_wins=repeat_wins,
            min_attempts=min_attempts,
        )
        statuses.append(status)
        if not status["repeatable"]:
            return difficulty, statuses
    # The ladder is finite. Keep the supervisor on the highest rung once every
    # rung has been claimed and made repeatable instead of emitting an invalid
    # difficulty or silently stopping its measurement loop.
    return civ6_ladder.LADDER[-1][0], statuses


def load_live(runs: Path) -> dict[str, Any]:
    """The FLEET's record: this seat's live ledger folded onto the committed one.

    ⚠⚠⚠ A RUNG IS A CLAIM ABOUT THE CONTROLLER, AND THIS READ ONE MACHINE'S
    COPY OF THE RECORD. `civ6_ladder.load` seeds a machine with no live ledger
    from the committed snapshot and then never looks at it again, so a second
    Civilization VI seat gates on a record that stopped at the moment it was
    seeded. Measured on 2026-08-23, that was not academic: the published
    snapshot said Settler was repeatable -- two wins in its trailing window --
    and it said so only because 76 Settler games from the other seat, whose
    last eight were all losses, were in no published record at all. The two
    seats therefore answered `DIFFICULTY_CHIEFTAIN` and `DIFFICULTY_SETTLER`
    for the same controller on the same day.

    Reading the union is strictly more evidence, in both directions: it adds
    the other seat's wins and the other seat's losses, and it is the only
    source that matches what `docs/CIV6_LADDER.md` claims to be -- what the
    controller has beaten, not what one laptop remembers beating.
    """
    live = civ6_ladder.load(civ6_ladder.live_ledger_for(runs))
    merged, _ = civ6_ladder.merge_state(civ6_ladder.load_snapshot(), live)
    return merged


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--runs", type=Path, default=RUNS_DEFAULT)
    parser.add_argument("--window", type=int, default=None)
    parser.add_argument("--repeat-wins", type=int, default=None)
    parser.add_argument("--min-attempts", type=int, default=None)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("target")
    sub.add_parser("explain")
    args = parser.parse_args(argv)

    defaults = policy_defaults()
    window = args.window or defaults[0]
    repeat_wins = args.repeat_wins or defaults[1]
    min_attempts = args.min_attempts or defaults[2]
    if min(window, repeat_wins, min_attempts) <= 0:
        parser.error("window, repeat-wins, and min-attempts must be positive")
    state = load_live(args.runs)
    target, statuses = next_target(
        state,
        window=window,
        repeat_wins=repeat_wins,
        min_attempts=min_attempts,
    )
    if args.command == "target":
        print(target)
    else:
        print(json.dumps({"target": target, "rungs": statuses}, indent=2,
                         sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
