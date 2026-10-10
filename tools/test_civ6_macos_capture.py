#!/usr/bin/env python3
"""Checks for the fast macOS screenshot helper used by popup_clear."""

from __future__ import annotations

import os
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import macos_capture  # noqa: E402


# A stand-in for `cgcapture-<digest> --serve [--fallback]` that speaks the same
# line protocol. FAKE_CAPTURE_NATIVE / FAKE_CAPTURE_FALLBACK script each
# backend: "ok" writes the frame, a number replies that status, "hang" never
# answers, "crash" exits mid-request. Every request is logged with the pid that
# served it, so a test can see whether frames shared one helper.
FAKE_HELPER = r"""
import os, sys, time
fallback = "--fallback" in sys.argv
backend = "fallback" if fallback else "native"
while True:
    line = sys.stdin.readline()
    if not line:
        break
    with open(os.environ["FAKE_CAPTURE_LOG"], "a") as log:
        log.write(f"{os.getpid()} {backend} {line}")
    behaviour = os.environ.get("FAKE_CAPTURE_" + backend.upper(), "ok")
    if behaviour == "hang":
        time.sleep(60)
    elif behaviour == "crash":
        sys.exit(3)
    elif behaviour != "ok":
        print(f"{behaviour} scripted {backend} failure", flush=True)
        continue
    with open(line.rstrip("\n").split(" ", 4)[4], "wb") as frame:
        frame.write(b"png")
    print("0", flush=True)
"""


class FakeHelperCase(unittest.TestCase):
    """Run `capture_region` against FAKE_HELPER through real pipes."""

    def setUp(self) -> None:
        macos_capture.stop_capture_servers()
        macos_capture.reset_fallback_breaker()
        self.addCleanup(macos_capture.reset_fallback_breaker)
        self.addCleanup(macos_capture.stop_capture_servers)
        root = tempfile.TemporaryDirectory()
        self.addCleanup(root.cleanup)
        self.root = Path(root.name)
        script = self.root / "fake_helper.py"
        script.write_text(FAKE_HELPER)
        self.binary = self.root / "cgcapture"
        self.binary.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{script}" "$@"\n')
        self.binary.chmod(self.binary.stat().st_mode | stat.S_IXUSR)
        self.log = self.root / "requests.log"
        self.log.touch()
        for patcher in (
            patch.object(macos_capture, "_native_binary", return_value=self.binary),
            patch.dict(os.environ, {"FAKE_CAPTURE_LOG": str(self.log)}),
        ):
            patcher.start()
            self.addCleanup(patcher.stop)
        self.output = self.root / "shot.png"

    def behave(self, native: str = "ok", fallback: str = "ok") -> None:
        os.environ["FAKE_CAPTURE_NATIVE"] = native
        os.environ["FAKE_CAPTURE_FALLBACK"] = fallback

    def requests(self) -> list[tuple[int, str, str]]:
        rows = []
        for line in self.log.read_text().splitlines():
            pid, backend, request = line.split(" ", 2)
            rows.append((int(pid), backend, request))
        return rows


