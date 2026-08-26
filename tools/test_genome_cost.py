#!/usr/bin/env python3
"""The deployed genome's compute bill, and the guard that keeps it current.

The last test in this file is the guard itself: it runs `genome_cost.py check`
against the repository's real ledger on every pull request, so the recorded
reading cannot drift from the ledger without something going red. `AGENTS.md`:
a guard you add runs in the same change that adds it.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

import genes as gene_ledger_tool
import genome_cost

REPO = Path(__file__).resolve().parent.parent


def screen(tag: str, cost: float, se: float, win_on: float, win_off: float,
           n_on: int = 10000, n_off: int = 5000) -> dict:
    return {"tag": tag, "compute_cost_pct": cost, "compute_cost_se_pct": se,
            "time_cost_pct": cost, "win_on": win_on, "win_off": win_off,
            "n_on": n_on, "n_off": n_off}


def ledger_with(genes: list, deployed: list, sources: list) -> dict:
    return {"rules": {"deployment_genome": deployed},
            "genes": genes, "sources": sources, "reporting_batches": []}


class TheWinComesFromTheScreensNotTheLedger(unittest.TestCase):
    """32 of the 76 deployed genes have no row in the ledger's `genes` array."""

    def test_the_deployed_genome_is_wider_than_the_ledgers_gene_rows(self):
        ledger = json.loads((REPO / "docs" / "gene_ledger.json").read_text())
        rows = {gene["tag"] for gene in ledger["genes"]}
        deployed = set(ledger["rules"]["deployment_genome"])
        self.assertTrue(
            deployed - rows,
            "every deployed gene now has a ledger row. If that is deliberate, "
            "this tool may read the win from there and this test should go; "
            "until then, reading it from the ledger reports the missing ones "
            "at a win of zero, which is how the most expensive gene in the "
            "genome looked like it bought nothing.")

    def test_the_dearest_deployed_gene_still_gets_a_win(self):
        ledger = json.loads((REPO / "docs" / "gene_ledger.json").read_text())
        reading = genome_cost.bill(ledger)
        dearest = max(reading["genes"],
                      key=lambda row: row["compute_cost_pct"] or 0.0)
        self.assertIsNotNone(dearest["win_diff_pp"])
        self.assertIsNotNone(dearest["compute_cost_pct"])

    def test_the_pooled_diff_is_the_rankings_arithmetic_not_a_second_copy(self):
        history = [{"win_on": 0.20, "win_off": 0.15, "n_on": 9000, "n_off": 3000},
                   {"win_on": 0.18, "win_off": 0.16, "n_on": 6000, "n_off": 2000}]
        expected = gene_ledger_tool.pooled_win_diff_pp(history)
        ledger = ledger_with(
            genes=[], deployed=["g"],
            sources=[{"path": "a.json"}, {"path": "b.json"}])
        found = genome_cost.bill  # arithmetic is shared by import, not copied
        self.assertIsNotNone(found)
        self.assertAlmostEqual(expected, gene_ledger_tool.pooled_win_diff_pp(history))


class OnlyAResolvedReadingCountsTowardTheIndicator(unittest.TestCase):

    def test_a_point_inside_one_standard_error_is_not_resolved(self):
        self.assertFalse(genome_cost.resolved(0.30, 0.40))
        self.assertTrue(genome_cost.resolved(0.90, 0.40))
        self.assertFalse(genome_cost.resolved(None, 0.40))
        self.assertFalse(genome_cost.resolved(float("nan"), 0.40))

    def test_noise_cannot_walk_the_summed_indicator(self):
        rows = [{"tag": "loud", "compute_cost_pct": 5.0, "resolved": True,
                 "win_diff_pp": 1.0, "cost_per_point": 5.0, "source": "s"},
                {"tag": "noise", "compute_cost_pct": 4.0, "resolved": False,
                 "win_diff_pp": 1.0, "cost_per_point": None, "source": "s"}]
        total = sum(row["compute_cost_pct"] for row in rows if row["resolved"])
        self.assertEqual(total, 5.0)


