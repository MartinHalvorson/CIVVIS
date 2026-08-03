#!/usr/bin/env python3
"""The north-up reflection must carry the rivers with it.

`rv` bits 1..32 describe E, SE, SW, W, NW and NE. The reconstruction applies
that table to staged coordinates, so `remap_river_masks` must reflect the edge
as well as the plot. These tests pin the composition, not either half: consumer
semantics applied to the staged plots must equal the game's own segments with
both endpoints flipped, including a known edge whose opposite tile is hidden.
"""

import importlib.util
from pathlib import Path
import sys
import unittest

MODULE_PATH = Path(__file__).with_name("follow.py")
SPEC = importlib.util.spec_from_file_location("follow", MODULE_PATH)
follow = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = follow
SPEC.loader.exec_module(follow)


def segments(masks, width):
    """The reconstruction's reading of `rv` at whatever coordinates it holds."""
    segs = set()
    directions = ((1, 0), (1, -1), (0, -1),
                  (-1, 0), (-1, 1), (0, 1))
    for (x, y), rv in masks.items():
        start = follow.offset_to_axial(x, y)
        for direction, bit in zip(directions, (1, 2, 4, 8, 16, 32)):
            if not rv & bit:
                continue
            end = follow.axial_to_offset(start[0] + direction[0],
                                         start[1] + direction[1])
            segs.add(frozenset(((x, y), (end[0] % width, end[1]))))
    return segs


def flipped_truth(masks, width, top):
    """The game's own segments, both endpoints reflected — the target.

    A segment wholly inside the dropped polar row cannot be shown; one that
    merely TOUCHES it is kept, as the surviving plot's edge toward the void —
    losing it would hide a river the game draws.
    """
    segs = set()
    for seg in segments(masks, width):
        a, b = tuple(seg)
        if a[1] > top and b[1] > top:
            continue
        segs.add(frozenset(((a[0], top - a[1]), (b[0], top - b[1]))))
    return segs


def stage(events, top):
    """remap + flip, returning the staged masks keyed by staged offset coords."""
    lost = follow.remap_river_masks(events, top)
    staged = {}
    dropped = []
    for event in events:
        flipped = follow.flip_north_up(event, top, dropped)
        if not isinstance(flipped, dict) or flipped.get("kind") != "tiles":
            continue
        for p in flipped.get("plots", []):
            if p.get("rv"):
                staged[(p["x"], p["y"])] = p["rv"]
    return staged, lost


def tiles_event(plots, width=8, height=6, turn=4, chunk=1):
    return {
        "kind": "tiles", "turn": turn, "width": width, "height": height,
        "chunk": chunk,
        "plots": [
            {"x": x, "y": y, "rv": rv} if rv else {"x": x, "y": y}
            for (x, y), rv in plots.items()
        ],
    }


class RiverFlipTests(unittest.TestCase):
    WIDTH, HEIGHT = 8, 6      # even height -> top 6, with one empty staging row

    def test_diagonal_segments_return_to_the_edge_the_game_reported(self):
        # Both parities, all three flags, a wrap across the east seam, and the
        # neighbourhood filled in so every segment has its flipped carrier.
        masks = {
            (3, 2): 1 | 2 | 4,   # even row: E, SE, SW
            (4, 3): 2 | 4,       # odd row: SE, SW
            (7, 3): 2,           # odd row at the seam: SE wraps to x 0
            (0, 2): 0, (2, 1): 0, (3, 1): 0, (4, 1): 0, (4, 2): 0,
            (5, 2): 0, (3, 3): 0, (7, 2): 0,
        }
        top = follow.mirror_axis(self.HEIGHT)
        # Two chunks, split so a gather has to cross the chunk boundary.
        items = list(masks.items())
        events = [
            tiles_event(dict(items[:4]), self.WIDTH, self.HEIGHT, chunk=1),
            tiles_event(dict(items[4:]), self.WIDTH, self.HEIGHT, chunk=2),
        ]
        staged, lost = stage(events, top)
        self.assertEqual(
            segments(staged, self.WIDTH),
            flipped_truth(masks, self.WIDTH, top),
            "staged flags must describe the game's segments on the flipped board",
        )
        self.assertEqual(lost, 0, "every segment here has a flipped carrier")

    def test_east_segments_survive_on_their_own_plot(self):
        masks = {(2, 2): 1, (3, 2): 0}
        top = follow.mirror_axis(self.HEIGHT)
        staged, lost = stage([tiles_event(masks, self.WIDTH, self.HEIGHT)], top)
        self.assertEqual(staged, {(2, top - 2): 1})
        self.assertEqual(lost, 0)

    def test_a_known_edge_survives_when_the_opposite_plot_is_hidden(self):
        # A SE flag at the fog frontier. Rust accepts all six directions and
        # stores one-sided boundary edges, so the revealed plot itself carries
        # the reflected fact; the hidden neighbour is not required.
        masks = {(3, 2): 2}
        top = follow.mirror_axis(self.HEIGHT)
        staged, lost = stage([tiles_event(masks, self.WIDTH, self.HEIGHT)], top)
        self.assertEqual(
            segments(staged, self.WIDTH),
            flipped_truth(masks, self.WIDTH, top),
        )
        self.assertEqual(lost, 0)

    def test_all_six_exported_directions_survive_and_reciprocals_deduplicate(self):
        masks = {
            (3, 2): 1 | 2 | 4 | 8 | 16 | 32,
            # Reciprocal copies exported from already-revealed neighbours.
            (4, 2): 8,
            (3, 1): 16,
            (2, 1): 32,
            (2, 2): 1,
            (2, 3): 2,
            (3, 3): 4,
        }
        top = follow.mirror_axis(self.HEIGHT)
        staged, lost = stage([tiles_event(masks, self.WIDTH, self.HEIGHT)], top)
        self.assertEqual(segments(staged, self.WIDTH),
                         flipped_truth(masks, self.WIDTH, top))
        self.assertEqual(lost, 0)

    def test_even_height_staging_keeps_the_polar_row_and_its_edges(self):
        # The extra staging row exists precisely so the final real row has a
        # same-parity reflection partner. Both its in-row and diagonal edges
        # must therefore survive.
        masks = {(3, 5): 1 | 4, (2, 4): 0, (3, 4): 0}
        top = follow.mirror_axis(self.HEIGHT)
        staged, lost = stage([tiles_event(masks, self.WIDTH, self.HEIGHT)], top)
        self.assertEqual(lost, 0)
        self.assertEqual(
            segments(staged, self.WIDTH),
            flipped_truth(masks, self.WIDTH, top),
            "the surviving row carries the gathered segment",
        )

    def test_non_tiles_events_and_riverless_boards_are_untouched(self):
        state = {"kind": "state", "turn": 4, "x": 1, "y": 2}
        dry = tiles_event({(1, 1): 0, (2, 1): 0}, self.WIDTH, self.HEIGHT)
        top = follow.mirror_axis(self.HEIGHT)
        lost = follow.remap_river_masks([state, dry], top)
        self.assertEqual(lost, 0)
        self.assertEqual(state["y"], 2, "remap must not move anything itself")
        self.assertNotIn("rv", dry["plots"][0])


if __name__ == "__main__":
    unittest.main()
