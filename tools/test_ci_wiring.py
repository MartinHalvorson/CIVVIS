#!/usr/bin/env python3
"""A tool whose own documentation says it runs in CI must actually run in CI.

⚠⚠ THIS IS THE SHAPE OF DEFECT THAT KEEPS COSTING THIS REPOSITORY, and it is
invisible to every other gate because nothing is red — the check simply is not
running.

`tools/civvis_inert.py` documented ``--max 0  # CI ratchet`` in its usage block
from the day it was written and no workflow ever called it. It needs nothing but
the repository, so nothing stood in the way; it was just never wired in. What it
would have reported, the whole time: Poland's Winged Hussar carried
``force_retreat`` in ``data/units.json`` and the engine read that key nowhere, so
the unit was a plain Cuirassier replacement with no unique ability at all
(#1941, #1943).

That is not the only instance. The same session found `tools/civ6_fidelity.py`
printing "(Gathering Storm load order)" as a hard-coded claim while auditing a
Vanilla database (#1946), the live harness setting ``--ruleset`` and never
reading it back (#1947), and `the_same_seed_generates_the_same_world_on_every_platform`
pinning one map script while ten platform-trig calls sat in the default one
(#1950). Four defects, one root: **something asserted, nothing checking**.

This gate closes the sub-case that is mechanically checkable. A tool may still
be unrunnable on a CI runner — most of the Civilization VI tools need the game
installed — but that has to be *written down* rather than left as an omission
nobody can tell apart from an oversight.
"""

from __future__ import annotations

import re
import unittest
from fnmatch import fnmatch
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TOOLS = REPO / "tools"
WORKFLOWS = REPO / ".github" / "workflows"

# A tool advertises CI use in its own header when it says so in the first lines
# a reader sees. Deliberately matched on the documentation rather than on the
# code: the claim is what creates the expectation.
CLAIMS_CI = re.compile(r"CI ratchet|\bin CI\b|CI gate|for CI\b", re.IGNORECASE)
HEADER_LINES = 40

# Tools that document CI use and genuinely cannot run on a CI runner. Each says
# why, so an omission and a decision cannot be mistaken for one another.
#
# ⚠ Shrinking this dict is the point. A tool listed here is a check the fleet
# does not get; if one becomes runnable — a fixture lands, a cache is committed —
# it belongs in a workflow and out of this list.
CANNOT_RUN_IN_CI = {
    "civ6_differential.py":
        "Compares two replay traces produced by a real bridged game and a "
        "headless run; neither artifact exists on a runner.",
    "civ6_modifiers.py":
        "Censuses the shipped Civ VI Modifiers tables, which need the game's "
        "own database.",
    "civ6_player_colors.py":
        "Reads Civ VI's player-colour tables from the install.",
    "civ6_yield_drift.py":
        "Reads a live mirror's per-turn economy drift from a bridged game.",
    "civvis_push_guard.py":
        "A pre-push hook for development clones. CI has already received the "
        "push it exists to gate.",
    "landing_battles.py":
        "Regenerates a file by building and serving the crate; the committed "
        "output is pinned by server.rs's string tests instead.",
}


# `collaboration-policy.yml` runs `unittest discover -s tools -p 'test_*.py'`,
# so a suite matching that pattern is wired by discovery and needs no name in a
# workflow. Detected rather than assumed: if that step is ever narrowed or
# removed, these files stop counting as wired and this gate says so.
DISCOVERY = re.compile(r"unittest discover -s tools -p '(?P<pattern>[^']+)'")


def discovery_pattern() -> str | None:
    found = DISCOVERY.search(workflow_text())
    return found.group("pattern") if found else None


