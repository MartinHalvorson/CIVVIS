import json
from pathlib import Path
import tempfile
import unittest

from gene_report_targets import analyze


class TargetReportTests(unittest.TestCase):
    def report(self, rows, gene="victory-portfolio"):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "rows.jsonl"
            records = [{"kind": "header", "genes": ["victory-portfolio"]}, *rows]
            path.write_text("".join(json.dumps(row) + "\n" for row in records))
            return analyze([path], gene)

    def row(self, seed, seat, on, win, target="civvis", **extra):
        return dict(kind="game", seed=seed, seat=seat, genome=str(int(on)),
                    win=win, player_target=target, turn=200, victory="science", **extra)

    def test_adaptive_and_fixed_contracts_are_separate(self):
        rows = [self.row(1, 0, True, True), self.row(1, 1, False, False),
                self.row(2, 0, True, False, "science"), self.row(2, 1, False, True, "science")]
        result = self.report(rows)
        self.assertEqual(result["targets"]["civvis"]["gene_contrast"]["win_difference_pp"], 100)
        self.assertEqual(result["targets"]["science"]["gene_contrast"]["win_difference_pp"], -100)

    def test_missing_targets_are_not_silently_adaptive(self):
        result = self.report([self.row(1, 0, True, True, "")])
        self.assertIn("unknown", result["targets"])
        self.assertNotIn("civvis", result["targets"])

    def test_shared_winners_use_game_clusters(self):
        rows = [self.row(1, 0, True, True), self.row(1, 1, False, False),
                self.row(2, 0, True, False), self.row(2, 1, False, True)]
        contrast = self.report(rows)["targets"]["civvis"]["gene_contrast"]
        self.assertEqual(contrast["games"], 2)
        self.assertAlmostEqual(contrast["cluster_se_pp"], 100)

    def test_single_game_and_missing_arm_have_no_precision_claim(self):
        result = self.report([self.row(1, 0, True, True)])
        contrast = result["targets"]["civvis"]["gene_contrast"]
        self.assertIsNone(contrast["win_difference_pp"])
        self.assertIsNone(contrast["cluster_se_pp"])

    def test_constant_outcomes_do_not_claim_zero_uncertainty(self):
        rows = [self.row(seed, seat, seat == 0, False)
                for seed in (1, 2) for seat in (0, 1)]
        contrast = self.report(rows)["targets"]["civvis"]["gene_contrast"]
        self.assertEqual(contrast["win_difference_pp"], 0)
        self.assertIsNone(contrast["interval_95_pp"])

    def test_missing_telemetry_is_counted_as_missing(self):
        diagnostics = self.report([self.row(1, 0, True, True)])["targets"]["civvis"]["diagnostics"]
        self.assertEqual(diagnostics["missing_seats"], 1)
        self.assertIsNone(diagnostics["primary_switches_mean"])

    def test_losses_are_censored_for_finish_error(self):
        telemetry = {"committed_turn": 100, "primary_switches": 2,
                     "trace": [{"turn": 100, "primary": "science", "expected_finish": 180}]}
        rows = [self.row(1, 0, True, True, victory_portfolio=telemetry),
                self.row(1, 1, False, False, victory_portfolio=telemetry)]
        diagnostics = self.report(rows)["targets"]["civvis"]["diagnostics"]
        self.assertEqual(diagnostics["realized_same_lane_forecasts"], 1)
        self.assertEqual(diagnostics["realized_finish_error_turns_mean"], 20)

    def test_allocation_uses_one_developed_snapshot_per_seat(self):
        def point(primary, growth):
            return dict(phase="finish", production_allocation=dict(
                primary=primary, secondary=0, other=growth, idle=0,
                settlers_and_builders=growth))
        rows = [self.row(1, 0, True, False, victory_portfolio={
                    "trace": [point(0, 10), point(8, 2)]}),
                self.row(2, 0, True, False, victory_portfolio={
                    "trace": [point(4, 6)]})]
        diagnostics = self.report(rows)["targets"]["civvis"]["diagnostics"]
        self.assertAlmostEqual(diagnostics["last_developed_allocation_shares"]["primary"], 0.6)
        self.assertAlmostEqual(diagnostics["last_developed_allocation_shares"]["settlers_and_builders"], 0.4)

    def test_duplicate_seats_are_rejected(self):
        row = self.row(1, 0, True, True)
        with self.assertRaisesRegex(ValueError, "duplicate seat"):
            self.report([row, row])

    def test_absent_gene_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "not in the active header"):
            self.report([self.row(1, 0, True, True)], "absent")

    def test_empty_input_is_an_error(self):
        with self.assertRaisesRegex(ValueError, "no measured"):
            self.report([])


if __name__ == "__main__":
    unittest.main()
