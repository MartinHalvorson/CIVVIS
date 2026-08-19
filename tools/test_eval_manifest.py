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
    def test_every_withholdable_treatment_names_a_runnable_arm(self):
        """⚠⚠ THE DEBT LIST NAMED TREATMENTS NOBODY COULD RUN.

        `docs/EVAL_STATUS.md` publishes the never-named list as the work
        roadmap objective 3 asks the fleet to do, and the arm name was derived
        as `live_without_{tag with underscores}`. There is no such rule, and
        both obvious ones are wrong somewhere: `ranged-line-of-sight`'s arm is
        `live_without_ranged_needs_line_of_sight`, after the flag, while
        `army-target-weighs-enemy` sets `army_target_weighs_the_enemy` and its
        arm is `live_without_army_target_weighs_enemy`, after the tag.

        So the arm is looked up in `EVAL_ONLY_AIS` rather than guessed, and a
        treatment with no arm raises. That found one immediately:
        ⚠ Scoped to the withholdable tags. `LIVE_TREATMENTS` also carries rows
        that are not live-bridge treatments — `strategic-wonders` is one — and
        those correctly have no arm; checking every row raised on them.
        """
        registry = eval_manifest.read_registry(REPO)
        withholdable = {
            tag
            for tag in registry["LIVE_BRIDGE_TREATMENTS"]["items"]
            if tag not in set(registry["FIRAXIS_ONLY_TREATMENTS"]["items"])
        }
        arms = eval_manifest.withholding_arms(
            REPO, registry["EVAL_ONLY_AIS"]["items"], withholdable
        )
        known = set(registry["EVAL_ONLY_AIS"]["items"])
        self.assertTrue(arms, "no withholding arms were resolved at all")
        for tag, arm in arms.items():
            self.assertIn(arm, known, f"{tag} resolves to {arm}, which is not a registered arm")
        # Non-vacuous: at least one tag's arm is not the naive transformation,
        # so the lookup is doing work a derivation could not.
        naive = {tag: f"live_without_{tag.replace('-', '_')}" for tag in arms}
        self.assertTrue(
            any(arms[tag] != naive[tag] for tag in arms),
            "every arm matches the naive spelling; this test would pass on the "
            "derivation it exists to replace",
        )

    def test_a_treatment_without_an_arm_is_refused(self):
        """The guard bites rather than guessing a name."""
        registry = eval_manifest.read_registry(REPO)
        short = [
            name
            for name in registry["EVAL_ONLY_AIS"]["items"]
            if not name.startswith("live_without_")
        ]
        withholdable = {
            tag
            for tag in registry["LIVE_BRIDGE_TREATMENTS"]["items"]
            if tag not in set(registry["FIRAXIS_ONLY_TREATMENTS"]["items"])
        }
        with self.assertRaises(ValueError) as refused:
            eval_manifest.withholding_arms(REPO, short, withholdable)
        self.assertIn("no evaluator arm", str(refused.exception))

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
