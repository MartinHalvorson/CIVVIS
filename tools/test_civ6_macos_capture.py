#!/usr/bin/env python3
"""Checks for the fast macOS screenshot helper used by popup_clear."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import call, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import macos_capture  # noqa: E402


def completed(arguments, **_kwargs):
    Path(arguments[-1]).write_bytes(b"png")
    return subprocess.CompletedProcess(arguments, 0, "", "")


class MacOSCaptureTest(unittest.TestCase):
    def test_helper_uses_the_fast_coregraphics_symbol_and_noninteractive_preflight(self) -> None:
        source = macos_capture._SWIFT_SOURCE
        self.assertIn('dlsym(framework, "CGWindowListCreateImage")', source)
        self.assertIn('dlsym(framework, "CGDisplayCreateImage")', source)
        self.assertNotIn("CGWindowListCreateImage(\n", source)
        self.assertNotIn("CGDisplayCreateImage(\n", source)
        self.assertIn("image.cropping(to: crop)", source)
        self.assertIn("import ScreenCaptureKit", source)
        self.assertIn("SCScreenshotManager.captureImage(in: rect)", source)
        self.assertIn("if #available(macOS 15.0, *)", source)
        self.assertIn('let fallbackMode = rawArguments.first == "--fallback"', source)
        self.assertIn("image = screenCaptureKitImage()", source)
        self.assertIn("image = windowListImage()", source)
        self.assertIn("exit(signalFallback ? 78 : 1)", source)
        self.assertIn("CGPreflightScreenCaptureAccess()", source)
        self.assertNotIn("CGRequestScreenCaptureAccess", source)

    def test_capture_passes_a_screen_point_region_to_the_cached_helper(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            output = Path(root) / "shot.png"
            with patch.object(macos_capture, "_native_binary",
                              return_value=Path("/tmp/cgcapture")), \
                 patch.object(macos_capture.subprocess, "run",
                              side_effect=completed) as run:
                macos_capture.capture_region((864, 33, 864, 542), output)

        run.assert_called_once_with(
            ["/tmp/cgcapture", "864", "33", "864", "542", str(output)],
            capture_output=True,
            text=True,
            check=False,
            timeout=macos_capture.NATIVE_TIMEOUT_SECONDS,
        )

    def test_capture_retries_in_a_fresh_window_list_helper_after_a_screen_capture_kit_miss(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            output = Path(root) / "shot.png"
            initial = subprocess.CompletedProcess(
                ["/tmp/cgcapture"], macos_capture.SCREEN_CAPTURE_FALLBACK_NEEDED,
                "", "ScreenCaptureKit returned no image",
            )
            def screen_capture_kit_then_window_list(arguments, **kwargs):
                if "--fallback" in arguments:
                    return completed(arguments, **kwargs)
                return initial
            with patch.object(macos_capture, "_native_binary",
                              return_value=Path("/tmp/cgcapture")), \
                 patch.object(macos_capture.subprocess, "run",
                              side_effect=screen_capture_kit_then_window_list) as run:
                macos_capture.capture_region((864, 33, 864, 542), output)

        normal = ["/tmp/cgcapture", "864", "33", "864", "542", str(output)]
        fallback = ["/tmp/cgcapture", "--fallback", "864", "33", "864", "542", str(output)]
        expected = dict(capture_output=True, text=True, check=False,
                        timeout=macos_capture.NATIVE_TIMEOUT_SECONDS)
        self.assertEqual(run.call_args_list, [call(normal, **expected), call(fallback, **expected)])

    def test_capture_does_not_compound_a_stalled_primary_backend(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            output = Path(root) / "shot.png"
            timeout = subprocess.TimeoutExpired(["/tmp/cgcapture"],
                                                macos_capture.NATIVE_TIMEOUT_SECONDS)
            with patch.object(macos_capture, "_native_binary",
                              return_value=Path("/tmp/cgcapture")), \
                 patch.object(macos_capture.subprocess, "run",
                              side_effect=timeout) as run:
                with self.assertRaises(macos_capture.CaptureUnavailable):
                    macos_capture.capture_region((864, 33, 864, 542), output)

        self.assertEqual(run.call_count, 1)

    def test_preflight_reports_denial_without_attempting_a_capture(self) -> None:
        with patch.object(macos_capture, "_native_binary",
                          return_value=Path("/tmp/cgcapture")), \
             patch.object(macos_capture.subprocess, "run", return_value=subprocess.CompletedProcess(
                 ["/tmp/cgcapture", "--preflight"],
                 macos_capture.SCREEN_CAPTURE_PERMISSION_DENIED,
                 "",
                 "screen capture permission unavailable",
             )) as run:
            self.assertFalse(macos_capture.screen_capture_access_available())

        run.assert_called_once_with(
            ["/tmp/cgcapture", "--preflight"],
            capture_output=True,
            text=True,
            check=False,
            timeout=macos_capture.NATIVE_TIMEOUT_SECONDS,
        )

    def test_capture_maps_permission_denial_to_a_specific_safe_error(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            output = Path(root) / "shot.png"
            with patch.object(macos_capture, "_native_binary",
                              return_value=Path("/tmp/cgcapture")), \
                 patch.object(macos_capture.subprocess, "run", return_value=subprocess.CompletedProcess(
                     ["/tmp/cgcapture"],
                     macos_capture.SCREEN_CAPTURE_PERMISSION_DENIED,
                     "",
                     "screen capture permission unavailable",
                 )):
                with self.assertRaises(macos_capture.CapturePermissionUnavailable):
                    macos_capture.capture_region((0, 0, 864, 542), output)


class TheKillIsLooserThanTheHelpersOwnGuard(unittest.TestCase):
    """⚠⚠ It was not, and a correct give-up was recorded as a crash.

    The Swift helper guards its ScreenCaptureKit call with a 3500 ms semaphore.
    Python killed it at 5 s, leaving 1.49 s for process start, framework load
    and exit.

    Measured 2026-08-28 on this host: a healthy capture takes 0.06 s, and a
    capture during a `systemstatusd` spin returns cleanly at **3.51 s** — the
    guard doing exactly its job. Under the load a spin implies, that margin was
    not always enough: `popup_clear.log` carries 379 `timed out after 5 seconds`
    kills in one day, steady at 10-100 per hour.

    The difference is not cosmetic. A helper that RETURNS reports "no image this
    pass" and the clearer retries next poll; a helper that is KILLED is an
    error, and an error blinds the popup backstop for thirty seconds. Run
    civvis-20260828T210457Z wedged at turn 77 with six cities after five
    straight minutes of error, pause, resume, error.
    """

    def test_the_outer_kill_leaves_the_inner_guard_room_to_report(self):
        self.assertGreater(macos_capture.NATIVE_TIMEOUT_SECONDS,
                           macos_capture.NATIVE_GUARD_SECONDS + 2.0,
                           "a helper giving up at its own guard must be able to "
                           "start, unwind and exit before Python kills it")

    def test_the_guard_matches_the_swift_semaphore_it_mirrors(self):
        """The constant is a copy of a number in the embedded Swift source."""
        source = (Path(macos_capture.__file__)).read_text(encoding="utf-8")
        self.assertIn(".milliseconds(3500)", source)
        self.assertEqual(macos_capture.NATIVE_GUARD_SECONDS, 3.5)

    def test_no_call_site_hardcodes_its_own_timeout(self):
        """Both invocations must move together with the guard."""
        source = (Path(macos_capture.__file__)).read_text(encoding="utf-8")
        self.assertNotIn("timeout=5", source)
        self.assertEqual(source.count("timeout=NATIVE_TIMEOUT_SECONDS"), 2)


if __name__ == "__main__":
    unittest.main()
