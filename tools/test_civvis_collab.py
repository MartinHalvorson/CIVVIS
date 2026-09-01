from pathlib import Path
from unittest import mock
import concurrent.futures
import ast
import plistlib
import re
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
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


def validation_block(checked=True):
    """The template's own validation list, ticked or not.

    Taken from `format_claim_body` rather than written out, so a fixture cannot
    quietly carry fewer items than the gate requires. That is the defect
    `validation_errors` exists for, and a fixture is exactly the place it would
    reappear: every one of these bodies used to carry a single checkbox, which
    is a body the required check would now refuse.
    """
    template = collab.format_claim_body(
        machine="m", agent="a", task="t", paths=["p"], coordinated=()
    )
    section = template.split("## Validation", 1)[1].split("\n## ", 1)[0]
    if checked:
        section = section.replace("- [ ]", "- [x]")
    return "## Validation" + section.rstrip() + "\n"


def body(
    machine="render-win-02",
    agent="codex-47",
    paths="`src/game.rs`, `data/**`",
    coordinated="none",
    checked=True,
):
    return f"""## Ownership claim

- Machine ID: `{machine}`
- Agent/session ID: `{agent}`
- Task: government cleanup
- Claimed paths: {paths}
- Coordinated with: {coordinated}

{validation_block(checked)}"""


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
            # The error check must accept exactly what the plist writes, or
            # every `start` on the fleet reports the service as outdated
            # forever.
            self.assertEqual(payload["ProgramArguments"],
                             collab.macos_freshness_command(root, worker))

        command = payload["ProgramArguments"]
        self.assertEqual(command[:2], ["/bin/sh", "-c"])
        script = command[2]
        for expected in ("refresh", "--scheduled", "--repo",
                         collab.shlex.quote(str(worker))):
            self.assertIn(expected, script)
        self.assertEqual(payload["StartInterval"], collab.FRESHNESS_INTERVAL_SECONDS)
        # ⚠ Nothing in the plist may name a path inside the clone: launchd
        # resolves these BEFORE spawning, and a job that cannot spawn once the
        # clone is deleted can never run its own self-cleanup.
        for spawn_time_key in ("WorkingDirectory", "StandardOutPath",
                               "StandardErrorPath"):
            self.assertNotIn(spawn_time_key, payload)

    def macos_freshness_script(self, base, root, worker):
        """The plist's shell script, with launchctl swapped for a recorder.

        `/bin/launchctl` is named absolutely in the script (launchd agents get
        a minimal PATH), so the test substitutes the one binary it must not
        actually call and runs everything else for real.
        """
        fake_home = base / "home"
        (fake_home / "Library" / "LaunchAgents").mkdir(parents=True, exist_ok=True)
        record = base / "launchctl-args.txt"
        launchctl = base / "launchctl"
        launchctl.write_text(
            f"#!/bin/sh\nprintf '%s\\n' \"$*\" > {collab.shlex.quote(str(record))}\n",
            encoding="utf-8",
        )
        launchctl.chmod(0o755)
        with patch.object(collab.Path, "home", return_value=fake_home):
            payload = collab.plistlib.loads(
                collab.macos_freshness_plist(root, worker)
            )
            plist_path = collab.macos_freshness_plist_path(root)
        plist_path.write_bytes(collab.plistlib.dumps(payload))
        script = payload["ProgramArguments"][2].replace(
            "/bin/launchctl", str(launchctl)
        )
        return script, plist_path, record

    def test_macos_service_removes_itself_when_its_clone_is_gone(self):
        """Six of this host's ten freshness agents pointed at deleted clones,
        each failing every five minutes forever. The job now cleans up: worker
        gone -> delete own plist, boot own label out."""
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            root = base / "repo"
            self.committed_repo(root)
            worker = collab.freshness_worker_path(root)  # never written: "deleted"
            script, plist_path, record = self.macos_freshness_script(
                base, root, worker
            )
            label = collab.freshness_service_label(root)
            subprocess.run(("/bin/sh", "-c", script), check=False,
                           capture_output=True)
            self.assertFalse(
                plist_path.exists(),
                "the agent must delete its own definition when its clone is gone",
            )
            self.assertEqual(
                record.read_text(encoding="utf-8").strip(),
                f"bootout gui/{os.getuid()}/{label}",
                "the agent must boot its own label out of the gui domain",
            )

    def test_macos_service_leaves_itself_alone_while_its_clone_exists(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            root = base / "repo"
            self.committed_repo(root)
            worker = collab.freshness_worker_path(root)
            worker.parent.mkdir(parents=True)
            ran = base / "worker-argv.txt"
            worker.write_text(
                "import sys, pathlib\n"
                f"pathlib.Path({str(ran)!r}).write_text(' '.join(sys.argv[1:]))\n",
                encoding="utf-8",
            )
            script, plist_path, record = self.macos_freshness_script(
                base, root, worker
            )
            subprocess.run(("/bin/sh", "-c", script), check=True,
                           capture_output=True)
            self.assertTrue(plist_path.exists(),
                            "a live clone's agent must not remove itself")
            self.assertFalse(record.exists(), "and must not touch launchctl")
            self.assertEqual(
                ran.read_text(encoding="utf-8"),
                f"refresh --scheduled --repo {collab.main_worktree(root)}",
                "the guarded branch must still run the real refresh",
            )

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


class FreshnessLockLivenessTests(unittest.TestCase):
    """A lock whose holder died must not outlive it by half an hour.

    Measured on 2026-08-23: a `refresh --scheduled` worker hung holding the
    lock, and every `civvis_collab.py start` on that machine failed with
    "another refresh is already running" until `FRESHNESS_LOCK_STALE_SECONDS`
    expired. The lock recorded the holder's PID the whole time.
    """

    def repo(self, base):
        """`freshness_dir` resolves through the repository's common git dir."""
        root = base / "clone"
        subprocess.run(
            ("git", "init", "--initial-branch=main", str(root)),
            check=True,
            capture_output=True,
        )
        return root

    def lock_written_by(self, root, pid):
        path = collab.freshness_dir(root) / "refresh.lock"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"pid": pid}), encoding="utf-8")
        return path

    def dead_pid(self):
        """A PID that is reliably not running: our own child, waited on.

        Spawning and reaping is the only portable way to name a PID that
        certainly existed and certainly does not now — an arbitrary large
        integer could belong to a live process on a busy machine.
        """
        child = subprocess.Popen(("true",))
        child.wait()
        return child.pid

    def test_a_lock_naming_a_dead_process_is_released(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = self.repo(Path(temporary))
            path = self.lock_written_by(root, self.dead_pid())
            self.assertTrue(collab.lock_holder_is_gone(path))
            with collab.FreshnessLock(root) as acquired:
                self.assertTrue(acquired)

    def test_a_lock_naming_a_live_process_is_respected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = self.repo(Path(temporary))
            path = self.lock_written_by(root, os.getpid())
            self.assertFalse(collab.lock_holder_is_gone(path))
            with collab.FreshnessLock(root) as acquired:
                self.assertFalse(acquired)

    def test_an_unreadable_lock_is_assumed_live(self):
        """The file is created O_EXCL and written after, so an empty or
        malformed one is a holder mid-write, not an orphan. Releasing it would
        hand two workers the same lock; the age check is the backstop."""
        with tempfile.TemporaryDirectory() as temporary:
            root = self.repo(Path(temporary))
            for content in ("", "{", '{"pid": "not a number"}', "{}", '{"pid": 0}'):
                path = collab.freshness_dir(root) / "refresh.lock"
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
                self.assertFalse(collab.lock_holder_is_gone(path), content)
            path.unlink()
            self.assertFalse(collab.lock_holder_is_gone(path))

    def test_the_age_backstop_still_releases_a_lock_it_cannot_judge(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = self.repo(Path(temporary))
            path = self.lock_written_by(root, os.getpid())
            stale = time.time() - collab.FRESHNESS_LOCK_STALE_SECONDS - 1
            os.utime(path, (stale, stale))
            with collab.FreshnessLock(root) as acquired:
                self.assertTrue(acquired)


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



def green_rows_for_unmentioned_required(rows):
    """Top up a fixture with a green row for every required check it omits.

    ⚠ Fixtures used to spell the required set out by hand, so adding a third
    required check broke six tests that were not about that check at all. The
    set is derived here for the same reason the production code has one tuple:
    a copy of a fact is a place for it to go stale.
    """
    named = {row.get("name") for row in rows}
    return list(rows) + [
        {"name": name, "startedAt": "2026-07-23T23:59:00Z",
         "status": "COMPLETED", "conclusion": "SUCCESS"}
        for name in collab.REQUIRED_CHECKS if name not in named
    ]

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

    def test_a_live_game_pr_without_a_citation_gets_a_notice_not_an_error(self):
        advisories = []
        errors = collab.validate_pr(
            pr(self.branch, body(paths="`tools/civ6_control/mod/**`"), draft=False),
            files=["tools/civ6_control/mod/CivvisControl.lua"],
            commit_subjects=[],
            advisories=advisories,
        )
        self.assertEqual(errors, [])
        self.assertTrue(any("quotes no shipped source" in note for note in advisories), advisories)

    def test_a_citation_in_the_body_satisfies_the_live_game_rule(self):
        for citation in (
            "modelled on `Base/Assets/UI/DiplomacyActionView.lua:2545`",
            "bands from `GameplayDB.BarbarianAttackForces`",
            "read from Civ6.app/Contents/Assets",
        ):
            with self.subTest(citation=citation):
                advisories = []
                collab.validate_pr(
                    pr(self.branch, body(paths="`tools/civ6_control/mod/**`") + "\n" + citation),
                    files=["tools/civ6_control/mod/CivvisControl.lua"],
                    commit_subjects=[],
                    advisories=advisories,
                )
                self.assertFalse(any("quotes no shipped source" in note for note in advisories))

    def test_game_rs_counts_only_when_the_added_lines_touch_the_live_game_code(self):
        for line, expected in (
            ("    fn barbarian_raid_force_size(&self) -> usize {", True),
            ("    let offer = self.quick_deal_value(pid, other);", True),
            ("    let yields = self.city_yields(city);", False),
        ):
            with self.subTest(line=line):
                advisories = []
                collab.validate_pr(
                    pr(self.branch, body()),
                    files=["src/game.rs"],
                    commit_subjects=[],
                    advisories=advisories,
                    added_lines={"src/game.rs": [line]},
                )
                self.assertEqual(
                    any("quotes no shipped source" in note for note in advisories), expected)

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
        ] + [
            {
                "name": name,
                "started_at": "2026-07-23T22:32:11Z",
                "completed_at": "2026-07-23T22:37:02Z",
                "conclusion": "success",
            }
            for name in collab.REQUIRED_CHECKS
            if name not in ("cargo-test", "collaboration-policy")
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
            for name in collab.REQUIRED_CHECKS
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

    def test_ship_default_waits_through_a_safe_spectator_handoff(self):
        """A healthy 250-turn spectator must not be restarted to verify a ship."""
        args = collab.build_parser().parse_args(["ship"])
        self.assertEqual(
            args.live_timeout_seconds, collab.LIVE_BUILD_HANDOFF_TIMEOUT_S)
        self.assertEqual(args.live_timeout_seconds, 30 * 60.0)

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
            "body": "## What changed\n\nDraft claim; implementation is in "
                    "progress.\n\n" + validation_block(checked=False),
        }
        errors = collab.ship_pr_errors(draft, draft["headRefName"])
        self.assertTrue(any("checkbox" in error for error in errors))
        self.assertTrue(any("What changed" in error for error in errors))

    def test_ship_accepts_a_documented_validated_feature(self):
        finished = {
            "state": "OPEN",
            "headRefName": "agent/m/a/task-20260723T210500Z-a31f",
            "body": "## What changed\n\nAdded the fast shipping path.\n\n"
                    + validation_block(checked=True),
        }
        self.assertEqual(
            collab.ship_pr_errors(finished, finished["headRefName"]), []
        )

    def test_explicit_merge_race_accepts_the_auto_merged_pr(self):
        """Auto-merge can land after the successful-check poll but before PUT."""
        merged = {
            "state": "MERGED",
            "mergeCommit": {"oid": "squash123"},
        }
        with (
            patch.object(collab, "gh_api_write", return_value=None) as write,
            patch.object(collab, "current_pr", return_value=merged),
        ):
            result = collab.merge_pr_or_observe_auto_merge(
                Path.cwd(), number=9, local_head="head123"
            )
        self.assertEqual(result, "squash123")
        write.assert_called_once_with(
            "PUT", f"repos/{collab.REPOSITORY}/pulls/9/merge",
            {"merge_method": "squash", "sha": "head123"}, check=False,
        )

    def test_in_progress_auto_merge_is_rechecked_not_called_a_failure(self):
        still_open = {"state": "OPEN", "mergeCommit": {"oid": "not-yet"}}
        with (
            patch.object(collab, "gh_api_write", return_value=None),
            patch.object(collab, "current_pr", return_value=still_open),
        ):
            result = collab.merge_pr_or_observe_auto_merge(
                Path.cwd(), number=9, local_head="head123"
            )
        self.assertIsNone(result)

    def test_explicit_merge_uses_its_returned_squash_commit(self):
        with (
            patch.object(collab, "gh_api_write",
                         return_value={"merged": True, "sha": "squash123"}),
            patch.object(collab, "current_pr") as current,
        ):
            result = collab.merge_pr_or_observe_auto_merge(
                Path.cwd(), number=9, local_head="head123"
            )
        self.assertEqual(result, "squash123")
        current.assert_not_called()

    def test_real_merge_rejection_still_names_the_reason(self):
        open_pr = {"state": "OPEN", "mergeCommit": {"oid": "not-yet"}}
        with (
            patch.object(collab, "gh_api_write",
                         return_value={"merged": False, "message": "not mergeable"}),
            patch.object(collab, "current_pr", return_value=open_pr),
        ):
            with self.assertRaisesRegex(collab.CommandError, "not mergeable"):
                collab.merge_pr_or_observe_auto_merge(
                    Path.cwd(), number=9, local_head="head123"
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
        self.assertEqual(
            collab.required_check_state(
                green_rows_for_unmentioned_required(rows)), ("success", []))

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
                green_rows_for_unmentioned_required(rows),
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
            ("pending", sorted(collab.REQUIRED_CHECKS)),
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
            collab.required_check_state(
                green_rows_for_unmentioned_required(failed)),
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
                "name": "overwrite-guard",
                "startedAt": "2026-08-18T00:52:20Z",
                "detailsUrl": "https://github.com/o/r/actions/runs/333/job/3",
            },
        ]
        original = collab.gh_api_write
        collab.gh_api_write = fake_write
        try:
            self.assertTrue(collab.rerun_required_check(rows, "cargo-test"))
            # An advisory check: `ship` does not gate on it, so it must not
            # spend CI re-running it either. This case named `rust-quality`
            # until it became required — and being required is exactly what
            # earns a check the retry, because a cancelled run reaches no
            # verdict and GitHub restarts nothing on its own.
            self.assertFalse(collab.rerun_required_check(rows, "overwrite-guard"))
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


