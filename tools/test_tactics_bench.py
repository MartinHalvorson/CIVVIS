#!/usr/bin/env python3
"""Regression checks for the Tactics arena benchmark.

These exercise the parsing and the reporting, which is where a benchmark goes
quietly wrong: a harness that misreads a tournament line reports a number that
looks fine and is not the one the engine produced.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import tactics_bench  # noqa: E402


STANDARDIZED = """
Anchored online Elo leaderboard (order-sensitive K-factor path, player across all draws):
  advanced                  1690.5   games=40   wins=31   winrate= 78%
  advanced_v1               1500.0   games=40   wins=9    winrate= 22%

Standardized direct performance Elo vs advanced_v1 (order-independent Jeffreys point; 95% Wilson interval transformed to Elo):
  advanced                  1708.2 (95%  1588.7.. 1841.0)   pair-score= 31.0/40   (77.5%, 95% 62.5..87.7%)
"""

LEADERBOARD_ONLY = """
Anchored online Elo leaderboard (order-sensitive K-factor path, player across all draws):
  basic                     1676.5   games=40   wins=39   winrate= 98%
  advanced                  1323.5   games=40   wins=1    winrate=  2%
"""


class ParsePairScoreTest(unittest.TestCase):
    def test_the_standardized_line_is_read_with_its_interval(self) -> None:
        found = None
        for line in STANDARDIZED.splitlines():
            match = tactics_bench.PAIR_LINE.match(line)
            if match and match["name"] == "advanced":
                found = match
        self.assertIsNotNone(found, "the standardized pair-score line must parse")
        self.assertEqual(float(found["score"]), 31.0)
        self.assertEqual(int(found["games"]), 40)
        self.assertAlmostEqual(float(found["pct"]), 77.5)
        self.assertAlmostEqual(float(found["lo"]), 62.5)
        self.assertAlmostEqual(float(found["hi"]), 87.7)

    def test_the_leaderboard_is_the_fallback_when_neither_side_is_the_anchor(self) -> None:
        """`advanced` against `basic` prints no standardized block, because the
        ledger's anchor is in neither seat. The win counts still answer it."""
        self.assertIsNone(
            next(
                (m for m in map(tactics_bench.PAIR_LINE.match, LEADERBOARD_ONLY.splitlines())
                 if m and m["name"] == "advanced"),
                None,
            ),
            "this shape has no standardized block to read",
        )
        rows = {}
        for line in LEADERBOARD_ONLY.splitlines():
            match = tactics_bench.WIN_LINE.match(line)
            if match:
                rows[match["name"]] = (int(match["wins"]), int(match["games"]))
        self.assertEqual(rows["advanced"], (1, 40))
        self.assertEqual(rows["basic"], (39, 40))

    def test_a_leaderboard_line_is_not_mistaken_for_a_pair_score(self) -> None:
        """The two shapes share a prefix. Reading a leaderboard row as a
        pair-score would report the order-sensitive figure as the standardized
        one, which is a different measurement."""
        for line in LEADERBOARD_ONLY.splitlines():
            self.assertIsNone(tactics_bench.PAIR_LINE.match(line), line)


class BaselineRoundTripTest(unittest.TestCase):
    def results(self) -> list[tactics_bench.Result]:
        return [
            tactics_bench.Result("capture", "advanced", "basic", 39.0, 40, 97.5, 87.1, 99.6),
            tactics_bench.Result("capture", "advanced", "advanced_v1", 31.0, 40, 77.5, 62.5, 87.7),
            tactics_bench.Result("attrition", "advanced", "basic", 1.0, 40, 2.5, 0.4, 12.9),
            tactics_bench.Result("attrition", "advanced", "advanced_v1", 19.0, 40, 47.5, 32.9, 62.5),
        ]

    def test_a_written_baseline_reads_back_exactly(self) -> None:
        text = tactics_bench.render_baseline(self.results(), games=40)
        recorded = tactics_bench.parse_baseline(text)
        self.assertEqual(recorded[("attrition", "advanced", "basic")], 2.5)
        self.assertEqual(recorded[("capture", "advanced", "basic")], 97.5)
        self.assertEqual(recorded[("attrition", "advanced", "advanced_v1")], 47.5)

    def test_the_document_warns_against_reading_the_anchor_column(self) -> None:
        """The null result against a frozen copy of the same controller is the
        one mistake this benchmark exists to prevent, so the document has to say
        so where the numbers are read."""
        text = tactics_bench.render_baseline(self.results(), games=40)
        self.assertIn("expected null", text)
        self.assertIn("basic", text)

    def test_every_regime_reaches_the_table(self) -> None:
        rendered = tactics_bench.table(self.results())
        for regime in tactics_bench.REGIMES:
            self.assertIn(regime.title, rendered)
        # A regime with no result still gets a row, so a partial run is visibly
        # partial rather than looking like a shorter battery.
        self.assertIn("—", rendered)

    def test_a_cell_without_an_interval_is_still_reported(self) -> None:
        bare = tactics_bench.Result("attrition", "advanced", "basic", 1.0, 40, 2.5, None, None)
        self.assertEqual(bare.cell(), "2.5%")
        self.assertIn("–", self.results()[0].cell())


if __name__ == "__main__":
    unittest.main()


class BaselineSaysHowOldItIs(unittest.TestCase):
    """A table with no age on it reads as current, and this one was not.

    ⚠ On 2026-08-17 the committed baseline said `advanced` took 30.0% of the
    pure-combat regime against `basic`; the same day's `main` measured 70.0%.
    The capture regime had gone the other way — 97.5% recorded, **75.8%
    measured over 120 games**, a 21.7-point regression in the shipped Tactics
    product that nobody had seen, because the only instrument that would show
    it runs when somebody remembers to run it.
    """

    def results(self) -> list:
        return [
            tactics_bench.Result("capture", "advanced", "basic", 91.0, 120, 75.8, 67.4, 82.6),
        ]

    def test_a_written_baseline_carries_the_revision_it_was_measured_on(self) -> None:
        text = tactics_bench.render_baseline(self.results(), games=120)
        stamp = tactics_bench.baseline_provenance(text)
        self.assertIn("commit", stamp)
        self.assertEqual(stamp["games"], 120)
        # The human-readable half must be there too: a machine comment nobody
        # reads is how the old baseline aged invisibly in the first place.
        self.assertIn("Measured on", text)

    def test_an_unstamped_baseline_says_its_age_is_unknown(self) -> None:
        note = tactics_bench.staleness_note("# some old baseline\n\nno stamp here\n")
        self.assertIn("predates revision stamping", note)
        self.assertIn("--write-baseline", note)

    def test_a_baseline_from_this_revision_says_so(self) -> None:
        text = tactics_bench.render_baseline(self.results(), games=120)
        note = tactics_bench.staleness_note(text)
        # In a git checkout this is "this revision"; in an export the commit is
        # unknown and the note says that instead. Both are honest; neither may
        # silently claim currency.
        self.assertTrue(
            "this revision" in note or "predates revision stamping" in note,
            note,
        )

    def test_a_stamped_baseline_from_an_unknown_commit_does_not_claim_currency(self) -> None:
        stamp = '<!-- measured: {"commit": "0" * 40, "date": "", "games": 40} -->'
        note = tactics_bench.staleness_note(stamp.replace('"0" * 40', '"' + "0" * 40 + '"'))
        self.assertNotIn("this revision", note)

    def test_a_corrupt_stamp_is_ignored_rather_than_trusted(self) -> None:
        note = tactics_bench.staleness_note("<!-- measured: not json at all -->")
        self.assertIn("predates revision stamping", note)
