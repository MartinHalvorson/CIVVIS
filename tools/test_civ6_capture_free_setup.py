#!/usr/bin/env python3
"""The recorded Create Game helper must remain explicitly capture-free."""

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

    def test_direct_profile_clicks_the_known_create_game_controls(self):
        clicks: list[tuple[int, int]] = []
        with mock.patch.object(setup.time, "sleep"), \
             mock.patch.object(setup.macos_window, "place_game"), \
             mock.patch.object(setup.macos_window, "focus_game"), \
             mock.patch.object(setup.macos_window, "game_window",
                               return_value=(100, 200, 864, 542)), \
             mock.patch.object(setup.macos_input, "move"), \
             mock.patch.object(setup.macos_input, "click",
                               side_effect=lambda x, y, **_: clicks.append((x, y))), \
             mock.patch.object(setup.macos_input, "press_key") as press:
            setup.start_direct_game()

        self.assertIn(setup.SINGLE_PLAYER_POINT, clicks)
        self.assertIn(setup.CREATE_GAME_POINT, clicks)
        # The first pointer event can be consumed while macOS keys the window;
        # it must be spent on empty artwork before a menu row is targeted.
        activation = (round(100 + 864 * setup.MENU_ACTIVATION_FRACTION[0]),
                      round(200 + 542 * setup.MENU_ACTIVATION_FRACTION[1]))
        self.assertEqual(clicks[:3], [activation, setup.SINGLE_PLAYER_POINT,
                                      setup.CREATE_GAME_POINT])
        # Restore Defaults, Emperor, Online, and Start Game are all relative
        # to the prepared fixed-size game window.
        self.assertIn((230, 222), clicks)
        self.assertIn((532, 371), clicks)
        self.assertIn((532, 477), clicks)
        self.assertIn((532, 442), clicks)
        self.assertIn((532, 730), clicks)
        press.assert_called_once_with("return", check=True)

    def test_start_only_does_not_reopen_the_main_menu(self):
        with mock.patch.object(setup.time, "sleep"), \
             mock.patch.object(setup.macos_window, "place_game"), \
             mock.patch.object(setup.macos_window, "focus_game"), \
             mock.patch.object(setup.macos_window, "game_window",
                               return_value=(0, 0, 864, 542)), \
             mock.patch.object(setup.macos_input, "move"), \
             mock.patch.object(setup.macos_input, "click") as click, \
             mock.patch.object(setup.macos_input, "press_key"):
            setup.start_direct_game(start_only=True,
                                    restore_defaults=False,
                                    emperor_online=False)
        click.assert_called_once()


if __name__ == "__main__":
    unittest.main()
