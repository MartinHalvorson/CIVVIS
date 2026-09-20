#!/usr/bin/env python3
"""Exercise failed startup ownership with real disposable child processes."""
import signal
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from civ6_control import launcher


class StartupCleanupTest(unittest.TestCase):
    def child(self, ignore_term=False):
        script = "import signal,time; "
        if ignore_term:
            script += "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
        script += "print('ready', flush=True); time.sleep(120)"
        child = subprocess.Popen([sys.executable, "-c", script],
                                 stdout=subprocess.PIPE, text=True)
        self.assertEqual(child.stdout.readline().strip(), "ready")
        def cleanup():
            if child.poll() is None:
                child.kill()
            child.wait(timeout=5)
            child.stdout.close()
        self.addCleanup(cleanup)
        return child

    def restart(self, child, ready=False, error=None):
        with patch.object(launcher, "stop", return_value=True), \
             patch.object(launcher, "clear_run_logs"), \
             patch.object(launcher, "launch", return_value=child), \
             patch.object(launcher, "wait_for_main_menu", return_value=ready,
                          side_effect=error):
            return launcher.restart(timeout_s=0.01)

    def test_timeout_reaps_owned_child_and_leaves_unrelated_child_alive(self):
        owned, unrelated = self.child(), self.child()
        self.assertFalse(self.restart(owned))
        self.assertIsNotNone(owned.poll())
        self.assertIsNone(unrelated.poll())

    def test_ready_menu_preserves_child(self):
        owned = self.child()
        self.assertTrue(self.restart(owned, ready=True))
        self.assertIsNone(owned.poll())

    @unittest.skipIf(sys.platform == "win32", "Windows terminate is already forcible")
    def test_term_resistant_child_is_reaped_without_touching_other_processes(self):
        owned, unrelated = self.child(ignore_term=True), self.child()
        with patch.object(launcher, "wait_for_main_menu", return_value=False):
            self.assertFalse(launcher.wait_for_launched_main_menu(
                owned, timeout_s=0.01, stop_timeout_s=0.1))
        self.assertEqual(owned.returncode, -signal.SIGKILL)
        self.assertIsNone(unrelated.poll())

    def test_already_exited_child_is_not_signalled(self):
        owned = self.child()
        owned.terminate()
        owned.wait(timeout=5)
        with patch.object(owned, "terminate") as terminate, \
             patch.object(owned, "kill") as kill:
            self.assertFalse(self.restart(owned))
        terminate.assert_not_called()
        kill.assert_not_called()

    def test_interrupted_startup_reaps_child_and_preserves_exception(self):
        owned = self.child()
        with self.assertRaises(KeyboardInterrupt):
            self.restart(owned, error=KeyboardInterrupt())
        self.assertIsNotNone(owned.poll())


if __name__ == "__main__":
    unittest.main()
