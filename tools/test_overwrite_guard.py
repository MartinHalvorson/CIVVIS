#!/usr/bin/env python3
"""Prove overwrite_guard's verdicts on a purpose-built history.

The fixture repo re-creates the failure this guard exists for, in miniature:
an old file, a young feature landed on main, and a branch that deletes the
young lines. Committer dates are pinned so the test controls "young" exactly
and never depends on the wall clock.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import overwrite_guard

NOW = 1_800_000_000
OLD = NOW - 30 * 86400
YOUNG = NOW - 2 * 86400


def run(repo: pathlib.Path, *args: str, date: int | None = None) -> str:
    env = dict(os.environ)
    if date is not None:
        stamp = f"{date} +0000"
        env["GIT_AUTHOR_DATE"] = stamp
        env["GIT_COMMITTER_DATE"] = stamp
    result = subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, text=True,
        env=env, check=True,
    )
    return result.stdout.strip()


class OverwriteGuardTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory(prefix="civvis-guard-")
        self.repo = pathlib.Path(self.dir.name)
        run(self.repo, "init", "-q", "-b", "main")
        run(self.repo, "config", "user.email", "guard@test")
        run(self.repo, "config", "user.name", "guard")
        (self.repo / "engine.txt").write_text(
            "\n".join(f"engine line {i}" for i in range(40)) + "\n")
        run(self.repo, "add", "engine.txt")
        run(self.repo, "commit", "-q", "-m", "Ancient foundation", date=OLD)
        feature = "\n".join(f"lens panel line {i}" for i in range(30)) + "\n"
        (self.repo / "panel.txt").write_text(feature)
        run(self.repo, "add", "panel.txt")
        run(self.repo, "commit", "-q", "-m", "Add the lens panel (#1109)",
            date=YOUNG)
        self.base = run(self.repo, "rev-parse", "HEAD")
        self.addCleanup(self.dir.cleanup)

    def branch_deleting(self, path: str, keep: int, message: str) -> str:
        run(self.repo, "checkout", "-q", "-b", f"topic-{keep}-{path}", self.base)
        lines = (self.repo / path).read_text().splitlines(keepends=True)
        (self.repo / path).write_text("".join(lines[:keep]))
        run(self.repo, "add", path)
        run(self.repo, "commit", "-q", "-m", message, date=NOW)
        return run(self.repo, "rev-parse", "HEAD")

    def verdict(self, head: str, body: str = "") -> int:
        body_file = self.repo / "body.txt"
        body_file.write_text(body)
        cwd = os.getcwd()
        os.chdir(self.repo)
        try:
            return overwrite_guard.main([
                "--base", self.base, "--head", head,
                "--body-file", str(body_file), "--now", str(NOW),
            ])
        finally:
            os.chdir(cwd)

    def test_deleting_young_work_unacknowledged_fails(self):
        head = self.branch_deleting("panel.txt", 0, "Redesign the panel")
        self.assertEqual(self.verdict(head), 1)

    def test_naming_the_victim_pr_passes(self):
        head = self.branch_deleting("panel.txt", 0, "Redesign the panel")
        self.assertEqual(self.verdict(head, "Supersedes: #1109"), 0)

    def test_the_waiver_passes(self):
        head = self.branch_deleting("panel.txt", 0, "Bulk rewrite")
        self.assertEqual(self.verdict(head, "overwrite-guard: allow"), 0)

    def test_deleting_old_lines_is_ordinary_maintenance(self):
        head = self.branch_deleting("engine.txt", 0, "Retire the foundation")
        self.assertEqual(self.verdict(head), 0)

    def test_small_adjacent_churn_stays_quiet(self):
        head = self.branch_deleting("panel.txt", 24, "Tweak six panel lines")
        self.assertEqual(self.verdict(head), 0)

    def test_a_rename_is_not_a_deletion(self):
        run(self.repo, "checkout", "-q", "-b", "rename", self.base)
        run(self.repo, "mv", "panel.txt", "panel_moved.txt")
        run(self.repo, "commit", "-q", "-m", "Move the panel", date=NOW)
        head = run(self.repo, "rev-parse", "HEAD")
        self.assertEqual(self.verdict(head), 0)


if __name__ == "__main__":
    unittest.main()
