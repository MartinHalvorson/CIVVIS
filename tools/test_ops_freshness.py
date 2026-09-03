#!/usr/bin/env python3
"""Merged ops fixes must reach the tree the live ops layer runs from.

...and must not reach it by rewriting a script out from under a running zsh.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

OPS = Path(__file__).resolve().parent / "ops"
FRESHNESS = OPS / "civvis-ops-freshness.sh"
RELOAD_MARKER = 'exec /bin/zsh "$SELF_PATH"'


class TheOpsTreeIsRefreshedOnlyWhereItIsSafe(unittest.TestCase):
    """★★★★★ ELIGIBILITY IS "CAN IT HAND OVER", NOT "IS IT STALE".

    Measured on this host 2026-09-03: replacing a running zsh script's bytes
    lets the buffered block finish, and then the process resumes reading the
    NEW file at its old byte offset and executes whatever is there --

        finished cleanly                        <- old loop, intact
        zshread.sh:9: command not found: than
        REPLACEMENT should never run            <- new file's tail, executed

    So a script may only be rewritten when the copy already running knows to
    `exec` itself on the change.
    """

    def setUp(self) -> None:
        if shutil.which("zsh") is None or shutil.which("git") is None:
            self.skipTest("zsh and git are needed here")
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name) / "tree"
        (self.repo / "tools" / "ops").mkdir(parents=True)
        self._git("init", "-q", "-b", "main")
        self._git("config", "user.email", "t@example.com")
        self._git("config", "user.name", "t")

    def _git(self, *args: str) -> str:
        return subprocess.run(["git", "-C", str(self.repo), *args],
                              capture_output=True, text=True,
                              check=True).stdout

    def _write(self, name: str, body: str) -> Path:
        path = self.repo / "tools" / "ops" / name
        path.write_text(body)
        return path

    def _commit(self, message: str) -> None:
        self._git("add", "-A")
        self._git("commit", "-q", "-m", message)

    def _run(self) -> str:
        log = Path(self.tmp.name) / "freshness.log"
        done = subprocess.run(
            ["zsh", str(FRESHNESS)],
            capture_output=True, text=True, timeout=60,
            env={"PATH": "/usr/bin:/bin:/usr/local/bin", "HOME": self.tmp.name,
                 "CIVVIS_OPS_TREE": str(self.repo),
                 "CIVVIS_OPS_FRESHNESS_REF": "target",
                 "CIVVIS_OPS_FRESHNESS_LOG": str(log)})
        self.assertEqual(done.returncode, 0, done.stderr)
        return log.read_text() if log.exists() else ""

    def _stage_an_update(self, running: str, merged: str) -> None:
        """`target` carries the merged version; the tree still runs `running`."""
        self._write("civvis-thing.sh", running)
        self._commit("running")
        self._git("checkout", "-q", "-b", "target")
        self._write("civvis-thing.sh", merged)
        self._commit("merged")
        self._git("checkout", "-q", "main")

    def test_a_self_reloading_script_is_refreshed(self) -> None:
        running = f"#!/bin/zsh\n# old\nSELF_PATH=${{0:A}}\n{RELOAD_MARKER}\n"
        merged = f"#!/bin/zsh\n# NEW AND MERGED\nSELF_PATH=${{0:A}}\n{RELOAD_MARKER}\n"
        self._stage_an_update(running, merged)
        log = self._run()
        self.assertIn("refreshed tools/ops/civvis-thing.sh", log)
        self.assertIn("NEW AND MERGED",
                      (self.repo / "tools/ops/civvis-thing.sh").read_text())

    def test_a_script_without_self_reload_is_reported_not_written(self) -> None:
        running = "#!/bin/zsh\n# old and running\nwhile true; do sleep 1; done\n"
        merged = "#!/bin/zsh\n# NEW AND MERGED\nwhile true; do sleep 1; done\n"
        self._stage_an_update(running, merged)
        log = self._run()
        self.assertIn("PENDING tools/ops/civvis-thing.sh", log)
        body = (self.repo / "tools/ops/civvis-thing.sh").read_text()
        self.assertIn("old and running", body)
        self.assertNotIn("NEW AND MERGED", body,
                         "rewriting a script with no self-reload is the hazard")

    def test_the_marker_is_read_from_the_running_copy(self) -> None:
        """A marker only the NEW version has cannot reload anything: the code
        that would do the reloading is the code already in memory."""
        running = "#!/bin/zsh\n# old, cannot hand over\nwhile true; do sleep 1; done\n"
        merged = f"#!/bin/zsh\n# now it can\nSELF_PATH=${{0:A}}\n{RELOAD_MARKER}\n"
        self._stage_an_update(running, merged)
        log = self._run()
        self.assertIn("PENDING", log)
        self.assertNotIn("now it can",
                         (self.repo / "tools/ops/civvis-thing.sh").read_text())

    def test_nothing_outside_ops_is_touched(self) -> None:
        (self.repo / "src").mkdir()
        (self.repo / "src" / "game.rs").write_text("// running\n")
        self._write("civvis-thing.sh", "#!/bin/zsh\n# same\n")
        self._commit("running")
        self._git("checkout", "-q", "-b", "target")
        (self.repo / "src" / "game.rs").write_text("// MERGED\n")
        self._commit("merged elsewhere")
        self._git("checkout", "-q", "main")
        self._run()
        self.assertEqual((self.repo / "src" / "game.rs").read_text(), "// running\n")

    def test_a_missing_tree_is_not_an_error(self) -> None:
        """It runs on a schedule; a host without this tree must not alarm."""
        log = Path(self.tmp.name) / "f.log"
        done = subprocess.run(
            ["zsh", str(FRESHNESS)], capture_output=True, text=True, timeout=60,
            env={"PATH": "/usr/bin:/bin", "HOME": self.tmp.name,
                 "CIVVIS_OPS_TREE": str(Path(self.tmp.name) / "absent"),
                 "CIVVIS_OPS_FRESHNESS_LOG": str(log)})
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("nothing to do", log.read_text())

    def test_the_script_is_valid_zsh(self) -> None:
        done = subprocess.run(["zsh", "-n", str(FRESHNESS)],
                              capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stderr)


if __name__ == "__main__":
    unittest.main()
