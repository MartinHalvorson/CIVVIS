#!/usr/bin/env python3
"""Focused contracts for Civ VI's macOS window and capture boundary."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_play
from civ6_control import macos_capture, macos_window


class DesktopGeometryTests(unittest.TestCase):
    def setUp(self) -> None:
        macos_window.reset_desktop_size_cache()

    def test_rejects_a_multi_display_union(self) -> None:
        with patch.object(
            macos_window.subprocess,
            "run",
            return_value=SimpleNamespace(stdout="3225,2557", returncode=0),
        ):
            self.assertIsNone(macos_window.desktop_size())

    def test_accepts_a_single_display(self) -> None:
        with patch.object(
            macos_window.subprocess,
            "run",
            return_value=SimpleNamespace(stdout="1728,1117", returncode=0),
        ):
            self.assertEqual(macos_window.desktop_size(), (1728, 1117))

    def test_reuses_one_verified_measurement_for_a_run(self) -> None:
        with patch.object(
            macos_window.subprocess,
            "run",
            return_value=SimpleNamespace(stdout="1728,1117", returncode=0),
        ) as run:
            self.assertEqual(macos_window.desktop_size(), (1728, 1117))
            self.assertEqual(macos_window.desktop_size(), (1728, 1117))
        run.assert_called_once()

    def test_refuses_unreadable_measurements(self) -> None:
        for bad in ("", "not,numbers", "1728", "0,0"):
            with self.subTest(value=bad):
                macos_window.reset_desktop_size_cache()
                with patch.object(
                    macos_window.subprocess,
                    "run",
                    return_value=SimpleNamespace(stdout=bad, returncode=0),
                ):
                    self.assertIsNone(macos_window.desktop_size())


class WindowPlacementTests(unittest.TestCase):
    PROCESS = "Civ6"

    def test_sizes_before_positioning_the_upper_quadrant(self) -> None:
        with patch.object(macos_window.subprocess, "run") as run:
            macos_window.place_game(
                self.PROCESS,
                "right",
                0.5,
                0.5,
                get_desktop_size=lambda: (1512, 982),
                get_game_window=lambda: None,
            )

        script = run.call_args.args[0][-1]
        self.assertLess(script.index("set size"), script.index("set position"))
        self.assertIn("set size to {756, 480}", script)
        self.assertIn("set position to {756, 33}", script)
        self.assertIn(f'process "{self.PROCESS}"', script)

    def test_does_not_rewrite_an_unchanged_frame(self) -> None:
        with patch.object(macos_window.subprocess, "run") as run:
            macos_window.place_game(
                self.PROCESS,
                "right",
                0.5,
                0.5,
                get_desktop_size=lambda: (1512, 982),
                get_game_window=lambda: (756, 33, 756, 480),
            )
        run.assert_not_called()

    def test_queries_and_focus_target_the_actual_process(self) -> None:
        with patch.object(
            macos_window.subprocess,
            "run",
            return_value=SimpleNamespace(stdout="756, 33, 756, 480", returncode=0),
        ) as run:
            self.assertEqual(
                macos_window.game_window(self.PROCESS), (756, 33, 756, 480))
            macos_window.focus_game(self.PROCESS)

        scripts = [call.args[0][-1] for call in run.call_args_list]
        self.assertTrue(all(f'process "{self.PROCESS}"' in script for script in scripts))
        self.assertTrue(all("first process whose name contains" not in script
                            for script in scripts))

    def test_locked_console_state_is_read_from_ioreg(self) -> None:
        with patch.object(
            macos_window.subprocess,
            "run",
            return_value=SimpleNamespace(
                stdout='"CGSSessionScreenIsLocked"=Yes', returncode=0),
        ):
            self.assertTrue(macos_window.screen_locked())

        with patch.object(
            macos_window.subprocess,
            "run",
            return_value=SimpleNamespace(
                stdout='"CGSSessionScreenIsLocked"=No', returncode=0),
        ):
            self.assertFalse(macos_window.screen_locked())

    def test_wait_for_unlock_uses_the_injected_session_probe(self) -> None:
        locked = iter((True, True, False))
        with patch.object(macos_window.time, "sleep") as sleep:
            macos_window.wait_for_unlocked_session(
                is_locked=lambda: next(locked), poll_s=0.25)
        sleep.assert_called_once_with(0.25)


class ScreenCaptureTests(unittest.TestCase):
    def setUp(self) -> None:
        # These model the transient-miss retry ladder, which is HEALTHY-host
        # behaviour. Without this they would ask the real host whether capture
        # looks available and fail on a machine whose `systemstatusd` is
        # spinning -- an environment-dependent test, not a contract.
        healthy = patch.object(macos_window, "capture_looks_unavailable",
                               return_value=False)
        healthy.start()
        self.addCleanup(healthy.stop)

    SIZE = (1512, 982)

    def _shot(self, path: Path, **kwargs) -> bool:
        return macos_window.screenshot(
            path, get_desktop_size=lambda: self.SIZE, **kwargs)

    def test_missing_capture_is_retried_then_reported(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(macos_window.macos_capture, "capture_region") as capture, \
             patch.object(macos_window.time, "sleep") as sleep:
            landed = self._shot(Path(temporary) / "missing.png")
        self.assertFalse(landed)
        self.assertEqual(capture.call_count,
                         len(macos_window.SHOT_BACKOFF_SECONDS) + 1)
        self.assertEqual([call.args[0] for call in sleep.call_args_list],
                         list(macos_window.SHOT_BACKOFF_SECONDS))

    def test_setup_capture_leaves_retries_to_the_outer_screen_poll(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(macos_window.macos_capture, "capture_region") as capture, \
             patch.object(macos_window.time, "sleep") as sleep:
            landed = self._shot(
                Path(temporary) / "setup-miss.png",
                attempts=macos_window.SETUP_SCREENSHOT_ATTEMPTS,
            )
        self.assertFalse(landed)
        capture.assert_called_once()
        sleep.assert_not_called()

    def test_autoclose_snapshot_leaves_retries_to_the_next_event(self) -> None:
        """A diagnostic miss must not hold the live game for five captures."""
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(macos_window.macos_capture, "capture_region") as capture, \
             patch.object(macos_window.time, "sleep") as sleep:
            landed = self._shot(
                Path(temporary) / "autoclose-stuck-turn-71.png",
            )
        self.assertFalse(landed)
        capture.assert_called_once()
        sleep.assert_not_called()

    def test_launcher_setup_readers_keep_the_outer_capture_poll(self) -> None:
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text()
        self.assertIn("screenshot(menushot, attempts=SETUP_SCREENSHOT_ATTEMPTS)", source)
        self.assertIn("screenshot(submenu, attempts=SETUP_SCREENSHOT_ATTEMPTS)", source)

    def test_capture_can_recover_after_a_transient_spike(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(macos_window.time, "sleep"):
            path = Path(temporary) / "late.png"
            attempts = []

            def spike(_region, output):
                attempts.append(1)
                if len(attempts) >= 3:
                    Path(output).write_bytes(b"x")

            with patch.object(macos_window.macos_capture, "capture_region",
                              side_effect=spike):
                self.assertTrue(self._shot(path))
        self.assertEqual(len(attempts), 3)

    def test_recording_frame_can_recover_on_the_fifth_capture(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(macos_window.time, "sleep"):
            path = Path(temporary) / "recording-late.png"
            attempts = []

            def late_frame(_region, output):
                attempts.append(1)
                if len(attempts) == 5:
                    Path(output).write_bytes(b"fresh")

            with patch.object(macos_window.macos_capture, "capture_region",
                              side_effect=late_frame):
                self.assertTrue(self._shot(path))
        self.assertEqual(len(attempts), 5)

    def test_backoff_is_escalating_and_bounded(self) -> None:
        steps = macos_window.SHOT_BACKOFF_SECONDS
        self.assertEqual(list(steps), sorted(steps))
        self.assertLess(sum(steps), 10.0)

    def test_first_capture_that_lands_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "shot.png"

            def fake_capture(_region, output):
                Path(output).write_bytes(b"x")

            with patch.object(macos_window.macos_capture, "capture_region",
                              side_effect=fake_capture) as capture:
                self.assertTrue(self._shot(path))
            capture.assert_called_once_with((0, 0, *self.SIZE), path)

    def test_a_preauthorized_capture_frame_replaces_a_stale_one(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "shot.png"
            path.write_bytes(b"stale")

            def capture_frame(_region, output):
                Path(output).write_bytes(b"fresh")

            with patch.object(macos_window.macos_capture, "capture_region",
                              side_effect=capture_frame) as capture:
                self.assertTrue(self._shot(path))
            capture.assert_called_once_with((0, 0, *self.SIZE), path)
            self.assertEqual(path.read_bytes(), b"fresh")

    def test_permission_denial_never_retries_or_requests_a_system_popup(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(
                 macos_window.macos_capture,
                 "capture_region",
                 side_effect=macos_capture.CapturePermissionUnavailable("denied"),
             ) as capture, \
             patch.object(macos_window.time, "sleep") as sleep:
            self.assertFalse(self._shot(Path(temporary) / "shot.png"))
        capture.assert_called_once()
        sleep.assert_not_called()


class SafeScreenCaptureTests(unittest.TestCase):
    def test_preauthorized_user_capture_ui_does_not_delay_setup(self) -> None:
        with patch.object(macos_window.popup_clear, "native_recording_ui_active",
                          return_value=True) as recording, \
             patch.object(macos_window.macos_capture,
                          "screen_capture_access_available", return_value=True) as access, \
             patch.object(macos_window.macos_capture,
                          "capture_probe", return_value=True) as probe, \
             patch.object(macos_window.time, "sleep") as sleep:
            macos_window.wait_for_safe_screen_capture(poll_s=0.25)

        recording.assert_called_once_with()
        access.assert_called_once_with()
        probe.assert_called_once_with()
        sleep.assert_not_called()

    def test_recording_ui_waits_only_until_preflight_passes(self) -> None:
        with patch.object(macos_window.popup_clear, "native_recording_ui_active",
                          return_value=True), \
             patch.object(macos_window.macos_capture,
                          "screen_capture_access_available", side_effect=[False, True]) as access, \
             patch.object(macos_window.macos_capture,
                          "capture_probe", return_value=True) as probe, \
             patch.object(macos_window.time, "sleep") as sleep:
            macos_window.wait_for_safe_screen_capture(poll_s=0.25)

        self.assertEqual(access.call_count, 2)
        probe.assert_called_once_with()
        sleep.assert_called_once_with(0.25)

    def test_missing_permission_is_deferred_until_preflight_passes(self) -> None:
        with patch.object(macos_window.popup_clear, "native_recording_ui_active",
                          return_value=False), \
             patch.object(macos_window.macos_capture,
                          "screen_capture_access_available", side_effect=[False, True]) as access, \
             patch.object(macos_window.macos_capture,
                          "capture_probe", return_value=True) as probe, \
             patch.object(macos_window.time, "sleep") as sleep:
            macos_window.wait_for_safe_screen_capture(poll_s=0.25)

        self.assertEqual(access.call_count, 2)
        probe.assert_called_once_with()
        sleep.assert_called_once_with(0.25)

    def test_authorized_but_empty_capture_waits_and_rechecks(self) -> None:
        with patch.object(macos_window.popup_clear, "native_recording_ui_active",
                          return_value=True), \
             patch.object(macos_window.macos_capture,
                          "screen_capture_access_available", return_value=True), \
             patch.object(macos_window.macos_capture,
                          "capture_probe", side_effect=[False, True]) as probe, \
             patch.object(macos_window.popup_clear,
                          "recover_stale_interactive_recording", return_value=False), \
             patch.object(macos_window.time, "sleep") as sleep:
            macos_window.wait_for_safe_screen_capture(poll_s=0.25)

        self.assertEqual(probe.call_count, 2)
        sleep.assert_called_once_with(0.25)

    def test_a_daemon_spike_without_a_recorder_does_not_hold_startup(self) -> None:
        with patch.object(macos_window.popup_clear, "native_recording_ui_active",
                          return_value=False), \
             patch.object(macos_window.macos_capture,
                          "screen_capture_access_available", return_value=True), \
             patch.object(macos_window.macos_capture,
                          "capture_probe", return_value=False), \
             patch.object(macos_window.popup_clear,
                          "recover_stale_interactive_recording", return_value=False) as recover, \
             patch.object(macos_window.time, "sleep") as sleep:
            macos_window.wait_for_safe_screen_capture(poll_s=0.25)

        recover.assert_called_once_with()
        sleep.assert_not_called()

    def test_stale_capture_recovery_is_retried_before_waiting(self) -> None:
        with patch.object(macos_window.popup_clear, "native_recording_ui_active",
                          return_value=True), \
             patch.object(macos_window.macos_capture,
                          "screen_capture_access_available", return_value=True), \
             patch.object(macos_window.macos_capture,
                          "capture_probe", side_effect=[False, True]), \
             patch.object(macos_window.popup_clear,
                          "recover_stale_interactive_recording", return_value=True) as recover, \
             patch.object(macos_window.macos_capture,
                          "reset_fallback_breaker") as reset, \
             patch.object(macos_window.time, "sleep") as sleep:
            macos_window.wait_for_safe_screen_capture(poll_s=0.25)

        recover.assert_called_once_with()
        reset.assert_called_once_with()
        sleep.assert_not_called()


class BoundedHostProbeTests(unittest.TestCase):
    PROCESS = "Civ6"

    def test_window_timeout_means_unknown_geometry(self) -> None:
        with patch.object(macos_window.subprocess, "run") as run:
            run.return_value = mock.Mock(stdout="10, 20, 800, 600")
            self.assertEqual(macos_window.game_window(self.PROCESS),
                             (10, 20, 800, 600))
            self.assertEqual(run.call_args.kwargs.get("timeout"),
                             macos_window.HOST_PROBE_TIMEOUT_S)
        with patch.object(
            macos_window.subprocess,
            "run",
            side_effect=subprocess.TimeoutExpired("osascript", 10),
        ):
            self.assertIsNone(macos_window.game_window(self.PROCESS))

    def test_timeout_in_lock_probe_keeps_the_game_playing(self) -> None:
        with patch.object(
            macos_window.subprocess,
            "run",
            side_effect=subprocess.TimeoutExpired("ioreg", 10),
        ):
            self.assertFalse(macos_window.screen_locked())

    def test_lock_probe_keeps_its_timeout_when_it_answers(self) -> None:
        with patch.object(macos_window.subprocess, "run") as run:
            run.return_value = mock.Mock(
                stdout='  "CGSSessionScreenIsLocked"=Yes')
            self.assertTrue(macos_window.screen_locked())
            self.assertEqual(run.call_args.kwargs.get("timeout"),
                             macos_window.HOST_PROBE_TIMEOUT_S)

    def test_placing_and_focusing_survive_stuck_system_events(self) -> None:
        with patch.object(
            macos_window.subprocess,
            "run",
            side_effect=subprocess.TimeoutExpired("osascript", 10),
        ):
            macos_window.focus_game(self.PROCESS)
            macos_window.place_game(
                self.PROCESS,
                get_desktop_size=lambda: (1512, 982),
                get_game_window=lambda: None,
            )

    def test_every_host_subprocess_is_bounded(self) -> None:
        import ast

        source = Path(macos_window.__file__).read_text(encoding="utf-8")
        unbounded = []
        for node in ast.walk(ast.parse(source)):
            if not isinstance(node, ast.Call):
                continue
            if getattr(node.func, "attr", None) not in (
                "run", "check_output", "call", "check_call"):
                continue
            owner = getattr(getattr(node.func, "value", None), "id", None)
            if owner == "subprocess" and "timeout" not in {
                    keyword.arg for keyword in node.keywords}:
                unbounded.append(node.lineno)
        self.assertEqual(unbounded, [],
                         f"macos_window.py line(s) {unbounded} lack a timeout")


class LauncherCompatibilityTests(unittest.TestCase):
    def test_launcher_routes_placement_through_the_host_boundary(self) -> None:
        with patch.object(civ6_play.macos_window, "place_game") as place, \
             patch.object(civ6_play, "desktop_size") as desktop, \
             patch.object(civ6_play, "game_window") as window:
            civ6_play.place_game("right", 0.5, 0.5)
        self.assertEqual(place.call_args.args[:4],
                         (civ6_play.GAME_PROCESS, "right", 0.5, 0.5))
        self.assertIs(place.call_args.kwargs["get_desktop_size"], desktop)
        self.assertIs(place.call_args.kwargs["get_game_window"], window)

    def test_launcher_routes_capture_with_its_display_boundary(self) -> None:
        path = Path("frame.png")
        with patch.object(civ6_play.macos_window, "screenshot", return_value=True) as shot, \
             patch.object(civ6_play, "desktop_size") as desktop:
            self.assertTrue(civ6_play.screenshot(path, attempts=1))
        self.assertEqual(shot.call_args.args, (path,))
        self.assertEqual(shot.call_args.kwargs["attempts"], 1)
        self.assertIs(shot.call_args.kwargs["get_desktop_size"], desktop)


class UnavailableHostShotBudgetTests(unittest.TestCase):
    """A host that says capture cannot work must not be asked five times.

    The retry ladder is priced for a transient miss.  While `systemstatusd`
    spins, every attempt runs the native 3.5 s guard out and returns nothing,
    so the full ladder costs 5 x 3.5 + 9.0 = 26.5 s per unreadable poll.
    """

    def _run_shot(self, *, unavailable: bool):
        attempts = []
        sleeps = []

        def capture(_region, path):
            attempts.append(path)
            path.write_bytes(b"")  # an empty frame: the failure this rides out

        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "frame.png"
            with patch.object(macos_window.macos_capture, "capture_region", capture), \
                 patch.object(macos_window, "capture_looks_unavailable",
                              return_value=unavailable), \
                 patch.object(macos_window.time, "sleep", sleeps.append):
                ok = macos_window.screenshot(target, get_desktop_size=lambda: (100, 100))
        return ok, attempts, sleeps

    def test_a_healthy_host_still_spends_the_whole_ladder(self) -> None:
        ok, attempts, sleeps = self._run_shot(unavailable=False)
        self.assertFalse(ok)
        self.assertEqual(len(attempts), len(macos_window.SHOT_BACKOFF_SECONDS) + 1)
        self.assertEqual(sum(sleeps), sum(macos_window.SHOT_BACKOFF_SECONDS))

    def test_an_unavailable_host_is_capped_but_still_retried_once(self) -> None:
        ok, attempts, sleeps = self._run_shot(unavailable=True)
        self.assertFalse(ok)
        # Capped -- and the cap is not a veto: the shot was still taken twice.
        self.assertEqual(len(attempts), macos_window.UNAVAILABLE_SHOT_ATTEMPTS)
        self.assertGreater(macos_window.UNAVAILABLE_SHOT_ATTEMPTS, 1)
        self.assertEqual(sum(sleeps), macos_window.SHOT_BACKOFF_SECONDS[0])

    def test_the_prediction_never_ends_a_run(self) -> None:
        """`capture_looks_unavailable` answers "no" rather than raising."""
        from civ6_control import popup_clear

        with patch.object(popup_clear, "capture_pause_reason",
                          side_effect=RuntimeError("host probe exploded")):
            self.assertFalse(macos_window.capture_looks_unavailable())


if __name__ == "__main__":
    unittest.main()
