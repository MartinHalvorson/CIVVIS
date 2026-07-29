#!/usr/bin/env python3
"""Read what the controller is doing out of the game's own log, as it runs.

Both mod contexts write one JSON object per event to ``Logs/Automation.log``,
prefixed ``CIVVISJSON``. That file is the only channel out of the game on this
build -- there is no ``Lua.log`` here, so ``print`` from a mod goes nowhere --
and it is append-only while the game runs, so tailing it gives a live view
without touching the game.

The tail has to survive two things the game does to that file: it truncates it
on restart, and it writes without flushing on a schedule this side controls.
So the reader tracks its offset, notices when the file shrinks, and re-reads
from the start rather than silently reporting nothing for the rest of a run.
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import civ6_env as env  # noqa: E402

PREFIX = "CIVVISJSON "


class LogTail:
    """Yields decoded CIVVISJSON events from Automation.log as they appear."""

    def __init__(self, path: Path | None = None):
        self.path = path or (env.logs_dir() / "Automation.log")
        self.offset = 0
        self.partial = ""

    def poll(self) -> list[dict]:
        if not self.path.is_file():
            return []
        size = self.path.stat().st_size
        if size < self.offset:
            # The game restarted and truncated the log. Anything before this is
            # a previous run's; start again rather than read from a stale
            # offset, which would skip the whole of the new run.
            self.offset = 0
            self.partial = ""
        if size == self.offset:
            return []
        with self.path.open("r", errors="replace") as handle:
            handle.seek(self.offset)
            chunk = handle.read()
            self.offset = handle.tell()
        text = self.partial + chunk
        lines = text.split("\n")
        self.partial = lines.pop()  # keep any half-written trailing line
        events = []
        for line in lines:
            index = line.find(PREFIX)
            if index < 0:
                continue
            try:
                events.append(json.loads(line[index + len(PREFIX):]))
            except json.JSONDecodeError:
                continue
        return events


def follow(tail: LogTail, timeout_s: float, on_event, poll_s: float = 2.0,
           stop_when=None, each_poll=None) -> str:
    """Pump events to ``on_event`` until ``stop_when`` says so or time runs out.

    Returns a short reason string. The game exiting is reported as its own
    reason rather than as a timeout: a crashed run and a slow run need
    different responses, and a timeout hides which happened.

    ``each_poll`` runs once per poll and is what keeps the game in the
    foreground. macOS throttles a background application to almost no frames,
    and this controller's turn loop runs off game-core events, which are tied
    to frames -- so a browser window taking focus stops the game dead. That
    looked exactly like a machine under load, and cost a run that sat on turn
    15 for ten minutes with nothing wrong in any log.
    """
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        for event in tail.poll():
            on_event(event)
            if stop_when is not None and stop_when(event):
                return "stopped"
        if not env.game_pids():
            for event in tail.poll():
                on_event(event)
            return "game exited"
        if each_poll is not None:
            each_poll()
        time.sleep(poll_s)
    return "timeout"


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--timeout", type=float, default=3600.0)
    ap.add_argument("--kinds", help="comma-separated event kinds to show")
    args = ap.parse_args()

    wanted = set(args.kinds.split(",")) if args.kinds else None

    def show(event: dict) -> None:
        if wanted and event.get("kind") not in wanted:
            return
        print(json.dumps(event, sort_keys=True), flush=True)

    reason = follow(LogTail(), args.timeout, show)
    print(f"# {reason}", file=sys.stderr)
