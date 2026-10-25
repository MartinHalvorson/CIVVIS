"""Native input archiving survives the host launcher and both game runners."""
from __future__ import annotations

import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
import civ6_capture_free_loop as capture_free
import civ6_civvis_climb as climb
import civ6_play as play

KEY = "CIVVIS_CAPTURE_DECIDER_INPUTS"
OPS = TOOLS / "ops"


class InputCapturePolicyTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which("zsh"), "zsh is unavailable")
    def test_launcher_validates_policy_and_clears_an_inherited_capture_arm(self):
        source = (OPS / "civvis-verified-head-launcher.sh").read_text()
        start = source.index("policy=(")
        defaults = source[start:source.index("\n)", start) + 2]
        start = source.index('    case "$key" in')
        case = source[start:source.index('    policy[$key]=$value', start)]
        start = source.index("unset CIVVIS_WITH ")
        exports = source[start:source.index('if [[ -f "$POLICY" ]]; then', start)]
        for value, expected in ((None, "0"), ("0", "0"), ("1", "1"),
                                ("2", None), ("true", None)):
            with self.subTest(value=value), tempfile.TemporaryDirectory() as tmp:
                script = "\n".join([
                    'typeset -A policy', defaults,
                    'refuse() { print -u2 -r -- "$*"; exit 64; }',
                    'say() { :; }', 'POLICY=$1', 'shift',
                    f'{KEY}=1',
                    'for value in "$@"; do', f'key={KEY}', case,
                    'policy[$key]=$value', 'done', exports,
                    f'print -r -- "${{{KEY}:-absent}}"',
                ])
                argv = ["zsh", "-c", script, "test", str(Path(tmp) / "absent")]
                if value is not None:
                    argv.append(value)
                done = subprocess.run(argv, capture_output=True, text=True)
                self.assertEqual(done.returncode, 64 if expected is None else 0,
                                 done.stderr)
                if expected is not None:
                    self.assertEqual(done.stdout.strip(), expected)

    @unittest.skipUnless(shutil.which("zsh"), "zsh is unavailable")
    def test_supervisor_passes_capture_to_both_actual_runner_commands(self):
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        start = source.index("CAPTURE_DECIDER_INPUTS=")
        selector = source[start:source.index("# The live ladder policy", start)]
        commands = re.findall(
            r'python3 -u tools/civ6_(?:capture_free_loop|civvis_climb)\.py .*?2>&1',
            source, re.S,
        )
        self.assertEqual(len(commands), 2)
        for value, expected in ((None, False), ("0", False), ("1", True),
                                ("2", None)):
            for command in commands:
                with self.subTest(value=value, runner=command.splitlines()[0]), \
                        tempfile.TemporaryDirectory() as tmp:
                    env = {k: v for k, v in os.environ.items() if not k.startswith("CIVVIS_")}
                    if value is not None:
                        env[KEY] = value
                    script = "\n".join([
                        'say() { :; }', 'LOGS=$1', 'TAG=test',
                        'python3() { for word in "$@"; do print -r -- "$word"; done; }',
                        selector, command,
                    ])
                    done = subprocess.run(["zsh", "-c", script, "test", tmp],
                                          env=env, capture_output=True, text=True)
                    self.assertEqual(done.returncode, 64 if expected is None else 0,
                                     done.stderr)
                    if expected is not None:
                        words = (Path(tmp) / "climb-test.log").read_text().splitlines()
                        self.assertEqual(words.count("--capture-decider-inputs"), int(expected))

    def test_visual_runner_keeps_capture_on_saved_game_continuations(self):
        args = SimpleNamespace(
            difficulty="DIFFICULTY_EMPEROR", map="Pangaea.lua", map_size="MAPSIZE_TINY",
            speed="GAMESPEED_ONLINE", leader=None, max_turns=250, timeout=7200.0,
            timeout_ceiling=None, probe_citizens=False, campus_specialist=False,
            envoys=False, victory="domination", strategy="auto", war_from_plan=False,
            tile_export_every=25, refresh_seconds=0, no_peace_deterrence=False,
            with_=[], without=[], no_counter_resolutions=False, combat_frames=0,
            replan_frames=2, settler_escort_cap_sync=True, capture_decider_inputs=False,
        )
        for save in (None, Path("AutoSave.Civ6Save")):
            before = climb.play_command(args, "test", Path("orders.sqlite"),
                                        Path("civvis_orders"), save)
            args.capture_decider_inputs = True
            after = climb.play_command(args, "test", Path("orders.sqlite"),
                                       Path("civvis_orders"), save)
            self.assertEqual(after.count("--civvis-capture-inputs"), 1)
            self.assertEqual([w for w in after if w != "--civvis-capture-inputs"], before)
            args.capture_decider_inputs = False

    def test_visual_runner_cli_accepts_the_capture_switch(self):
        done = subprocess.run([sys.executable, str(TOOLS / "civ6_civvis_climb.py"),
                               "--capture-decider-inputs", "--help"],
                              capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("--capture-decider-inputs", done.stdout)

    @unittest.skipUnless(shutil.which("zsh"), "zsh is unavailable")
    def test_host_installer_can_seed_the_same_policy_key(self):
        source = (OPS / "civvis-install-host-automation.sh").read_text()
        start = source.index("POLICY_KEYS=(")
        keys = source[start:source.index(")", start) + 1]
        start = source.index("write_policy() {")
        writer = source[start:source.index('\nsay "policy:"', start)]
        with tempfile.TemporaryDirectory() as tmp:
            policy = Path(tmp) / "policy"
            script = "\n".join([keys, 'POLICY=$1', 'head_repo=',
                                 'say() { :; }', f'{KEY}=1', writer, 'write_policy'])
            done = subprocess.run(["zsh", "-c", script, "test", str(policy)],
                                  capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertIn(f"{KEY}=1\n", policy.read_text())

    def test_capture_free_runner_parser_and_attach_command_forward_the_switch(self):
        off = capture_free.parser().parse_args([])
        on = capture_free.parser().parse_args(["--capture-decider-inputs"])
        before = capture_free.attach_command(off, "test", Path("orders.sqlite"))
        after = capture_free.attach_command(on, "test", Path("orders.sqlite"))
        self.assertEqual(after.count("--civvis-capture-inputs"), 1)
        self.assertEqual([w for w in after if w != "--civvis-capture-inputs"], before)

    def test_play_forwards_the_switch_to_the_existing_brain_capture_option(self):
        args = SimpleNamespace(civvis_victory="domination", civvis_strategy="auto",
                               timeout=7200, civvis_war_from_plan=False,
                               civvis_refresh_seconds=0, civvis_with=[],
                               civvis_without=[], civvis_capture_inputs=False)
        before = play.supervised_brain_command(args, Path("run"), Path("orders.sqlite"), Path("bin"))
        args.civvis_capture_inputs = True
        after = play.supervised_brain_command(args, Path("run"), Path("orders.sqlite"), Path("bin"))
        self.assertEqual(after.count("--capture-inputs"), 1)
        self.assertEqual([w for w in after if w != "--capture-inputs"], before)


if __name__ == "__main__":
    unittest.main()
