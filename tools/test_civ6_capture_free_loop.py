#!/usr/bin/env python3
"""Unit checks for the supervised non-visual CIVVIS game owner."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_capture_free_loop as loop


def args(**changes):
    values = {
        "difficulty": loop.PROFILE["difficulty"],
        "leader": loop.PROFILE["leader"],
        "ruleset": loop.PROFILE["ruleset"],
        "map": loop.PROFILE["map"],
        "map_size": loop.PROFILE["map_size"],
        "speed": loop.PROFILE["speed"],
        "game_mode": [],
        "max_turns": 650,
        "timeout": 10_800.0,
        "timeout_ceiling": 14_400.0,
        "victory": "science",
        "refresh_seconds": 0.0,
        "with_": ["rapid-city-expansion-2"],
        "without": [],
        "civvis_bin": "/tmp/civvis_orders",
    }
    values.update(changes)
    return SimpleNamespace(**values)


class FixedProfileTests(unittest.TestCase):
    def test_config_is_rome_emperor_online_continents_with_modes_cleared(self):
        config = loop.build_config(args(), "civvis-test", Path("/tmp/orders.sqlite"))
        self.assertEqual(config["Leader"], "LEADER_TRAJAN")
        self.assertEqual(config["Difficulty"], "DIFFICULTY_EMPEROR")
        self.assertEqual(config["GameSpeed"], "GAMESPEED_ONLINE")
        self.assertEqual(config["MapScript"], "Continents.lua")
        self.assertEqual(config["MapSize"], "MAPSIZE_SMALL")
        self.assertEqual(config["MaxTurns"], 650)
        self.assertTrue(config["CivvisDecides"])
        self.assertTrue(config["ExportState"])
        self.assertFalse(any(config["GameModes"].values()))

    def test_profile_refuses_a_setting_the_known_controls_cannot_verify(self):
        with self.assertRaisesRegex(ValueError, "difficulty"):
            loop.validate_profile(args(difficulty="DIFFICULTY_KING"))
        with self.assertRaisesRegex(ValueError, "game mode"):
            loop.validate_profile(args(game_mode=["GAMEMODE_HEROES"]))

    def test_attached_player_command_is_nonvisual_and_civvis_driven(self):
        command = loop.attach_command(args(), "civvis-test", Path("/tmp/orders.sqlite"))
        self.assertIn("--attach-running", command)
        self.assertIn("--civvis-decides", command)
        self.assertIn("--export-state", command)
        self.assertIn("--no-deal-sessions", command)
        self.assertIn("rapid-city-expansion-2", command)
        self.assertNotIn("--load-save", command)

    def test_source_never_invokes_a_capture_or_ocr_reader(self):
        source = Path(loop.__file__).read_text(encoding="utf-8")
        self.assertNotIn("macos_capture", source)
        self.assertNotIn("macos_ocr", source)
        self.assertNotIn("screenshot(", source)
        self.assertNotIn("civ6_nudge_end_turn", source)


class _Player:
    returncode = None

    def poll(self):
        return self.returncode


class SilenceRecoveryTests(unittest.TestCase):
    def test_event_silence_returns_the_wedge_to_the_supervisor(self):
        ticks = [0.0]

        def now():
            return ticks[0]

        def sleep(seconds):
            ticks[0] += seconds

        reason = loop.monitor_player(
            _Player(), Path("/definitely/missing/events.jsonl"),
            silence_s=4.0, poll_s=1.0,
            should_stop=lambda: False,
            now=now, sleep=sleep,
        )
        self.assertEqual(reason, "wedge")
        self.assertEqual(ticks[0], 4.0)

    def test_operator_stop_returns_without_a_recovery_action(self):
        reason = loop.monitor_player(
            _Player(), Path("/definitely/missing/events.jsonl"),
            silence_s=1.0, poll_s=1.0,
            should_stop=lambda: True,
        )
        self.assertEqual(reason, "stopped")


if __name__ == "__main__":
    unittest.main()
