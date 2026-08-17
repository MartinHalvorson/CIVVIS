import datetime
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch


MODULE_PATH = Path(__file__).with_name("stranded_work_report.py")
SPEC = importlib.util.spec_from_file_location("stranded_work_report", MODULE_PATH)
assert SPEC and SPEC.loader
report = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(report)


class RecordingApi:
    """Stand-in for report.api that scripts the issue search response."""

    def __init__(self, issues):
        self.issues = issues
        self.calls = []

    def __call__(self, path, method="GET", data=None):
        self.calls.append((path, method, data))
        if method == "GET":
            return self.issues
        if method == "POST":
            return {"number": 999}
        return {}

    def writes(self):
        return [call for call in self.calls if call[1] != "GET"]


def issue(number, state):
    return {"number": number, "state": state, "title": report.ISSUE_TITLE}


class UpsertTests(unittest.TestCase):
    def test_search_covers_closed_issues_so_a_close_does_not_fork_the_timeline(self):
        api = RecordingApi([issue(1266, "closed")])
        with patch.object(report, "api", api):
            report.upsert("body", actionable=False)
        (search, _, _) = api.calls[0]
        self.assertIn("state=all", search)
        (path, method, data) = api.writes()[0]
        self.assertIn("/issues/1266", path)
        self.assertEqual(method, "PATCH")
        self.assertEqual(data, {"body": "body"})

    def test_actionable_rows_reopen_a_closed_issue(self):
        api = RecordingApi([issue(1266, "closed")])
        with patch.object(report, "api", api):
            report.upsert("body", actionable=True)
        (_, _, data) = api.writes()[0]
        self.assertEqual(data, {"body": "body", "state": "open"})

    def test_a_quiet_update_leaves_the_operator_close_in_place(self):
        api = RecordingApi([issue(1266, "closed")])
        with patch.object(report, "api", api):
            report.upsert("body", actionable=False)
        (_, _, data) = api.writes()[0]
        self.assertNotIn("state", data)

    def test_an_open_issue_is_never_patched_with_state(self):
        api = RecordingApi([issue(1266, "open")])
        with patch.object(report, "api", api):
            report.upsert("body", actionable=True)
        (_, _, data) = api.writes()[0]
        self.assertEqual(data, {"body": "body"})

    def test_no_issue_at_all_creates_one(self):
        api = RecordingApi([])
        with patch.object(report, "api", api):
            report.upsert("body", actionable=False)
        (path, method, data) = api.writes()[0]
        self.assertTrue(path.endswith("/issues"))
        self.assertEqual(method, "POST")
        self.assertEqual(data["title"], report.ISSUE_TITLE)

    def test_other_issues_wearing_the_label_are_not_the_report(self):
        stray = {"number": 7, "state": "open", "title": "Something else"}
        api = RecordingApi([stray, issue(1266, "closed")])
        with patch.object(report, "api", api):
            report.upsert("body", actionable=True)
        (path, _, _) = api.writes()[0]
        self.assertIn("/issues/1266", path)


class ComposeTests(unittest.TestCase):
    NOW = datetime.datetime(2026, 8, 7, 12, 0)

    def compose(self, closes=(), idle=(), rescue=()):
        with (
            patch.object(report, "commentless_closes", lambda now: list(closes)),
            patch.object(report, "idle_branches", lambda now: list(idle)),
            patch.object(report, "rescue_refs", lambda: list(rescue)),
        ):
            return report.compose(self.NOW)

    def test_rescue_snapshots_alone_are_preserved_history_not_a_summons(self):
        body, actionable = self.compose(rescue=["- `wip/snapshot` — kept"])
        self.assertFalse(actionable)
        self.assertIn("wip/snapshot", body)

    def test_a_commentless_close_is_actionable(self):
        _, actionable = self.compose(closes=["- #1 closed with no stated reason"])
        self.assertTrue(actionable)

    def test_an_idle_branch_is_actionable(self):
        _, actionable = self.compose(idle=["- `agent/x` idle 2.0d"])
        self.assertTrue(actionable)

    def test_an_empty_report_still_writes_every_section(self):
        body, actionable = self.compose()
        self.assertFalse(actionable)
        self.assertEqual(body.count("Nothing stranded here today."), 3)

    def test_rows_link_directly_to_the_pr_and_branch_for_triage(self):
        with (
            patch.object(
                report,
                "commentless_closes",
                lambda now: [
                    "- [#42](https://github.com/MartinHalvorson/CIVVIS/pull/42) closed"
                ],
            ),
            patch.object(
                report,
                "idle_branches",
                lambda now: [
                    "- [agent/example/task]"
                    "(https://github.com/MartinHalvorson/CIVVIS/tree/"
                    "agent/example/task) idle"
                ],
            ),
            patch.object(report, "rescue_refs", lambda: []),
        ):
            body, _ = report.compose(self.NOW)
        self.assertIn("github.com/MartinHalvorson/CIVVIS/pull/42", body)
        self.assertIn(
            "github.com/MartinHalvorson/CIVVIS/tree/agent/example/task", body
        )


class LinkTests(unittest.TestCase):
    def test_github_url_encodes_agent_branch_slashes(self):
        self.assertEqual(
            report.github_url("tree", "agent/example/task"),
            "https://github.com/MartinHalvorson/CIVVIS/tree/agent/example/task",
        )


if __name__ == "__main__":
    unittest.main()
