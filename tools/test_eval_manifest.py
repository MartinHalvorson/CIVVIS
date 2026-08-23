#!/usr/bin/env python3
"""Tests for the generated evaluation inventory and status snapshot."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

try:
    from tools import eval_manifest
except ImportError:  # unittest discovery adds tools/ directly to sys.path
    import eval_manifest


REPO = Path(__file__).resolve().parent.parent


class EvalManifestTests(unittest.TestCase):
    def setUp(self) -> None:
        self.manifest = eval_manifest.build_manifest(REPO)

    def test_every_registry_declaration_is_counted_without_duplicates(self):
        for name, row in self.manifest["registry"].items():
            self.assertEqual(row["declared_count"], len(row["items"]), name)
            self.assertEqual(len(row["items"]), len(set(row["items"])), name)

    def test_derived_counts_are_from_the_registry(self):
        registry = self.manifest["registry"]
        derived = self.manifest["derived"]
        live = set(registry["LIVE_BRIDGE_TREATMENTS"]["items"])
        firaxis = set(registry["FIRAXIS_ONLY_TREATMENTS"]["items"])
        self.assertEqual(derived["live_bridge_count"], len(live))
        self.assertEqual(derived["firaxis_only_count"], len(firaxis))
        self.assertEqual(derived["withholdable_live_count"], len(live - firaxis))

    def test_committed_json_and_status_are_current(self):
        self.assertEqual(
            json.loads((REPO / "docs" / "eval_manifest.json").read_text()),
            self.manifest,
        )
        self.assertEqual(
            (REPO / "docs" / "EVAL_STATUS.md").read_text(),
            eval_manifest.render_status(self.manifest),
        )

    def test_evidence_and_roadmap_link_to_the_current_snapshot(self):
        for path in (REPO / "docs" / "EVAL.md", REPO / "docs" / "ROADMAP.md"):
            self.assertIn("EVAL_STATUS.md", path.read_text(encoding="utf-8"), path.name)



class LadderDistanceTest(unittest.TestCase):
    """The distance block has to survive the ledger it is actually given."""

    def test_ungraded_ledger_says_the_distance_is_unknown(self):
        attempts = [
            {"configured": True, "turns": 250, "score": 400, "victory": 0},
            {"configured": True, "turns": 250, "score": 600, "victory": 0},
        ]
        distance = eval_manifest.ladder_distance(attempts)
        self.assertEqual(distance["full_length"], 2)
        self.assertEqual(distance["graded"], 0)
        self.assertNotIn("lead_median", distance)
        rendered = "\n".join(eval_manifest._ladder_distance_lines(distance))
        self.assertIn("Distance to a win: **unknown**", rendered)

    def test_a_rival_victory_before_the_clock_is_counted_and_named(self):
        attempts = [
            # Ahead on score and beaten by somebody else's condition.
            {
                "configured": True,
                "turns": 243,
                "score": 1406,
                "rival_best": 997,
                "victory": 6,
            },
            # Behind, same shape.
            {
                "configured": True,
                "turns": 219,
                "score": 643,
                "rival_best": 973,
                "victory": 3,
            },
            # Ran the clock out: ends on score, not stolen.
            {
                "configured": True,
                "turns": 250,
                "score": 702,
                "rival_best": 1140,
                "victory": 0,
            },
            # A win is never a theft.
            {
                "configured": True,
                "turns": 222,
                "score": 900,
                "rival_best": 800,
                "victory": 6,
                "won": True,
            },
        ]
        endings = eval_manifest.ladder_endings(attempts)
        self.assertEqual(endings["stolen"], {"diplomatic": 1, "culture": 1})
        self.assertEqual(endings["stolen_while_ahead"], 1)

    def test_an_empty_ledger_reports_no_finished_attempt(self):
        distance = eval_manifest.ladder_distance([])
        rendered = "\n".join(eval_manifest._ladder_distance_lines(distance))
        self.assertIn("no finished attempt on record", rendered)


if __name__ == "__main__":
    unittest.main()


class BundleCoverageTests(unittest.TestCase):
    """The bundle says how much of itself the evidence has ever discussed.

    On 2026-08-18, 34 of the 51 withholdable live-bridge treatments had never
    been named in any recorded round, and nothing said so — `docs/ROADMAP.md`
    objective 3 asks for exactly this bundle to be priced by withholding, while
    the generated inventory counted what *exists*.
    """

    def setUp(self):
        self.manifest = eval_manifest.build_manifest(REPO)
        self.coverage = self.manifest["coverage"]

    def test_the_three_numbers_agree_with_each_other(self):
        self.assertEqual(
            self.coverage["named"] + self.coverage["never_named"],
            self.coverage["withholdable"],
        )
        self.assertEqual(len(self.coverage["never_named_treatments"]),
                         self.coverage["never_named"])

    def test_coverage_counts_the_withholdable_set_the_inventory_reports(self):
        self.assertEqual(self.coverage["withholdable"],
                         self.manifest["derived"]["withholdable_live_count"])

    def test_a_firaxis_only_treatment_is_not_counted_as_debt(self):
        """Those cannot be withheld from the live seat, so they are not owed a
        withholding result."""
        firaxis = set(self.manifest["registry"]["FIRAXIS_ONLY_TREATMENTS"]["items"])
        self.assertEqual(
            firaxis & set(self.coverage["never_named_treatments"]), set())

    def test_every_spelling_a_round_can_use_is_searched(self):
        """⚠ FOUND BY CHECKING THE INSTRUMENT AGAINST A KNOWN RESULT. Rounds
        write the registry tag (`bounded-recovery`), the Rust flag
        (`bounded_recovery`), and the derived arm
        (`live_without_bounded_recovery`). Searching only two of the three
        called `bounded-recovery` never-named — a treatment whose confirmed-null
        result is why it was deleted from production — and overstated the debt
        by a fifth."""
        live = ["alpha-one", "beta-two", "gamma-three"]
        for spelling in ("alpha-one", "beta_two", "live_without_gamma_three"):
            with self.subTest(spelling=spelling):
                coverage = eval_manifest.bundle_coverage(
                    live, set(), f"a round that mentions {spelling} somewhere")
                self.assertEqual(coverage["named"], 1, coverage)

    def test_a_treatment_named_nowhere_is_counted_as_debt(self):
        coverage = eval_manifest.bundle_coverage(
            ["never-mentioned"], set(), "an evidence blob about other things")
        self.assertEqual(coverage["never_named"], 1)
        self.assertEqual(coverage["never_named_treatments"], ["never-mentioned"])

    def test_the_evidence_is_globbed_not_listed(self):
        """A round that exists but is not searched is evidence nobody has."""
        evidence = eval_manifest.read_evidence(REPO)
        newest = sorted((REPO / "docs" / "eval").glob("*.md"))[-1]
        self.assertIn(newest.read_text(encoding="utf-8")[:200], evidence)

    def test_the_published_page_carries_the_number_to_act_on(self):
        page = (REPO / "docs" / "EVAL_STATUS.md").read_text(encoding="utf-8")
        self.assertIn("## Bundle coverage", page)
        self.assertIn(f"Never named in any round: {self.coverage['never_named']}",
                      page)


class StolenTurnsTest(unittest.TestCase):
    """The turn a stolen lane lands on, not only how often it is stolen."""

    def attempt(self, victory, turns, **kw):
        row = {"configured": True, "won": False, "victory": victory, "turns": turns}
        row.update(kw)
        return row

    def test_a_stolen_lane_reports_the_window_it_landed_in(self):
        """★★★★★ THE COUNT SAYS WHICH LANE, THE TURN SAYS WHETHER OURS BEHAVES.

        A rival's diplomatic victory has never landed before turn 202 on this
        ladder and lands at a median of 234 — a photo finish against the
        250-turn limit. Four iterations of work on CIVVIS' own diplomatic lane
        were aimed at a count with no window beside it, and 'our native lane
        produces almost none' cannot be read against 'theirs lands at 239'
        without knowing the second number.
        """
        rows = [
            self.attempt(6, 202),
            self.attempt(6, 234),
            self.attempt(6, 247),
            self.attempt(3, 145),
        ]
        summary = eval_manifest.ladder_endings(rows)
        windows = summary["stolen_turns"]
        self.assertEqual(
            windows["diplomatic"], {"earliest": 202, "median": 234, "latest": 247}
        )
        self.assertEqual(
            windows["culture"], {"earliest": 145, "median": 145, "latest": 145}
        )

    def test_a_lane_that_ran_the_clock_out_is_not_stolen(self):
        """A game that reaches the limit ends on score by construction, so it
        has not been stolen and must not widen anyone's window."""
        rows = [self.attempt(6, 250), self.attempt(6, 210)]
        summary = eval_manifest.ladder_endings(rows)
        self.assertEqual(
            summary["stolen_turns"]["diplomatic"],
            {"earliest": 210, "median": 210, "latest": 210},
        )
