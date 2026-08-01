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
            "gold": 25, "faith": 100, "trade_capacity": 3,
            "rivals": [{"score": 926, "military": 995}],
        }
        board = {"me": {"trade_capacity": 3}, "players": [
            {"id": 0, "score": 177, "military": 2, "gold": 25.0, "faith": 100.0,
             "yields": {"science": 6.8, "culture": 6.0}},
            {"id": 1, "score": 926, "military": 995},
        ]}
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(state, board), [])
        board["players"][1]["military"] = 64
        self.assertIn("seat 1 military", civ6_mirror_check.public_fact_mismatches(state, board)[0])
        board["players"][1]["military"] = 995
        board["me"]["trade_capacity"] = 4
        self.assertIn(
            "trade_capacity", civ6_mirror_check.public_fact_mismatches(state, board)[0]
        )

    def test_city_data_check_covers_non_positional_state(self) -> None:
        state = {"cities": [{
            "x": 18, "y": 35, "name": "Istanbul", "pop": 9, "food": 15.8281,
            "loyalty": 100, "loyalty_per_turn": 10.2656, "defense": 40,
            "damage": 50, "max_damage": 200,
            "wall_damage": 40, "max_wall_damage": 100,
            "religion": "RELIGION_ORTHODOXY",
            "buildings": ["BUILDING_MONUMENT", "BUILDING_PALACE", "BUILDING_CASTLE",
                          "BUILDING_PYRAMIDS"],
            "wonders": [{"type": "BUILDING_PYRAMIDS", "x": 19, "y": 35}],
            "districts": [
                {"type": "DISTRICT_CITY_CENTER"}, {"type": "DISTRICT_CAMPUS"},
                {"type": "DISTRICT_WONDER"},
            ],
        }]}
        board = {"cities": [{
            "pos": [14, 9], "name": "Istanbul", "pop": 9, "food": 15.8,
            "loyalty": 100.0, "loyalty_per_turn": 10.3, "defense": 40.0,
            "hp": 150, "wall_hp": 60, "wall_max": 100,
            "religion": "Orthodoxy",
            "buildings": ["monument", "palace", "medieval_walls"],
            "wonders": {"pyramids": [15, 9]},
            "districts": {"campus": [15, 8]},
        }]}
        self.assertEqual(civ6_mirror_check.city_fact_mismatches(state, board, 44), [])
        board["cities"][0]["loyalty_per_turn"] = -5.0
        self.assertIn(
            "loyalty_per_turn",
            civ6_mirror_check.city_fact_mismatches(state, board, 44)[0],
        )

    def test_city_health_check_catches_each_independent_pool(self) -> None:
        state = {"cities": [{
            "x": 3, "y": 5, "name": "Rome", "damage": 25, "max_damage": 100,
            "wall_damage": 12, "max_wall_damage": 50,
        }]}
        board = {"cities": [{
            "pos": [1, 5], "name": "Rome", "hp": 150,
            "wall_hp": 38, "wall_max": 50,
        }]}
        self.assertEqual(civ6_mirror_check.city_fact_mismatches(state, board, 10), [])

        board["cities"][0].update(hp=149, wall_hp=37, wall_max=49)
        mismatches = civ6_mirror_check.city_fact_mismatches(state, board, 10)
        self.assertTrue(any(" hp " in mismatch for mismatch in mismatches), mismatches)
        self.assertTrue(any("wall_hp" in mismatch for mismatch in mismatches), mismatches)
        self.assertTrue(any("wall_max" in mismatch for mismatch in mismatches), mismatches)

    def test_unit_data_check_covers_health_and_fortification(self) -> None:
        state = {"units": [{
            "kind": "UNIT_SCYTHIAN_HORSE_ARCHER", "x": 3, "y": 5,
            "hp": 64, "fortified": True, "fortify_turns": 2,
        }]}
        board = {"view_player": 0, "units": [{
            "owner": 0, "type": "saka_horse_archer", "pos": [1, 5],
            "hp": 64, "fortified": True, "fortify_turns": 2,
        }]}
        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])
        board["units"][0].update(hp=100, fortified=False, fortify_turns=0)
        mismatches = civ6_mirror_check.unit_fact_mismatches(state, board, 10)
        self.assertTrue(any(" hp " in mismatch for mismatch in mismatches), mismatches)
        self.assertTrue(any(" fortified " in mismatch for mismatch in mismatches), mismatches)
        self.assertTrue(any("fortify_turns" in mismatch for mismatch in mismatches), mismatches)

    def test_visible_rival_unique_unit_cannot_disappear(self) -> None:
        state = {"rivals": [{"units": [{
            "kind": "UNIT_MONGOLIAN_KESHIG", "x": 3, "y": 5,
            "hp": 72, "fortified": False, "fortify_turns": 0,
        }]}]}
        board = {"view_player": 0, "units": []}
        mismatches = civ6_mirror_check.unit_fact_mismatches(state, board, 10)
        self.assertEqual(len(mismatches), 1)
        self.assertIn("Civ6=1 CIVVIS=0", mismatches[0])

        board["units"].append({
            "owner": 1, "type": "keshig", "pos": [1, 5],
            "hp": 72, "fortified": False, "fortify_turns": 0,
        })
        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

    def test_hostile_type_field_is_checked_as_a_real_unit_kind(self) -> None:
        state = {"hostiles": [{
            "type": "UNIT_WARRIOR", "player": 63, "x": 3, "y": 5,
            "hp": 79, "fortified": False, "fortify_turns": 0,
        }]}
        board = {"view_player": 0, "units": [{
            "owner": 4, "type": "warrior", "pos": [1, 5],
            "hp": 79, "fortified": False, "fortify_turns": 0,
        }]}
        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

    def test_stacked_same_type_units_match_as_a_multiset(self) -> None:
        source = {
            "kind": "UNIT_BUILDER", "x": 3, "y": 5,
            "hp": 100, "fortified": False, "fortify_turns": 0,
        }
        state = {"units": [dict(source) for _ in range(3)]}
        board = {"view_player": 0, "units": [{
            "owner": 0, "type": "builder", "pos": [1, 5],
            "hp": 100, "fortified": False, "fortify_turns": 0,
        } for _ in range(3)]}
        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

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

    def test_renamed_city_state_matches_capital_not_legacy_type(self) -> None:
        state = {"rivals": [], "minors": [{
            "player": 8, "civ": "CIVILIZATION_JAKARTA", "score": 33,
            "military": 59, "envoys": 1, "suzerain": -1,
            "cities": [{
                "name": "Bandar Brunei", "capital": True, "x": 23, "y": 8,
            }],
        }]}
        board = {
            "players": [{
                "id": 6, "civ": "Bandar Brunei", "is_minor": True,
                "is_barbarian": False, "score": 33, "military": 59,
                "my_envoys": 1, "suzerain": None,
            }],
            "cities": [{
                "owner": 6, "name": "Bandar Brunei",
                "pos": list(civ6_mirror_check.axial(23, 44 - 8)),
            }],
        }

        self.assertEqual(civ6_mirror_check.minor_fact_mismatches(state, board, 44), [])

    def test_dormant_free_cities_is_not_a_city_state(self) -> None:
        state = {"minors": [{
            "player": 62, "civ": "CIVILIZATION_FREE_CITIES",
            "at_war": True, "cities": [], "units": [],
        }]}
        self.assertEqual(civ6_mirror_check.mirrored_minor_sources(state), [])
        self.assertEqual(civ6_mirror_check.minor_fact_mismatches(state, {}, 44), [])

    def test_present_free_city_uses_the_dedicated_seat(self) -> None:
        source = {
            "player": 62, "civ": "CIVILIZATION_FREE_CITIES",
            "score": 20, "military": 35,
            "cities": [{"x": 18, "y": 35, "name": "Free City"}], "units": [],
        }
        state = {"rivals": [], "minors": [source]}
        board = {
            "players": [{
                "id": 6, "civ": "Free Cities", "is_minor": True,
                "is_barbarian": True, "is_free_city": True,
                "score": 20, "military": 35,
            }],
            "cities": [{"owner": 6, "pos": [14, 9], "name": "Free City"}],
        }
        self.assertEqual(civ6_mirror_check.minor_fact_mismatches(state, board, 44), [])

    def test_production_identifiers_match_civvis_queue_items(self) -> None:
        self.assertEqual(civ6_mirror_check.production_item_name("UNIT_BUILDER"),
                         ("unit", "builder"))
        self.assertEqual(civ6_mirror_check.production_item_name("BUILDING_MONUMENT"),
                         ("building", "monument"))
        self.assertEqual(civ6_mirror_check.production_item_name("DISTRICT_CAMPUS"),
                         ("district", "campus"))
        self.assertEqual(civ6_mirror_check.production_item_name("DISTRICT_GOVERNMENT"),
                         ("district", "government_plaza"))
        self.assertEqual(civ6_mirror_check.production_item_name("DISTRICT_THEATER"),
                         ("district", "theater_square"))
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