class EveryCheckIsRequiredOrSaysWhyNot(unittest.TestCase):
    """There is no branch protection on this repository, so `REQUIRED_CHECKS`
    is the only thing that makes a check binding.

    `rust-quality` was in neither list and merged red: five commits on `main`
    in its forty most recent runs are failures, and #1954 merged with its final
    `rust-quality` run FAILING while every other check was green. A ratchet
    nobody has to pass ratchets nothing.

    Discovered, not listed: the check names are read out of the workflows that
    actually run on a pull request, so a gate added tomorrow is a deliberate
    decision rather than an omission nobody can tell from an oversight.
    """

    WORKFLOWS = Path(collab.__file__).resolve().parent.parent / ".github" / "workflows"

    def pull_request_checks(self) -> set:
        """Every check name a pull request gets: one per job, not per workflow.

        `published-build` and `control-mod` are jobs inside `cargo-test`'s
        workflow, so keying on workflow names alone would miss two of the six.
        """
        names = set()
        for path in sorted(self.WORKFLOWS.glob("*.yml")):
            text = path.read_text(encoding="utf-8")
            trigger = text.split("jobs:", 1)[0]
            if "pull_request" not in trigger:
                continue
            inside = False
            for line in text.splitlines():
                if line.startswith("jobs:"):
                    inside = True
                    continue
                if inside and line and not line[0].isspace():
                    break
                if inside and re.match(r"^  [A-Za-z][\w-]*:\s*$", line):
                    names.add(line.strip().rstrip(":"))
        return names

    def test_the_workflows_are_read_not_assumed(self):
        found = self.pull_request_checks()
        self.assertTrue(found, "no pull-request workflow jobs were found")
        # The two that live inside another workflow are the reason this reads
        # jobs rather than workflow names.
        self.assertIn("published-build", found)
        self.assertIn("control-mod", found)

    def test_every_one_is_required_or_carries_a_reason(self):
        unaccounted = sorted(
            self.pull_request_checks()
            - set(collab.REQUIRED_CHECKS)
            - set(collab.ADVISORY_CHECKS))
        self.assertEqual(unaccounted, [], (
            "these checks run on every pull request and nothing says whether "
            "they may block a merge. Add them to REQUIRED_CHECKS, or to "
            "ADVISORY_CHECKS with the reason they may go red and merge anyway: "
            f"{unaccounted}"))

    def test_no_check_is_both(self):
        self.assertEqual(
            set(collab.REQUIRED_CHECKS) & set(collab.ADVISORY_CHECKS), set())

    def test_an_advisory_reason_is_a_reason(self):
        for name, reason in collab.ADVISORY_CHECKS.items():
            with self.subTest(check=name):
                self.assertGreater(len(reason), 40,
                                   f"{name} is waved through without a reason")

    def test_rust_quality_is_required(self):
        """The specific hole this closes; a rename must not silently reopen it."""
        self.assertIn("rust-quality", collab.REQUIRED_CHECKS)


