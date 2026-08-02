#!/usr/bin/env python3
"""Regression checks for safe Civilization VI process detection."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_env  # noqa: E402


class Civ6EnvTest(unittest.TestCase):
    def test_game_process_detection_ignores_osascript_argument_text(self) -> None:
        listing = """
 101 /usr/bin/osascript -e tell application \"System Events\" to tell process \"Civ6_Exe_Child\" to get name
 102 /bin/zsh -lc pgrep -f Civ6_Exe
 103 /Users/martbot/Library/Application Support/Steam/steamapps/common/Sid Meier's Civilization VI/Civ6.app/Contents/MacOS/Civ6_Exe_Child
 104 /Users/martbot/Library/Application Support/Steam/steamapps/common/Sid Meier's Civilization VI/Civ6.app/Contents/MacOS/Civ6_Exe --child
 105 /Applications/Steam.app/Contents/MacOS/steam_osx
"""

        self.assertEqual(civ6_env._game_pids_from_ps(listing), [103, 104])


if __name__ == "__main__":
    unittest.main()
