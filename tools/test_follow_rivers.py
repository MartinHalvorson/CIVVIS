#!/usr/bin/env python3
"""The north-up reflection must carry the rivers with it.

`rv` bit 1 is `IsWOfRiver` (river on the EAST edge), bit 2 `IsNWOfRiver`
(SOUTH-EAST edge), bit 4 `IsNEOfRiver` (SOUTH-WEST edge). The reconstruction
applies that table to staged coordinates, so after `flip_north_up` the two
diagonal flags describe the vertically mirrored edge unless
`remap_river_masks` has moved them to the plot that owns the segment on the
flipped board. These tests pin the composition, not either half: consumer
semantics applied to the staged plots must equal the game's own segments with
both endpoints flipped.
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
    for (x, y), rv in masks.items():
        par = y & 1
        if rv & 1:
            segs.add(frozenset(((x, y), ((x + 1) % width, y))))
        if rv & 2:
            segs.add(frozenset(((x, y), ((x + par) % width, y - 1))))
        if rv & 4:
            segs.add(frozenset(((x, y), ((x - 1 + par) % width, y - 1))))
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
    WIDTH, HEIGHT = 8, 6      # height 6 -> top 4, row 5 is the dropped polar row

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

    def test_a_flag_with_no_carrier_is_counted_not_silent(self):
        # A SE flag at the fog frontier: the plot below is not exported, so on
        # the flipped board nothing can carry the segment. It must be counted,
        # and it must NOT survive on the mirrored-wrong edge.
        masks = {(3, 2): 2}
        top = follow.mirror_axis(self.HEIGHT)
        staged, lost = stage([tiles_event(masks, self.WIDTH, self.HEIGHT)], top)
        self.assertEqual(staged, {}, "the mirrored-wrong edge is worse than absence")
        self.assertEqual(lost, 1)

    def test_the_dropped_polar_row_counts_its_east_segments(self):
        # Row 5 falls off a 6-row board. Its E segment leaves with it; its SW
        # flag is gathered by row 4 and survives as that plot's segment.
        masks = {(3, 5): 1 | 4, (2, 4): 0, (3, 4): 0}
        top = follow.mirror_axis(self.HEIGHT)
        staged, lost = stage([tiles_event(masks, self.WIDTH, self.HEIGHT)], top)
        self.assertEqual(lost, 1, "only the in-row E segment is unrepresentable")
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
