#!/usr/bin/env python3
"""Keep the record of which Civilization VI difficulties the controller has beaten.

A rung is claimed by evidence, not by a run finishing. ``record`` accepts a
run summary written by ``tools/civ6_play.py`` and only counts it as a win when
three things hold together:

- the game reported a victory naming the controller's own team,
- the game that was played is the one the run asked for (the summary's
  ``configured`` flag, which comes from the in-game settings marker rather than
  from the command line), and
- the difficulty in the summary is a rung on the ladder.

The second condition is the one that matters. A run started from the main menu
carries the menu's defaults, so a summary can say "Settler" while the game was
Prince; without the marker the ledger would record a rung that was never
climbed. See ``docs/CIV6_COMPUTER_CONTROL.md``.

Usage::

    python tools/civ6_ladder.py record ~/civvis-civ6-runs/control/<tag>/summary.json
    python tools/civ6_ladder.py show
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LEDGER = REPO / "docs" / "CIV6_LADDER.md"
DATA = REPO / "docs" / "civ6_ladder.json"

LADDER = [
    ("DIFFICULTY_SETTLER", "Settler"),
    ("DIFFICULTY_CHIEFTAIN", "Chieftain"),
    ("DIFFICULTY_WARLORD", "Warlord"),
    ("DIFFICULTY_PRINCE", "Prince"),
    ("DIFFICULTY_KING", "King"),
    ("DIFFICULTY_EMPEROR", "Emperor"),
    ("DIFFICULTY_IMMORTAL", "Immortal"),
    ("DIFFICULTY_DEITY", "Deity"),
]
NAMES = dict(LADDER)


def load() -> dict:
    if DATA.is_file():
        return json.loads(DATA.read_text())
    return {"attempts": [], "wins": {}}


def save(state: dict) -> None:
    DATA.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")


def is_win(summary: dict) -> bool:
    outcome = summary.get("outcome") or {}
    return bool(outcome.get("kind") == "victory" and outcome.get("won"))


def record(summary_path: Path) -> int:
    summary = json.loads(summary_path.read_text())
    state = load()
    difficulty = summary.get("difficulty")
    entry = {
        "tag": summary.get("tag"),
        "utc": summary.get("finished_utc") or datetime.now(timezone.utc)
            .strftime("%Y-%m-%dT%H:%M:%SZ"),
        "difficulty": difficulty,
        "configured": bool(summary.get("configured")),
        "won": is_win(summary),
        "victory": (summary.get("outcome") or {}).get("victory"),
        "turns": summary.get("last_turn"),
        "score": summary.get("last_score"),
        "map_size": summary.get("map_size"),
        "speed": summary.get("speed"),
        "seed": summary.get("seed"),
        "reason": summary.get("reason"),
    }
    state["attempts"].append(entry)

    if entry["won"] and entry["configured"] and difficulty in NAMES:
        prior = state["wins"].get(difficulty)
        # First win stands. A later one does not move the timestamp -- the
        # milestone is when the rung was first climbed, not the most recent
        # time it was repeated.
        if prior is None:
            state["wins"][difficulty] = entry
    elif entry["won"] and not entry["configured"]:
        print("won, but the game was not the one this run configured; "
              "not claiming the rung", file=sys.stderr)

    save(state)
    write_markdown(state)
    return 0


def write_markdown(state: dict) -> None:
    wins = state["wins"]
    attempts = state["attempts"]
    lines = [
        "# The Civilization VI difficulty ladder",
        "",
        "What the controller in `tools/civ6_control` has actually beaten, and when.",
        "A rung is claimed only by a victory event naming the controller's own team,",
        "in a game whose settings marker proves it was the game the run configured.",
        "`tools/civ6_ladder.py` writes this file; do not edit it by hand.",
        "",
        "| rung | difficulty | beaten (UTC) | victory | turns | run |",
        "|---|---|---|---|---|---|",
    ]
    for index, (key, label) in enumerate(LADDER, start=1):
        win = wins.get(key)
        if win:
            lines.append(f"| {index} | {label} | {win['utc']} | "
                         f"{win.get('victory') or '?'} | {win.get('turns')} | "
                         f"`{win.get('tag')}` |")
        else:
            lines.append(f"| {index} | {label} | — | | | |")
    lines += ["", f"Attempts recorded: {len(attempts)}.", ""]

    if attempts:
        lines += [
            "## Every attempt",
            "",
            "| run | difficulty | configured | outcome | turns | score | ended |",
            "|---|---|---|---|---|---|---|",
        ]
        for a in attempts[-40:]:
            outcome = "win" if a["won"] else (a.get("reason") or "—")
            lines.append(
                f"| `{a.get('tag')}` | {NAMES.get(a.get('difficulty'), a.get('difficulty'))} "
                f"| {'yes' if a['configured'] else 'NO'} | {outcome} "
                f"| {a.get('turns')} | {a.get('score')} | {a.get('utc')} |")
        lines.append("")
    LEDGER.write_text("\n".join(lines))


def show() -> int:
    state = load()
    if not LEDGER.is_file():
        write_markdown(state)
    print(LEDGER.read_text())
    return 0


def next_rung() -> int:
    """Print the lowest difficulty not yet beaten, for a driver to pick up."""
    state = load()
    for key, _ in LADDER:
        if key not in state["wins"]:
            print(key)
            return 0
    print("")  # every rung beaten
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    rec = sub.add_parser("record")
    rec.add_argument("summary", type=Path)
    sub.add_parser("show")
    sub.add_parser("next")
    args = ap.parse_args(argv)

    if args.command == "record":
        return record(args.summary)
    if args.command == "next":
        return next_rung()
    return show()


if __name__ == "__main__":
    sys.exit(main())
