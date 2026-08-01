#!/usr/bin/env python3
"""Focused setup-contract checks for the live Civ VI launcher."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import call, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_play  # noqa: E402


def args(**changes):
    values = {
        "difficulty": "DIFFICULTY_SETTLER",
        "map_size": "MAPSIZE_SMALL",
        "speed": "GAMESPEED_ONLINE",
        "map": "Continents.lua",
        "leader": "LEADER_TRAJAN",
        "game_mode": [],
    }
    values.update(changes)
    return SimpleNamespace(**values)


class Civ6PlayTest(unittest.TestCase):
    def test_place_game_sizes_before_positioning_the_upper_quadrant(self) -> None:
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.subprocess, "run") as run:
            civ6_play.place_game("right", 0.5, 0.5)

        script = run.call_args.args[0][-1]
        self.assertLess(script.index("set size"), script.index("set position"))
        self.assertIn("set size to {756, 480}", script)
        self.assertIn("set position to {756, 33}", script)

    def test_screen_locked_reads_console_session_state(self) -> None:
        with patch.object(
            civ6_play.subprocess,
            "run",
            return_value=SimpleNamespace(
                stdout='"CGSSessionScreenIsLocked"=Yes', returncode=0
            ),
        ):
            self.assertTrue(civ6_play.screen_locked())

        with patch.object(
            civ6_play.subprocess,
            "run",
            return_value=SimpleNamespace(
                stdout='"CGSSessionScreenIsLocked"=No', returncode=0
            ),
        ):
            self.assertFalse(civ6_play.screen_locked())

    def test_play_refuses_to_launch_while_console_is_locked(self) -> None:
        with patch.object(civ6_play.vision, "available", return_value=True), \
             patch.object(civ6_play, "screen_locked", return_value=True), \
             patch.object(civ6_play.gamelock, "acquire") as acquire, \
             patch.object(civ6_play, "_play") as run:
            result = civ6_play.play(args())

        self.assertEqual(result, 7)
        acquire.assert_not_called()
        run.assert_not_called()

    def test_civvis_decision_mode_always_enables_state_export(self) -> None:
        self.assertTrue(civ6_play.state_export_enabled(
            SimpleNamespace(export_state=False, civvis_decides=True)
        ))
        self.assertTrue(civ6_play.state_export_enabled(
            SimpleNamespace(export_state=True, civvis_decides=False)
        ))
        self.assertFalse(civ6_play.state_export_enabled(
            SimpleNamespace(export_state=False, civvis_decides=False)
        ))

    def test_setup_does_not_start_when_a_required_dropdown_is_unverified(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "set_dropdown", return_value=False) as setter, \
             patch.object(civ6_play, "select_requested_leader") as leader, \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "click_at") as click:
            started = civ6_play.configure_and_start((100, 33, 756, 480), args(), Path(temporary))

        self.assertFalse(started)
        setter.assert_called_once_with(
            (100, 33, 756, 480), "difficulty", "DIFFICULTY_SETTLER", Path(temporary)
        )
        screenshot.assert_not_called()
        leader.assert_not_called()
        click.assert_not_called()

    def test_setup_starts_only_after_every_required_dropdown_succeeds(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "set_dropdown", return_value=True) as setter, \
             patch.object(civ6_play, "select_requested_leader", return_value=True) as leader, \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "click_at") as click:
            started = civ6_play.configure_and_start((100, 33, 756, 480), args(), Path(temporary))

        self.assertTrue(started)
        self.assertEqual(
            setter.call_args_list,
            [
                call((100, 33, 756, 480), "difficulty", "DIFFICULTY_SETTLER", Path(temporary)),
                call((100, 33, 756, 480), "map_size", "MAPSIZE_SMALL", Path(temporary)),
                call((100, 33, 756, 480), "speed", "GAMESPEED_ONLINE", Path(temporary)),
            ],
        )
        leader.assert_called_once_with(
            (100, 33, 756, 480), "LEADER_TRAJAN", Path(temporary)
        )
        screenshot.assert_called_once_with(Path(temporary) / "setup.png")
        click.assert_called_once_with(100 + int(756 * civ6_play.START_GAME[0]),
                                      33 + int(480 * civ6_play.START_GAME[1]))

    def test_setup_refuses_to_start_when_requested_leader_is_unverified(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "set_dropdown", return_value=True), \
             patch.object(civ6_play, "select_requested_leader", return_value=False), \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "click_at") as click:
            started = civ6_play.configure_and_start(
                (100, 33, 756, 480), args(), Path(temporary)
            )

        self.assertFalse(started)
        screenshot.assert_not_called()
        click.assert_not_called()

    def test_roster_resolves_requested_leader_to_rendered_name(self) -> None:
        self.assertEqual(civ6_play.leader_display_name("LEADER_JADWIGA"), "Jadwiga")

    def test_leader_picker_scans_past_the_old_harald_cutoff(self) -> None:
        bounds = (756, 33, 756, 480)
        row = {"text": "Jadwiga", "x": 0.73, "y": 0.29,
               "width": 0.04, "height": 0.02}
        selected = {"text": "Jadwiga", "x": 0.73, "y": 0.155,
                    "width": 0.04, "height": 0.02}
        observations = [[] for _ in range(15)] + [[row], [selected]]

        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "screenshot"), \
             patch.object(civ6_play, "list_opened", return_value=True), \
             patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", side_effect=observations), \
             patch.object(civ6_play.macos_input, "move"), \
             patch.object(civ6_play.macos_input, "scroll") as scroll, \
             patch.object(civ6_play, "click_at") as click, \
             patch.object(civ6_play.time, "sleep"):
            found = civ6_play.select_requested_leader(
                bounds, "LEADER_JADWIGA", Path(temporary)
            )

        self.assertTrue(found)
        self.assertEqual(scroll.call_count, 16)
        self.assertEqual(scroll.call_args_list[0], call(civ6_play.LEADER_SCROLL_RESET))
        self.assertTrue(all(
            invocation == call(civ6_play.LEADER_SCROLL_AMOUNT)
            for invocation in scroll.call_args_list[1:]
        ))
        self.assertEqual(click.call_count, 2)

    def test_leader_ocr_maps_the_upscaled_crop_back_to_the_desktop(self) -> None:
        from PIL import Image

        observation = {
            "text": "Jadwiga", "x": 0.30, "y": 0.50,
            "width": 0.10, "height": 0.05,
        }
        with tempfile.TemporaryDirectory() as temporary:
            shot = Path(temporary) / "leader.png"
            Image.new("RGB", (3024, 1964), "white").save(shot)
            with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
                 patch.object(civ6_play.macos_ocr, "recognize", return_value=[observation]):
                mapped = civ6_play._leader_ocr(
                    shot, (756, 33, 756, 480)
                )

        self.assertEqual(mapped[0]["text"], "Jadwiga")
        self.assertAlmostEqual(mapped[0]["x"], 0.733, places=3)
        self.assertAlmostEqual(mapped[0]["y"], 0.293, places=3)
        self.assertAlmostEqual(mapped[0]["width"], 0.011, places=3)

    def test_setup_recovery_backs_out_until_main_menu_is_verified(self) -> None:
        bounds = (756, 33, 756, 480)
        observations = [[], [], [{"text": "Single Player"}]]
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "screenshot"), \
             patch.object(civ6_play.macos_ocr, "recognize", side_effect=observations), \
             patch.object(civ6_play, "click_at") as click, \
             patch.object(civ6_play.time, "sleep"):
            recovered = civ6_play.return_to_main_menu(
                bounds, Path(temporary), attempt=3
            )

        self.assertTrue(recovered)
        self.assertEqual(
            click.call_args_list,
            [call(1307, 116), call(1307, 116)],
        )

    def test_main_menu_click_uses_observed_row_center(self) -> None:
        observation = {
            "text": "Single Player", "x": 0.72, "y": 0.255,
            "width": 0.03, "height": 0.01,
        }
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=[observation]):
            point = civ6_play._main_menu_point(
                Path("menu.png"), (756, 33, 756, 480)
            )

        self.assertEqual(point, (1111, 255))

    def test_main_menu_click_tolerates_one_vision_glyph_error(self) -> None:
        observation = {
            "text": "Single Plaver", "x": 0.72, "y": 0.255,
            "width": 0.03, "height": 0.01,
        }
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=[observation]):
            point = civ6_play._main_menu_point(
                Path("menu.png"), (756, 33, 756, 480)
            )

        self.assertEqual(point, (1111, 255))

    def test_create_game_click_uses_its_visible_label(self) -> None:
        observations = [
            {"text": "Resume Game", "x": 0.75, "y": 0.28,
             "width": 0.03, "height": 0.01},
            {"text": "Create Game", "x": 0.75, "y": 0.31,
             "width": 0.03, "height": 0.01},
        ]
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=observations):
            point = civ6_play._observed_label_point(
                Path("submenu.png"), "Create Game", (756, 33, 756, 480)
            )

        self.assertEqual(point, (1156, 309))

    def test_saved_game_bootstrap_uses_named_row_and_lower_action_button(self) -> None:
        bounds = (756, 33, 756, 480)
        observations = [
            [{"text": "Single Player", "x": 0.72, "y": 0.255,
              "width": 0.03, "height": 0.01}],
            [{"text": "Single Player", "x": 0.72, "y": 0.255,
              "width": 0.03, "height": 0.01}],
            [{"text": "Load Game", "x": 0.75, "y": 0.30,
              "width": 0.03, "height": 0.01}],
            [{"text": "CivvisWriterRepro", "x": 0.68, "y": 0.18,
              "width": 0.04, "height": 0.01}],
            [
                {"text": "LOAD GAME", "x": 0.73, "y": 0.11,
                 "width": 0.04, "height": 0.01},
                {"text": "Load Game", "x": 0.69, "y": 0.49,
                 "width": 0.04, "height": 0.02},
            ],
        ]

        class Tail:
            def poll(self):
                return [{"kind": "state", "turn": 89}]

        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "focus_game"), \
             patch.object(civ6_play, "game_window", return_value=bounds), \
             patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play, "screenshot"), \
             patch.object(civ6_play.macos_ocr, "recognize", side_effect=observations), \
             patch.object(civ6_play.env, "game_pids", return_value=[123]), \
             patch.object(civ6_play.time, "sleep"), \
             patch.object(civ6_play, "click_at") as click:
            loaded = civ6_play.bootstrap_saved_game(
                Tail(), lambda _event: None, Path(temporary),
                args(load_save="/saves/CivvisWriterRepro.Civ6Save"),
            )

        self.assertTrue(loaded)
        self.assertEqual(
            click.call_args_list,
            [call(869, 441), call(1111, 255), call(1156, 299),
             call(1058, 181), call(1073, 491)],
        )

    def test_seat_match_requires_map_and_leader(self) -> None:
        event = {
            "difficulty": "DIFFICULTY_SETTLER",
            "size": "MAPSIZE_SMALL",
            "speed": "GAMESPEED_ONLINE",
            "map": "Continents.lua",
            "leader": "LEADER_TRAJAN",
            "modes": [],
        }

        self.assertEqual(civ6_play.seat_matches_requested(event, args()), (True, True))
        self.assertEqual(
            civ6_play.seat_matches_requested({**event, "leader": "LEADER_CLEOPATRA"}, args()),
            (False, True),
        )
        self.assertEqual(
            civ6_play.seat_matches_requested({**event, "map": "Pangaea.lua"}, args()),
            (False, True),
        )

    def test_seat_match_accepts_the_reported_leader_when_none_was_requested(self) -> None:
        event = {
            "difficulty": "DIFFICULTY_SETTLER",
            "size": "MAPSIZE_SMALL",
            "speed": "GAMESPEED_ONLINE",
            "map": "Continents.lua",
            "leader": "LEADER_GANDHI",
            "modes": [],
        }

        self.assertEqual(
            civ6_play.seat_matches_requested(event, args(leader=None)),
            (True, True),
        )


if __name__ == "__main__":
    unittest.main()
