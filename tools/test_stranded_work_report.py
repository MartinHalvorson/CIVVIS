#!/usr/bin/env python3
"""The stranded-work report has to be short enough to act on.

Its rescue-snapshot section listed every snapshot the worktree audit had ever
preserved — 151 of them on 2026-08-18 — in one undifferentiated wall, and the
wall is why the section was never drained. The overwhelming majority are the
preserved history of work that DID land, and nothing distinguished them from the
few that hold something nobody merged. Resolving each snapshot's branch against
GitHub's merged pull requests cut the same day's list from 151 to 26.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import stranded_work_report as report  # noqa: E402


SNAPSHOTS = "\n".join(
    [
        "refs/civvis/wip/agent/a/landed\t2026-08-05T10:00:00+00:00\twip snapshot of landed",
        "refs/civvis/wip/agent/a/stranded\t2026-08-05T11:00:00+00:00\twip snapshot of stranded",
        "refs/civvis/wip/agent/b/also-landed\t2026-08-06T09:00:00+00:00\twip snapshot of also-landed",
    ]
)


def fake_git(*args: str) -> str:
    if args[0] == "for-each-ref":
        return SNAPSHOTS
    if args[0] == "diff":
        return " 1 file changed, 5 insertions(+)"
    return ""


class TheSnapshotSectionIsAQueue(unittest.TestCase):
    def setUp(self):
        patch = mock.patch.object(report, "git", fake_git)
        patch.start()
        self.addCleanup(patch.stop)

    def rows(self, merged):
        with mock.patch.object(report, "merged_branches", lambda: set(merged)):
            return report.rescue_refs()

    def test_a_snapshot_whose_branch_merged_is_not_listed(self):
        rows = self.rows({"agent/a/landed", "agent/b/also-landed"})
        listed = [r for r in rows if r.startswith("- ")]
        self.assertEqual(len(listed), 1, rows)
        self.assertIn("agent/a/stranded", listed[0])

    def test_the_work_that_landed_is_counted_not_hidden(self):
        rows = self.rows({"agent/a/landed", "agent/b/also-landed"})
        tail = rows[-1]
        self.assertIn("2 further snapshot(s)", tail)
        self.assertIn("merged", tail)

    def test_nothing_merged_means_everything_is_still_listed(self):
        rows = self.rows(set())
        listed = [r for r in rows if r.startswith("- ")]
        self.assertEqual(len(listed), 3)
        self.assertFalse(
            any("further snapshot" in r for r in rows),
            "there is nothing to count when nothing landed",
        )

    def test_github_being_unreachable_still_produces_a_report(self):
        """A report that cannot reach GitHub reports everything, not nothing.

        The section's value is the short list, but degrading to the old wall is
        strictly better than a scheduled job that throws and posts no report at
        all — the failure would look exactly like "nothing is stranded".
        """
        def explode():
            raise RuntimeError("GitHub is down")

        with mock.patch.object(report, "merged_branches", explode):
            rows = report.rescue_refs()
        listed = [r for r in rows if r.startswith("- ")]
        self.assertEqual(len(listed), 3)


class ItDoesNotJudgeBySquashedDiff(unittest.TestCase):
    """A snapshot of work that landed still shows its whole diff against main.

    The fleet squash-merges, so the merge-base diff of a snapshot never empties
    just because the work landed — measured 2026-08-18, 150 of 151 snapshots
    still carried a diff, including the 125 whose pull requests had merged.
    Judging by that diff would call almost everything stranded, which is as
    useless as calling nothing stranded.
    """

    def test_the_reporter_asks_github_rather_than_diffing_against_main(self):
        source = Path(report.__file__).read_text(encoding="utf-8")
        self.assertIn("def merged_branches", source)
        # Scoped to `rescue_refs`. `idle_branches` legitimately diffs a branch
        # against main — it is asking how much work a branch holds, not whether
        # that work landed, and those are different questions.
        body = source.split("def rescue_refs")[1].split("\ndef ")[0]
        self.assertNotIn(
            "origin/main...",
            body,
            "rescue_refs judges a snapshot by its branch's pull request, not by "
            "a diff that a squash merge never empties",
        )


class TheBranchLookupIsPaged(unittest.TestCase):
    def test_one_listing_not_a_call_per_snapshot(self):
        """151 snapshots must not become 151 API requests on a scheduled job."""
        calls: list[str] = []

        def fake_api(path, method="GET", data=None):
            calls.append(path)
            # ⚠ `"page=1" in path` is TRUE for every page, because `per_page=100`
            # contains `page=100` which contains `page=1`. The first draft of
            # this fake asserted a paged loop and drove it to its ten-page cap.
            if path.endswith("&page=1"):
                return [
                    {"merged_at": "2026-08-05T00:00:00Z", "head": {"ref": "agent/a/landed"}},
                    {"merged_at": None, "head": {"ref": "agent/a/stranded"}},
                ]
            return []

        with mock.patch.object(report, "api", fake_api):
            merged = report.merged_branches()
        self.assertEqual(merged, {"agent/a/landed"})
        self.assertLessEqual(len(calls), 3, calls)
        self.assertTrue(all("per_page=100" in c for c in calls), calls)


if __name__ == "__main__":
    unittest.main()
