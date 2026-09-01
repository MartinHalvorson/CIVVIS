#!/usr/bin/env python3
"""The live-repair census reads a recorded run the way the live harness did."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import live_repair_census as census  # noqa: E402


def _state(turn: int, **fields) -> dict:
    return {"kind": "state", "ctx": "agent", "turn": turn, **fields}


def _person(**fields) -> dict:
    """One `great_person` block as the control mod exports it."""
    base = {"can_activate": False, "charges": 0, "activation_plots": []}
    return {**base, **fields}


def _plots(*open_flags) -> list[dict]:
    return [{"x": 1, "y": 2, "distance": 1, "slot_open": flag}
            for flag in open_flags]


class TheSlotPredicateMatchesTheRustItTranscribes(unittest.TestCase):
    """★ ONE TABLE, NOT TWO.

    `starved_after` transcribes `StateGreatPerson::slot_starved`
    (`src/mirror.rs`). The cases below are the same ones the Rust unit test
    `the_rome_stack_is_starved_even_though_the_empire_owns_empty_slots`
    (`src/bin/civvis_orders.rs`) asserts, reproduced field for field from live
    run `civvis-20260822T020434Z` — including the three the predicate must NOT
    change. If either side moves, both fail together and get re-read.
    """

    def test_the_rome_stack_is_starved(self):
        writer = _person(empty_slots=24, class_="GREAT_PERSON_CLASS_WRITER",
                         activation_plots=_plots(False, False, False))
        musician = _person(empty_slots=4, activation_plots=_plots(False, False))
        artist = _person(empty_slots=2, activation_plots=_plots(False))
        for label, person in (("writer", writer), ("musician", musician),
                              ("artist", artist)):
            with self.subTest(label):
                self.assertTrue(census.starved_after(person))
                self.assertFalse(
                    census.starved_before(person),
                    "24/4/2 empire-wide slots is exactly why the old gate stayed shut")

    def test_no_plot_offered_is_a_missing_district_not_a_missing_slot(self):
        scientist = _person(charges=1, required_district="DISTRICT_SPACEPORT")
        self.assertFalse(census.starved_after(scientist))

    def test_a_person_the_host_will_activate_is_never_starved(self):
        person = _person(can_activate=True, empty_slots=0,
                         activation_plots=_plots(False))
        self.assertFalse(census.starved_after(person))
        self.assertFalse(census.starved_before(person))

    def test_one_reachable_slot_is_not_starved(self):
        person = _person(empty_slots=24, activation_plots=_plots(False, True, False))
        self.assertFalse(census.starved_after(person))

    def test_an_older_mod_keeps_its_benefit_of_the_doubt(self):
        person = _person(empty_slots=24, activation_plots=_plots(None, None))
        self.assertFalse(census.starved_after(person))

    def test_zero_empire_wide_slots_is_still_sufficient(self):
        person = _person(empty_slots=0)
        self.assertTrue(census.starved_after(person))
        self.assertTrue(census.starved_before(person))


class TheGreatPersonSectionCountsWhatChanged(unittest.TestCase):
    def test_a_flip_is_counted_once_per_observation(self):
        person = _person(empty_slots=24, class_="x",
                         activation_plots=_plots(False))
        person["class"] = "GREAT_PERSON_CLASS_WRITER"
        records = [
            _state(t, units=[{"kind": "UNIT_GREAT_WRITER", "id": 7,
                              "great_person": person}])
            for t in (10, 11, 12)
        ]
        reading = census.great_person_reading(records)
        self.assertEqual(reading["gp_unit_frames"], 3)
        self.assertEqual(reading["starved_before"], 0)
        self.assertEqual(reading["starved_after"], 3)
        self.assertEqual(reading["flips"], 3)
        self.assertEqual(reading["distinct_people_flipped"], 1)
        self.assertEqual(reading["cultural_flips"], 3)
        self.assertTrue(reading["exports_slot_open"])

    def test_an_older_mod_is_reported_as_such_and_flips_nothing(self):
        person = _person(activation_plots=[{"x": 1, "y": 2, "distance": 1}])
        records = [_state(5, units=[{"kind": "UNIT_GREAT_WRITER", "id": 7,
                                     "great_person": person}])]
        reading = census.great_person_reading(records)
        self.assertFalse(reading["exports_slot_open"])
        self.assertFalse(reading["exports_empty_slots"])
        self.assertEqual(reading["flips"], 0)

    def test_the_mod_idle_counter_is_carried_through(self):
        records = [{"kind": "orders", "gp_idle": n} for n in (0, 3, 11, 8)]
        reading = census.great_person_reading(records)
        self.assertEqual(reading["gp_idle_peak"], 11)
        self.assertEqual(reading["gp_idle_final"], 8)


class TheTradeLedgerSeparatesOneRefusalFromTwo(unittest.TestCase):
    def test_a_pairing_refused_once_is_the_one_corroboration_hands_back(self):
        records = [
            {"kind": "trade_route_refused", "from_x": 1, "from_y": 2, "x": 3, "y": 4},
            {"kind": "trade_route_refused", "from_x": 5, "from_y": 6, "x": 7, "y": 8},
            {"kind": "trade_route_refused", "from_x": 5, "from_y": 6, "x": 7, "y": 8},
            _state(9, trade_capacity=6, trade_routes=[1, 2],
                   units=[{"kind": "UNIT_TRADER"}, {"kind": "UNIT_TRADER"}]),
        ]
        reading = census.trade_reading(records)
        self.assertEqual(reading["refusals"], 3)
        self.assertEqual(reading["distinct_pairings"], 2)
        self.assertEqual(reading["pairings_refused_once"], 1)
        self.assertEqual(reading["pairings_refused_twice_or_more"], 1)
        self.assertEqual(reading["final_idle_capacity"], 4)
        self.assertEqual(reading["final_traders_alive"], 2)


class TheRestartSectionRunsTheHarnessOwnFunction(unittest.TestCase):
    """⚠ Not a transcription. `below_leader_score_reading` — the one remaining
    early stop — is imported from `tools/civ6_play.py` and fed the
    recorded stream in file order, which is what `_play`'s `finished()` does
    with the live one."""

    def _behind(self, turn: int) -> list[dict]:
        return [
            _state(turn, science=10, culture=5,
                   rivals=[{"science": 100, "culture": 90}]),
            {"kind": "turn", "ctx": "agent", "turn": turn,
             "score": 100, "rival_best": 1000},
        ]

    def test_it_fires_on_the_first_qualifying_reading(self):
        floor = census.civ6_play.LEADER_SCORE_MIN_TURN
        self.assertEqual(census.RESTART_RATIO,
                         census.civ6_play.DEFAULT_LEADER_SCORE_RATIO)
        self.assertFalse(census.restart_reading(
            self._behind(floor - 1), census.RESTART_RATIO, {})["fired"])
        verdict = census.restart_reading(self._behind(floor),
                                         census.RESTART_RATIO, {})
        self.assertTrue(verdict["fired"])
        self.assertEqual(verdict["fire_turn"], floor)

    def test_a_disabled_ratio_never_fires(self):
        records = [r for t in range(100, 130) for r in self._behind(t)]
        self.assertFalse(census.restart_reading(records, 0.0, {})["fired"])

    def test_an_explicit_science_summary_is_not_stopped_for_a_score_gap(self):
        floor = census.civ6_play.LEADER_SCORE_MIN_TURN
        reading = census.restart_reading(
            self._behind(floor), census.RESTART_RATIO,
            {"victory_target": "science"},
        )
        self.assertFalse(reading["score_stop_allowed"])
        self.assertFalse(reading["fired"])

    def test_the_outcome_comes_from_the_summary_not_the_stream(self):
        records = [r for t in range(100, 110) for r in self._behind(t)]
        rival_won = {"outcome": {"kind": "victory", "won": False},
                     "last_score": 400, "rival_best": 1000}
        reading = census.restart_reading(records, census.RESTART_RATIO, rival_won)
        self.assertFalse(reading["won"], "a rival's victory record is not our win")
        self.assertEqual(reading["final_score_ratio"], 0.4)
        ours = {"outcome": {"kind": "victory", "won": True}}
        self.assertTrue(census.restart_reading(
            records, census.RESTART_RATIO, ours)["won"])


class TheSettlerSectionSeparatesFoundedFromIdle(unittest.TestCase):
    def test_a_settler_that_never_founds_is_measured_by_how_long_it_stood(self):
        records = [
            _state(t, units=[{"kind": "UNIT_SETTLER", "id": 1},
                             {"kind": "UNIT_SETTLER", "id": 2}],
                   cities=[{"producing": "UNIT_SETTLER"}])
            for t in range(10, 41)
        ]
        records.append({"kind": "found", "unit": 1, "turn": 41})
        records.append({"kind": "found_refused", "turn": 20})
        reading = census.settler_reading(records)
        self.assertEqual(reading["settlers_seen"], 2)
        self.assertEqual(reading["settlers_that_founded"], 1)
        self.assertEqual(reading["settlers_that_never_founded"], 1)
        self.assertEqual(reading["never_founded_max_turns_alive"], 30)
        self.assertEqual(reading["found_refused"], 1)
        self.assertEqual(reading["settler_city_turns"], 31)


class TheCoordinateRoundTripIsTheOneTheJournalPrints(unittest.TestCase):
    """⚠ Civ 6 speaks odd-r OFFSET, CIVVIS stores AXIAL, and mixing them is
    silent. These four pairs are copied out of a real `why.log`, where the
    decider prints both sides of the conversion in one bracket."""

    def test_it_reproduces_the_journal_pairs(self):
        for offset, axial in (((56, 11), (51, 11)), ((20, 33), (4, 33)),
                              ((10, 11), (5, 11)), ((15, 6), (12, 6))):
            with self.subTest(offset):
                self.assertEqual(census.offset_to_axial(*offset), axial)

    def test_hex_distance_is_hex_distance(self):
        self.assertEqual(census.hex_distance((0, 0), (0, 0)), 0)
        self.assertEqual(census.hex_distance((0, 0), (3, 0)), 3)
        self.assertEqual(census.hex_distance((0, 0), (-1, 1)), 1)
        self.assertEqual(census.hex_distance((0, 0), (2, -1)), 2)


class TheVetoSectionTellsFogFromLoyalty(unittest.TestCase):
    JOURNAL = (
        "[why] t100 Expansion/Detail Settler refuses (51, 11) before walking there"
        " | it lies beyond the empire's Loyalty reach on ground the seat has not"
        " explored, where the rivals that press it have never been seen; the site"
        " is retired and the next best is asked  [civ6 (56,11) = axial (51,11)]\n"
        "[why] t101 Expansion/Detail Settler refuses (6, 28) before walking there"
        " | it lies beside a rival's border whose city the seat has never seen —"
        " that city presses the site from the fog; the site is retired  "
        "[civ6 (20,28) = axial (6,28)]\n"
        "[why] t102 Expansion/Detail Settler refuses (4, 33) before walking there"
        " | the city would lose 22.0 Loyalty a turn beside its neighbours and"
        " revolt in about 5 turns; the site is retired  [civ6 (20,33) = axial (4,33)]\n"
        "[why] t103 Cities/Decision Something else entirely | not a veto at all\n"
    )

    def test_it_classifies_and_counts_distinct_sites(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = Path(tmp) / "civvis-20260819T000000Z"
            run.mkdir()
            (run / "why.log").write_text(self.JOURNAL)
            (run / "events.jsonl").write_text("")
            reading = census.veto_reading(run / "events.jsonl", [], 3)
        self.assertTrue(reading["veto_log"])
        self.assertEqual(reading["unexplored"], 1)
        self.assertEqual(reading["unseen_rival_city"], 1)
        self.assertEqual(reading["loyalty_rate"], 1)
        self.assertEqual(reading["other"], 0)
        self.assertEqual(reading["distinct_sites"], 3)

    def test_a_rival_city_that_appears_after_the_veto_counts_as_taking_it(self):
        # Vetoed at axial (51, 11); a rival city later stands at civ6 (57, 12),
        # which is axial (51, 12) — one hex away.
        records = [_state(150, rivals=[{"cities": [{"x": 57, "y": 12}]}])]
        with tempfile.TemporaryDirectory() as tmp:
            run = Path(tmp) / "civvis-20260819T000000Z"
            run.mkdir()
            (run / "why.log").write_text(self.JOURNAL)
            (run / "events.jsonl").write_text("")
            near = census.veto_reading(run / "events.jsonl", records, 3)
            far = census.veto_reading(run / "events.jsonl", records, 0)
        self.assertEqual(near["sites_taken_by_a_rival"], 1)
        self.assertEqual(far["sites_taken_by_a_rival"], 0,
                         "a radius of zero must require the exact tile")

    def test_a_rival_city_already_there_before_the_veto_does_not_count(self):
        records = [_state(50, rivals=[{"cities": [{"x": 57, "y": 12}]}])]
        with tempfile.TemporaryDirectory() as tmp:
            run = Path(tmp) / "civvis-20260819T000000Z"
            run.mkdir()
            (run / "why.log").write_text(self.JOURNAL)
            (run / "events.jsonl").write_text("")
            reading = census.veto_reading(run / "events.jsonl", records, 3)
        self.assertEqual(reading["sites_taken_by_a_rival"], 0)

    def test_the_report_never_calls_the_vetoing_runs_the_journalled_ones(self):
        """★ TWO DENOMINATORS. 356 of the 560 recorded runs carry a decider
        journal and only 76 of them ever veto a site, so a per-run rate quoted
        against the wrong one is out by 4.7x. Both counts must appear, and the
        rate must say which it is over."""
        rows = [
            {"vetoes": {"veto_log": True, "total": 10, "unexplored": 8,
                        "unseen_rival_city": 1, "loyalty_rate": 1, "other": 0,
                        "distinct_sites": 4, "sites_taken_by_a_rival": 2}},
            {"vetoes": {"veto_log": True, "total": 0, "unexplored": 0,
                        "unseen_rival_city": 0, "loyalty_rate": 0, "other": 0,
                        "distinct_sites": 0, "sites_taken_by_a_rival": 0}},
            {"vetoes": {"veto_log": False, "total": 0, "unexplored": 0,
                        "unseen_rival_city": 0, "loyalty_rate": 0, "other": 0,
                        "distinct_sites": 0, "sites_taken_by_a_rival": 0}},
        ]
        report = "\n".join(census.report_vetoes(rows))
        self.assertIn("runs with a decider journal          2", report)
        self.assertIn("vetoing at least one site  1", report)
        self.assertIn("10 per VETOING run", report)
        self.assertIn("5 per journalled run", report)

    def test_a_run_with_no_journal_is_reported_as_having_none(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = Path(tmp) / "civvis-20260819T000000Z"
            run.mkdir()
            (run / "events.jsonl").write_text("")
            reading = census.veto_reading(run / "events.jsonl", [], 3)
        self.assertFalse(reading["veto_log"])
        self.assertEqual(reading["total"], 0)


class TheArmySectionDoesNotCallABuilderAnArmy(unittest.TestCase):
    def test_civilian_production_is_not_counted_as_military(self):
        records = [_state(
            120, military=200,
            rivals=[{"military": 100, "at_war": False}],
            units=[{"kind": "UNIT_ARCHER"}, {"kind": "UNIT_BUILDER"},
                   {"kind": "UNIT_GREAT_WRITER",
                    "great_person": _person(empty_slots=0)}],
            cities=[{"producing": "UNIT_BUILDER", "buildings": ["BUILDING_WALLS"]},
                    {"producing": "UNIT_ARCHER", "buildings": []},
                    {"producing": "BUILDING_CASTLE",
                     "buildings": ["BUILDING_WALLS", "BUILDING_CASTLE"]}])]
        reading = census.army_reading(records)
        self.assertEqual(reading["military_city_turns"], 1)
        self.assertEqual(reading["civilian_city_turns"], 1)
        self.assertEqual(reading["wall_city_builds"], 1)
        self.assertEqual(reading["held_walls"], 2)
        self.assertEqual(reading["held_castle"], 1)
        self.assertEqual(reading["mean_army_units"], 1.0,
                         "one Archer; a Builder and a Great Writer are not army")
        self.assertEqual(reading["mean_peace_army_ratio"], 2.0)
        self.assertEqual(reading["war_frames"], 0)

    def test_a_war_frame_is_never_mixed_into_the_peacetime_reading(self):
        records = [_state(120, military=200,
                          rivals=[{"military": 100, "at_war": True}],
                          units=[], cities=[])]
        reading = census.army_reading(records)
        self.assertEqual(reading["peace_frames"], 0)
        self.assertEqual(reading["war_frames"], 1)


class TheCorpusIsDiscoveredNotListed(unittest.TestCase):
    def test_the_window_filters_by_the_run_stamp(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus = Path(tmp)
            for stamp in ("20260801T000000Z", "20260819T000000Z",
                          "20260822T000000Z"):
                run = corpus / f"civvis-{stamp}"
                run.mkdir()
                (run / "events.jsonl").write_text("")
            (corpus / "not-a-run").mkdir()
            self.assertEqual(len(census.run_dirs(corpus, None, None)), 3)
            self.assertEqual(len(census.run_dirs(corpus, "20260819", None)), 2)
            self.assertEqual(len(census.run_dirs(corpus, None, "20260819")), 2)
            self.assertEqual(
                len(census.run_dirs(corpus, "20260819", "20260819")), 1)

    def test_a_truncated_last_record_does_not_lose_the_run(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "events.jsonl"
            path.write_text(json.dumps({"kind": "turn", "turn": 1}) + "\n"
                            + '{"kind": "sta')
            self.assertEqual(len(list(census.events(path))), 1)

    def test_a_missing_summary_is_an_absence_not_a_crash(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(census.summary(Path(tmp) / "events.jsonl"), {})


class TheQuantileNamesAnObservation(unittest.TestCase):
    def test_it_never_interpolates_a_value_nothing_produced(self):
        self.assertEqual(census._quantile([], 0.9), 0.0)
        self.assertEqual(census._quantile([5], 0.9), 5)
        self.assertEqual(census._quantile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 0.9), 9,
                         'nearest rank names the 9th of ten, never an 11th')
        self.assertEqual(census._quantile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 1.0), 10)
        self.assertEqual(census._quantile([1, 2, 3, 4, 5], 0.5), 3)


if __name__ == "__main__":
    unittest.main()
