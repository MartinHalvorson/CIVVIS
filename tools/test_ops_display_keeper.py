#!/usr/bin/env python3
"""The dedicated CIVVIS display has to be BESIDE the game, not behind it.

★★★★★ IT WAS BEHIND IT, EXACTLY. `civvis-display-keeper.mjs` launched Chrome
with `--window-position=0,33 --window-size=864,542`, and on the display this
fleet runs, `--window-side left --window-frac 0.5 --window-vfrac 0.5` — the
flags `civ6_civvis_climb` hands `civ6_play` for every attempt — place
Civilization VI at 0,33 sized 864x542. The same rectangle, to the pixel. The
keeper reported a healthy display, the mirror served frames, and the window sat
under a game the keeper's own header says is deliberately held frontmost.

Both rectangles are read from the code that produces them rather than restated
here: the game's from `macos_window.place_game`, the display's from the
keeper's source. A change to either side that puts them back on top of each
other fails this file.
"""

from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parent
KEEPER = TOOLS / "ops" / "civvis-display-keeper.mjs"
CLIMB = TOOLS / "civ6_civvis_climb.py"
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "civ6_control"))

import macos_window  # noqa: E402

# The display the fleet's verification host runs, in points.
SCREEN = (1728, 1117)


def game_rectangle(side: str, fraction: float, vfraction: float,
                   screen: tuple[int, int] = SCREEN) -> tuple[int, int, int, int]:
    """(x, y, width, height) `place_game` actually asks System Events for.

    Captured from the AppleScript it builds, so this is the real placement rule
    and not a second copy of its arithmetic. The window probe answers None, which
    is the "cannot read it back" path and leaves the request untouched.
    """
    scripts: list[str] = []
    with mock.patch.object(macos_window, "_best_effort_osascript",
                           lambda script, what: scripts.append(script)):
        macos_window.place_game("Civ6", side, fraction, vfraction,
                                get_desktop_size=lambda: screen,
                                get_game_window=lambda: None)
    size = re.search(r"set size to \{(\d+), (\d+)\}", scripts[0])
    position = re.search(r"set position to \{(\d+), (\d+)\}", scripts[0])
    assert size and position, scripts[0]
    return (int(position.group(1)), int(position.group(2)),
            int(size.group(1)), int(size.group(2)))


def display_rectangle() -> tuple[int, int, int, int]:
    """(x, y, width, height) the keeper hands Chrome, read from its source."""
    source = KEEPER.read_text(encoding="utf-8")
    values: dict[str, int] = {}
    for name in ("displayWidth", "displayHeight", "displayX", "displayY"):
        found = re.search(rf"^const {name} = (\w+);", source, re.MULTILINE)
        assert found, f"{name} is no longer a plain constant in {KEEPER.name}"
        written = found.group(1)
        # A constant may be a literal or an earlier one by name (`displayX` is
        # `displayWidth`, so the two panes stay adjacent by construction).
        values[name] = int(written) if written.isdigit() else values[written]
    assert "`--window-position=${displayX},${displayY}`" in source
    assert "`--window-size=${displayWidth},${displayHeight}`" in source
    return (values["displayX"], values["displayY"],
            values["displayWidth"], values["displayHeight"])


def overlap(first: tuple[int, int, int, int],
            second: tuple[int, int, int, int]) -> int:
    """Area the two rectangles share, in square points."""
    ax, ay, aw, ah = first
    bx, by, bw, bh = second
    across = max(0, min(ax + aw, bx + bw) - max(ax, bx))
    down = max(0, min(ay + ah, by + bh) - max(ay, by))
    return across * down


class DisplayKeeperPlacementTests(unittest.TestCase):
    def test_the_climb_still_parks_the_game_in_the_upper_left(self):
        """The premise of everything below. If the game moves, this fails first
        and says so, rather than the overlap test failing mysteriously."""
        source = CLIMB.read_text(encoding="utf-8")
        self.assertIn('"--window-side", "left"', source)
        self.assertIn('"--window-frac", "0.5", "--window-vfrac", "0.5"', source)

    def test_the_display_does_not_sit_on_top_of_the_game(self):
        game = game_rectangle("left", 0.5, 0.5)
        display = display_rectangle()
        self.assertEqual(
            overlap(game, display), 0,
            f"the CIVVIS display {display} overlaps Civilization VI {game}; "
            "Civ VI is held frontmost, so an overlapping display is invisible")

    def test_the_display_is_the_upper_right_quadrant_beside_the_game(self):
        game = game_rectangle("left", 0.5, 0.5)
        display = display_rectangle()
        self.assertEqual(display[2:], game[2:], "the two panes are the same size")
        self.assertEqual(display[1], game[1], "they share a top edge")
        self.assertEqual(display[0], game[0] + game[2],
                         "the display starts where the game ends")
        self.assertEqual(display[0] + display[2], SCREEN[0],
                         "and reaches the right edge of the display")

    def test_the_old_rectangle_is_the_one_this_replaces(self):
        """Named so the defect stays legible: these were the keeper's constants,
        and they are the game's own quadrant."""
        self.assertEqual(game_rectangle("left", 0.5, 0.5), (0, 33, 864, 542))

    def test_no_dead_window_geometry_claims_the_opposite_layout(self):
        """`follow.py` carried an unused `MIRROR_BOUNDS` describing the left half
        as beside a game parked on the right — neither half of which was true."""
        source = (TOOLS / "follow.py").read_text(encoding="utf-8")
        self.assertNotIn("CIVVIS_MIRROR_BOUNDS", source)
        self.assertNotIn("--window-side right", source)


if __name__ == "__main__":
    unittest.main()
