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


class TheRankingIsReal(unittest.TestCase):
    def test_it_ranks_something_and_ranks_it_by_merges(self):
        rows = conflict_hotspots.ranking(60)
        self.assertTrue(rows, "no file was touched by 3 of the last 60 merges")
        counts = [hits for _, hits, _ in rows]
        self.assertEqual(counts, sorted(counts, reverse=True))

    def test_generated_and_non_source_paths_are_left_out(self):
        """Their contention is answered by regenerating, not by splitting."""
        paths = [path for path, _, _ in conflict_hotspots.ranking(60)]
        self.assertEqual([p for p in paths if p.endswith(".md")], [])
        self.assertEqual([p for p in paths if p.endswith(".json")], [])


class TheCheckIsWiredAndOneDirectional(unittest.TestCase):
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