class GenomeCoverageTests(unittest.TestCase):
    """The count of what the genome instrument cannot see.

    ⚠⚠ THE FIRST VERSION OF THIS COUNT INVENTED DEBT, and the trap is the one
    the manifest's row scrape already records from the other side: a treatment row's
    FIELD string and its toggle's NAME are not the same word. The row for
    `army-target-weighs-enemy` reads
    `("army_target_weighs_enemy", …, disable_army_target_weighs_the_enemy)` —
    note the "the" — so matching the toggle set against the field string alone
    reported a gene the ledger has measured as unreachable by any screen. A
    published debt list that names a measured gene is worse than no list.
    """

    def setUp(self):
        self.coverage = eval_manifest.genome_coverage(REPO)

    def test_the_counts_are_consistent(self):
        c = self.coverage
        self.assertGreaterEqual(c["capability_toggles"],
                                c["unreachable_by_any_screen"])
        self.assertGreaterEqual(c["reachable_as_a_gene"],
                                c["measured_by_a_screen"])
        self.assertGreaterEqual(c["measured_by_a_screen"],
                                c["resolved_by_a_screen"])
        self.assertEqual(len(c["unreachable_toggles"]),
                         c["unreachable_by_any_screen"])

    def test_a_toggle_named_differently_from_its_row_is_still_reachable(self):
        """The real fixture, not a synthetic one: this exact flag is the
        mismatch that broke the first draft."""
        self.assertNotIn("army_target_weighs_the_enemy",
                         self.coverage["unreachable_toggles"],
                         "a gene the ledger has measured cannot be unreachable; "
                         "the field/function spelling join is broken again")

    def test_a_measured_gene_is_reachable_or_is_a_recorded_removal(self):
        """A screen measured it, so a screen can reach it — with the one
        exception the repository documents: a native leg deliberately removed,
        leaving a host-only gene whose ledger row is history.

        The bound is what matters, not the number. If this starts failing,
        something has quietly stopped being screenable while its row still
        steers `ledger_default_on`.
        """
        ledger = json.loads(
            (REPO / "docs" / "gene_ledger.json").read_text(encoding="utf-8"))
        measured = len(ledger["genes"])
        stranded = measured - self.coverage["measured_by_a_screen"]
        self.assertLessEqual(
            stranded, 1,
            f"{stranded} measured genes are no longer reachable by any screen; "
            "each one is a ledger row governing a deployment default that no "
            "future screen can revisit")

    def test_the_scrape_fails_loudly_rather_than_reporting_zero_debt(self):
        """An empty answer is the dangerous one: it reads as "no debt"."""
        import tempfile
        with tempfile.TemporaryDirectory() as empty:
            with self.assertRaises((ValueError, OSError, IndexError)):
                eval_manifest.genome_coverage(Path(empty))

    def test_the_section_reaches_the_status_page(self):
        """AGENTS.md: a guard you add runs in the same change that adds it. A
        count nobody publishes is a count nobody acts on."""
        page = (REPO / "docs" / "EVAL_STATUS.md").read_text(encoding="utf-8")
        self.assertIn("## Genome coverage", page)
        self.assertIn(f"Unreachable by any screen: "
                      f"{self.coverage['unreachable_by_any_screen']}", page)
