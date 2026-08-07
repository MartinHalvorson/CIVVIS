#!/usr/bin/env python3
"""Regression tests for the systematic host-control module."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import computer_control as cc  # noqa: E402


class QuadrantFrameTest(unittest.TestCase):
    """Geometry on this host's real 1728x1117 desktop, so failures are legible."""

    def test_upper_right_is_where_the_game_goes(self) -> None:
        x, y, w, h = cc.quadrant_frame("upper-right", 1728, 1117)
        self.assertEqual((x, y), (864, 33))
        self.assertEqual((w, h), (864, 525))

    def test_lower_left_reaches_the_bottom_edge(self) -> None:
        x, y, w, h = cc.quadrant_frame("lower-left", 1728, 1117)
        self.assertEqual((x, y), (0, 558))
        self.assertEqual(y + h, 1117, "the lower row must reach the screen bottom")

    def test_rows_meet_exactly_once(self) -> None:
        _, upper_y, _, upper_h = cc.quadrant_frame("upper-left", 1728, 1117)
        _, lower_y, _, _ = cc.quadrant_frame("lower-left", 1728, 1117)
        self.assertEqual(upper_y + upper_h, lower_y,
                         "upper and lower quadrants must neither gap nor overlap")

    def test_menu_bar_is_carved_from_the_top_half_only(self) -> None:
        _, y, _, _ = cc.quadrant_frame("upper-left", 1728, 1117)
        self.assertEqual(y, cc.MENU_BAR_PT)

    def test_a_typo_is_an_error_not_a_quadrant(self) -> None:
        with self.assertRaises(ValueError):
            cc.quadrant_frame("center", 1728, 1117)

    def test_lower_right_exists_for_callers_but_not_in_the_standard_layout(self) -> None:
        """The operator keeps that quadrant; nothing of ours may claim it."""
        cc.quadrant_frame("lower-right", 1728, 1117)
        self.assertNotIn("lower-right",
                         [spec["quadrant"] for spec in cc.STANDARD_LAYOUT])


class DismissalPolicyTest(unittest.TestCase):
    """The Gatekeeper sheet's DEFAULT button destroys the install. Policy, not vibes."""

    def test_gatekeeper_damaged_sheet_gets_cancel_never_move_to_trash(self) -> None:
        modal = {"owner": "CoreServicesUIAgent",
                 "text": "“Civilization VI” is damaged and can’t be opened.",
                 "buttons": ["Move to Trash", "Cancel"]}
        self.assertEqual(cc.choose_dismissal(modal), "Cancel")

    def test_a_sheet_offering_only_destruction_is_left_alone(self) -> None:
        modal = {"owner": "CoreServicesUIAgent",
                 "text": "“Civilization VI” is damaged and can’t be opened.",
                 "buttons": ["Move to Trash"]}
        self.assertIsNone(cc.choose_dismissal(modal))

    def test_problem_reporter_never_gets_reopen(self) -> None:
        modal = {"owner": "Problem Reporter",
                 "text": "Problem Report for Civilization VI",
                 "buttons": ["Reopen", "OK"]}
        self.assertEqual(cc.choose_dismissal(modal), "OK")

    def test_an_unknown_owner_is_reported_not_clicked(self) -> None:
        modal = {"owner": "SomeNewAgent", "text": "anything", "buttons": ["OK"]}
        self.assertIsNone(cc.choose_dismissal(modal))

    def test_the_admin_auth_sheet_gets_cancel_and_never_credentials(self) -> None:
        """The sheet a refused symlink move left on screen for an hour."""
        modal = {"owner": "SecurityAgent",
                 "text": "Finder wants to make changes.",
                 "buttons": ["Use Password…", "Cancel"]}
        self.assertEqual(cc.choose_dismissal(modal), "Cancel")


