#!/usr/bin/env python3
"""The surrogate analysis refuses the two readings that would have been wrong.

Both are readings a careful person makes by default:

* fitting share against win inside one screen, where the same seats produce
  both numbers and the shared luck looks like signal; and
* quoting a slope off the near-zero genes, where correcting for attenuation
  divides by nearly nothing and returns a number four times too large.

Each has a test below that fails if the guard is removed.
"""

from __future__ import annotations

import json
import math
import random
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import share_predicts_win as spw  # noqa: E402


def write_screens(root, genes, screens, slope, structural, share_se, win_se,
                  seed=11):
    """A synthetic corpus with a KNOWN slope and a KNOWN structural residual.

    Each gene gets one true share effect and a true win effect on the line plus
    its own structural offset. Every screen then observes both with independent
    sampling noise, which is exactly the structure the tool claims to undo.
    """
    rng = random.Random(seed)
    truth = {}
    for g in range(genes):
        true_share = rng.gauss(0.0, 0.35)
        truth[f"gene-{g}"] = (true_share,
                              slope * true_share + rng.gauss(0.0, structural))
    for s in range(screens):
        rows = []
        for tag, (ts, tw) in truth.items():
            rows.append(dict(tag=tag, seats=6000,
                             share_delta_pp=ts + rng.gauss(0.0, share_se),
                             share_se_pp=share_se,
                             win_delta_pp=tw + rng.gauss(0.0, win_se),
                             win_se_pp=win_se))
        (root / f"screen-{s:03d}.json").write_text(
            json.dumps(dict(shape="standard", genes=rows)))
    return truth


