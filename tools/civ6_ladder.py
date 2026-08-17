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
import os
import sys
import time
from contextlib import contextmanager
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


class LedgerBusy(RuntimeError):
    """Another writer held the ledger for longer than this one would wait."""


@contextmanager
def ledger_lock(ledger: Path, *, timeout: float = 30.0,
                stale_after: float = 120.0):
    """Serialise one ledger's read-modify-write across processes.

    ⚠⚠ THE LEDGER IS A WHOLE-FILE REWRITE, AND THIS HOST FINISHES GAMES IN
    PARALLEL. ``record_summary`` and ``sync`` both load the ledger, fold one
    attempt in, and write the entire document back. Two runs finishing within
    the same few milliseconds therefore read the same state and the second
    write erases the first attempt — no error, no partial file, just an
    attempt that was recorded and then was not.

    That is the best explanation on the evidence for the **41 summaries found
    on disk but missing from the live ledger on 2026-08-17**, spanning
    2026-08-14 to 2026-08-16 while neighbouring runs in the same window
    recorded fine. One of the forty-one was ``civvis-20260816T054344Z`` — the
    project's *first* Settler victory, which is exactly the kind of row that
    must not be able to vanish.

    A lock directory would be tidier, but an ``O_CREAT | O_EXCL`` file is the
    one primitive that behaves the same on macOS, Linux and the fleet's
    Windows desktop without importing a platform module. A lock older than
    ``stale_after`` is broken rather than waited on: a killed run must not
    wedge the record for the next one, and a write takes milliseconds, so an
    old lock is always a corpse.
    """
    lock = ledger.parent / (ledger.name + ".lock")
    lock.parent.mkdir(parents=True, exist_ok=True)
    deadline = time.monotonic() + timeout
    held = False
    while True:
        try:
            handle = os.open(str(lock), os.O_CREAT | os.O_EXCL | os.O_WRONLY)
        except FileExistsError:
            try:
                age = time.time() - lock.stat().st_mtime
            except FileNotFoundError:
                continue  # released between the open and the stat; retry now
            if age > stale_after:
                # Whoever wrote this is gone. Breaking it can itself race, but
                # only with another breaker, and they agree on the outcome.
                try:
                    lock.unlink()
                except FileNotFoundError:
                    pass
                continue
            if time.monotonic() >= deadline:
                raise LedgerBusy(
                    f"{ledger} was locked by another writer for {timeout:g}s; "
                    f"the summary is still on disk and `civ6_ladder.py sync` "
                    f"will record it")
            time.sleep(0.02)
            continue
        else:
            os.write(handle, f"{os.getpid()}\n".encode())
            os.close(handle)
            held = True
            break
    try:
        yield
    finally:
        if held:
            try:
                lock.unlink()
            except FileNotFoundError:
                pass


def save(state: dict, ledger: Path) -> None:
    """Replace the ledger atomically.

    ``write_text`` truncates first, so a process killed mid-write leaves a
    half-written ledger that the next ``load`` cannot parse — and the ledger
    is the only copy of the attempt history that is not re-derivable. Writing
    beside it and renaming means a reader sees either the old document or the
    new one, never a torn one.
    """
    ledger.parent.mkdir(parents=True, exist_ok=True)
    scratch = ledger.parent / f".{ledger.name}.{os.getpid()}.tmp"
    try:
        scratch.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")
        os.replace(str(scratch), str(ledger))
    finally:
        try:
            scratch.unlink()
        except FileNotFoundError:
            pass


def is_win(summary: dict) -> bool:
    outcome = summary.get("outcome") or {}
    return bool(outcome.get("kind") == "victory" and outcome.get("won"))


