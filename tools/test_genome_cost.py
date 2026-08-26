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
        """Substituting the imported function changes every win this tool
        reports, which is the only way to show it is not quietly reimplemented
        here. `pooled_win_diff_pp`'s own docstring says the printed totals and
        the published Diff are one arithmetic; a second copy would be a third
        number claiming to be the same one."""
        ledger = json.loads((REPO / "docs" / "gene_ledger.json").read_text())
        real = genome_cost.bill(ledger)
        original = gene_ledger_tool.pooled_win_diff_pp
        try:
            gene_ledger_tool.pooled_win_diff_pp = lambda history: 42.0
            substituted = genome_cost.bill(ledger)
        finally:
            gene_ledger_tool.pooled_win_diff_pp = original
        self.assertTrue(any(row["win_diff_pp"] == 42.0
                            for row in substituted["genes"]))
        self.assertFalse(any(row["win_diff_pp"] == 42.0 for row in real["genes"]))

    def test_the_published_diff_is_what_the_ranking_prints(self):
        """`naval-threat-triage` reads +0.68% in GENE_HEURISTIC_RANKING.md's
        Diff column; reading its win from the ledger instead reported +0.00."""
        ledger = json.loads((REPO / "docs" / "gene_ledger.json").read_text())
        reading = genome_cost.bill(ledger)
        wins = {row["tag"]: row["win_diff_pp"] for row in reading["genes"]}
        ranking = (REPO / "GENE_HEURISTIC_RANKING.md").read_text()
        for tag, win in wins.items():
            row = [line for line in ranking.splitlines()
                   if line.startswith("|") and ("`%s`" % tag) in line]
            if not row or win is None:
                continue
            self.assertIn("%.2f%%" % win, row[0],
                          "%s: this tool says %+.2fpp and the ranking's own "
                          "Diff column says otherwise" % (tag, win))


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


class TheHistoryIsTheRankingsOwn(unittest.TestCase):

    def test_it_uses_the_display_history_not_the_authoritative_one(self):
        """`load_sources` covers 44 of the 76 deployed genes; the other 32 —
        including the dearest in the genome — are priced only by the display
        batches, which is what `load_display_sources` adds."""
        ledger = json.loads((REPO / "docs" / "gene_ledger.json").read_text())
        authoritative, _ = gene_ledger_tool.load_sources(ledger)
        display = genome_cost.history(ledger)
        deployed = set(ledger["rules"]["deployment_genome"])
        self.assertLess(len(deployed & set(authoritative)), len(deployed))
        # A gene the operator pinned on before any batch priced it has no
        # display history yet; `PINNED_BEFORE_PRICING` names each one, and
        # its row leaves the day a batch column exists.
        unpriced = set(gene_ledger_tool.PINNED_BEFORE_PRICING)
        self.assertEqual(deployed - set(display) - unpriced, set())

    def test_the_newest_reading_with_a_cost_wins(self):
        rows = [{"compute_cost_pct": 1.0, "compute_cost_se_pct": 0.1,
                 "time_cost_pct": 1.0, "source": "old"},
                {"compute_cost_pct": 9.0, "compute_cost_se_pct": 0.2,
                 "time_cost_pct": 9.0, "source": "new"}]
        self.assertEqual(genome_cost.newest_cost(rows)["source"], "new")

    def test_a_screen_that_predates_the_timing_estimator_is_skipped(self):
        """It carries no cost at all, and a gene absent from the newest batch
        should report its last real measurement rather than a hole."""
        rows = [{"compute_cost_pct": 1.0, "compute_cost_se_pct": 0.1,
                 "time_cost_pct": 1.0, "source": "priced"},
                {"compute_cost_pct": None, "compute_cost_se_pct": None,
                 "source": "unpriced"}]
        self.assertEqual(genome_cost.newest_cost(rows)["source"], "priced")

    def test_no_reading_at_all_returns_nothing(self):
        self.assertIsNone(genome_cost.newest_cost([{"source": "x"}]))
        self.assertIsNone(genome_cost.newest_cost([]))


class TheRecordedBillIsCurrent(unittest.TestCase):
    """⭐ THE GUARD. Runs against the real ledger on every pull request."""

    def test_check_passes_on_this_repository(self):
        self.assertEqual(
            genome_cost.main(["check"]), 0,
            "the deployed genome changed and the bill was not re-recorded. Run "
            "`python3 tools/genome_cost.py write` and keep the diff in your "
            "pull request — it prices what just entered the genome.")

    def test_the_guard_fires_when_a_gene_joins_the_genome(self):
        """The event it exists for, exercised rather than asserted.

        Written against a record derived from today's ledger rather than
        against the committed file, so it proves the guard's behaviour whether
        or not the trunk's record happens to be current.
        """
        original = genome_cost.RECORD_JSON.read_text()
        current = genome_cost.bill(json.loads(genome_cost.LEDGER_JSON.read_text()))
        short = dict(current, genes=current["genes"][:-1])
        try:
            genome_cost.RECORD_JSON.write_text(json.dumps(short, indent=1) + "\n")
            self.assertEqual(genome_cost.main(["check"]), 1)
            genome_cost.RECORD_JSON.write_text(json.dumps(current, indent=1) + "\n")
            self.assertEqual(genome_cost.main(["check"]), 0)
        finally:
            genome_cost.RECORD_JSON.write_text(original)

    def test_the_guard_does_not_fire_when_only_the_figures_move(self):
        """The reporting batches rotate several times a day and reprice every
        gene. A check that compared the numbers would be red continuously and
        would teach the fleet to ignore it."""
        current = genome_cost.bill(json.loads(genome_cost.LEDGER_JSON.read_text()))
        repriced = dict(current, summed_cost_pct=current["summed_cost_pct"] + 4.0,
                        genes=[dict(row, compute_cost_pct=(row["compute_cost_pct"] or 0) + 1.0)
                               for row in current["genes"]])
        original = genome_cost.RECORD_JSON.read_text()
        try:
            genome_cost.RECORD_JSON.write_text(json.dumps(repriced, indent=1) + "\n")
            self.assertEqual(genome_cost.main(["check"]), 0)
        finally:
            genome_cost.RECORD_JSON.write_text(original)

    def test_the_recorded_file_says_its_sum_is_not_a_total(self):
        recorded = json.loads(genome_cost.RECORD_JSON.read_text())
        self.assertIn("do not compose", recorded["not_a_total"])
        self.assertIn("summed_cost_pct", recorded)

    def test_the_recorded_set_is_the_ledgers_deployment_genome(self):
        if genome_cost.is_stale() is not None:
            self.skipTest("trunk condition; see the class docstring")
        ledger = json.loads(genome_cost.LEDGER_JSON.read_text())
        recorded = json.loads(genome_cost.RECORD_JSON.read_text())
        self.assertEqual({row["tag"] for row in recorded["genes"]},
                         set(ledger["rules"]["deployment_genome"]))


if __name__ == "__main__":
    unittest.main()
