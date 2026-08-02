#!/usr/bin/env python3
"""Focused checks for the direct-session Automation.log relay."""

from __future__ import annotations

import json
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import watch  # noqa: E402
from civ6_control.watch import EventLogBridge, PREFIX  # noqa: E402


def line(event: dict) -> str:
    return PREFIX + json.dumps(event) + "\n"


class EventLogBridgeTest(unittest.TestCase):
    def test_backfills_only_the_requested_run_and_preserves_duplicates(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log = root / "Automation.log"
            run = root / "run"
            repeated = {"kind": "await", "run": "live", "polls": 1}
            existing = {"kind": "seat", "run": "live", "leader": "LEADER_TRAJAN"}
            log.write_text(
                line({"kind": "seat", "run": "other"})
                + line(existing)
                + line(repeated)
                + line(repeated)
            )
            run.mkdir()
            (run / "events.jsonl").write_text(json.dumps(existing) + "\n")

            bridge = EventLogBridge(run, tag="live", log_path=log)
            self.assertEqual(bridge.pump(), 2)
            self.assertEqual(bridge.pump(), 0)

            events = [json.loads(row) for row in
                      (run / "events.jsonl").read_text().splitlines()]
            self.assertEqual(events, [existing, repeated, repeated])

    def test_keeps_a_partial_log_line_until_it_is_complete(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log = root / "Automation.log"
            event = {"kind": "state", "run": "live", "turn": 1}
            payload = PREFIX + json.dumps(event)
            log.write_text(payload[:20])

            bridge = EventLogBridge(root / "run", tag="live", log_path=log)
            self.assertEqual(bridge.pump(), 0)
            with log.open("a") as output:
                output.write(payload[20:] + "\n")
            self.assertEqual(bridge.pump(), 1)
            self.assertEqual(
                [json.loads(row) for row in
                 (root / "run" / "events.jsonl").read_text().splitlines()],
                [event],
            )


class FollowTest(unittest.TestCase):
    def test_locked_interval_does_not_consume_timeout(self) -> None:
        class Tail:
            def __init__(self) -> None:
                self.polls = 0

            def poll(self) -> list[dict]:
                self.polls += 1
                return [{"kind": "done"}] if self.polls == 3 else []

        seen: list[dict] = []
        with patch.object(watch.time, "monotonic",
                          side_effect=[0.0, 0.0, 2.0, 4.0, 4.0]), \
             patch.object(watch.time, "sleep") as sleep, \
             patch.object(watch.env, "game_pids", return_value=[123]):
            reason = watch.follow(
                Tail(), 1.0, seen.append, poll_s=0.25,
                stop_when=lambda event: event["kind"] == "done",
                pause_when=iter([True, True, False]).__next__,
            )

        self.assertEqual(reason, "stopped")
        self.assertEqual(seen, [{"kind": "done"}])
        self.assertEqual(sleep.call_count, 2)
class FakeTail:
    """Emits whatever the test scripts, one batch per poll."""

    def __init__(self, batches):
        self.batches = list(batches)

    def poll(self):
        return self.batches.pop(0) if self.batches else []


def _follow(batches, **kw):
    # `env.game_pids()` is consulted every pass and must look alive, or the loop
    # returns "game exited" before either watchdog is reached.
    original = watch.env.game_pids
    watch.env.game_pids = lambda: [1]
    try:
        return watch.follow(FakeTail(batches), timeout_s=5.0, on_event=lambda e: None,
                            poll_s=0.01, **kw)
    finally:
        watch.env.game_pids = original


def test_a_frozen_turn_is_caught_even_though_events_keep_arriving():
    # The exact shape of the congress wedge: a turn record, then the same turn
    # over and over, forever.
    batches = [[{"kind": "turn", "turn": 209}]] + [[{"kind": "state", "turn": 209}]] * 400
    reason = _follow(batches, stall_s=None, frozen_s=0.05)
    assert reason.startswith("stalled"), reason
    assert "209" in reason and "not advanced" in reason, reason


class RisingTail:
    """A game that never stops advancing, so the loop can only end on timeout.

    ⚠ A finite script cannot test this: once it runs out the turn really has
    stopped, and the watchdog fires — correctly. My first version of this test
    asserted the bug back.
    """

    def __init__(self):
        self.turn = 0

    def poll(self):
        self.turn += 1
        return [{"kind": "turn", "turn": self.turn}]


def test_a_run_whose_turn_is_advancing_is_never_called_frozen():
    original = watch.env.game_pids
    watch.env.game_pids = lambda: [1]
    try:
        reason = watch.follow(RisingTail(), timeout_s=0.5, on_event=lambda e: None,
                              poll_s=0.01, stall_s=None, frozen_s=0.05)
    finally:
        watch.env.game_pids = original
    assert reason == "timeout", reason


def test_setup_emits_no_turn_and_must_not_be_killed():
    """⚠ Before a turn is ever seen there is nothing to freeze.

    Setup emits no turn at all. Treating "no turn yet" as a frozen turn would kill
    every run before it started — the same mistake `popup_clear.py` guards with its
    own "no turn recorded yet; this is setup" refusal.
    """
    batches = [[{"kind": "seat", "civ": "CIVILIZATION_CANADA"}]] * 400
    reason = _follow(batches, stall_s=None, frozen_s=0.05)
    assert "not advanced" not in reason, reason


def test_silence_is_still_caught_by_its_own_watchdog():
    """The original death must keep working; the two are independent."""
    reason = _follow([[]], stall_s=0.05, frozen_s=None)
    assert reason.startswith("stalled: no event"), reason


class FrozenFollowTest(unittest.TestCase):
    def test_frozen_turn_is_caught_while_events_keep_arriving(self) -> None:
        test_a_frozen_turn_is_caught_even_though_events_keep_arriving()

    def test_advancing_turn_is_never_called_frozen(self) -> None:
        test_a_run_whose_turn_is_advancing_is_never_called_frozen()

    def test_setup_without_a_turn_is_not_called_frozen(self) -> None:
        test_setup_emits_no_turn_and_must_not_be_killed()

    def test_silence_keeps_its_independent_watchdog(self) -> None:
        test_silence_is_still_caught_by_its_own_watchdog()


if __name__ == "__main__":
    failures = 0
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            started = time.time()
            try:
                fn()
                print(f"PASS {name}  ({time.time() - started:.2f}s)")
            except AssertionError as exc:
                failures += 1
                print(f"FAIL {name}: {exc}")
    raise SystemExit(1 if failures else 0)