class AManagedJobThatFailsToLoadSaysSo(unittest.TestCase):
    """`launchctl bootout` is asynchronous and `bootstrap` behind it fails.

    Measured on `mbp-m5-max-128` 2026-08-18: `bootstrap` printed "installed
    CIVVIS spectator service" while `launchctl print` could not find the job at
    all. The identical command by hand a moment later loaded it first try — the
    signature of a race, not a bad plist. Every call in the loader passed
    `check=False`, so nothing said a word, and the service simply stopped
    existing.
    """

    def _driver(self, script):
        """A fake `run` whose `launchctl print` answers follow `script`."""
        state = {"loaded": script.pop(0), "calls": []}

        class Done:
            def __init__(self, rc):
                self.returncode = rc
                self.stdout = ""
                self.stderr = ""

        def run(argv, **kwargs):
            state["calls"].append(list(argv))
            verb = argv[1] if len(argv) > 1 else ""
            if verb == "print":
                return Done(0 if state["loaded"] else 1)
            if verb == "bootout":
                return Done(0)
            if verb == "bootstrap":
                if script:
                    state["loaded"] = script.pop(0)
                return Done(0 if state["loaded"] else 1)
            return Done(0)

        return run, state

    def test_a_bootstrap_that_loses_the_race_is_retried(self):
        # loaded, then still-not-loaded once, then loaded.
        run, state = self._driver([True, False, True])
        with mock.patch.object(collab, "run", run), \
             mock.patch.object(collab.time, "sleep", lambda s: None):
            collab.load_managed_job("com.example.job", Path("/x.plist"))
        bootstraps = [c for c in state["calls"] if c[1] == "bootstrap"]
        self.assertGreaterEqual(len(bootstraps), 2, state["calls"])

    def test_a_job_that_never_loads_raises_rather_than_reporting_success(self):
        run, _ = self._driver([False])
        with mock.patch.object(collab, "run", run), \
             mock.patch.object(collab.time, "sleep", lambda s: None):
            with self.assertRaises(collab.CommandError) as caught:
                collab.load_managed_job("com.example.job", Path("/x.plist"),
                                               attempts=3)
        self.assertIn("absent", str(caught.exception))

    def test_a_job_that_loads_first_try_is_not_retried(self):
        run, state = self._driver([False, True])
        with mock.patch.object(collab, "run", run), \
             mock.patch.object(collab.time, "sleep", lambda s: None):
            collab.load_managed_job("com.example.job", Path("/x.plist"))
        bootstraps = [c for c in state["calls"] if c[1] == "bootstrap"]
        self.assertEqual(len(bootstraps), 1, state["calls"])

    def test_the_bootout_is_waited_out_before_bootstrapping(self):
        run, state = self._driver([True, True])
        with mock.patch.object(collab, "run", run), \
             mock.patch.object(collab.time, "sleep", lambda s: None):
            try:
                collab.load_managed_job("com.example.job", Path("/x.plist"),
                                               attempts=2)
            except collab.CommandError:
                pass
        order = [c[1] for c in state["calls"]]
        self.assertIn("bootout", order)
        self.assertLess(order.index("bootout"), order.index("bootstrap"),
                        "the teardown has to be waited out first")


