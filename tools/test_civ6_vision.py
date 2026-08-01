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
