"""Hermetic tests for the yield-fidelity instrument.

No binary, no run directory: these pin the pure comparison and episode logic
on synthetic host/model records, the way the instrument reads them.
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_yield_drift as drift  # noqa: E402


def yields(**values):
    base = {k: 0.0 for k in drift.YIELDS}
    base.update(values)
    return base


class YieldDeltaTest(unittest.TestCase):
    def test_delta_is_model_minus_host_per_yield(self):
        delta = drift.yield_delta(yields(food=5, gold=2.5), yields(food=6, gold=2.5, faith=1))
        self.assertEqual(delta["food"], 1.0)
        self.assertEqual(delta["gold"], 0.0)
        self.assertEqual(delta["faith"], 1.0)
        self.assertEqual(delta["science"], 0.0)

    def test_single_precision_host_noise_rounds_away(self):
        # The mod prints single-precision floats (5.85156 for 6.5 * 0.9).
        delta = drift.yield_delta(yields(science=5.85156), yields(science=5.85))
        self.assertEqual(delta["science"], 0.0)


class CityComparisonTest(unittest.TestCase):
    def test_cities_pair_by_host_coordinates_and_skip_exports_without_yields(self):
        state = {"cities": [
            {"name": "Rome", "x": 5, "y": 4, "yields": yields(food=10), "housing": 8,
             "amenities": 3},
            {"name": "Antium", "x": 9, "y": 9},  # older mod: no yields
        ]}
        dump = {"cities": [
            {"name": "Rome", "x": 5, "y": 4, "model_yields": yields(food=12),
             "model_housing": 9.5, "model_amenities": 5},
            {"name": "Antium", "x": 9, "y": 9, "model_yields": yields(food=1)},
        ]}
        records = drift.city_comparisons(state, dump)
        self.assertEqual([r["name"] for r in records], ["Rome"])
        self.assertEqual(records[0]["delta"]["food"], 2.0)
        self.assertEqual(records[0]["housing_delta"], 1.5)
        self.assertEqual(records[0]["amenities_delta"], 2)

    def test_the_mods_could_not_read_sentinel_is_not_a_housing_claim(self):
        state = {"cities": [{"name": "Rome", "x": 5, "y": 4, "yields": yields(),
                             "housing": -1, "amenities": -1}]}
        dump = {"cities": [{"name": "Rome", "x": 5, "y": 4, "model_yields": yields(),
                            "model_housing": 4.0, "model_amenities": 2}]}
        record = drift.city_comparisons(state, dump)[0]
        self.assertNotIn("housing_delta", record)
        self.assertNotIn("amenities_delta", record)


class TileDiffTest(unittest.TestCase):
    def test_disagreeing_worked_plots_are_named_with_the_mirrors_plot(self):
        host = {"x": 5, "y": 4, "worked": [
            {"x": 5, "y": 4, "yields": yields(food=3)},            # centre: skipped
            {"x": 6, "y": 4, "yields": yields(food=4, production=1)},
            {"x": 7, "y": 4, "yields": yields(food=2)},
            {"x": 5, "y": 5, "yields": yields()},                   # district plot: no model tile
        ]}
        model = {"ledger": {"tiles": [
            {"x": 6, "y": 4, "yields": yields(food=2)},
            {"x": 7, "y": 4, "yields": yields(food=2)},
        ]}}
        plots = {(6, 4): {"t": "grassland", "f": "volcanic_soil"}}
        diffs = drift.tile_diffs(host, model, plots)
        self.assertEqual(len(diffs), 1)
        self.assertEqual((diffs[0]["x"], diffs[0]["y"]), (6, 4))
        self.assertEqual(diffs[0]["delta"]["food"], -2.0)
        self.assertEqual(diffs[0]["delta"]["production"], -1.0)
        self.assertEqual(diffs[0]["plot"]["f"], "volcanic_soil")

    def test_an_export_without_plot_yields_reports_nothing(self):
        host = {"x": 5, "y": 4, "worked": [{"x": 6, "y": 4}]}
        model = {"ledger": {"tiles": [{"x": 6, "y": 4, "yields": yields(food=2)}]}}
        self.assertEqual(drift.tile_diffs(host, model), [])


class EpisodeTest(unittest.TestCase):
    def test_runs_split_on_a_change_of_delta_and_on_gaps_in_turns(self):
        series = {("Rome", "gold"): {10: 1.0, 11: 1.0, 12: 1.0, 13: 2.0, 14: 2.0,
                                     16: 2.0, 17: 0.0, 18: -0.9, 19: -0.9, 20: -0.9}}
        found = drift.episodes(series, min_len=3)
        spans = [(e["start"], e["end"], e["delta"], e["persistent"]) for e in found]
        self.assertEqual(spans, [
            (10, 12, 1.0, True),
            (13, 14, 2.0, False),
            (16, 16, 2.0, False),
            (18, 20, -0.9, True),
        ])

    def test_zero_deltas_never_form_an_episode(self):
        series = {("Rome", "food"): {1: 0.0, 2: 0.0, 3: 0.01}}
        self.assertEqual(drift.episodes(series), [])


class StateChangeTest(unittest.TestCase):
    def test_names_city_and_empire_changes_between_two_states(self):
        before = {"policies": ["POLICY_A"], "techs": ["TECH_X"], "government": "G1",
                  "cities": [{"name": "Rome", "buildings": ["BUILDING_MONUMENT"],
                              "districts": [], "worked": [{"x": 1, "y": 1}], "pop": 3}]}
        after = {"policies": ["POLICY_B"], "techs": ["TECH_X", "TECH_Y"], "government": "G1",
                 "cities": [{"name": "Rome",
                             "buildings": ["BUILDING_MONUMENT", "BUILDING_GRANARY"],
                             "pillaged_buildings": ["BUILDING_MONUMENT"],
                             "districts": [], "worked": [{"x": 1, "y": 2}], "pop": 4}]}
        lines = drift.state_changes(before, after, "Rome")
        joined = "\n".join(lines)
        self.assertIn("city.pop: 3 -> 4", joined)
        self.assertIn("BUILDING_GRANARY", joined)
        self.assertIn("city.pillaged_buildings", joined)
        self.assertIn("worked: -[(1, 1)] +[(1, 2)]", joined)
        self.assertIn("state.policies: -['POLICY_A'] +['POLICY_B']", joined)
        self.assertIn("state.techs: -[] +['TECH_Y']", joined)
        self.assertNotIn("government", joined)

    def test_first_turn_has_nothing_to_compare(self):
        self.assertEqual(drift.state_changes(None, {}, "Rome"), ["(first turn compared)"])


class YieldSourceParserTest(unittest.TestCase):
    def test_amount_and_label_rows_survive_and_headers_drop(self):
        text = "+12.4 Science per turn\n\n+4.0 from Districts\n+3 from Buildings\n" \
               "+3.5 from Citizens\n-10% from Amenities\nSomething without a number"
        rows = drift.parse_yield_sources(text)
        self.assertEqual(rows, [
            (12.4, "Science per turn"),
            (4.0, "from Districts"),
            (3.0, "from Buildings"),
            (3.5, "from Citizens"),
            (-10.0, "from Amenities"),
        ])

    def test_empty_or_missing_text_is_no_rows(self):
        self.assertEqual(drift.parse_yield_sources(""), [])
        self.assertEqual(drift.parse_yield_sources(None), [])


if __name__ == "__main__":
    unittest.main()
