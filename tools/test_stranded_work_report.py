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

import datetime
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


class ARunThatNeverEndsIsStrandedWork(unittest.TestCase):
    """`release.yml` run 31116714949, as a test.

    A re-run of the `v0.6.1` tag build sat `queued` with zero jobs allocated
    from 2026-08-06 to 2026-08-22 — 390 hours. `timeout-minutes` cannot bound
    that (its clock starts when a runner claims a job, and this run had none),
    so the only thing that would ever have surfaced it is a report that asks.
    """

    NOW = datetime.datetime(2026, 8, 23, 0, 0, 0)

    def payloads(self, runs):
        def fake_api(path, *args, **kwargs):
            status = "queued" if "status=queued" in path else "in_progress"
            return {"workflow_runs": [r for r in runs if r["_status"] == status]}
        return mock.patch.object(report, "api", fake_api)

    def run_row(self, **over):
        row = {"_status": "queued", "id": 31116714949, "name": "release",
               "html_url": "https://example.invalid/31116714949",
               "head_branch": "v0.6.1", "event": "push",
               "display_title": "Publish a lane whose revision predates assets",
               "created_at": "2026-08-06T15:37:40Z",
               "run_started_at": "2026-08-06T20:12:32Z"}
        row.update(over)
        return row

    def test_the_390_hour_run_is_reported(self):
        with self.payloads([self.run_row()]):
            rows = report.stuck_runs(self.NOW)
        self.assertEqual(len(rows), 1, rows)
        self.assertIn("31116714949", rows[0])
        self.assertIn("queued", rows[0])
        self.assertIn("16.2d", rows[0])

    def test_a_run_inside_the_window_is_not_reported(self):
        fresh = self.run_row(run_started_at="2026-08-22T23:00:00Z")
        with self.payloads([fresh]):
            self.assertEqual(report.stuck_runs(self.NOW), [])

    def test_a_long_in_progress_run_is_reported_too(self):
        """The other non-terminal state; a wedged job hangs the same way."""
        stuck = self.run_row(_status="in_progress", id=7,
                             run_started_at="2026-08-20T00:00:00Z")
        with self.payloads([stuck]):
            rows = report.stuck_runs(self.NOW)
        self.assertEqual(len(rows), 1, rows)
        self.assertIn("in_progress", rows[0])

    def test_a_stuck_run_alone_reopens_the_issue(self):
        """It is actionable, so it must not be a footnote like a snapshot is.

        `upsert` reopens a closed report only when `actionable` is true. A
        stuck run that could not set it would be written into an issue nobody
        gets notified about — which is how this one lasted 390 hours.
        """
        with mock.patch.object(report, "commentless_closes", lambda now: []), \
             mock.patch.object(report, "idle_branches", lambda now: []), \
             mock.patch.object(report, "rescue_refs", lambda: []), \
             mock.patch.object(report, "stuck_runs", lambda now: ["- a stuck run"]):
            body, actionable = report.compose(self.NOW)
        self.assertTrue(actionable)
        self.assertIn("Workflow runs that never ended", body)
        self.assertIn("- a stuck run", body)

    def test_an_empty_run_list_leaves_the_report_quiet(self):
        with mock.patch.object(report, "commentless_closes", lambda now: []), \
             mock.patch.object(report, "idle_branches", lambda now: []), \
             mock.patch.object(report, "rescue_refs", lambda: []), \
             mock.patch.object(report, "stuck_runs", lambda now: []):
            body, actionable = report.compose(self.NOW)
        self.assertFalse(actionable)
        self.assertIn("Nothing stranded here today.", body)

    def test_github_losing_the_actions_api_still_produces_a_report(self):
        def boom(path, *args, **kwargs):
            raise RuntimeError("actions API unavailable")
        with mock.patch.object(report, "api", boom):
            self.assertEqual(report.stuck_runs(self.NOW), [])


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
