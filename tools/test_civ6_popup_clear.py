#!/usr/bin/env python3
"""Pixel-safety regressions for the Civilization VI popup backstop."""

from __future__ import annotations

import os
import sys
import tempfile
import time
import unittest
from unittest import mock
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:  # pragma: no cover - depends on the host, not the code
    Image = ImageDraw = None

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import popup_clear  # noqa: E402


class PixelSamplingCompatibilityTest(unittest.TestCase):
    def test_the_current_pillow_pixel_api_is_preferred(self) -> None:
        class CurrentImage:
            def get_flattened_data(self):
                return (1, 2, 3)

            def getdata(self):
                raise AssertionError("deprecated getdata should not be used")

        self.assertEqual(list(popup_clear.pixel_values(CurrentImage())), [1, 2, 3])

    def test_old_pillow_images_keep_the_backstop_working(self) -> None:
        class LegacyImage:
            def getdata(self):
                return (4, 5, 6)

        self.assertEqual(list(popup_clear.pixel_values(LegacyImage())), [4, 5, 6])


class PopupTimingTest(unittest.TestCase):
    def test_covered_dialogue_poll_is_bounded_to_a_quarter_second(self) -> None:
        self.assertEqual(popup_clear.covered_poll_delay(10.0), 0.25)
        self.assertEqual(popup_clear.covered_poll_delay(0.1), 0.1)
        self.assertEqual(popup_clear.covered_poll_delay(0.0), 0.05)

    def test_click_settle_constants_leave_the_backstop_subsecond(self) -> None:
        self.assertLessEqual(popup_clear.DIALOGUE_POLL_SECONDS, 0.25)
        self.assertLessEqual(popup_clear.POINTER_SETTLE_SECONDS, 0.1)
        self.assertLessEqual(popup_clear.POST_CLICK_SETTLE_SECONDS, 0.25)


class NativeRecordingProtectionTest(unittest.TestCase):
    def test_native_recording_commands_are_distinguished_from_png_fallbacks(self) -> None:
        self.assertTrue(popup_clear.native_recording_command(
            "/usr/sbin/screencapture -pdiU -z keyboard.interactive"))
        self.assertTrue(popup_clear.native_recording_command(
            "/usr/sbin/screencapture -v -V 5 /tmp/recording.mov"))
        self.assertFalse(popup_clear.native_recording_command(
            "screencapture -x -t png -R0,0,100,100 /tmp/frame.png"))
        self.assertFalse(popup_clear.native_recording_command(
            "/usr/bin/python3 tools/civ6_control/popup_clear.py"))

    def test_native_recording_ui_always_yields_to_the_user_capture(self) -> None:
        with mock.patch.object(popup_clear, "native_recording_ui_active", return_value=True), \
                mock.patch.object(popup_clear, "NATIVE_CAPTURE_DISABLED", False), \
                mock.patch.object(popup_clear, "systemstatusd_cpu") as daemon_cpu:
            self.assertEqual(
                popup_clear.capture_pause_reason(),
                "native screen recording UI is active",
            )
        daemon_cpu.assert_not_called()

    def test_native_recording_ui_yields_even_when_coregraphics_is_available(self) -> None:
        with mock.patch.object(popup_clear, "native_recording_ui_active", return_value=True), \
                mock.patch.object(popup_clear, "NATIVE_CAPTURE_DISABLED", False), \
                mock.patch.object(popup_clear, "systemstatusd_cpu", return_value=12.0):
            self.assertEqual(
                popup_clear.capture_pause_reason(),
                "native screen recording UI is active",
            )

    def test_a_stale_interactive_helper_does_not_hold_the_ladder_forever(self) -> None:
        stale = "/usr/sbin/screencapture -pdiU -z keyboard.interactive"
        with mock.patch.object(popup_clear.subprocess, "run",
                               return_value=mock.Mock(stdout=stale, returncode=0)):
            self.assertFalse(popup_clear.native_recording_ui_active())

    def test_the_visible_cmd_shift_5_ui_is_a_user_capture(self) -> None:
        commands = "\n".join([
            "/usr/sbin/screencapture -pdiU -z keyboard.interactive",
            "/System/Library/CoreServices/screencaptureui.app/Contents/MacOS/screencaptureui",
        ])
        with mock.patch.object(popup_clear.subprocess, "run",
                               return_value=mock.Mock(stdout=commands, returncode=0)):
            self.assertTrue(popup_clear.native_recording_ui_active())

    def test_capture_permission_is_rechecked_without_a_raw_fallback(self) -> None:
        with mock.patch.object(popup_clear, "native_recording_ui_active", return_value=False), \
                mock.patch.object(popup_clear, "NATIVE_CAPTURE_DISABLED", True), \
                mock.patch.object(popup_clear.macos_capture,
                                  "screen_capture_access_available", return_value=False), \
                mock.patch.object(popup_clear, "systemstatusd_cpu") as daemon_cpu:
            self.assertEqual(
                popup_clear.capture_pause_reason(),
                "screen capture access is unavailable",
            )
        daemon_cpu.assert_not_called()

    def test_permission_denial_never_falls_back_to_screencapture(self) -> None:
        with mock.patch.object(popup_clear, "NATIVE_CAPTURE_DISABLED", False), \
                mock.patch.object(popup_clear, "_image_library", return_value=mock.Mock()), \
                mock.patch.object(popup_clear.macos_capture, "capture_region",
                                  side_effect=popup_clear.macos_capture.CapturePermissionUnavailable(
                                      "denied")), \
                mock.patch.object(popup_clear.subprocess, "run") as run:
            with self.assertRaises(popup_clear.macos_capture.CapturePermissionUnavailable):
                popup_clear.capture((0, 0, 864, 542))
        run.assert_not_called()

    def test_status_daemon_spin_pauses_pixel_capture(self) -> None:
        with mock.patch.object(popup_clear, "native_recording_ui_active", return_value=False), \
                mock.patch.object(popup_clear, "systemstatusd_cpu", return_value=85.4):
            self.assertEqual(
                popup_clear.capture_pause_reason(),
                "systemstatusd is spinning at 85% CPU",
            )

    def test_ordinary_capture_activity_does_not_pause(self) -> None:
        with mock.patch.object(popup_clear, "native_recording_ui_active", return_value=False), \
                mock.patch.object(popup_clear, "systemstatusd_cpu", return_value=12.0):
            self.assertIsNone(popup_clear.capture_pause_reason())


