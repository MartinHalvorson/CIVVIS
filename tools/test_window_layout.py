#!/usr/bin/env python3
"""The operator's saved window layout (operator 2026-10-14) and its readers."""
from __future__ import annotations

import json
import re
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS / "civ6_control"))
import window_layout  # noqa: E402
from civ6_control import macos_window  # noqa: E402

SCREEN = (1728, 1117)
SAVED = {
    "upper-left": {"process": "Civ6", "frame": [0, 33, 902, 611]},
    "upper-right": {"title": r"CIVVIS|127\.0\.0\.1", "frame": [898, 33, 830, 614]},
    "lower-left": {"process": "Terminal", "frame": [0, 648, 867, 469]},
    "lower-right": {"process": "Terminal", "frame": [861, 650, 867, 460]},
}


class WindowLayoutFileTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / "layout.json"

    def tearDown(self):
        self.dir.cleanup()

    def test_save_then_load_round_trips_every_quadrant(self):
        window_layout.save(SAVED, SCREEN, self.path)
        loaded = window_layout.load(SCREEN, self.path)
        self.assertEqual(set(loaded), set(SAVED))
        self.assertEqual(loaded["upper-left"]["frame"], (0, 33, 902, 611))
        self.assertEqual(loaded["lower-right"]["process"], "Terminal")

    def test_another_screen_size_ignores_the_file(self):
        window_layout.save(SAVED, SCREEN, self.path)
        self.assertEqual(window_layout.load((2560, 1440), self.path), {})
        self.assertIsNone(window_layout.frame_for("upper-left", (2560, 1440), self.path))

    def test_missing_or_malformed_files_are_no_layout(self):
        self.assertEqual(window_layout.load(SCREEN, self.path), {})
        self.path.write_text("{not json")
        self.assertEqual(window_layout.load(SCREEN, self.path), {})
        self.path.write_text(json.dumps({"screen": list(SCREEN), "windows": {
            "upper-left": {"frame": [0, 33, 0, 611]}, "middle": {"frame": [1, 2, 3, 4]}}}))
        self.assertEqual(window_layout.load(SCREEN, self.path), {})

    def test_quadrant_of_reads_the_window_centre(self):
        for quadrant, spec in SAVED.items():
            self.assertEqual(window_layout.quadrant_of(tuple(spec["frame"]), SCREEN), quadrant)


class SavedFramePlacementTests(unittest.TestCase):
    def placed(self, **kwargs):
        scripts: list[str] = []
        with mock.patch.object(macos_window, "_best_effort_osascript",
                               lambda script, what: scripts.append(script)):
            macos_window.place_game("Civ6", "left", 0.5, 0.5,
                                    get_desktop_size=lambda: SCREEN,
                                    get_game_window=lambda: None, **kwargs)
        size = re.search(r"set size to \{(\d+), (\d+)\}", scripts[0])
        position = re.search(r"set position to \{(\d+), (\d+)\}", scripts[0])
        return (int(position.group(1)), int(position.group(2)),
                int(size.group(1)), int(size.group(2)))

    def test_without_a_frame_the_game_takes_the_computed_quadrant(self):
        self.assertEqual(self.placed(), (0, 33, 864, 542))

    def test_a_saved_frame_wins_over_the_fractions(self):
        self.assertEqual(self.placed(frame=(0, 33, 902, 611)), (0, 33, 902, 611))

    def test_civ6_play_hands_the_saved_upper_left_frame_to_the_placer(self):
        import civ6_play
        seen = {}
        with mock.patch.object(civ6_play, "desktop_size", lambda: SCREEN), \
             mock.patch.object(civ6_play.window_layout, "frame_for",
                               lambda quadrant, size: (0, 33, 902, 611) if quadrant == "upper-left" else None), \
             mock.patch.object(civ6_play.macos_window, "place_game",
                               lambda *a, **k: seen.update(k)):
            civ6_play.place_game("left", 0.5, 0.5)
            self.assertEqual(seen["frame"], (0, 33, 902, 611))
            civ6_play.place_game("left", 0.5, 1.0)  # a full-height request is not the quadrant
            self.assertIsNone(seen["frame"])


class ComputerControlLayoutTests(unittest.TestCase):
    def test_the_saved_lower_right_terminal_is_placed_too(self):
        import computer_control as cc
        placed = []
        saved = {q: dict(spec, frame=tuple(spec["frame"])) for q, spec in SAVED.items()}
        with mock.patch.object(cc, "desktop_points", lambda: SCREEN), \
             mock.patch.object(cc, "place_window",
                               lambda frame, process=None, title=None, skip_owners=frozenset():
                               placed.append((process or title, frame)) or None):
            report = cc.layout(saved=saved)
        self.assertIn(("Terminal", (861, 650, 867, 460)), placed)
        self.assertIn(("Civ6", (0, 33, 902, 611)), placed)
        self.assertEqual(len(report), 4)
        self.assertTrue(all(row["placed"] for row in report))

    def test_no_saved_layout_keeps_the_computed_three(self):
        import computer_control as cc
        placed = []
        with mock.patch.object(cc, "desktop_points", lambda: SCREEN), \
             mock.patch.object(cc, "place_window",
                               lambda frame, process=None, title=None, skip_owners=frozenset():
                               placed.append((process or title, frame)) or None):
            cc.layout(saved={})
        self.assertEqual(len(placed), 3)
        self.assertIn(("Civ6", cc.quadrant_frame("upper-left", *SCREEN)), placed)


if __name__ == "__main__":
    unittest.main()
