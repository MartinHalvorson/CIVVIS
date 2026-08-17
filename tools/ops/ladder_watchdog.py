#!/usr/bin/env python3
"""Restart the game supervisor when the ladder stops recording attempts.

Why this exists
---------------
On 2026-08-17 the Civilization VI climb loop stopped at 05:43Z and nobody
noticed for 14.3 hours. The project's first active objective is making a
Settler win repeatable, and the only way to move it is to finish games, so
every one of those hours was the top objective starved.

The galling part was that the detector already existed and was correct:
`civ6_ladder.py check --stale-hours 3` printed the right sentence and exited 1
the whole time. It was wired to nothing. The supervisor was the only caller, so
the check died with the process it was meant to watch.

Two layers, because there are two failure modes
-----------------------------------------------
launchd `KeepAlive` on `com.civvis.ladder` covers the supervisor *exiting* —
crash, terminal closed, logout, reboot. It cannot see the other failure: a
supervisor that is alive and looping but producing no attempts, because Civ 6
wedged, a build broke, or a game hung short of its summary. To the process
table that is a healthy job.

This watchdog is the second layer. It runs on its own launchd interval, in its
own process, and asks the only question that actually matters — *when did we
last finish a game?* Being a separate job is the entire point: it must be able
to outlive the thing it supervises.

Acting, not warning
-------------------
A notification at 03:00 is not a fix. When the ledger is stale this kicks the
supervisor job, which is the remedy a human would apply anyway. `--cooldown`
bounds that: a wedge this cannot clear escalates to one report rather than
kicking every interval until morning.

Staleness only
--------------
`check` exits nonzero for three different problems and they have three
different remedies — unrecorded summaries want `sync`, a trailing snapshot
wants `publish`. Restarting the supervisor fixes neither, so this reads the
staleness signal specifically, through `civ6_ladder.staleness_problem`, rather
than keying on that exit code.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import civ6_ladder  # noqa: E402

SUPERVISOR_LABEL = "com.civvis.ladder"
STATE_DEFAULT = Path.home() / ".cache" / "civvis" / "ladder-watchdog.json"
LOG_DEFAULT = Path.home() / "Library" / "Logs" / "civvis-ladder-watchdog.log"


def log(path: Path, message: str) -> None:
    line = f"{datetime.now(timezone.utc).isoformat(timespec='seconds')} {message}"
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


def kick(label: str, runner=None) -> tuple[bool, str]:
    """Restart the supervisor job. `launchctl kickstart -k` stops then starts.

    A plain `kickstart` on an already-running job is a no-op, which is exactly
    wrong for the wedged case this exists to clear: the job IS running, that is
    the problem. `-k` is what makes this a restart.

    `runner` resolves at call time rather than as a default argument: a default
    binds `subprocess.run` once at import, and no test could then substitute it.
    """
    runner = runner or subprocess.run
    domain = f"gui/{os.getuid()}"
    done = runner(["launchctl", "kickstart", "-k", f"{domain}/{label}"],
                  capture_output=True, text=True, timeout=60)
    detail = (done.stderr or done.stdout or "").strip()
    return done.returncode == 0, detail or f"exit {done.returncode}"


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
                        help="minimum hours between two kicks (default 2)")
    parser.add_argument("--label", default=SUPERVISOR_LABEL,
                        help=f"launchd job to restart (default {SUPERVISOR_LABEL})")
    parser.add_argument("--runs", type=Path, default=None,
                        help="runs directory the live ledger sits beside")
    parser.add_argument("--state", type=Path, default=STATE_DEFAULT)
    parser.add_argument("--log", type=Path, default=LOG_DEFAULT)
    parser.add_argument("--dry-run", action="store_true",
                        help="report the decision, restart nothing")
    args = parser.parse_args(argv)

    runs = args.runs if args.runs is not None else civ6_ladder.RUNS_DEFAULT
    ledger = runs / "ladder.json"
    now = datetime.now(timezone.utc)

    state = civ6_ladder.load(ledger)
    problem = civ6_ladder.staleness_problem(state, args.stale_hours, now=now)
    if problem is None:
        age = civ6_ladder.newest_attempt_age_hours(state, now=now)
        # Quiet on the healthy path. This runs every few minutes forever, and a
        # log that reports "fine" 400 times a day is a log nobody reads on the
        # morning it says something else.
        if args.dry_run:
            print(f"ladder is current ({age:.1f}h since the newest attempt)")
        return 0

    memory = read_state(args.state)
    since = hours_since(memory.get("last_kick_utc"), now)
    if since is not None and since < args.cooldown_hours:
        log(args.log, f"STALE {problem} — kicked {since:.1f}h ago, inside the "
                      f"{args.cooldown_hours:g}h cooldown; not kicking again")
        return 1

    log(args.log, f"STALE {problem}")
    if args.dry_run:
        log(args.log, f"  DRY RUN — would restart {args.label}")
        return 1

    ok, detail = kick(args.label)
    memory["last_kick_utc"] = now.isoformat(timespec="seconds")
    memory["kicks"] = int(memory.get("kicks", 0)) + 1
    memory["last_problem"] = problem
    write_state(args.state, memory)

    if ok:
        log(args.log, f"  restarted {args.label} (kick #{memory['kicks']})")
        notify("CIVVIS ladder loop restarted", problem)
    else:
        log(args.log, f"  could not restart {args.label}: {detail}")
        notify("CIVVIS ladder loop is stopped",
               f"restart failed: {detail}. See {args.log}.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
