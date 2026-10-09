#!/usr/bin/env python3
"""Tests for the native screenshot OCR wrapper."""
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from civ6_control import macos_ocr  # noqa: E402


class RecognizeTimeoutTest(unittest.TestCase):
    def test_a_timed_out_vision_pass_is_ocr_unavailable(self):
        expired = subprocess.TimeoutExpired(cmd=["vision-ocr"], timeout=45)
        with patch.object(macos_ocr, "_native_binary", return_value=Path("/bin/false")), \
             patch.object(macos_ocr.subprocess, "run", side_effect=expired):
            with self.assertRaises(macos_ocr.OCRUnavailable) as raised:
                macos_ocr.recognize(Path("/tmp/shot.png"))
        self.assertIn("timed out", str(raised.exception))


if __name__ == "__main__":
    unittest.main()
