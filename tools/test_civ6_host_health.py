#!/usr/bin/env python3
"""The host fault marker: hold the lane until a restart, a configured game, or an hour."""

from __future__ import annotations

import io
import json
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import host_health  # noqa: E402

TOOLS = Path(__file__).resolve().parent
# What System Events said on 2026-11-04 and on 2026-10-16.
GONE = 'execution error: System Events got an error: Can’t get process "Civ6_Exe_Child". (-1728)'
SYNTAX = '49:50: syntax error: Expected end of line but found “"”. (-2741)'


class MarkerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = TemporaryDirectory()
        self.path = Path(self.dir.name) / "host-needs-restart"
        boot = mock.patch.object(host_health, "boot_time", return_value=1_000)
        boot.start()
        self.addCleanup(boot.stop)
        self.addCleanup(self.dir.cleanup)

    def test_a_recorded_fault_holds_the_lane_and_says_how_to_fix_it(self) -> None:
        host_health.record(SYNTAX, tag="civvis-x", now=5_000, path=self.path)
        reason = host_health.hold(now=5_060, path=self.path)
        self.assertIsNotNone(reason)
        self.assertIn("-2741", reason)
        self.assertIn("restart this Mac", reason)
        self.assertIn("civvis-x", reason)

    def test_no_marker_is_no_hold(self) -> None:
        self.assertIsNone(host_health.hold(now=5_000, path=self.path))

    def test_a_restart_clears_the_marker(self) -> None:
        host_health.record(GONE, now=5_000, path=self.path)
        with mock.patch.object(host_health, "boot_time", return_value=9_999):
            self.assertIsNone(host_health.hold(now=5_060, path=self.path))
        self.assertFalse(self.path.exists())

    def test_an_unknown_boot_keeps_the_hold(self) -> None:
        host_health.record(GONE, now=5_000, path=self.path)
        with mock.patch.object(host_health, "boot_time", return_value=None):
            self.assertIsNotNone(host_health.hold(now=5_060, path=self.path))

    def test_one_retry_an_hour_is_let_through(self) -> None:
        host_health.record(GONE, now=5_000, path=self.path)
        self.assertIsNotNone(host_health.hold(
            now=5_000 + host_health.RETRY_AFTER_S - 1, path=self.path))
        self.assertIsNone(host_health.hold(
            now=5_000 + host_health.RETRY_AFTER_S, path=self.path))
        # The marker stays: a retry that fails again rewrites it, and only a
        # configured game or a restart removes it.
        self.assertTrue(self.path.exists())

    def test_clear_reports_whether_there_was_a_fault(self) -> None:
        self.assertFalse(host_health.clear(self.path))
        host_health.record(GONE, now=5_000, path=self.path)
        self.assertTrue(host_health.clear(self.path))
        self.assertIsNone(host_health.read(self.path))

    def test_the_marker_is_json_with_its_boot(self) -> None:
        host_health.record(GONE, tag="t", now=5_000, path=self.path)
        data = json.loads(self.path.read_text())
        self.assertEqual(data["boot"], 1_000)
        self.assertEqual(data["retry_after"], 5_000 + host_health.RETRY_AFTER_S)
        self.assertEqual(data["detected_utc"], "1970-01-01T01:23:20Z")

    def test_a_corrupt_marker_does_not_hold(self) -> None:
        self.path.write_text("{not json")
        self.assertIsNone(host_health.hold(now=5_000, path=self.path))


class RemedyTest(unittest.TestCase):
    def test_launch_services_faults_need_a_restart(self) -> None:
        for error in (GONE, SYNTAX, "Application isn’t running. (-600)"):
            self.assertIn("restart this Mac", host_health.remedy(error))

    def test_a_missing_privacy_grant_is_not_told_to_restart(self) -> None:
        error = "osascript is not allowed assistive access. (-25211)"
        self.assertIn("Accessibility", host_health.remedy(error))
        self.assertNotIn("restart", host_health.remedy(error))


class BootTimeTest(unittest.TestCase):
    def test_reads_the_seconds_out_of_sysctl(self) -> None:
        out = "{ sec = 1793757239, usec = 364781 } Tue Oct 27 11:22:58 2026\n"
        with mock.patch.object(host_health.subprocess, "run",
                               return_value=mock.Mock(stdout=out)):
            self.assertEqual(host_health.boot_time(), 1793757239)

    def test_an_unreadable_host_is_unknown(self) -> None:
        with mock.patch.object(host_health.subprocess, "run", side_effect=OSError):
            self.assertIsNone(host_health.boot_time())


class CommandLineTest(unittest.TestCase):
    """The supervisor reads `--hold` by exit status; status reads `--show`."""

    def run_main(self, path: Path, *argv: str) -> tuple[int, str]:
        out = io.StringIO()
        with mock.patch.object(host_health, "MARKER", path), \
                mock.patch.object(host_health, "boot_time", return_value=1_000), \
                redirect_stdout(out):
            return host_health.main(list(argv)), out.getvalue()

    def test_hold_exits_zero_and_prints_only_while_holding(self) -> None:
        with TemporaryDirectory() as tmp:
            path = Path(tmp) / "m"
            self.assertEqual(self.run_main(path, "--hold"), (1, ""))
            with mock.patch.object(host_health, "boot_time", return_value=1_000):
                host_health.record(GONE, path=path)
            code, text = self.run_main(path, "--hold")
            self.assertEqual(code, 0)
            self.assertIn("-1728", text)
            self.assertEqual(self.run_main(path, "--show")[0], 0)
            self.assertEqual(self.run_main(path, "--clear"), (0, "cleared\n"))
            self.assertEqual(self.run_main(path, "--show")[0], 1)


class SupervisorHoldTest(unittest.TestCase):
    """The supervisor asks before every launch, and before quitting a failed one."""

    def test_the_hold_comes_before_the_failed_start_recycle(self) -> None:
        script = (TOOLS / "ops" / "civvis-game-supervisor.sh").read_text()
        hold = script.index('host_health.py" --hold')
        self.assertLess(script.index("verification_intent_running; then"), hold)
        self.assertLess(hold, script.index("if (( consecutive_failures > 0 )); then"))
        block = script[hold:script.index("# <<< host fault hold")]
        self.assertIn("HOST NEEDS AN OPERATOR", block)
        self.assertIn("continue", block)

    def test_status_names_the_fault(self) -> None:
        script = (TOOLS / "ops" / "civvis-games.sh").read_text()
        self.assertIn('host_health.py" --show', script)


if __name__ == "__main__":
    unittest.main()
