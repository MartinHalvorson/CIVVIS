import unittest
from civ6_transfer_calibration import finite, quantiles, summarize


class CalibrationTests(unittest.TestCase):
    def test_missing_values_are_not_zero(self):
        self.assertEqual(quantiles([]), {"n": 0})
        for missing in (None, -1, float("nan"), float("inf"), True):
            self.assertFalse(finite(missing))
        self.assertTrue(finite(0))

    def test_percentiles_include_tails(self):
        self.assertEqual(quantiles([0, 10, 20]), {"n": 3, "p10": 2., "median": 10., "p90": 18.})

    def test_difficulty_is_not_fitted_away(self):
        rows = [{"cohort": ("online", difficulty, 100), "source": source,
                 "run": source, "seat": 0, "target": "science", "values": {"techs": 40}}
                for source, difficulty in (("native", "prince"), ("live", "emperor"))]
        report = summarize(rows)
        self.assertEqual(len(report["cohorts"]), 2)
        self.assertTrue(all(c["status"] == "unmatched_cohort" for c in report["cohorts"]))
        self.assertTrue(all(c["metrics"]["techs"]["median_gap"] is None for c in report["cohorts"]))

    def test_all_six_seats_contribute_not_only_the_winner(self):
        rows = [{"cohort": ("online", "prince", 100), "source": "native", "run": "game",
                 "seat": seat, "target": "culture", "values": {"techs": seat}} for seat in range(6)]
        cohort = summarize(rows)["cohorts"][0]
        self.assertEqual(cohort["metrics"]["techs"]["native"]["n"], 6)
        self.assertEqual(cohort["native_games"], 1)


if __name__ == "__main__": unittest.main()
