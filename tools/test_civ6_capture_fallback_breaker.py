"""The CoreGraphics fallback breaker: stop paying 7.5 s for a backend that hangs."""
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import macos_capture


def completed(returncode: int, stderr: str = "") -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess(args=["cgcapture"], returncode=returncode,
                                       stdout="", stderr=stderr)


class FallbackBreakerTest(unittest.TestCase):
    def setUp(self) -> None:
        macos_capture.reset_fallback_breaker()
        self.addCleanup(macos_capture.reset_fallback_breaker)
        self.now = 1000.0
        patch = mock.patch.object(macos_capture.time, "monotonic",
                                  side_effect=lambda: self.now)
        patch.start()
        self.addCleanup(patch.stop)
        binary = mock.patch.object(macos_capture, "_native_binary",
                                   return_value=Path("/tmp/cgcapture"))
        binary.start()
        self.addCleanup(binary.stop)

    def run_capture(self, backends, output: Path):
        """`backends` is one result per `_capture_once` call; None means timeout."""
        calls = []

        def fake_once(command):
            calls.append("fallback" if "--fallback" in command else "native")
            return backends[len(calls) - 1]

        with mock.patch.object(macos_capture, "_capture_once", side_effect=fake_once):
            error = None
            try:
                macos_capture.capture_region((0, 0, 10, 10), output)
            except macos_capture.CaptureUnavailable as raised:
                error = raised
        return calls, error

    def test_first_fallback_timeout_is_still_attempted(self):
        """One kill under momentary load is the ordinary transient; retry it."""
        calls, error = self.run_capture(
            [completed(macos_capture.SCREEN_CAPTURE_FALLBACK_NEEDED), None],
            Path("/tmp/never-written.png"))
        self.assertEqual(calls, ["native", "fallback"])
        self.assertIn("timed out", str(error))

    def test_the_breaker_opens_after_two_consecutive_timeouts(self):
        """Measured 2026-09-02: the fallback did not return in 20 s, repeatedly."""
        shot = Path("/tmp/never-written.png")
        needed = macos_capture.SCREEN_CAPTURE_FALLBACK_NEEDED
        self.run_capture([completed(needed), None], shot)
        self.run_capture([completed(needed), None], shot)
        calls, error = self.run_capture([completed(needed), None], shot)
        self.assertEqual(calls, ["native"], "the fallback must not be spawned again")
        self.assertIn("skipping it", str(error))

    def test_the_breaker_closes_again_after_its_interval(self):
        """A picker closes, a grant changes, a spin ends. Never permanent."""
        shot = Path("/tmp/never-written.png")
        needed = macos_capture.SCREEN_CAPTURE_FALLBACK_NEEDED
        self.run_capture([completed(needed), None], shot)
        self.run_capture([completed(needed), None], shot)
        calls, _ = self.run_capture([completed(needed), None], shot)
        self.assertEqual(calls, ["native"])
        self.now += macos_capture.FALLBACK_BREAKER_SECONDS + 1.0
        calls, _ = self.run_capture([completed(needed), None], shot)
        self.assertEqual(calls, ["native", "fallback"])

    def test_a_working_fallback_clears_the_streak(self):
        """Alternating one timeout with one success must never open the breaker."""
        shot = Path(__file__)  # any existing non-empty file satisfies the size check
        needed = macos_capture.SCREEN_CAPTURE_FALLBACK_NEEDED
        for _ in range(5):
            self.run_capture([completed(needed), None], Path("/tmp/never-written.png"))
            calls, error = self.run_capture([completed(needed), completed(0)], shot)
            self.assertEqual(calls, ["native", "fallback"])
            self.assertIsNone(error)

    def test_a_healthy_native_capture_never_reaches_the_fallback(self):
        calls, error = self.run_capture([completed(0)], Path(__file__))
        self.assertEqual(calls, ["native"])
        self.assertIsNone(error)

    def test_a_permission_denial_is_not_charged_to_the_fallback(self):
        """`CapturePermissionUnavailable` has its own handling upstream and must
        keep its type through the breaker."""
        calls, error = self.run_capture(
            [completed(macos_capture.SCREEN_CAPTURE_PERMISSION_DENIED, "denied")],
            Path("/tmp/never-written.png"))
        self.assertEqual(calls, ["native"])
        self.assertIsInstance(error, macos_capture.CapturePermissionUnavailable)

    def test_a_native_timeout_does_not_open_the_fallback_breaker(self):
        """The breaker is about the fallback backend only."""
        shot = Path("/tmp/never-written.png")
        for _ in range(4):
            calls, _ = self.run_capture([None], shot)
            self.assertEqual(calls, ["native"])
        needed = macos_capture.SCREEN_CAPTURE_FALLBACK_NEEDED
        calls, _ = self.run_capture([completed(needed), None], shot)
        self.assertEqual(calls, ["native", "fallback"])

    def test_the_breaker_interval_is_shorter_than_the_wedge_watchdog(self):
        self.assertLess(macos_capture.FALLBACK_BREAKER_SECONDS, 300.0)


if __name__ == "__main__":
    unittest.main()
