#!/usr/bin/env python3
"""The split objective points at files that are really contended.

`docs/ROADMAP.md` objective 5 named `src/game.rs`, `src/ai/advanced.rs` and
`web/assets/app.js`. Measured over the 200 merges preceding 2026-08-18, the
third is touched by **one merge in fifty**, while `src/elo.rs` — unnamed — is
contended more often than `game.rs` despite being a seventh of its length. The
list was built from file size, and size is not the tax.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import conflict_hotspots  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
WORKFLOWS = REPO / ".github" / "workflows"

OBJECTIVE = """
5. **Relieve the measured conflict hotspots**

   | file | merges touching it | why |
   |---|---:|---|
   | `src/ai/advanced.rs` | 26% | size |

   Prose mentioning `tools/conflict_hotspots.py` is not a target.
6. **Something else**
"""


class TheObjectiveIsFound(unittest.TestCase):
    def test_it_is_anchored_on_the_phrase_not_the_verb(self):
        """It was called "Split the three conflict hotspots" while it named a
        file nobody edits; renaming it must not be a way to stop being checked.
        """
        for heading in ("**Split the three conflict hotspots**",
                        "**Relieve the measured conflict hotspots**",
                        "**Conflict hotspots, ranked**"):
            with self.subTest(heading=heading):
                text = f"\n5. {heading} something\n6. **Next** x\n"
                self.assertTrue(conflict_hotspots.roadmap_objective(text))

    def test_an_unrelated_objective_is_not_mistaken_for_it(self):
        self.assertEqual(
            conflict_hotspots.roadmap_objective(
                "\n5. **Delete measured-null code** x\n6. **Next** y\n"), "")

    def test_the_live_roadmap_still_states_one(self):
        self.assertTrue(conflict_hotspots.roadmap_objective(
            (REPO / "docs" / "ROADMAP.md").read_text(encoding="utf-8")))


class OnlyTheTargetTableCounts(unittest.TestCase):
    def test_prose_paths_are_not_treated_as_targets(self):
        """The tool that produced the ranking is named in the objective, and is
        not a thing anyone is being asked to go and split."""
        import re
        named = sorted({
            path
            for line in OBJECTIVE.splitlines() if line.lstrip().startswith("|")
            for path in re.findall(
                r"`([\w.-]+(?:/[\w.-]+)+\.(?:rs|js|py|sh))`", line)})
        self.assertEqual(named, ["src/ai/advanced.rs"])


def needs_history(count: int = 60):
    """Skip where the clone has no history to rank.

    ⚠ `origin/main` is often absent in a pull-request checkout, which is how
    the first CI run of this tool died. The tool falls back to `HEAD`; these
    tests still skip rather than fail on a genuinely shallow clone, because a
    depth-1 checkout is a fact about the runner and not a defect in the tree.
    """
    reachable = len(conflict_hotspots.recent_merges(count))
    return unittest.skipIf(reachable < count,
                           f"only {reachable} of {count} merges are reachable")


class TheRankingIsReal(unittest.TestCase):
    @needs_history()
    def test_it_ranks_something_and_ranks_it_by_merges(self):
        rows = conflict_hotspots.ranking(60)
        self.assertTrue(rows, "no file was touched by 3 of the last 60 merges")
        counts = [hits for _, hits, _ in rows]
        self.assertEqual(counts, sorted(counts, reverse=True))

    @needs_history()
    def test_generated_and_non_source_paths_are_left_out(self):
        """Their contention is answered by regenerating, not by splitting."""
        paths = [path for path, _, _ in conflict_hotspots.ranking(60)]
        self.assertEqual([p for p in paths if p.endswith(".md")], [])
        self.assertEqual([p for p in paths if p.endswith(".json")], [])

    @needs_history()
    def test_the_control_mod_is_rankable(self):
        """The omission that hid the fifth-most-contended file in the repository.

        The suffix filter read `(rs|js|py|sh)`, so
        `tools/civ6_control/mod/CivvisControlAgent.lua` — 21 of the last 200
        merges, 10%, ahead of three files the ranking did print — could not
        appear however contended it became. An absent file and an uncontended
        one print identically, so nothing about the omission was visible in the
        output.

        Pinned on the suffix rather than on the file's current rank: the rank
        moves with every merge, and what must not come back is a filter that
        cannot see a language the repository writes by hand.
        """
        import re as _re

        for hand_written in (
            "tools/civ6_control/mod/CivvisControlAgent.lua",
            "web/index.html",
            "src/game.rs",
            "tools/civ6_play.py",
        ):
            self.assertRegex(hand_written, conflict_hotspots.SOURCE_SUFFIXES)
        for generated in ("docs/EVAL_STATUS.md", "docs/eval_manifest.json"):
            self.assertIsNone(
                _re.search(conflict_hotspots.SOURCE_SUFFIXES, generated),
                f"{generated} is rewritten wholesale by a tool",
            )

    def test_a_missing_origin_main_falls_back_to_head(self):
        """The exact CI failure: the PR checkout had no `origin/main` ref."""
        real = conflict_hotspots.subprocess.run

        def only_head(args, **kwargs):
            if "origin/main" in args:
                return real(["git", "-C", str(conflict_hotspots.REPO),
                             "log", "does-not-exist", "--format=%H", "-1"],
                            **kwargs)
            return real(args, **kwargs)

        conflict_hotspots.subprocess.run = only_head
        try:
            self.assertTrue(conflict_hotspots.recent_merges(5))
        finally:
            conflict_hotspots.subprocess.run = real

    def test_a_shallow_clone_refuses_rather_than_passing_vacuously(self):
        """Every file reads 0% there, so the check would pass by measuring
        nothing — which is the failure a check exists to not have."""
        real = conflict_hotspots.recent_merges
        conflict_hotspots.recent_merges = lambda count: ["sha"] * 10
        try:
            self.assertEqual(conflict_hotspots.main(["--check"]), 1)
        finally:
            conflict_hotspots.recent_merges = real


class TheCheckIsWiredAndOneDirectional(unittest.TestCase):
    @needs_history()
    def test_the_live_roadmap_passes_its_own_check(self):
        self.assertEqual(conflict_hotspots.main(["--check"]), 0)

    def test_a_workflow_runs_it(self):
        """A guard nothing calls is the defect this repository keeps paying for."""
        ran = [path.name for path in WORKFLOWS.glob("*.yml")
               if "conflict_hotspots.py --check" in path.read_text()]
        self.assertTrue(ran, "no workflow runs conflict_hotspots.py --check")

    def test_that_workflow_fetches_enough_history_to_count_merges(self):
        """`actions/checkout` is shallow by default, and a shallow clone would
        make every file read 0% — the check would pass by measuring nothing."""
        for name in (path for path in WORKFLOWS.glob("*.yml")
                     if "conflict_hotspots.py --check" in path.read_text()):
            with self.subTest(workflow=name.name):
                self.assertIn("fetch-depth: 0", name.read_text())


if __name__ == "__main__":
    unittest.main()
