"""Visible checkbox borders, not nearby artwork, determine game modes."""

from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

try:
    from PIL import Image, ImageDraw
except ImportError:
    Image = None

import civ6_play
from civ6_control.game_mode_boxes import checkbox_center


class PixelImage:
    def __init__(self, width, height):
        self.width = width
        self.height = height
        self.pixels = {}

    def getpixel(self, position):
        return self.pixels.get(position, (12, 20, 25))

    def border(self, left, top, size, thickness=2, missing_left=False):
        for horizontal in range(left, left + size + 1):
            for vertical in range(top, top + size + 1):
                if (vertical < top + thickness or vertical > top + size - thickness
                        or horizontal > left + size - thickness
                        or (not missing_left and horizontal < left + thickness)):
                    self.pixels[horizontal, vertical] = (160, 125, 50)


class CheckboxCenterTests(unittest.TestCase):
    def test_finds_a_displaced_outline_at_different_scales(self):
        for scale in [0.5, 1.0, 2.0]:
            with self.subTest(scale=scale):
                image = PixelImage(200, 200)
                size = round(16 * scale)
                image.border(60, 60, size, max(1, round(2 * scale)))
                found = checkbox_center(image, (60 + size, 60), scale)
                self.assertIsNotNone(found)
                self.assertAlmostEqual(found[0], 60 + size / 2, delta=1)
                self.assertAlmostEqual(found[1], 60 + size / 2, delta=1)

    def test_missing_or_partial_border_is_not_a_checkbox(self):
        image = PixelImage(100, 100)
        self.assertIsNone(checkbox_center(image, (50, 50), 1.0))
        image.border(40, 40, 16, missing_left=True)
        self.assertIsNone(checkbox_center(image, (48, 48), 1.0))


@unittest.skipUnless(Image, "requires Pillow")
class GameModeBoxesTests(unittest.TestCase):
    def test_live_scaled_menu_reads_the_eight_actual_centers(self):
        fixture = Path(__file__).parent / "fixtures/game_modes_scaled.png"
        image = Image.new("RGB", (3456, 2234), (12, 20, 25))
        with Image.open(fixture) as crop:
            image.paste(crop, (740, 785))
        with tempfile.TemporaryDirectory() as raw:
            capture = Path(raw) / "game-modes.png"
            image.save(capture)
            with patch.object(civ6_play, "desktop_size", return_value=(1728, 1117)), \
                 patch.object(civ6_play, "_observed_label_point", return_value=(452, 384)):
                boxes = civ6_play._game_mode_boxes(capture, (0, 33, 905, 613))
        self.assertIsNotNone(boxes)
        self.assertEqual(len(boxes), 8)
        self.assertEqual([box[2] for box in boxes], [False] * 8)
        expected_centers = [(783, 832), (850, 832), (917, 832), (984, 832),
                            (1051, 832), (783, 899), (850, 899), (917, 899)]
        for actual, expected in zip(boxes, expected_centers):
            self.assertAlmostEqual(actual[0] * 2, expected[0], delta=2)
            self.assertAlmostEqual(actual[1] * 2, expected[1], delta=2)

    def test_a_ticked_box_is_read_from_its_interior(self):
        image = Image.new("RGB", (100, 100), (12, 20, 25))
        draw = ImageDraw.Draw(image)
        draw.rectangle((40, 40, 56, 56), outline=(160, 125, 50), width=2)
        draw.line((44, 48, 47, 52, 53, 44), fill=(200, 190, 150), width=3)
        found = checkbox_center(image, (54, 43), 1.0)
        self.assertIsNotNone(found)
        self.assertAlmostEqual(found[0], 48, delta=1)
        self.assertAlmostEqual(found[1], 48, delta=1)

    def test_live_menu_reports_a_real_tick_without_toggling_empty_boxes(self):
        fixture = Path(__file__).parent / "fixtures/game_modes_scaled.png"
        image = Image.new("RGB", (3456, 2234), (12, 20, 25))
        with Image.open(fixture) as crop:
            image.paste(crop, (740, 785))
        ImageDraw.Draw(image).line((779, 831, 783, 835, 788, 827),
                                  fill=(200, 190, 150), width=3)
        with tempfile.TemporaryDirectory() as raw:
            capture = Path(raw) / "game-modes.png"
            image.save(capture)
            with patch.object(civ6_play, "desktop_size", return_value=(1728, 1117)), \
                 patch.object(civ6_play, "_observed_label_point", return_value=(452, 384)):
                boxes = civ6_play._game_mode_boxes(capture, (0, 33, 905, 613))
        self.assertIsNotNone(boxes)
        self.assertEqual([box[2] for box in boxes], [True] + [False] * 7)

    def test_missing_borders_refuse_the_entire_row(self):
        with tempfile.TemporaryDirectory() as raw:
            capture = Path(raw) / "blank.png"
            Image.new("RGB", (3456, 2234), (12, 20, 25)).save(capture)
            with patch.object(civ6_play, "desktop_size", return_value=(1728, 1117)), \
                 patch.object(civ6_play, "_observed_label_point", return_value=(452, 384)):
                self.assertIsNone(civ6_play._game_mode_boxes(capture, (0, 33, 905, 613)))


if __name__ == "__main__":
    unittest.main()
