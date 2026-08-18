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
# A live lock holder protects a single active game.  An explicit halt is a
# different thing: it is the operator saying that *no* new game may start until
# they resume it.  Keeping that intent in a small durable marker avoids making
# a Terminal/launchd process immortal just to preserve a user decision.
OPERATOR_HALT = Path(os.environ.get(
    "CIVVIS_OPERATOR_HALT_FILE",
    str(Path.home() / ".civvis-operator-halt.json"),
))


def _holder() -> dict | None:
    info = LOCK / "holder.json"
    if not info.is_file():
        return None
    try:
        return json.loads(info.read_text())
    except (json.JSONDecodeError, OSError):
        return None


def operator_halt() -> dict | None:
    """The durable operator halt request, if one exists.

    A malformed or unreadable marker is deliberately still a halt.  Starting
    a live game after an operator asked it to stay down is worse than requiring
    an explicit ``--resume`` to clear a damaged marker.
    """
    try:
        value = json.loads(OPERATOR_HALT.read_text())
    except FileNotFoundError:
        return None
    except (json.JSONDecodeError, OSError):
        return {"invalid": True}
    if not isinstance(value, dict):
        return {"invalid": True}
    return value


def operator_halt_description() -> str | None:
    """A human-readable explicit halt, or ``None`` when play is allowed."""
    requested = operator_halt()
    if requested is None:
        return None
    if requested.get("invalid"):
        return ("an unreadable explicit operator halt marker is present; "
                "refusing to start until it is cleared with --resume")
    since = requested.get("since") or "an unknown time"
    reason = str(requested.get("reason") or "").strip()
    suffix = f" (reason: {reason})" if reason else ""
    return (f"the game is explicitly halted since {since}{suffix}; "
            "run gamelock.py --resume before starting another game")


def request_operator_halt(reason: str = "") -> dict:
    """Persist an operator halt atomically and return the recorded request."""
    requested = {
        "pid": os.getpid(),
        "since": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "reason": reason.strip(),
    }
    OPERATOR_HALT.parent.mkdir(parents=True, exist_ok=True)
    temporary = OPERATOR_HALT.with_name(f".{OPERATOR_HALT.name}.{os.getpid()}.tmp")
    try:
        temporary.write_text(json.dumps(requested, indent=2) + "\n")
        os.replace(temporary, OPERATOR_HALT)
    finally:
        temporary.unlink(missing_ok=True)
    return requested


def clear_operator_halt() -> bool:
    """Clear the explicit halt only when an operator asks to resume."""
    try:
        OPERATOR_HALT.unlink()
    except FileNotFoundError:
        return False
    return True


def _alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


#: Argv substrings that mark a process as a harness actually driving a run.
#: Matching the tag alone is not enough -- an editor, a `tail -f` on the run's
#: log, or a shell history expansion all carry the tag in their argv and none of
#: them is holding the game.
#:
#: ⚠ ERR TOWARDS INCLUDING A LAUNCHER. A name missing here makes a genuinely
#: live run look dead and lets a second writer in, which is the silent
#: corruption this whole module exists to prevent. A name that is here but never
#: writes the tag costs nothing. `civ6_run.py` installs the *grounding* mod
#: rather than CivvisControl and so cannot produce this tag today; it is listed
#: anyway so that changing which mod it installs cannot quietly open a hole.
_HARNESS_MARKERS = (
    "civ6_play.py",
    "civ6_brain.py",
    "civ6_civvis_climb.py",
    "civ6_run.py",
)