class MacOSCaptureTest(FakeHelperCase):

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
        self.assertIn('let serveMode = rawArguments.first == "--serve"', source)
        self.assertIn('let fallbackMode = modeArguments.first == "--fallback"', source)
        self.assertIn("image = screenCaptureKitImage(rect)", source)
        self.assertIn("image = windowListImage(rect)", source)
        self.assertIn("return (signalFallback ? 78 : 1,", source)
        self.assertIn("guard let request = readLine() else { break }", source)
        self.assertIn("alarm(120)", source)
        self.assertLess(macos_capture.SERVER_MAX_IDLE_SECONDS, 120)
        self.assertIn("CGPreflightScreenCaptureAccess()", source)
        self.assertNotIn("CGRequestScreenCaptureAccess", source)

    def test_capture_passes_a_screen_point_region_to_the_cached_helper(self) -> None:
        self.behave()
        macos_capture.capture_region((864, 33, 864, 542), self.output)

        self.assertEqual([(backend, request) for _, backend, request in self.requests()],
                         [("native", f"864 33 864 542 {self.output}")])
        self.assertEqual(self.output.read_bytes(), b"png")

    def test_capture_retries_in_a_window_list_helper_after_a_screen_capture_kit_miss(self) -> None:
        self.behave(native=str(macos_capture.SCREEN_CAPTURE_FALLBACK_NEEDED))
        with patch("builtins.print"):
            macos_capture.capture_region((864, 33, 864, 542), self.output)

        rows = self.requests()
        self.assertEqual([backend for _, backend, _ in rows], ["native", "fallback"])
        # ScreenCaptureKit and CoreGraphics never share a process.
        self.assertNotEqual(rows[0][0], rows[1][0])
        self.assertEqual(self.output.read_bytes(), b"png")

    def test_capture_does_not_compound_a_stalled_primary_backend(self) -> None:
        self.behave(native="hang")
        with patch.object(macos_capture, "NATIVE_TIMEOUT_SECONDS", 0.5):
            with self.assertRaises(macos_capture.CaptureUnavailable):
                macos_capture.capture_region((864, 33, 864, 542), self.output)

        self.assertEqual([backend for _, backend, _ in self.requests()], ["native"])

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
        self.behave(native=str(macos_capture.SCREEN_CAPTURE_PERMISSION_DENIED))
        with self.assertRaises(macos_capture.CapturePermissionUnavailable):
            macos_capture.capture_region((0, 0, 864, 542), self.output)

    def test_capture_probe_uses_the_real_capture_path(self) -> None:
        with patch.object(macos_capture, "capture_region") as capture:
            self.assertTrue(macos_capture.capture_probe())

        capture.assert_called_once()
        self.assertEqual(capture.call_args.args[0], macos_capture.PROBE_REGION_POINTS)
        probe_output = capture.call_args.args[1]
        self.assertTrue(str(probe_output).endswith("/frame.png"))

    def test_capture_probe_treats_a_transient_empty_frame_as_unavailable(self) -> None:
        with patch.object(
            macos_capture,
            "capture_region",
            side_effect=macos_capture.CaptureUnavailable("no frame"),
        ):
            self.assertFalse(macos_capture.capture_probe())

    def test_capture_probe_keeps_permission_denial_distinct(self) -> None:
        with patch.object(
            macos_capture,
            "capture_region",
            side_effect=macos_capture.CapturePermissionUnavailable("denied"),
        ):
            with self.assertRaises(macos_capture.CapturePermissionUnavailable):
                macos_capture.capture_probe()


