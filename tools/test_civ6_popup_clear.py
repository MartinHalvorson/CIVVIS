#!/usr/bin/env python3
"""Pixel tests for the no-stray-click popup classifier."""

from __future__ import annotations

import sys
import os
import tempfile
import time
import unittest
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import popup_clear  # noqa: E402


class PopupClearTest(unittest.TestCase):
    def test_advisor_card_uses_its_leftmost_continue_button(self) -> None:
        image = Image.new("RGB", (1000, 600), (99, 95, 62))
        draw = ImageDraw.Draw(image)
        draw.rectangle((330, 40, 670, 245), fill=(225, 221, 202))
        # Rasterization makes same-row controls differ slightly in centroid y.
        # Ordering must still choose the left acknowledge action, not the
        # right-side "Tell me more" action.
        draw.rectangle((405, 181, 505, 206), fill=(32, 86, 148))
        draw.rectangle((525, 180, 625, 205), fill=(32, 86, 148))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual(kind, "advisor")
        self.assertEqual(len(targets), 2)
        self.assertLess(targets[0][0], targets[1][0])
        self.assertEqual(popup_clear.click_target(kind, targets, image.width), targets[0])

    def test_dark_world_congress_advisor_beats_generic_leader_detection(self) -> None:
        # The World Congress introduction leaves a broadly dark panel behind a
        # standard advisor card. Its actual Continue control is safe to press,
        # but checking darkness first used to misclassify it as a leader scene
        # and leave the live game blocked.
        image = Image.new("RGB", (1000, 600), (12, 12, 12))
        draw = ImageDraw.Draw(image)
        draw.rectangle((330, 40, 670, 245), fill=(225, 221, 202))
        draw.rectangle((405, 181, 505, 206), fill=(32, 86, 148))

        kind, targets, dark = popup_clear.classify(image)

        self.assertGreater(dark, popup_clear.LEADER_DARK_FRACTION)
        self.assertEqual(kind, "advisor")
        self.assertEqual(len(targets), 1)
        self.assertEqual(popup_clear.click_target(kind, targets, image.width), targets[0])

    def test_advisor_right_side_only_action_is_never_clicked(self) -> None:
        image = Image.new("RGB", (1000, 600), (99, 95, 62))
        draw = ImageDraw.Draw(image)
        draw.rectangle((330, 40, 670, 245), fill=(225, 221, 202))
        # This represents a Tell me more-style action without a recognized
        # Continue action.  The watchdog must wait rather than opening help.
        draw.rectangle((525, 180, 625, 205), fill=(32, 86, 148))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual(kind, "advisor")
        self.assertIsNone(popup_clear.click_target(kind, targets, image.width))

    def test_paired_left_continue_survives_a_decorated_paper_probe(self) -> None:
        image = Image.new("RGB", (1000, 600), (99, 95, 62))
        draw = ImageDraw.Draw(image)
        draw.rectangle((330, 40, 670, 245), fill=(225, 221, 202))
        # Artwork covers the tight probe above the left action, but its
        # right-hand companion is visibly inside the same advisor card.
        draw.rectangle((355, 75, 555, 181), fill=(99, 95, 62))
        draw.rectangle((405, 181, 505, 206), fill=(32, 86, 148))
        draw.rectangle((525, 181, 625, 206), fill=(32, 86, 148))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual(kind, "advisor")
        self.assertEqual(len(targets), 2)
        self.assertLess(targets[0][0], targets[1][0])
        self.assertEqual(popup_clear.click_target(kind, targets, image.width), targets[0])

    def test_advisor_card_with_a_decorated_paper_probe_is_not_missed(self) -> None:
        image = Image.new("RGB", (1000, 600), (99, 95, 62))
        draw = ImageDraw.Draw(image)
        # This fills 34.4% of the tight paper probe above the action. It mirrors
        # the live Tribal Village card, where artwork and decoration lower the
        # bright-pixel fraction below the previous 36% cutoff.
        draw.rectangle((355, 148, 555, 180), fill=(225, 221, 202))
        draw.rectangle((405, 181, 505, 206), fill=(32, 86, 148))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual(kind, "advisor")
        self.assertEqual(len(targets), 1)

    def test_lower_advisor_card_action_is_not_misclassified_as_map(self) -> None:
        image = Image.new("RGB", (1000, 600), (99, 95, 62))
        draw = ImageDraw.Draw(image)
        # The live new-city loyalty card puts its controls at 43% of the game
        # window height, below the original 40% upper bound.
        draw.rectangle((330, 80, 670, 350), fill=(225, 221, 202))
        draw.rectangle((405, 250, 505, 275), fill=(32, 86, 148))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual(kind, "advisor")
        self.assertEqual(len(targets), 1)

    def test_blue_map_pins_are_not_an_advisor(self) -> None:
        image = Image.new("RGB", (1000, 600), (99, 95, 62))
        draw = ImageDraw.Draw(image)
        draw.ellipse((450, 180, 485, 215), fill=(32, 86, 148))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual((kind, targets), ("map", []))

    def test_advisor_wins_over_a_red_map_marker(self) -> None:
        image = Image.new("RGB", (1000, 600), (99, 95, 62))
        draw = ImageDraw.Draw(image)
        draw.rectangle((330, 40, 670, 245), fill=(225, 221, 202))
        draw.rectangle((405, 180, 505, 205), fill=(32, 86, 148))
        # Meets the completion-card red-cluster geometry, but is only a map pin.
        draw.rectangle((590, 240, 612, 262), fill=(180, 20, 20))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual(kind, "advisor")
        self.assertEqual(len(targets), 1)

    def test_blue_hud_beside_an_advisor_card_is_not_a_second_action(self) -> None:
        image = Image.new("RGB", (1000, 600), (99, 95, 62))
        draw = ImageDraw.Draw(image)
        draw.rectangle((430, 40, 770, 245), fill=(225, 221, 202))
        draw.rectangle((550, 180, 650, 205), fill=(32, 86, 148))
        # The card's paper spills into the old wide probe for this HUD control,
        # but the control itself is not enclosed by the card.
        draw.rectangle((350, 216, 450, 241), fill=(32, 86, 148))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual(kind, "advisor")
        self.assertEqual(len(targets), 1)
        self.assertGreater(targets[0][0], 500)

    def test_world_congress_review_uses_only_its_return_button(self) -> None:
        image = Image.new("RGB", (1000, 600), (15, 15, 14))
        draw = ImageDraw.Draw(image)
        draw.rectangle((500, 558, 630, 578), fill=(45, 125, 80))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual(kind, "congress")
        self.assertEqual(len(targets), 1)
        self.assertGreater(targets[0][1], 550)

    def test_green_bottom_hud_control_is_not_a_world_congress_screen(self) -> None:
        image = Image.new("RGB", (1000, 600), (99, 95, 62))
        draw = ImageDraw.Draw(image)
        draw.rectangle((500, 558, 630, 578), fill=(45, 125, 80))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual((kind, targets), ("map", []))

    def test_governor_panel_has_a_dedicated_close_target(self) -> None:
        image = Image.new("RGB", (1000, 600), (95, 90, 59))
        draw = ImageDraw.Draw(image)
        draw.rectangle((0, 125, 999, 155), fill=(19, 44, 71))
        draw.ellipse((968, 128, 992, 152), fill=(180, 20, 20))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual(kind, "governor")
        self.assertEqual(len(targets), 1)

    def test_right_edge_red_marker_without_governor_header_is_not_clicked(self) -> None:
        image = Image.new("RGB", (1000, 600), (95, 90, 59))
        draw = ImageDraw.Draw(image)
        draw.ellipse((968, 128, 992, 152), fill=(180, 20, 20))

        kind, targets, _ = popup_clear.classify(image)

        self.assertEqual((kind, targets), ("map", []))

    def test_stalled_turn_is_allowed_only_with_explicit_long_grace(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            run = Path(root) / "active"
            run.mkdir()
            events = run / "events.jsonl"
            events.write_text('{"kind":"turn","turn":4}\n')
            stale = time.time() - 300
            os.utime(events, (stale, stale))

            self.assertFalse(popup_clear.game_in_progress(root, fresh_seconds=180))
            self.assertTrue(popup_clear.game_in_progress(root, fresh_seconds=600))
