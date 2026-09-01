#!/usr/bin/env python3
"""Dismiss only the verified in-game unit-loss popup in a protected recovery.

The normal Lua autocloser reports ``InGamePopup`` but this Civ VI build can
leave that particular unit-loss acknowledgement modal on screen.  When it is
left up, the agent correctly emits its orders yet receives no further game
events.  This narrow helper follows one run's event stream and presses Escape
only after that exact screen is reported.  It takes no screenshots and never
acts before the loaded game has seated the agent.
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import sys
import time
from pathlib import Path


def parse_args() -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tag", required=True)
    ap.add_argument("--run-dir", type=Path, required=True)
    ap.add_argument("--log", type=Path, required=True)
    return ap.parse_args()


def say(log: Path, message: str) -> None:
    line = f"[protected-popup-guard] {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())} {message}"
    print(line, flush=True)
    with log.open("a") as handle:
        handle.write(line + "\n")


def matching_player_is_live(tag: str) -> bool:
    # Avoid depending on pgrep's own command line.  The continuation launcher
    # creates exactly one python player with this tag; an absent player means
    # the guard has no business sending global input.
    try:
        import subprocess

        result = subprocess.run(
            ["pgrep", "-f", r"[c]iv6_play\.py"],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        return False
    for raw in result.stdout.splitlines():
        if not raw.isdigit():
            continue
        proc = Path("/bin/ps")
        command = subprocess.run(
            [str(proc), "-p", raw, "-o", "command="],
            capture_output=True,
            text=True,
            check=False,
        ).stdout
        if "civ6_play.py" in command and f"--tag {tag}" in command:
            return True
    return False


def main() -> int:
    args = parse_args()
    repo_tools = Path("/Users/martbot-mbp-m5-max-128/CIVVIS/tools")
    sys.path.insert(0, str(repo_tools))
    import civ6_play  # noqa: PLC0415 - require the known local runtime
    from civ6_control import macos_input  # noqa: PLC0415

    stopped = False

    def stop(_signum: int, _frame: object) -> None:
        nonlocal stopped
        stopped = True

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    events = args.run_dir / "events.jsonl"
    offset = 0
    waiting_since = time.monotonic()
    seen_seat = False
    seen_popups: set[tuple[int | None, int]] = set()
    say(args.log, f"watching {args.tag} for InGamePopup after seat")

    while not stopped:
        if (args.run_dir / "summary.json").exists():
            say(args.log, "summary appeared; stopping without more input")
            return 0
        if not matching_player_is_live(args.tag):
            # The player may need a few minutes to reach the loaded-game seat.
            if time.monotonic() - waiting_since > 300:
                say(args.log, "player never became live; stopping")
                return 0
            time.sleep(0.5)
            continue
        if not events.exists():
            time.sleep(0.25)
            continue
        with events.open("r") as handle:
            handle.seek(offset)
            lines = handle.readlines()
            offset = handle.tell()
        for raw in lines:
            try:
                event = json.loads(raw)
            except json.JSONDecodeError:
                continue
            if event.get("kind") == "seat":
                seen_seat = True
                continue
            if not seen_seat:
                continue
            if event.get("kind") != "autoclose" or event.get("screen") != "InGamePopup":
                continue
            identity = (event.get("turn"), offset)
            if identity in seen_popups:
                continue
            seen_popups.add(identity)
            # Let the event-producing frame settle before the key event.  It is
            # intentionally one press: further Escape presses on the map would
            # open the pause menu rather than make progress.
            time.sleep(0.4)
            civ6_play.focus_game()
            time.sleep(0.15)
            result = macos_input.press_key("escape")
            say(args.log, f"dismissed reported InGamePopup (returncode={result.returncode})")
        time.sleep(0.2)
    say(args.log, "received stop signal")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
