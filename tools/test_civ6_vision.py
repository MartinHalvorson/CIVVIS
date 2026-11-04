#!/usr/bin/env python3
"""Regression checks for the screenshot-only main-menu locator."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import vision  # noqa: E402


@unittest.skipUnless(vision.available(), "Pillow is required for vision tests")
class MenuVisionTest(unittest.TestCase):
    def test_two_pixel_menu_rule_is_not_an_extra_row(self) -> None:
        from PIL import Image

        image = Image.new("L", (40, 300), 50)
        # Five text bands at evenly spaced menu positions and one two-pixel frame
        # rule before the final entry, exactly the live menu's failure shape.
        for top in (20, 70, 120, 170, 250):
            for y in range(top, top + 10):
                for x in range(image.width):
                    image.putpixel((x, y), 150)
        for y in range(230, 232):
            for x in range(image.width):
                image.putpixel((x, y), 100)

        with tempfile.TemporaryDirectory() as directory:
            shot = Path(directory) / "menu.png"
            image.save(shot)
            rows = vision.rows_in(
                shot, (0, 0, image.width, image.height), (0.0, 0.0, 1.0, 1.0),
                scale=1.0,
            )

        self.assertEqual(len(rows), 5)
        self.assertAlmostEqual(rows[-3], 0.417, places=3)

    def test_transition_text_bands_are_not_accepted_as_a_submenu(self) -> None:
        # The title-card screenshot that caused the live misclick had many bright
        # bands but no three consecutive Civ VI menu rows.
        self.assertEqual(
            vision._regular_menu_rows(
                [0.4979, 0.5113, 0.5248, 0.5390, 0.5527, 0.5664, 0.6620],
            ),
            [],
        )

    def test_regular_rows_remain_actionable(self) -> None:
        rows = [0.5137, 0.5445, 0.5751, 0.6056, 0.6365]
        self.assertEqual(
            vision._regular_menu_rows(rows),
            rows,
        )


def _menu_window_image(background: int, band: int):
    """A 200x300 game window with five evenly spaced bright bands in the menu column."""
    from PIL import Image

    image = Image.new("L", (200, 300), background)
    left = int(200 * vision.MENU_COLUMN[0])
    right = int(200 * vision.MENU_COLUMN[2])
    for top in (130, 140, 150, 160, 170):
        for y in range(top, top + 4):
            for x in range(left, right):
                image.putpixel((x, y), band)
    return image


@unittest.skipUnless(vision.available(), "Pillow is required for vision tests")
class LoadingCardTest(unittest.TestCase):
    """The boot loading card's stripes are not the top-level menu."""

    # Mean window luminance measured on the live 864x542-point window over the
    # 116 top-menu clicks of 2026-11-03 05:10Z .. 11-04 06:02Z: every row-fallback
    # click landed on a loading card, every label read on a real menu.
    LOADING_CARDS = (5.8, 19.4)
    REAL_MENUS = (58.6, 71.7)

    def _rows(self, image):
        with tempfile.TemporaryDirectory() as directory:
            shot = Path(directory) / "menu-attempt1.png"
            image.save(shot)
            bounds = (0, 0, image.width, image.height)
            return (vision.menu_rows(shot, bounds, scale=1.0),
                    vision.rows_in(shot, bounds, vision.MENU_COLUMN, scale=1.0),
                    vision.window_luma(shot, bounds, scale=1.0))

    def test_dark_loading_card_reads_no_menu_rows(self) -> None:
        rows, unguarded, luma = self._rows(_menu_window_image(background=4, band=230))
        self.assertGreaterEqual(len(unguarded), 4)  # what the fallback used to click
        self.assertLess(luma, vision.DARK_WINDOW_LUMA)
        self.assertEqual(rows, [])

    def test_lit_menu_rows_are_unchanged(self) -> None:
        rows, unguarded, luma = self._rows(_menu_window_image(background=60, band=200))
        self.assertGreaterEqual(luma, vision.DARK_WINDOW_LUMA)
        self.assertEqual(len(rows), 5)
        self.assertEqual(rows, unguarded)

    def test_cut_sits_between_measured_cards_and_menus(self) -> None:
        self.assertLess(self.LOADING_CARDS[1], vision.DARK_WINDOW_LUMA)
        self.assertLess(vision.DARK_WINDOW_LUMA, self.REAL_MENUS[0])

    def test_missing_screenshot_has_no_luma(self) -> None:
        self.assertIsNone(vision.window_luma(Path("/nonexistent/menu.png"), (0, 0, 10, 10)))
