"""CIVVIS_VICTORIES_OFF: victory types the host policy switches off at setup.

Operator 2026-11-19 ~00:53Z: "tilt harder towards science and culture wins
please. we can turn off diplo in the games". The Create Game screen reads the
configuration database, so the installed CivvisControlConfig.xml carries one
`Victories` update per named type (`EnabledByDefault = 0`), which the shipped
`VictoryUpdates` query honours when the ruleset is chosen.
"""
from __future__ import annotations

import contextlib
import io
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import install  # noqa: E402
import civ6_play  # noqa: E402


class Defaults(SimpleNamespace):
    def __getattr__(self, name):
        return None


def config(**changes):
    return civ6_play.build_config(
        Defaults(tag="t", game_mode=[], dialogue_seconds=0.25, difficulty="DIFFICULTY_DEITY",
                 map_size="MAPSIZE_SMALL", speed="GAMESPEED_ONLINE", map="Continents.lua",
                 leader="LEADER_ALEXANDER", **changes))


def installed_xml(cfg: dict) -> str:
    with tempfile.TemporaryDirectory() as tmp:
        install._write_mod(Path(tmp), dict(cfg))
        return (Path(tmp) / "CivvisControlConfig.xml").read_text()


class VictoriesOff(unittest.TestCase):
    def test_the_policy_names_victory_types_in_the_shipped_order(self):
        self.assertEqual(civ6_play.parse_victories_off(None), [])
        self.assertEqual(civ6_play.parse_victories_off(""), [])
        self.assertEqual(civ6_play.parse_victories_off(" victory_diplomatic "), ["VICTORY_DIPLOMATIC"])
        self.assertEqual(civ6_play.parse_victories_off("VICTORY_SCORE,VICTORY_DIPLOMATIC"),
                         ["VICTORY_DIPLOMATIC", "VICTORY_SCORE"])
        with contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(civ6_play.parse_victories_off("VICTORY_DIPLOMACY,VICTORY_DIPLOMATIC"),
                             ["VICTORY_DIPLOMATIC"])
        self.assertIn("VICTORY_DIPLOMACY", out.getvalue())

    def test_a_named_type_is_not_enabled_by_default_in_the_installed_setup(self):
        cfg = config(victories_off=["VICTORY_DIPLOMATIC"])
        self.assertEqual(cfg["VictoriesOff"], ["VICTORY_DIPLOMATIC"])
        root = ET.fromstring(installed_xml(cfg))
        updates = root.findall("./Victories/Update")
        self.assertEqual(len(updates), 1)
        self.assertEqual(updates[0].find("Where").get("VictoryType"), "VICTORY_DIPLOMATIC")
        self.assertEqual(updates[0].find("Set").get("EnabledByDefault"), "0")

    def test_no_policy_leaves_every_victory_as_shipped(self):
        cfg = config()
        self.assertEqual(cfg["VictoriesOff"], [])
        self.assertIsNone(ET.fromstring(installed_xml(cfg)).find("./Victories"))

    def test_a_malformed_name_is_refused_at_install(self):
        with self.assertRaises(ValueError):
            installed_xml(dict(config(), VictoriesOff=['VICTORY_X"/><Bad']))


if __name__ == "__main__":
    unittest.main()
