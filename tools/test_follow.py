#!/usr/bin/env python3
"""Regression checks for the live mirror's north-up staging."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import follow  # noqa: E402


class FollowTest(unittest.TestCase):
    def test_north_up_reflection_reencodes_river_on_the_other_endpoint(self) -> None:
        event = {
            "kind": "tiles", "turn": 7, "height": 9,
            "plots": [
                {"x": 3, "y": 3, "rv": 2, "ri": True},
                {"x": 4, "y": 2, "rv": 0, "ri": True},
            ],
        }
        with tempfile.TemporaryDirectory() as temporary:
            previous = follow.STAGE
            follow.STAGE = temporary
            try:
                follow.stage_events([json.dumps(event).encode()], 9)
                staged = json.loads((Path(temporary) / "events.jsonl").read_text())
            finally:
                follow.STAGE = previous

        plots = {(plot["x"], plot["y"]): plot for plot in staged["plots"]}
        self.assertEqual(plots[(3, 5)]["rv"], 32)
        self.assertEqual(plots[(4, 6)]["rv"], 0)
        self.assertTrue(plots[(3, 5)]["ri"])
