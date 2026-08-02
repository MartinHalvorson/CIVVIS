#!/usr/bin/env python3
"""Focused checks for the direct-session Automation.log relay."""

from __future__ import annotations

import json
import sys
import tempfile
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
