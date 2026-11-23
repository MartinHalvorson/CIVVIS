#!/usr/bin/env python3
"""`civ6_map_script_golden.py` throws the crate's dice, builds the crate's
fractal, and has written a fixture for every world it says it writes.

The fixtures are what `src/mapgen/shipped/tests.rs` holds the Rust port to, so
the two sides have to start from the same numbers. The values pinned here are
pinned again in that file (`the_fixture_tool_and_the_port_share_their_dice_and
_their_fractal`); changing either implementation alone turns one of them red.

Nothing here needs the game. Running the shipped scripts does, which is what
`civ6_map_script_golden.py check` is for on a fleet Mac.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_map_script_golden as golden  # noqa: E402


class TheDiceAreTheCrates(unittest.TestCase):
    def test_a_seed_throws_what_src_rng_throws(self):
        rng = golden.Rng(1)
        self.assertEqual([rng.below(1000) for _ in range(6)], [387, 965, 744, 470, 780, 485])

    def test_c_division_rounds_toward_zero(self):
        self.assertEqual(golden.tdiv(-7, 2), -3)
        self.assertEqual(golden.tdiv(7, -2), -3)
        self.assertEqual(golden.tdiv(-7, -2), 3)
        self.assertEqual(golden.tdiv(7, 2), 3)


class TheFractalIsTheCrates(unittest.TestCase):
    def test_a_polar_field(self):
        field = golden.Fractal(golden.Rng(7), 44, 26, 2, golden.POLAR, None, 6, 5)
        self.assertEqual(field.percent(62), 103)
        self.assertEqual([field.height(x, 13) for x in (0, 10, 22, 43)], [0, 11, 98, 34])

    def test_a_ridged_wrapping_field(self):
        field = golden.Fractal(golden.Rng(7), 44, 26, 3, golden.WRAP_X, None, -1, -1)
        field.ridges(golden.Rng(8), 9, golden.WRAP_X, 10, 5)
        self.assertEqual(field.percent(89), 211)
        self.assertEqual([field.height(x, 13) for x in (0, 10, 22, 43)], [175, 152, 235, 119])

    def test_a_plot_off_the_map_stands_at_nothing(self):
        field = golden.Fractal(golden.Rng(7), 44, 26, 2, golden.POLAR, None, 6, 5)
        self.assertEqual(field.height(-1, 3), 0)
        self.assertEqual(field.height(44, 3), 0)
        self.assertEqual(field.height(3, 26), 0)


class TheEnginesLuaIsReadAsLua(unittest.TestCase):
    def test_a_type_annotation_is_dropped(self):
        self.assertEqual(
            golden.strip_annotations("local iHillPercentByDistance:table = {50, 40, 30};"),
            "local iHillPercentByDistance = {50, 40, 30};")
        self.assertEqual(golden.strip_annotations("function f(a:number, b:table)"), "function f(a, b)")

    def test_a_method_call_is_left_alone(self):
        for line in ("featuregen:AddFeatures(true, true);", "local area = plot:GetArea();",
                     "aTable:table(1)", "x = t:number"):
            with self.subTest(line=line):
                self.assertEqual(golden.strip_annotations(line), line)


class AWorldIsWrittenAsText(unittest.TestCase):
    def test_every_kind_of_plot(self):
        # Two rows of three, by plot index from the south-west: Ocean, Coast
        # and flat grass on the southern row; plains Hills, a desert Mountain
        # and a plot the script never wrote (flat snow) on the northern one.
        plots = {0: 3, 1: 3, 2: 2, 3: 1, 4: 0, 5: None}
        terrain = {0: 16, 1: 15, 2: 0, 3: 3, 4: 8, 5: 12}
        self.assertEqual(golden.render(plots, terrain, 3, 2), "P ^ds\n  . g\n")

    def test_trailing_sea_is_not_written(self):
        plots = {0: 2, 1: 3, 2: 3}
        terrain = {0: 9, 1: 16, 2: 16}
        self.assertEqual(golden.render(plots, terrain, 3, 1), "t\n")


class EveryFixtureIsOnDisk(unittest.TestCase):
    def test_each_script_has_each_size_and_seed(self):
        missing = [
            str(golden.fixture_path(name, *fixture).relative_to(golden.REPO))
            for name in golden.SCRIPTS
            for fixture in golden.FIXTURES
            if not golden.fixture_path(name, *fixture).is_file()
        ]
        self.assertEqual(missing, [], "run tools/civ6_map_script_golden.py write on a fleet Mac")

    def test_nothing_on_disk_is_unaccounted_for(self):
        wanted = {golden.fixture_path(name, *fixture).name
                  for name in golden.SCRIPTS for fixture in golden.FIXTURES}
        found = {path.name for path in golden.GOLDEN.glob("*.txt")}
        self.assertTrue(found, "no fixtures at all")
        self.assertEqual(found - wanted, set(), "fixtures the tool would not rewrite")

    def test_a_fixture_is_a_whole_map(self):
        for name in golden.SCRIPTS:
            for width, height, seed in golden.FIXTURES:
                with self.subTest(name=name, size=(width, height), seed=seed):
                    lines = golden.fixture_path(name, width, height, seed).read_text().splitlines()
                    self.assertEqual(len(lines), height)
                    self.assertTrue(all(len(line) <= 2 * width for line in lines))
                    self.assertTrue(any(line.strip() for line in lines), "an empty world")


class WithoutTheGame(unittest.TestCase):
    def test_no_install_is_exit_two(self):
        saved = golden.civ6_env.assets_dir

        def missing(explicit=None):
            raise SystemExit("Civilization VI install not found")

        golden.civ6_env.assets_dir = missing
        try:
            self.assertEqual(golden.main(["check"]), 2)
        finally:
            golden.civ6_env.assets_dir = saved


if __name__ == "__main__":
    unittest.main()