# Unlike the module under test, these checks really do need Pillow: every one
# of them paints a synthetic Civilization VI screen and asserts on what the
# classifier makes of the pixels. A host without it skips them by name rather
# than failing to collect the file, which is what used to happen.
@unittest.skipUnless(Image is not None, "Pillow is not installed on this host")
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
        # Continue action. The watchdog must wait rather than opening help.
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

    def test_centered_one_button_notice_requires_its_modal_frame(self) -> None:
        image = Image.new("RGB", (1000, 600), (70, 85, 65))
        draw = ImageDraw.Draw(image)
        draw.rectangle((400, 250, 600, 320), fill=(210, 210, 190))
        draw.rectangle((350, 235, 650, 320), outline=(15, 80, 150), width=8)
        draw.rectangle((450, 335, 550, 355), outline=(15, 80, 150), width=8)

        kind, targets, _dark = popup_clear.classify(image)
        self.assertEqual(kind, "notice")
        self.assertEqual(len(targets), 1)

        # A similarly placed ordinary blue map control is never enough to
        # license a click without the wide modal frame and bright paper.
        bare = Image.new("RGB", image.size, (70, 85, 65))
        ImageDraw.Draw(bare).rectangle(
            (450, 335, 550, 355), outline=(15, 80, 150), width=8
        )
        self.assertEqual(popup_clear.classify(bare)[:2], ("map", []))

    def test_dim_gathering_storm_goodbye_border_is_a_leader_button(self) -> None:
        image = Image.new("RGB", (1000, 600), (5, 5, 5))
        draw = ImageDraw.Draw(image)
        draw.rectangle(
            (60, 535, 260, 555), outline=(120, 120, 120), width=3
        )
        draw.line((100, 545, 220, 545), fill=(120, 120, 120), width=2)

        kind, targets, _dark = popup_clear.classify(image)
        self.assertEqual(kind, "leader")
        self.assertEqual(len(targets), 1)
        self.assertTrue(60 <= targets[0][0] <= 260)
        self.assertTrue(535 <= targets[0][1] <= 555)

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

    def test_animation_wobble_reads_as_the_same_scene(self) -> None:
        # Run civvis-20260815T020727Z: the leader target drifted (1197,387) ->
        # (1196,387) between passes, the exact-match no-op guard kept resetting,
        # and one conversation ate clicks for 900 s. A pixel or two of drift is
        # the SAME scene; a different button across the window is not.
        self.assertTrue(popup_clear.near((1197, 387), (1196, 387)))
        self.assertTrue(popup_clear.same_scene(
            "leader", (1196.4, 387.0), ("leader", (1197, 387))))
        self.assertFalse(popup_clear.same_scene(
            "leader", (1300, 387), ("leader", (1197, 387))))
        self.assertFalse(popup_clear.same_scene(
            "card", (1197, 387), ("leader", (1197, 387))))
        self.assertFalse(popup_clear.same_scene("leader", (1197, 387), None))


if __name__ == "__main__":
    unittest.main()
