#!/usr/bin/env python3
"""Regression tests for live Civilization VI failure detectors."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_watchdogs


class DroppedUnitTest(unittest.TestCase):
    def report(self, notes: list[str]) -> dict:
        with tempfile.TemporaryDirectory() as directory:
            run = Path(directory)
            rows = [json.dumps({"note": note}) for note in notes]
            (run / "civvis_notes.jsonl").write_text("\n".join(rows) + "\n")
            return civ6_watchdogs.dropped_units(run)

    def test_bridge_managed_great_people_are_not_unordered_unit_failures(self) -> None:
        report = self.report([
            "dropped_units=2 [UNIT_GREAT_PROPHET@8,4:great_person "
            "UNIT_GREAT_SCIENTIST@2,3:great_person]"
        ])

        self.assertEqual(report["turns_with_drops"], 0)
        self.assertEqual(report["worst_on_one_turn"], 0)
        self.assertEqual(report["by_reason"], {})
        self.assertEqual(report["bridge_managed_great_person_observations"], 2)

    def test_real_drop_remains_loud_beside_a_managed_great_person(self) -> None:
        report = self.report([
            "dropped_units=2 [UNIT_GREAT_WRITER@8,4:great_person "
            "UNIT_UNKNOWN@2,3:untranslatable]"
        ])

        self.assertEqual(report["turns_with_drops"], 1)
        self.assertEqual(report["worst_on_one_turn"], 1)
        self.assertEqual(report["by_reason"], {"untranslatable": 1})
        self.assertEqual(report["bridge_managed_great_person_observations"], 1)

    def test_stock_role_approximation_is_not_reported_as_a_missing_unit(self) -> None:
        report = self.report([
            "dropped_units=3 [UNIT_HUNGARY_BLACK_ARMY@18,22:approximated_as_courser "
            "UNIT_HUNGARY_HUSZAR@18,27:approximated_as_cavalry "
            "UNIT_GREAT_ARTIST@18,25:great_person]"
        ])

        self.assertEqual(report["turns_with_drops"], 0)
        self.assertEqual(report["worst_on_one_turn"], 0)
        self.assertEqual(report["by_reason"], {})
        self.assertEqual(report["approximation_turns"], 1)
        self.assertEqual(report["approximated_units"], 2)
        self.assertEqual(
            report["approximated_by_reason"],
            {"approximated_as_courser": 1, "approximated_as_cavalry": 1},
        )
        self.assertEqual(report["bridge_managed_great_person_observations"], 1)


class IdleStackTest(unittest.TestCase):
    def test_bridge_managed_great_people_do_not_inflate_ordinary_stack_metrics(self) -> None:
        city = {"x": 8, "y": 4}
        events = [
            {
                "kind": "state",
                "turn": 10,
                "cities": [city],
                "units": [
                    {"id": 1, "kind": "UNIT_WARRIOR", "x": 8, "y": 4},
                    {"id": 2, "kind": "UNIT_GREAT_WRITER", "x": 8, "y": 4},
                ],
            },
            {
                "kind": "state",
                "turn": 11,
                "cities": [city],
                "units": [
                    {"id": 1, "kind": "UNIT_WARRIOR", "x": 9, "y": 4},
                    {"id": 2, "kind": "UNIT_GREAT_WRITER", "x": 8, "y": 4},
                ],
            },
        ]

        report = civ6_watchdogs.idle_stack(events)

        self.assertEqual(report["unit_turns"], 1)
        self.assertEqual(report["stuck_unit_turns"], 0)
        self.assertEqual(report["worst_stack"], 1)
        self.assertEqual(report["units_seen"], 1)
        self.assertEqual(report["bridge_managed_great_person_observations"], 2)

    def test_unique_great_person_uses_the_same_bridge_managed_classification(self) -> None:
        self.assertTrue(civ6_watchdogs.is_bridge_managed_great_person({
            "kind": "UNIT_COMANDANTE_GENERAL"
        }))
        self.assertFalse(civ6_watchdogs.is_bridge_managed_great_person({
            "kind": "UNIT_WARRIOR"
        }))


class ReachVerdictTest(unittest.TestCase):
    @staticmethod
    def report(first: int, last: int) -> dict:
        return {
            "idle_stack": {
                "reach": {
                    "furthest_ever": 7,
                    "furthest_ever_turn": first,
                    "last_turn": last,
                    "observed_turn_span": last - first,
                }
            }
        }

    def test_late_loaded_replay_is_not_mistaken_for_a_whole_game(self) -> None:
        verdicts = civ6_watchdogs.verdicts(self.report(89, 96), 0.35, 0.98)

        self.assertFalse(any("EMPIRE NEVER REACHED" in verdict for verdict in verdicts))

    def test_long_observation_still_detects_an_empire_that_never_reached(self) -> None:
        verdicts = civ6_watchdogs.verdicts(self.report(1, 60), 0.35, 0.98)

        self.assertTrue(any("EMPIRE NEVER REACHED" in verdict for verdict in verdicts))


class InfrastructureMirrorTest(unittest.TestCase):
    def test_latest_state_event_cuts_the_stream_at_the_selected_frame(self) -> None:
        events = [
            {"kind": "tiles", "turn": 10, "plots": [{"x": 1, "y": 1}]},
            {"kind": "state", "turn": 10},
            {"kind": "tiles", "turn": 11, "plots": [{"x": 2, "y": 2}]},
            {"kind": "state", "turn": 11},
        ]

        index, state = civ6_watchdogs.latest_state_event(events)

        self.assertEqual(index, 3)
        self.assertEqual(state["turn"], 11)
        self.assertEqual(
            civ6_watchdogs.latest_tiles(events[:index + 1]),
            {(1, 1): events[0]["plots"][0], (2, 2): events[2]["plots"][0]},
        )

    def test_latest_state_event_can_select_an_older_same_turn_frame(self) -> None:
        events = [
            {"kind": "state", "turn": 40, "frame": 0},
            {"kind": "tiles", "turn": 40, "plots": [{"x": 4, "y": 5}]},
            {"kind": "state", "turn": 40, "frame": 1},
        ]

        index, state = civ6_watchdogs.latest_state_event(events, turn=40)

        self.assertEqual(index, 2)
        self.assertEqual(state["frame"], 1)

    def test_audit_resolves_truncated_district_and_firaxis_wonder_names(self) -> None:
        self.assertEqual(
            civ6_watchdogs.model_infrastructure_name(
                "DISTRICT_GOVERNMENT", "DISTRICT_",
                civ6_watchdogs.MODELLED_DISTRICTS,
            ),
            "government_plaza",
        )
        self.assertEqual(
            civ6_watchdogs.model_infrastructure_name(
                "BUILDING_STATUE_LIBERTY", "BUILDING_",
                civ6_watchdogs.MODELLED_WONDERS,
            ),
            "statue_of_liberty",
        )
        self.assertEqual(
            civ6_watchdogs.model_infrastructure_name(
                "DISTRICT_WATER_ENTERTAINMENT_COMPLEX", "DISTRICT_",
                civ6_watchdogs.MODELLED_DISTRICTS,
            ),
            "water_park",
        )
        self.assertEqual(
            civ6_watchdogs.model_infrastructure_name(
                "DISTRICT_WATER_STREET_CARNIVAL", "DISTRICT_",
                civ6_watchdogs.MODELLED_DISTRICTS,
            ),
            "copacabana",
        )

    def test_city_roster_distinguishes_completed_foundation_and_wonder_plots(self) -> None:
        events = [{
            "kind": "state",
            "turn": 40,
            "cities": [{
                "districts": [
                    {"type": "DISTRICT_CAMPUS", "x": 4, "y": 5,
                     "pillaged": True},
                    {"type": "DISTRICT_HOLY_SITE", "x": 5, "y": 5,
                     "pillaged": False, "complete": False},
                    {"type": "DISTRICT_CITY_CENTER", "x": 3, "y": 5},
                    {"type": "DISTRICT_WONDER", "x": 6, "y": 5},
                ],
                "wonders": [{"type": "BUILDING_PYRAMIDS", "x": 6, "y": 5}],
            }],
        }]

        expected = civ6_watchdogs.expected_infrastructure(events)

        self.assertEqual(expected[(4, 5)], {
            "district": "campus", "foundation": None,
            "wonder": None, "pillaged": True,
        })
        self.assertEqual(expected[(5, 5)], {
            "district": None, "foundation": "holy_site",
            "wonder": None, "pillaged": False,
        })
        self.assertEqual(expected[(6, 5)], {
            "district": None, "foundation": None,
            "wonder": "pyramids", "pillaged": False,
        })
        self.assertNotIn((3, 5), expected)

    def test_one_missing_district_is_loud_even_when_tile_agreement_exceeds_threshold(self) -> None:
        report = {
            "mirror": {
                "agree": 168,
                "compared": 168,
                "agree_fraction": 1.0,
                "missing_in_mirror": 0,
                "infrastructure_agree": 3,
                "infrastructure_compared": 4,
                "infrastructure_disagree_by_field": {"district": 1},
                "infrastructure_examples": {},
            }
        }

        verdicts = civ6_watchdogs.verdicts(report, 0.35, 0.98)

        self.assertTrue(any("INFRASTRUCTURE DISAGREES" in verdict for verdict in verdicts))

    def test_city_economy_compares_every_yield_assignment_progress_and_great_work(self) -> None:
        events = [{
            "kind": "state", "turn": 91, "science": 5, "culture": 4,
            "cities": [{
                "x": 62, "y": 16,
                "yields": {"food": 8, "production": 7, "gold": 6,
                           "science": 5, "culture": 4, "faith": 3},
                # The centre, one tile, and the Theater plot the specialist
                # staffs — Firaxis lists all three as worked.
                "worked": [{"x": 62, "y": 16}, {"x": 61, "y": 16}, {"x": 63, "y": 16}],
                "districts": [{"type": "DISTRICT_THEATER", "x": 63, "y": 16,
                               "complete": True}],
                "specialists": ["DISTRICT_THEATER"],
                "producing": "UNIT_WARRIOR",
                "production_progress": 12.5,
                "great_works": [{"object": "GREATWORKOBJECT_WRITING"}],
            }],
        }]
        dump = {
            "cities": [{
                "x": 62, "y": 16,
                "yields": {"food": 8, "production": 7, "gold": 6,
                           "science": 5, "culture": 4, "faith": 3},
                "model_yields": {"food": 7, "production": 7, "gold": 6,
                                 "science": 5, "culture": 2, "faith": 3},
                "worked": [{"x": 61, "y": 16}],
                "specialists": ["theater_square"],
                "production_progress": 12.5,
            }],
            "great_works": {"writing": 1},
            "empire_yields": {"science": 5, "culture": 4},
        }

        report = civ6_watchdogs.city_economy_agreement(events, dump)

        self.assertEqual(report["agree"], 1)
        self.assertEqual(report["great_work_agree"], 1)
        self.assertEqual(report["empire_agree"], 2)
        self.assertEqual(report["disagree_by_field"], {})
        self.assertEqual(report["max_model_yield_drift"], 2.0)

    def test_idle_production_sentinel_is_not_a_city_economy_mismatch(self) -> None:
        events = [{
            "kind": "state", "turn": 91,
            "cities": [{
                "x": 62, "y": 16,
                "yields": {"food": 2, "production": 1, "gold": 0,
                           "science": 0, "culture": 1, "faith": 0},
                "producing": None,
                "production_progress": -1,
            }],
        }]
        dump = {
            "cities": [{
                "x": 62, "y": 16,
                "yields": {"food": 2, "production": 1, "gold": 0,
                           "science": 0, "culture": 1, "faith": 0},
                "production_progress": 0.0,
            }],
        }

        report = civ6_watchdogs.city_economy_agreement(events, dump)

        self.assertEqual(report["agree"], 1)
        self.assertEqual(report["disagree_by_field"], {})

    def test_active_production_progress_still_has_to_match(self) -> None:
        events = [{
            "kind": "state", "turn": 91,
            "cities": [{
                "x": 62, "y": 16,
                "yields": {"food": 2, "production": 1, "gold": 0,
                           "science": 0, "culture": 1, "faith": 0},
                "producing": "UNIT_WARRIOR",
                "production_progress": 12.5,
            }],
        }]
        dump = {
            "cities": [{
                "x": 62, "y": 16,
                "yields": {"food": 2, "production": 1, "gold": 0,
                           "science": 0, "culture": 1, "faith": 0},
                "production_progress": 11.5,
            }],
        }

        report = civ6_watchdogs.city_economy_agreement(events, dump)

        self.assertEqual(report["agree"], 0)
        self.assertEqual(report["disagree_by_field"], {"production_progress": 1})

    def test_city_economy_disagreement_is_a_loud_verdict(self) -> None:
        report = {
            "mirror": {
                "agree": 168, "compared": 168, "agree_fraction": 1.0,
                "missing_in_mirror": 0,
                "infrastructure_agree": 7, "infrastructure_compared": 7,
                "infrastructure_disagree_by_field": {},
                "city_economy_agree": 1, "city_economy_compared": 2,
                "great_work_agree": 0, "great_work_compared": 1,
                "city_economy_disagree_by_field": {
                    "yield_culture": 1, "great_works": 1,
                },
                "city_economy_examples": {},
            }
        }

        verdicts = civ6_watchdogs.verdicts(report, 0.35, 0.98)

        self.assertTrue(any("CITY ECONOMY DISAGREES" in verdict for verdict in verdicts))

    def test_governor_agreement_compares_roster_assignment_promotions_and_titles(self) -> None:
        events = [{
            "kind": "state", "turn": 92,
            "governor_points": 4, "governor_points_spent": 4,
            "governors": [{
                "type": "GOVERNOR_THE_DEFENDER", "x": 62, "y": 16,
                "established": True, "neutralized_turns": 0,
                "promotions": [
                    "GOVERNOR_PROMOTION_REDOUBT",
                    "GOVERNOR_PROMOTION_GARRISON_COMMANDER",
                    "GOVERNOR_PROMOTION_DEFENSE_LOGISTICS",
                ],
            }],
        }]
        dump = {
            "governor_points": 4,
            "governor_points_spent": 4,
            "governor_points_available": 0,
            "governors": [{
                "type": "GOVERNOR_THE_DEFENDER", "x": 62, "y": 16,
                "established": True, "neutralized": False,
                "promotions": [
                    "GOVERNOR_PROMOTION_DEFENSE_LOGISTICS",
                    "GOVERNOR_PROMOTION_GARRISON_COMMANDER",
                ],
            }],
        }

        report = civ6_watchdogs.governor_agreement(events, dump)

        self.assertEqual(report["compared"], 4)
        self.assertEqual(report["agree"], 4)
        self.assertEqual(report["disagree_by_field"], {})

    def test_governor_disagreement_is_a_loud_verdict(self) -> None:
        report = {
            "mirror": {
                "agree": 168, "compared": 168, "agree_fraction": 1.0,
                "missing_in_mirror": 0,
                "infrastructure_disagree_by_field": {},
                "city_economy_disagree_by_field": {},
                "governor_agree": 3, "governor_compared": 5,
                "governor_disagree_by_field": {
                    "promotions": 1, "titles_spent": 1,
                },
                "governor_examples": {},
            }
        }

        verdicts = civ6_watchdogs.verdicts(report, 0.35, 0.98)

        self.assertTrue(any("GOVERNORS DISAGREE" in verdict for verdict in verdicts))


class TerrainMirrorTest(unittest.TestCase):
    def test_expected_matches_mirror_aliases_and_national_park_flag(self) -> None:
        vocab = civ6_watchdogs.load_vocab()
        common = {"t": "TERRAIN_GRASSLAND", "w": False, "o": 0}

        beach_resort = civ6_watchdogs.expected({
            **common,
            "im": "IMPROVEMENT_BEACH_RESORT",
        }, vocab)
        self.assertEqual(beach_resort["im"], "seaside_resort")

        national_park = civ6_watchdogs.expected({
            **common,
            "np": True,
        }, vocab)
        self.assertEqual(national_park["im"], "national_park")


if __name__ == "__main__":
    unittest.main()