def workflow_text() -> str:
    """Every workflow, with comments stripped.

    ⚠ THIS GATE'S FIRST DRAFT SEARCHED THE RAW FILES AND SO GRADED ITSELF ON A
    COMMENT. `.github/workflows/tests.yml` explains the inert ratchet in prose
    above the step that runs it, so deleting the step left the tool's name still
    in the file and the check still passing — the exact "looks wired, is not"
    failure this exists to catch, reproduced inside it. Verified by deleting the
    step: without this stripping the gate stays green.
    """
    stripped = []
    for path in sorted(WORKFLOWS.glob("*.yml")):
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            head, _, _ = line.partition("#")
            stripped.append(head)
    return "\n".join(stripped)


def tools_claiming_ci() -> list[str]:
    claiming = []
    for path in sorted(TOOLS.glob("*.py")):
        header = "\n".join(
            path.read_text(encoding="utf-8", errors="replace").splitlines()[:HEADER_LINES])
        if CLAIMS_CI.search(header):
            claiming.append(path.name)
    return claiming


class AToolThatSaysItRunsInCiRunsInCi(unittest.TestCase):
    def test_every_tool_claiming_ci_is_wired_or_says_why_not(self):
        workflows = workflow_text()
        pattern = discovery_pattern()
        unwired = [
            name
            for name in tools_claiming_ci()
            if name not in workflows
            and name not in CANNOT_RUN_IN_CI
            and not (pattern and fnmatch(name, pattern))
        ]
        self.assertEqual(
            unwired, [],
            "these tools document CI use and no workflow calls them. Wire them "
            "into .github/workflows/, or add them to CANNOT_RUN_IN_CI with the "
            "reason a runner cannot execute them. An unwired check is not a "
            "check — civvis_inert.py sat unwired long enough to hide a shipped "
            "unit with no ability (#1943).")

    def test_a_suite_collected_by_discovery_counts_as_wired(self):
        """This file is the case in point: no workflow names it, and
        `unittest discover -s tools` runs it on every pull request."""
        pattern = discovery_pattern()
        self.assertIsNotNone(
            pattern, "no workflow discovers tools/ suites any more; that is the "
                     "regression collaboration-policy.yml was fixed to prevent")
        self.assertTrue(fnmatch("test_ci_wiring.py", pattern))
        self.assertFalse(fnmatch("civvis_inert.py", pattern),
                         "a plain tool is not a discovered suite")

    def test_a_waiver_goes_stale_when_its_tool_gets_wired(self):
        """A tool that reaches a workflow must leave this list, or the list stops
        describing anything and starts hiding the next omission."""
        workflows = workflow_text()
        stale = sorted(name for name in CANNOT_RUN_IN_CI if name in workflows)
        self.assertEqual(
            stale, [],
            "these are wired into a workflow and still carry a cannot-run "
            "waiver; drop the rows")

    def test_every_waiver_names_a_tool_that_exists(self):
        missing = sorted(
            name for name in CANNOT_RUN_IN_CI if not (TOOLS / name).is_file())
        self.assertEqual(missing, [], "waivers for tools that are gone")

    def test_every_waiver_gives_a_reason(self):
        for name, reason in CANNOT_RUN_IN_CI.items():
            with self.subTest(tool=name):
                self.assertGreater(
                    len(reason), 40,
                    f"{name}'s waiver has to say why a runner cannot execute it")

    def test_the_detector_reads_documentation_not_code(self):
        """The claim is what creates the expectation, so the match is on the
        header a reader sees rather than on anything the tool does."""
        self.assertTrue(CLAIMS_CI.search("    --max 0    # CI ratchet"))
        self.assertTrue(CLAIMS_CI.search("floors it in CI"))
        self.assertIsNone(CLAIMS_CI.search("this is a scientific measurement"))

    def test_the_gate_can_see_the_tool_it_was_written_for(self):
        """civvis_inert.py is the reason this file exists; if the detector stops
        recognising it, the gate has quietly stopped working."""
        self.assertIn("civvis_inert.py", tools_claiming_ci())


if __name__ == "__main__":
    unittest.main()
