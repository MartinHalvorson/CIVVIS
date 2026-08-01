#!/usr/bin/env python3
"""Focused checks for the direct-session Automation.log relay."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

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