def _processes() -> list[tuple[int, str]]:
    """Every process as (pid, argv), or an empty list if `ps` cannot be read."""
    import subprocess  # noqa: PLC0415 - only needed on this path

    try:
        out = subprocess.run(
            ["ps", "-Ao", "pid=,args="],
            capture_output=True, text=True, timeout=10, check=False,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    rows = []
    for line in out.splitlines():
        pid_text, _, args = line.strip().partition(" ")
        try:
            rows.append((int(pid_text), args))
        except ValueError:
            continue
    return rows


def _tag_has_live_owner(tag: str) -> bool:
    """Whether a live harness process is actually driving this run tag.

    ⚠ EXCLUDES THIS PROCESS AND ITS ANCESTRY. A caller that already carries the
    tag in its own argv would otherwise find itself and conclude that somebody
    else holds the game. That exact self-match has bitten this repository
    before, when a liveness probe matched its own `grep` argv and reported every
    run live.

    ⚠ Fails CLOSED. If `ps` cannot be read we cannot prove the tag is dead, and
    the safe answer is that it is alive -- concurrency is the failure this
    module exists to prevent, so an unreadable process table must not hand out
    the game.
    """
    rows = _processes()
    if not rows:
        return True
    mine = {os.getpid(), os.getppid()}
    for pid, args in rows:
        if pid in mine:
            continue
        if tag in args and any(marker in args for marker in _HARNESS_MARKERS):
            return True
    return False


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
    # ⚠⚠ A TAG IS NOT AN OWNER. Every other path in this module treats a holder
    # whose process is gone as stale and takes it over -- the module docstring
    # says so outright, "the failure this is guarding against is concurrency,
    # not crashes". This check was the one that did not, and the asymmetry
    # WEDGES THE MACHINE: a harness that dies without teardown leaves its tag
    # written into the installed mod, and from then on every new run is
    # "foreign" to a corpse. Civilization VI stays up, so the guard never
    # expires, and nothing starts again until a human clears it by hand.
    #
    # Observed 2026-08-03: tag 'civvis-20260803T212834Z' with no process behind
    # it refused three consecutive launches over seven minutes. `civ6_play.py`
    # run directly cannot recover from it at all -- only `civ6_civvis_climb.py`
    # clears the tag, and only on the teardown path it actually reaches.
    #
    # So ask whether anybody is really driving that tag. A tag with nothing
    # behind it describes nothing, which is the same conclusion the climb
    # teardown reached; this puts it where every launcher benefits.
    if not _tag_has_live_owner(installed):
        return None
    return f"a game is running under run tag {installed!r}"


def acquire(tag: str, wait_s: float = 0.0, poll_s: float = 15.0) -> bool:
    """Take the lock, optionally waiting. False when someone else holds it."""
    deadline = time.monotonic() + wait_s
    while True:
        # This check belongs here as well as in the supervisors.  It prevents a
        # manual or legacy launcher from bypassing the operator's explicit
        # halt simply because it did not use the current host script.
        if operator_halt() is not None:
            return False
        foreign = foreign_run(tag)
        if foreign is not None:
            if time.monotonic() >= deadline:
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
            if time.monotonic() >= deadline:
                return False
            time.sleep(poll_s)
            continue
        (LOCK / "holder.json").write_text(json.dumps({
            "pid": os.getpid(),
            "tag": tag,
            "since": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        }, indent=2))
        return True


def standing_hold() -> str | None:
    """A deliberate persistent halt or live holder that drives no run, or None.

    ⚠⚠ THE HOLDER CHECK AND THE FOREIGN-RUN CHECK ASK DIFFERENT QUESTIONS, AND
    THE WEAKER ONE GUARDS THE LOCK. `acquire` treats a holder as real when its
    *pid is alive*; `foreign_run` — after the 2026-08-03 wedge recorded above —
    treats a tag as real only when a harness is actually behind it. A process
    can be alive and driving nothing, and the commonest example is deliberate:
    `com.civvis.operator-halt` takes this lock and calls `signal.pause()`, which
    is exactly how an operator stops the machine playing.

    That is a legitimate holder and this module must NOT break it — a halt that
    anything can override is not a halt. What it must not be is *silent*. On
    this host a halt taken on 2026-08-02 was still in force fifteen days later;
    every climb was refused, each wrote one `blocked` row nobody reads, and the
    ladder keeper would have restarted the loop every fifteen minutes forever
    because a restart is its only remedy and it could not see the cause.

    So this reports the state rather than resolving it: whoever is deciding
    whether to act can tell "nothing is playing because the machine is halted"
    from "nothing is playing because the loop is wedged", which are the same
    symptom and opposite remedies.
    """
    explicit = operator_halt_description()
    if explicit is not None:
        return explicit

    held = _holder()
    if held is None:
        return None
    pid = held.get("pid", -1)
    if not _alive(pid):
        return None
    tag = held.get("tag") or ""
    if _tag_has_live_owner(tag):
        return None
    since = held.get("since")
    age = ""
    if since:
        try:
            started = datetime.strptime(since, "%Y-%m-%dT%H:%M:%SZ").replace(
                tzinfo=timezone.utc)
            age = f", {(datetime.now(timezone.utc) - started).days}d ago"
        except ValueError:
            age = ""
    return (f"the game is held by pid {pid} under tag {tag!r} since {since}"
            f"{age}, and no harness is driving that tag")


def release(force: bool = False) -> None:
    """Give the lock up. Only the holder releases it unless forced."""
    held = _holder()
    if not force and held is not None and held.get("pid") != os.getpid():
        return
    shutil.rmtree(LOCK, ignore_errors=True)


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    actions = ap.add_mutually_exclusive_group()
    actions.add_argument("--halt", action="store_true",
                         help="persist an explicit operator halt")
    actions.add_argument("--resume", action="store_true",
                         help="clear the explicit operator halt")
    actions.add_argument("--hold-status", action="store_true",
                         help="print an explicit or standing hold and exit 0, else exit 1")
    ap.add_argument("--reason", default="",
                    help="optional note recorded with --halt")
    ap.add_argument("--status", action="store_true")
    ap.add_argument("--break-stale", action="store_true",
                    help="clear a lock whose holder is no longer running")
    args = ap.parse_args()

    if args.halt:
        request_operator_halt(args.reason)
        print(operator_halt_description())
        sys.exit(0)
    if args.resume:
        cleared = clear_operator_halt()
        print("cleared explicit operator halt" if cleared
              else "no explicit operator halt was present")
        sys.exit(0)
    if args.hold_status:
        standing = standing_hold()
        if standing is None:
            sys.exit(1)
        print(standing)
        sys.exit(0)

    print(describe())
    standing = standing_hold()
    if standing is not None:
        print(f"standing hold: {standing}")
    if args.break_stale:
        held = _holder()
        if held is not None and not _alive(held.get("pid", -1)):
            release(force=True)
            print("cleared a stale lock")
        else:
            print("not stale; left alone")
    sys.exit(0)