class GeneratedDocumentMergeTests(unittest.TestCase):
    """`ship` resolves a generated-document conflict and nothing else."""

    def git(self, repo, *args):
        return subprocess.run(
            ("git", "-C", str(repo), *args),
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    def repo_with_generator(self, root):
        """A repo whose `docs/generated.txt` is the sorted union of `arms/`."""
        self.git(root.parent, "init", "--initial-branch=main", str(root))
        self.git(root, "config", "user.email", "merge@example.invalid")
        self.git(root, "config", "user.name", "Merge Test")
        (root / "tools").mkdir()
        (root / "docs").mkdir()
        (root / "arms").mkdir()
        (root / "tools" / "gen.py").write_text(
            "import sys\n"
            "from pathlib import Path\n"
            "root = Path(__file__).resolve().parent.parent\n"
            "arms = sorted(p.name for p in (root / 'arms').iterdir())\n"
            "want = ''.join(name + '\\n' for name in arms)\n"
            "out = root / 'docs' / 'generated.txt'\n"
            "if '--check' in sys.argv:\n"
            "    sys.exit(0 if out.read_text() == want else 1)\n"
            "out.write_text(want)\n",
            encoding="utf-8",
        )
        (root / "arms" / "base").write_text("", encoding="utf-8")
        (root / "docs" / "generated.txt").write_text("base\n", encoding="utf-8")
        self.git(root, "add", "-A")
        self.git(root, "commit", "-m", "seed")

    def diverge(self, root):
        """Two branches that each register an arm, so the generated file collides."""
        self.git(root, "checkout", "-b", "task")
        (root / "arms" / "mine").write_text("", encoding="utf-8")
        (root / "docs" / "generated.txt").write_text("base\nmine\n", encoding="utf-8")
        self.git(root, "add", "-A")
        self.git(root, "commit", "-m", "mine")
        self.git(root, "checkout", "main")
        (root / "arms" / "theirs").write_text("", encoding="utf-8")
        (root / "docs" / "generated.txt").write_text("base\ntheirs\n", encoding="utf-8")
        self.git(root, "add", "-A")
        self.git(root, "commit", "-m", "theirs")
        self.git(root, "checkout", "task")

    def registry(self):
        return {"docs/generated.txt": ("tools/gen.py", "--write")}

    def test_a_generated_only_conflict_is_resolved_by_regenerating_both_sides(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            self.repo_with_generator(root)
            self.diverge(root)
            merged = subprocess.run(
                ("git", "-C", str(root), "merge", "--no-edit", "main"),
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(merged.returncode, 0, "the fixture must actually conflict")
            with patch.dict(collab.REGENERATED_ON_MERGE, self.registry(), clear=True):
                conflicted = collab.regenerable_conflicts(root)
                self.assertEqual(conflicted, ["docs/generated.txt"])
                collab.resolve_by_regenerating(root, conflicted)
            self.git(root, "commit", "--no-edit")
            # Both registrations survive: the resolution is the generator's
            # output over the merged sources, not either branch's file.
            self.assertEqual(
                (root / "docs" / "generated.txt").read_text(encoding="utf-8"),
                "base\nmine\ntheirs\n",
            )
            self.assertEqual(self.git(root, "diff", "--name-only", "--diff-filter=U"), "")

    def test_a_conflict_touching_anything_else_is_left_to_the_author(self):
        """⚠ The whole safety argument: regenerating over a conflicted *source*
        would publish one side's arms and call it a merge."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            self.repo_with_generator(root)
            self.git(root, "checkout", "-b", "task")
            (root / "arms" / "mine").write_text("", encoding="utf-8")
            (root / "docs" / "generated.txt").write_text("base\nmine\n", encoding="utf-8")
            (root / "tools" / "gen.py").write_text("# mine\n", encoding="utf-8")
            self.git(root, "add", "-A")
            self.git(root, "commit", "-m", "mine")
            self.git(root, "checkout", "main")
            (root / "arms" / "theirs").write_text("", encoding="utf-8")
            (root / "docs" / "generated.txt").write_text("base\ntheirs\n", encoding="utf-8")
            (root / "tools" / "gen.py").write_text("# theirs\n", encoding="utf-8")
            self.git(root, "add", "-A")
            self.git(root, "commit", "-m", "theirs")
            self.git(root, "checkout", "task")
            subprocess.run(
                ("git", "-C", str(root), "merge", "--no-edit", "main"),
                capture_output=True,
                text=True,
            )
            unmerged = self.git(root, "diff", "--name-only", "--diff-filter=U").split("\n")
            self.assertIn("tools/gen.py", unmerged)
            with patch.dict(collab.REGENERATED_ON_MERGE, self.registry(), clear=True):
                self.assertEqual(
                    collab.regenerable_conflicts(root),
                    [],
                    "a source conflict must disable the automatic resolution entirely",
                )

    def test_a_clean_tree_offers_nothing_to_resolve(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            self.repo_with_generator(root)
            with patch.dict(collab.REGENERATED_ON_MERGE, self.registry(), clear=True):
                self.assertEqual(collab.regenerable_conflicts(root), [])

    def test_the_registry_covers_every_artifact_the_generator_owns(self):
        """Discovered, not listed: a third artifact cannot be added to
        `eval_manifest.py` and quietly left out of the merge resolver."""
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import eval_manifest

        self.assertEqual(
            sorted(collab.REGENERATED_ON_MERGE),
            sorted(eval_manifest.GENERATED_OUTPUTS),
        )
        self.assertTrue(eval_manifest.GENERATED_OUTPUTS)

    def test_settle_finishes_a_generated_only_conflict(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            self.repo_with_generator(root)
            self.diverge(root)
            subprocess.run(
                ("git", "-C", str(root), "merge", "--no-edit", "main"),
                capture_output=True,
                text=True,
            )
            with patch.dict(collab.REGENERATED_ON_MERGE, self.registry(), clear=True):
                collab.settle_merge_conflict(root, "conflict in docs/generated.txt")
            self.assertEqual(self.git(root, "diff", "--name-only", "--diff-filter=U"), "")
            self.assertEqual(
                (root / "docs" / "generated.txt").read_text(encoding="utf-8"),
                "base\nmine\ntheirs\n",
            )
            # The merge is committed, not left staged for somebody else to find.
            self.assertEqual(self.git(root, "status", "--porcelain"), "")

    def test_settle_re_raises_anything_it_may_not_touch(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            self.repo_with_generator(root)
            with patch.dict(collab.REGENERATED_ON_MERGE, self.registry(), clear=True):
                with self.assertRaises(collab.CommandError) as raised:
                    collab.settle_merge_conflict(root, "CONFLICT in src/game.rs")
            self.assertIn("resolve this task worktree", str(raised.exception))
            self.assertIn("src/game.rs", str(raised.exception))


# --- Writing about the ownership gate must not switch it off ----------------
#
# Every fixture in the four classes below carries the exact text it is about,
# because that is what the old code matched: a body that *discussed* the
# protocol replaced the claim the required `collaboration-policy` check then
# validated. `assert_quotes` asserts the quoted line really is present before
# asserting that it changed nothing, so a fixture that quietly stopped carrying
# it would fail rather than pass while proving nothing — the same guard
# `test_overwrite_guard.assert_mentions_but_does_not_waive` uses.

BRANCH = "agent/render-win-02/codex-47/government-cleanup-20260723T210500Z-a31f"


class QuotationFixture(unittest.TestCase):
    def assert_quotes(self, text, quoted):
        self.assertIn(quoted, text,
                      "fixture no longer contains the text it exists to quote")


class ClaimHijackTests(QuotationFixture):
    """The claim is the ownership block, not the last matching line anywhere.

    `parse_claims` matched `^\\s*-\\s*([^:]+):` over the whole body, at any
    indentation, and assigned on every match — last occurrence wins. Both
    directions were live and neither made a sound.
    """

    def test_a_fenced_example_block_does_not_replace_the_claim(self):
        quoted = "- Claimed paths: `src/**`, `web/**`"
        text = body() + f"\nThe template looks like:\n\n```\n{quoted}\n```\n"
        self.assert_quotes(text, quoted)
        self.assertEqual(
            collab.split_paths(collab.parse_claims(text)["paths"]),
            ["src/game.rs", "data/**"])

    def test_a_fenced_example_block_does_not_replace_the_machine_id(self):
        quoted = "- Machine ID: `render-win-99`"
        text = body() + f"\nAnother machine writes:\n\n```\n{quoted}\n```\n"
        self.assert_quotes(text, quoted)
        self.assertEqual(collab.parse_claims(text)["machine"], "render-win-02")

    def test_an_indented_example_block_is_not_a_claim(self):
        quoted = "    - Claimed paths: `src/**`"
        text = body() + f"\nFor example:\n\n{quoted}\n"
        self.assert_quotes(text, quoted)
        self.assertEqual(
            collab.split_paths(collab.parse_claims(text)["paths"]),
            ["src/game.rs", "data/**"])

    def test_a_later_unfenced_line_does_not_win_and_is_reported(self):
        quoted = "- Coordinated with: #4242"
        text = body().replace(
            "- Coordinated with: none",
            f"- Coordinated with: none\n{quoted}",
        )
        self.assert_quotes(text, quoted)
        self.assertEqual(collab.parse_claims(text)["coordinated"], "none")
        advisories = []
        collab.validate_pr(pr(BRANCH, text), files=["src/game.rs"],
                           commit_subjects=[], advisories=advisories)
        self.assertTrue(any("was ignored" in note for note in advisories),
                        advisories)

    def test_a_claim_line_outside_the_ownership_section_is_ignored(self):
        quoted = "- Claimed paths: `src/**`"
        text = body() + f"\n## Notes for integration\n\n{quoted}\n"
        self.assert_quotes(text, quoted)
        self.assertEqual(
            collab.split_paths(collab.parse_claims(text)["paths"]),
            ["src/game.rs", "data/**"])

    def test_quoting_a_broad_claim_no_longer_widens_the_real_one(self):
        """The end-to-end version: this silenced the path gate completely."""
        quoted = "- Claimed paths: `src/**`, `web/**`, `docs/**`"
        text = body(paths="`tools/only_this.py`") + (
            f"\nA claim is written like this:\n\n```\n{quoted}\n```\n")
        self.assert_quotes(text, quoted)
        errors = collab.validate_pr(pr(BRANCH, text), files=["src/game.rs"],
                                    commit_subjects=[])
        self.assertTrue(any("changed path is not claimed" in e for e in errors),
                        errors)

    def test_quoting_a_foreign_machine_no_longer_refuses_an_honest_pr(self):
        """The same looseness in the other direction: a false refusal."""
        quoted = "- Machine ID: `render-win-99`"
        text = body() + f"\nOther agents write:\n\n```\n{quoted}\n```\n"
        self.assert_quotes(text, quoted)
        errors = collab.validate_pr(pr(BRANCH, text), files=["src/game.rs"],
                                    commit_subjects=[])
        self.assertEqual(errors, [])

    def test_a_body_without_the_heading_still_parses_and_says_so(self):
        text = body().replace("## Ownership claim\n", "")
        self.assertEqual(collab.parse_claims(text)["machine"], "render-win-02")
        advisories = []
        collab.validate_pr(pr(BRANCH, text), files=["src/game.rs"],
                           commit_subjects=[], advisories=advisories)
        self.assertTrue(
            any("no '## Ownership claim' heading" in note for note in advisories),
            advisories)

    def test_the_launcher_body_round_trips_through_its_own_checker(self):
        """A legitimate, launcher-written claim must still work.

        `start` writes this body; `check-pr` reads it. If tightening the parser
        broke the round trip, every new task would open refusing itself.
        """
        machine, agent = "render-win-02", "codex-47"
        text = collab.format_claim_body(
            machine=machine, agent=agent, task="government-cleanup",
            paths=["src/game.rs", "data/**"], coordinated=[2337],
        )
        claims = collab.parse_claims(text)
        self.assertEqual(claims["machine"], machine)
        self.assertEqual(claims["agent"], agent)
        self.assertEqual(collab.split_paths(claims["paths"]),
                         ["src/game.rs", "data/**"])
        self.assertEqual(collab.split_coordination(claims["coordinated"]), {2337})
        # A draft, as `start` creates it: no errors, unticked boxes and all.
        self.assertEqual(
            collab.validate_pr(pr(BRANCH, text), files=["src/game.rs"],
                               commit_subjects=[]),
            [],
        )
        # And ready once the boxes are ticked, which is the only edit `ship`
        # asks the author for.
        ready = text.replace("- [ ]", "- [x]").replace(
            "Draft claim; implementation is in progress.", "Did the thing.")
        self.assertEqual(
            collab.validate_pr(pr(BRANCH, ready, draft=False),
                               files=["src/game.rs"], commit_subjects=[]),
            [],
        )
        self.assertEqual(collab.ship_pr_errors(
            {"state": "OPEN", "headRefName": BRANCH, "body": ready}, BRANCH), [])


class CoordinationEscapeTests(QuotationFixture):
    """`Coordinated with: #N` is the escape that makes the overlap error optional."""

    OTHERS = {5: {"src/game.rs": [(100, 140)]}}
    MINE = {"src/game.rs": [(110, 130)]}

    def verdict(self, text, *, other_coordination=None, advisories=None):
        return collab.validate_pr(
            pr(BRANCH, text, number=9, draft=False),
            files=["src/game.rs"], commit_subjects=[], ranges=dict(self.MINE),
            other_ranges={k: dict(v) for k, v in self.OTHERS.items()},
            other_coordination=other_coordination,
            advisories=advisories,
        )

    def test_a_fenced_coordination_line_does_not_silence_the_overlap(self):
        quoted = "- Coordinated with: #5"
        text = body() + f"\nCoordination is recorded like this:\n\n```\n{quoted}\n```\n"
        self.assert_quotes(text, quoted)
        errors = self.verdict(text)
        self.assertTrue(any("edits collide with PR #5" in e for e in errors), errors)

    def test_a_real_coordination_still_passes_but_is_recorded(self):
        advisories = []
        errors = self.verdict(body(coordinated="#5"), advisories=advisories)
        self.assertEqual(errors, [])
        self.assertTrue(
            any("does not name PR #9 back" in note for note in advisories),
            advisories)

    def test_a_reciprocated_coordination_is_not_flagged(self):
        advisories = []
        errors = self.verdict(body(coordinated="#5"),
                              other_coordination={5: {9}}, advisories=advisories)
        self.assertEqual(errors, [])
        self.assertFalse(any("does not name PR #9 back" in n for n in advisories),
                         advisories)


class ValidationSectionTests(QuotationFixture):
    """A gate that only checks unticked boxes rewards deleting the boxes."""

    def ready(self, text):
        return collab.validate_pr(pr(BRANCH, text, draft=False),
                                  files=["src/game.rs"], commit_subjects=[])

    def test_a_ready_pr_with_no_validation_section_is_refused(self):
        text = body().split("## Validation")[0]
        self.assertNotIn("- [", text)
        self.assertTrue(any("must carry a '## Validation' section" in e
                            for e in self.ready(text)))

    def test_ship_refuses_a_body_with_no_validation_section(self):
        text = body().split("## Validation")[0] + "## What changed\n\nDid it.\n"
        errors = collab.ship_pr_errors(
            {"state": "OPEN", "headRefName": BRANCH, "body": text}, BRANCH)
        self.assertTrue(any("must carry a '## Validation' section" in e
                            for e in errors), errors)

    def test_a_shortened_checklist_is_refused(self):
        text = body().replace(
            "- [x] Soak run for engine changes, or reason it is not applicable\n", "")
        self.assertTrue(any("the template asks for" in e for e in self.ready(text)),
                        self.ready(text))

    def test_a_fenced_unticked_box_no_longer_refuses_a_ready_pr(self):
        quoted = "- [ ] Relevant focused tests"
        text = body() + f"\nThe template ships:\n\n```\n{quoted}\n```\n"
        self.assert_quotes(text, quoted)
        self.assertEqual(self.ready(text), [])

    def test_an_unticked_box_still_refuses_a_ready_pr(self):
        self.assertTrue(any("must complete every validation checkbox" in e
                            for e in self.ready(body(checked=False))))

    def test_the_required_count_follows_the_template(self):
        template = collab.format_claim_body(
            machine="m", agent="a", task="t", paths=["p"], coordinated=())
        self.assertEqual(collab.required_validation_items(),
                         template.count("- [ ] "))
        self.assertGreater(collab.required_validation_items(), 1)


class EvidenceGateTests(unittest.TestCase):
    """`docs/EVAL_INTEGRITY.md` §4: a promoted number is not quotable alone."""

    def report(self, *lines):
        return collab.unevidenced_effect_sizes({"docs/EVAL.md": list(lines)})

    def test_continuous_integration_is_not_a_confidence_interval(self):
        self.assertTrue(self.report(
            "The adaptive seat is worth +61 Elo-equivalent.",
            "CI runs the gate on every pull request, so nothing regresses."))

    def test_a_real_interval_still_carries_the_number(self):
        self.assertFalse(self.report(
            "The adaptive seat is worth +61 Elo-equivalent, 95% CI [+51, +147]."))

    def test_ci_naming_its_interval_still_carries_the_number(self):
        self.assertFalse(self.report(
            "holy-lane-parity returned +99 Elo, CI [+51, +147] on disjoint seeds."))

    def test_a_distant_unrelated_issue_number_does_not_launder_a_bare_claim(self):
        self.assertTrue(self.report(
            "The adaptive seat is worth +61 Elo-equivalent.",
            "x" * (collab.CITATION_WINDOW_CHARS + 40),
            "This follows the worktree cleanup in #2290."))

    def test_a_citation_attached_to_the_figure_still_carries_it(self):
        self.assertFalse(self.report(
            "`city_target_floor = 6` measured **-41 Elo** and was removed (#1504)."))


class PushGuardMarkerTests(unittest.TestCase):
    """Naming this repository's guard is not being it."""

    def install_over(self, existing):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        (root / "tools").mkdir()
        (root / "tools" / "civvis_push_guard.py").write_text(
            f'#!/usr/bin/env python3\nPUSH_GUARD_MARKER = "{collab.PUSH_GUARD_MARKER}"\n',
            encoding="utf-8")
        hooks = root / ".git" / "hooks"
        hooks.mkdir(parents=True)
        (hooks / "pre-push").write_text(existing, encoding="utf-8")
        with patch.object(collab, "common_git_dir", return_value=root / ".git"):
            collab.install_push_guard(root)
        return (hooks / "pre-push").read_text(encoding="utf-8")

    def test_a_hook_that_only_mentions_the_guard_is_not_overwritten(self):
        mine = ("#!/bin/sh\n"
                f"# runs before the {collab.PUSH_GUARD_MARKER} does\n"
                "exit 0\n")
        self.assertIn(collab.PUSH_GUARD_MARKER, mine)
        with self.assertRaises(collab.CommandError) as raised:
            self.install_over(mine)
        self.assertIn("unmanaged pre-push hook", str(raised.exception))

    def test_a_hook_carrying_the_marker_as_its_own_line_is_replaced(self):
        replaced = self.install_over(
            f"#!/usr/bin/env python3\n# {collab.PUSH_GUARD_MARKER}\nold body\n")
        self.assertIn("PUSH_GUARD_MARKER", replaced)
        self.assertNotIn("old body", replaced)

    def test_a_hook_carrying_the_constant_assignment_is_replaced(self):
        replaced = self.install_over(
            "#!/usr/bin/env python3\n"
            f'PUSH_GUARD_MARKER = "{collab.PUSH_GUARD_MARKER}"\nold body\n')
        self.assertNotIn("old body", replaced)

    def test_the_marker_matches_the_versioned_guards_own(self):
        self.assertEqual(collab.PUSH_GUARD_MARKER, push_guard.PUSH_GUARD_MARKER)
        source = (Path(__file__).resolve().parent / "civvis_push_guard.py").read_text(
            encoding="utf-8")
        self.assertTrue(collab.is_managed_push_guard(source.encode("utf-8")),
                        "the shipped guard must recognise itself")


class OneIdiomTests(unittest.TestCase):
    """The fleet gets one anchoring idiom, and that is a check, not a claim.

    `overwrite_guard.WAIVER`, `speed_ab.ACKNOWLEDGEMENT` and `CLAIM_LINE` all
    exist because a marker matched as a bare substring let a body switch a
    required gate off by writing about it. Three hand-maintained copies of that
    shape drift, and the way they drift is one of them getting looser.
    """

    def setUp(self):
        import overwrite_guard
        import speed_ab
        self.guard = overwrite_guard
        self.speed = speed_ab

    def test_all_three_gates_blank_fenced_blocks_identically(self):
        text = "intro\n- Machine ID: `real`\n```\n- Machine ID: `fake`\n```\ntail\n"
        self.assertEqual(collab.prose(text), self.guard.prose(text))
        self.assertEqual(collab.prose(text), self.speed.prose(text))

    def test_all_three_gates_agree_on_what_a_fence_is(self):
        self.assertEqual(collab.FENCE.pattern, self.guard.FENCE.pattern)
        self.assertEqual(collab.FENCE.pattern, self.speed.FENCE.pattern)

    def test_the_claim_line_anchors_the_way_the_waiver_does(self):
        bullet = r"^(?:[-*+][ \t]+)"
        self.assertTrue(self.guard.WAIVER.pattern.startswith(bullet))
        self.assertTrue(self.speed.ACKNOWLEDGEMENT.pattern.startswith(bullet))
        self.assertTrue(collab.CLAIM_LINE.pattern.startswith(bullet))

    def test_the_claim_line_matches_under_the_same_flags(self):
        self.assertEqual(collab.CLAIM_LINE.flags, self.guard.WAIVER.flags)

    def test_every_anchored_marker_refuses_four_space_indentation(self):
        """Four leading spaces is a Markdown code block in all three."""
        self.assertIsNone(self.guard.waiver_reason("    overwrite-guard: allow why"))
        self.assertIsNone(self.speed.acknowledged("    paired-cost: allow why"))
        self.assertEqual(collab.parse_claims("    - Machine ID: `sneaky`"), {})


class TheLadderKeeperIsInstalledSomewhereItSurvives(unittest.TestCase):
    """Two ways the keeper for the live Civ 6 ladder stopped being a keeper.

    Both were live on 2026-08-28 and neither is visible from the process table:
    the job was loaded and green the whole time.
    """

    def test_a_state_directory_is_not_somewhere_a_service_may_live(self):
        home = Path.home()
        self.assertTrue(collab.ephemeral_service_source(
            home / ".civvis-gene-batch-joined-20260828/repo/tools/ops/ladder_watchdog.py"))
        self.assertTrue(collab.ephemeral_service_source(
            home / ".civvis-gene-batch/sources/abc/tools/ops/ladder_watchdog.py"))
        self.assertFalse(collab.ephemeral_service_source(
            home / "CIVVIS/tools/ops/ladder_watchdog.py"))
        self.assertFalse(collab.ephemeral_service_source(
            home / "civvis-main/tools/ops/ladder_watchdog.py"))

    def test_a_durable_keeper_is_recognised_and_an_ephemeral_one_is_not(self):
        with tempfile.TemporaryDirectory() as tmp:
            durable = Path(tmp) / "durable.plist"
            durable.write_bytes(collab.macos_ladder_watchdog_plist(
                Path.home() / "CIVVIS/tools/ops/ladder_watchdog.py"))
            self.assertTrue(collab.installed_keeper_is_durable(durable))

            ephemeral = Path(tmp) / "ephemeral.plist"
            ephemeral.write_bytes(collab.macos_ladder_watchdog_plist(
                Path.home()
                / ".civvis-gene-batch-joined-20260828/repo/tools/ops/ladder_watchdog.py"))
            self.assertFalse(collab.installed_keeper_is_durable(ephemeral))

            self.assertFalse(collab.installed_keeper_is_durable(
                Path(tmp) / "absent.plist"))

    def test_the_keeper_hands_the_operators_own_wrapper_to_the_restart(self):
        """Without `--supervisor` a recovery restarts the STOCK launcher.

        That is how the configured chain — difficulty, victory lane, mirror
        bounds, attempts per cycle — was replaced by the tree's defaults on
        2026-08-19 and stayed replaced.
        """
        watchdog = Path.home() / "CIVVIS/tools/ops/ladder_watchdog.py"
        wrapper = Path.home() / "civvis-verification-launch.command"
        arguments = plistlib.loads(
            collab.macos_ladder_watchdog_plist(watchdog, wrapper)
        )["ProgramArguments"]
        self.assertEqual(arguments[-2:], ["--supervisor", str(wrapper)])
        self.assertIn("--stale-hours", arguments)

        without = plistlib.loads(
            collab.macos_ladder_watchdog_plist(watchdog))["ProgramArguments"]
        self.assertNotIn("--supervisor", without)

    def test_an_ephemeral_bootstrap_leaves_a_durable_keeper_alone(self):
        """It may install one where none exists; it may not replace a good one."""
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            agents = home / "Library" / "LaunchAgents"
            agents.mkdir(parents=True)
            path = agents / f"{collab.LADDER_WATCHDOG_LABEL}.plist"
            durable = home / "CIVVIS" / "tools" / "ops" / "ladder_watchdog.py"
            durable.parent.mkdir(parents=True)
            durable.write_text("#\n")
            path.write_bytes(collab.macos_ladder_watchdog_plist(durable))
            before = path.read_bytes()

            scratch = (home / ".civvis-gene-batch-joined-20260828" / "repo"
                       / "tools" / "ops")
            scratch.mkdir(parents=True)
            watchdog = scratch / "ladder_watchdog.py"
            watchdog.write_text("#\n")
            supervisor = scratch / "civvis-game-supervisor.sh"
            supervisor.write_text("#\n")
            (home / "civvis-civ6-runs").mkdir()

            with mock.patch.object(collab.Path, "home", staticmethod(lambda: home)), \
                    mock.patch.object(collab, "repo_root",
                                      lambda repo: Path(repo)), \
                    mock.patch.object(collab, "retire_ladder_keepalive_job",
                                      lambda: None), \
                    mock.patch.object(collab.sys, "platform", "darwin"):
                written = collab.install_ladder_supervisor(
                    home / ".civvis-gene-batch-joined-20260828" / "repo")

            self.assertEqual(written, [path])
            self.assertEqual(path.read_bytes(), before,
                             "an ephemeral bootstrap overwrote the live keeper")



if __name__ == "__main__":
    unittest.main()