class OneHelperServesManyFrames(FakeHelperCase):
    """★★★★★ A process per frame stopped the operator's own recordings.

    Each fresh helper is a new screen-capture client `systemstatusd` must
    attribute and publish; ~1,500 an hour kept it spinning, and while it spins
    Cmd-Shift-5 dies one second after "Recording started"
    (`getDisplayForDisplayId timed out`). See `_CaptureServer`.
    """

    def test_consecutive_frames_share_one_helper_process(self) -> None:
        self.behave()
        for _ in range(5):
            macos_capture.capture_region((0, 33, 864, 542), self.output)
        pids = {pid for pid, _, _ in self.requests()}
        self.assertEqual(len(self.requests()), 5)
        self.assertEqual(len(pids), 1, "every frame must come from the same helper")

    def test_a_hung_helper_is_killed_and_the_next_frame_gets_a_fresh_one(self) -> None:
        self.behave(native="hang")
        with patch.object(macos_capture, "NATIVE_TIMEOUT_SECONDS", 0.5):
            self.assertIsNone(macos_capture._capture_once(
                macos_capture._capture_command((0, 0, 8, 8), self.output, fallback=False)))
        hung = self.requests()[0][0]
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            try:
                os.kill(hung, 0)
            except ProcessLookupError:
                break
            time.sleep(0.05)
        else:
            self.fail("the hung helper is still alive")
        self.behave()
        macos_capture.capture_region((0, 0, 8, 8), self.output)
        self.assertNotEqual(self.requests()[1][0], hung)

    def test_a_helper_that_crashes_mid_frame_fails_that_frame_only(self) -> None:
        self.behave(native="crash")
        with self.assertRaises(macos_capture.CaptureUnavailable):
            macos_capture.capture_region((0, 0, 8, 8), self.output)
        self.behave()
        macos_capture.capture_region((0, 0, 8, 8), self.output)
        first, second = (pid for pid, _, _ in self.requests())
        self.assertNotEqual(first, second)

    def assert_ended(self, pid: int) -> None:
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                return
            time.sleep(0.05)
        self.fail(f"helper {pid} is still alive")

    def test_a_failed_frame_retires_its_helper_at_once(self) -> None:
        # ★★★★★ A helper that failed a frame can be left re-dialling replayd
        # thousands of times a second for as long as it lives (2026-10-09).
        self.behave(native="1")
        with self.assertRaises(macos_capture.CaptureUnavailable):
            macos_capture.capture_region((0, 0, 8, 8), self.output)
        failed = self.requests()[0][0]
        self.assert_ended(failed)
        self.behave()
        macos_capture.capture_region((0, 0, 8, 8), self.output)
        self.assertNotEqual(self.requests()[1][0], failed)

    def test_a_screen_capture_kit_miss_retires_the_native_helper(self) -> None:
        self.behave(native=str(macos_capture.SCREEN_CAPTURE_FALLBACK_NEEDED))
        with patch("builtins.print"):
            macos_capture.capture_region((0, 0, 8, 8), self.output)
        native = next(pid for pid, backend, _ in self.requests() if backend == "native")
        self.assert_ended(native)
        self.assertEqual(self.output.read_bytes(), b"png")  # the fallback still served it

    def test_an_idle_helper_is_retired_before_the_next_frame(self) -> None:
        self.behave()
        macos_capture.capture_region((0, 0, 8, 8), self.output)
        with patch.object(macos_capture, "SERVER_MAX_IDLE_SECONDS", 0.0):
            macos_capture.capture_region((0, 0, 8, 8), self.output)
        first, second = (pid for pid, _, _ in self.requests())
        self.assertNotEqual(first, second)
        self.assert_ended(first)

    def test_a_permission_denial_recycles_the_helper(self) -> None:
        self.behave(native=str(macos_capture.SCREEN_CAPTURE_PERMISSION_DENIED))
        with self.assertRaises(macos_capture.CapturePermissionUnavailable):
            macos_capture.capture_region((0, 0, 8, 8), self.output)
        self.behave()
        macos_capture.capture_region((0, 0, 8, 8), self.output)
        first, second = (pid for pid, _, _ in self.requests())
        self.assertNotEqual(first, second)

    def test_an_old_helper_is_retired_between_frames(self) -> None:
        self.behave()
        macos_capture.capture_region((0, 0, 8, 8), self.output)
        with patch.object(macos_capture, "SERVER_MAX_AGE_SECONDS", 0.0):
            macos_capture.capture_region((0, 0, 8, 8), self.output)
        first, second = (pid for pid, _, _ in self.requests())
        self.assertNotEqual(first, second)

    def test_stopping_the_servers_ends_the_helpers(self) -> None:
        self.behave()
        macos_capture.capture_region((0, 0, 8, 8), self.output)
        process = macos_capture._SERVERS[False].process
        macos_capture.stop_capture_servers()
        self.assertIsNotNone(process.poll())

    def test_a_newline_in_the_output_path_is_refused(self) -> None:
        with self.assertRaises(macos_capture.CaptureUnavailable):
            macos_capture.capture_region((0, 0, 8, 8), self.root / "a\nb.png")


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