class EnsureSingleGameTest(unittest.TestCase):
    def test_the_oldest_child_is_kept_and_newer_ones_are_culled(self) -> None:
        """This session's real duplicate: 47272 (leftover) then 47533 (fresh)."""
        procs = [
            {"pid": 47272, "started": "Thu Aug  7 10:29:41 2026", "role": "child"},
            {"pid": 47533, "started": "Thu Aug  7 10:31:02 2026", "role": "child"},
        ]
        killed = []
        with mock.patch.object(cc, "game_processes", return_value=procs), \
             mock.patch.object(cc.subprocess, "run",
                               side_effect=lambda cmd, **kw: killed.append(cmd)), \
             mock.patch.object(cc.time, "sleep"):
            report = cc.ensure_single_game(kill=True)
        self.assertEqual(report["kept"]["pid"], 47272)
        self.assertEqual([["kill", "-9", "47533"]], killed)

    def test_the_stub_is_not_counted_as_a_game(self) -> None:
        procs = [{"pid": 9, "started": "x", "role": "stub"}]
        with mock.patch.object(cc, "game_processes", return_value=procs):
            report = cc.ensure_single_game(kill=False)
        self.assertIsNone(report["kept"])
        self.assertEqual(report["would_kill"], [])

    def test_dry_run_kills_nothing(self) -> None:
        procs = [
            {"pid": 1, "started": "a", "role": "child"},
            {"pid": 2, "started": "b", "role": "child"},
        ]
        with mock.patch.object(cc, "game_processes", return_value=procs), \
             mock.patch.object(cc.subprocess, "run") as run:
            report = cc.ensure_single_game(kill=False)
        run.assert_not_called()
        self.assertEqual([p["pid"] for p in report["would_kill"]], [2])


class CensusPolicyTest(unittest.TestCase):
    def test_steams_ordinary_main_window_is_not_censused_as_a_modal(self) -> None:
        """Measured: `census` reported steam_osx's client window, text empty."""
        def fake_osascript(script, timeout=30.0):
            if "count windows" in script and "steam_osx" in script:
                return mock.Mock(returncode=0, stdout="1\n", stderr="")
            if "static text" in script:
                return mock.Mock(returncode=0, stdout="\n", stderr="")
            if "every button" in script:
                return mock.Mock(returncode=0, stdout="\n", stderr="")
            return mock.Mock(returncode=1, stdout="", stderr="no window")
        with mock.patch.object(cc, "_osascript", side_effect=fake_osascript):
            self.assertEqual(cc.modal_census(), [])

    def test_steams_launch_error_dialog_is_censused(self) -> None:
        def fake_osascript(script, timeout=30.0):
            if "count windows" in script and "steam_osx" in script:
                return mock.Mock(returncode=0, stdout="1\n", stderr="")
            if "static text" in script and "steam_osx" in script:
                return mock.Mock(returncode=0, stdout="Game configuration unavailable\n",
                                 stderr="")
            if "every button" in script and "steam_osx" in script:
                return mock.Mock(returncode=0, stdout="OK\n", stderr="")
            return mock.Mock(returncode=1, stdout="", stderr="no window")
        with mock.patch.object(cc, "_osascript", side_effect=fake_osascript):
            census = cc.modal_census()
        self.assertEqual(len(census), 1)
        self.assertTrue(census[0]["recognized"])
        self.assertEqual(cc.choose_dismissal(census[0]), "OK")


class GameProcessParseTest(unittest.TestCase):
    def test_child_and_stub_are_distinguished_from_real_ps_output(self) -> None:
        ps = ("  47272 Thu Aug  7 10:29:41 2026 /…/Civ6.app/Contents/MacOS/Civ6_Exe_Child\n"
              "  47000 Thu Aug  7 10:28:02 2026 /…/Civ6.app/Contents/MacOS/Civ6_Exe\n"
              "  50000 Thu Aug  7 10:32:00 2026 vim Civ6_notes.txt\n")
        with mock.patch.object(cc.subprocess, "run",
                               return_value=mock.Mock(stdout=ps)):
            rows = cc.game_processes()
        roles = {r["pid"]: r["role"] for r in rows}
        self.assertEqual(roles[47272], "child")
        self.assertEqual(roles[47000], "stub")
        # The vim line mentions Civ6 but is neither binary; it must not appear
        # as a stub — only the real executables count.
        self.assertNotIn(50000, {r["pid"] for r in rows if r["role"] == "child"})


class LayoutReportTest(unittest.TestCase):
    def test_every_assignment_is_reported_even_when_a_window_is_missing(self) -> None:
        with mock.patch.object(cc, "desktop_points", return_value=(1728, 1117)), \
             mock.patch.object(cc, "place_window",
                               side_effect=[None, "no visible window matches", None]):
            report = cc.layout()
        self.assertEqual([r["placed"] for r in report], [True, False, True])

    def test_no_desktop_size_is_one_error_not_a_crash(self) -> None:
        with mock.patch.object(cc, "desktop_points", return_value=None):
            report = cc.layout()
        self.assertEqual(len(report), 1)
        self.assertIn("error", report[0])


if __name__ == "__main__":
    unittest.main()
