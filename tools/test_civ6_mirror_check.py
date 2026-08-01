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
    def test_live_runtime_rejects_a_game_with_a_stale_missing_controller(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            events = Path(temporary) / "events.jsonl"
            events.write_text("{}\n")
            problems = civ6_mirror_check.live_runtime_problems(
                temporary,
                process_text="/game/Civ6_Exe_Child\n",
                now=events.stat().st_mtime + 121,
            )
        self.assertTrue(any("controller is absent" in problem for problem in problems))
        self.assertTrue(any("export is 121s stale" in problem for problem in problems))

    def test_live_runtime_requires_the_decision_worker_for_civvis_control(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tag = Path(temporary).name
            events = Path(temporary) / "events.jsonl"
            events.write_text("{}\n")
            processes = (
                "/game/Civ6_Exe_Child\n"
                f"python tools/civ6_play.py --tag {tag} --civvis-decides\n"
            )
            problems = civ6_mirror_check.live_runtime_problems(
                temporary, process_text=processes, now=events.stat().st_mtime
            )
        self.assertEqual(problems, ["the CIVVIS decision worker is absent"])

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

    def test_rivals_are_compared_with_their_compacted_mirror_seats(self) -> None:
        state = {
            "rivals": [{
                "player": 3,
                "civ": "CIVILIZATION_SCYTHIA",
                "leader": "LEADER_TOMYRIS",
            }]
        }
        correct = {
            "players": [
                {"id": 1, "civ": "Scythia", "leader": "Tomyris"},
                {"id": 3, "civ": "Egypt", "leader": "Cleopatra"},
            ]
        }
        wrong = {
            "players": [
                {"id": 1, "civ": "Egypt", "leader": "Cleopatra"},
                {"id": 3, "civ": "Scythia", "leader": "Tomyris"},
            ]
        }
        self.assertEqual(civ6_mirror_check.rival_identity_mismatches(state, correct), [])
        self.assertIn(
            "seat 1",
            civ6_mirror_check.rival_identity_mismatches(state, wrong)[0],
        )

    def test_public_scores_military_and_empire_yields_are_exact(self) -> None:
        state = {
            "score": 177, "military": 2, "science": 6.75, "culture": 6.03125,
            "gold": 25, "faith": 100,
            "rivals": [{"score": 926, "military": 995}],
        }
        board = {"players": [
            {"id": 0, "score": 177, "military": 2, "gold": 25.0, "faith": 100.0,
             "yields": {"science": 6.8, "culture": 6.0}},
            {"id": 1, "score": 926, "military": 995},
        ]}
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(state, board), [])
        board["players"][1]["military"] = 64
        self.assertIn("seat 1 military", civ6_mirror_check.public_fact_mismatches(state, board)[0])

    def test_city_data_check_covers_non_positional_state(self) -> None:
        state = {"cities": [{
            "x": 18, "y": 35, "name": "Istanbul", "pop": 9, "food": 15.8281,
            "loyalty": 100, "loyalty_per_turn": 10.2656, "defense": 40,
            "religion": "RELIGION_ORTHODOXY",
            "buildings": ["BUILDING_MONUMENT", "BUILDING_PALACE"],
            "districts": [
                {"type": "DISTRICT_CITY_CENTER"}, {"type": "DISTRICT_CAMPUS"}
            ],
        }]}
        board = {"cities": [{
            "pos": [14, 9], "name": "Istanbul", "pop": 9, "food": 15.8,
            "loyalty": 100.0, "loyalty_per_turn": 10.3, "defense": 40.0,
            "religion": "Orthodoxy", "buildings": ["monument", "palace"],
            "districts": {"campus": [15, 8]},
        }]}
        self.assertEqual(civ6_mirror_check.city_fact_mismatches(state, board, 44), [])
        board["cities"][0]["loyalty_per_turn"] = -5.0
        self.assertIn(
            "loyalty_per_turn",
            civ6_mirror_check.city_fact_mismatches(state, board, 44)[0],
        )

    def test_met_city_state_check_includes_envoys_suzerain_and_city(self) -> None:
        state = {"rivals": [], "minors": [{
            "player": 6, "civ": "CIVILIZATION_KABUL", "score": 91,
            "military": 74, "envoys": 3, "suzerain": 0,
            "cities": [{"x": 18, "y": 35, "name": "Kabul"}],
        }]}
        board = {
            "players": [{
                "id": 6, "civ": "Kabul", "is_minor": True, "is_barbarian": False,
                "score": 91, "military": 74, "my_envoys": 3, "suzerain": 0,
            }],
            "cities": [{"owner": 6, "pos": [14, 9], "name": "Kabul"}],
        }
        self.assertEqual(civ6_mirror_check.minor_fact_mismatches(state, board, 44), [])
        board["players"][0]["suzerain"] = None
        self.assertIn(
            "suzerain",
            civ6_mirror_check.minor_fact_mismatches(state, board, 44)[0],
        )

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

    def test_exact_tile_check_compares_contents_not_only_coordinate_overlap(self) -> None:
        plot = {
            "x": 4,
            "y": 5,
            "t": "TERRAIN_GRASS_HILLS",
            "f": "FEATURE_FOREST",
            "r": "RESOURCE_DEER",
            "im": "IMPROVEMENT_CAMP",
            "ri": True,
            "cl": 2,
        }
        exact = {
            "terrain": "grassland",
            "hills": True,
            "feature": "forest",
            "resource": "deer",
            "improvement": "camp",
            "river": True,
            "coastal_lowland": 2,
        }
        counts, examples = civ6_mirror_check.exact_tile_mismatches([(exact, plot)])
        self.assertEqual(counts, Counter())
        self.assertEqual(examples, [])

        wrong = dict(exact, terrain="desert", resource=None, river=False)
        counts, examples = civ6_mirror_check.exact_tile_mismatches([(wrong, plot)])
        self.assertEqual(counts, Counter({"terrain": 1, "resource": 1, "river": 1}))
        self.assertEqual(len(examples), 3)

    def test_locked_resource_export_is_treated_as_a_knowledge_leak(self) -> None:
        plot = {"x": 4, "y": 5, "t": "TERRAIN_PLAINS", "r": "RESOURCE_NITER"}
        board = {"terrain": "plains", "hills": False, "feature": None,
                 "resource": None, "improvement": None, "river": False,
                 "coastal_lowland": 0}
        state = {"techs": ["TECH_MINING"], "civics": []}
        counts, _ = civ6_mirror_check.exact_tile_mismatches([(board, plot)], state)
        self.assertEqual(counts, Counter())
        self.assertEqual(
            civ6_mirror_check.leaked_hidden_resources([(board, plot)], state),
            ["niter@4,5"],
        )
