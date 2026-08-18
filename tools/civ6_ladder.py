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
    python tools/civ6_ladder.py render          # redraw the docs markdown only
    python tools/civ6_ladder.py check --stale-hours 12
    python tools/civ6_ladder.py show

``publish`` and ``render`` are not interchangeable. ``publish`` lands run data
from this machine's live ledger; ``render`` only redraws the committed snapshot
and is what a change to the table's shape wants.
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

# ⚠ THE MOD'S OWN ANSWER FOR A SETTING IT COULD NOT RESOLVE, on every axis it
# surveys: `CivvisControlAgent.lua` writes `try(..., "?") or "?"` for the
# difficulty, speed, map, size and ruleset alike. It is the ABSENCE of an
# answer, never an answer that disagrees, and the two must not be folded
# together — `civ6_play.seat_matches_requested` imports this name so the record
# and the harness cannot drift on what the sentinel is.
UNREADABLE = "?"

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


def is_defeat(summary: dict) -> bool:
    """Whether OUR civilization was eliminated — the ending, not the ceiling.

    ★★★★★ A LEDGER THAT CANNOT TELL DEFEAT FROM A WEDGE CANNOT BE USED TO
    COMPARE ANYTHING. `civ6_play.py` keeps the terminal event for a defeat
    exactly as it does for a victory, and the mod synthesises one when no
    cities remain, but nothing here ever read it: only `won`, `victory` and
    `victory_type` were projected, and a defeat carries none of the three. Our
    own elimination therefore landed as `{"reason": "stopped", "victory": null,
    "won": false}` — byte-identical to a run that hung. Measured on three live
    rows: `civvis-20260815T160346Z` (eliminated at t233, score 272),
    `195951Z` (t102, score 153) and `210845Z` (t226, score 313). All three
    carry `reason: "stopped"` — the same answer 259 of the ledger's 325 rows
    give, including all 30 of its stalls.

    ⚠ `ours` IS LOAD-BEARING. Civilization VI emits a `defeat` every time ANY
    player is eliminated, including a city-state, and 39 of the 111 runs whose
    event streams are still on this machine carry at least one rival's.
    `civ6_play.finished()` already refuses to stop on those — a run was once
    cut sixteen turns short of a score victory because player 7 died — and
    reading the flag off the event here is the same distinction on the
    recording side.
    """
    outcome = summary.get("outcome") or {}
    return bool(outcome.get("kind") == "defeat" and outcome.get("ours"))


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


def decider_revisions(updates_path: Path) -> list[str] | None:
    """Ordered, consecutive-deduplicated revisions that decided this run.

    The brain writes a `start` row when it opens and a `handoff` row each time
    it re-execs onto a newer origin/main mid-game, so the file is the run's
    whole code history. None when the file is absent (an older brain, or a
    stock-mode run): absence must stay distinct from an empty history.
    """
    if not updates_path.is_file():
        return None
    revisions: list[str] = []
    with updates_path.open() as handle:
        for line in handle:
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("kind") != "runtime_update":
                continue
            if event.get("status") not in ("start", "handoff"):
                continue
            revision = event.get("to_revision")
            if revision and (not revisions or revisions[-1] != revision):
                revisions.append(revision)
    return revisions or None


