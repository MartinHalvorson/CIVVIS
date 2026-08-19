#!/usr/bin/env python3
"""Regression tests for the systematic host-control module."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
import subprocess
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

    def test_the_local_network_prompt_that_covered_a_recorded_game_is_denied(self) -> None:
        """Measured 2026-08-19: this sheet sat over a recorded verification game.

        Its buttons are Allow / Don't Allow, so none of OK, Cancel or Close was
        present and the keeper left it up until a human cleared it.
        """
        modal = {"owner": "UserNotificationCenter",
                 "text": "Allow \u201cGoogle Chrome\u201d to find devices on local "
                         "networks?, This will allow you to select from available "
                         "devices and display content on them.",
                 "buttons": ["Don\u2019t Allow", "Allow"]}
        self.assertEqual(cc.choose_dismissal(modal), "Don\u2019t Allow")

    def test_a_straight_apostrophe_deny_button_is_matched_too(self) -> None:
        """macOS renders the curly form, but the accessibility name is not guaranteed."""
        modal = {"owner": "UserNotificationCenter",
                 "text": "Allow \u201cSteam\u201d to find devices on local networks?",
                 "buttons": ["Don't Allow", "Allow"]}
        self.assertEqual(cc.choose_dismissal(modal), "Don't Allow")

    def test_a_permission_prompt_is_never_granted(self) -> None:
        """Granting a permission is never this tool's to do -- only declining is."""
        modal = {"owner": "UserNotificationCenter",
                 "text": "Allow \u201cGoogle Chrome\u201d to find devices on local networks?",
                 "buttons": ["Allow"]}
        self.assertIsNone(cc.choose_dismissal(modal))

    def test_a_file_access_prompt_is_reported_rather_than_silently_denied(self) -> None:
        """The reason denial is text-gated instead of blanket.

        Auto-denying this one would cut the lane off from its own run artifacts,
        and the sheet gives no hint that it was answered.  Report it and let a
        human decide.
        """
        modal = {"owner": "UserNotificationCenter",
                 "text": "\u201cTerminal\u201d would like to access files in your "
                         "Documents folder.",
                 "buttons": ["Don\u2019t Allow", "Allow"]}
        self.assertIsNone(cc.choose_dismissal(modal))


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


class ScreenshotScaleTest(unittest.TestCase):
    def test_the_scale_that_put_a_click_on_the_ad_carousel_is_reported(self) -> None:
        with mock.patch.object(cc.subprocess, "run"), \
             mock.patch.object(cc, "desktop_points", return_value=(1728, 1117)):
            report = cc.screenshot(Path("/tmp/x.png"), max_dimension=1400)
        self.assertAlmostEqual(report["scale"], 0.8102, places=4)

    def test_a_capture_smaller_than_the_cap_is_not_upscaled(self) -> None:
        with mock.patch.object(cc.subprocess, "run"), \
             mock.patch.object(cc, "desktop_points", return_value=(1280, 800)):
            report = cc.screenshot(Path("/tmp/x.png"), max_dimension=1400)
        self.assertEqual(report["scale"], 1.0)


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

    def test_a_title_spec_cannot_claim_a_window_a_process_spec_owns(self) -> None:
        """The upper-left slot must not be given the terminal.

        `CIVVIS|127\\.0\\.0\\.1` is deliberately loose and it matched a *Terminal*
        window on this host, because a shell session had been named "CIVVIS gaps
        and priorities". Terminal is placed by its own process spec, so the live
        mirror would have lost the slot to a window that already had one.

        Drives `place_window` for real; only `_osascript` is faked.
        """
        listing = ("Terminal\tmartbot \u2014 CIVVIS gaps and priorities\n"
                   "Google Chrome\tCIVVIS \u00b7 Civ VI Simulator\n")
        scripts = []

        def fake(script, timeout=30.0):
            scripts.append(script)
            out = listing if "every window of proc" in script else ""
            return subprocess.CompletedProcess([], 0, out, "")

        with mock.patch.object(cc, "_osascript", side_effect=fake):
            error = cc.place_window((0, 0, 10, 10), title=r"CIVVIS|127\.0\.0\.1",
                                    skip_owners=frozenset({"Terminal"}))
        self.assertIsNone(error)
        placed = scripts[-1]
        self.assertIn("Google Chrome", placed)
        self.assertNotIn("gaps and priorities", placed)


