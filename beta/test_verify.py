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


class UncaughtTests(unittest.TestCase):
    def test_a_thrown_error_fails_and_a_warning_does_not(self):
        thrown = "TypeError: bad value\n    at tone (app_moments.js:41:17)"
        self.assertEqual(verify.uncaught_problems(["a warning", "another"]), [])
        problems = verify.uncaught_problems(["a warning", thrown, thrown])
        self.assertEqual(len(problems), 1)
        self.assertIn("threw 2 time(s)", problems[0])
        self.assertIn("TypeError: bad value", problems[0])

    def test_the_check_runs_with_sound_on(self):
        source = open(verify.__file__, encoding="utf-8").read()
        self.assertIn('"--autoplay-policy=no-user-gesture-required"', source)

    def test_the_check_builds_no_audio_context(self):
        # On a runner with no sound device this hung and failed every deploy.
        source = open(verify.__file__, encoding="utf-8").read()
        self.assertNotIn("await Promise.race", source)


class MapCanvasTests(unittest.TestCase):
    def test_the_painted_check_reads_the_map_canvas_by_id(self):
        # The first canvas in the document became the WebGL surface under the
        # map, which has no 2D context: every publish failed with -1 colours.
        self.assertIn("document.getElementById('map')", verify.MAP_CANVAS_COLOURS_JS)
        self.assertNotIn("querySelector('canvas')", verify.MAP_CANVAS_COLOURS_JS)

    def test_the_viewer_still_names_its_map_canvas(self):
        index = verify.pathlib.Path(verify.__file__).resolve().parent.parent
        html = (index / "web" / "index.html").read_text(encoding="utf-8")
        self.assertEqual(html.count('<canvas id="map">'), 1)


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
