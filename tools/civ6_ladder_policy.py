#!/usr/bin/env python3
"""Choose the next real-Civ VI ladder rung from measured live evidence.

The rule (operator, 2026-08-23): **play the highest rung the controller has
claimed until it has three wins there, then move up.** A rung is claimed by
its first win; three wins is what makes a rung earned rather than lucky. Two
wins in a trailing window of eight used to advance the seat, which is how
Chieftain was claimed on one win in eight and Warlord was played with none —
a window reads the recent record, and the recent record of a two-seat fleet
is whichever seat published last.

So the gate counts **wins**, over the whole record, and nothing else: a loss
is not evidence against a rung, it is a game that did not win. Only a game
the harness could confirm was the game it asked for — ``configured`` read back
from inside the running session — is evidence at all. And it reads the whole
fleet's record (``load_live``), not one seat's copy of it.

This module is deliberately read-only: it derives the next target from the
ledger, and the supervisor passes that target into the harness explicitly.

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
DEFAULT_WINS_REQUIRED = 3


def _positive_env(name: str, default: int) -> int:
    raw = os.environ.get(name, "").strip()
    if not raw:
        return default
    try:
        value = int(raw)
    except ValueError:
        return default
    return value if value > 0 else default


def wins_required_default() -> int:
    """The wins a rung needs before the seat moves above it, from the
    environment (``CIVVIS_LADDER_WINS_REQUIRED``) or the default of three."""
    return _positive_env("CIVVIS_LADDER_WINS_REQUIRED", DEFAULT_WINS_REQUIRED)


def comparable_attempt(attempt: dict[str, Any], difficulty: str) -> bool:
    """Whether one ledger row is evidence for this rung.

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

    `tools/test_civ6_ladder_policy.py` fails if a key appears here that no
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
    wins_required: int = DEFAULT_WINS_REQUIRED,
) -> dict[str, Any]:
    """What the record says about one rung: its comparable attempts, its wins,
    whether it is claimed, and whether it is earned (``wins_required`` wins)."""
    attempts = [
        attempt
        for attempt in state.get("attempts", [])
        if isinstance(attempt, dict) and comparable_attempt(attempt, difficulty)
    ]
    wins = sum(attempt.get("won") is True for attempt in attempts)
    claimed = difficulty in (state.get("wins") or {}) or wins > 0
    return {
        "difficulty": difficulty,
        "claimed": claimed,
        "comparable_attempts": len(attempts),
        "wins": wins,
        "wins_required": wins_required,
        "earned": claimed and wins >= wins_required,
    }


def next_target(
    state: dict[str, Any],
    *,
    wins_required: int = DEFAULT_WINS_REQUIRED,
) -> tuple[str, list[dict[str, Any]]]:
    """The rung to play: the highest claimed rung until it is earned, then the
    one above it. Nothing claimed means Settler; every rung earned means the
    ladder is finite and the seat stays on the top rung.

    A claimed rung below the highest one is not revisited however many wins it
    holds: the seat that claimed the rung above has answered the question the
    lower rung asks. The ``explain`` output still prints every rung's count.
    """
    statuses = [
        rung_status(state, difficulty, wins_required=wins_required)
        for difficulty, _label in civ6_ladder.LADDER
    ]
    claimed = [index for index, status in enumerate(statuses) if status["claimed"]]
    if not claimed:
        return civ6_ladder.LADDER[0][0], statuses
    highest = claimed[-1]
    if not statuses[highest]["earned"]:
        return civ6_ladder.LADDER[highest][0], statuses
    above = min(highest + 1, len(civ6_ladder.LADDER) - 1)
    return civ6_ladder.LADDER[above][0], statuses


def load_live(runs: Path) -> dict[str, Any]:
    """The FLEET's record: this seat's live ledger folded onto the committed one.

    ⚠⚠⚠ A RUNG IS A CLAIM ABOUT THE CONTROLLER, AND THIS READ ONE MACHINE'S
    COPY OF THE RECORD. `civ6_ladder.load` seeds a machine with no live ledger
    from the committed snapshot and then never looks at it again, so a second
    Civilization VI seat gates on a record that stopped at the moment it was
    seeded. Measured on 2026-08-23, that was not academic: the two seats
    answered `DIFFICULTY_CHIEFTAIN` and `DIFFICULTY_SETTLER` for the same
    controller on the same day, because 76 of one seat's games were in no
    published record.

    Reading the union is strictly more evidence: it adds the other seat's wins,
    and it is the only source that matches what `docs/CIV6_LADDER.md` claims to
    be — what the controller has beaten, not what one laptop remembers beating.
    A win recorded on the other seat and not yet published still counts only
    once it is published; that is what `civ6_ladder.py publish` is for.
    """
    live = civ6_ladder.load(civ6_ladder.live_ledger_for(runs))
    merged, _ = civ6_ladder.merge_state(civ6_ladder.load_snapshot(), live)
    return merged


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--runs", type=Path, default=RUNS_DEFAULT)
    parser.add_argument("--wins-required", type=int, default=None,
                        help="wins a rung needs before the seat moves above it "
                             f"(default {DEFAULT_WINS_REQUIRED}, or "
                             "CIVVIS_LADDER_WINS_REQUIRED)")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("target")
    sub.add_parser("explain")
    args = parser.parse_args(argv)

    wins_required = args.wins_required or wins_required_default()
    if wins_required <= 0:
        parser.error("wins-required must be positive")
    state = load_live(args.runs)
    target, statuses = next_target(state, wins_required=wins_required)
    if args.command == "target":
        print(target)
    else:
        print(json.dumps({"target": target, "rungs": statuses}, indent=2,
                         sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
