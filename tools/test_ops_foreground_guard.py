#!/usr/bin/env python3
"""The foreground guard clears exactly one alert, and runs where it can.

Registering a new LaunchAgent label makes Background Task Management post a
persistent Notification Center alert ("App Background Activity" on macOS 26,
naming Login Items & Extensions) over the Civ VI game being recorded; a click on
it opens System Settings on that pane, in front of everything. Operator,
2026-08-28: "make sure login extensions windows does not appear in foreground
covering our work here". `civvis-foreground-guard.sh` dismisses that alert and
closes that pane, and nothing else — and it has to be started from the
Terminal-descended chain, because a launchd job holds no Automation grant.

Driving Notification Center itself is not something a test can do on a CI
runner; what these tests hold is the contract around it — the script is
selective, single-instance, stands down on its marker, exits when the lane is
gone, its AppleScript compiles, and the wrapper starts it.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import time
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

TOOLS = Path(__file__).resolve().parent
OPS = TOOLS / "ops"
GUARD = OPS / "civvis-foreground-guard.sh"
WRAPPER = OPS / "civvis-verified-head-launcher.sh"
HAS_ZSH = shutil.which("zsh") is not None
HAS_OSACOMPILE = shutil.which("osacompile") is not None
HARDCODED_HOME = re.compile(r"/Users/(?!\$)[A-Za-z][A-Za-z0-9._-]*/")
GITHUB = "https://github.com/MartinHalvorson/CIVVIS.git"


def applescript_of(script: Path) -> str:
    """The heredoc body; its opening line may carry a redirection after the tag."""
    text = script.read_text()
    after_tag = text.split("<<'APPLESCRIPT'", 1)[1]
    body = after_tag.split("\n", 1)[1]
    return body.split("\nAPPLESCRIPT", 1)[0]


def clean_env(**extra: str) -> dict:
    env = {k: v for k, v in os.environ.items() if not k.startswith("CIVVIS_")}
    env.update(extra)
    return env


def zsh(script: Path, *args: str, env=None, timeout: int = 60):
    return subprocess.run(["zsh", str(script), *args], env=env,
                          capture_output=True, text=True, timeout=timeout)


def stub_osascript(directory: Path, result: str) -> Path:
    """An `osascript` that records each call and answers `result`."""
    stub = directory / "osascript"
    stub.write_text("#!/bin/zsh\n"
                    'print -r -- "$*" >> "$STUB_CALLS"\n'
                    "cat > /dev/null\n"
                    f"print -r -- '{result}'\n")
    stub.chmod(0o755)
    return stub


class TheGuardIsSelective(unittest.TestCase):
    def test_it_names_the_alert_and_the_pane_and_nothing_else(self):
        script = applescript_of(GUARD)
        for needle in ("Login Items & Extensions", "App Background Activity",
                       "Background Items Added"):
            self.assertIn(needle, script)
        self.assertIn('description of act is "Close"', script,
                      "an alert is dismissed through its own Close action, not clicked")
        self.assertIn('n contains "Login Items"', script,
                      "only a Settings window on the Login Items pane is closed")
        self.assertIn('settingsMode is "close"', script,
                      "the pane is closed only when the caller says the lane is up")
        # No blanket dismissal: the alert's Close action is performed in one
        # place, inside the text match (`perform action "AXPress"` on the
        # Settings window's close button is the other, separate, operation).
        self.assertEqual(len(re.findall(r"perform act\b", script)), 1)
        self.assertEqual(script.count('perform action "AXPress"'), 1)

    def test_it_derives_its_paths(self):
        executable = [line for line in GUARD.read_text().splitlines()
                      if not line.lstrip().startswith("#")]
        hits = [hit for line in executable for hit in HARDCODED_HOME.findall(line)]
        self.assertEqual(hits, [])
        self.assertIn("OPS=${0:A:h}", GUARD.read_text())

    def test_the_wrapper_starts_it_detached_and_can_be_told_not_to(self):
        text = WRAPPER.read_text()
        self.assertIn("GUARD=${CIVVIS_FOREGROUND_GUARD_SCRIPT:-$OPS/civvis-foreground-guard.sh}", text)
        self.assertIn('( /bin/zsh "$GUARD" >/dev/null 2>&1 & )', text)
        self.assertIn('"${CIVVIS_FOREGROUND_GUARD:-1}" != 0', text)
        self.assertLess(text.index("foreground guard started"),
                        text.index('exec /bin/zsh "$LAUNCHER"'),
                        "the guard starts before the wrapper hands over")

    def test_every_pgrep_is_bracket_escaped(self):
        """civ6_civvis_climb.busy() greps the process table for the game; a
        bare pattern in this script's argv would match itself."""
        for line in GUARD.read_text().splitlines():
            if "pgrep -f" in line and not line.lstrip().startswith("#"):
                self.assertRegex(line, r"\[[a-z]\]", line)