def orders_totals(events_path: Path) -> tuple[int, int] | None:
    """Sum (seen, applied) over the run's agent turn events.

    The bridge's health is how much of what CIVVIS said the engine actually
    did. It was 79.9% once and nobody noticed for days, because the number
    lived in a status tool somebody had to think to run; summing it into the
    summary puts it on the ledger where `check --min-applied` can floor it.
    Tolerant of a truncated tail line — the game can die mid-write.
    """
    if not events_path.is_file():
        return None
    seen = applied = 0
    counted = False
    with events_path.open() as handle:
        for line in handle:
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("kind") != "turn" or event.get("ctx") != "agent":
                continue
            if event.get("orders_seen") is None:
                continue
            counted = True
            seen += int(event.get("orders_seen") or 0)
            applied += int(event.get("orders_applied") or 0)
    return (seen, applied) if counted else None


def final_standing(events_path: Path) -> tuple[int, int] | None:
    """(our score, best rival score) from the last agent turn that saw both.

    The outcome event's `score` is the LOCAL seat's score at the victory
    moment — reading it as the winner's margin made a 469-point gap look
    like a two-point near miss (2026-08-16). The mirror's per-turn
    `rival_best` is the honest comparison, so the ledger carries it.
    """
    if not events_path.is_file():
        return None
    last = None
    with events_path.open() as handle:
        for line in handle:
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if (event.get("kind") == "turn" and event.get("ctx") == "agent"
                    and event.get("rival_best") is not None
                    and event.get("score") is not None):
                last = (int(event["score"]), int(event["rival_best"]))
    return last


def applied_pct(summary: dict) -> float | None:
    """Bridge health as one number: applied orders over issued, in percent."""
    seen = summary.get("orders_seen")
    applied = summary.get("orders_applied")
    if not seen:
        return None
    return round(100.0 * (applied or 0) / seen, 1)


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
        "applied_pct": applied_pct(summary),
        "rival_best": summary.get("rival_best"),
        "lead": (summary["last_score"] - summary["rival_best"]
                 if summary.get("last_score") is not None
                 and summary.get("rival_best") is not None else None),
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
        # The EARLIEST win stands, and "earliest" means by the clock, not by
        # the order attempts happened to reach this function. A later win does
        # not move the timestamp -- the milestone is when the rung was first
        # climbed, not the most recent time it was repeated.
        #
        # ⚠ THIS USED TO READ `if state["wins"].get(difficulty) is None`, WHICH
        # IS INSERTION ORDER WEARING CHRONOLOGY'S NAME. It is the same answer
        # only while every attempt is recorded as it finishes. It is the wrong
        # answer the moment one is recorded late -- and `sync` exists precisely
        # to record attempts late. On 2026-08-17 that was not hypothetical:
        # the ledger held the 23:23:58Z Settler win while the 06:49:58Z win
        # from the same day sat unrecorded on disk, so the backfill that
        # rescued it would have filed the *first* climb of the ladder as an
        # ordinary repeat and left the milestone 16.5 hours late, permanently.
        #
        # `utc` is an ISO-8601 Z stamp, so a string compare is a time compare.
        # A missing stamp sorts last rather than winning by accident.
        recorded = state["wins"].get(difficulty)
        if recorded is None or (entry.get("utc") or "￿") < (
                recorded.get("utc") or "￿"):
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
    # Load INSIDE the lock. Reading first and locking second would reintroduce
    # exactly the lost update the lock exists to prevent.
    with ledger_lock(ledger):
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


