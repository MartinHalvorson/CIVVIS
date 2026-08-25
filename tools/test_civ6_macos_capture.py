#!/usr/bin/env python3
"""Checks for the fast macOS screenshot helper used by popup_clear."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import macos_capture  # noqa: E402


def completed(arguments, **_kwargs):
    Path(arguments[-1]).write_bytes(b"png")
    return subprocess.CompletedProcess(arguments, 0, "", "")


class MacOSCaptureTest(unittest.TestCase):
    def test_helper_uses_the_fast_coregraphics_symbol_without_sdk_call(self) -> None:
        source = macos_capture._SWIFT_SOURCE
        self.assertIn('dlsym(framework, "CGWindowListCreateImage")', source)
        self.assertNotIn("CGWindowListCreateImage(\n", source)

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
            timeout=5,
        )


if __name__ == "__main__":
    unittest.main()
