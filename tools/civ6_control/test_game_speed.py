"""Tests for the per-speed turn limits the live lane plays to.

`game_speed.TURN_LIMITS` states five numbers. What must hold: they are Civil-
ization VI's own (summed from the shipped `GameSpeed_Turns`, wherever the game
is installed), they are the numbers CIVVIS's engine uses (`GameSpeed::turn_limit`
in `src/setup.rs`), and every spelling the lane carries resolves to them.

Run: python3 -m unittest tools/civ6_control/test_game_speed.py
"""

from __future__ import annotations

import re
import sys
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from civ6_control import game_speed  # noqa: E402

REPO = Path(__file__).resolve().parents[2]

#: Where Steam puts the game on a fleet Mac. CI runners have no install, so the
#: XML check runs on play hosts only; the Rust check below runs everywhere.
GAMESPEEDS_XML = (
    Path.home() / "Library/Application Support/Steam/steamapps/common"
    / "Sid Meier's Civilization VI/Civ6.app/Contents/Assets/Base/Assets"
    / "Gameplay/Data/GameSpeeds.xml"
)


class TurnLimits(unittest.TestCase):
    def test_the_five_speeds(self):
        self.assertEqual(game_speed.TURN_LIMITS, {
            "GAMESPEED_ONLINE": 250,
            "GAMESPEED_QUICK": 330,
            "GAMESPEED_STANDARD": 500,
            "GAMESPEED_EPIC": 750,
            "GAMESPEED_MARATHON": 1500,
        })

    def test_every_spelling_the_lane_carries(self):
        for spelling in ("GAMESPEED_STANDARD", "standard", " Standard ",
                         "gamespeed_standard"):
            with self.subTest(spelling=spelling):
                self.assertEqual(game_speed.speed_type(spelling), "GAMESPEED_STANDARD")
                self.assertEqual(game_speed.turn_limit(spelling), 500)
                self.assertEqual(game_speed.civvis_id(spelling), "standard")

    def test_an_unknown_speed_is_unknown_not_online(self):
        # A fallback belongs to the caller, which knows what it is guessing.
        for spelling in ("", "  ", "GAMESPEED_", "GAMESPEED_LUDICROUS", None, 500):
            with self.subTest(spelling=spelling):
                self.assertIsNone(game_speed.speed_type(spelling))
                self.assertIsNone(game_speed.turn_limit(spelling))
                self.assertIsNone(game_speed.civvis_id(spelling))

    def test_the_default_speed_is_a_known_one(self):
        self.assertIn(game_speed.DEFAULT_SPEED, game_speed.TURN_LIMITS)


class AgreesWithTheEngine(unittest.TestCase):
    def test_civvis_game_speed_turn_limit_is_the_same_table(self):
        source = (REPO / "src/setup.rs").read_text(encoding="utf-8")
        body = re.search(
            r"pub const fn turn_limit\(self\) -> u32 \{(.*?)\n    \}", source, re.S)
        self.assertIsNotNone(body, "GameSpeed::turn_limit moved; re-anchor this test")
        arms = dict(re.findall(r"Self::(\w+) => (\d+)", body.group(1)))
        self.assertEqual(
            {f"GAMESPEED_{name.upper()}": int(turns) for name, turns in arms.items()},
            game_speed.TURN_LIMITS,
        )


@unittest.skipUnless(GAMESPEEDS_XML.is_file(), "Civilization VI is not installed here")
class AgreesWithTheGame(unittest.TestCase):
    def test_the_sums_of_gamespeed_turns(self):
        root = ET.parse(GAMESPEEDS_XML).getroot()
        totals: dict[str, int] = {}
        for row in root.find("GameSpeed_Turns"):
            fields = {child.tag: child.text for child in row}
            speed = fields["GameSpeedType"]
            totals[speed] = totals.get(speed, 0) + int(fields["TurnsPerIncrement"])
        self.assertEqual(totals, game_speed.TURN_LIMITS)


if __name__ == "__main__":
    unittest.main()