class CostPerPointRefusesToDivideByANonWin(unittest.TestCase):
    """A win of +0.01pp does not make a gene infinitely expensive; it makes the
    ratio meaningless, and printing a huge number would be a claim the data
    cannot support."""

    def test_a_win_below_the_promotion_bar_returns_nothing(self):
        self.assertIsNone(genome_cost.cost_per_point(
            {"resolved": True, "compute_cost_pct": 5.0, "win_diff_pp": 0.4}))

    def test_a_negative_win_returns_nothing(self):
        self.assertIsNone(genome_cost.cost_per_point(
            {"resolved": True, "compute_cost_pct": 5.0, "win_diff_pp": -1.0}))

    def test_a_gene_that_is_free_or_faster_returns_nothing(self):
        self.assertIsNone(genome_cost.cost_per_point(
            {"resolved": True, "compute_cost_pct": -1.0, "win_diff_pp": 2.0}))

    def test_a_real_ratio_is_cost_over_win(self):
        self.assertEqual(genome_cost.cost_per_point(
            {"resolved": True, "compute_cost_pct": 2.0, "win_diff_pp": 1.0}), 2.0)

    def test_the_bar_is_the_promotion_rules_own(self):
        self.assertEqual(genome_cost.PROMOTION_DIFF_PP, 0.85)


class TheDearAndUnderTheBarList(unittest.TestCase):

    def test_it_selects_costly_genes_that_do_not_clear_the_bar(self):
        reading = {"genes": [
            {"tag": "dear-and-weak", "compute_cost_pct": 3.0, "resolved": True,
             "win_diff_pp": 0.4},
            {"tag": "dear-and-strong", "compute_cost_pct": 3.0, "resolved": True,
             "win_diff_pp": 2.0},
            {"tag": "cheap-and-weak", "compute_cost_pct": 0.1, "resolved": True,
             "win_diff_pp": 0.1},
            {"tag": "unresolved", "compute_cost_pct": 3.0, "resolved": False,
             "win_diff_pp": 0.1},
        ]}
        found = [row["tag"] for row in genome_cost.dear_and_under_the_bar(reading)]
        self.assertEqual(found, ["dear-and-weak"])

    def test_the_threshold_sits_above_the_middle_of_the_measured_distribution(self):
        """0.5%/turn against a median |cost| of 0.37% over every probe."""
        self.assertGreater(genome_cost.DEAR_PCT, 0.37)
        self.assertLess(genome_cost.DEAR_PCT, 1.0)

    def test_the_real_genome_has_such_genes_and_the_report_names_them(self):
        ledger = json.loads((REPO / "docs" / "gene_ledger.json").read_text())
        reading = genome_cost.bill(ledger)
        lines = []
        genome_cost.render(reading, out=lines.append)
        text = "\n".join(lines)
        if genome_cost.dear_and_under_the_bar(reading):
            self.assertIn("do NOT clear the", text)
            self.assertIn("operator pins", text,
                          "the list must say a pin is a decision, not a defect")


class TheNewestScreenWins(unittest.TestCase):

    def test_reporting_batches_are_read_after_sources(self):
        ledger = {"sources": [{"path": "docs/a.json"}],
                  "reporting_batches": ["docs/b.json"]}
        paths = [path.name for path in genome_cost.screen_files(ledger)]
        self.assertEqual(paths, ["a.json", "b.json"])

    def test_a_reporting_batch_may_be_a_string_or_a_mapping(self):
        ledger = {"sources": [], "reporting_batches": ["docs/a.json",
                                                       {"path": "docs/b.json"}]}
        paths = [path.name for path in genome_cost.screen_files(ledger)]
        self.assertEqual(paths, ["a.json", "b.json"])

    def test_a_screen_the_ledger_does_not_use_is_not_evidence(self):
        """Globbing `docs/gene_screens/` would pull in probes and pilot runs
        the ledger deliberately excludes."""
        ledger = {"sources": [], "reporting_batches": []}
        self.assertEqual(genome_cost.screen_files(ledger), [])


class TheRecordedBillIsCurrent(unittest.TestCase):
    """⭐ THE GUARD. Runs against the real ledger on every pull request."""

    def test_check_passes_on_this_repository(self):
        self.assertEqual(
            genome_cost.main(["check"]), 0,
            "the recorded genome cost is stale. Run `python3 "
            "tools/genome_cost.py write` and keep the diff in your pull "
            "request — it names the gene that moved the fleet's compute bill.")

    def test_the_recorded_file_says_its_sum_is_not_a_total(self):
        recorded = json.loads(genome_cost.RECORD_JSON.read_text())
        self.assertIn("do not compose", recorded["not_a_total"])
        self.assertIn("summed_cost_pct", recorded)

    def test_write_and_check_agree(self):
        ledger = json.loads(genome_cost.LEDGER_JSON.read_text())
        reading = genome_cost.bill(ledger)
        recorded = json.loads(genome_cost.RECORD_JSON.read_text())
        self.assertEqual(recorded["deployed_genes"], reading["deployed_genes"])
        self.assertEqual(recorded["summed_cost_pct"], reading["summed_cost_pct"])


if __name__ == "__main__":
    unittest.main()