class WindowListingRetryTest(unittest.TestCase):
    """A racing enumeration is not "no window matches"."""

    def _listing(self, rc: int, out: str = "", err: str = ""):
        return subprocess.CompletedProcess([], rc, out, err)

    def test_a_transient_failure_is_retried_and_then_succeeds(self) -> None:
        """`-1719` while Civilization VI launches, which is when layout is wanted.

        Walking every visible process's windows fails when the process list
        changes under it. Observed twice on 2026-08-18: `layout` reported the
        mirror as `placed: false`, and the identical call succeeded seconds later
        with nothing else changed — so the operator was sent looking for a
        mapping bug that did not exist.
        """
        found = "Google Chrome\tCIVVIS \u00b7 Civ VI Simulator\n"
        calls = [
            self._listing(1, err="System Events got an error: Invalid index. (-1719)"),
            self._listing(0, found),
            self._listing(0, ""),   # the placement call
        ]
        with mock.patch.object(cc, "_osascript", side_effect=calls), \
             mock.patch.object(cc.time, "sleep"):
            error = cc.place_window((0, 0, 10, 10), title="CIVVIS")
        self.assertIsNone(error)

    def test_a_persistent_failure_still_reports_its_own_message(self) -> None:
        """A retry loop must not swallow a real failure into a generic one."""
        calls = [self._listing(1, err="System Events got an error: boom")] * 3
        with mock.patch.object(cc, "_osascript", side_effect=calls), \
             mock.patch.object(cc.time, "sleep"):
            error = cc.place_window((0, 0, 10, 10), title="CIVVIS")
        self.assertIn("boom", error)

    def test_a_window_that_is_really_absent_is_not_retried_into_existence(self) -> None:
        """An empty listing is an answer, not a failure — one call, one verdict."""
        with mock.patch.object(cc, "_osascript",
                               side_effect=[self._listing(0, "Finder\tDesktop\n")]) as spy:
            error = cc.place_window((0, 0, 10, 10), title="CIVVIS")
        self.assertIn("no visible window matches", error)
        self.assertEqual(spy.call_count, 1)


class AppleScriptStringTest(unittest.TestCase):
    """A window title is data, and AppleScript has to be handed it as data."""

    def test_the_script_place_window_builds_carries_no_backslash_u(self) -> None:
        """`json.dumps` emits `\\uXXXX`, which AppleScript cannot parse.

        Chrome titles the live viewer `CIVVIS \u00b7 Civ VI Simulator`, so the one
        window the standard layout places by title carried a character JSON would
        escape and osascript rejected with error -2741 — `placed: false`, every
        time, on the slot the operator watches the game beside.

        Asserted on the script `place_window` actually builds, not on the helper
        in isolation: the defect was at the call site.
        """
        listing = "Google Chrome\tCIVVIS \u00b7 Civ VI Simulator\n"

        def fake(script, timeout=30.0):
            out = listing if "every window of proc" in script else ""
            return subprocess.CompletedProcess([], 0, out, "")

        with mock.patch.object(cc, "_osascript", side_effect=fake) as spy:
            cc.place_window((0, 0, 10, 10), title="CIVVIS")
        built = spy.call_args_list[-1].args[0]
        self.assertNotIn("\\u", built)
        self.assertIn("\u00b7", built)

    def test_the_two_characters_that_end_a_literal_are_escaped(self) -> None:
        self.assertEqual(cc._as('a"b'), '"a\\"b"')
        self.assertEqual(cc._as("a\\b"), '"a\\\\b"')


class ProcessWindowSelectionTest(unittest.TestCase):
    def test_process_specs_choose_by_requested_frame_not_window_order(self) -> None:
        with mock.patch.object(
            cc, "_osascript",
            return_value=subprocess.CompletedProcess([], 0, "", ""),
        ) as run:
            self.assertIsNone(cc.place_window((0, 558, 864, 559), process="Terminal"))

        script = run.call_args.args[0]
        self.assertIn("repeat with i from 1 to (count of windows)", script)
        self.assertIn("candidateScore", script)
        self.assertIn("set position to {0, 558}", script)
        self.assertNotIn("to tell window 1", script)


if __name__ == "__main__":
    unittest.main()
