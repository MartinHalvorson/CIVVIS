#!/usr/bin/env python3
"""A person at the keyboard is not a harness event, and vice versa."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "civ6_control"))

from civ6_control import macos_input, operator_presence  # noqa: E402


class OperatorActiveTests(unittest.TestCase):
    def test_a_recent_human_event_means_active(self):
        self.assertTrue(operator_presence.operator_active(
            15.0, idle=3.0, synthetic_at=None, now=1000.0))

    def test_an_old_idle_clock_means_unattended(self):
        self.assertFalse(operator_presence.operator_active(
            15.0, idle=15.0, synthetic_at=None, now=1000.0))
        self.assertFalse(operator_presence.operator_active(
            15.0, idle=900.0, synthetic_at=None, now=1000.0))

    def test_the_harness_own_click_is_not_a_person(self):
        """⚠⚠ A synthetic click resets macOS's idle clock like any other event.
        Without attribution the gate would defer to its own footsteps forever:
        click, see 'someone just moved the mouse', wait, click, …"""
        now = 1000.0
        # Idle clock says the last event was at 999.5; we clicked at 999.4.
        self.assertFalse(operator_presence.operator_active(
            15.0, idle=0.5, synthetic_at=999.4, now=now))
        # Same clock, but our last click was long before -- that is a person.
        self.assertTrue(operator_presence.operator_active(
            15.0, idle=0.5, synthetic_at=990.0, now=now))

    def test_attribution_has_a_small_window_not_a_large_one(self):
        """An event a few seconds after our click is the person, not the click."""
        now = 1000.0
        event_after_click = operator_presence.SYNTHETIC_ATTRIBUTION_SECONDS + 1.0
        self.assertTrue(operator_presence.operator_active(
            15.0, idle=2.0, synthetic_at=now - 2.0 - event_after_click, now=now))

    def test_an_unreadable_idle_clock_falls_back_to_unattended(self):
        """The harness ran unattended for months without this gate; 'cannot
        tell' must be that behaviour, never a frozen game."""
        with mock.patch.object(operator_presence, "hid_idle_seconds", return_value=None):
            self.assertFalse(operator_presence.operator_active(15.0))

    def test_the_real_readers_are_used_when_nothing_is_injected(self):
        with mock.patch.object(operator_presence, "hid_idle_seconds", return_value=1.0), \
             mock.patch.object(operator_presence, "last_synthetic_input", return_value=None):
            self.assertTrue(operator_presence.operator_active(15.0, now=1000.0))


class SyntheticInputRecordTests(unittest.TestCase):
    def test_note_and_read_round_trip(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "last"
            self.assertIsNone(operator_presence.last_synthetic_input(path))
            operator_presence.note_synthetic_input(path, now=1234.5)
            self.assertEqual(operator_presence.last_synthetic_input(path), 1234.5)

    def test_an_unwritable_record_never_breaks_input(self):
        operator_presence.note_synthetic_input(Path("/nonexistent-dir/x"), now=1.0)

    def test_every_input_backend_records_its_event(self):
        """Both dispatchers, because the popup clearer and the game controller
        are separate processes and either may be the one that clicks."""
        for backend, args in (("_run_cliclick", (["m:1,1"],)),
                              ("_run_native", (["move", "1", "1"],))):
            with self.subTest(backend=backend), \
                 mock.patch.object(operator_presence, "note_synthetic_input") as note, \
                 mock.patch.object(macos_input.subprocess, "run",
                                   return_value=mock.Mock(returncode=0, stdout="", stderr="")), \
                 mock.patch.object(macos_input, "_cliclick", return_value="/usr/bin/true"), \
                 mock.patch.object(macos_input, "_native_binary", return_value=Path("/usr/bin/true")):
                getattr(macos_input, backend)(*args, check=False)
                note.assert_called_once()

    def test_ioreg_parsing(self):
        fake = mock.Mock(returncode=0, stdout='  |   "HIDIdleTime" = 2500000000\n')
        with mock.patch.object(operator_presence.subprocess, "run", return_value=fake):
            self.assertEqual(operator_presence.hid_idle_seconds(), 2.5)
        with mock.patch.object(operator_presence.subprocess, "run",
                               side_effect=OSError("no ioreg")):
            self.assertIsNone(operator_presence.hid_idle_seconds())


if __name__ == "__main__":
    unittest.main()
