#!/usr/bin/env python3
"""The verified-head policy must make capture-free mode explicit and validated."""

from __future__ import annotations

import shutil
import subprocess
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
LAUNCHER = TOOLS / "ops" / "civvis-verified-head-launcher.sh"
SUPERVISOR = TOOLS / "ops" / "civvis-game-supervisor.sh"


class CaptureFreePolicyTests(unittest.TestCase):
    def test_verified_head_accepts_only_a_boolean_capture_free_policy(self):
        source = LAUNCHER.read_text(encoding="utf-8")
        self.assertIn("CIVVIS_CAPTURE_FREE         0", source)
        self.assertIn("CIVVIS_CAPTURE_FREE)", source)
        self.assertIn("must be 0 or 1", source)
        self.assertIn("CIVVIS_CAPTURE_FREE CIVVIS_PLAY_ATTEMPTS", source)

    def test_supervisor_selects_the_capture_free_owner_and_keeps_the_play_log_contract(self):
        source = SUPERVISOR.read_text(encoding="utf-8")
        self.assertIn("CAPTURE_FREE=${CIVVIS_CAPTURE_FREE:-0}", source)
        self.assertIn("tools/civ6_capture_free_loop.py", source)
        self.assertIn("--max-turns 650", source)
        self.assertIn("--logs \"$LOGS\"", source)
        self.assertIn("capture-free batch skips screen gene", source)

    def test_changed_shell_scripts_still_parse(self):
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed")
        for script in (LAUNCHER, SUPERVISOR):
            done = subprocess.run(["zsh", "-n", str(script)],
                                  capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr)


if __name__ == "__main__":
    unittest.main()