@unittest.skipUnless(HAS_OSACOMPILE, "no osacompile on this runner")
class TheAppleScriptCompiles(unittest.TestCase):
    def test_it_compiles(self):
        """A syntax error here would be invisible: the guard runs osascript for
        its effect and logs whatever comes back."""
        with TemporaryDirectory() as raw:
            done = subprocess.run(
                ["osacompile", "-o", str(Path(raw) / "g.scpt"), "-"],
                input=applescript_of(GUARD), capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr)


@unittest.skipUnless(HAS_ZSH, "the guard is zsh; this runner has no zsh")
class TheGuardRunsWhereItShould(unittest.TestCase):
    def _env(self, raw: str, result: str = "alerts=0 closed=0 settings=0", **extra: str) -> dict:
        home = Path(raw)
        stub = stub_osascript(home, result)
        env = clean_env(HOME=raw, STUB_CALLS=str(home / "calls"),
                        CIVVIS_FOREGROUND_GUARD_OSASCRIPT=str(stub),
                        CIVVIS_FOREGROUND_GUARD_LOG=str(home / "guard.log"),
                        CIVVIS_FOREGROUND_GUARD_LOCK=str(home / "lock"),
                        CIVVIS_FOREGROUND_GUARD_OFF=str(home / "off"),
                        CIVVIS_FOREGROUND_GUARD_INTERVAL="1",
                        CIVVIS_FOREGROUND_GUARD_GRACE="2")
        env.update(extra)
        return env

    def test_once_reports_one_pass_in_close_mode(self):
        with TemporaryDirectory() as raw:
            done = zsh(GUARD, "--once", env=self._env(raw, "alerts=1 closed=1 settings=0"))
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertEqual(done.stdout.strip(), "alerts=1 closed=1 settings=0")
            self.assertEqual((Path(raw) / "calls").read_text().strip(), "- close")
            self.assertIn("once: alerts=1 closed=1", (Path(raw) / "guard.log").read_text())

    def test_it_exits_when_the_lane_has_been_gone_for_the_grace(self):
        with TemporaryDirectory() as raw:
            started = time.monotonic()
            done = zsh(GUARD, env=self._env(raw, CIVVIS_FOREGROUND_GUARD_LANE="0"), timeout=30)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertLess(time.monotonic() - started, 15)
            log = (Path(raw) / "guard.log").read_text()
            self.assertIn("no game lane for", log)
            calls = (Path(raw) / "calls").read_text().split()
            self.assertTrue(calls and set(calls) == {"-", "keep"},
                            f"between games the pane is kept, not closed: {calls}")
            self.assertFalse((Path(raw) / "lock").exists(), "the lock is released on exit")

    def test_it_closes_the_pane_only_while_the_lane_is_up(self):
        with TemporaryDirectory() as raw:
            env = self._env(raw, CIVVIS_FOREGROUND_GUARD_LANE="1")
            proc = subprocess.Popen(["zsh", str(GUARD)], env=env,
                                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                deadline = time.monotonic() + 10
                calls = ""
                while time.monotonic() < deadline and "close" not in calls:
                    time.sleep(0.2)
                    calls = (Path(raw) / "calls").read_text() if (Path(raw) / "calls").exists() else ""
                self.assertIn("- close", calls)
                (Path(raw) / "off").touch()
                proc.wait(timeout=10)
            finally:
                if proc.poll() is None:
                    proc.kill()
            self.assertEqual(proc.returncode, 0)
            self.assertIn("off marker", (Path(raw) / "guard.log").read_text())

    def test_a_second_instance_stands_down_and_a_stale_lock_is_taken_over(self):
        with TemporaryDirectory() as raw:
            lock = Path(raw) / "lock"
            lock.mkdir()
            (lock / "pid").write_text(str(os.getpid()))
            done = zsh(GUARD, env=self._env(raw, CIVVIS_FOREGROUND_GUARD_LANE="0"), timeout=30)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertIn("already running as pid", (Path(raw) / "guard.log").read_text())
            self.assertTrue(lock.is_dir(), "the live holder keeps its lock")
            (lock / "pid").write_text("999999")
            done = zsh(GUARD, env=self._env(raw, CIVVIS_FOREGROUND_GUARD_LANE="0"), timeout=30)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertIn("no game lane for", (Path(raw) / "guard.log").read_text())

    def test_a_pass_that_does_not_answer_is_killed_and_logged(self):
        """28 stray guards once parked an osascript each on System Events and
        every pass on the host hung; a pass is bounded now."""
        with TemporaryDirectory() as raw:
            home = Path(raw)
            # Not `osascript`: _env writes the fast stub under that name.
            slow = home / "slow-osascript"
            slow.write_text("#!/bin/zsh\ncat > /dev/null\nsleep 30\nprint -r -- 'alerts=0 closed=0 settings=0'\n")
            slow.chmod(0o755)
            env = self._env(raw, CIVVIS_FOREGROUND_GUARD_LANE="1",
                            CIVVIS_FOREGROUND_GUARD_PASS_TIMEOUT="1",
                            CIVVIS_FOREGROUND_GUARD_OSASCRIPT=str(slow))
            started = time.monotonic()
            done = zsh(GUARD, "--once", env=env, timeout=30)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertLess(time.monotonic() - started, 10, "the pass must be bounded")
            self.assertEqual(done.stdout.strip(), "timeout")
            self.assertEqual(subprocess.run(["pgrep", "-f", f"[s]leep 30"],
                                            capture_output=True, text=True).stdout, "",
                             "the slow osascript must not be left running")

    def test_a_guard_whose_lock_is_gone_exits(self):
        """A test's temporary HOME, a reaped directory: the guard's lock lives
        there, and a guard without its lock has nothing to guard for."""
        with TemporaryDirectory() as raw:
            env = self._env(raw, CIVVIS_FOREGROUND_GUARD_LANE="1")
            proc = subprocess.Popen(["zsh", str(GUARD)], env=env,
                                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline and not (Path(raw) / "lock" / "pid").exists():
                    time.sleep(0.1)
                self.assertTrue((Path(raw) / "lock" / "pid").exists())
                shutil.rmtree(Path(raw) / "lock")
                proc.wait(timeout=10)
            finally:
                if proc.poll() is None:
                    proc.kill()
            self.assertEqual(proc.returncode, 0)
            self.assertIn("lost the lock", (Path(raw) / "guard.log").read_text())

    def test_the_wrapper_starts_the_guard_it_is_pointed_at(self):
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
            launcher = home / "launcher.sh"
            launcher.write_text("#!/bin/zsh\nprint -r -- launcher-ran\n")
            launcher.chmod(0o755)
            guard = home / "guard.sh"
            guard.write_text('#!/bin/zsh\ntouch "$GUARD_MARK"\n')
            guard.chmod(0o755)
            env = clean_env(HOME=raw, GUARD_MARK=str(home / "guard-ran"),
                            CIVVIS_PINFILE=str(home / "pin"),
                            CIVVIS_VERIFICATION_POLICY=str(home / "policy"),
                            CIVVIS_LADDER_LOG=str(home / "ladder.log"),
                            CIVVIS_LADDER_LAUNCHER=str(launcher),
                            CIVVIS_FOREGROUND_GUARD_SCRIPT=str(guard))
            done = zsh(WRAPPER, env=env)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertIn("launcher-ran", done.stdout)
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline and not (home / "guard-ran").exists():
                time.sleep(0.1)
            self.assertTrue((home / "guard-ran").exists(), "the wrapper must start the guard")
            self.assertIn("foreground guard started", (home / "ladder.log").read_text())
            # And it can be told not to.
            (home / "guard-ran").unlink()
            done = zsh(WRAPPER, env={**env, "CIVVIS_FOREGROUND_GUARD": "0"})
            self.assertEqual(done.returncode, 0, done.stderr)
            time.sleep(0.5)
            self.assertFalse((home / "guard-ran").exists())


if __name__ == "__main__":
    unittest.main()
