#!/usr/bin/env python3
"""The verified-head policy must make capture-free mode explicit and validated."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
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

    @unittest.skipUnless(shutil.which("zsh"), "zsh is not installed")
    def test_the_lobby_policy_validates_and_exports_map_size_and_speed(self):
        """★ THE LOBBY WAS NOT A POLICY AT ALL. A host could pin the rung, the
        leader and the lane, and still had no way to say which world the seat
        plays or how many majors are in it — the supervisor passed none of the
        three and every game silently took the climb's defaults."""
        source = LAUNCHER.read_text(encoding="utf-8")
        case = source[source.index('    case "$key" in'):source.index('    policy[$key]=$value')]
        exports = source[source.index("unset CIVVIS_WITH "):source.index('if [[ -f "$POLICY" ]]; then', source.index("unset CIVVIS_WITH "))]
        for key, value, accepted in (
                ("CIVVIS_MAP", "Pangaea.lua", True),
                ("CIVVIS_MAP", "Continents.lua", True),
                ("CIVVIS_MAP", "Pangaea", False),
                ("CIVVIS_MAP", "../x.lua", False),
                ("CIVVIS_MAP", "Pangaea.lua;exit", False),
                ("CIVVIS_MAP_SIZE", "MAPSIZE_TINY", True),
                ("CIVVIS_MAP_SIZE", "Tiny", False),
                ("CIVVIS_MAP_SIZE", "", False),
                ("CIVVIS_SPEED", "GAMESPEED_STANDARD", True),
                ("CIVVIS_SPEED", "Standard", False),
                ("CIVVIS_SPEED", "", False)):
            script = '\n'.join([
                'typeset -A policy', 'refuse() { print -u2 -- "$*"; exit 64; }',
                f'key={key}', 'value=$1', case,
                'policy[$key]=$value', f'{key}=STALE', exports,
                f'print -r -- "${key}"',
            ])
            done = subprocess.run(["zsh", "-c", script, "test", value],
                                  capture_output=True, text=True)
            with self.subTest(key=key, value=value):
                self.assertEqual(done.returncode, 0 if accepted else 64, done.stderr)
                if accepted:
                    self.assertEqual(done.stdout.strip(), value)

    def test_the_launcher_asks_the_tree_which_lobby_values_are_selectable(self):
        """⚠ A LIST WRITTEN INTO THIS FILE WOULD BE COMPLETE THE DAY IT WAS
        WRITTEN. The shape check above cannot know that `Atlantis.lua` is not a
        map the Create Game panel offers; only `civ6_play.OPTIONS` knows, and a
        value it does not hold has no rendered label to click and no legal
        read-back, so it fails inside `set_dropdown` — after Civilization VI has
        launched. This check runs the launcher's own embedded reader."""
        source = LAUNCHER.read_text(encoding="utf-8")
        start = source.index("\n", source.index("<<'PYTHON'")) + 1
        reader = source[start:source.index("\nPYTHON\n", start)]
        with tempfile.TemporaryDirectory() as scratch:
            script = Path(scratch) / "reader.py"
            script.write_text(reader, encoding="utf-8")
            for values, accepted in (
                    ({}, True),
                    ({"CIVVIS_LOBBY_MAP": "Pangaea.lua"}, True),
                    ({"CIVVIS_LOBBY_MAP": "Atlantis.lua"}, False),
                    ({"CIVVIS_LOBBY_MAP_SIZE": "MAPSIZE_TINY"}, True),
                    ({"CIVVIS_LOBBY_MAP_SIZE": "MAPSIZE_ENORMOUS"}, False),
                    ({"CIVVIS_LOBBY_SPEED": "GAMESPEED_STANDARD"}, True),
                    ({"CIVVIS_LOBBY_SPEED": "GAMESPEED_GLACIAL"}, False)):
                done = subprocess.run(
                    [sys.executable, str(script), str(TOOLS / "civ6_play.py")],
                    capture_output=True, text=True,
                    env={**os.environ, "CIVVIS_LOBBY_MAP": "",
                         "CIVVIS_LOBBY_MAP_SIZE": "", "CIVVIS_LOBBY_SPEED": "",
                         **values})
                with self.subTest(values=values):
                    self.assertEqual(done.returncode, 0 if accepted else 1,
                                     done.stdout + done.stderr)
                    if not accepted:
                        self.assertIn("Create Game panel offers", done.stderr)

    @unittest.skipUnless(shutil.which("zsh"), "zsh is not installed")
    def test_supervisor_forwards_the_lobby_as_words_and_keeps_todays_defaults(self):
        """⚠⚠ ONE EXPANSION PER WORD, and the defaults are the values the climb
        was already using — a host that sets nothing keeps playing the game it
        played before these knobs existed."""
        source = SUPERVISOR.read_text(encoding="utf-8")
        for flag, variable in (("--map", "MAP"), ("--map-size", "MAPSIZE"),
                               ("--speed", "SPEED")):
            self.assertIn(f'{flag} "${variable}"', source)
        settings = [line for line in source.splitlines()
                    if line.startswith(("MAP=", "MAPSIZE=", "SPEED="))]
        self.assertEqual(len(settings), 3, settings)
        script = '\n'.join(settings + ['print -rl -- --map "$MAP" --map-size "$MAPSIZE" --speed "$SPEED"'])
        done = subprocess.run(["zsh", "-c", script], capture_output=True, text=True,
                              env={key: value for key, value in os.environ.items()
                                   if not key.startswith("CIVVIS_")})
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(done.stdout.splitlines(),
                         ["--map", "Continents.lua", "--map-size", "MAPSIZE_SMALL",
                          "--speed", "GAMESPEED_ONLINE"])
        done = subprocess.run(
            ["zsh", "-c", script], capture_output=True, text=True,
            env={**os.environ, "CIVVIS_MAP": "Pangaea.lua",
                 "CIVVIS_MAP_SIZE": "MAPSIZE_TINY",
                 "CIVVIS_SPEED": "GAMESPEED_STANDARD"})
        self.assertEqual(done.stdout.splitlines(),
                         ["--map", "Pangaea.lua", "--map-size", "MAPSIZE_TINY",
                          "--speed", "GAMESPEED_STANDARD"])

    def test_capture_free_says_it_is_ignoring_a_lobby_policy(self):
        """The capture-free profile is fixed on purpose. A ledger row that
        quietly played Continents under a Pangaea policy would be a claim
        nothing checked."""
        source = SUPERVISOR.read_text(encoding="utf-8")
        self.assertIn("capture-free batch ignores lobby policy", source)

    def test_changed_shell_scripts_still_parse(self):
        if shutil.which("zsh") is None:
            self.skipTest("zsh is not installed")
        for script in (LAUNCHER, SUPERVISOR):
            done = subprocess.run(["zsh", "-n", str(script)],
                                  capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr)


if __name__ == "__main__":
    unittest.main()
