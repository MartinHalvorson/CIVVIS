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
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "click_at") as click:
            started = civ6_play.configure_and_start((100, 33, 756, 480), args(), Path(temporary))

        self.assertFalse(started)
        setter.assert_called_once_with(
            (100, 33, 756, 480), "difficulty", "DIFFICULTY_SETTLER", Path(temporary)
        )
        screenshot.assert_not_called()
        click.assert_not_called()

    def test_setup_starts_only_after_every_required_dropdown_succeeds(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "set_dropdown", return_value=True) as setter, \
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
        screenshot.assert_called_once_with(Path(temporary) / "setup.png")
        click.assert_called_once_with(100 + int(756 * civ6_play.START_GAME[0]),
                                      33 + int(480 * civ6_play.START_GAME[1]))

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
