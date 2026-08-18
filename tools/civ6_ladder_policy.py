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
``repeat_wins`` wins.  A settings mismatch, blocked start, or unconfigured game
is not evidence for repeatability.

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
    """Whether one ledger row is safe to compare with this rung's batch."""
    return (
        attempt.get("configured") is True
        and attempt.get("difficulty") == difficulty
        and isinstance(attempt.get("won"), bool)
        and not attempt.get("settings_mismatch")
        and not attempt.get("blocked")
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
    tail = attempts[-window:]
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
    return civ6_ladder.load(civ6_ladder.live_ledger_for(runs))


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
