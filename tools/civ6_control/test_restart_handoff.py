"""Tests for New game by restart: one parked Civ VI handed to the next run.

What must hold: the handoff names exactly one parked game and its settings;
the next run takes it only when that game is still the running one, the
request is fresh and the settings are its own; and it is single-use, so a
refused or failed restart can never be retried against the same game.

Run: python3 -m unittest tools/civ6_control/test_restart_handoff.py
"""

from __future__ import annotations

import json
import os
import sqlite3
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import civ6_brain  # noqa: E402
from civ6_control import orders, restart_handoff  # noqa: E402

CONFIG = {
    "RunTag": "civvis-old", "RuleSet": "RULESET_EXPANSION_2",
    "MapScript": "Pangaea.lua", "MapSize": "MAPSIZE_TINY",
    "Difficulty": "DIFFICULTY_EMPEROR", "GameSpeed": "GAMESPEED_ONLINE",
    "Leader": "LEADER_SIMON_BOLIVAR", "StartEra": None, "MaxTurns": 650,
    "HumanPlayers": 1, "GameModes": {"GAMEMODE_HEROES": False},
    "OrdersDb": "/runs/civvis-old/orders.sqlite",
}


class Handoff(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = TemporaryDirectory()
        self.path = Path(self.tmp.name) / "restart.json"

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def park(self, now: float = 1000.0, pids=(4242,)) -> dict:
        return restart_handoff.write("civvis-old", CONFIG["OrdersDb"], CONFIG, pids,
                                     path=self.path, now=now)

    def test_parking_records_the_run_its_channel_its_game_and_its_settings(self):
        written = self.park(pids=(43, 42))
        on_disk = json.loads(self.path.read_text())
        self.assertEqual(on_disk, written)
        self.assertEqual(on_disk["from_tag"], "civvis-old")
        self.assertEqual(on_disk["orders_db"], "/runs/civvis-old/orders.sqlite")
        self.assertEqual(on_disk["civ_pids"], [42, 43])
        self.assertEqual(on_disk["settings"]["Leader"], "LEADER_SIMON_BOLIVAR")
        self.assertNotIn("RunTag", on_disk["settings"], "a tag is not a setting")
        self.assertRegex(on_disk["parked_utc"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
        self.assertTrue(restart_handoff.parked_for("civvis-old", self.path))
        self.assertFalse(restart_handoff.parked_for("civvis-new", self.path))

    def test_a_missing_or_malformed_file_is_no_handoff(self):
        self.assertIsNone(restart_handoff.read(self.path))
        self.assertFalse(restart_handoff.parked_for("civvis-old", self.path))
        self.assertEqual(restart_handoff.claim(CONFIG, [4242], path=self.path), (None, None))
        self.path.write_text("{not json")
        self.assertIsNone(restart_handoff.read(self.path))
        self.assertIsNone(restart_handoff.standing([4242], self.path))
        handoff, why = restart_handoff.claim(CONFIG, [4242], path=self.path)
        self.assertIsNone(handoff)
        self.assertIn("could not be read", why)
        self.assertFalse(self.path.exists())

    def test_the_next_run_takes_a_fresh_handoff_at_its_own_settings_once(self):
        self.park(now=1000.0)
        handoff, why = restart_handoff.claim(dict(CONFIG, RunTag="civvis-new"), [4242],
                                             path=self.path, now=1030.0)
        self.assertIsNone(why)
        self.assertEqual(handoff["from_tag"], "civvis-old")
        self.assertFalse(self.path.exists(), "a handoff is single-use")
        self.assertEqual(restart_handoff.claim(CONFIG, [4242], path=self.path), (None, None))

    def test_every_refusal_says_why_and_still_consumes_the_handoff(self):
        cases = [
            ({}, [], "no longer running"),
            ({}, [5151], "is not the one civvis-old parked"),
            ({}, [4242, 5151], "is not the one"),
            ({"now": 1000.0 + restart_handoff.MAX_AGE_S + 1}, [4242], "waited"),
            ({"config": dict(CONFIG, Difficulty="DIFFICULTY_IMMORTAL")}, [4242],
             "different settings (Difficulty)"),
            ({"config": dict(CONFIG, Leader="LEADER_TRAJAN", MapSize="MAPSIZE_SMALL")},
             [4242], "different settings (Leader, MapSize)"),
        ]
        for extra, pids, reason in cases:
            with self.subTest(reason=reason):
                self.park(now=1000.0)
                handoff, why = restart_handoff.claim(
                    extra.get("config", CONFIG), pids, path=self.path,
                    now=extra.get("now", 1010.0))
                self.assertIsNone(handoff)
                self.assertIn(reason, why)
                self.assertFalse(self.path.exists())

    def test_a_standing_handoff_accounts_for_exactly_the_running_game(self):
        self.park(now=0.0)  # age is not the climb's question
        self.assertIsNotNone(restart_handoff.standing([4242], self.path))
        self.assertIsNone(restart_handoff.standing([], self.path))
        self.assertIsNone(restart_handoff.standing([4242, 9], self.path))
        self.assertIsNone(restart_handoff.standing([9], self.path))

    def test_the_default_path_and_the_mode_override(self):
        with mock.patch.dict(os.environ, {}, clear=False):
            os.environ.pop(restart_handoff.HANDOFF_ENV, None)
            os.environ.pop(restart_handoff.MODE_ENV, None)
            self.assertEqual(restart_handoff.handoff_path(),
                             Path.home() / ".civvis-live-restart.json")
            self.assertEqual(restart_handoff.restart_mode(), restart_handoff.MODE_RESTART)
            for value, mode in (("2", 2), ("3", 3), ("9", 1), ("x", 1)):
                os.environ[restart_handoff.MODE_ENV] = value
                self.assertEqual(restart_handoff.restart_mode(), mode)
            os.environ[restart_handoff.HANDOFF_ENV] = str(self.path)
            self.assertEqual(restart_handoff.handoff_path(), self.path)


class Rows(unittest.TestCase):
    """The two rows the parked game reads, through the brain's own schema."""

    def channel(self, tmp: str) -> Path:
        path = Path(tmp) / "orders.sqlite"
        civ6_brain.connect(path).close()
        return path

    def rows(self, path: Path) -> list[tuple]:
        with sqlite3.connect(path) as db:
            return db.execute(
                "SELECT run, kind, verb, x, frame FROM orders ORDER BY seq").fetchall()

    def test_a_restart_retire_carries_x_1_and_a_plain_one_none(self):
        with TemporaryDirectory() as tmp:
            path = self.channel(tmp)
            self.assertTrue(orders.request_retire(path, "civvis-old", 7, "operator",
                                                  restart=True))
            self.assertEqual(self.rows(path), [("civvis-old", "retire", "operator", 1, 990)])
            self.assertTrue(orders.request_retire(path, "civvis-old", 7, "operator"))
            self.assertEqual(self.rows(path),
                             [("civvis-old", "retire", "operator", None, 990)])

    def test_the_restart_row_names_the_parked_run_and_the_mode(self):
        with TemporaryDirectory() as tmp:
            path = self.channel(tmp)
            self.assertTrue(orders.request_restart(path, "civvis-old", 2))
            self.assertEqual(self.rows(path), [("civvis-old", "restart", "operator", 2, 993)])
            # Never creates a channel: the parked game attached a real one.
            self.assertFalse(orders.request_restart(Path(tmp) / "absent.sqlite",
                                                    "civvis-old", 1))
            self.assertFalse((Path(tmp) / "absent.sqlite").exists())


if __name__ == "__main__":
    unittest.main()
