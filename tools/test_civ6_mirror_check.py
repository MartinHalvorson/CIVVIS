#!/usr/bin/env python3
"""Regression checks for mirror checker temporal alignment."""

from __future__ import annotations

import json
import io
import os
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from collections import Counter
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_mirror_check  # noqa: E402


class MirrorCheckTest(unittest.TestCase):
    def test_unavailable_live_mirror_is_reported_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            events = Path(temporary) / "events.jsonl"
            events.write_text(json.dumps({"kind": "state", "turn": 1}) + "\n")
            output = io.StringIO()
            with (
                mock.patch.object(
                    civ6_mirror_check,
                    "live_runtime_problems",
                    return_value=[],
                ),
                mock.patch.object(
                    civ6_mirror_check.urllib.request,
                    "urlopen",
                    side_effect=OSError("connection refused"),
                ),
                redirect_stdout(output),
            ):
                result = civ6_mirror_check.main([temporary])

        self.assertEqual(result, 1)
        self.assertIn("MIRROR  ⚠ unavailable", output.getvalue())
        self.assertIn("connection refused", output.getvalue())

    def test_live_state_is_exact_while_turn_completion_is_in_flight(self) -> None:
        self.assertTrue(civ6_mirror_check.exact_host_frame(96, 96, 95))
        self.assertFalse(civ6_mirror_check.exact_host_frame(96, 95, 95))

    def test_live_same_turn_replans_are_deferred_until_completion(self) -> None:
        self.assertTrue(
            civ6_mirror_check.live_same_turn_frame_handoff(86, 86, 85, 3)
        )
        self.assertTrue(
            civ6_mirror_check.live_same_turn_frame_handoff(86, 86, 86, 3)
        )
        self.assertFalse(
            civ6_mirror_check.live_same_turn_frame_handoff(86, 86, 87, 3)
        )
        self.assertFalse(
            civ6_mirror_check.live_same_turn_frame_handoff(86, 86, 85, 1)
        )
        self.assertFalse(
            civ6_mirror_check.live_same_turn_frame_handoff(
                86, 86, 85, 3, archive=True
            )
        )

    def test_archive_state_still_requires_a_completed_boundary(self) -> None:
        self.assertFalse(
            civ6_mirror_check.exact_host_frame(251, 251, 250, archive=True)
        )
        self.assertTrue(
            civ6_mirror_check.exact_host_frame(
                251, 251, 250, archive=True, terminal_turn=251
            )
        )

    def test_terminal_frame_is_an_exact_completed_archive_boundary(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            events = Path(temporary) / "events.jsonl"
            events.write_text(
                json.dumps({"kind": "turn", "turn": 250}) + "\n"
                + json.dumps({"kind": "state", "turn": 251}) + "\n"
                + json.dumps({"kind": "victory", "turn": 251}) + "\n"
            )
            _, playable_turn = civ6_mirror_check.load_export(temporary)
            terminal_turn = civ6_mirror_check.latest_terminal_turn(temporary)

        self.assertEqual(playable_turn, 250)
        self.assertEqual(terminal_turn, 251)

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
        self.assertTrue(civ6_mirror_check.civ_id_matches("babylon_stk", "babylon"))
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
            "gold": 25, "gold_per_turn": -3.25, "faith": 100, "trade_capacity": 3,
            "tourism_per_turn": 12.5,
            "government": "GOVERNMENT_MONARCHY",
            "dark_age": False, "golden_age": True, "heroic_golden_age": False,
            "public_stats": {
                "city_count": 3, "population": 22, "food": 26.25, "production": 16.75,
                "wonder_count": 1, "suzerain_count": 2,
                "nuclear_devices": 0, "thermonuclear_devices": 1,
            },
            "rivals": [{
                "score": 926, "military": 995, "science": 41.5, "culture": 23,
                "faith_per_turn": 19, "gold": 512, "gold_per_turn": -3,
                "faith": 88, "techs": 53, "civics": 44, "tourism": 61,
                "government": "GOVERNMENT_FASCISM",
                "dark_age": False, "golden_age": False, "heroic_golden_age": True,
                "public_stats": {
                    "city_count": 7, "population": 49, "food": 76, "production": 43,
                    "wonder_count": 5, "suzerain_count": 2,
                    "nuclear_devices": 4, "thermonuclear_devices": 1,
                },
            }],
        }
        board = {"me": {"trade_capacity": 3}, "players": [
            {"id": 0, "score": 177, "military": 2, "observed_military": 2,
             "gold": 25.0, "gold_per_turn": -3.25, "faith": 100.0,
             "tourism_per_turn": 12.5,
             "government": "monarchy", "age": "golden",
             "cities": 3, "population": 22, "wonder_count": 1, "suzerain_count": 2,
             "nuclear_devices": 0, "thermonuclear_devices": 1,
             "yields": {"food": 26.3, "production": 16.8, "science": 6.8, "culture": 6.0}},
            {"id": 1, "score": 926, "military": 995, "observed_military": 995,
             "gold": 512.0, "gold_per_turn": -3.0, "faith": 88.0,
             "government": "fascism", "age": "heroic",
             "cities": 7, "population": 49, "wonder_count": 5, "suzerain_count": 2,
             "nuclear_devices": 4, "thermonuclear_devices": 1,
             "tourism_per_turn": 61.0,
             "yields": {"food": 76.0, "production": 43.0, "science": 41.5,
                        "culture": 23.0, "faith": 19.0},
             "victories": {"science": {"techs": 53}, "culture": {"civics": 44}}},
        ]}
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(state, board), [])
        board["players"][1]["observed_military"] = 64
        self.assertIn("seat 1 military", civ6_mirror_check.public_fact_mismatches(state, board)[0])
        board["players"][1]["observed_military"] = 995
        board["me"]["trade_capacity"] = 4
        self.assertIn(
            "trade_capacity", civ6_mirror_check.public_fact_mismatches(state, board)[0]
        )
        board["me"]["trade_capacity"] = 3

        board["players"][1]["population"] = 48
        self.assertIn(
            "seat 1 population Civ6=49",
            civ6_mirror_check.public_fact_mismatches(state, board)[0],
        )
        board["players"][1]["population"] = 49
        board["players"][1]["victories"]["science"]["techs"] = 52
        self.assertIn(
            "seat 1 techs Civ6=53",
            civ6_mirror_check.public_fact_mismatches(state, board)[0],
        )
        board["players"][1]["victories"]["science"]["techs"] = 53
        board["players"][1]["tourism_per_turn"] = 60.5
        self.assertIn(
            "seat 1 tourism/turn Civ6=61",
            civ6_mirror_check.public_fact_mismatches(state, board)[0],
        )
        board["players"][1]["tourism_per_turn"] = 61.0

        board["players"][0]["tourism_per_turn"] = 12.0
        self.assertIn(
            "seat 0 tourism/turn Civ6=12.5",
            civ6_mirror_check.public_fact_mismatches(state, board)[0],
        )
        board["players"][0]["tourism_per_turn"] = 12.5

        board["players"][1]["government"] = "democracy"
        self.assertIn(
            "seat 1 government Civ6=fascism",
            civ6_mirror_check.public_fact_mismatches(state, board)[0],
        )
        board["players"][1]["government"] = "fascism"
        board["players"][1]["age"] = "normal"
        self.assertIn(
            "seat 1 age Civ6=heroic",
            civ6_mirror_check.public_fact_mismatches(state, board)[0],
        )
        board["players"][1]["age"] = "heroic"

        # Faith PER TURN is a rate like science and culture, and it is not the
        # city sum: the host pays unrecruitable Great Person points as Faith.
        # An older export has no key and a missing host answer is null; both
        # are silence, not disagreement.
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(state, board), [])
        state["faith_per_turn"] = None
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(state, board), [])
        state["faith_per_turn"] = 114.6
        self.assertIn(
            "seat 0 faith/turn Civ6=114.6",
            civ6_mirror_check.public_fact_mismatches(state, board)[0],
        )
        board["players"][0]["yields"]["faith"] = 114.6
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(state, board), [])

        # Zero is a valid top-bar yield; only the host's negative sentinel is
        # unavailable. A nonzero reconstructed reading must therefore be named.
        state["science"] = 0
        board["players"][0]["yields"]["science"] = 1
        self.assertIn(
            "seat 0 science/turn Civ6=0",
            civ6_mirror_check.public_fact_mismatches(state, board)[0],
        )
        state["science"] = 6.75
        board["players"][0]["yields"]["science"] = 6.8
        state["culture"] = 0
        board["players"][0]["yields"]["culture"] = 1
        self.assertIn(
            "seat 0 culture/turn Civ6=0",
            civ6_mirror_check.public_fact_mismatches(state, board)[0],
        )
        state["culture"] = 6.03125
        board["players"][0]["yields"]["culture"] = 6.0
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(state, board), [])

    def test_a_stronger_own_strength_model_is_not_a_bridge_disagreement(self) -> None:
        """`military_power` is max(observed, our own sum), so for our OWN seat it
        may legitimately exceed the host. That is a MODEL difference, not a
        mapping one, and this check used to report it as
        `seat 0 military Civ6=520 CIVVIS=545`. Measured over 2,713 turn-records
        the host's figure wins that max ~90% of the time, so the warning was rare,
        benign, and exactly the kind that trains an operator to ignore the report.
        """
        state = {"score": 100, "military": 520, "rivals": []}
        board = {"me": {}, "players": [
            {"id": 0, "score": 100, "military": 545, "observed_military": 520},
        ]}
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(state, board), [])

        # But a genuine MAPPING failure must still be caught.
        board["players"][0]["observed_military"] = 400
        self.assertIn(
            "seat 0 military", civ6_mirror_check.public_fact_mismatches(state, board)[0]
        )

    def test_public_victory_tracker_facts_are_exact(self) -> None:
        state = {
            "techs": ["TECH_POTTERY", "TECH_WRITING"],
            "civics": ["CIVIC_CODE_OF_LAWS", "CIVIC_CRAFTSMANSHIP", "CIVIC_FOREIGN_TRADE"],
            # Manhattan and Ivy are strategic projects, but not progress through
            # the four science-victory milestones. Base-game Mars parts count once.
            "science_projects": [
                "PROJECT_MANHATTAN_PROJECT", "PROJECT_OPERATION_IVY",
                "PROJECT_LAUNCH_EARTH_SATELLITE", "PROJECT_LAUNCH_MOON_LANDING",
                "PROJECT_LAUNCH_MARS_REACTOR", "PROJECT_LAUNCH_MARS_HABITATION",
                "PROJECT_LAUNCH_MARS_HYDROPONICS", "PROJECT_LAUNCH_EXOPLANET_EXPEDITION",
            ],
            "science_victory_points": 17.3,
            "science_victory_points_per_turn": 1.2,
            "science_victory_points_needed": 50,
            "foreign_tourists": 19,
            "domestic_tourists": 25,
            "cities_following_religion": 7,
            "dvp": 4,
            "rivals": [{
                # World Rankings wins over the older counted loop when both cross.
                "player": 17, "techs": 53, "techs_researched": 54, "civics": 44,
                "science_projects": [
                    "launch_earth_satellite", "PROJECT_LAUNCH_MARS_REACTOR",
                    "PROJECT_LAUNCH_MARS_HABITATION", "PROJECT_LAUNCH_MARS_HYDROPONICS",
                ],
                "science_victory_points": 6.4,
                "science_victory_points_per_turn": 0.9,
                "science_victory_points_needed": 25,
                "foreign_tourists": 31,
                "domestic_tourists": 29,
                "cities_following_religion": 16,
                "dvp": 13,
            }],
        }
        board = {"players": [
            {"id": 0, "victories": {
                "science": {"projects": 4, "distance": 17.3, "speed": 1.2,
                            "distance_target": 50, "techs": 2},
                "culture": {"tourists": 19, "domestic": 25, "civics": 3},
                "religious": {"cities_following": 7},
                "diplomatic": {"points": 4},
            }},
            {"id": 1, "victories": {
                "science": {"projects": 2, "distance": 6.4, "speed": 0.9,
                            "distance_target": 25, "techs": 54},
                "culture": {"tourists": 31, "domestic": 29, "civics": 44},
                "religious": {"cities_following": 16},
                "diplomatic": {"points": 13},
            }},
        ]}
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(state, board), [])

        checks = [
            (0, "science", "projects", 3, "seat 0 science projects Civ6=4"),
            (0, "science", "distance", 16.0, "seat 0 science distance Civ6=17.3"),
            (0, "science", "speed", 0.8, "seat 0 science speed Civ6=1.2"),
            (0, "science", "distance_target", 49, "seat 0 science target Civ6=50"),
            (0, "science", "techs", 1, "seat 0 techs Civ6=2"),
            (0, "culture", "tourists", 18, "seat 0 foreign tourists Civ6=19"),
            (0, "culture", "domestic", 24, "seat 0 domestic tourists Civ6=25"),
            (0, "culture", "civics", 2, "seat 0 civics Civ6=3"),
            (0, "religious", "cities_following", 6,
             "seat 0 cities following religion Civ6=7"),
            (0, "diplomatic", "points", 3, "seat 0 diplomatic points Civ6=4"),
            (1, "science", "projects", 1, "seat 1 science projects Civ6=2"),
            (1, "science", "distance", 6.0, "seat 1 science distance Civ6=6.4"),
            (1, "science", "speed", 0.7, "seat 1 science speed Civ6=0.9"),
            (1, "science", "distance_target", 24, "seat 1 science target Civ6=25"),
            (1, "science", "techs", 53, "seat 1 techs Civ6=54"),
            (1, "culture", "tourists", 30, "seat 1 foreign tourists Civ6=31"),
            (1, "culture", "domestic", 28, "seat 1 domestic tourists Civ6=29"),
            (1, "culture", "civics", 43, "seat 1 civics Civ6=44"),
            (1, "religious", "cities_following", 15,
             "seat 1 cities following religion Civ6=16"),
            (1, "diplomatic", "points", 12, "seat 1 diplomatic points Civ6=13"),
        ]
        for seat, lane, field, wrong, expected in checks:
            value = board["players"][seat]["victories"][lane][field]
            board["players"][seat]["victories"][lane][field] = wrong
            self.assertTrue(
                any(expected in mismatch
                    for mismatch in civ6_mirror_check.public_fact_mismatches(state, board)),
                expected,
            )
            board["players"][seat]["victories"][lane][field] = value

        # An unavailable ranking counter falls back to the older tree count.
        state["rivals"][0]["techs_researched"] = -1
        board["players"][1]["victories"]["science"]["techs"] = 53
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(state, board), [])

        # The absence of a fresh rival DVP is intentionally filled from the
        # congress table, so check that exact, potentially stale source too.
        state["rivals"][0]["dvp"] = None
        state["congress_dvp"] = {"points": [{"player": 17, "points": 12}]}
        board["players"][1]["victories"]["diplomatic"]["points"] = 12
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(state, board), [])
        board["players"][1]["victories"]["diplomatic"]["points"] = 11
        self.assertIn(
            "seat 1 diplomatic points Civ6=12",
            civ6_mirror_check.public_fact_mismatches(state, board)[0],
        )
        board["players"][1]["victories"]["diplomatic"]["points"] = 12

        # Old or refused fields are unknown, not a zero that should make the
        # checker call a reconstructed estimate a data mismatch.
        unavailable = {
            "techs": None, "civics": None, "science_projects": None,
            "science_victory_points": -1, "science_victory_points_per_turn": -1,
            "science_victory_points_needed": -1, "foreign_tourists": -1,
            "domestic_tourists": -1, "cities_following_religion": None, "dvp": None,
            "rivals": [{
                "techs": -1, "techs_researched": None, "civics": -1,
                "science_projects": None, "science_victory_points": -1,
                "science_victory_points_per_turn": -1, "science_victory_points_needed": -1,
                "foreign_tourists": -1, "domestic_tourists": -1,
                "cities_following_religion": None, "dvp": None,
            }],
        }
        self.assertEqual(civ6_mirror_check.public_fact_mismatches(unavailable, board), [])

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
                {"type": "DISTRICT_THEATER", "complete": False},
            ],
        }]}
        board = {"cities": [{
            "pos": [14, 9], "name": "Istanbul", "pop": 9, "food": 15.8,
            "loyalty": 100.0, "loyalty_per_turn": 10.3, "defense": 40.0,
            "hp": 150, "wall_hp": 60, "wall_max": 100,
            "religion": "Orthodoxy",
            "buildings": ["monument", "medieval_walls"],
            "wonders": {"pyramids": [15, 9]},
            "districts": {"campus": [15, 8]},
        }]}
        self.assertEqual(civ6_mirror_check.city_fact_mismatches(state, board, 44), [])
        board["cities"][0]["loyalty_per_turn"] = -5.0
        self.assertIn(
            "loyalty_per_turn",
            civ6_mirror_check.city_fact_mismatches(state, board, 44)[0],
        )

    def test_city_data_check_covers_housing_amenities_and_per_city_yields(self) -> None:
        # The board carries a host-to-model correction for Housing, the Amenity
        # count and every yield, so all three must read the host's figure to the
        # rounding; `amenities_needed` has no correction and guards the rule.
        state = {"cities": [{
            "x": 18, "y": 35, "name": "Rome", "pop": 12, "loyalty": 100,
            "housing": 10, "amenities": 4, "amenities_needed": 6,
            "yields": {"food": 35, "production": 16.8008, "gold": 21.6016,
                       "science": 6.40234, "culture": 6.87891, "faith": 4.80078},
            "buildings": [], "districts": [{"type": "DISTRICT_CITY_CENTER"}],
        }]}
        board = {"cities": [{
            "pos": [14, 9], "name": "Rome", "pop": 12, "loyalty": 100.0,
            "housing": 10.0, "amenities": 4, "amenities_required": 6,
            "yields": {"food": 35.0, "production": 16.8, "gold": 21.6,
                       "science": 6.4, "culture": 6.9, "faith": 4.8},
            "buildings": [], "districts": {}, "wonders": {},
        }]}
        self.assertEqual(civ6_mirror_check.city_fact_mismatches(state, board, 44), [])
        board["cities"][0]["housing"] = 8.5
        board["cities"][0]["amenities"] = 6
        board["cities"][0]["yields"]["science"] = 11.25
        found = civ6_mirror_check.city_fact_mismatches(state, board, 44)
        self.assertEqual(len(found), 3, found)
        self.assertIn("housing Civ6=10 CIVVIS=8.5", found[0])
        self.assertIn("amenities Civ6=4 CIVVIS=6", found[1])
        self.assertIn("yields.science Civ6=6.40234 CIVVIS=11.25", found[2])
        # An older export without the figures makes no claim.
        for key in ("housing", "amenities", "amenities_needed", "yields"):
            state["cities"][0].pop(key)
        board["cities"][0]["housing"] = 8.5
        self.assertEqual(civ6_mirror_check.city_fact_mismatches(state, board, 44), [])

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
        board = {"view_player": 0, "units": [], "visible": [[1, 5]]}
        mismatches = civ6_mirror_check.unit_fact_mismatches(state, board, 10)
        self.assertEqual(len(mismatches), 1)
        self.assertIn("Civ6=1 CIVVIS=0", mismatches[0])

        board["units"].append({
            "owner": 1, "type": "keshig", "pos": [1, 5],
            "hp": 72, "fortified": False, "fortify_turns": 0,
        })
        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

    def test_visible_phoenician_bireme_uses_its_modelled_unique_name(self) -> None:
        state = {"rivals": [{"units": [{
            "kind": "UNIT_PHOENICIA_BIREME", "base": "UNIT_GALLEY",
            "x": 3, "y": 5, "hp": 100, "fortified": False,
            "fortify_turns": 0,
        }]}]}
        board = {"view_player": 0, "units": [{
            "owner": 1, "type": "bireme", "pos": [1, 5],
            "hp": 100, "fortified": False, "fortify_turns": 0,
        }], "visible": [[1, 5]]}

        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

    def test_free_city_unit_exported_in_both_lists_is_counted_once(self) -> None:
        unit = {
            "id": 196608, "kind": "UNIT_CROSSBOWMAN", "x": 3, "y": 5,
            "hp": 100, "fortified": False, "fortify_turns": 0,
        }
        state = {
            "minors": [{
                "player": 62, "civ": "CIVILIZATION_FREE_CITIES",
                "units": [dict(unit)],
            }],
            "hostiles": [{
                "id": unit["id"], "type": unit["kind"], "player": 62,
                "x": unit["x"], "y": unit["y"], "hp": unit["hp"],
                "fortified": unit["fortified"],
                "fortify_turns": unit["fortify_turns"],
            }],
        }
        board = {"view_player": 0, "units": [{
            "owner": 6, "type": "crossbowman", "pos": [1, 5],
            "hp": 100, "fortified": False, "fortify_turns": 0,
        }], "visible": [[1, 5]]}

        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

    def test_hidden_rival_unit_is_not_compared_to_the_seated_board(self) -> None:
        state = {"rivals": [{"units": [{
            "kind": "UNIT_KNIGHT", "x": 3, "y": 5,
            "hp": 100, "fortified": False, "fortify_turns": 0,
        }]}]}
        board = {"view_player": 0, "units": [], "visible": [[0, 0]]}
        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

        board["visible"] = [[1, 5]]
        mismatches = civ6_mirror_check.unit_fact_mismatches(state, board, 10)
        self.assertEqual(len(mismatches), 1)
        self.assertIn("UNIT_KNIGHT@(1, 5) count Civ6=1 CIVVIS=0", mismatches[0])

    def test_hidden_minor_unit_is_not_compared_to_the_seated_board(self) -> None:
        state = {"minors": [{
            "player": 6, "civ": "CIVILIZATION_KABUL",
            "units": [{
                "kind": "UNIT_WARRIOR", "x": 3, "y": 5,
                "hp": 100, "fortified": False, "fortify_turns": 0,
            }],
        }]}
        board = {"view_player": 0, "units": [], "visible": [[0, 0]]}
        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

        board["visible"] = [[1, 5]]
        mismatches = civ6_mirror_check.unit_fact_mismatches(state, board, 10)
        self.assertEqual(len(mismatches), 1)
        self.assertIn("UNIT_WARRIOR@(1, 5) count Civ6=1 CIVVIS=0", mismatches[0])

    def test_foreign_unit_stack_does_not_consume_a_seated_unit(self) -> None:
        source = {
            "kind": "UNIT_TRADER", "x": 3, "y": 5,
            "hp": 100, "fortified": False, "fortify_turns": 0,
        }
        state = {
            "units": [source],
            "minors": [{
                "player": 6, "civ": "CIVILIZATION_KABUL",
                "units": [dict(source)],
            }],
        }
        board = {
            "view_player": 0,
            "units": [
                {"owner": 0, "type": "trader", "pos": [1, 5],
                 "hp": 100, "fortified": False, "fortify_turns": 0},
                {"owner": 6, "type": "trader", "pos": [1, 5],
                 "hp": 100, "fortified": False, "fortify_turns": 0},
            ],
            "visible": [[1, 5]],
        }

        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

    def test_unmodelled_unique_unit_uses_firaxis_replacement_role(self) -> None:
        state = {"rivals": [{"units": [{
            "kind": "UNIT_SCOTTISH_HIGHLANDER", "x": 3, "y": 5,
            "hp": 72, "fortified": False, "fortify_turns": 0,
        }]}]}
        board = {"view_player": 0, "units": [{
            "owner": 1, "type": "ranger", "pos": [1, 5],
            "hp": 72, "fortified": False, "fortify_turns": 0,
        }]}

        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

    def test_georgian_khevsureti_uses_man_at_arms_replacement_role(self) -> None:
        state = {"rivals": [{"units": [{
            "kind": "UNIT_GEORGIAN_KHEVSURETI", "x": 3, "y": 5,
            "hp": 88, "fortified": False, "fortify_turns": 0,
        }]}]}
        board = {"view_player": 0, "units": [{
            "owner": 1, "type": "man_at_arms", "pos": [1, 5],
            "hp": 88, "fortified": False, "fortify_turns": 0,
        }]}

        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

    def test_hostile_type_field_is_checked_as_a_real_unit_kind(self) -> None:
        state = {"hostiles": [{
            "type": "UNIT_WARRIOR", "player": 63, "x": 3, "y": 5,
            "hp": 79, "fortified": False, "fortify_turns": 0,
        }]}
        board = {"view_player": 0, "units": [{
            "owner": 4, "type": "warrior", "pos": [1, 5],
            "hp": 79, "fortified": False, "fortify_turns": 0,
        }], "visible": [[1, 5]]}
        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

    def test_hidden_hostile_is_not_compared_to_the_seated_board(self) -> None:
        state = {"hostiles": [{
            "type": "UNIT_SWORDSMAN", "player": 63, "x": 3, "y": 5,
            "hp": 83, "fortified": False, "fortify_turns": 0,
        }]}
        board = {"view_player": 0, "units": [], "visible": [[0, 0]]}
        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

        board["visible"] = [[1, 5]]
        mismatches = civ6_mirror_check.unit_fact_mismatches(state, board, 10)
        self.assertEqual(len(mismatches), 1)
        self.assertIn("UNIT_SWORDSMAN@(1, 5) count Civ6=1 CIVVIS=0", mismatches[0])

    def test_barbarian_horse_archer_uses_the_exact_host_variant(self) -> None:
        state = {"hostiles": [{
            "type": "UNIT_BARBARIAN_HORSE_ARCHER", "player": 63,
            "x": 3, "y": 5, "hp": 79, "fortified": False,
            "fortify_turns": 0,
        }]}
        board = {"view_player": 0, "units": [{
            "owner": 4, "type": "barbarian_horse_archer", "pos": [1, 5],
            "hp": 79, "fortified": False, "fortify_turns": 0,
        }], "visible": [[1, 5]]}
        self.assertEqual(civ6_mirror_check.unit_fact_mismatches(state, board, 10), [])

    def test_barbarian_horseman_uses_the_exact_host_variant(self) -> None:
        state = {"hostiles": [{
            "type": "UNIT_BARBARIAN_HORSEMAN", "player": 63,
            "x": 3, "y": 5, "hp": 79, "fortified": False,
            "fortify_turns": 0,
        }]}
        board = {"view_player": 0, "units": [{
            "owner": 4, "type": "barbarian_horseman", "pos": [1, 5],
            "hp": 79, "fortified": False, "fortify_turns": 0,
        }], "visible": [[1, 5]]}
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

    def test_city_state_military_uses_the_host_observation_not_unit_sum(self) -> None:
        state = {"rivals": [], "minors": [{
            "player": 6, "civ": "CIVILIZATION_KABUL", "score": 91,
            "military": 128, "envoys": 3, "suzerain": 0,
            "cities": [{"x": 18, "y": 35, "name": "Kabul"}],
        }]}
        board = {
            "players": [{
                "id": 6, "civ": "Kabul", "is_minor": True, "is_barbarian": False,
                "score": 91, "military": 135, "observed_military": 128,
                "my_envoys": 3, "suzerain": 0,
            }],
            "cities": [{"owner": 6, "pos": [14, 9], "name": "Kabul"}],
        }
        self.assertEqual(civ6_mirror_check.minor_fact_mismatches(state, board, 44), [])

        board["players"][0]["observed_military"] = 127
        self.assertIn(
            "kabul military Civ6=128",
            civ6_mirror_check.minor_fact_mismatches(state, board, 44)[0],
        )

        # A pre-observed-military board remains compatible with the checker;
        # there is no host-only value to compare on that older wire.
        board["players"][0].pop("observed_military")
        board["players"][0]["military"] = 128
        self.assertEqual(civ6_mirror_check.minor_fact_mismatches(state, board, 44), [])

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
        self.assertEqual(
            civ6_mirror_check.production_item_name("BUILDING_GOV_CITYSTATES"),
            ("building", "foreign_ministry"),
        )
        self.assertEqual(civ6_mirror_check.production_item_name("DISTRICT_CAMPUS"),
                         ("district", "campus"))
        self.assertEqual(civ6_mirror_check.production_item_name("DISTRICT_GOVERNMENT"),
                         ("district", "government_plaza"))
        self.assertEqual(civ6_mirror_check.production_item_name("DISTRICT_THEATER"),
                         ("district", "theater_square"))
        self.assertEqual(
            civ6_mirror_check.production_item_name("PROJECT_ENHANCE_DISTRICT_THEATER"),
            ("project", "theater_square_festival"),
        )
        self.assertEqual(civ6_mirror_check.production_item_name(None), None)
        self.assertEqual(civ6_mirror_check.queue_item_name({"district": "campus", "pos": [2, 3]}),
                         ("district", "campus"))

    def test_a_wonder_in_production_is_the_boards_wonder_kind(self) -> None:
        # Firaxis files wonders under BUILDING_; the board queues them as
        # `wonder`, so the PRODUCTION check must compare like with like.
        self.assertIn("taj_mahal", civ6_mirror_check.MIRRORED_WONDERS)
        self.assertEqual(civ6_mirror_check.production_item_name("BUILDING_TAJ_MAHAL"),
                         ("wonder", "taj_mahal"))
        self.assertEqual(civ6_mirror_check.production_item_name("BUILDING_PYRAMIDS"),
                         ("wonder", "pyramids"))
        self.assertEqual(
            civ6_mirror_check.production_item_name("BUILDING_TAJ_MAHAL"),
            civ6_mirror_check.queue_item_name({"wonder": "taj_mahal", "pos": [4, 5]}),
        )
        # An ordinary building keeps its kind.
        self.assertEqual(civ6_mirror_check.production_item_name("BUILDING_WALLS"),
                         ("building", "walls"))

    def test_state_selection_does_not_compare_a_future_turn_to_the_board(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            events = Path(temporary) / "events.jsonl"
            events.write_text(
                json.dumps({"kind": "state", "turn": 60, "frame": 0, "units": [{"id": 1}]}) + "\n"
                + json.dumps({"kind": "state", "turn": 60, "frame": 1, "units": [{"id": 2}]}) + "\n"
                + json.dumps({"kind": "state", "turn": 61, "units": [{"id": 2}]}) + "\n"
            )

            self.assertEqual(civ6_mirror_check.latest_state(temporary, upto=60)["turn"], 60)
            self.assertEqual(civ6_mirror_check.latest_state(temporary)["turn"], 61)
            latest, same_turn_frames = civ6_mirror_check.latest_state_and_frame_count(
                temporary, upto=60
            )
            self.assertEqual(latest["frame"], 1)
            self.assertEqual(same_turn_frames, 2)

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

        mountain_road = {
            "x": 7,
            "y": 6,
            "t": "TERRAIN_PLAINS_MOUNTAIN",
            "im": "IMPROVEMENT_MOUNTAIN_ROAD",
        }
        road_on_board = {
            "terrain": "mountain",
            "hills": False,
            "feature": None,
            "resource": None,
            "improvement": "qhapaq_nan",
            "river": False,
            "coastal_lowland": 0,
        }
        counts, examples = civ6_mirror_check.exact_tile_mismatches(
            [(road_on_board, mountain_road)]
        )
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

    def test_national_park_flag_is_the_boards_improvement(self) -> None:
        plot = {"x": 7, "y": 6, "t": "TERRAIN_PLAINS", "np": True}
        board = {"terrain": "plains", "hills": False, "feature": None,
                 "resource": None, "improvement": "national_park",
                 "river": False, "coastal_lowland": 0}
        counts, examples = civ6_mirror_check.exact_tile_mismatches([(board, plot)])
        self.assertEqual(counts, Counter())
        self.assertEqual(examples, [])


def test_a_rig_binary_older_than_the_decider_is_reported():
    """★★★★★ Presence is not currency.

    CONTROL used to say "export and CIVVIS worker are current" having checked only
    that the worker PROCESS exists. On 2026-08-02 the deployed rig binary was a day
    older than the decider's, so the CIVVIS window and every check in this file were
    reading a day-old reconstruction of a current game — and CONTROL reported OK.
    Rebuilding it moved four axes from failing to passing.
    """
    with tempfile.TemporaryDirectory() as tmp:
        rig = os.path.join(tmp, "civvis")
        decider = os.path.join(tmp, "civvis_orders")
        open(rig, "w").close()
        os.utime(rig, (1_000_000, 1_000_000))
        open(decider, "w").close()
        os.utime(decider, (1_000_000 + 7200, 1_000_000 + 7200))
        lines = [f"python3 tools/civ6_brain.py --run-dir /x --bin {decider} --victory score"]
        found = civ6_mirror_check.stale_rig_problems(lines, rig=rig)
        assert found, "a rig two hours behind the decider must be reported"
        assert "2.0h older" in found[0], found[0]


def test_a_current_rig_is_silent():
    """⚠ A check that always fires says nothing."""
    with tempfile.TemporaryDirectory() as tmp:
        rig = os.path.join(tmp, "civvis")
        decider = os.path.join(tmp, "civvis_orders")
        open(decider, "w").close()
        os.utime(decider, (1_000_000, 1_000_000))
        open(rig, "w").close()
        os.utime(rig, (1_000_000 + 60, 1_000_000 + 60))
        lines = [f"python3 tools/civ6_brain.py --run-dir /x --bin {decider}"]
        assert civ6_mirror_check.stale_rig_problems(lines, rig=rig) == []


def test_no_brain_means_nothing_to_compare_against():
    """⚠ With no decider named there is no claim to make, and inventing one would
    fail every archive replay."""
    assert civ6_mirror_check.stale_rig_problems(["python3 something_else.py"]) == []


def test_an_absent_rig_is_not_reported_as_stale():
    """⚠ Absent is not stale. A missing rig is a different failure and belongs to
    whoever starts the server."""
    with tempfile.TemporaryDirectory() as tmp:
        decider = os.path.join(tmp, "civvis_orders")
        open(decider, "w").close()
        lines = [f"python3 tools/civ6_brain.py --bin {decider}"]
        assert civ6_mirror_check.stale_rig_problems(lines, rig=os.path.join(tmp, "nope")) == []


def test_the_binary_actually_serving_is_the_one_compared():
    """⚠⚠⚠ The rig path was ASSUMED while the decider's was ASKED FOR.

    `follow.py`'s repo copy resolves its binary to `<repo>/target/release/civvis`,
    not the deployed `~/civvis-civ6-mirror/civvis` this defaulted to. So on any
    checkout-run follower the check compared a file NOBODY WAS RUNNING — and a stale
    binary at the real path would have been reported as fine, which is precisely the
    2026-08-02 failure this whole check exists to prevent.
    """
    with tempfile.TemporaryDirectory() as tmp:
        serving = os.path.join(tmp, "serving-civvis")
        unused = os.path.join(tmp, "deployed-civvis")
        decider = os.path.join(tmp, "civvis_orders")
        for path, when in ((serving, 1_000_000),          # stale, and it is the one running
                           (unused, 1_000_000 + 99_000),  # fresh, and nobody runs it
                           (decider, 1_000_000 + 7200)):
            open(path, "w").close()
            os.utime(path, (when, when))
        lines = [
            f"python3 tools/civ6_brain.py --run-dir /x --bin {decider}",
            f"nice -n 5 {serving} play --mirror /stage --players 6 --port 8610",
        ]
        found = civ6_mirror_check.stale_rig_problems(lines, rig=unused)
        assert found, "the binary on the server's own command line must win"
        assert serving in found[0], found[0]
        assert "2.0h older" in found[0], found[0]


def test_the_message_says_mtime_is_only_a_proxy():
    """A fresh copy of identical sources trips this, and a touched stale binary does
    not. Whoever reads it must be told to confirm before spending a rebuild."""
    with tempfile.TemporaryDirectory() as tmp:
        rig, decider = os.path.join(tmp, "civvis"), os.path.join(tmp, "civvis_orders")
        open(rig, "w").close(); os.utime(rig, (1_000_000, 1_000_000))
        open(decider, "w").close(); os.utime(decider, (1_003_600, 1_003_600))
        found = civ6_mirror_check.stale_rig_problems(
            [f"python3 tools/civ6_brain.py --bin {decider}"], rig=rig)
        assert "PROXY" in found[0], found[0]


def test_the_controllers_own_command_line_is_not_mistaken_for_the_server():
    """⚠ `civ6_play.py` also carries `play`. Matching it would compare a Python
    script's mtime against the decider and warn on every single run."""
    with tempfile.TemporaryDirectory() as tmp:
        rig, decider = os.path.join(tmp, "civvis"), os.path.join(tmp, "civvis_orders")
        open(rig, "w").close(); os.utime(rig, (1_003_600, 1_003_600))
        open(decider, "w").close(); os.utime(decider, (1_000_000, 1_000_000))
        lines = [
            f"python3 tools/civ6_brain.py --bin {decider}",
            "python3 -u /repo/tools/civ6_play.py --tag civvis-x --mirror-ish --civvis-decides",
        ]
        assert civ6_mirror_check.stale_rig_problems(lines, rig=rig) == []
