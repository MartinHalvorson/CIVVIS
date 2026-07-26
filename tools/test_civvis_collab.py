from pathlib import Path
import concurrent.futures
import os
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civvis_collab as collab
import civvis_push_guard as push_guard


def pr(branch, body, *, number=9, draft=True):
    return {
        "number": number,
        "headRefName": branch,
        "body": body,
        "isDraft": draft,
    }


def body(
    machine="render-win-02",
    agent="codex-47",
    paths="`src/game.rs`, `data/**`",
    coordinated="none",
    checked=True,
):
    mark = "x" if checked else " "
    return f"""## Ownership claim

- Machine ID: `{machine}`
- Agent/session ID: `{agent}`
- Task: government cleanup
- Claimed paths: {paths}
- Coordinated with: {coordinated}

## Validation

- [{mark}] Branch started from current `origin/main`
"""


class BranchTests(unittest.TestCase):
    def test_launcher_and_push_guard_branch_formats_stay_in_sync(self):
        self.assertEqual(collab.BRANCH_RE.pattern, push_guard.BRANCH_RE.pattern)

    def test_fleet_branch_is_accepted(self):
        value = "agent/render-win-02/codex-47/government-cleanup-20260723T210500Z-a31f"
        self.assertIsNotNone(collab.BRANCH_RE.fullmatch(value))

    def test_ambiguous_legacy_branch_is_rejected(self):
        self.assertIsNone(collab.BRANCH_RE.fullmatch("agent/government-cleanup"))

    def test_remote_heads_are_parsed_without_symbolic_refs(self):
        raw = (
            "abc123\trefs/heads/main\n"
            "def456\trefs/heads/agent/render-win-02/codex-47/task-20260723T210500Z-a31f\n"
            "999999\trefs/tags/v1\n"
        )
        self.assertEqual(
            collab.parse_remote_heads(raw),
            {
                "main": "abc123",
                "agent/render-win-02/codex-47/task-20260723T210500Z-a31f": "def456",
            },
        )

    def test_shared_push_guard_installs_and_audits_current(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(("git", "init", str(root)), check=True, capture_output=True)
            tools = root / "tools"
            tools.mkdir()
            source = tools / "civvis_push_guard.py"
            source.write_text(
                f"#!/usr/bin/env python3\n# {collab.PUSH_GUARD_MARKER}\n",
                encoding="utf-8",
            )
            target = collab.install_push_guard(root)
            self.assertEqual(target.read_bytes(), source.read_bytes())
            self.assertIsNone(collab.push_guard_error(root))
            if os.name != "nt":
                self.assertTrue(os.access(target, os.X_OK))

    def test_installer_preserves_an_unmanaged_hook(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(("git", "init", str(root)), check=True, capture_output=True)
            tools = root / "tools"
            tools.mkdir()
            (tools / "civvis_push_guard.py").write_text(
                f"# {collab.PUSH_GUARD_MARKER}\n", encoding="utf-8"
            )
            target = collab.common_git_dir(root) / "hooks" / "pre-push"
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            with self.assertRaises(collab.CommandError):
                collab.install_push_guard(root)
            self.assertEqual(target.read_text(encoding="utf-8"), "#!/bin/sh\nexit 0\n")

    def test_simultaneous_installers_leave_one_complete_guard(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(("git", "init", str(root)), check=True, capture_output=True)
            tools = root / "tools"
            tools.mkdir()
            source = tools / "civvis_push_guard.py"
            source.write_text(
                f"#!/usr/bin/env python3\n# {collab.PUSH_GUARD_MARKER}\n",
                encoding="utf-8",
            )
            with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
                targets = list(pool.map(lambda _: collab.install_push_guard(root), range(24)))
            self.assertEqual(len(set(targets)), 1)
            self.assertEqual(targets[0].read_bytes(), source.read_bytes())
            self.assertEqual(list(targets[0].parent.glob(".pre-push.civvis-*")), [])


class ClaimTests(unittest.TestCase):
    def test_claims_are_parsed_from_the_pr_contract(self):
        parsed = collab.parse_claims(body())
        self.assertEqual(parsed["machine"], "render-win-02")
        self.assertEqual(parsed["agent"], "codex-47")
        self.assertEqual(collab.split_paths(parsed["paths"]), ["src/game.rs", "data/**"])

    def test_glob_and_prefix_claims_overlap(self):
        self.assertTrue(collab.claim_patterns_overlap("data/**", "data/units.json"))
        self.assertFalse(collab.claim_patterns_overlap("data/**", "web/index.html"))

    def test_root_wide_and_parent_traversal_claims_are_rejected(self):
        self.assertFalse(collab.valid_claim_pattern("**"))
        self.assertFalse(collab.valid_claim_pattern("../src/game.rs"))


class HunkTests(unittest.TestCase):
    def test_base_side_ranges_are_read_from_hunk_headers(self):
        patch = (
            "@@ -10,5 +10,7 @@ fn one()\n"
            " ctx\n-old\n+new\n"
            "@@ -200 +202 @@ fn two()\n"
            "-single\n+single\n"
        )
        self.assertEqual(collab.patch_base_ranges(patch), [(10, 14), (200, 200)])

    def test_a_pure_insertion_is_a_zero_width_range_at_the_seam(self):
        self.assertEqual(collab.patch_base_ranges("@@ -40,0 +41,3 @@\n"), [(40, 40)])

    def test_nearby_edits_touch_because_git_needs_context(self):
        self.assertTrue(collab.ranges_touch([(10, 20)], [(22, 30)]))
        self.assertFalse(collab.ranges_touch([(10, 20)], [(40, 50)]))

    def test_an_unreadable_patch_falls_back_to_whole_file(self):
        self.assertTrue(collab.file_edits_collide(None, [(1, 2)]))
        self.assertTrue(collab.file_edits_collide([(1, 2)], None))
        self.assertFalse(collab.file_edits_collide([(1, 2)], [(90, 95)]))

    def test_the_cli_range_reader_matches_the_api_reader(self):
        rows = [
            {"filename": "web/index.html", "patch": "@@ -12,4 +12,6 @@\n-a\n+b\n"},
            {"filename": "assets/logo.png"},
        ]
        with patch.object(collab, "gh_json", return_value=rows):
            ranges = collab.gh_pr_file_ranges(7)
        # A readable patch yields ranges; a binary file stays None so the
        # audit falls back to whole-file exactly like check-pr does.
        self.assertEqual(ranges["web/index.html"], [(12, 15)])
        self.assertIsNone(ranges["assets/logo.png"])

    def test_only_genuinely_colliding_paths_are_reported(self):
        mine = {"a.rs": [(1, 10)], "b.rs": [(1, 10)], "c.rs": [(1, 10)]}
        theirs = {"a.rs": [(5, 15)], "b.rs": [(500, 510)]}
        self.assertEqual(collab.colliding_paths(mine, theirs), ["a.rs"])


class PolicyTests(unittest.TestCase):
    branch = "agent/render-win-02/codex-47/government-cleanup-20260723T210500Z-a31f"

    def test_valid_draft_claim_passes(self):
        errors = collab.validate_pr(
            pr(self.branch, body()),
            files=["src/game.rs", "data/governments.json"],
            commit_subjects=["claim: government cleanup", "Fix government cleanup"],
        )
        self.assertEqual(errors, [])

    def test_branch_and_body_identity_must_match(self):
        errors = collab.validate_pr(
            pr(self.branch, body(machine="other-host")),
            files=["src/game.rs"],
            commit_subjects=[],
        )
        self.assertIn("Machine ID must match the branch machine component", errors)

    def test_every_changed_file_must_be_claimed(self):
        errors = collab.validate_pr(
            pr(self.branch, body()),
            files=["web/index.html"],
            commit_subjects=[],
        )
        self.assertTrue(
            any("changed path is not claimed: web/index.html" in e for e in errors)
        )

    def test_autosync_commits_are_forbidden(self):
        errors = collab.validate_pr(
            pr(self.branch, body()),
            files=["src/game.rs"],
            commit_subjects=["autosync: workstation checkpoint"],
        )
        self.assertTrue(any("autosync commit" in error for error in errors))

    def test_unknown_patches_collide_conservatively_on_a_ready_pr(self):
        errors = collab.validate_pr(
            pr(self.branch, body(), draft=False),
            files=["src/game.rs"],
            commit_subjects=[],
            other_files={5: {"src/game.rs"}},
        )
        self.assertTrue(any("collide with PR #5" in error for error in errors))
        coordinated = collab.validate_pr(
            pr(self.branch, body(coordinated="#5"), draft=False),
            files=["src/game.rs"],
            commit_subjects=[],
            other_files={5: {"src/game.rs"}},
        )
        self.assertEqual(coordinated, [])

    def test_a_draft_reports_collisions_without_failing(self):
        advisories = []
        errors = collab.validate_pr(
            pr(self.branch, body()),
            files=["src/game.rs"],
            commit_subjects=[],
            other_files={5: {"src/game.rs"}},
            advisories=advisories,
        )
        self.assertEqual(errors, [])
        self.assertTrue(any("collide with PR #5" in note for note in advisories))

    def test_disjoint_edits_to_one_file_never_gate_a_ready_pr(self):
        advisories = []
        errors = collab.validate_pr(
            pr(self.branch, body(), draft=False),
            files=["src/game.rs"],
            commit_subjects=[],
            ranges={"src/game.rs": [(10, 20)]},
            other_ranges={5: {"src/game.rs": [(900, 910)]}},
            advisories=advisories,
        )
        self.assertEqual(errors, [])
        self.assertTrue(any("different places" in note for note in advisories))

    def test_edits_to_the_same_lines_gate_a_ready_pr(self):
        errors = collab.validate_pr(
            pr(self.branch, body(), draft=False),
            files=["src/game.rs"],
            commit_subjects=[],
            ranges={"src/game.rs": [(100, 140)]},
            other_ranges={5: {"src/game.rs": [(130, 160)]}},
        )
        self.assertTrue(any("collide with PR #5" in error for error in errors))

    def test_a_declared_collision_is_accepted(self):
        errors = collab.validate_pr(
            pr(self.branch, body(coordinated="#5"), draft=False),
            files=["src/game.rs"],
            commit_subjects=[],
            ranges={"src/game.rs": [(100, 140)]},
            other_ranges={5: {"src/game.rs": [(130, 160)]}},
        )
        self.assertEqual(errors, [])

    def test_ready_pr_must_complete_checkboxes(self):
        errors = collab.validate_pr(
            pr(self.branch, body(checked=False), draft=False),
            files=["src/game.rs"],
            commit_subjects=[],
        )
        self.assertTrue(
            any("must complete every validation checkbox" in e for e in errors)
        )

    def test_main_commit_requires_the_matching_merged_pr_commit(self):
        rows = [
            {"number": 12, "merged_at": "2026-07-23T22:00:00Z", "merge_commit_sha": "abc"},
            {"number": 13, "merged_at": None, "merge_commit_sha": "def"},
        ]
        self.assertEqual(collab.commit_is_pr_backed(rows, "abc"), 12)
        self.assertIsNone(collab.commit_is_pr_backed(rows, "def"))
        self.assertIsNone(collab.commit_is_pr_backed(rows, "missing"))

    def test_a_stale_merge_is_still_caught_after_the_fact(self):
        # Staleness is only an advisory on an open PR, because GitHub blocks
        # the merge itself. Once something *has* merged while behind main, that
        # is a real violation and must stay a hard error.
        views = {
            "pr": {
                "headRefOid": "head1234",
                "mergedAt": "2026-07-25T03:00:00Z",
            },
            "compare": {"status": "diverged"},
            "checks": {"check_runs": []},
        }

        def fake_gh_json(args, *, cwd=None):
            joined = " ".join(args)
            if "compare" in joined:
                return views["compare"]
            if "check-runs" in joined:
                return views["checks"]
            return views["pr"]

        with patch.object(collab, "gh_json", side_effect=fake_gh_json):
            errors = collab.merged_pr_gate_errors(42, base_sha="base5678")
        self.assertIn("PR head did not contain current main before merge", errors)

    def test_a_rejected_write_can_fall_back_instead_of_raising(self):
        # ship updates a stale branch through GitHub, but a conflict GitHub
        # cannot resolve must hand back to the local merge path rather than
        # aborting the whole ship.
        rejected = subprocess.CompletedProcess([], 1, stdout="", stderr="merge conflict")
        with patch.object(subprocess, "run", return_value=rejected):
            self.assertIsNone(
                collab.gh_api_write("PUT", "/repos/x/y/pulls/1/update-branch", {}, check=False)
            )
        with patch.object(subprocess, "run", return_value=rejected):
            with self.assertRaises(collab.CommandError):
                collab.gh_api_write("PUT", "/repos/x/y/pulls/1/merge", {})

    def test_a_successful_write_still_returns_its_payload(self):
        ok = subprocess.CompletedProcess([], 0, stdout='{"merged": true}', stderr="")
        with patch.object(subprocess, "run", return_value=ok):
            self.assertEqual(
                collab.gh_api_write("PUT", "/repos/x/y/pulls/1/merge", {}, check=False),
                {"merged": True},
            )

    def test_only_ahead_or_identical_heads_include_current_main(self):
        self.assertTrue(collab.compare_status_is_current("ahead"))
        self.assertTrue(collab.compare_status_is_current("identical"))
        self.assertFalse(collab.compare_status_is_current("behind"))
        self.assertFalse(collab.compare_status_is_current("diverged"))

    def test_required_checks_must_finish_successfully_before_merge(self):
        merged_at = "2026-07-23T22:37:13Z"
        runs = [
            {
                "name": "cargo-test",
                "started_at": "2026-07-23T22:32:11Z",
                "completed_at": "2026-07-23T22:37:02Z",
                "conclusion": "success",
            },
            {
                "name": "collaboration-policy",
                "started_at": "2026-07-23T22:37:13Z",
                "completed_at": "2026-07-23T22:37:19Z",
                "conclusion": "failure",
            },
        ]
        self.assertEqual(
            collab.required_check_gate_errors(runs, merged_at),
            ["required check collaboration-policy was not green before merge"],
        )

    def test_successful_required_checks_before_merge_pass_the_gate(self):
        runs = [
            {
                "name": name,
                "started_at": "2026-07-23T22:30:00Z",
                "completed_at": "2026-07-23T22:35:00Z",
                "conclusion": "success",
            }
            for name in ("cargo-test", "collaboration-policy")
        ]
        self.assertEqual(
            collab.required_check_gate_errors(runs, "2026-07-23T22:36:00Z"), []
        )

    def test_personal_repository_protection_omits_organization_only_fields(self):
        payload = collab.personal_repository_protection_payload()
        reviews = payload["required_pull_request_reviews"]
        self.assertNotIn("bypass_pull_request_allowances", reviews)
        self.assertNotIn("dismissal_restrictions", reviews)
        self.assertNotIn("required_signatures", payload)
        self.assertEqual(reviews["required_approving_review_count"], 0)
        self.assertFalse(payload["allow_force_pushes"])

    def test_personal_repository_protection_cannot_hard_block_main(self):
        """The gate must fail open for admins and must not serialise the fleet.

        `strict` would invalidate every other open PR each time one lands, and
        `enforce_admins` left main unmergeable during the 2026-07-25 Actions
        billing outage, when no job could start at all. The required contexts
        still gate every PR; these two only decide who waits on whom.
        """
        payload = collab.personal_repository_protection_payload()
        self.assertFalse(payload["required_status_checks"]["strict"])
        self.assertFalse(payload["enforce_admins"])
        self.assertEqual(
            payload["required_status_checks"]["contexts"],
            ["cargo-test", "collaboration-policy"],
        )


class ShipTests(unittest.TestCase):
    def test_current_pr_names_the_branch_for_repo_scoped_gh(self):
        branch = "agent/m/a/task-20260723T210500Z-a31f"
        with (
            patch.object(collab, "git", return_value=branch),
            patch.object(collab, "gh_json", return_value={"number": 9}) as gh,
        ):
            self.assertEqual(collab.current_pr(Path.cwd()), {"number": 9})
        self.assertEqual(gh.call_args.args[0][2], branch)

    def test_pr_head_waits_for_the_pr_view_to_observe_the_pushed_ref(self):
        branch = "agent/m/a/task-20260723T210500Z-a31f"
        with (
            patch.object(
                collab,
                "current_pr",
                side_effect=[{"headRefOid": "old"}, {"headRefOid": "new"}],
            ),
            patch.object(collab, "remote_heads", return_value={branch: "new"}),
            patch.object(collab.time, "sleep"),
        ):
            result = collab.wait_for_pr_head(
                Path.cwd(),
                branch,
                "new",
                deadline=collab.time.monotonic() + 1,
                poll_seconds=0,
            )
        self.assertEqual(result["headRefOid"], "new")

    def test_pr_head_wait_stops_when_auto_merge_closes_the_pr(self):
        branch = "agent/m/a/task-20260723T210500Z-a31f"
        merged = {
            "number": 9,
            "state": "MERGED",
            "headRefOid": "old",
            "mergeCommit": {"oid": "squash123"},
        }
        with (
            patch.object(collab, "current_pr", return_value=merged),
            patch.object(collab, "remote_heads") as remote_heads,
        ):
            result = collab.wait_for_pr_head(
                Path.cwd(),
                branch,
                "new",
                deadline=collab.time.monotonic() + 1,
                poll_seconds=0,
            )
        self.assertEqual(result, merged)
        remote_heads.assert_not_called()

    def test_only_a_merged_pr_exposes_its_squash_commit(self):
        self.assertEqual(
            collab.pr_merge_sha(
                {"state": "MERGED", "mergeCommit": {"oid": "squash123"}}
            ),
            "squash123",
        )
        self.assertEqual(
            collab.pr_merge_sha(
                {"state": "OPEN", "mergeCommit": {"oid": "premature"}}
            ),
            "",
        )

    def test_ship_requires_a_finished_summary_and_every_checkbox(self):
        draft = {
            "state": "OPEN",
            "headRefName": "agent/m/a/task-20260723T210500Z-a31f",
            "body": """## What changed

Draft claim; implementation is in progress.

## Validation

- [ ] Tests
""",
        }
        errors = collab.ship_pr_errors(draft, draft["headRefName"])
        self.assertTrue(any("checkbox" in error for error in errors))
        self.assertTrue(any("What changed" in error for error in errors))

    def test_ship_accepts_a_documented_validated_feature(self):
        finished = {
            "state": "OPEN",
            "headRefName": "agent/m/a/task-20260723T210500Z-a31f",
            "body": """## What changed

Added the fast shipping path.

## Validation

- [x] Tests
""",
        }
        self.assertEqual(
            collab.ship_pr_errors(finished, finished["headRefName"]), []
        )

    def test_required_checks_use_the_newest_run_for_each_name(self):
        rows = [
            {
                "name": "cargo-test",
                "startedAt": "2026-07-23T22:30:00Z",
                "status": "COMPLETED",
                "conclusion": "FAILURE",
            },
            {
                "name": "cargo-test",
                "startedAt": "2026-07-23T22:35:00Z",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
            {
                "name": "collaboration-policy",
                "startedAt": "2026-07-23T22:35:01Z",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
        ]
        self.assertEqual(collab.required_check_state(rows), ("success", []))

    def test_ready_transition_does_not_reuse_an_old_draft_policy_check(self):
        rows = [
            {
                "name": "cargo-test",
                "startedAt": "2026-07-23T22:35:00Z",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
            {
                "name": "collaboration-policy",
                "startedAt": "2026-07-23T22:34:00Z",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
        ]
        self.assertEqual(
            collab.required_check_state(
                rows,
                minimum_started={"collaboration-policy": "2026-07-23T22:35:00Z"},
            ),
            ("pending", ["collaboration-policy"]),
        )

    def test_pending_and_failed_checks_are_distinct(self):
        pending = [
            {
                "name": "cargo-test",
                "startedAt": "2026-07-23T22:35:00Z",
                "status": "IN_PROGRESS",
                "conclusion": "",
            }
        ]
        self.assertEqual(
            collab.required_check_state(pending),
            ("pending", ["cargo-test", "collaboration-policy"]),
        )
        failed = pending + [
            {
                "name": "collaboration-policy",
                "startedAt": "2026-07-23T22:35:01Z",
                "status": "COMPLETED",
                "conclusion": "FAILURE",
            }
        ]
        self.assertEqual(
            collab.required_check_state(failed),
            ("failed", ["collaboration-policy"]),
        )

    def test_exact_live_revision_is_accepted_without_git_lookup(self):
        self.assertTrue(
            collab.deployed_commit_covers(
                Path.cwd(), "abc1234", "abc1234567890"
            )
        )


if __name__ == "__main__":
    unittest.main()
