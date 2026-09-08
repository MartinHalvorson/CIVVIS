#!/usr/bin/env python3
"""Contracts for the narrow CIVVIS Terminal helper-window guard.

The recovery host must occasionally run a GUI-capable one-shot helper through
Terminal.  A legacy `do script` caller can put that document in the foreground,
covering the game.  The guard may hide only the named helper documents, marks
them so their completed tabs can be reaped, and restores Civ VI only when the
matching document had actually been frontmost.  CI stubs Apple Events; these
tests enforce the selection and lifecycle around them.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import time
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

TOOLS = Path(__file__).resolve().parent
OPS = TOOLS / "ops"
GUARD = OPS / "civvis-terminal-window-guard.sh"
WRAPPER = OPS / "civvis-verified-head-launcher.sh"
HAS_ZSH = shutil.which("zsh") is not None
HAS_OSACOMPILE = shutil.which("osacompile") is not None
GITHUB = "https://github.com/MartinHalvorson/CIVVIS.git"


def applescript_of(script: Path) -> str:
    text = script.read_text()
    after_tag = text.split("<<'APPLESCRIPT'", 1)[1]
    body = after_tag.split("\n", 1)[1]
    return body.split("\nAPPLESCRIPT", 1)[0]


def clean_env(**extra: str) -> dict:
    env = {key: value for key, value in os.environ.items()
           if not key.startswith("CIVVIS_")}
    env.update(extra)
    return env


def zsh(script: Path, *args: str, env=None, timeout: int = 60):
    return subprocess.run(["zsh", str(script), *args], env=env,
                          capture_output=True, text=True, timeout=timeout)


def stub_osascript(directory: Path, result: str) -> Path:
    stub = directory / "osascript"
    stub.write_text("#!/bin/zsh\n"
                    'print -r -- "$*" >> "$STUB_CALLS"\n'
                    "cat > /dev/null\n"
                    f"print -r -- '{result}'\n")
    stub.chmod(0o755)
    return stub


class TheGuardTargetsOnlyManagedHelpers(unittest.TestCase):
    def test_it_names_the_exact_helper_documents_and_marks_them(self):
        script = applescript_of(GUARD)
        for helper in ("civvis-rehost-bootstrap.py", "civvis-capture-free-setup.py",
                       "civvis-attach-cont2-", "civvis-resume-cont2-"):
            self.assertIn(helper, script)
        self.assertIn('set custom title of t to windowMarker', script)
        self.assertIn('set title displays custom title of t to true', script)
        self.assertIn('set miniaturized of w to true', script)
        self.assertIn('if application "Terminal" is not running then', script)
        self.assertIn('set n to name of w', script)
        self.assertIn('(selected of t) is true', script)
        self.assertIn('if wasFrontmost then set restoreCiv6 to true', script)
        self.assertIn('set frontmost of process "Civ6_Exe_Child" to true', script)
        self.assertNotIn('tell application "Terminal" to activate', script)
        self.assertNotIn('tell application "Terminal" to do script', script)

    def test_it_touches_only_a_previously_marked_one_tab_document(self):
        script = applescript_of(GUARD)
        self.assertIn('set marked to ((custom title of t) is windowMarker)', script)
        self.assertIn('if marked then', script)
        self.assertIn('if busy of t then', script)
        self.assertIn('set tabCount to count of tabs of w', script)
        self.assertIn('if tabCount is 1 then', script)
        self.assertIn('close w', script)
        self.assertLess(script.index('if tabCount is 1 then'),
                        script.index('set miniaturized of w to true'),
                        "a user multi-tab window must never be miniaturized")

    def test_it_never_matches_every_civvis_or_python_terminal(self):
        script = applescript_of(GUARD)
        self.assertNotIn('n contains "civvis-"', script)
        self.assertNotIn('n contains "python"', script.lower())

    def test_every_pgrep_is_bracket_escaped(self):
        for line in GUARD.read_text().splitlines():
            if "pgrep -f" in line and not line.lstrip().startswith("#"):
                self.assertRegex(line, r"\[[a-z]\]", line)


@unittest.skipUnless(HAS_OSACOMPILE, "no osacompile on this runner")
class TheAppleScriptCompiles(unittest.TestCase):
    def test_it_compiles(self):
        with TemporaryDirectory() as raw:
            done = subprocess.run(
                ["osacompile", "-o", str(Path(raw) / "guard.scpt"), "-"],
                input=applescript_of(GUARD), capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr)


@unittest.skipUnless(HAS_ZSH, "the guard is zsh; this runner has no zsh")
class TheGuardRunsWhereItShould(unittest.TestCase):
    def _env(self, raw: str, result: str = "hidden=0 reaped=0 focused=0", **extra: str) -> dict:
        home = Path(raw)
        stub = stub_osascript(home, result)
        env = clean_env(HOME=raw, STUB_CALLS=str(home / "calls"),
                        CIVVIS_TERMINAL_WINDOW_GUARD_OSASCRIPT=str(stub),
                        CIVVIS_TERMINAL_WINDOW_GUARD_LOG=str(home / "guard.log"),
                        CIVVIS_TERMINAL_WINDOW_GUARD_LOCK=str(home / "lock"),
                        CIVVIS_TERMINAL_WINDOW_GUARD_OFF=str(home / "off"),
                        CIVVIS_TERMINAL_WINDOW_GUARD_INTERVAL="1",
                        CIVVIS_TERMINAL_WINDOW_GUARD_GRACE="2")
        env.update(extra)
        return env

    def test_once_runs_one_bounded_pass(self):
        with TemporaryDirectory() as raw:
            done = zsh(GUARD, "--once", env=self._env(raw, "hidden=1 reaped=0 focused=1"))
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertEqual(done.stdout.strip(), "hidden=1 reaped=0 focused=1")
            self.assertEqual((Path(raw) / "calls").read_text().strip(), "- CIVVIS managed helper")
            self.assertIn("once: hidden=1 reaped=0 focused=1",
                          (Path(raw) / "guard.log").read_text())

    def test_it_stands_down_after_the_lane_is_gone(self):
        with TemporaryDirectory() as raw:
            started = time.monotonic()
            done = zsh(GUARD, env=self._env(raw, CIVVIS_TERMINAL_WINDOW_GUARD_LANE="0"), timeout=30)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertLess(time.monotonic() - started, 15)
            self.assertIn("no game lane", (Path(raw) / "guard.log").read_text())

    def test_the_wrapper_starts_the_guard_and_allows_an_opt_out(self):
        with TemporaryDirectory() as raw:
            home = Path(raw)
            tree = home / "tree"
            tree.mkdir()
            subprocess.run(["git", "init", "-q", "-b", "scratch", str(tree)],
                           check=True, capture_output=True)
            subprocess.run(["git", "-C", str(tree), "remote", "add", "origin", GITHUB],
                           check=True, capture_output=True)
            (tree / "Cargo.toml").write_text("[package]\n")
            (home / "pin").write_text("head\n")
            (home / "policy").write_text(f"CIVVIS_HEAD_REPO={tree}\n")
            (home / ".civvis-operator-intent").write_text("running\n")
            launcher = home / "launcher.sh"
            launcher.write_text("#!/bin/zsh\nprint -r -- launcher-ran\n")
            launcher.chmod(0o755)
            terminal_guard = home / "terminal-guard.sh"
            terminal_guard.write_text('#!/bin/zsh\ntouch "$TERMINAL_GUARD_MARK"\n')
            terminal_guard.chmod(0o755)
            env = clean_env(HOME=raw, TERMINAL_GUARD_MARK=str(home / "terminal-guard-ran"),
                            CIVVIS_PINFILE=str(home / "pin"),
                            CIVVIS_VERIFICATION_POLICY=str(home / "policy"),
                            CIVVIS_OPERATOR_INTENT_FILE=str(home / ".civvis-operator-intent"),
                            CIVVIS_LADDER_LOG=str(home / "ladder.log"),
                            CIVVIS_LADDER_LAUNCHER=str(launcher),
                            CIVVIS_FOREGROUND_GUARD="0",
                            CIVVIS_TERMINAL_WINDOW_GUARD_SCRIPT=str(terminal_guard))
            done = zsh(WRAPPER, env=env)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertIn("launcher-ran", done.stdout)
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline and not (home / "terminal-guard-ran").exists():
                time.sleep(0.1)
            self.assertTrue((home / "terminal-guard-ran").exists())
            self.assertIn("terminal window guard started", (home / "ladder.log").read_text())
            (home / "terminal-guard-ran").unlink()
            done = zsh(WRAPPER, env={**env, "CIVVIS_TERMINAL_WINDOW_GUARD": "0"})
            self.assertEqual(done.returncode, 0, done.stderr)
            time.sleep(0.5)
            self.assertFalse((home / "terminal-guard-ran").exists())


if __name__ == "__main__":
    unittest.main()
