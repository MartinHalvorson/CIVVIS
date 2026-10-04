import json
import unittest

from civ6_production_report import report


class ProductionReportTests(unittest.TestCase):
    def state(self, turn, production=10):
        return {"kind": "state", "turn": turn, "cities": [{
            "x": 1, "y": 1, "yields": {"production": production},
            "worked": [{"x": 1, "y": 1}, {"x": 2, "y": 1, "yields": {"production": 2}}, {"x": 3, "y": 1}],
        }], "rivals": [{"player": 1, "public_stats": {"production": 40, "city_count": 4}, "cities": [{"pop": 2}]}],
            "units": [{"kind": "UNIT_BUILDER", "build_charges": 3}]}

    def test_first_state_and_public_totals_with_turn_gaps(self):
        events = [self.state(1), self.state(1, 999), self.state(3, 20)]
        result = report(map(json.dumps, events), {1, 2, 3})
        row = result["checkpoints"][-1]
        self.assertEqual(row["production"], 20)
        self.assertEqual(row["observed_production_rate_sum"], 30)
        self.assertEqual(row["missing_turn_count"], 1)
        self.assertEqual(row["ratio_to_best_observed_rival"], .5)
        self.assertEqual(row["rivals"][0]["production_per_city"], 10)
        self.assertEqual(result["missing_checkpoints"], [2])
        self.assertEqual(row["builder_charges"], 3)

    def test_plot_chunks_are_as_of_the_state_and_unknown_is_not_bare(self):
        events = [{"kind": "tiles", "plots": [{"x": 2, "y": 1}]}, self.state(1),
                  {"kind": "tiles", "plots": [{"x": 2, "y": 1, "im": "IMPROVEMENT_MINE"}]}, self.state(2)]
        rows = report(map(json.dumps, events), {1, 2})["checkpoints"]
        self.assertEqual(rows[0]["worked_unimproved_productive_tiles"], 1)
        self.assertEqual(rows[1]["worked_unimproved_tiles"], 0)
        self.assertEqual(rows[1]["worked_noncenter_tiles"], 2)
        self.assertEqual(rows[1]["worked_tiles_with_plot_records"], 1)

    def test_unknown_rival_yields_and_incomplete_live_line(self):
        state = self.state(1)
        state["rivals"] = [{"player": 1, "cities": [{"pop": 30}]}]
        result = report([json.dumps(state), '{"kind":'], {1})
        self.assertIsNone(result["checkpoints"][0]["ratio_to_best_observed_rival"])
        self.assertIsNone(result["checkpoints"][0]["rivals"][0]["production"])


if __name__ == "__main__":
    unittest.main()
