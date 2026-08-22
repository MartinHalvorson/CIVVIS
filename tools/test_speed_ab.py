#!/usr/bin/env python3
"""The paired harness refuses the conclusions the prose kept having to warn about."""

from __future__ import annotations

import re
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

    def test_disagreeing_arms_beat_any_overhead_claim(self):
        """A change that alters behaviour has no overhead measurement at all,
        however large or clean the timing difference looks.

        ⚠ CHANGED 2026-08-22, AND THE HALF THAT CHANGED IS NAMED HERE. This
        test used to assert `assertNotIn("-50", said)` — that the percentage
        is withheld entirely when the arms disagree. The claim above it is
        correct and is kept intact below. Withholding the NUMBER is what could
        not survive: a promoted feature changes play by construction, so the
        reports differ by construction, and #2059 — six times slower, the
        event `docs/SIMULATOR_PERFORMANCE.md` wrote the standing rule after —
        would have been answered with a refusal and no six.

        So both statements are made, and they are different statements: this
        is not overhead and no optimization claim survives it, AND this is
        what the changed behaviour costs.
        """
        said = speed_ab.verdict(self._result(100.0, 50.0, mismatched=[7]))
        self.assertIn("NOT a measure of overhead", said)
        self.assertIn("no optimization claim", said)
        self.assertIn("-50.00%", said)

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


class TheBudgetIsWhatMakesItAGate(unittest.TestCase):
    """`--budget` is the whole reason this harness can run unattended.

    Without it the exit code is non-zero exactly when the reports disagree —
    which is every pull request that changes how the agent plays, i.e. most of
    them. A gate wired that way is red on the ordinary case and silent on the
    dangerous one.
    """

    def _result(self, change, mismatched=()):
        return {"totals": {"baseline": 100.0, "candidate": 100.0 + change},
                "change_pct": change, "mismatched": list(mismatched),
                "seeds": [1]}

    def test_no_budget_never_fails_on_cost(self):
        self.assertFalse(speed_ab.over_budget(self._result(900.0, [1]), None))

    def test_inside_the_budget_passes(self):
        self.assertFalse(speed_ab.over_budget(self._result(49.9), 50.0))

    def test_outside_the_budget_fails(self):
        self.assertTrue(speed_ab.over_budget(self._result(50.1), 50.0))

    def test_a_regression_that_also_changes_play_is_still_a_regression(self):
        """#2059 in miniature, and the only case the gate exists for."""
        self.assertTrue(speed_ab.over_budget(self._result(515.0, [900001]),
                                             50.0))

    def test_getting_faster_is_never_over_budget(self):
        """The boring case, which is also almost every run. A gate that fires
        here is a gate somebody turns off within a day."""
        self.assertFalse(speed_ab.over_budget(self._result(-48.7, [1, 2]), 50.0))
        self.assertFalse(speed_ab.over_budget(self._result(0.0), 50.0))

    def test_an_improvement_does_not_read_as_a_regression(self):
        """It printed "done slower" for a −0.33% reading in the first
        end-to-end run of the new wording. Half of what this tool reports is
        an improvement."""
        self.assertIn("faster", speed_ab.verdict(self._result(-5.0)))
        self.assertNotIn("slower", speed_ab.verdict(self._result(-5.0)))
        self.assertIn("slower", speed_ab.verdict(self._result(+5.0)))


class TheGateIsActuallyWired(unittest.TestCase):
    """AGENTS.md: "a guard you add runs in the same change that adds it".

    `test_ci_wiring.py` checks that a tool claiming CI is *named* by some
    workflow. That is not enough here — naming it without `--budget` would run
    the harness and then ignore its verdict, the same defect one level down.
    """

    def test_a_workflow_runs_the_harness_with_a_budget(self):
        workflows = sorted(
            (Path(__file__).resolve().parent.parent
             / ".github" / "workflows").glob("*.yml"))
        text = "\n".join(
            "\n".join(line.partition("#")[0]
                      for line in path.read_text(encoding="utf-8").splitlines())
            for path in workflows)
        # ⚠ Anchored on `python3 …`, not on the bare path. The first draft
        # matched `- 'tools/speed_ab.py'` in the workflow's own `paths:` filter
        # and read the trailing quote as the argument list — a check that
        # passed by finding the file's NAME while the step that runs it could
        # have been deleted. Verified by deleting the step: it now fails.
        invocation = re.search(
            r"python3 tools/speed_ab\.py(?P<args>(?:[^\n]*\\\n)*[^\n]*)", text)
        self.assertIsNotNone(
            invocation,
            "tools/speed_ab.py is named in a workflow but never executed")
        self.assertIn("--budget", invocation.group("args"),
                      "the harness runs but nothing reads its verdict")


if __name__ == "__main__":
    unittest.main()
