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
        self.assertEqual(derived["eval_only_count"], len(registry["EVAL_ONLY_AIS"]["items"]))
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


if __name__ == "__main__":
    unittest.main()


class BundleCoverageTests(unittest.TestCase):
    """The bundle says how much of itself the evidence has ever discussed.

    On 2026-08-18, 34 of the 51 withholdable live-bridge treatments had never
    been named in any recorded round, and nothing said so — `docs/ROADMAP.md`
    objective 3 asks for exactly this bundle to be priced by withholding, while
    the generated inventory counted the arms that *exist*.
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