class RecoversWhatItClaims(unittest.TestCase):
    def test_slope_and_structural_residual_come_back(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            write_screens(root, genes=140, screens=12, slope=3.0,
                          structural=0.45, share_se=0.12, win_se=0.73)
            rows, _, _ = spw.load_screens(str(root / "*.json"))
            by = {}
            for r in rows:
                by.setdefault(r["tag"], []).append(r)
            fits = [spw.corrected_fit(spw.disjoint_points(by, s)) for s in range(60)]
            fits = [f for f in fits if f]
            slope = sorted(f["slope"] for f in fits)[len(fits) // 2]
            resid = sorted(f["structural_sd"] for f in fits)[len(fits) // 2]
            self.assertAlmostEqual(slope, 3.0, delta=0.35)
            self.assertAlmostEqual(resid, 0.45, delta=0.18)

    def test_no_structural_residual_reads_as_none(self):
        """Generated exactly on the line, the floor must collapse -- otherwise
        the tool would put an irreducible floor under every prediction and
        argue against a surrogate that in fact works perfectly."""
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            write_screens(root, genes=140, screens=12, slope=3.0,
                          structural=0.0, share_se=0.12, win_se=0.73)
            rows, _, _ = spw.load_screens(str(root / "*.json"))
            by = {}
            for r in rows:
                by.setdefault(r["tag"], []).append(r)
            fits = [spw.corrected_fit(spw.disjoint_points(by, s)) for s in range(60)]
            resid = sorted(f["structural_sd"] for f in fits if f)[len(fits) // 2]
            self.assertLess(resid, 0.15)


class TheDisjointDesignIsActuallyDisjoint(unittest.TestCase):
    def test_no_screen_feeds_both_sides_of_a_gene(self):
        """The whole argument rests on this. If one screen fed both axes, the
        errors would be correlated and the slope inflated -- silently."""
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            write_screens(root, genes=25, screens=9, slope=3.0, structural=0.4,
                          share_se=0.12, win_se=0.73)
            rows, _, _ = spw.load_screens(str(root / "*.json"))
            by = {}
            for r in rows:
                by.setdefault(r["tag"], []).append(r)
            for seed in range(25):
                rng = random.Random(seed)
                for tag, records in sorted(by.items()):
                    order = list(range(len(records)))
                    rng.shuffle(order)
                    half = len(order) // 2
                    left = {records[i]["screen"] for i in order[:half]}
                    right = {records[i]["screen"] for i in order[half:]}
                    self.assertEqual(left & right, set(), f"{tag} shares a screen")
                    self.assertTrue(left and right)

    def test_a_gene_seen_once_is_dropped(self):
        rows = [dict(screen="a", tag="solo", share=1.0, share_se=0.1,
                     win=3.0, win_se=0.5, seats=10)]
        by = {"solo": rows}
        self.assertEqual(spw.disjoint_points(by, 0), [])


class TheGuardsFire(unittest.TestCase):
    def test_low_signal_band_is_marked_unquotable(self):
        """Restricting to near-zero genes makes the corrected Sxx tiny, and the
        slope explodes. The SNR must catch it rather than the reader."""
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            write_screens(root, genes=200, screens=10, slope=3.0,
                          structural=0.45, share_se=0.30, win_se=0.9)
            rows, _, _ = spw.load_screens(str(root / "*.json"))
            by = {}
            for r in rows:
                by.setdefault(r["tag"], []).append(r)
            points = spw.disjoint_points(by, 0)
            narrow = [p for p in points if abs(p[1]) < 0.10]
            self.assertGreater(len(narrow), 8, "need a populated low band")
            fit = spw.corrected_fit(narrow)
            if fit is not None:
                self.assertLess(fit["snr"], spw.BAND_MIN_SNR)

    def test_an_empty_glob_is_an_error_not_an_empty_report(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaises(SystemExit):
                spw.load_screens(str(Path(d) / "nothing-*.json"))

    def test_a_corpus_without_errors_is_refused(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / "s.json").write_text(json.dumps(
                dict(genes=[dict(tag="g", share_delta_pp=1.0, win_delta_pp=3.0)])))
            with self.assertRaises(SystemExit):
                spw.load_screens(str(root / "*.json"))

    def test_a_zero_standard_error_is_dropped_not_divided_by(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / "s.json").write_text(json.dumps(dict(genes=[
                dict(tag="ok", share_delta_pp=1.0, share_se_pp=0.1,
                     win_delta_pp=3.0, win_se_pp=0.5, seats=10),
                dict(tag="bad", share_delta_pp=1.0, share_se_pp=0.0,
                     win_delta_pp=3.0, win_se_pp=0.5, seats=10)])))
            rows, _, _ = spw.load_screens(str(root / "*.json"))
            self.assertEqual([r["tag"] for r in rows], ["ok"])


class DetectionIsADifferentQuestion(unittest.TestCase):
    """The crossover prices an effect's SIZE. Asking whether a gene does
    anything compares one z against another and never meets the floor."""

    def _corpus(self, slope, structural, share_se, win_se, genes=160, screens=10):
        d = tempfile.TemporaryDirectory()
        root = Path(d.name)
        write_screens(root, genes=genes, screens=screens, slope=slope,
                      structural=structural, share_se=share_se, win_se=win_se)
        rows, _, _ = spw.load_screens(str(root / "*.json"))
        by = {}
        for r in rows:
            by.setdefault(r["tag"], []).append(r)
        return d, by

    def test_the_z_ratio_is_not_the_standard_error_ratio(self):
        """The trap this function exists to close. With a win SE 6x the share SE
        AND a win effect 3x the share effect, the detection advantage is about
        2x, not 6x -- most of the SE ratio cancels against the effect ratio."""
        d, by = self._corpus(slope=3.0, structural=0.0, share_se=0.12, win_se=0.72)
        with d:
            det = spw.detection_comparison(by)
            self.assertLess(det["median_z_ratio"], 3.0,
                            "a 6x SE ratio must not read as a 6x detection edge")
            self.assertGreater(det["median_z_ratio"], 1.2,
                               "share should still be the stronger detector")

    def test_share_detects_more_genes_than_wins_do(self):
        d, by = self._corpus(slope=3.0, structural=0.3, share_se=0.12, win_se=0.72)
        with d:
            det = spw.detection_comparison(by)
            self.assertGreater(det["share_only"], det["win_only"])

    def test_signs_agree_when_the_link_is_real(self):
        d, by = self._corpus(slope=3.0, structural=0.05, share_se=0.10, win_se=0.60)
        with d:
            det = spw.detection_comparison(by)
            self.assertGreater(det["both"], 10, "need genes on both axes")
            self.assertGreater(det["agree"], 8 * det["disagree"] + 1)

    def test_an_unrelated_axis_produces_no_sign_agreement(self):
        """If share carried no information about wins, the sign agreement that
        licenses the whole method would collapse to a coin flip."""
        d, by = self._corpus(slope=0.0, structural=1.2, share_se=0.10, win_se=0.60)
        with d:
            det = spw.detection_comparison(by)
            if det["both"] >= 10:
                self.assertLess(det["agree"], det["both"] * 0.85)

    def test_every_gene_lands_in_exactly_one_bucket(self):
        d, by = self._corpus(slope=3.0, structural=0.3, share_se=0.12, win_se=0.72)
        with d:
            det = spw.detection_comparison(by)
            self.assertEqual(
                det["both"] + det["share_only"] + det["win_only"] + det["neither"],
                det["genes"])
            self.assertEqual(det["agree"] + det["disagree"], det["both"])


class TheCorpusIsPartOfTheAnswer(unittest.TestCase):
    """A slope quoted without its corpus is the mistake these filters exist to
    prevent: the ledger curates 10 of 82 screens and reads a different slope."""

    def test_shape_filter_selects_only_that_shape(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            write_screens(root, genes=6, screens=3, slope=3.0, structural=0.3,
                          share_se=0.2, win_se=0.9)
            # relabel one screen's shape
            odd = sorted(root.glob("*.json"))[0]
            doc = json.loads(odd.read_text())
            doc["shape"] = "legacy"
            odd.write_text(json.dumps(doc))
            everything, _, _ = spw.load_screens(str(root / "*.json"))
            standard, _, _ = spw.load_screens(str(root / "*.json"), shape="standard")
            legacy, _, _ = spw.load_screens(str(root / "*.json"), shape="legacy")
            self.assertEqual(len(everything), len(standard) + len(legacy))
            self.assertEqual(len(legacy), 6)

    def test_an_empty_filter_result_names_the_filter(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            write_screens(root, genes=4, screens=2, slope=3.0, structural=0.3,
                          share_se=0.2, win_se=0.9)
            with self.assertRaises(SystemExit) as caught:
                spw.load_screens(str(root / "*.json"), shape="no-such-shape")
            self.assertIn("no-such-shape", str(caught.exception))

    def test_ledger_sources_are_read_from_the_ledger_not_guessed(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            ledger = root / "ledger.json"
            ledger.write_text(json.dumps(
                dict(sources=[{"path": "a.json"}, {"path": "b.json"}])))
            self.assertEqual(spw.ledger_sources(str(ledger)), {"a.json", "b.json"})

    def test_a_ledger_with_no_sources_is_an_error(self):
        with tempfile.TemporaryDirectory() as d:
            ledger = Path(d) / "ledger.json"
            ledger.write_text(json.dumps(dict(sources=[])))
            with self.assertRaises(SystemExit):
                spw.ledger_sources(str(ledger))

    def test_the_real_ledger_curates_a_subset(self):
        """The premise the whole warning rests on. If the ledger ever read every
        screen on disk, the warning would be wrong and should come out."""
        repo = Path(__file__).resolve().parent.parent
        ledger = repo / "docs" / "gene_ledger.json"
        screens = sorted((repo / "docs" / "gene_screens").glob("*.json"))
        if not ledger.exists() or not screens:
            self.skipTest("no committed ledger or screens here")
        sources = spw.ledger_sources(str(ledger))
        self.assertLess(len(sources), len(screens),
                        "ledger reads every screen -- the doc's warning is stale")


class TheCrossoverIsTheRealAnswer(unittest.TestCase):
    def test_the_advantage_falls_as_seats_rise_and_ends(self):
        """A surrogate with a structural floor cannot win forever: a direct
        reading's error shrinks with seats and the prediction's does not."""
        slope, structural, ratio = 2.94, 0.466, 6.09
        advantage = lambda s: (ratio * s) ** 2 / (slope * slope * s * s + structural ** 2)
        self.assertGreater(advantage(0.60), advantage(0.20))
        self.assertGreater(advantage(0.20), advantage(0.08))
        self.assertGreater(advantage(0.60), 1.0)
        self.assertLess(advantage(0.05), 1.0)
        crossover = structural / math.sqrt(ratio ** 2 - slope ** 2)
        self.assertAlmostEqual(advantage(crossover), 1.0, places=6)

    def test_a_surrogate_with_no_floor_never_crosses_over(self):
        slope, structural, ratio = 2.94, 0.0, 6.09
        advantage = lambda s: (ratio * s) ** 2 / (slope * slope * s * s + structural ** 2)
        self.assertAlmostEqual(advantage(0.6), advantage(0.05), places=6)
        self.assertGreater(advantage(0.05), 1.0)


if __name__ == "__main__":
    unittest.main()
