#!/usr/bin/env python3
"""The quantity contrast has to be the screen's own estimator, not a second one.

The whole argument for reading `techs` off a screen's rows is that the same
arithmetic, run on `win`, reproduces what `gene_screen --analyze` prints. If it
did not, a tech reading would be a number from a tool nobody has checked.
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gene_quantity_contrast as contrast  # noqa: E402

GENES = ["alpha", "beta", "gamma"]


def rows(spec):
    """A screen file from (pair, seat, genome, techs-arm0, techs-arm1) tuples."""
    out = [json.dumps({"kind": "header", "genes": GENES})]
    for pair, seat, genome, first, second in spec:
        flipped = "".join("1" if bit == "0" else "0" for bit in genome)
        for arm, genes_on, techs in ((0, genome, first), (1, flipped, second)):
            out.append(
                json.dumps(
                    {
                        "kind": "game",
                        "pair": pair,
                        "arm": arm,
                        "seed": 1000 + pair,
                        "seat": seat,
                        "genome": genes_on,
                        "techs": techs,
                        "win": techs > 50,
                    }
                )
            )
    return "\n".join(out) + "\n"


class QuantityContrast(unittest.TestCase):
    def analyse(self, spec, metric="techs", gene="alpha"):
        with TemporaryDirectory() as tmp:
            path = Path(tmp) / "run.jsonl"
            path.write_text(rows(spec))
            genes, game_rows = contrast.read([path])
            pairs = contrast.seat_pairs(game_rows)
            return contrast.contrast(pairs, genes, metric, gene)

    def test_a_clean_two_tech_edge_is_read_back_as_two(self):
        # `alpha` on in arm 0 of every pair, and arm 0 ends two techs ahead.
        spec = [(p, 0, "100", 52, 50) for p in range(6)]
        mean, se, z, clusters = self.analyse(spec)
        # The estimate is per unit of the +/-1 coding, so on-minus-off is twice
        # it: the two techs the fixture actually put there.
        self.assertAlmostEqual(2 * mean, 2.0)
        self.assertEqual(clusters, 6)
        self.assertEqual(se, 0.0)

    def test_the_sign_follows_the_genome_and_not_the_arm(self):
        # Same two-tech edge, but half the pairs carry `alpha` in arm 1
        # instead — and the edge follows the gene, so in those pairs it is arm
        # 1 that ends ahead. A reader that keyed off the arm rather than the
        # genome would cancel these to exactly zero.
        spec = [
            (p, 0, "100", 52, 50) if p % 2 == 0 else (p, 0, "011", 50, 52)
            for p in range(6)
        ]
        mean, _se, _z, _clusters = self.analyse(spec)
        self.assertAlmostEqual(2 * mean, 2.0)

    def test_a_gene_that_does_nothing_reads_zero(self):
        mean, _se, _z, _clusters = self.analyse(
            [(p, 0, "100", 50, 50) for p in range(6)], gene="beta"
        )
        self.assertAlmostEqual(mean, 0.0)

    def test_seats_of_one_game_are_one_cluster(self):
        # An all-seats run puts six seats of the same game in one pair; they
        # are not six independent observations and must not be counted so.
        spec = [(0, seat, "100", 52, 50) for seat in range(6)]
        spec += [(1, seat, "100", 52, 50) for seat in range(6)]
        _mean, _se, _z, clusters = self.analyse(spec)
        self.assertEqual(clusters, 2)

    def test_win_is_read_as_a_number_so_the_estimator_can_be_checked(self):
        # Arm 0 wins every pair, arm 1 never does: a 100-point on-minus-off
        # difference, which is what a screen would print as +100.0 pp.
        spec = [(p, 0, "100", 60, 40) for p in range(4)]
        mean, _se, _z, _clusters = self.analyse(spec, metric="win")
        self.assertAlmostEqual(2 * mean * 100, 100.0)

    def test_pooling_runs_that_screened_different_genes_is_refused(self):
        with TemporaryDirectory() as tmp:
            first = Path(tmp) / "a.jsonl"
            second = Path(tmp) / "b.jsonl"
            first.write_text(rows([(0, 0, "100", 52, 50)]))
            second.write_text(
                json.dumps({"kind": "header", "genes": ["alpha", "delta"]}) + "\n"
            )
            with self.assertRaises(SystemExit):
                contrast.read([first, second])

    def test_an_incomplete_pair_is_dropped_rather_than_half_counted(self):
        with TemporaryDirectory() as tmp:
            path = Path(tmp) / "run.jsonl"
            text = rows([(0, 0, "100", 52, 50), (1, 0, "100", 52, 50)])
            # Drop the last line: pair 1 now has arm 0 and no arm 1.
            path.write_text("\n".join(text.splitlines()[:-1]) + "\n")
            _genes, game_rows = contrast.read([path])
            self.assertEqual(len(contrast.seat_pairs(game_rows)), 1)


if __name__ == "__main__":
    unittest.main()
