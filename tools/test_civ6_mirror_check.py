#!/usr/bin/env python3
"""Regression checks for mirror checker temporal alignment."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_mirror_check  # noqa: E402


class MirrorCheckTest(unittest.TestCase):
    def test_civ6_setup_identifiers_normalize_to_the_board_vocabulary(self) -> None:
        self.assertEqual(civ6_mirror_check.civ6_id("GAMESPEED_ONLINE", "GAMESPEED_"), "online")
        self.assertEqual(civ6_mirror_check.civ6_id("DIFFICULTY_SETTLER", "DIFFICULTY_"), "settler")
        self.assertEqual(civ6_mirror_check.civ6_map_script("Continents.lua"), "continents")
        self.assertEqual(civ6_mirror_check.civ6_map_script("SmallContinents.lua"),
                         "small_continents")

    def test_roster_aliases_do_not_hide_a_real_setup_mismatch(self) -> None:
        self.assertTrue(civ6_mirror_check.civ_id_matches("ottoman", "ottomans"))
        self.assertTrue(civ6_mirror_check.leader_id_matches("suleiman_alt", "suleiman"))
        self.assertFalse(civ6_mirror_check.civ_id_matches("ottoman", "rome"))
        self.assertFalse(civ6_mirror_check.leader_id_matches("suleiman_alt", "saladin"))

    def test_production_identifiers_match_civvis_queue_items(self) -> None:
        self.assertEqual(civ6_mirror_check.production_item_name("UNIT_BUILDER"),
                         ("unit", "builder"))
        self.assertEqual(civ6_mirror_check.production_item_name("BUILDING_MONUMENT"),
                         ("building", "monument"))
        self.assertEqual(civ6_mirror_check.production_item_name("DISTRICT_CAMPUS"),
                         ("district", "campus"))
        self.assertEqual(civ6_mirror_check.production_item_name("DISTRICT_GOVERNMENT"),
                         ("district", "government_plaza"))
        self.assertEqual(civ6_mirror_check.production_item_name(None), None)
        self.assertEqual(civ6_mirror_check.queue_item_name({"district": "campus", "pos": [2, 3]}),
                         ("district", "campus"))

    def test_state_selection_does_not_compare_a_future_turn_to_the_board(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            events = Path(temporary) / "events.jsonl"
            events.write_text(
                json.dumps({"kind": "state", "turn": 60, "units": [{"id": 1}]}) + "\n"
                + json.dumps({"kind": "state", "turn": 61, "units": [{"id": 2}]}) + "\n"
            )

            self.assertEqual(civ6_mirror_check.latest_state(temporary, upto=60)["turn"], 60)
            self.assertEqual(civ6_mirror_check.latest_state(temporary)["turn"], 61)

    def test_active_trade_routes_compare_endpoint_pairs_not_trader_positions(self) -> None:
        state = {
            "trade_routes": [{
                "trader": 42,
                "origin_x": 6,
                "origin_y": 4,
                "destination_x": 9,
                "destination_y": 5,
            }]
        }
        board = {
            "cities": [
                {"id": 101, "pos": [-14, 41]},
                {"id": 102, "pos": [-11, 40]},
            ],
            "me": {"routes": [{"origin": 101, "dest": 102}]},
        }
        expected = Counter({((-14, 41), (-11, 40)): 1})
        self.assertEqual(civ6_mirror_check.exported_route_pairs(state, 45), expected)
        self.assertEqual(civ6_mirror_check.board_route_pairs(board), expected)
