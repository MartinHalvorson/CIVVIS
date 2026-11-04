#!/usr/bin/env python3
"""A host fault that no relaunch can fix: macOS stops showing the harness its game.

★★★★★ THE LANE STALLED FOR HOURS ON A HOST THAT ONLY A RESTART COULD FIX.

Every few weeks this Mac's LaunchServices / WindowServer stops registering new
apps. Civilization VI still launches and draws its main menu: CoreGraphics lists
its 1634x1084 window on screen. But System Events, which the harness uses to
find, place, click and quit the game, no longer knows the process at all
(``Can't get process "Civ6_Exe_Child"``), and a relaunched System Events sits
suspended at launch (``-2741`` / ``-600``). The game logs ``NSXPCSharedListener
... com.apple.view-bridge`` errors every ten seconds from its first second.

What the lane did with that, before this module existed:

* setup waited out sixteen "no game window yet" probes (~6 min) and reported
  "could not start a game from the main menu";
* the menu quit needs System Events, the SIGTERM after it was ignored, and the
  game was correctly never SIGKILLed, so it stayed up holding the seat;
* the supervisor retried the graceful quit every ~105 s, for hours.

Measured: 2026-09-27 (~6 h), 2026-10-15/16 (~20 h), 2026-10-27 (~8 h) and
2026-11-04 (01:54Z on). The 10-16 log carries the same System Events ``-2741``
syntax error, and 10-16 and 10-27 ended only when the Mac was restarted. On
11-04 user-level recovery was tried and failed: restarting System Events or
CoreServicesUIAgent does not help, and a test Calculator launch also stayed
suspended.

So the harness now names the fault after three probes (about a minute), records
it here, and stops the child it launched. The supervisor reads this marker
before every launch and holds the lane instead of orphaning another game. The
marker stamps the boot it was written in: a restart clears it, and so does a
later game that configures. It also allows one retry an hour, in case the host
recovers by itself.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

MARKER = Path(os.environ.get("CIVVIS_HOST_FAULT_MARKER",
                             str(Path.home() / ".civvis-host-needs-restart")))

#: One relaunch an hour tests whether the host has recovered without a restart.
#: On a host that has not, that relaunch costs about two minutes and one child
#: stopped before it hosted a game.
RETRY_AFTER_S = 3600.0

#: Words in the System Events error that mean the harness lacks a privacy
#: grant, not that LaunchServices is wedged. A restart does not fix these.
PERMISSION_WORDS = ("assistive access", "-25211", "-1743", "not authorized",
                    "not allowed")


def boot_time() -> int | None:
    """This boot's start, in epoch seconds, or None when the host cannot say."""
    try:
        out = subprocess.run(["sysctl", "-n", "kern.boottime"], capture_output=True,
                             text=True, timeout=5).stdout
    except (OSError, subprocess.SubprocessError):
        return None
    match = re.search(r"\bsec\s*=\s*(\d+)", out or "")
    return int(match.group(1)) if match else None


def remedy(evidence: str) -> str:
    """What fixes this fault, judged from the System Events error it carries."""
    lowered = evidence.lower()
    if any(word in lowered for word in PERMISSION_WORDS):
        return ("System Settings > Privacy & Security: allow the terminal that runs "
                "the lane under Accessibility and Automation")
    return ("restart this Mac. LaunchServices has stopped registering new apps, "
            "and a restart is the only cure seen so far (10-16, 10-27, 11-04)")


def record(evidence: str, tag: str | None = None, now: float | None = None,
           path: Path | None = None) -> dict:
    """Write the marker for a fault seen in this boot. Returns what was written."""
    path = path or MARKER
    now = time.time() if now is None else now
    data = {
        "boot": boot_time(),
        "detected_utc": datetime.fromtimestamp(now, timezone.utc).strftime(
            "%Y-%m-%dT%H:%M:%SZ"),
        "retry_after": now + RETRY_AFTER_S,
        "tag": tag,
        "evidence": evidence,
        "remedy": remedy(evidence),
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(data, indent=2) + "\n")
    temporary.replace(path)
    return data


def read(path: Path | None = None) -> dict | None:
    try:
        data = json.loads((path or MARKER).read_text())
    except (OSError, ValueError):
        return None
    return data if isinstance(data, dict) else None


def clear(path: Path | None = None) -> bool:
    """Remove the marker. True when there was one."""
    try:
        (path or MARKER).unlink()
    except FileNotFoundError:
        return False
    return True


def describe(data: dict) -> str:
    return (f"since {data.get('detected_utc', '?')} ({data.get('tag') or 'no run tag'}) "
            f"{data.get('evidence', 'the harness could not see the game')}. "
            f"Fix: {data.get('remedy') or remedy(str(data.get('evidence', '')))}. "
            "Games resume by themselves once it is fixed.")


def current(path: Path | None = None) -> dict | None:
    """The recorded fault if it was seen in this boot. An older one is removed:
    the restart since then was the fix."""
    data = read(path)
    if data is None:
        return None
    boot = boot_time()
    if boot is not None and data.get("boot") not in (None, boot):
        clear(path)
        return None
    return data


def hold(now: float | None = None, path: Path | None = None) -> str | None:
    """Why the lane must not launch now, or None when it may.

    A marker from an earlier boot is stale, because the restart was the fix, so
    it is removed. A marker whose hour is up lets one launch through. That
    launch rewrites the marker if the fault is still there and clears it if a
    game configures.
    """
    data = current(path)
    if data is None:
        return None
    now = time.time() if now is None else now
    try:
        retry_after = float(data.get("retry_after", 0))
    except (TypeError, ValueError):
        retry_after = 0.0
    if now >= retry_after:
        return None
    return describe(data)


def main(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--hold", action="store_true",
                        help="print why the lane must wait; exit 0 to hold, 1 to launch")
    action.add_argument("--show", action="store_true",
                        help="print the recorded fault, if any; exit 0 when there is one")
    action.add_argument("--clear", action="store_true", help="remove the marker")
    args = parser.parse_args(argv)
    if args.hold:
        reason = hold()
        if reason is None:
            return 1
        print(reason)
        return 0
    if args.show:
        data = current()
        if data is None:
            return 1
        print(describe(data))
        return 0
    print("cleared" if clear() else "no marker")
    return 0


if __name__ == "__main__":
    sys.exit(main())
