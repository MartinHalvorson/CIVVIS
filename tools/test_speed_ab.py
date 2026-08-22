#!/usr/bin/env python3
"""The speed gate's verdicts, and the wiring that makes them a check.

⚠⚠ THE DEFECT THIS SUITE EXISTS FOR IS AN ABSTENTION, NOT A WRONG ANSWER.
`speed_ab.py` used to answer the disagreeing case — the reports differ, so the
arms played different games — by withholding the percentage entirely and saying
no claim could be made. That is correct about *overhead* and catastrophic as a
gate, because a promoted feature changes play by construction. Run against
#2059, which made every simulation six times slower, the old wording would have
printed a refusal and not the six.

So the tests below are mostly about the case that returns "I cannot tell you":
they check that it now tells you anyway, in the right vocabulary.
"""

import importlib.util
import re
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
WORKFLOWS = REPO / ".github" / "workflows"


def load():
    spec = importlib.util.spec_from_file_location(
        "speed_ab", REPO / "tools" / "speed_ab.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


speed_ab = load()


def result(change, mismatched=()):
    return {"change_pct": change, "mismatched": list(mismatched),
            "totals": {"baseline": 100.0, "candidate": 100.0 + change},
            "seeds": [1, 2, 3]}


class TheDisagreeingCaseStillReportsItsNumber(unittest.TestCase):
    """#2059 in miniature: play changed AND cost exploded."""

    def test_a_behaviour_change_reports_the_percentage(self):
        said = speed_ab.verdict(result(+515.0, mismatched=[900001]))
        self.assertIn("+515.00%", said,
                      "a promoted feature changes play by construction; if the "
                      "gate withholds the number exactly then, it withholds it "
                      "exactly when it matters")

    def test_a_behaviour_change_still_refuses_the_overhead_claim(self):
        said = speed_ab.verdict(result(+515.0, mismatched=[900001]))
        self.assertIn("NOT a measure of overhead", said)
        self.assertIn("FEATURE COST", said)

    def test_a_behaviour_change_is_judged_against_the_budget(self):
        self.assertTrue(speed_ab.over_budget(result(+515.0, [900001]), 50.0),
                        "a regression that also changes play is still a "
                        "regression, and is the kind this repository shipped")


class TheBudget(unittest.TestCase):
    def test_no_budget_never_fails(self):
        self.assertFalse(speed_ab.over_budget(result(+900.0, [1]), None))

    def test_inside_the_budget_passes(self):
        self.assertFalse(speed_ab.over_budget(result(+49.9), 50.0))

    def test_outside_the_budget_fails(self):
        self.assertTrue(speed_ab.over_budget(result(+50.1), 50.0))

    def test_getting_faster_is_never_over_budget(self):
        """The boring case, which is also 99% of the runs: a PR that costs
        nothing. A gate that fires here would be turned off within a day."""
        self.assertFalse(speed_ab.over_budget(result(-48.7, [1, 2]), 50.0))
        self.assertFalse(speed_ab.over_budget(result(0.0), 50.0))


class TheAgreeingCaseIsUnchanged(unittest.TestCase):
    def test_inside_the_floor_is_noise(self):
        said = speed_ab.verdict(result(+0.1))
        self.assertIn("NOISE", said)

    def test_outside_the_floor_is_overhead(self):
        said = speed_ab.verdict(result(+5.0))
        self.assertIn("overhead", said)
        self.assertIn("+5.00%", said)


class TheGateIsActuallyWired(unittest.TestCase):
    """AGENTS.md: "a guard you add runs in the same change that adds it".

    `test_ci_wiring.py` checks that a tool claiming CI is *named* by some
    workflow. That is not enough here: naming it without `--budget` would run
    the harness and then ignore its verdict, which is the same shape of defect
    one level down.
    """

    def test_a_workflow_runs_the_harness_with_a_budget(self):
        text = "\n".join(
            "\n".join(line.partition("#")[0] for line in
                      path.read_text(encoding="utf-8").splitlines())
            for path in sorted(WORKFLOWS.glob("*.yml")))
        self.assertIn("tools/speed_ab.py", text,
                      "no workflow runs the paired speed harness")
        # ⚠ Anchored on `python3 …`, not on the bare path. The first draft
        # matched `- 'tools/speed_ab.py'` in the workflow's own `paths:`
        # filter and read the trailing quote as the argument list — a check
        # that passed by finding the file's NAME while the step that runs it
        # could have been deleted. Its own suite caught it; the anchor is why
        # it stays caught.
        invocation = re.search(
            r"python3 tools/speed_ab\.py(?P<args>(?:[^\n]*\\\n)*[^\n]*)", text)
        self.assertIsNotNone(
            invocation,
            "tools/speed_ab.py is named in a workflow but never executed")
        self.assertIn("--budget", invocation.group("args"),
                      "the harness runs but nothing reads its verdict: without "
                      "--budget it exits non-zero only when the reports "
                      "disagree, which is every AI pull request")


if __name__ == "__main__":
    unittest.main()
