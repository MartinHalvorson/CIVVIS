import unittest
import json
import tempfile
from pathlib import Path
from civ6_transfer_calibration import finite, live_samples, native_samples, quantiles, summarize


class CalibrationTests(unittest.TestCase):
    def comparison_rows(self):
        profile = {"map": "continents", "width": 74, "height": 46, "players": 6,
                   "city_states": 9, "ruleset": "RULESET_EXPANSION_2", "modes": [], "native_competitions": True}
        return [{"cohort": ("online", "emperor", 100), "source": source,
                 "run": source, "seat": 0, "target": "science", "model_build": "build",
                 "profile": dict(profile), "values": {"techs": value}}
                for source, value in (("live", 20), ("native", 30))]

    def test_only_complete_matching_profiles_can_report_a_pace_gap(self):
        rows = self.comparison_rows()
        cohort = summarize(rows)["cohorts"][0]
        self.assertEqual(cohort["status"], "comparable_pace")
        self.assertEqual(cohort["metrics"]["techs"]["median_gap"], 10)
        for key, mismatch in (("map", "pangaea"), ("width", 60), ("players", 4),
                              ("city_states", 0), ("ruleset", "RULESET_STANDARD"),
                              ("modes", ["monopolies"]), ("native_competitions", False)):
            with self.subTest(key=key):
                rows = self.comparison_rows()
                rows[1]["profile"][key] = mismatch
                cohorts = summarize(rows)["cohorts"]
                self.assertEqual(len(cohorts), 2)
                self.assertTrue(all(c["status"] == "unmatched_cohort" for c in cohorts))
                self.assertTrue(all(c["metrics"]["techs"]["median_gap"] is None for c in cohorts))

    def test_missing_profile_or_unidentified_build_is_not_comparable(self):
        rows = self.comparison_rows()
        for row in rows: row["profile"].pop("players")
        self.assertEqual(summarize(rows)["cohorts"][0]["status"], "incomplete_profile")
        rows = self.comparison_rows()
        rows[1].pop("model_build")
        self.assertEqual(summarize(rows)["cohorts"][0]["status"], "unidentified_or_mixed_native_builds")

    def test_duplicate_continuation_observations_cannot_inflate_a_cohort(self):
        rows = self.comparison_rows()
        self.assertEqual(summarize(rows), summarize(rows + rows))
        duplicate = dict(rows[0], values={"techs": 99})
        with self.assertRaisesRegex(ValueError, "conflicting observations"):
            summarize(rows + [duplicate])

    def test_mode_order_is_irrelevant_but_native_build_identity_is_not(self):
        rows = self.comparison_rows()
        rows[0]["profile"]["modes"] = ["a", "b"]
        rows[1]["profile"]["modes"] = ["b", "a"]
        self.assertEqual(len(summarize(rows)["cohorts"]), 1)
        rows.append(dict(rows[1], model_build="other-build"))
        cohort = summarize(rows)["cohorts"][0]
        self.assertEqual(cohort["status"], "unidentified_or_mixed_native_builds")
        self.assertIsNone(cohort["metrics"]["techs"]["median_gap"])

    def test_deliberate_action_probes_are_not_opponent_calibration_data(self):
        with tempfile.TemporaryDirectory() as directory:
            (Path(directory) / "summary.json").write_text(json.dumps(
                {"configured": True, "isolated_action_probes": True}))
            with self.assertRaisesRegex(ValueError, "diagnostic game"):
                list(live_samples(directory))

    def test_different_information_contracts_cannot_be_pooled(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / "epochs.jsonl"
            file.write_text('\n'.join(json.dumps({"kind": "header", "player_contract": contract})
                                      for contract in ("legacy", "observed-player-v1")))
            with self.assertRaisesRegex(ValueError, "mix player contracts"):
                list(native_samples([file]))

    def test_different_target_mixtures_cannot_supply_one_reference_distribution(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / "mixtures.jsonl"
            file.write_text('\n'.join(json.dumps({"kind": "header", "player_contract": "observed-player-v1", "target_mix": mix})
                                      for mix in ("science,culture", "science,science")))
            with self.assertRaisesRegex(ValueError, "target mixtures"):
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

    def test_eliminated_seats_are_not_zero_pace_live_opponents(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / "rows.jsonl"
            rows = [{"kind": "header"}, {"kind": "game", "seed": 1, "seat": 0,
                    "trajectory": [{"turn": 25, "alive": True, "cities": 1},
                                   {"turn": 50, "alive": False, "cities": 0}]}]
            file.write_text("\n".join(map(json.dumps, rows)))
            samples = list(native_samples([file]))
            self.assertEqual(len(samples), 1)
            self.assertEqual(samples[0]["cohort"][2], 25)

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
