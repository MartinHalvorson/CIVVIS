#!/usr/bin/env python3
"""Keep the Civilization VI climb loop running, and running *productively*.

Why this exists
---------------
On 2026-08-17 the climb loop stopped at 05:43Z and nobody noticed for 14.3
hours. The project's first active objective is making a Settler win repeatable
and the only way to move it is to finish games, so every one of those hours was
the top objective starved.

The detector already existed and was correct: `civ6_ladder.py check
--stale-hours 3` printed the right sentence and exited 1 the whole time. It was
wired to nothing. The supervisor was its only caller, so the check died with
the process it was meant to watch.

⚠⚠⚠ THE LOOP CANNOT RUN AS A BARE LaunchAgent ON macOS
------------------------------------------------------
The obvious fix — run `civvis-game-supervisor.sh` under launchd with KeepAlive
— was shipped in #1888 and does not work, and the way it fails is silent enough
to be worth writing down. launchd started the supervisor, the supervisor built
head, launched the climb, and every attempt died at

    NO GAME — PermissionError: cannot install .../Assets/DLC/CivvisControl

Installing the control mod writes inside `Civ6.app`, and macOS attributes that
permission to the RESPONSIBLE process, not the user. Terminal holds the grant
on this host; `launchd` does not, and a LaunchAgent's children inherit
launchd's empty set. Measured both directions on 2026-08-17 with the same three
lines of Python:

    from a Terminal child          → direct write OK
    from a bare LaunchAgent        → PermissionError: Operation not permitted
    from a LaunchAgent that ran
      `open -a Terminal <script>`  → direct write OK

`install.py`'s Finder fallback does not rescue it either: driving Finder is an
Apple Event, and sending one needs an Automation grant that launchd also lacks.

So launchd stays the supervisor — it is the only thing here that survives a
closed session, a logout and a reboot — but it must start the loop THROUGH
Terminal, and this tool is what it runs.

Two failure modes, one keeper
-----------------------------
* **The loop is gone** — crashed, terminal closed, machine rebooted. Start one,
  through Terminal, so it inherits the grants it needs.
* **The loop is alive but not playing** — Civ 6 wedged, a build broke, a game
  hung short of its summary. To the process table that is a healthy job, and
  KeepAlive can never see it. Only the ledger can: *when did we last finish a
  game?* Stop the wedged supervisor and let the next tick start a fresh one.

Both rules live in one interval job rather than two, because they are the same
question asked in two directions and splitting them needs an ordering between
jobs that neither can enforce. What matters is that this runs in its OWN
process: it has to be able to outlive the thing it supervises.

Acting, not warning
-------------------
A notification at 03:00 is not a fix. `--cooldown-hours` bounds the acting: a
wedge this cannot clear escalates to one report rather than restarting the loop
every interval until morning. A restart that FAILED is not a restart and does
not start that clock.

Staleness only
--------------
`check` exits nonzero for three different problems with three different
remedies — unrecorded summaries want `sync`, a trailing snapshot wants
`publish`. Restarting the game loop fixes neither, so this reads the staleness
signal specifically, through `civ6_ladder.staleness_problem`, rather than
keying on that exit code.
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import civ6_ladder  # noqa: E402

SUPERVISOR_LABEL = "com.civvis.ladder"
STATE_DEFAULT = Path.home() / ".cache" / "civvis" / "ladder-watchdog.json"
LOG_DEFAULT = Path.home() / "Library" / "Logs" / "civvis-ladder-watchdog.log"
SUPERVISOR_LOCK = Path.home() / ".civvis-game-supervisor.lock"
SUPERVISOR_SCRIPT = (Path(__file__).resolve().parent
                     / "civvis-ladder-terminal-launcher.sh")


def log(path: Path, message: str) -> None:
    """Append to the log, and echo to a terminal only when there is one.

    ⚠ launchd points this job's StandardOutPath at the same file this writes,
    so an unconditional `print` puts every line in twice. That is what the
    first run of this watchdog produced, and a log that repeats itself is a log
    that makes you doubt the count. `isatty` keeps the echo for an operator
    running it by hand and drops it under launchd, where stdout IS the file.
    """
    line = f"{datetime.now(timezone.utc).isoformat(timespec='seconds')} {message}"
    if sys.stdout.isatty():
        print(line, flush=True)
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a") as handle:
            handle.write(line + "\n")
    except OSError:
        pass


def read_state(path: Path) -> dict:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return {}


def write_state(path: Path, state: dict) -> None:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp = path.with_suffix(".tmp")
        tmp.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")
        os.replace(tmp, path)
    except OSError:
        pass


def hours_since(stamp: str | None, now: datetime) -> float | None:
    if not stamp:
        return None
    try:
        then = datetime.fromisoformat(stamp)
    except ValueError:
        return None
    if then.tzinfo is None:
        then = then.replace(tzinfo=timezone.utc)
    return (now - then).total_seconds() / 3600


def supervisor_pid(lock: Path | None = None) -> int | None:
    """The live supervisor's pid, or `None`. A dead holder is not a supervisor.

    The supervisor writes its pid into its own lock directory and removes the
    directory from an EXIT trap, but a `kill -9` or a power cut leaves the
    directory behind — so the pid file is read as a claim to be verified, never
    as an answer.
    """
    lock = lock or SUPERVISOR_LOCK
    try:
        pid = int((lock / "pid").read_text().strip())
    except (OSError, ValueError):
        return None
    try:
        os.kill(pid, 0)
    except (ProcessLookupError, PermissionError):
        return None
    return pid


def start_supervisor(script: Path | None = None, runner=None) -> tuple[bool, str]:
    """Start the loop THROUGH Terminal, which is the only context that can play.

    ⚠⚠ NOT `zsh script`, and not a LaunchAgent that runs it directly. Installing
    the control mod writes inside `Civ6.app`, macOS attributes that permission
    to the responsible process, and a LaunchAgent's children inherit launchd's
    empty grant set — every attempt then dies at "cannot install
    .../DLC/CivvisControl" having played no turns. `open -a Terminal` hands the
    script to the one application on this host that holds the grant, and its
    children inherit it. Measured 2026-08-17; see this module's header.

    `open` returns as soon as Terminal has the document, so this reports that
    the loop was ASKED to start. Whether it took the lock is the next tick's
    question, which is the right place for it: that tick re-reads the lock.
    """
    runner = runner or subprocess.run
    script = script or SUPERVISOR_SCRIPT
    done = runner(["open", "-a", "Terminal", str(script)],
                  capture_output=True, text=True, timeout=60)
    detail = (done.stderr or done.stdout or "").strip()
    return done.returncode == 0, detail or f"exit {done.returncode}"


def stop_supervisor(pid: int, killer=None) -> tuple[bool, str]:
    """Ask a wedged supervisor to stop, so the next tick can start a fresh one.

    SIGTERM only. The supervisor traps it and releases its lock, and its own
    header is emphatic that hard kills wedge the Civilization VI core — the
    remedy would become the fault. A supervisor that ignores TERM is left alone
    and reported; the cooldown keeps that from repeating every interval.
    """
    killer = killer or os.kill
    try:
        killer(pid, signal.SIGTERM)
    except ProcessLookupError:
        return True, "already gone"
    except PermissionError as exc:
        return False, f"not permitted: {exc}"
    return True, f"SIGTERM sent to pid {pid}"


def notify(title: str, message: str, runner=None) -> None:
    runner = runner or subprocess.run
    script = f'display notification {message!r} with title {title!r}'
    try:
        runner(["osascript", "-e", script], capture_output=True, timeout=15)
    except (OSError, subprocess.SubprocessError):
        pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stale-hours", type=float, default=3.0,
                        help="ledger age that counts as stopped (default 3)")
    parser.add_argument("--cooldown-hours", type=float, default=2.0,
                        help="minimum hours between two restarts (default 2)")
    parser.add_argument("--supervisor", type=Path, default=None,
                        help="the loop to start (default: beside this file)")
    parser.add_argument("--lock", type=Path, default=None,
                        help="the supervisor's lock directory")
    parser.add_argument("--runs", type=Path, default=None,
                        help="runs directory the live ledger sits beside")
    parser.add_argument("--state", type=Path, default=STATE_DEFAULT)
    parser.add_argument("--log", type=Path, default=LOG_DEFAULT)
    parser.add_argument("--dry-run", action="store_true",
                        help="report the decision, change nothing")
    args = parser.parse_args(argv)

    runs = args.runs if args.runs is not None else civ6_ladder.RUNS_DEFAULT
    ledger = runs / "ladder.json"
    now = datetime.now(timezone.utc)
    alive = supervisor_pid(args.lock)

    state = civ6_ladder.load(ledger)
    problem = civ6_ladder.staleness_problem(state, args.stale_hours, now=now)

    if problem is None:
        age = civ6_ladder.newest_attempt_age_hours(state, now=now)
        # Quiet on the healthy path. This runs every few minutes forever, and a
        # log that reports "fine" 400 times a day is a log nobody reads on the
        # morning it says something else.
        if args.dry_run:
            print(f"ladder is current ({age:.1f}h since the newest attempt); "
                  f"supervisor {'pid ' + str(alive) if alive else 'absent'}")
        if alive is None:
            # Fresh ledger, no loop: the last game finished and the supervisor
            # then died. Start one — waiting for the ledger to go stale would
            # throw away the hours between now and the staleness limit.
            return start_the_loop(args, now, "no supervisor is running")
        return 0

    if alive is None:
        return start_the_loop(args, now, problem)

    # Alive, and not playing. KeepAlive can never see this; only the ledger can.
    memory = read_state(args.state)
    since = hours_since(memory.get("last_kick_utc"), now)
    if since is not None and since < args.cooldown_hours:
        log(args.log, f"STALE {problem} — supervisor pid {alive} was restarted "
                      f"{since:.1f}h ago, inside the {args.cooldown_hours:g}h "
                      f"cooldown; leaving it alone")
        return 1

    log(args.log, f"STALE {problem} — supervisor pid {alive} is alive but not "
                  f"finishing games")
    if args.dry_run:
        log(args.log, f"  DRY RUN — would stop pid {alive}")
        return 1

    ok, detail = stop_supervisor(alive)
    memory["last_problem"] = problem
    record_action(args, memory, now, ok, detail,
                  won=f"stopped the wedged supervisor: {detail}. "
                      f"The next tick starts a fresh one.",
                  lost=f"could not stop pid {alive}: {detail}")
    return 1


def start_the_loop(args: argparse.Namespace, now: datetime,
                   reason: str) -> int:
    """No supervisor is running. Start one, through Terminal."""
    memory = read_state(args.state)
    since = hours_since(memory.get("last_kick_utc"), now)
    if since is not None and since < args.cooldown_hours:
        log(args.log, f"{reason} — started {since:.1f}h ago and it is gone "
                      f"again, inside the {args.cooldown_hours:g}h cooldown; "
                      f"not starting another")
        return 1

    log(args.log, f"{reason} — starting the loop through Terminal")
    if args.dry_run:
        log(args.log, f"  DRY RUN — would run: open -a Terminal "
                      f"{args.supervisor or SUPERVISOR_SCRIPT}")
        return 1

    ok, detail = start_supervisor(args.supervisor)
    memory["last_problem"] = reason
    record_action(args, memory, now, ok, detail,
                  won="asked Terminal to start the loop",
                  lost=f"could not start the loop: {detail}")
    return 1


def record_action(args: argparse.Namespace, memory: dict, now: datetime,
                  ok: bool, detail: str, *, won: str, lost: str) -> None:
    """Log the outcome and start the cooldown clock only on one that worked.

    ⚠ THE COOLDOWN CLOCK STARTS ON AN ACTION THAT LANDED, NOT ON ONE THAT
    FAILED. It exists so a wedge we cannot clear is not restarted every ten
    minutes until morning, and that reasoning only holds when the remedy
    actually ran. An action that never took effect has changed nothing, so
    parking it for two hours just extends the outage this exists to end.
    Measured on this host at 2026-08-17T20:41:48Z: the first restart failed
    because the job it addressed was not loaded, and the failure took the full
    cooldown with it.
    """
    if ok:
        memory["last_kick_utc"] = now.isoformat(timespec="seconds")
        memory["kicks"] = int(memory.get("kicks", 0)) + 1
        write_state(args.state, memory)
        log(args.log, f"  {won} (action #{memory['kicks']})")
        notify("CIVVIS ladder loop", won)
    else:
        memory["last_failed_kick_utc"] = now.isoformat(timespec="seconds")
        memory["failed_kicks"] = int(memory.get("failed_kicks", 0)) + 1
        write_state(args.state, memory)
        log(args.log, f"  {lost}")
        notify("CIVVIS ladder loop is stopped",
               f"{lost}. See {args.log}.")


if __name__ == "__main__":
    sys.exit(main())
