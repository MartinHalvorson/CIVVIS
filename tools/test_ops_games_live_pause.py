"""An explicit lane start resumes a saved pause; recovery never does."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


REPO = Path(__file__).resolve().parent.parent
SWITCH = REPO / "tools/ops/civvis-games.sh"


@unittest.skipUnless(shutil.which("zsh"), "requires zsh")
class LivePauseSwitchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="live pause switch ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.home = self.root / "home"
        self.home.mkdir()
        self.request = self.root / "pause request.json"
        self.report = self.root / "pause report.json"
        self.intent = self.home / ".civvis-operator-intent"
        self.intent.write_text("running\n")
        self.halt = self.home / ".civvis-operator-halt.json"
        self.opened = self.root / "opened"
        self.fake_bin = self.root / "bin"
        self.fake_bin.mkdir()
        for name, source in {
            "launchctl": "#!/bin/zsh\nexit 0\n",
            "pgrep": ('#!/bin/zsh\nif [[ "$TEST_CHAIN_ACTIVE" == 1 ]]; then '
                      'print 42424; exit 0; fi\nexit 1\n'),
            "open": ('#!/bin/zsh\nif [[ -e "$CIVVIS_LIVE_PAUSE_FILE" ]]; then '
                     'print paused > "$TEST_OPENED"; else '
                     'print playing > "$TEST_OPENED"; fi\n'),
        }.items():
            command = self.fake_bin / name
            command.write_text(source)
            command.chmod(0o755)
        self.env = {key: value for key, value in os.environ.items()
                    if not key.startswith("CIVVIS_")}
        self.env.update(
            HOME=str(self.home),
            PATH=str(self.fake_bin) + os.pathsep + os.environ["PATH"],
            CIVVIS_REPO=str(REPO),
            CIVVIS_LIVE_PAUSE_FILE=str(self.request),
            CIVVIS_LIVE_PAUSE_STATE_FILE=str(self.report),
            CIVVIS_RUNS_ROOT=str(self.root / "runs"),
            TEST_CHAIN_ACTIVE="1",
            TEST_OPENED=str(self.opened),
        )

    def paused(self):
        self.request.write_text(json.dumps({"paused": True}))
        self.report.write_text(json.dumps({"held": True, "turn": 29,
                                           "tag": "paused-game"}))

    def switch(self, verb, **extra):
        return subprocess.run(
            [shutil.which("zsh"), str(SWITCH), verb],
            env={**self.env, **extra}, capture_output=True,
            text=True, timeout=20,
        )

    def test_on_resumes_an_existing_harness_without_restarting_it(self):
        self.paused()
        report = self.report.read_bytes()
        result = self.switch("on")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertFalse(self.request.exists())
        self.assertEqual(self.report.read_bytes(), report,
                         "only the game can acknowledge that it resumed")
        self.assertFalse(self.opened.exists())
        self.assertIn("live pause request cleared", result.stdout)

    def test_on_clears_the_pause_before_launching_a_missing_chain(self):
        self.paused()
        result = self.switch("on", TEST_CHAIN_ACTIVE="0")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.opened.read_text(), "playing\n")
        self.assertEqual(self.intent.read_text(), "running\n")
        self.assertEqual((self.home / ".civvis-play-pin").read_text(), "head\n")

    def test_on_is_safe_when_no_pause_exists(self):
        result = self.switch("on", TEST_CHAIN_ACTIVE="0")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.opened.read_text(), "playing\n")

    def test_ensure_preserves_a_requested_pause(self):
        self.paused()
        request = self.request.read_bytes()
        result = self.switch("ensure")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.request.read_bytes(), request)
        self.assertFalse(self.opened.exists())

    def test_status_reports_the_request_and_actual_game_acknowledgement(self):
        self.paused()
        result = self.switch("status")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('"requested": true', result.stdout)
        self.assertIn('"held": true', result.stdout)
        self.assertIn('"turn": 29', result.stdout)
        self.assertTrue(self.request.exists())

    def test_a_failed_resume_does_not_clear_a_halt_or_authorize_recovery(self):
        self.request.mkdir()
        self.halt.write_text(json.dumps({"reason": "test halt"}))
        halt = self.halt.read_bytes()
        self.intent.write_text("stopped\n")
        result = self.switch("on", TEST_CHAIN_ACTIVE="0")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.intent.read_text(), "stopped\n")
        self.assertEqual(self.halt.read_bytes(), halt)
        self.assertFalse(self.opened.exists())
        self.assertIn("could not clear the live pause", result.stdout)


if __name__ == "__main__":
    unittest.main()
