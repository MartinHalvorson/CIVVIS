#!/usr/bin/env python3
"""One writer at a time for the Civilization VI installation.

The repository already insists on one writer per branch and per worktree. The
game needs the same rule and does not have it: there is a single installation,
a single mod directory inside it, a single log file, and a single process. Two
harnesses driving that at once do not conflict loudly -- they conflict
*silently*, and the result reads as a flaky game.

That is not hypothetical. Two sessions ran the ladder against this install at
the same time; the second one's ``install()`` overwrote the first's mod between
its turns, so the first was reading events written by a mod it had not
installed, under a run tag it had never used. It looked like the game stalling,
crashing, and losing focus at random. Nothing in either run's log said another
run existed.

The lock is a directory with the holder's details in it, which is atomic to
create on every filesystem that matters here. A holder whose process is gone is
treated as stale and taken over, so a killed run does not block the next one
forever -- the failure this is guarding against is concurrency, not crashes.
"""

from __future__ import annotations

import json
import os
import shutil
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

LOCK = Path.home() / ".civvis-civ6-game.lock"


def _holder() -> dict | None:
    info = LOCK / "holder.json"
    if not info.is_file():
        return None
    try:
        return json.loads(info.read_text())
    except (json.JSONDecodeError, OSError):
        return None


def _alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def describe() -> str:
    held = _holder()
    if held is None:
        return "free"
    state = "running" if _alive(held.get("pid", -1)) else "stale"
    return (f"held by pid {held.get('pid')} ({state}) "
            f"tag={held.get('tag')} since {held.get('since')}")


def foreign_run(tag: str) -> str | None:
    """A description of somebody else's run in progress, or None.

    The lock only binds harnesses that take it. A run started before the lock
    existed, or from another checkout, will not -- so this asks the installation
    itself: if the game is up and the mod installed in it carries a run tag that
    is not ours, someone else is mid-game and starting now would overwrite their
    mod between their turns. That is exactly what happened once already, and it
    reads as the game being flaky rather than as a second writer.
    """
    import civ6_env as env  # noqa: PLC0415 - avoids a cycle at import time

    if not env.game_pids():
        return None
    config = env.assets_dir() / "DLC" / "CivvisControl" / "config.json"
    if not config.is_file():
        return None
    try:
        installed = json.loads(config.read_text()).get("RunTag")
    except (json.JSONDecodeError, OSError):
        return None
    if installed in (None, tag):
        return None
    return f"a game is running under run tag {installed!r}"


def acquire(tag: str, wait_s: float = 0.0, poll_s: float = 15.0) -> bool:
    """Take the lock, optionally waiting. False when someone else holds it."""
    deadline = time.time() + wait_s
    while True:
        foreign = foreign_run(tag)
        if foreign is not None:
            if time.time() >= deadline:
                return False
            time.sleep(poll_s)
            continue
        try:
            LOCK.mkdir()
        except FileExistsError:
            held = _holder()
            if held is not None and not _alive(held.get("pid", -1)):
                # The holder is gone. Clear it and try again rather than wait
                # out a lock nobody is using.
                release(force=True)
                continue
            if time.time() >= deadline:
                return False
            time.sleep(poll_s)
            continue
        (LOCK / "holder.json").write_text(json.dumps({
            "pid": os.getpid(),
            "tag": tag,
            "since": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        }, indent=2))
        return True


def release(force: bool = False) -> None:
    """Give the lock up. Only the holder releases it unless forced."""
    held = _holder()
    if not force and held is not None and held.get("pid") != os.getpid():
        return
    shutil.rmtree(LOCK, ignore_errors=True)


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--status", action="store_true")
    ap.add_argument("--break-stale", action="store_true",
                    help="clear a lock whose holder is no longer running")
    args = ap.parse_args()

    print(describe())
    if args.break_stale:
        held = _holder()
        if held is not None and not _alive(held.get("pid", -1)):
            release(force=True)
            print("cleared a stale lock")
        else:
            print("not stale; left alone")
    sys.exit(0)