def runtime_heartbeat_problem(heartbeat: Path, max_minutes: float,
                              now: datetime | None = None) -> str | None:
    """Why the origin/main watcher is not provably alive, or None.

    The watcher writes its heartbeat every refresh cycle, success or failure.
    A machine that has never run the live loop has no cache directory and is
    nobody's problem; a cache directory with a missing, stale, or erroring
    heartbeat means the verification game may be silently playing old code —
    the exact silence this check exists to make loud.
    """
    if not heartbeat.parent.is_dir():
        return None
    if not heartbeat.is_file():
        return (f"the live-runtime watcher has no heartbeat at {heartbeat} "
                f"(cache exists; the brain's updater is not running)")
    try:
        beat = json.loads(heartbeat.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        return f"unreadable runtime heartbeat {heartbeat}: {exc}"
    now = now or datetime.now(timezone.utc)
    stamp = beat.get("utc")
    try:
        age = (now - datetime.strptime(stamp, "%Y-%m-%dT%H:%M:%SZ")
               .replace(tzinfo=timezone.utc)).total_seconds() / 60
    except (TypeError, ValueError):
        return f"runtime heartbeat carries no readable utc: {stamp!r}"
    if age > max_minutes:
        return (f"runtime heartbeat is {age:.0f} min old (limit "
                f"{max_minutes:g}) — the game may be playing old code")
    error = (beat.get("last_error") or "").strip()
    if error:
        return f"runtime refresh is failing: {error[:200]}"
    return None


RUNTIME_HEARTBEAT_DEFAULT = (Path.home() / ".cache" / "civvis"
                             / "live-game-runtime" / "heartbeat.json")


def applied_pct(summary: dict) -> float | None:
    """Bridge health as one number: applied orders over issued, in percent."""
    seen = summary.get("orders_seen")
    applied = summary.get("orders_applied")
    if not seen:
        return None
    return round(100.0 * (applied or 0) / seen, 1)


def with_bridge_health(summary: dict, summary_path: Path) -> dict:
    """Fill a summary's order totals from the run's own events, if it lacks them.

    ⚠⚠ THE BACKFILL RECORDED FORTY-ONE ATTEMPTS WITH NO BRIDGE HEALTH WHILE THE
    EVIDENCE SAT BESIDE THEM ON DISK. `civ6_play.py` sums `events.jsonl` into
    the summary at the end of a run, and `sync` — the self-healing path that
    exists precisely because that write is best-effort and may not happen —
    recorded whatever the summary happened to contain. So the one path built
    for runs whose bookkeeping failed was the path that could not repair the
    number, and it lost it permanently: rows 266 to 306 of the live ledger, the
    same 41 summaries `civ6_civvis_climb.heal_the_ladder` was written to
    rescue, carry no `applied_pct` at all. **Both Settler wins are among them**,
    so the bridge health of the project's only external results is unknown.

    A run's `events.jsonl` is the evidence and it outlives the summary write.
    Reading it here means a backfilled row is as complete as a live one, and
    the derivation stays in one place rather than two that can disagree.

    Non-destructive: a summary that already carries totals is returned
    unchanged, and an unreadable or absent events file leaves it as it was.
    """
    if summary.get("orders_seen"):
        return summary
    totals = orders_totals(Path(summary_path).parent / "events.jsonl")
    if not totals:
        return summary
    enriched = dict(summary)
    enriched["orders_seen"], enriched["orders_applied"] = totals
    return enriched


def trailing_unmeasured(attempts: list) -> int:
    """How many of the newest attempts carry no bridge-health rate.

    Counted from the end in recorded order rather than filtered, because the
    question is "has the instrument gone dark", not "how many rows lack it" —
    the ledger legitimately holds hundreds of older rows from before the rate
    was recorded at all, and a total would report those forever.
    """
    dark = 0
    for attempt in reversed(attempts):
        if attempt.get("applied_pct") is not None:
            break
        dark += 1
    return dark


def victory_type(summary: dict) -> str | None:
    """The name of the victory this run ended on, from the HOST'S OWN table.

    ⚠⚠ THE LADDER RECORDED AN UNTRANSLATED INTEGER AND SAID SO ON PURPOSE.
    `docs/CIV6_LADDER.md` refuses guessed names for `TeamVictory`'s type index —
    rightly, because a guessed literal is how an unfireable type name hides. But
    the refusal was written when the only alternative to a guess was silence,
    and it has not been true since the agent mod began exporting
    `GameInfo.Victories()` as `seat.victory_types`: the run carries the index
    and the table the index comes from, in the same record.

    This is that join, and it is not a guess in either direction. A run whose
    seat event predates the export, or whose game never emitted a terminal
    event, still answers `None` and still renders as the raw index alone.

    It matters now because the milestones are stated per victory type — a
    Science win and a Culture win are different claims, and a ladder that
    reports `5` can substantiate neither.
    """
    outcome = summary.get("outcome") or {}
    index = outcome.get("victory")
    if index is None:
        return None
    rows = ((summary.get("seat") or {}).get("victory_types")) or []
    for row in rows:
        if isinstance(row, dict) and row.get("index") == index:
            name = row.get("type")
            return str(name) if name else None
    return None


def entry_from(summary: dict) -> dict:
    return {
        "tag": summary.get("tag"),
        "utc": summary.get("finished_utc") or datetime.now(timezone.utc)
            .strftime("%Y-%m-%dT%H:%M:%SZ"),
        "difficulty": summary.get("difficulty"),
        "configured": bool(summary.get("configured")),
        "won": is_win(summary),
        # ⚠⚠ AND WHETHER WE LOST, which is a different question from not
        # winning. `won` keeps its meaning exactly — a victory event naming our
        # team — and this column carries the other terminal ending, so a stall
        # and an elimination stop being the same row. See `is_defeat`.
        "defeat": is_defeat(summary),
        "victory": (summary.get("outcome") or {}).get("victory"),
        # Kept BESIDE the index, never instead of it. The index is what the
        # event reported and stays the primary key; the name is the host's
        # gloss on it, and rows recorded before the export have none.
        "victory_type": victory_type(summary),
        # And what the run ASKED the agent to play for, which is a different
        # question from what it ended on. Rows recorded before #1871 have none;
        # rows recorded after it can differ from each other, and a ledger that
        # cannot separate the lanes cannot say which one wins.
        "victory_target": summary.get("victory_target"),
        # ⚠⚠ WHICH GAME THIS ROW WAS PLAYED IN, which the record could not say.
        #
        # `civ6_play.py` verifies both of these from inside the running game and
        # refuses a run that does not match — a wrong-modes or wrong-ruleset run
        # is recorded as a refusal rather than a result. None of that reached
        # here: 307 rows carried neither field, so the published record asserted
        # that every row was the same game and held no evidence of it.
        #
        # It matters because a mode PERSISTS: `GameConfiguration.SetToDefaults()`
        # does not clear one, and GAMEMODE_HEROES was found true on a live run
        # that had been playing with twelve hero units while every log said plain
        # Gathering Storm. The ruleset is the same axis one level up — CIVVIS
        # models Gathering Storm and nothing else, and the compiled gameplay
        # cache on the development machine was found to be a Vanilla database.
        #
        # Recorded as what the GAME reported, not what the command line asked
        # for; the two disagreeing is the whole thing worth catching.
        "ruleset": summary.get("ruleset"),
        "modes": summary.get("modes"),
        "turns": summary.get("last_turn"),
        "score": summary.get("last_score"),
        "map_size": summary.get("map_size"),
        "speed": summary.get("speed"),
        "reason": summary.get("reason"),
        "applied_pct": applied_pct(summary),
        "revisions": summary.get("decider_revisions"),
        # Which arm played this row: the live treatments withheld from the
        # decider, or [] for the full shipped bundle. Rows recorded before the
        # launchers could withhold anything carry None — unknown, which is not
        # the same claim as "nothing was withheld".
        "withheld": summary.get("withheld"),
        "mod_arms": summary.get("mod_arms"),
        # The opening tempo (`civ6_play.OPENING_TEMPO_TURN`). Over the 35
        # completed runs of 2026-08-16/17 these were the strongest correlates
        # the live ladder has produced: cities at t60 r=+0.69 with final lead,
        # second-city founding turn r=-0.49, with total city count EQUAL
        # between the groups. Carried per row so a tempo regression is visible
        # without reconstructing it from events.jsonl.
        "city_two_turn": summary.get("city_two_turn"),
        "cities_at_60": summary.get("cities_at_60"),
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
        # ⚠⚠ AND SEPARATELY BY VICTORY TYPE, BECAUSE `wins` HAS ONE SLOT PER
        # DIFFICULTY AND THAT SLOT IS ALREADY FULL.
        #
        # A rung is claimed by the first win at a difficulty, which is the right
        # rule for a rung and the wrong one for anything else: the Settler slot
        # holds a Score win from 2026-08-16, so a Settler *Science* win arriving
        # tomorrow is strictly later, loses the comparison above, and is kept
        # only as an ordinary attempt row. The record would show one Settler
        # milestone forever no matter how many distinct victories were won at
        # it.
        #
        # That is not hypothetical bookkeeping. The current objective list is
        # five separate victory types at ONE difficulty, and until this line
        # existed the ladder could represent exactly one of them.
        #
        # The per-type record is NOT materialised beside this one. It is derived
        # from `attempts` in `victory_board`, applying the same earliest-wins
        # rule to a different key — so it needs no migration, it is correct for
        # the 307 attempts already committed the moment this ships, and it
        # cannot drift from the rows it summarises.
    elif entry["won"] and not entry["configured"]:
        print("won, but the game was not the one this run configured; "
              "not claiming the rung", file=sys.stderr)
    # The host's own victory table, kept once so the board below can list every
    # victory Civilization VI offers rather than only the ones already beaten.
    # First writer wins; it is a property of the game, not of the run.
    table = (summary.get("seat") or {}).get("victory_types")
    if table and not state.get("victory_types"):
        state["victory_types"] = table
    return True


def record_summary(summary_path: Path, ledger: Path | None = None) -> bool:
    """Record one summary into the live ledger. This is the automatic path:
    ``civ6_play.py`` calls it as soon as the summary file exists."""
    summary_path = Path(summary_path)
    if ledger is None:
        # <runs>/<tag>/summary.json -> the ledger beside <runs>.
        ledger = live_ledger_for(summary_path.parent.parent)
    summary = with_bridge_health(json.loads(summary_path.read_text()),
                                 summary_path)
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
            summary = with_bridge_health(summary, path)
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


def render(snapshot: Path | None = None, markdown: Path | None = None) -> int:
    """Rewrite the markdown from the COMMITTED snapshot, importing nothing.

    ⚠⚠ `publish` IS NOT THE TOOL FOR A RENDERING CHANGE, and the difference is
    not obvious from either name. `publish` reads this machine's live ledger,
    which `load` seeds from the snapshot and then tops up with every local run —
    so a contributor who changes how a row is drawn and reaches for `publish` to
    regenerate the file also lands however many attempts their own machine
    happens to be holding. Running it here on a change that touched only
    `markdown_for` would have added eighteen rows from this laptop's own
    fortnight-old experiments to the shared record, dated before the newest row
    already published, with nothing in the diff to say why the count moved.

    Landing run data is `publish`'s job and a deliberate act. Redrawing the
    table is this one, and it cannot import anything: there is no ledger in the
    signature to import from.
    """
    snapshot = DATA if snapshot is None else snapshot
    markdown = LEDGER if markdown is None else markdown
    state = json.loads(snapshot.read_text())
    markdown.write_text(markdown_for(state))
    print(f"rendered {len(state.get('attempts', []))} attempt(s) from {snapshot} "
          f"to {markdown}")
    return 0


def newest_attempt_age_hours(state: dict, now: datetime | None = None
                             ) -> float | None:
    """Hours since the newest recorded attempt, or `None` when there are none.

    The one definition of "when did this project last play a game". `check`
    reports it to a human and `ops/ladder_watchdog.py` acts on it, and those two
    must never be able to disagree about what stale means — a watchdog that
    restarts the supervisor on a different clock than the check that reports it
    is a watchdog nobody can reason about.
    """
    newest = None
    for a in state.get("attempts") or []:
        if a.get("utc") and (newest is None or a["utc"] > newest):
            newest = a["utc"]
    if newest is None:
        return None
    now = now or datetime.now(timezone.utc)
    stamp = datetime.strptime(newest, "%Y-%m-%dT%H:%M:%SZ").replace(
        tzinfo=timezone.utc)
    return (now - stamp).total_seconds() / 3600


def staleness_problem(state: dict, stale_hours: float,
                      now: datetime | None = None) -> str | None:
    """The staleness sentence, or `None` when the loop is producing attempts."""
    age = newest_attempt_age_hours(state, now=now)
    if age is None:
        return "the ledger holds no attempts at all"
    if age > stale_hours:
        return (f"newest recorded attempt is {age:.1f}h old "
                f"(limit {stale_hours:g}h) — is the supervisor running?")
    return None


def opening_tempo_problem(attempts: list[dict], floor: float,
                          window: int = 10) -> str | None:
    """The tempo alarm: has the opening gone slow across the recent window?

    Reads the MEDIAN second-city turn over the newest `window` rows that carry
    one, because a single late founding is ordinary map variance — six of the
    35 runs measured sat past t35 and three of those still finished within 250
    points. A median that slips is the empire, not the map.

    Returns None when too few rows carry the column to say anything, which is
    the honest answer for a ledger recorded before the column existed.
    """
    turns = [a["city_two_turn"] for a in attempts
             if isinstance(a.get("city_two_turn"), int)]
    if len(turns) < window:
        return None
    recent = sorted(turns[-window:])
    middle = len(recent) // 2
    median = (recent[middle] if len(recent) % 2
              else (recent[middle - 1] + recent[middle]) / 2)
    if median <= floor:
        return None
    return (f"opening tempo regressed: median second city at turn {median:g} "
            f"over the last {window} recorded runs (floor {floor:g}) — the "
            f"ladder's strongest correlate with final lead")


def check(runs_dir: Path, ledger: Path, stale_hours: float | None,
          snapshot: Path | None = None, now: datetime | None = None,
          min_applied: float | None = None,
          heartbeat_minutes: float | None = None,
          heartbeat: Path | None = None,
          max_city_two_turn: float | None = None,
          unmeasured_limit: int = 5) -> int:
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
        now = now or datetime.now(timezone.utc)
        stale = staleness_problem(state, stale_hours, now=now)
        if stale:
            problems.append(stale)

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
        # ⚠⚠ A GAP IN MEASUREMENT IS NOT A GAP IN ATTEMPTS, AND NOTHING WAS
        # WATCHING FOR IT. The reasoning above is right for ONE unmeasured run
        # and wrong for a run of them: this floor only ever reads attempts that
        # carry a rate, so an instrument that goes dark makes it silently
        # unfalsifiable, while `--stale-hours` stays quiet because attempts are
        # still arriving. Measured on the live ledger 2026-08-18: attempts 266
        # to 306 — forty-one consecutive games, every one of them played to the
        # 250-turn clock, INCLUDING BOTH SETTLER WINS — recorded no rate at all,
        # and `check` was green throughout. The bridge health of the project's
        # only two external results is therefore unknown.
        dark = trailing_unmeasured(state["attempts"])
        if dark >= unmeasured_limit:
            problems.append(
                f"bridge health is unmeasured on the last {dark} attempt(s) "
                f"(limit {unmeasured_limit}): no `applied_pct` reached the "
                f"ledger, so the {min_applied:g}% floor cannot fire on them. "
                f"An unmeasured bridge is not a healthy one — check that the "
                f"run summaries carry `orders_seen`")

    if max_city_two_turn is not None:
        problem = opening_tempo_problem(state["attempts"], max_city_two_turn)
        if problem:
            problems.append(problem)

    if heartbeat_minutes is not None:
        problem = runtime_heartbeat_problem(
            heartbeat or RUNTIME_HEARTBEAT_DEFAULT, heartbeat_minutes, now=now)
        if problem:
            problems.append(problem)

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


def victory_board(state: dict) -> list[tuple[int, str | None, dict]]:
    """Every victory condition, and the date each was first beaten per rung.

    ★★★★★ THE OBJECTIVE LIST IS FIVE VICTORY TYPES AT ONE DIFFICULTY AND THE
    RECORD HAD ONE SLOT FOR THEM. `wins` is keyed by difficulty alone and holds
    the earliest win there, which is exactly right for claiming a rung and
    cannot represent "Settler, but by Science this time". This is that record.

    Rows come from the host's `GameInfo.Victories()` when a run has exported it,
    so a condition this install offers and nobody has beaten is still a visible
    empty row rather than a silent absence — the difference between a checklist
    and a list of things that happened. Before any run exports the table, the
    rows are the indices that have actually been won, which is the same
    conservative fallback the rest of this file takes.
    """
    beaten: dict[int, dict[str, str]] = {}
    names: dict[int, str] = {}
    for entry in state.get("attempts") or []:
        if not isinstance(entry, dict):
            continue
        index = entry.get("victory")
        difficulty = entry.get("difficulty")
        # The same three conditions the rung claim demands: we won it, the game
        # was the one the run configured, and the difficulty is a rung. A win in
        # a game that drifted off its settings claims nothing here either.
        if index is None or not entry.get("won") or not entry.get("configured"):
            continue
        if difficulty not in NAMES:
            continue
        name = entry.get("victory_type")
        if name and index not in names:
            names[index] = str(name)
        stamp = entry.get("utc")
        held = beaten.setdefault(index, {}).get(difficulty)
        # Earliest wins, by the clock — the same rule and the same reason as the
        # rung claim above: the milestone is when it was first done.
        if held is None or (stamp or "￿") < held:
            beaten[index][difficulty] = stamp
    if not beaten and not state.get("victory_types"):
        return []
    rows: list[tuple[int, str | None, dict]] = []
    seen: set[int] = set()
    for row in state.get("victory_types") or []:
        if not isinstance(row, dict):
            continue
        index = row.get("index")
        if not isinstance(index, int) or index in seen:
            continue
        seen.add(index)
        name = row.get("type") or names.get(index)
        rows.append((index, str(name) if name else None, beaten.get(index, {})))
    # Anything won whose index the exported table does not carry still belongs
    # on the board: the win is the evidence, the table is only the gloss.
    for index in sorted(set(beaten) - seen):
        rows.append((index, names.get(index), beaten[index]))
    return rows


def same_game_note(attempts: list) -> list[str]:
    """One line saying whether every recorded row was the same game.

    ⚠ A LADDER IS A COMPARISON, AND A COMPARISON NEEDS THE ROWS TO BE THE SAME
    GAME. `civ6_play.py` checks the ruleset and the optional modes from inside
    the running game and refuses a run that does not match, but none of that
    reached this record: rows carried neither field, so "these are all Settler
    games" was an assertion with no evidence under it.

    Rows recorded before those fields existed answer `None`, which is reported as
    unverified rather than folded in with the ones that agree — an old row is not
    a row that was checked.

    ⚠ AND `?` BELONGS IN THAT SAME BUCKET, not in the list of rulesets recorded.
    `?` is the mod's answer for a readback that failed (`UNREADABLE`), so a row
    carrying it was never checked either — but because it is a truthy string it
    used to be printed as a ruleset this record had observed, and the published
    prose read "Rulesets recorded: ?, RULESET_EXPANSION_2" over three rows whose
    ruleset nobody knows.
    """
    if not attempts:
        return []
    rulesets = {a.get("ruleset") for a in attempts if isinstance(a, dict)}
    unverified = sum(
        1 for a in attempts if isinstance(a, dict)
        and a.get("ruleset") in (None, UNREADABLE))
    named = sorted(r for r in rulesets if r and r != UNREADABLE)
    moded = sorted({
        mode
        for a in attempts if isinstance(a, dict)
        for mode in (a.get("modes") or [])
    })
    out = ["Every row above is one game's settings as the game itself reported "
           "them, not as the command line asked for them."]
    if named:
        out.append(f"Rulesets recorded: {', '.join(named)}.")
    if unverified:
        out.append(
            f"{unverified} row(s) carry no ruleset readback — the run predates "
            f"it, or the game could not report one — and are unverified rather "
            f"than agreed. Unverified is not a mismatch: those games were "
            f"played and their endings stand.")
    # ⚠ AND THE ROWS WHERE THAT WENT WRONG SAY SO, because a row is never
    # rewritten. Three games were filed as `wrong_ruleset` on an UNREADABLE
    # readback rather than a differing one — `civvis-20260818T032030Z` at 223
    # turns and score 937, `040903Z` at 250/1138, `045332Z` at 250/683 — and
    # without this line the sentence above contradicts the table below, where
    # all three still read `wrong_ruleset` and `configured: NO`.
    misfiled = sum(
        1 for a in attempts if isinstance(a, dict)
        and a.get("reason") == "wrong_ruleset"
        and a.get("ruleset") in (None, UNREADABLE))
    if misfiled:
        out.append(
            f"⚠ {misfiled} of those row(s) were nevertheless recorded as "
            f"`wrong_ruleset` and non-comparable, back when an unreadable "
            f"readback and a differing one were the same answer. They were "
            f"played to the end; rows are never rewritten, so the misfiling "
            f"stands in the record and this line is how it is known.")
    if moded:
        out.append(
            f"⚠ Optional game modes were on in some rows: {', '.join(moded)}. "
            f"A mode adds units and rules CIVVIS does not model, so those rows "
            f"are not measuring the same game as the rest.")
    return ["", " ".join(out), ""]


def victory_census(attempts: list) -> list[tuple[int, str | None, int]]:
    """Which victory conditions have actually ended a game here, most first.

    ★★★★★ THE LADDER HELD THIS ANSWER FOR 307 ATTEMPTS AND NEVER REPORTED IT.
    Each row already carried the terminal event's victory index; nothing ever
    grouped by it, so the one empirical fact the record can offer about lane
    reachability — which conditions complete inside 250 turns at this
    difficulty, as demonstrated by the rivals who complete them — was sitting in
    a column no reader could see. Every objective this controller has been
    pointed at was chosen from argument, not from this table.

    A name is attached only where some run in the record exported the host's own
    `GameInfo.Victories()` for that index. Indices never seen with a name stay
    unnamed rather than being guessed into one.
    """
    counts: dict[int, int] = {}
    names: dict[int, str] = {}
    for attempt in attempts:
        if not isinstance(attempt, dict):
            continue
        index = attempt.get("victory")
        if index is None:
            continue
        counts[index] = counts.get(index, 0) + 1
        name = attempt.get("victory_type")
        if name and index not in names:
            names[index] = str(name)
    # Commonest first; the index breaks ties so the table is deterministic.
    return sorted(((index, names.get(index), count)
                   for index, count in counts.items()),
                  key=lambda row: (-row[2], row[0]))


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
        "`TeamVictory` event reported it, kept raw on purpose: a guessed name is",
        "how an unfireable type literal hides (see `.github/workflows/tests.yml`).",
        "`type` beside it is not a guess either — it is the row the index names in",
        "the host's own `GameInfo.Victories()`, exported by the agent mod as",
        "`seat.victory_types` and joined by `tools/civ6_ladder.py`. A run recorded",
        "before that export carries the index alone and reads `—` here.",
        "",
        "| rung | difficulty | beaten (UTC) | victory | type | turns | run |",
        "|---|---|---|---|---|---|---|",
    ]
    for index, (key, label) in enumerate(LADDER, start=1):
        win = wins.get(key)
        if win:
            lines.append(f"| {index} | {label} | {cell(win.get('utc'))} | "
                         f"{cell(win.get('victory'))} | {cell(win.get('victory_type'))} | "
                         f"{cell(win.get('turns'))} | `{win.get('tag')}` |")
        else:
            lines.append(f"| {index} | {label} | — | | | | |")
    lines += ["", f"Attempts recorded: {len(attempts)}.", ""]
    lines += same_game_note(attempts)

    board = victory_board(state)
    if board:
        lines += [
            "## Which victories have been won, per difficulty",
            "",
            "A rung is claimed by the FIRST win at a difficulty; this table is the",
            "other question — which of Civilization VI's victory conditions the",
            "controller has beaten, and where. The two differ as soon as a second",
            "victory type is won at a rung already claimed, which the rung table",
            "records as an ordinary repeat.",
            "",
            "Rows are the host's own `GameInfo.Victories()`, so a condition this",
            "install offers and nobody has won still appears, empty.",
            "",
            "| victory | type | " + " | ".join(label for _, label in LADDER) + " |",
            "|---|---|" + "---|" * len(LADDER),
        ]
        for index, name, beaten in board:
            row = " | ".join(cell(beaten.get(key)) for key, _ in LADDER)
            lines.append(f"| {cell(index)} | {cell(name)} | {row} |")
        lines.append("")

    endings = victory_census(attempts)
    if endings:
        lines += [
            "## How these games ended",
            "",
            "Every terminal `TeamVictory` in the record, ours and the rivals'.",
            "A rival completing a victory condition is the strongest evidence",
            "available that the condition is reachable inside this profile's turn",
            "budget — it is a rival, at Settler, on the same map and clock. Lanes",
            "absent from this table have never been completed by anyone here.",
            "",
            "| victory | type | games | of ended |",
            "|---|---|---|---|",
        ]
        total = sum(count for _, _, count in endings)
        for index, name, count in endings:
            share = f"{100.0 * count / total:.0f}%" if total else "—"
            lines.append(f"| {cell(index)} | {cell(name)} | {count} | {share} |")
        lost = sum(1 for a in attempts
                   if isinstance(a, dict) and a.get("defeat"))
        # A defeat IS a terminal ending; it is simply not a `TeamVictory`, so it
        # is absent from the table above and used to be swept into "the rest"
        # alongside the stalls it is nothing like.
        lines += ["",
                  f"{total} of {len(attempts)} attempts reached a terminal "
                  "victory event"
                  + (f", and {lost} more ended in our own elimination"
                     if lost else "")
                  + "; the rest stalled, exited, or were stopped before one.",
                  ""]

    if attempts:
        lines += [
            "## Every attempt",
            "",
            "`outcome` is what the game did, not what the harness saw last.",
            "`defeat` means this controller was eliminated and the game said so;",
            "`stopped`, `stalled` and `timeout` mean nobody won and nobody lost.",
            "A ledger that cannot tell defeat from a wedge cannot be used to",
            "compare anything, and until `defeat` existed here the two were the",
            "same row.",
            "",
            "| run | difficulty | playing for | configured | outcome | turns | score | ended |",
            "|---|---|---|---|---|---|---|---|",
        ]
        for a in attempts[-40:]:
            outcome = ("win" if a["won"]
                       else "defeat" if a.get("defeat")
                       else cell(a.get("reason")))
            difficulty = NAMES.get(a.get("difficulty"), a.get("difficulty"))
            lines.append(
                f"| `{a.get('tag')}` | {cell(difficulty)} "
                f"| {cell(a.get('victory_target'))} "
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
    sub.add_parser(
        "render",
        help="redraw the markdown from the committed snapshot; imports "
             "no runs, unlike `publish`")
    chk = sub.add_parser("check")
    chk.add_argument("--stale-hours", type=float, default=None)
    chk.add_argument("--min-applied", type=float, default=None,
                     help="fail when the newest measured run applied under "
                          "this percentage of its orders")
    wat = sub.add_parser(
        "watch",
        help="only the origin/main watcher probe, for loops that poll it")
    wat.add_argument("--minutes", type=float, default=10.0)
    chk.add_argument("--max-unmeasured", type=int, default=5,
                     help="consecutive newest attempts allowed to record no "
                          "bridge-health rate before that is itself a problem "
                          "(default 5); only read when --min-applied is set, "
                          "because it is that floor this keeps falsifiable")
    chk.add_argument("--max-city-two-turn", type=float, default=None,
                     help="fail when the MEDIAN second-city founding turn over "
                          "the last ten recorded runs exceeds this — the "
                          "ladder's strongest measured correlate with final "
                          "lead (r=-0.49; by t30 median lead -59, after t30 "
                          "-717)")
    chk.add_argument("--heartbeat-minutes", type=float, default=None,
                     help="fail when the origin/main watcher's heartbeat is "
                          "older than this, unreadable, or reporting an "
                          "error (skipped on machines with no runtime cache)")
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
    if args.command == "render":
        return render()
    if args.command == "check":
        return check(args.runs, ledger, args.stale_hours,
                     min_applied=args.min_applied,
                     unmeasured_limit=args.max_unmeasured,
                     heartbeat_minutes=args.heartbeat_minutes,
                     max_city_two_turn=args.max_city_two_turn)
    if args.command == "watch":
        problem = runtime_heartbeat_problem(
            RUNTIME_HEARTBEAT_DEFAULT, args.minutes)
        if problem:
            print(f"LADDER: {problem}")
            return 1
        return 0
    if args.command == "next":
        return next_rung(ledger)
    return show(ledger)


if __name__ == "__main__":
    sys.exit(main())
