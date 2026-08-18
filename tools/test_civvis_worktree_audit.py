#!/usr/bin/env python3
"""The reaper removes finished task worktrees and nothing else.

On 2026-08-18 it removed `civvis-spectator-src` — the tree the live civvis.ai
exhibition runs its supervisor from — and took the exhibition down. That tree
passed every check the reaper had: clean, idle, HEAD plainly on GitHub. It was
never a task worktree at all. `docs/SPECTATOR_DEPLOY.md` prescribes creating it
with `--detach`, so the shape that distinguishes it was already written down and
simply not consulted.
"""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civvis_worktree_audit as audit  # noqa: E402


def run(*args, cwd=None):
    return subprocess.run(args, cwd=cwd, capture_output=True, text=True, check=True)


def a_repo_with(tmp: Path, worktrees: dict[str, str]) -> Path:
    """A real repo with a real remote and real worktrees, keyed name -> branch.

    A branch of `None` means detached, which is the case that mattered.
    """
    origin = tmp / "origin.git"
    run("git", "init", "--bare", "-q", str(origin))
    repo = tmp / "repo"
    run("git", "clone", "-q", str(origin), str(repo))
    run("git", "-C", str(repo), "config", "user.email", "t@example.com")
    run("git", "-C", str(repo), "config", "user.name", "t")
    (repo / "a.txt").write_text("one\n")
    run("git", "-C", str(repo), "add", "a.txt")
    run("git", "-C", str(repo), "commit", "-qm", "one")
    run("git", "-C", str(repo), "branch", "-M", "main")
    run("git", "-C", str(repo), "push", "-q", "-u", "origin", "main")
    for name, branch in worktrees.items():
        target = tmp / name
        if branch is None:
            run("git", "-C", str(repo), "worktree", "add", "-q", "--detach",
                str(target), "origin/main")
        else:
            run("git", "-C", str(repo), "worktree", "add", "-q", "-b", branch,
                str(target), "origin/main")
    return repo


class OnlyTaskWorktreesAreRemoved(unittest.TestCase):
    def reaped(self, repo: Path):
        with mock.patch.object(audit, "process_is_running_from", lambda p: False), \
             mock.patch.object(audit, "on_github", lambda repo, head: True):
            return audit.reap(str(repo), [], idle_minutes=0.0, apply=False)

    def test_a_detached_deploy_checkout_is_never_a_candidate(self):
        """The exact shape of `civvis-spectator-src`, and the exact outage."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {"deploy": None})
            rows = self.reaped(repo)
            self.assertEqual(
                [r["path"] for r in rows], [],
                "a detached worktree is a deploy checkout, not finished task "
                "work; removing one took civvis.ai down",
            )

    def test_a_task_worktree_is_still_a_candidate(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {"task": "agent/m/a/finished-thing"})
            rows = self.reaped(repo)
            self.assertEqual(len(rows), 1, rows)
            self.assertTrue(rows[0]["branch"].startswith("agent/"))

    def test_a_branch_that_is_not_agent_shaped_is_spared(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {"other": "somebodys-deploy-branch"})
            self.assertEqual(self.reaped(repo), [])


class ARunningServiceIsNeverReaped(unittest.TestCase):
    def test_a_tree_a_process_runs_from_is_spared_whatever_its_branch(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            repo = a_repo_with(tmp, {"task": "agent/m/a/looks-finished"})
            with mock.patch.object(audit, "process_is_running_from", lambda p: True), \
                 mock.patch.object(audit, "on_github", lambda repo, head: True):
                rows = audit.reap(str(repo), [], idle_minutes=0.0, apply=False)
            self.assertEqual(rows, [], "a live service is not finished work")

    def test_being_unable_to_tell_means_not_removing(self):
        """`ps` or `lsof` failing must not read as 'nothing is running'."""
        def explode(*a, **k):
            raise OSError("no ps here")

        with mock.patch.object(audit.subprocess, "run", explode):
            self.assertTrue(audit.process_is_running_from("/tmp/anything"))

    def test_a_quiet_directory_is_reported_as_not_running(self):
        with TemporaryDirectory() as raw:
            self.assertFalse(audit.process_is_running_from(raw))


if __name__ == "__main__":
    unittest.main()
