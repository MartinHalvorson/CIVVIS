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
        self.assertIn("--max-turns 250", source)
        self.assertIn("--logs \"$LOGS\"", source)
        self.assertIn("capture-free batch skips screen gene", source)

    def test_capture_free_uses_its_fixed_emperor_profile_before_the_ladder(self):
        source = SUPERVISOR.read_text(encoding="utf-8")
        start = source.index("  DIFFICULTY=$EXPLICIT_DIFFICULTY")
        end = source.index("  # ⚠⚠⚠ THE MIRROR SERVER", start)
        selection = source[start:end]
        fixed = selection.index("DIFFICULTY=DIFFICULTY_EMPEROR")
        ladder = selection.index("civ6_ladder_policy.py")
        self.assertLess(
            fixed,
            ladder,
            "capture-free mode must not ask the general ladder for an invalid rung",
        )
        self.assertIn("capture-free profile selects fixed difficulty", selection)
        self.assertIn("explicit CIVVIS_DIFFICULTY", selection)
        self.assertIn("requires difficulty DIFFICULTY_EMPEROR", selection)

    @unittest.skipUnless(shutil.which("zsh"), "zsh is not installed")
    def test_leader_policy_validates_and_exports_the_selected_value(self):
        source = LAUNCHER.read_text(encoding="utf-8")
        case = source[source.index('    case "$key" in'):source.index('    policy[$key]=$value')]
        exports = source[source.index("unset CIVVIS_WITH "):source.index('if [[ -f "$POLICY" ]]; then', source.index("unset CIVVIS_WITH "))]
        for value, accepted in (("LEADER_PERICLES", True), ("LEADER_TRAJAN", True),
                                ("Pericles", False), ("LEADER_PERICLES;exit", False), ("", False)):
            script = '\n'.join([
                'typeset -A policy', 'refuse() { print -u2 -- "$*"; exit 64; }',
                'key=CIVVIS_LEADER', 'value=$1', case,
                'policy[$key]=$value', 'CIVVIS_LEADER=LEADER_STALE', exports,
                'print -r -- "$CIVVIS_LEADER"',
            ])
            done = subprocess.run(["zsh", "-c", script, "test", value], capture_output=True, text=True)
            with self.subTest(value=value):
                self.assertEqual(done.returncode, 0 if accepted else 64, done.stderr)
                if accepted:
                    self.assertEqual(done.stdout.strip(), value)

    @unittest.skipUnless(shutil.which("zsh"), "zsh is not installed")
    def test_supervisor_forwards_configured_leader_as_one_argument(self):
        source = SUPERVISOR.read_text(encoding="utf-8")
        setting = next(line for line in source.splitlines() if line.startswith("LEADER="))
        self.assertEqual(source.count('--leader "$LEADER"'), 2)
        for value in ("", "LEADER_PERICLES"):
            script = 'CIVVIS_LEADER=$1\n' + setting + '\nprint -rl -- --leader "$LEADER"'
            done = subprocess.run(["zsh", "-c", script, "test", value], capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertEqual(done.stdout.splitlines(), ["--leader", value or "LEADER_TRAJAN"])

    def test_changed_shell_scripts_still_parse(self):
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed")
        for script in (LAUNCHER, SUPERVISOR):
            done = subprocess.run(["zsh", "-n", str(script)],
                                  capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr)


if __name__ == "__main__":
    unittest.main()
