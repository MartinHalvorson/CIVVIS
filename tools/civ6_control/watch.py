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

from collections import Counter
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import civ6_env as env  # noqa: E402

PREFIX = "CIVVISJSON "


def _encoded_event(event: dict) -> str:
    """Return the canonical on-disk representation for one controller event."""
    return json.dumps(event, sort_keys=True, separators=(",", ":"))


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


class EventLogBridge:
    """Copy one live run's Automation.log events into its ``events.jsonl``.

    The game cannot write into a run directory directly.  ``civ6_play.py``
    normally performs this relay while it owns the game, but an operator can
    also start a verified lobby by hand.  In that case CIVVIS's brain would
    otherwise wait forever for a state file despite the game exporting it.

    Starting a bridge after a game has already begun is deliberate: it tails
    the complete Automation log once, filters by the run tag, and replays every
    missing event.  Existing output is treated as a multiset rather than a
    simple set so two identical, meaningful events are never collapsed.
    """

    def __init__(self, run_dir: Path, *, tag: str | None = None,
                 log_path: Path | None = None):
        self.run_dir = Path(run_dir)
        self.tag = tag or self.run_dir.name
        self.events_path = self.run_dir / "events.jsonl"
        self.tail = LogTail(log_path)
        self._already_written: Counter[str] = Counter()
        self.run_dir.mkdir(parents=True, exist_ok=True)
        self.events_path.touch(exist_ok=True)
        self._read_existing()

    def _read_existing(self) -> None:
        """Count completed current-run lines so a restarted bridge is lossless."""
        for line in self.events_path.read_text(errors="replace").splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(event, dict) and event.get("run") == self.tag:
                self._already_written[_encoded_event(event)] += 1

    def pump(self) -> int:
        """Append newly seen events and return how many were copied."""
        lines: list[str] = []
        for event in self.tail.poll():
            if event.get("run") != self.tag:
                continue
            encoded = _encoded_event(event)
            if self._already_written[encoded]:
                self._already_written[encoded] -= 1
                continue
            lines.append(encoded)
        if not lines:
            return 0
        with self.events_path.open("a", buffering=1) as output:
            output.write("\n".join(lines) + "\n")
        return len(lines)

    def follow(self, timeout_s: float, *, poll_s: float = 0.5,
               on_write=None) -> int:
        """Relay events for ``timeout_s`` seconds and return the copied count."""
        deadline = time.time() + timeout_s
        copied = 0
        while time.time() < deadline:
            wrote = self.pump()
            copied += wrote
            if wrote and on_write is not None:
                on_write(wrote)
            time.sleep(poll_s)
        return copied


def follow(tail: LogTail, timeout_s: float, on_event, poll_s: float = 2.0,
           stop_when=None, each_poll=None, stall_s: float | None = 600.0) -> str:
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
    last_event = time.time()
    while time.time() < deadline:
        for event in tail.poll():
            on_event(event)
            last_event = time.time()
            if stop_when is not None and stop_when(event):
                return "stopped"
        if not env.game_pids():
            for event in tail.poll():
                on_event(event)
            return "game exited"
        # ⚠ A game can end without its process ending. One run played to turn
        # 152, dropped back to the main menu, and sat there: `game_pids()` was
        # still non-empty so this loop waited another twenty-six minutes and
        # would have burned the whole timeout. A live process is not a live
        # game, and a stalled attempt costs the next one its slot.
        if stall_s is not None and time.time() - last_event > stall_s:
            return f"stalled: no event for {stall_s:.0f}s"
        if each_poll is not None:
            each_poll()
        time.sleep(poll_s)
    return "timeout"


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--timeout", type=float, default=3600.0)
    ap.add_argument("--kinds", help="comma-separated event kinds to show")
    ap.add_argument("--run-dir", type=Path,
                    help="relay this run's Automation.log events into events.jsonl")
    ap.add_argument("--tag",
                    help="run tag to relay (defaults to the run directory name)")
    ap.add_argument("--poll", type=float, default=0.5,
                    help="seconds between Automation.log polls while relaying")
    args = ap.parse_args()

    if args.run_dir is not None:
        bridge = EventLogBridge(args.run_dir, tag=args.tag)

        def report(wrote: int) -> None:
            print(f"# relayed {wrote} event(s) for {bridge.tag}", flush=True)

        copied = bridge.follow(args.timeout, poll_s=args.poll, on_write=report)
        print(f"# relay finished after copying {copied} event(s)", file=sys.stderr)
        raise SystemExit(0)

    wanted = set(args.kinds.split(",")) if args.kinds else None

    def show(event: dict) -> None:
        if wanted and event.get("kind") not in wanted:
            return
        print(json.dumps(event, sort_keys=True), flush=True)

    reason = follow(LogTail(), args.timeout, show)
    print(f"# {reason}", file=sys.stderr)