def sync(runs_dir: Path, ledger: Path, *, quiet: bool = False) -> int:
    """Record every summary on disk the live ledger is missing.

    This is the self-healing half of the record. Recording is best-effort by
    design -- ``civ6_play.py`` swallows a recording failure so a finished game
    is never lost to a bookkeeping error -- which only works if something
    routinely comes back for what was missed. The climb loop calls this before
    every attempt for exactly that reason.
    """
    paths = summaries_under(runs_dir)
    recorded = skipped = broken = 0
    with ledger_lock(ledger):
        state = load(ledger)
        seen = {a.get("tag") for a in state["attempts"]}
        for path in paths:
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
        held = len(state["attempts"])
    if not quiet or recorded or broken:
        print(f"recorded {recorded} attempt(s), {skipped} already in the ledger"
              + (f", {broken} unreadable" if broken else "")
              + f"; ledger holds {held}")
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
          snapshot: Path | None = None, now: datetime | None = None,
          min_applied: float | None = None) -> int:
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

    if min_applied is not None:
        # The newest attempt that measured itself, not the newest attempt: a
        # run that died before its first turn has no rate and is the
        # staleness check's problem, not this one's.
        measured = [a for a in state["attempts"] if a.get("applied_pct") is not None]
        if measured:
            latest = max(measured, key=lambda a: a.get("utc") or "")
            if latest["applied_pct"] < min_applied:
                problems.append(
                    f"bridge health regressed: {latest['applied_pct']:g}% of "
                    f"orders applied on {latest.get('tag')} (floor "
                    f"{min_applied:g}%) — read the refusal ledger, "
                    f"docs/CIV6_COMPUTER_CONTROL.md")

    for problem in problems:
        print(f"LADDER: {problem}")
    if not problems:
        print(f"ladder current: {len(state['attempts'])} attempt(s) recorded, "
              f"snapshot in step")
    return 1 if problems else 0


def cell(value) -> str:
    """One table cell: an em dash for absent, the value for everything else.

    ⚠⚠ `value or "—"` IS THE BUG THIS REPLACES, AND IT HID THE ONE NUMBER THE
    LADDER EXISTS TO REPORT. Civilization VI's Score Victory is victory type
    `0`, and `0` is falsy — so the rung table rendered the victory type of a
    score win as `?`, and score at the turn limit is the ONLY victory this
    ladder's 250-turn cap can reach. The first claimed rung in the project's
    history would have published with its victory type unknown.

    Absent and zero are different answers and every cell here must be able to
    tell them apart, so no caller gets to write the short version again.
    """
    return "—" if value is None or value == "" else str(value)


def markdown_for(state: dict) -> str:
    wins = state["wins"]
    attempts = state["attempts"]
    lines = [
        "# The Civilization VI difficulty ladder",
        "",
        "What the controller in `tools/civ6_control` has actually beaten, and when.",
        "A rung is claimed only by a victory event naming the controller's own team,",
        "in a game whose settings marker proves it was the game the run configured.",
        "`tools/civ6_ladder.py` writes this file; do not edit it by hand — a",
        "test regenerates it from `docs/civ6_ladder.json` and fails if they differ.",
        "",
        "`victory` is Civilization VI's own victory identifier as the",
        "`TeamVictory` event reported it, kept raw on purpose: nothing in this",
        "repository maps those indices to names, and a guessed name is how an",
        "unfireable type literal hides (see `.github/workflows/tests.yml`).",
        "",
        "| rung | difficulty | beaten (UTC) | victory | turns | run |",
        "|---|---|---|---|---|---|",
    ]
    for index, (key, label) in enumerate(LADDER, start=1):
        win = wins.get(key)
        if win:
            lines.append(f"| {index} | {label} | {cell(win.get('utc'))} | "
                         f"{cell(win.get('victory'))} | {cell(win.get('turns'))} | "
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
            outcome = "win" if a["won"] else cell(a.get("reason"))
            difficulty = NAMES.get(a.get("difficulty"), a.get("difficulty"))
            lines.append(
                f"| `{a.get('tag')}` | {cell(difficulty)} "
                f"| {'yes' if a['configured'] else 'NO'} | {outcome} "
                f"| {cell(a.get('turns'))} | {cell(a.get('score'))} "
                f"| {cell(a.get('utc'))} |")
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
    chk.add_argument("--min-applied", type=float, default=None,
                     help="fail when the newest measured run applied under "
                          "this percentage of its orders")
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
        return check(args.runs, ledger, args.stale_hours,
                     min_applied=args.min_applied)
    if args.command == "next":
        return next_rung(ledger)
    return show(ledger)


if __name__ == "__main__":
    sys.exit(main())
