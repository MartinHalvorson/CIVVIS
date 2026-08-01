#!/usr/bin/env python3
"""Unit checks for the no-package-manager macOS input fallback."""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import call, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import macos_input  # noqa: E402


def completed(arguments, **_kwargs):
    return subprocess.CompletedProcess(arguments, 0, "", "")


class MacOSInputTest(unittest.TestCase):
    def test_held_click_keeps_cliclick_behavior_when_installed(self) -> None:
        with patch.object(macos_input.shutil, "which", return_value="/opt/bin/cliclick"), \
             patch.object(macos_input.subprocess, "run", side_effect=completed) as run, \
             patch.object(macos_input.time, "sleep") as sleep:
            macos_input.click(41, 73, hold_s=0.125, check=True)

        self.assertEqual(
            run.call_args_list,
            [
                call(
                    ["/opt/bin/cliclick", "dd:41,73"],
                    capture_output=True, text=True, check=True, timeout=20,
                ),
                call(
                    ["/opt/bin/cliclick", "du:41,73"],
                    capture_output=True, text=True, check=True, timeout=20,
                ),
            ],
        )
        sleep.assert_called_once_with(0.125)

    def test_native_backend_receives_the_held_click_as_one_event(self) -> None:
        with patch.object(macos_input.shutil, "which", return_value=None), \
             patch.object(macos_input, "_native_binary", return_value=Path("/tmp/cginput")), \
             patch.object(macos_input.subprocess, "run", side_effect=completed) as run:
            macos_input.click(41, 73, hold_s=0.125, check=True)

        run.assert_called_once_with(
            ["/tmp/cginput", "click", "41", "73", "125"],
            capture_output=True, text=True, check=True, timeout=20,
        )

    def test_escape_uses_the_native_virtual_keycode_without_cliclick(self) -> None:
        with patch.object(macos_input.shutil, "which", return_value=None), \
             patch.object(macos_input, "_native_binary", return_value=Path("/tmp/cginput")), \
             patch.object(macos_input.subprocess, "run", side_effect=completed) as run:
            macos_input.press_key("escape")

        run.assert_called_once_with(
            ["/tmp/cginput", "key", "53"],
            capture_output=True, text=True, check=False, timeout=20,
        )

    def test_native_scroll_preserves_wheel_direction(self) -> None:
        with patch.object(macos_input.shutil, "which", return_value=None), \
             patch.object(macos_input, "_native_binary", return_value=Path("/tmp/cginput")), \
             patch.object(macos_input.subprocess, "run", side_effect=completed) as run:
            macos_input.scroll(-12, check=True)

        run.assert_called_once_with(
            ["/tmp/cginput", "scroll", "-12"],
            capture_output=True, text=True, check=True, timeout=20,
        )
