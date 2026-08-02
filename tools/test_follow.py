#!/usr/bin/env python3
"""Regression checks for the live mirror's north-up staging."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import follow  # noqa: E402


class FollowTest(unittest.TestCase):
    def test_visible_server_start_fails_fast_when_child_exits(self) -> None:
        class DeadProcess:
            returncode = 17

            @staticmethod
            def poll() -> int:
                return 17

        messages = []
        with tempfile.TemporaryDirectory() as temporary:
            previous_rig = follow.RIG
            follow.RIG = temporary
            try:
                with mock.patch.object(follow, "read_events", return_value=([], 1, 4, 9)), \
                     mock.patch.object(follow, "stage_events", return_value=temporary), \
                     mock.patch.object(follow, "server_alive", return_value=False), \
                     mock.patch.object(follow, "log", side_effect=messages.append), \
                     mock.patch.object(follow.subprocess, "Popen", return_value=DeadProcess()) as popen, \
                     mock.patch.object(follow.time, "sleep") as sleep:
                    self.assertFalse(follow.start_visible_server(temporary, 4))
            finally:
                follow.RIG = previous_rig

        popen.assert_called_once()
        sleep.assert_not_called()
        self.assertTrue(any("status 17" in message for message in messages))

    def test_even_height_map_keeps_both_polar_rows_on_an_extra_staging_row(self) -> None:
        event = {
            "kind": "tiles", "turn": 7, "height": 46,
            "plots": [
                {"x": 3, "y": 0, "rv": 0},
                {"x": 4, "y": 45, "rv": 0},
            ],
        }
        with tempfile.TemporaryDirectory() as temporary:
            previous = follow.STAGE
            follow.STAGE = temporary
            try:
                follow.stage_events([json.dumps(event).encode()], 46)
                staged = json.loads((Path(temporary) / "events.jsonl").read_text())
            finally:
                follow.STAGE = previous

        self.assertEqual(staged["height"], 47)
        self.assertEqual(
            {(plot["x"], plot["y"]) for plot in staged["plots"]},
            {(3, 46), (4, 1)},
        )

    def test_north_up_reflection_transforms_qualified_coordinate_pairs(self) -> None:
        event = {
            "kind": "state", "turn": 7,
            "trade_routes": [{
                "trader": 42,
                "origin_x": 3, "origin_y": 1,
                "destination_x": 6, "destination_y": 6,
            }],
            "refusal": {
                "from_x": 2, "from_y": 3,
                "x": 4, "y": 4,
            },
        }
        with tempfile.TemporaryDirectory() as temporary:
            previous = follow.STAGE
            follow.STAGE = temporary
            try:
                follow.stage_events([json.dumps(event).encode()], 9)
                staged = json.loads((Path(temporary) / "events.jsonl").read_text())
            finally:
                follow.STAGE = previous

        route = staged["trade_routes"][0]
        self.assertEqual((route["origin_x"], route["origin_y"]), (3, 7))
        self.assertEqual((route["destination_x"], route["destination_y"]), (6, 2))
        self.assertEqual((staged["refusal"]["from_x"], staged["refusal"]["from_y"]), (2, 5))
        self.assertEqual((staged["refusal"]["x"], staged["refusal"]["y"]), (4, 4))

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

    def test_north_up_reflection_keeps_a_boundary_edge_from_all_six_bits(self) -> None:
        event = {
            "kind": "tiles", "turn": 7, "height": 9,
            "plots": [{"x": 3, "y": 3, "rv": 8, "ri": True}],
        }
        with tempfile.TemporaryDirectory() as temporary:
            previous = follow.STAGE
            follow.STAGE = temporary
            try:
                follow.stage_events([json.dumps(event).encode()], 9)
                staged = json.loads((Path(temporary) / "events.jsonl").read_text())
            finally:
                follow.STAGE = previous

        self.assertEqual(staged["plots"][0]["rv"], 8)
        self.assertTrue(staged["plots"][0]["ri"])
