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

The record has two homes, deliberately:

- The **live ledger** sits beside the runs it records (``<runs>/ladder.json``)
  and is written automatically — ``civ6_play.py`` records every summary the
  moment it writes one. Recording must never require touching the repository:
  the play harness runs from a read-only-main management worktree, and the one
  time recording was a by-hand step it simply stopped happening (211 summaries
  accumulated unrecorded between July 31 and August 16, 2026).
- The **published snapshot** (``docs/civ6_ladder.json`` + ``docs/CIV6_LADDER.md``)
  is the copy the repository shows; ``publish`` refreshes it from the live
  ledger and the change lands like any other PR.

``check`` is the alarm in between: it fails when summaries on disk are missing
from the live ledger, when the published snapshot trails the live ledger, or —
with ``--stale-hours`` — when no run has finished recently, which is how a
halted supervisor becomes a visible failure instead of a silent one.

Usage::

    python tools/civ6_ladder.py record ~/civvis-civ6-runs/control/<tag>/summary.json
    python tools/civ6_ladder.py sync            # record every unrecorded summary
    python tools/civ6_ladder.py publish         # refresh the docs snapshot
    python tools/civ6_ladder.py check --stale-hours 12
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
RUNS_DEFAULT = Path.home() / "civvis-civ6-runs" / "control"

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


def live_ledger_for(runs_dir: Path) -> Path:
    """The live ledger lives beside the runs it records."""
    return runs_dir / "ladder.json"


def load(ledger: Path, snapshot: Path | None = None) -> dict:
    """Load the live ledger, seeding it from the published snapshot.

    A fresh machine (or the first run after this file learned to live with the
    runs) starts from the committed history rather than from nothing, so the
    record stays one continuous timeline across the move.
    """
    snapshot = DATA if snapshot is None else snapshot
    if ledger.is_file():
        return json.loads(ledger.read_text())
    if snapshot.is_file():
        return json.loads(snapshot.read_text())
    return {"attempts": [], "wins": {}}


def save(state: dict, ledger: Path) -> None:
    ledger.parent.mkdir(parents=True, exist_ok=True)
    ledger.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")


def is_win(summary: dict) -> bool:
    outcome = summary.get("outcome") or {}
    return bool(outcome.get("kind") == "victory" and outcome.get("won"))


def entry_from(summary: dict) -> dict:
    return {
        "tag": summary.get("tag"),
        "utc": summary.get("finished_utc") or datetime.now(timezone.utc)
            .strftime("%Y-%m-%dT%H:%M:%SZ"),
        "difficulty": summary.get("difficulty"),
        "configured": bool(summary.get("configured")),
        "won": is_win(summary),
        "victory": (summary.get("outcome") or {}).get("victory"),
        "turns": summary.get("last_turn"),
        "score": summary.get("last_score"),
        "map_size": summary.get("map_size"),
        "speed": summary.get("speed"),
        "reason": summary.get("reason"),
    }


def apply(state: dict, summary: dict) -> bool:
    """Fold one summary into the state. False if its tag is already recorded.

    Idempotence is what lets the automatic path and a by-hand ``record`` (or a
    later ``sync``) coexist without double-counting an attempt.
    """
    tag = summary.get("tag")
    if tag and any(a.get("tag") == tag for a in state["attempts"]):
        return False
    entry = entry_from(summary)
    state["attempts"].append(entry)

    difficulty = entry["difficulty"]
    if entry["won"] and entry["configured"] and difficulty in NAMES:
        # First win stands. A later one does not move the timestamp -- the
        # milestone is when the rung was first climbed, not the most recent
        # time it was repeated.
        if state["wins"].get(difficulty) is None:
            state["wins"][difficulty] = entry
    elif entry["won"] and not entry["configured"]:
        print("won, but the game was not the one this run configured; "
              "not claiming the rung", file=sys.stderr)
    return True


def record_summary(summary_path: Path, ledger: Path | None = None) -> bool:
    """Record one summary into the live ledger. This is the automatic path:
    ``civ6_play.py`` calls it as soon as the summary file exists."""
    summary_path = Path(summary_path)
    if ledger is None:
        # <runs>/<tag>/summary.json -> the ledger beside <runs>.
        ledger = live_ledger_for(summary_path.parent.parent)
    summary = json.loads(summary_path.read_text())
    state = load(ledger)
    changed = apply(state, summary)
    if changed:
        save(state, ledger)
    return changed


