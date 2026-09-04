#!/usr/bin/env python3
"""The recorded Play Now helper must remain explicitly capture-free."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_capture_free_setup as setup


class CaptureFreeSetupTests(unittest.TestCase):
    def test_source_never_imports_or_calls_visual_readers(self):
        source = Path(setup.__file__).read_text(encoding="utf-8")
        self.assertNotIn("macos_capture", source)
        self.assertNotIn("macos_ocr", source)
        self.assertNotIn("vision.", source)
        self.assertNotIn("screenshot(", source)

    def test_bootstrap_clicks_the_known_play_now_controls(self):
        clicks: list[tuple[int, int]] = []
        desktop = (1728, 1117)
        with mock.patch.object(setup.time, "sleep"), \
             mock.patch.object(setup.macos_window, "place_game"), \
             mock.patch.object(setup.macos_window, "focus_game"), \
             mock.patch.object(setup.macos_window, "desktop_size",
                               return_value=desktop), \
             mock.patch.object(setup.macos_input, "move"), \
             mock.patch.object(setup.macos_input, "click",
                               side_effect=lambda x, y, **_: clicks.append((x, y))):
            setup.start_bootstrap_game()
            setup.begin_bootstrap_game()

        width, height = desktop
        point = lambda fraction: (int(width * fraction[0]),
                                  int(height * fraction[1]))
        # The first pointer event can be consumed while macOS keys the window;
        # it must be spent on empty artwork before a menu row is targeted.
        self.assertEqual(clicks[:4], [point(setup.MENU_ACTIVATION_FRACTION),
                                      point(setup.SINGLE_PLAYER_FRACTION),
                                      point(setup.PLAY_NOW_FRACTION),
                                      point(setup.BEGIN_GAME_FRACTION)])

    def test_desktop_fraction_refuses_an_unknown_desktop(self):
        with mock.patch.object(setup.macos_window, "desktop_size",
                               return_value=None):
            with self.assertRaisesRegex(RuntimeError, "desktop size is unavailable"):
                setup.click_desktop_fraction(0.5, 0.5)

    def test_bootstrap_wait_uses_the_agent_log_not_the_desktop(self):
        with mock.patch.object(setup.time, "monotonic", side_effect=[0.0, 0.0]), \
             mock.patch.object(setup.env, "game_pids", return_value=[123]), \
             mock.patch.object(Path, "read_text", return_value='CIVVISJSON {"kind":"loaded"}'):
            self.assertTrue(setup.wait_for_agent_loaded(timeout_s=1.0))


if __name__ == "__main__":
    unittest.main()
