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

    def test_a_failed_chrome_query_is_not_an_absent_mirror(self):
        """2026-11-12: "Application isn't running (-600)" was read as a tab
        list with no mirror in it, and a window was opened over the live one."""
        self.intent.write_text("managed\n")
        failed = mock.Mock(returncode=1, stdout="",
                           stderr="execution error: Google Chrome got an error: "
                                  "Application isn\u2019t running. (-600)")
        with mock.patch.object(follow.subprocess, "run") as run, \
             mock.patch.object(follow, "log"):
            run.side_effect = [mock.Mock(returncode=0), failed]
            self.assertIsNone(follow.mirror_on_screen())
            follow.forget_mirror_presence()
            run.side_effect = [mock.Mock(returncode=0), failed]
            self.assertEqual(follow.ensure_on_screen(2), 0)
        scripts = [call.args[0][2] for call in run.call_args_list if call.args[0][0] == "osascript"]
        self.assertTrue(scripts)
        self.assertFalse(any("make new window" in script for script in scripts))

    def test_restore_looks_for_the_mirror_in_the_script_that_opens_it(self):
        self.intent.write_text("managed\n")
        with mock.patch.object(follow.subprocess, "run") as run, \
             mock.patch.object(follow, "mirror_target_url", return_value="http://127.0.0.1:8610/"), \
             mock.patch.object(follow, "log") as log:
            run.side_effect = [
                mock.Mock(returncode=0),
                mock.Mock(returncode=0, stdout="https://example.com/", stderr=""),
                mock.Mock(returncode=0, stdout="present\n", stderr=""),
            ]
            self.assertEqual(follow.ensure_on_screen(2), 0)
        script = run.call_args_list[2].args[0][2]
        self.assertLess(script.index('return "present"'), script.index("make new window"))
        self.assertIn(f'contains ":{follow.PORT}"', script)
        logged = " ".join(str(call.args[0]) for call in log.call_args_list)
        self.assertIn("https://example.com/", logged)
        self.assertIn("no window opened", logged)

    def test_duplicate_single_tab_mirror_windows_are_closed_once_a_minute(self):
        self.intent.write_text("managed\n")
        follow._MIRROR_DEDUPED_AT = 0.0
        self.addCleanup(setattr, follow, "_MIRROR_DEDUPED_AT", 0.0)
        self.addCleanup(setattr, follow, "_MIRROR_TAB_COUNT", 0)
        listing = f"{follow.MIRROR_URL}?instance=7, {follow.MIRROR_URL}?instance=7&game=1, https://example.com/"
        with mock.patch.object(follow.subprocess, "run") as run, \
             mock.patch.object(follow, "log") as log:
            run.side_effect = [
                mock.Mock(returncode=0),
                mock.Mock(returncode=0, stdout=listing, stderr=""),
                mock.Mock(returncode=0, stdout="1\n", stderr=""),
            ]
            self.assertEqual(follow.ensure_on_screen(1), 0)
            script = run.call_args_list[2].args[0][2]
            self.assertIn("(count of tabs of thisWindow) is 1", script)
            self.assertIn("close (first window whose id is", script)
            self.assertNotIn("make new window", script)
            self.assertIn("closed 1 duplicate", " ".join(str(c.args[0]) for c in log.call_args_list))
            # The same listing a moment later: no second closing pass.
            run.reset_mock()
            run.side_effect = [mock.Mock(returncode=0),
                               mock.Mock(returncode=0, stdout=listing, stderr="")]
            self.assertEqual(follow.ensure_on_screen(0), 0)
            self.assertEqual(len(run.call_args_list), 2)

    def test_one_mirror_tab_closes_nothing(self):
        self.intent.write_text("managed\n")
        follow._MIRROR_DEDUPED_AT = 0.0
        self.addCleanup(setattr, follow, "_MIRROR_TAB_COUNT", 0)
        with mock.patch.object(follow.subprocess, "run") as run:
            run.side_effect = [
                mock.Mock(returncode=0),
                mock.Mock(returncode=0, stdout=f"{follow.MIRROR_URL}?instance=7&game=1", stderr=""),
            ]
            self.assertEqual(follow.ensure_on_screen(2), 0)
        self.assertEqual(len(run.call_args_list), 2)

    def test_replacing_a_wedged_tab_keeps_the_window_where_it_was(self):
        self.intent.write_text("managed\n")
        watch = {"dead_since": 1.0, "last_revival": 0.0, "revivals": 2}
        with mock.patch.object(follow.subprocess, "run") as run, \
             mock.patch.object(follow, "mirror_viewers", return_value=0), \
             mock.patch.object(follow, "mirror_on_screen", return_value=True), \
             mock.patch.object(follow, "mirror_target_url", return_value="http://127.0.0.1:8610/"), \
             mock.patch.object(follow, "log"):
            run.return_value = mock.Mock(returncode=0, stdout="", stderr="")
            follow.ensure_watching(watch)
        script = run.call_args_list[-1].args[0][2]
        self.assertIn("set oldBounds to bounds of thisWindow", script)
        self.assertLess(script.index("make new window"), script.index("set bounds of window 1 to oldBounds"))

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
