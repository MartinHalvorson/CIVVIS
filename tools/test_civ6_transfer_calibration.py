import unittest
import json
import tempfile
from pathlib import Path
from civ6_transfer_calibration import finite, native_samples, quantiles, summarize


class CalibrationTests(unittest.TestCase):
    def test_different_information_contracts_cannot_be_pooled(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / "epochs.jsonl"
            file.write_text('\n'.join(json.dumps({"kind": "header", "player_contract": contract})
                                      for contract in ("legacy", "observed-player-v1")))
            with self.assertRaisesRegex(ValueError, "mix player contracts"):
                list(native_samples([file]))

    def test_repeated_inputs_deduplicate_without_erasing_other_difficulties(self):
        with tempfile.TemporaryDirectory() as directory:
            files = []
            for difficulty in ("prince", "emperor"):
                file = Path(directory) / f"{difficulty}.jsonl"
                header = {"kind": "header", "speed": "online", "difficulty": difficulty,
                          "build": {"binary_sha256": "same-build"}}
                rows = [header, {"kind": "game", "seed": 1, "seat": 0,
                                 "player_target": "science", "trajectory": [{"turn": 25, "cities": 3}]}]
                file.write_text("".join(json.dumps(row) + "\n" for row in rows))
                files.append(file)
            samples = list(native_samples(files + files))
        self.assertEqual(len(samples), 2)
        self.assertEqual({s["cohort"][1] for s in samples}, {"prince", "emperor"})
        self.assertEqual(len({s["run"] for s in samples}), 2)

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
