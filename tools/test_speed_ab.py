#!/usr/bin/env python3
"""The paired harness refuses the conclusions the prose kept having to warn about."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import speed_ab  # noqa: E402


class TheReportIdentityCheck(unittest.TestCase):
    def test_the_timing_line_is_stripped_not_compared(self):
        """Two runs of the same game differ in elapsed time and nothing else."""
        a = "[12.5s] done\nRome score=17\nChina score=17"
        b = "[13.9s] done\nRome score=17\nChina score=17"
        self.assertEqual(speed_ab.report_digest(a), speed_ab.report_digest(b))

    def test_any_other_difference_is_a_different_game(self):
        a = "[12.5s] done\nRome score=17"
        b = "[12.5s] done\nRome score=18"
        self.assertNotEqual(speed_ab.report_digest(a), speed_ab.report_digest(b))


class TheArmsAlternate(unittest.TestCase):
    """Never all of A and then all of B: host load has to fall on both."""

    def _order(self, seeds):
        order = []

        def fake(binary, seed, opts):
            order.append((seed, Path(binary).name))
            return 1.0, "same"

        with mock.patch.object(speed_ab, "run_once", fake):
            speed_ab.compare(Path("/x/base"), Path("/x/cand"), seeds,
                             speed_ab.DEFAULTS)
        return order

    def test_both_arms_run_on_every_seed(self):
        order = self._order([10, 11, 12])
        for seed in (10, 11, 12):
            arms = {name for s, name in order if s == seed}
            self.assertEqual(arms, {"base", "cand"}, f"seed {seed}: {order}")

    def test_the_order_flips_between_seeds(self):
        """A host drifting in one direction would otherwise always favour
        whichever arm runs second."""
        order = self._order([10, 11])
        first_of = {seed: [name for s, name in order if s == seed][0]
                    for seed in (10, 11)}
        self.assertNotEqual(first_of[10], first_of[11], order)


class TheVerdictIsNotOverstated(unittest.TestCase):
    def _result(self, base, cand, mismatched=()):
        return {"totals": {"baseline": base, "candidate": cand},
                "change_pct": 100.0 * (cand - base) / base,
                "mismatched": list(mismatched), "seeds": [1]}

    def test_a_change_inside_the_floor_is_not_a_result(self):
        self.assertIn("NOISE", speed_ab.verdict(self._result(100.0, 100.1)))
        self.assertIn("NOISE", speed_ab.verdict(self._result(100.0, 99.9)))

    def test_a_change_outside_the_floor_is_reported_as_one(self):
        self.assertIn("-11.", speed_ab.verdict(self._result(100.0, 89.0)))

    def test_disagreeing_arms_beat_any_timing_number(self):
        """A change that alters behaviour has no overhead measurement at all,
        however large or clean the timing difference looks."""
        said = speed_ab.verdict(self._result(100.0, 50.0, mismatched=[7]))
        self.assertIn("ARMS DISAGREE", said)
        self.assertNotIn("-50", said)

    def test_disagreeing_arms_exit_nonzero(self):
        with mock.patch.object(speed_ab, "run_once",
                               lambda b, s, o: (1.0, Path(b).name)), \
             mock.patch.object(speed_ab, "civvis_processes", lambda: 0), \
             mock.patch.object(Path, "is_file", lambda self: True):
            code = speed_ab.main(["--baseline", "/x/a", "--candidate", "/x/b",
                                  "--games", "2"])
        self.assertEqual(code, 1)


class TheHostIsWatched(unittest.TestCase):
    def test_other_civvis_games_are_counted(self):
        lines = ("civvis simulate --seed 1\n"
                 "/usr/bin/python3 something_else.py\n"
                 "target/ci/ai_eval advanced advanced_v1\n")
        with mock.patch.object(speed_ab.subprocess, "run",
                               lambda *a, **k: mock.Mock(stdout=lines)):
            self.assertEqual(speed_ab.civvis_processes(), 2)

    def test_an_unremarkable_process_table_is_quiet(self):
        with mock.patch.object(speed_ab.subprocess, "run",
                               lambda *a, **k: mock.Mock(stdout="zsh\nFinder\n")):
            self.assertEqual(speed_ab.civvis_processes(), 0)


if __name__ == "__main__":
    unittest.main()