def summaries_under(runs_dir: Path) -> list[Path]:
    """Every run summary, oldest first, so a backfill replays history in order."""
    def stamp(path: Path) -> str:
        try:
            finished = json.loads(path.read_text()).get("finished_utc")
        except (OSError, json.JSONDecodeError):
            finished = None
        # Fall back to mtime for summaries from before finished_utc existed;
        # the ISO stamp and a float sort cannot be compared, so stringify.
        return finished or datetime.fromtimestamp(
            path.stat().st_mtime, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    found = list(runs_dir.glob("*/summary.json"))
    return sorted(found, key=stamp)


def sync(runs_dir: Path, ledger: Path) -> int:
    state = load(ledger)
    seen = {a.get("tag") for a in state["attempts"]}
    recorded = skipped = broken = 0
    for path in summaries_under(runs_dir):
        try:
            summary = json.loads(path.read_text())
        except (OSError, json.JSONDecodeError) as exc:
            print(f"unreadable summary {path}: {exc}", file=sys.stderr)
            broken += 1
            continue
        if summary.get("tag") in seen:
            skipped += 1
            continue
        if apply(state, summary):
            seen.add(summary.get("tag"))
            recorded += 1
        else:
            skipped += 1
    if recorded:
        save(state, ledger)
    print(f"recorded {recorded} attempt(s), {skipped} already in the ledger"
          + (f", {broken} unreadable" if broken else "")
          + f"; ledger holds {len(state['attempts'])}")
    return 0


def publish(ledger: Path, snapshot: Path | None = None,
            markdown: Path | None = None) -> int:
    """Refresh the repository's snapshot of the live ledger."""
    snapshot = DATA if snapshot is None else snapshot
    markdown = LEDGER if markdown is None else markdown
    state = load(ledger)
    snapshot.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")
    markdown.write_text(markdown_for(state))
    print(f"published {len(state['attempts'])} attempt(s) to {snapshot} and {markdown}")
    return 0


def check(runs_dir: Path, ledger: Path, stale_hours: float | None,
          snapshot: Path | None = None, now: datetime | None = None) -> int:
    """Exit nonzero when the record is behind the truth on disk.

    Three separate failures, reported together, because they have three
    different remedies: unrecorded summaries want ``sync``, a trailing
    snapshot wants ``publish`` (landed as a PR), and a stale newest-summary
    means the supervisor itself has stopped playing.
    """
    snapshot = DATA if snapshot is None else snapshot
    problems = []
    state = load(ledger)
    seen = {a.get("tag") for a in state["attempts"]}
    paths = summaries_under(runs_dir)

    unrecorded = []
    for path in paths:
        try:
            tag = json.loads(path.read_text()).get("tag")
        except (OSError, json.JSONDecodeError):
            continue
        if tag not in seen:
            unrecorded.append(tag)
    if unrecorded:
        problems.append(f"{len(unrecorded)} summary(ies) on disk are not in the "
                        f"live ledger (run `civ6_ladder.py sync`)")

    if snapshot.is_file():
        published = json.loads(snapshot.read_text())
        behind = len(state["attempts"]) - len(published.get("attempts", []))
        if behind > 0:
            problems.append(f"published snapshot trails the live ledger by "
                            f"{behind} attempt(s) (run `civ6_ladder.py publish` "
                            f"and land it)")

    if stale_hours is not None:
        newest = None
        for a in state["attempts"]:
            if a.get("utc") and (newest is None or a["utc"] > newest):
                newest = a["utc"]
        now = now or datetime.now(timezone.utc)
        if newest is None:
            problems.append("the ledger holds no attempts at all")
        else:
            age = (now - datetime.strptime(newest, "%Y-%m-%dT%H:%M:%SZ")
                   .replace(tzinfo=timezone.utc)).total_seconds() / 3600
            if age > stale_hours:
                problems.append(f"newest recorded attempt is {age:.1f}h old "
                                f"(limit {stale_hours:g}h) — is the supervisor "
                                f"running?")

    for problem in problems:
        print(f"LADDER: {problem}")
    if not problems:
        print(f"ladder current: {len(state['attempts'])} attempt(s) recorded, "
              f"snapshot in step")
    return 1 if problems else 0


def markdown_for(state: dict) -> str:
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
    return "\n".join(lines)


def show(ledger: Path) -> int:
    print(markdown_for(load(ledger)))
    return 0


def next_rung(ledger: Path) -> int:
    """Print the lowest difficulty not yet beaten, for a driver to pick up."""
    state = load(ledger)
    for key, _ in LADDER:
        if key not in state["wins"]:
            print(key)
            return 0
    print("")  # every rung beaten
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", type=Path, default=RUNS_DEFAULT,
                    help="runs directory the live ledger sits beside")
    ap.add_argument("--ledger", type=Path, default=None,
                    help="live ledger path (default: <runs>/ladder.json)")
    sub = ap.add_subparsers(dest="command", required=True)
    rec = sub.add_parser("record")
    rec.add_argument("summary", type=Path)
    sub.add_parser("sync")
    sub.add_parser("publish")
    chk = sub.add_parser("check")
    chk.add_argument("--stale-hours", type=float, default=None)
    sub.add_parser("show")
    sub.add_parser("next")
    args = ap.parse_args(argv)
    ledger = args.ledger or live_ledger_for(args.runs)

    if args.command == "record":
        changed = record_summary(args.summary, ledger)
        if not changed:
            print("already recorded")
        return 0
    if args.command == "sync":
        return sync(args.runs, ledger)
    if args.command == "publish":
        return publish(ledger)
    if args.command == "check":
        return check(args.runs, ledger, args.stale_hours)
    if args.command == "next":
        return next_rung(ledger)
    return show(ledger)


if __name__ == "__main__":
    sys.exit(main())
