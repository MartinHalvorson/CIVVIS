#!/usr/bin/env python3
"""Focused contracts for the published browser verifier."""

import unittest

import verify


def map_report(
    *, full_frames=1, patch_frames=1, full_tiles=100, patch_tiles=5,
    full_bytes=1_000, patch_bytes=100,
):
    return {
        "fullMapFrames": full_frames,
        "patchFrames": patch_frames,
        "fullMapTiles": full_tiles,
        "patchTiles": patch_tiles,
        "fullMapBytes": full_bytes,
        "patchBytes": patch_bytes,
    }


class MapDeliveryTests(unittest.TestCase):
    def test_current_lane_requires_a_patch(self):
        problems = verify.map_delivery_problems(
            map_report(patch_frames=0, patch_tiles=0, patch_bytes=0)
        )
        self.assertEqual(
            problems,
            ["the browser never received a map patch after its first frame"],
        )

    def test_pinned_stable_lane_may_predate_patches(self):
        problems = verify.map_delivery_problems(
            map_report(patch_frames=0, patch_tiles=0, patch_bytes=0),
            allow_legacy_full_map=True,
        )
        self.assertEqual(problems, [])

    def test_every_lane_still_requires_an_initial_complete_map(self):
        problems = verify.map_delivery_problems(
            map_report(full_frames=0, full_tiles=0, patch_frames=0),
            allow_legacy_full_map=True,
        )
        self.assertEqual(
            problems,
            ["the browser never received its initial complete map"],
        )

    def test_inefficient_patches_are_never_exempt(self):
        problems = verify.map_delivery_problems(
            map_report(patch_tiles=100, patch_bytes=1_000),
            allow_legacy_full_map=True,
        )
        self.assertEqual(len(problems), 2)
        self.assertIn("tiles per frame", problems[0])
        self.assertIn("bytes per frame", problems[1])

    def test_compact_patches_pass(self):
        self.assertEqual(verify.map_delivery_problems(map_report()), [])


if __name__ == "__main__":
    unittest.main()
