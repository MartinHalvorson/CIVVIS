#!/usr/bin/env python3
"""An unattended mirror must not reopen Chrome without operator opt-in."""

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import follow


class BrowserIntentTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.intent = Path(directory.name) / "browser-intent"
        patcher = mock.patch.object(follow, "BROWSER_INTENT", str(self.intent))
        patcher.start()
        self.addCleanup(patcher.stop)
        follow.forget_mirror_presence()
        self.addCleanup(follow.forget_mirror_presence)

    def test_default_background_work_never_invokes_browser_automation(self):
        with mock.patch.object(follow.subprocess, "run") as run, \
             mock.patch.object(follow, "mirror_viewers") as viewers:
            for _ in range(10):
                self.assertEqual(follow.ensure_on_screen(99), 0)
                follow.ensure_watching({"dead_since": 1, "revivals": 3})
                follow.refresh_mirror_page(4242)
            self.assertEqual(follow.chrome('tell application "Google Chrome" to beep'), "")
        run.assert_not_called()
        viewers.assert_not_called()

    def test_explicit_opt_in_restores_a_missing_mirror(self):
        self.intent.write_text("managed\n")
        with mock.patch.object(follow.subprocess, "run") as run, \
             mock.patch.object(follow, "mirror_target_url", return_value="http://127.0.0.1:8610/"), \
             mock.patch.object(follow, "log"):
            run.side_effect = [mock.Mock(returncode=1), mock.Mock(returncode=0, stdout="", stderr="")]
            self.assertEqual(follow.ensure_on_screen(2), 0)
        self.assertEqual(run.call_args_list[0].args[0], ["pgrep", "-x", "Google Chrome"])
        self.assertEqual(run.call_args_list[1].args[0][0], "osascript")
        self.assertIn("make new window", run.call_args_list[1].args[0][2])

    def test_revocation_invalidates_cached_presence_and_blocks_final_dispatch(self):
        self.intent.write_text("managed\n")
        with mock.patch.object(follow.subprocess, "run") as run:
            run.side_effect = [mock.Mock(returncode=0), mock.Mock(returncode=0, stdout=follow.MIRROR_URL)]
            self.assertTrue(follow.mirror_on_screen())
            run.reset_mock()
            self.intent.write_text("manual\n")
            self.assertIsNone(follow.mirror_on_screen(fresh=False))
            self.assertEqual(follow.chrome('tell application "Google Chrome" to beep'), "")
            run.assert_not_called()
        self.assertIsNone(follow._MIRROR_ENUM_CACHE_VALUE)

    def test_invalid_or_removed_opt_in_fails_closed(self):
        for value in (b"", b"true", b"running", b"MANAGED", b"\xff"):
            with self.subTest(value=value):
                self.intent.write_bytes(value)
                self.assertFalse(follow.browser_management_enabled())
        self.intent.unlink()
        self.intent.mkdir()  # Reading a directory is an OSError, as is denied access.
        self.assertFalse(follow.browser_management_enabled())
        self.intent.rmdir()
        self.assertFalse(follow.browser_management_enabled())


if __name__ == "__main__":
    unittest.main()
