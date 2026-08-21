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
    def test_the_checked_in_table_matches_the_ledgers_sources(self):
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        self.assertEqual(
            ranking.render(ledger),
            ranking.RANKING_MD.read_text(),
            "HEURISTIC_GENE_RANKING.md is stale: run tools/heuristic_gene_ranking.py --write",
        )

    def test_every_screenable_gene_with_a_native_measurement_is_ranked(self):
        ledger = json.loads(ranking.LEDGER_JSON.read_text())
        native, _, _, _ = ranking.load_sources(ledger)
        text = ranking.RANKING_MD.read_text()
        for tag in ranking.screenable_tags():
            if tag in native:
                self.assertIn(f"`{tag}`", text, tag)
        self.assertNotIn("`step-and-reassess` | ", text, "a host-only flag is not ranked natively")

    def test_descriptions_come_from_the_toggle_docs(self):
        desc = ranking.descriptions()
        self.assertGreater(len(desc), 50)
        self.assertTrue(desc["recon-replacement"].startswith("Rebuild the recon arm"))
        self.assertTrue(desc["loyalty-rate-alarm"].startswith("Rank loyalty emergencies"))


if __name__ == "__main__":
    unittest.main()
