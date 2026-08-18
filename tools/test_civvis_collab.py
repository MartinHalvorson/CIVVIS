from pathlib import Path
import concurrent.futures
import ast
import json
import os
import shutil
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


class FreshnessTests(unittest.TestCase):
    def git(self, repo, *args):
        return subprocess.run(
            ("git", "-C", str(repo), *args),
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    def committed_repo(self, root):
        self.git(root.parent, "init", "--initial-branch=main", str(root))
        self.git(root, "config", "user.email", "freshness@example.invalid")
        self.git(root, "config", "user.name", "Freshness Test")
        Path(root, "tracked.txt").write_text("one\n", encoding="utf-8")
        self.git(root, "add", "tracked.txt")
        self.git(root, "commit", "-m", "one")

    def test_refresh_updates_main_without_changing_a_development_worktree(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            remote = base / "remote.git"
            seed = base / "seed"
            clone = base / "clone"
            task = base / "task"
            self.git(base, "init", "--bare", str(remote))
            self.committed_repo(seed)
            self.git(seed, "remote", "add", "origin", str(remote))
            self.git(seed, "push", "-u", "origin", "main")
            # A bare repository created before the first push still points HEAD
            # at master. Set it explicitly so clone checks out main.
            self.git(remote, "symbolic-ref", "HEAD", "refs/heads/main")
            self.git(base, "clone", str(remote), str(clone))
            self.git(clone, "config", "user.email", "freshness@example.invalid")
            self.git(clone, "config", "user.name", "Freshness Test")
            self.git(clone, "worktree", "add", "-b", "local-task", str(task))
            Path(task, "tracked.txt").write_text("local dirty work\n", encoding="utf-8")
            task_head = self.git(task, "rev-parse", "HEAD")

            Path(seed, "upstream.txt").write_text("two\n", encoding="utf-8")
            self.git(seed, "add", "upstream.txt")
            self.git(seed, "commit", "-m", "two")
            upstream_head = self.git(seed, "rev-parse", "HEAD")
            self.git(seed, "push", "origin", "main")

            report = collab.refresh_repository(clone)

            self.assertEqual(report["origin_main"], upstream_head)
            self.assertEqual(
                report["main_update"]["mode"],
                "fast-forward",
            )
            self.assertEqual(self.git(clone, "rev-parse", "HEAD"), upstream_head)
            self.assertTrue(Path(clone, "upstream.txt").is_file())
            self.assertEqual(self.git(task, "rev-parse", "HEAD"), task_head)
            self.assertEqual(
                Path(task, "tracked.txt").read_text(encoding="utf-8"),
                "local dirty work\n",
            )
            self.assertFalse(Path(task, "upstream.txt").exists())
            rows = {row["branch"]: row for row in report["worktrees"]}
            self.assertEqual(rows["main"]["behind"], 0)
            self.assertFalse(rows["main"]["dirty"])
            self.assertTrue(rows["local-task"]["dirty"])
            self.assertEqual(rows["local-task"]["behind"], 1)

    def test_refresh_preserves_divergent_main_before_forcing_remote_head(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            remote = base / "remote.git"
            seed = base / "seed"
            clone = base / "clone"
            self.git(base, "init", "--bare", str(remote))
            self.committed_repo(seed)
            self.git(seed, "remote", "add", "origin", str(remote))
            self.git(seed, "push", "-u", "origin", "main")
            self.git(remote, "symbolic-ref", "HEAD", "refs/heads/main")
            self.git(base, "clone", str(remote), str(clone))
            self.git(clone, "config", "user.email", "freshness@example.invalid")
            self.git(clone, "config", "user.name", "Freshness Test")

            Path(clone, "local.txt").write_text("preserve me\n", encoding="utf-8")
            self.git(clone, "add", "local.txt")
            self.git(clone, "commit", "-m", "local main commit")
            local_head = self.git(clone, "rev-parse", "HEAD")

            Path(seed, "upstream.txt").write_text("remote head\n", encoding="utf-8")
            self.git(seed, "add", "upstream.txt")
            self.git(seed, "commit", "-m", "remote main commit")
            upstream_head = self.git(seed, "rev-parse", "HEAD")
            self.git(seed, "push", "origin", "main")

            report = collab.refresh_repository(clone)

            update = report["main_update"]
            self.assertEqual(update["mode"], "forced")
            self.assertEqual(update["before"], local_head)
            self.assertEqual(update["after"], upstream_head)
            self.assertEqual(self.git(clone, "rev-parse", "HEAD"), upstream_head)
            self.assertEqual(
                self.git(clone, "rev-parse", update["recovery_ref"]),
                local_head,
            )
            self.assertFalse(Path(clone, "local.txt").exists())
            self.assertTrue(Path(clone, "upstream.txt").is_file())

    def test_refresh_refuses_to_overwrite_dirty_main(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            remote = base / "remote.git"
            seed = base / "seed"
            clone = base / "clone"
            self.git(base, "init", "--bare", str(remote))
            self.committed_repo(seed)
            self.git(seed, "remote", "add", "origin", str(remote))
            self.git(seed, "push", "-u", "origin", "main")
            self.git(remote, "symbolic-ref", "HEAD", "refs/heads/main")
            self.git(base, "clone", str(remote), str(clone))
            local_head = self.git(clone, "rev-parse", "HEAD")
            Path(clone, "tracked.txt").write_text("dirty local work\n", encoding="utf-8")

            Path(seed, "upstream.txt").write_text("remote head\n", encoding="utf-8")
            self.git(seed, "add", "upstream.txt")
            self.git(seed, "commit", "-m", "remote main commit")
            upstream_head = self.git(seed, "rev-parse", "HEAD")
            self.git(seed, "push", "origin", "main")

            report = collab.refresh_repository(clone)

            self.assertIn("dirty main management worktree", report["main_update_error"])
            self.assertEqual(self.git(clone, "rev-parse", "HEAD"), local_head)
            self.assertEqual(
                Path(clone, "tracked.txt").read_text(encoding="utf-8"),
                "dirty local work\n",
            )
            [row] = report["worktrees"]
            self.assertTrue(row["dirty"])
            self.assertEqual(row["behind"], 1)
            self.assertIn(
                "last automatic main update failed",
                collab.freshness_state_error(clone, upstream_head),
            )

    def test_main_update_fails_closed_when_status_cannot_be_read(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            self.committed_repo(root)
            head = self.git(root, "rev-parse", "HEAD")
            original_git = collab.git
            mutating_commands = []

            def fail_status(repo, *args, check=True):
                if args and args[0] in {"merge", "reset", "update-ref"}:
                    mutating_commands.append(args[0])
                if args[:3] == (
                    "status",
                    "--porcelain=v1",
                    "--untracked-files=all",
                ):
                    if check:
                        raise collab.CommandError("simulated status read failure")
                    return ""
                return original_git(repo, *args, check=check)

            with patch.object(collab, "git", side_effect=fail_status):
                with self.assertRaisesRegex(
                    collab.CommandError,
                    "simulated status read failure",
                ):
                    collab.force_update_main_worktree(root, head)

            self.assertEqual(mutating_commands, [])
            self.assertEqual(self.git(root, "rev-parse", "HEAD"), head)

    def test_macos_service_runs_the_automatic_main_refresh_command(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            self.committed_repo(root)
            worker = root / ".git" / "managed.py"
            payload = collab.plistlib.loads(collab.macos_freshness_plist(root, worker))

        command = payload["ProgramArguments"]
        self.assertEqual(command[2:4], ["refresh", "--scheduled"])
        self.assertIn("--repo", command)
        self.assertEqual(payload["StartInterval"], collab.FRESHNESS_INTERVAL_SECONDS)

    def test_windows_service_uses_a_hidden_wscript_launcher(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            root = base / "repo with spaces"
            worker = root / ".git" / "managed.py"
            launcher = root / ".git" / "run-hidden.vbs"

            with patch.object(collab, "main_worktree", return_value=root):
                launcher_data = collab.windows_freshness_launcher_data(root, worker)
            launcher_text = launcher_data.decode("utf-16")
            python_command = subprocess.list2cmdline(
                (
                    sys.executable,
                    str(worker),
                    "refresh",
                    "--scheduled",
                    "--repo",
                    str(root),
                )
            )
            self.assertIn(
                f'command = "{python_command.replace(chr(34), chr(34) * 2)}"',
                launcher_text,
            )
            self.assertIn("shell.Run(command, 0, True)", launcher_text)
            self.assertIn(collab.FRESHNESS_MARKER, launcher_text)

            with patch.object(
                collab.shutil,
                "which",
                return_value=r"C:\Windows\System32\wscript.exe",
            ):
                task_command = collab.windows_freshness_task_command(launcher)

        self.assertEqual(
            task_command,
            subprocess.list2cmdline(
                (
                    r"C:\Windows\System32\wscript.exe",
                    "//B",
                    str(launcher),
                )
            ),
        )

    def test_windows_service_fails_closed_without_wscript(self):
        with patch.object(collab.shutil, "which", return_value=None):
            with self.assertRaisesRegex(collab.CommandError, "wscript.exe is missing"):
                collab.windows_freshness_task_command(Path("run-hidden.vbs"))

    def test_utf16_scheduler_definition_is_recognized_as_managed(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "run-hidden.vbs"
            content = f"' {collab.FRESHNESS_MARKER}\nmanaged\n".encode("utf-16")
            self.assertTrue(collab.write_managed_service(path, content))
            self.assertFalse(collab.write_managed_service(path, content))

    def test_existing_managed_worker_self_updates_from_remote_main(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            remote = base / "remote.git"
            seed = base / "seed"
            clone = base / "clone"
            self.git(base, "init", "--bare", str(remote))
            self.committed_repo(seed)
            tools = seed / "tools"
            tools.mkdir()
            source = tools / "civvis_collab.py"
            old_worker = f"# {collab.FRESHNESS_MARKER}\nold worker\n"
            source.write_text(old_worker, encoding="utf-8")
            self.git(seed, "add", "tools/civvis_collab.py")
            self.git(seed, "commit", "-m", "old worker")
            self.git(seed, "remote", "add", "origin", str(remote))
            self.git(seed, "push", "-u", "origin", "main")
            self.git(remote, "symbolic-ref", "HEAD", "refs/heads/main")
            self.git(base, "clone", str(remote), str(clone))

            target = collab.freshness_worker_path(clone)
            target.parent.mkdir(parents=True)
            target.write_text(old_worker, encoding="utf-8")
            new_worker = f"# {collab.FRESHNESS_MARKER}\nnew automatic sync worker\n"
            source.write_text(new_worker, encoding="utf-8")
            self.git(seed, "add", "tools/civvis_collab.py")
            self.git(seed, "commit", "-m", "new worker")
            self.git(seed, "push", "origin", "main")

            report = collab.refresh_repository(clone)

            self.assertNotIn("fetch_error", report)
            self.assertEqual(target.read_text(encoding="utf-8"), new_worker)

    def test_a_stale_or_wrong_revision_heartbeat_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            self.committed_repo(root)
            old = collab.dt.datetime.now(collab.dt.timezone.utc) - collab.dt.timedelta(hours=1)
            collab.write_freshness_state(
                root,
                {
                    "schema": collab.FRESHNESS_SCHEMA,
                    "machine": "",
                    "fetched_at": old.isoformat(),
                    "origin_main": "old",
                },
            )
            self.assertIn("stale", collab.freshness_state_error(root, "new"))
            collab.write_freshness_state(
                root,
                {
                    "schema": collab.FRESHNESS_SCHEMA,
                    "machine": "",
                    "fetched_at": collab.utc_now(),
                    "origin_main": "old",
                },
            )
            self.assertIn(
                "current GitHub main",
                collab.freshness_state_error(root, "new"),
            )

    def test_an_unmanaged_scheduler_definition_is_preserved(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "service.plist"
            path.write_text("owned by somebody else\n", encoding="utf-8")
            with self.assertRaises(collab.CommandError):
                collab.write_managed_service(path, b"replacement")
            self.assertEqual(
                path.read_text(encoding="utf-8"), "owned by somebody else\n"
            )

    def test_reinstalling_an_identical_scheduler_is_a_no_op(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "service"
            content = f"# {collab.FRESHNESS_MARKER}\nmanaged\n".encode("utf-8")
            self.assertTrue(collab.write_managed_service(path, content))
            modified = path.stat().st_mtime_ns
            self.assertFalse(collab.write_managed_service(path, content))
            self.assertEqual(path.stat().st_mtime_ns, modified)


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


class EffectSizeEvidenceTests(unittest.TestCase):
    """The R3 gate: a number in a document must say where it came from.

    The case that motivates all of this is real. `docs/GENOME.md` carried
    "`strategic_deep` at +45 Elo" as a promoted gain; PR #482 measured -8
    (95% CI -27..+12) over 220 maps and *excluded* it, and that refutation
    reached a PR body and never reached the document.
    """

    #: The exact wrapping of the real defect. The figure ends one 80-column line
    #: and its unit begins the next, which is why the check joins before matching.
    BARE_CLAIM = [
        "evolution. Meanwhile every promoted gain in the repository has come from",
        "**giving the search more counterfactual rollout** — `strategic_deep` at +45",
        "Elo, warm branches at +37.",
    ]

    def test_the_real_bare_claim_is_rejected_even_though_it_wraps(self):
        problems = collab.unevidenced_effect_sizes({"docs/GENOME.md": self.BARE_CLAIM})

        self.assertEqual(len(problems), 1)
        self.assertIn("docs/GENOME.md", problems[0])
        self.assertIn("no evidence beside it", problems[0])

    def test_the_refutation_that_replaced_it_passes(self):
        problems = collab.unevidenced_effect_sizes(
            {
                "docs/GENOME.md": [
                    "It cited `strategic_deep` at +45 Elo and warm branches at +37.",
                    "**#482 excludes the +45**: pooled over 220 mirrored maps on two",
                    "disjoint seeds it measured Elo-equivalent **−8 (95% CI −27..+12)**.",
                ]
            }
        )

        self.assertEqual(problems, [])

    def test_a_seed_alone_is_enough_provenance(self):
        self.assertEqual(
            collab.unevidenced_effect_sizes(
                {"README.md": ["`advanced` measured +207 Elo-equivalent (seed 77200000)."]}
            ),
            [],
        )

    def test_an_explicit_discovery_estimate_marker_is_enough(self):
        self.assertEqual(
            collab.unevidenced_effect_sizes(
                {"docs/EVAL.md": ["gain of +114 Elo (DISCOVERY ESTIMATE, not yet confirmed)"]}
            ),
            [],
        )

    def test_only_prose_documents_are_gated(self):
        """Source and data carry Elo numbers for reasons a lint cannot judge."""
        for path in ("src/elo.rs", "data/elo_ratings.json", "tools/x.py"):
            self.assertEqual(
                collab.unevidenced_effect_sizes({path: ["let promoted = +45; // Elo"]}),
                [],
                path,
            )

    def test_prose_without_a_figure_is_untouched(self):
        self.assertEqual(
            collab.unevidenced_effect_sizes(
                {"docs/EVAL.md": ["Rollout search remains the best-supported lever."]}
            ),
            [],
        )

    def test_distant_evidence_does_not_launder_a_bare_claim(self):
        """A measurement elsewhere in the same hunk is not this number's source."""
        problems = collab.unevidenced_effect_sizes(
            {
                "docs/EVAL.md": [
                    "The prophet-first arm measured +12 over 120 maps on seed 4100.",
                    "x" * (collab.EVIDENCE_WINDOW_CHARS + 80),
                    "Search is worth +45 Elo.",
                ]
            }
        )

        self.assertEqual(len(problems), 1)
        self.assertIn("+45 Elo", problems[0])

    def test_added_lines_are_read_from_the_patch_without_the_file_header(self):
        patch = "@@ -1,2 +1,3 @@\n context\n-gone\n+kept\n+also kept\n"

        self.assertEqual(collab.patch_added_lines(patch), ["kept", "also kept"])
        self.assertEqual(
            collab.patch_added_lines("+++ b/docs/EVAL.md\n+real line\n"), ["real line"]
        )

    def test_the_gate_runs_inside_validate_pr(self):
        pr = {
            "number": 700,
            "headRefName": "agent/m1/a1/task-20260731T000000Z-abcd",
            "body": (
                "- Machine ID: `m1`\n- Agent/session ID: `a1`\n- Task: t\n"
                "- Claimed paths: `docs/GENOME.md`\n- Coordinated with: none\n"
            ),
            "isDraft": False,
        }

        errors = collab.validate_pr(
            pr,
            files=["docs/GENOME.md"],
            commit_subjects=["doc"],
            added_lines={"docs/GENOME.md": self.BARE_CLAIM},
        )

        self.assertTrue(any("no evidence beside it" in error for error in errors), errors)


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

    def test_a_pin_only_collision_does_not_fail_a_ready_pr(self):
        """End-to-end: the exemption must be wired into validate_pr, not just exist.

        Two PRs both re-pinning ANCHOR_BEHAVIOUR_FNV collide on that
        one line by construction. Without the exemption this is a hard error on
        a ready PR and the author is told to coordinate — with a set that
        changes again on the next merge.
        """
        pin_only = [
            "/// #999 gates a thing. A compatibility re-pin.",
            "const ANCHOR_BEHAVIOUR_FNV: u64 = 0xdead_beef_dead_beef;",
        ]
        errors = collab.validate_pr(
            pr(self.branch, body(paths="`src/main.rs`"), draft=False),
            files=["src/main.rs"],
            commit_subjects=[],
            other_files={5: {"src/main.rs"}},
            added_lines={"src/main.rs": pin_only},
        )
        self.assertEqual(errors, [])

    def test_a_real_main_rs_collision_still_fails_a_ready_pr(self):
        """The exemption is for the pin line, not for src/main.rs generally."""
        errors = collab.validate_pr(
            pr(self.branch, body(paths="`src/main.rs`"), draft=False),
            files=["src/main.rs"],
            commit_subjects=[],
            other_files={5: {"src/main.rs"}},
            added_lines={
                "src/main.rs": [
                    "const ANCHOR_BEHAVIOUR_FNV: u64 = 0xdead_beef_dead_beef;",
                    "    let entrants = parse_tournament_entrants(spec)?;",
                ]
            },
        )
        self.assertTrue(any("collide with PR #5" in error for error in errors), errors)

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

    def test_a_newer_prs_coordination_acknowledges_an_older_ready_pr(self):
        """A later task cannot be named by an older PR's already-ready body."""
        advisories = []
        errors = collab.validate_pr(
            pr(self.branch, body(), draft=False),
            files=["src/game.rs"],
            commit_subjects=[],
            ranges={"src/game.rs": [(100, 140)]},
            other_ranges={5: {"src/game.rs": [(130, 160)]}},
            other_coordination={5: {9}},
            advisories=advisories,
        )
        self.assertEqual(errors, [])
        self.assertTrue(
            any("PR #5 already declares coordination with PR #9" in note for note in advisories)
        )

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

    def test_a_far_stale_merge_is_still_caught_after_the_fact(self):
        # An earlier version of this test called ANY behind-merge a violation,
        # on the stated premise that GitHub blocks such merges. It does not:
        # protection runs with `strict: false` on purpose, and `ship`
        # deliberately leaves the head alone while the gate runs — refreshing
        # it cancelled the in-flight run every time `main` advanced, which on
        # 2026-08-06 killed 44% of all cargo-test runs. The hard line that
        # remains is the one `base_staleness` draws pre-merge: a head at or
        # past STALE_BASE_LIMIT merged as a tree no CI run has approximated.
        views = {
            "pr": {
                "headRefOid": "head1234",
                "mergedAt": "2026-07-25T03:00:00Z",
            },
            "compare": {
                "status": "diverged",
                "behind_by": collab.STALE_BASE_LIMIT,
            },
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
        self.assertTrue(
            any("behind main at merge" in error for error in errors),
            errors,
        )

    def test_a_mildly_stale_merge_is_the_designed_outcome(self):
        # Below the limit, a green run merging while the trunk moved on is how
        # this repository converges at all: the push-to-main gate tests the
        # actual squash result. The audit must not flag it, or the fleet
        # relearns to ignore red.
        views = {
            "pr": {
                "headRefOid": "head1234",
                "mergedAt": "2026-07-25T03:00:00Z",
            },
            "compare": {"status": "diverged", "behind_by": 3},
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
        self.assertFalse(
            [error for error in errors if "behind main" in error],
            errors,
        )

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


class LiveBuildWaitTests(unittest.TestCase):
    """`ship` must not spend ten minutes polling a port with nothing behind it.

    The guard it had reads `local_deploy_root`, which only says this clone is a
    production host. All CIVVIS automation was deliberately stopped on
    2026-07-31 and the keeper LaunchAgent is not loaded, so on this machine
    every merge waited out the full `--live-timeout-seconds` and then warned
    about a service that was switched off on purpose.
    """

    @staticmethod
    def _clock():
        now = {"t": 0.0}
        return (lambda: now["t"]),\
               (lambda s: now.__setitem__("t", now["t"] + s)), now

    def _wait(self, *, answers, commits, timeout=600.0):
        monotonic, sleep, now = self._clock()
        with (
            patch.object(collab, "local_deploy_root", return_value=Path("/x")),
            patch.object(collab, "live_status_answers", side_effect=answers),
            patch.object(collab, "live_status_commit", side_effect=commits),
            patch.object(collab, "deployed_commit_covers",
                         side_effect=lambda _r, d, m: bool(d) and d == m),
            patch.object(collab, "fetch_main", return_value=None),
            patch.object(collab.time, "monotonic", monotonic),
            patch.object(collab.time, "sleep", sleep),
        ):
            got = collab.wait_for_local_live_build(
                Path("/repo"), "abc1234",
                url="http://127.0.0.1:8766/status",
                timeout_seconds=timeout, poll_seconds=10.0)
        return got, now["t"]

    def test_a_dead_port_gives_up_after_the_grace_not_the_whole_timeout(self):
        got, elapsed = self._wait(answers=[False] * 500, commits=[])
        self.assertFalse(got)
        self.assertLessEqual(
            elapsed, collab.LIVE_PRESENCE_GRACE_S + 2.0,
            "nothing listening must cost the grace, not --live-timeout-seconds")

    def test_a_spectator_that_is_merely_restarting_is_still_waited_for(self):
        """⚠ `ship` runs exactly when a spectator may be restarting.

        A refused connection in that instant is expected, so the early exit must
        not fire on the first probe — it comes back and then confirms.
        """
        answers = [False, False, True] + [True] * 50
        got, _ = self._wait(answers=answers, commits=["abc1234"])
        self.assertTrue(got)

    def test_a_live_but_stale_spectator_still_gets_the_full_timeout(self):
        """The case the timeout exists for must be untouched."""
        got, elapsed = self._wait(
            answers=[True] * 500, commits=["oldsha"] * 500, timeout=600.0)
        self.assertFalse(got)
        self.assertGreaterEqual(
            elapsed, 600.0,
            "a spectator that answers is still building; that wait is the point")

    def test_an_http_error_still_counts_as_something_being_there(self):
        """HTTPError subclasses URLError and OSError, so order matters.

        Caught in the wrong order, a live spectator returning 503 mid-restart
        reads as absent and the merge stops verifying.
        """
        err = collab.urllib.error.HTTPError("u", 503, "busy", None, None)
        with patch.object(collab.urllib.request, "urlopen", side_effect=err):
            self.assertTrue(collab.live_status_answers("http://x/status"))
        with patch.object(collab.urllib.request, "urlopen",
                          side_effect=ConnectionRefusedError()):
            self.assertFalse(collab.live_status_answers("http://x/status"))


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

    def test_a_cancelled_check_is_retryable_not_a_failure(self):
        """A cancelled run reached no verdict, and nothing re-starts it.

        This is the shape that stranded PR #1933: `cargo-test` was superseded
        by its own concurrency group, ended CANCELLED, and armed auto-merge
        then waited for a green check that could never arrive. Reporting it as
        a failure blames the tree for something the tree did not do; reporting
        it as pending waits forever.
        """
        for conclusion in ("CANCELLED", "TIMED_OUT", "STALE"):
            with self.subTest(conclusion=conclusion):
                rows = [
                    {
                        "name": "cargo-test",
                        "startedAt": "2026-08-18T00:52:20Z",
                        "status": "COMPLETED",
                        "conclusion": conclusion,
                    },
                    {
                        "name": "collaboration-policy",
                        "startedAt": "2026-08-18T00:52:21Z",
                        "status": "COMPLETED",
                        "conclusion": "SUCCESS",
                    },
                ]
                self.assertEqual(
                    collab.required_check_state(rows),
                    ("retryable", ["cargo-test"]),
                )

    def test_a_real_failure_outranks_a_cancellation_beside_it(self):
        """Report the verdict the tree earned, not the noise next to it."""
        rows = [
            {
                "name": "cargo-test",
                "startedAt": "2026-08-18T00:52:20Z",
                "status": "COMPLETED",
                "conclusion": "CANCELLED",
            },
            {
                "name": "collaboration-policy",
                "startedAt": "2026-08-18T00:52:21Z",
                "status": "COMPLETED",
                "conclusion": "FAILURE",
            },
        ]
        self.assertEqual(
            collab.required_check_state(rows),
            ("failed", ["collaboration-policy"]),
        )

    def test_a_cancellation_outranks_a_pending_sibling(self):
        """Dispatch the re-run now; the sibling can keep running meanwhile."""
        rows = [
            {
                "name": "cargo-test",
                "startedAt": "2026-08-18T00:52:20Z",
                "status": "COMPLETED",
                "conclusion": "CANCELLED",
            },
            {
                "name": "collaboration-policy",
                "startedAt": "2026-08-18T00:52:21Z",
                "status": "IN_PROGRESS",
                "conclusion": "",
            },
        ]
        self.assertEqual(
            collab.required_check_state(rows),
            ("retryable", ["cargo-test"]),
        )

    def test_check_run_id_reads_the_run_not_the_job(self):
        """`gh` hands back the job URL; only the run can be re-dispatched."""
        self.assertEqual(
            collab.check_run_id(
                {
                    "detailsUrl": "https://github.com/MartinHalvorson/CIVVIS"
                    "/actions/runs/32086139697/job/95570795617"
                }
            ),
            "32086139697",
        )
        # A status context from outside Actions has nothing to re-dispatch.
        self.assertIsNone(
            collab.check_run_id({"detailsUrl": "https://example.com/scan/17"})
        )
        self.assertIsNone(collab.check_run_id({}))

    def test_rerun_targets_the_newest_run_of_the_named_check(self):
        calls: list = []

        def fake_write(method, path, payload, *, check=True):
            calls.append((method, path))
            return {}

        rows = [
            {
                "name": "cargo-test",
                "startedAt": "2026-08-18T00:49:31Z",
                "detailsUrl": "https://github.com/o/r/actions/runs/111/job/1",
            },
            {
                "name": "cargo-test",
                "startedAt": "2026-08-18T00:52:20Z",
                "detailsUrl": "https://github.com/o/r/actions/runs/222/job/2",
            },
            {
                "name": "rust-quality",
                "startedAt": "2026-08-18T00:52:20Z",
                "detailsUrl": "https://github.com/o/r/actions/runs/333/job/3",
            },
        ]
        original = collab.gh_api_write
        collab.gh_api_write = fake_write
        try:
            self.assertTrue(collab.rerun_required_check(rows, "cargo-test"))
            # Not a required check: `ship` does not gate on it, so it must not
            # spend CI re-running it either.
            self.assertFalse(collab.rerun_required_check(rows, "rust-quality"))
            self.assertFalse(collab.rerun_required_check([], "cargo-test"))
        finally:
            collab.gh_api_write = original
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][0], "POST")
        self.assertTrue(calls[0][1].endswith("/actions/runs/222/rerun"))

    def test_a_pin_only_edit_is_not_a_real_collision(self):
        """The source-contract pin collides by construction; see SOURCE_CONTRACT_PIN."""
        pin_only = [
            "/// #999 does a thing behind a flag. A compatibility re-pin.",
            "const ANCHOR_BEHAVIOUR_FNV: u64 = 0xdead_beef_dead_beef;",
        ]
        self.assertTrue(collab.is_pin_only_edit(pin_only))
        self.assertEqual(
            collab.drop_pin_only_collisions(
                ["src/main.rs"], {"src/main.rs": pin_only}
            ),
            [],
        )

    def test_a_real_main_rs_edit_still_collides(self):
        """Exempting the pin must not exempt the file it lives in."""
        with_real_code = [
            "const ANCHOR_BEHAVIOUR_FNV: u64 = 0xdead_beef_dead_beef;",
            "    let entrants = parse_tournament_entrants(spec)?;",
        ]
        self.assertFalse(collab.is_pin_only_edit(with_real_code))
        self.assertEqual(
            collab.drop_pin_only_collisions(
                ["src/main.rs"], {"src/main.rs": with_real_code}
            ),
            ["src/main.rs"],
        )

    def test_other_files_are_never_exempted(self):
        """Only src/main.rs carries the pin; nothing else may claim the exemption."""
        self.assertEqual(
            collab.drop_pin_only_collisions(
                ["src/elo.rs"],
                {"src/elo.rs": ["const ANCHOR_BEHAVIOUR_FNV: u64 = 0x1;"]},
            ),
            ["src/elo.rs"],
        )

    def test_an_edit_that_never_touches_the_pin_is_not_pin_only(self):
        """A doc-comment-only change to main.rs is a real edit, not a re-pin."""
        self.assertFalse(collab.is_pin_only_edit(["/// just a comment"]))
        self.assertFalse(collab.is_pin_only_edit([]))

    def test_exact_live_revision_is_accepted_without_git_lookup(self):
        self.assertTrue(
            collab.deployed_commit_covers(
                Path.cwd(), "abc1234", "abc1234567890"
            )
        )


class BaseStalenessTests(unittest.TestCase):
    def test_a_current_branch_is_left_alone(self):
        self.assertIsNone(collab.base_staleness("ahead", 0))
        self.assertIsNone(collab.base_staleness("identical", 0))

    def test_mild_staleness_advises_and_says_nothing_enforces_it(self):
        kind, message = collab.base_staleness("behind", 3)
        self.assertEqual(kind, "advisory")
        self.assertIn("3 commits", message)
        self.assertIn("Nothing enforces freshness", message)

    def test_severe_staleness_is_refused(self):
        kind, message = collab.base_staleness(
            "behind", collab.STALE_BASE_LIMIT)
        self.assertEqual(kind, "error")
        self.assertIn("no CI run has tested", message)

    def test_diverged_history_uses_the_same_ladder(self):
        self.assertEqual(collab.base_staleness("diverged", 1)[0], "advisory")
        self.assertEqual(
            collab.base_staleness("diverged", collab.STALE_BASE_LIMIT + 40)[0],
            "error",
        )

    def test_one_commit_under_the_limit_still_only_advises(self):
        kind, _ = collab.base_staleness("behind", collab.STALE_BASE_LIMIT - 1)
        self.assertEqual(kind, "advisory")


class ComparisonUnavailableTests(unittest.TestCase):
    """A comparison GitHub will not answer must not block a merge.

    On 2026-08-17 `/repos/.../compare/A...B` returned 404 for this repository
    for every pair — including two adjacent commits on `main` — while
    `/commits/<sha>` resolved both SHAs. The call was unguarded in all three
    of its sites, so `collaboration-policy` exited 1 on a traceback and a
    required check nobody could turn green stopped every merge in the fleet.
    """

    def event(self, tmp: Path, *, draft: bool = False) -> Path:
        path = tmp / "event.json"
        path.write_text(json.dumps({"pull_request": {
            "number": 77,
            "head": {"ref": "agent/mbp-m5-max-128/claude-1/t-20260817T000000Z-aaaa",
                     "sha": "head1234"},
            "base": {"sha": "base5678"},
            "draft": draft,
            "body": body(machine="mbp-m5-max-128", agent="claude-1",
                         paths="`docs/X.md`"),
        }}))
        return path

    def run_check(self, compare_raises: bool):
        """Run `check_pr_action` with only the compare call able to fail."""
        def fake_github_json(path, token):
            if "/compare/" in path:
                if compare_raises:
                    raise collab.CommandError(
                        "GitHub API /repos/x/compare/a...b failed (404): "
                        '{"message":"Not Found"}')
                return {"status": "ahead", "behind_by": 0}
            if path.endswith("pulls?state=open&per_page=100"):
                return []
            return []

        printed = []
        tmp = Path(tempfile.mkdtemp(prefix="civvis-compare-"))
        self.addCleanup(shutil.rmtree, tmp, True)
        with patch.object(collab, "github_json", side_effect=fake_github_json), \
                patch.object(collab, "pr_file_ranges", return_value={"docs/X.md": None}), \
                patch.object(collab, "pr_files", return_value=["docs/X.md"]), \
                patch.object(collab, "pr_commit_subjects", return_value=["work"]), \
                patch.object(collab, "pr_added_lines", return_value={}), \
                patch.object(collab, "machine_registry", return_value=None), \
                patch("builtins.print", side_effect=lambda *a, **k: printed.append(" ".join(str(x) for x in a))):
            code = collab.check_pr_action(
                self.event(tmp), "token", "MartinHalvorson/CIVVIS")
        return code, "\n".join(printed)

    def test_the_gate_passes_when_github_will_not_compare(self):
        code, output = self.run_check(compare_raises=True)
        self.assertEqual(code, 0, output)
        self.assertIn("::notice::", output)
        self.assertIn("could not measure how far this branch trails main",
                      output)
        self.assertNotIn("::error::", output)

    def test_a_measurable_current_branch_still_passes_silently(self):
        code, output = self.run_check(compare_raises=False)
        self.assertEqual(code, 0, output)
        self.assertNotIn("could not measure", output)

    def test_a_real_staleness_error_is_still_an_error(self):
        # The degradation must not have softened the one refusal that matters.
        kind, _ = collab.base_staleness("behind", collab.STALE_BASE_LIMIT)
        self.assertEqual(kind, "error")

    def test_an_unmeasurable_comparison_is_never_read_as_a_verdict(self):
        for body_value in (None, {}, {"status": ""}, "nonsense"):
            comparison, reason = collab.comparison_or_reason(
                lambda value=body_value: value)
            self.assertIsNone(comparison, body_value)
            self.assertTrue(reason)

    def test_a_real_comparison_passes_through(self):
        comparison, reason = collab.comparison_or_reason(
            lambda: {"status": "behind", "behind_by": 4})
        self.assertEqual(reason, "")
        self.assertEqual(comparison["behind_by"], 4)

    def test_the_merged_pr_audit_reports_unknown_not_over_the_limit(self):
        def fake_gh_json(args, *, cwd=None):
            joined = " ".join(args)
            if "compare" in joined:
                raise collab.CommandError("gh api compare failed (404): x")
            if "check-runs" in joined:
                return {"check_runs": []}
            return {"headRefOid": "head1234",
                    "mergedAt": "2026-08-17T03:00:00Z"}

        with patch.object(collab, "gh_json", side_effect=fake_gh_json):
            errors = collab.merged_pr_gate_errors(42, base_sha="base5678")
        self.assertTrue(any("staleness unverified" in e for e in errors), errors)
        self.assertFalse(any("past the" in e for e in errors), errors)


class MachineRegistryTests(unittest.TestCase):
    def registry_file(self, text: str) -> Path:
        directory = tempfile.mkdtemp(prefix="civvis-machines-")
        path = Path(directory) / "MACHINES.md"
        path.write_text(text, encoding="utf-8")
        self.addCleanup(lambda: subprocess.run(
            [sys.executable, "-c",
             f"import shutil; shutil.rmtree({directory!r}, ignore_errors=True)"]))
        return path

    def test_collects_every_backticked_id_wherever_it_sits(self):
        registry = collab.machine_registry(self.registry_file(
            "| `martin-desktop` | desktop | `old-name` |\n"
            "- `mbp-martin`\n"
            "Prose mentioning `mbp-m5-max-128` counts too.\n"
        ))
        self.assertEqual(
            registry,
            {"martin-desktop", "old-name", "mbp-martin", "mbp-m5-max-128"},
        )

    def test_rejects_tokens_that_could_never_be_machine_ids(self):
        registry = collab.machine_registry(self.registry_file(
            "`martin-desktop` but not `Not-Valid` nor `has_underscore`\n"
        ))
        self.assertEqual(registry, {"martin-desktop"})

    def test_a_missing_registry_reads_as_none_not_empty(self):
        missing = Path(tempfile.mkdtemp(prefix="civvis-machines-")) / "no.md"
        self.assertIsNone(collab.machine_registry(missing))
        # None means "stay silent"; an empty set would notice every PR.


if __name__ == "__main__":
    unittest.main()


class EveryManagedServiceIsRepairedOnEveryTask(unittest.TestCase):
    """A service `bootstrap` installs and `start` does not repair reaches only
    the machines bootstrapped after it was written.

    That is not hypothetical. `com.civvis.ladder-watchdog` was built on
    2026-08-17 to end a 14.3-hour silent ladder outage, and on 2026-08-18
    `mbp-m5-pro-64` — a host `host_plays_civ6()` calls a Civilization VI seat,
    with both freshness services loaded — did not have it, because the machine
    was bootstrapped before it existed and `start` repaired only the push guard
    and the freshness service.
    """

    def _source(self) -> str:
        return Path(collab.__file__).read_text(encoding="utf-8")

    def test_every_launchagent_installer_is_in_the_registry(self):
        """Discovered, not listed: find the installers, do not trust a list.

        Any `install_*` whose body writes into `~/Library/LaunchAgents` is a
        managed service and has to be repaired like one.
        """
        tree = ast.parse(self._source())
        installers = set()
        for node in ast.walk(tree):
            if not isinstance(node, ast.FunctionDef):
                continue
            if not node.name.startswith("install_"):
                continue
            body = ast.dump(node)
            if "LaunchAgents" in body:
                installers.add(node.name)
        self.assertTrue(installers, "no LaunchAgent installer was found at all")
        registered = {function for _name, function, _absent
                      in collab.MANAGED_SERVICES}
        self.assertEqual(
            installers - registered, set(),
            "an installer writes a LaunchAgent but nothing repairs it on "
            "`start`; add it to MANAGED_SERVICES",
        )

    def test_the_registry_names_real_functions(self):
        for _name, function, _absent in collab.MANAGED_SERVICES:
            with self.subTest(function=function):
                self.assertTrue(callable(getattr(collab, function, None)),
                                f"MANAGED_SERVICES names {function}, which does "
                                f"not exist")

    def test_start_repairs_the_services_and_not_one_of_them(self):
        """`start` used to call `install_freshness_service` directly, which is
        precisely the shape that could not grow."""
        source = self._source()
        start = source.split("def start_task(")[1].split("\ndef ")[0]
        self.assertIn("install_managed_services(root)", start)
        self.assertNotIn("install_freshness_service(root)", start)

    def test_bootstrap_repairs_through_the_same_registry(self):
        source = self._source()
        boot = source.split("def bootstrap_command(")[1].split("\ndef ")[0]
        self.assertIn("install_managed_services(root)", boot)
        for _name, function, _absent in collab.MANAGED_SERVICES:
            with self.subTest(function=function):
                self.assertNotIn(f"{function}(root)", boot)
