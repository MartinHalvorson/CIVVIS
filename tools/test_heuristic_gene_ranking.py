"""The heuristic gene ranking is derived from the ledger's sources and must
not fall behind them."""
from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import heuristic_gene_ranking as ranking  # noqa: E402


class TheTableIsDerived(unittest.TestCase):
    EXPECTED_COLUMNS = (
        "| Rank | Gene | Description | Default | ± Wins Last 10k | ± Wins 10k Prior | "
        "Total (on) Win rate | Total (off) Win rate | Total Games (on+off) | "
        "cost (compute) | cost (time) |"
    )

    def test_the_checked_in_table_matches_the_ledgers_sources(self):
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        self.assertEqual(
            ranking.render(ledger),
            ranking.RANKING_MD.read_text(),
            "HEURISTIC_GENE_RANKING.md is stale: run tools/heuristic_gene_ranking.py --write",
        )

    def test_every_screenable_gene_is_visible(self):
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        native, _, _, _ = ranking.load_sources(ledger)
        text = ranking.RANKING_MD.read_text()
        for tag in ranking.screenable_tags():
            if tag in native:
                self.assertIn(f"`{tag}`", text, tag)
            else:
                self.assertIn("## Awaiting native measurement", text)
                self.assertIn(f"| `{tag}` | off (unmeasured) |", text, tag)
        self.assertNotIn("`step-and-reassess` | ", text, "a host-only flag is not ranked natively")

    def test_descriptions_come_from_the_toggle_docs(self):
        desc = ranking.descriptions()
        self.assertGreater(len(desc), 50)
        self.assertTrue(desc["recon-replacement"].startswith("Rebuild the recon arm"))
        self.assertTrue(desc["loyalty-rate-alarm"].startswith("Rank loyalty emergencies"))

    def test_operator_columns_are_in_the_requested_order(self):
        header = next(
            line for line in ranking.RANKING_MD.read_text().splitlines()
            if line.startswith("| Rank |")
        )
        self.assertEqual(header, self.EXPECTED_COLUMNS)

    def test_cost_uses_the_newest_real_measurement_and_never_invents_zero(self):
        history = [
            {"compute_cost_pct": 90.0, "compute_cost_se_pct": 4.0},
            {"compute_cost_pct": None, "compute_cost_se_pct": None},
            {"compute_cost_pct": 1.234, "compute_cost_se_pct": 0.456},
        ]
        self.assertEqual(
            ranking.cost_cell(history, "compute_cost_pct", "compute_cost_se_pct"),
            "+1.23% ±0.46%",
        )
        self.assertEqual(
            ranking.cost_cell([{}], "compute_cost_pct", "compute_cost_se_pct"),
            "–",
        )


if __name__ == "__main__":
    unittest.main()
