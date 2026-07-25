"""Tests for the frame-coverage checker's analysis."""
from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from civvis_frames import gaps  # noqa: E402


class GapsTest(unittest.TestCase):
    def test_a_viewer_that_saw_every_turn_has_no_gaps(self):
        self.assertEqual(gaps([4, 5, 6, 7]), [])

    def test_polling_twice_inside_one_turn_is_not_a_gap(self):
        # The page redraws whatever the server hands it, so a pace slower than
        # the poll gives the same turn back several times over.
        self.assertEqual(gaps([4, 4, 5, 5, 5, 6]), [])

    def test_turns_that_never_arrived_are_reported(self):
        self.assertEqual(gaps([4, 6, 9]), [5, 7, 8])

    def test_only_the_span_actually_watched_counts(self):
        # Turns before the viewer attached and after it stopped are nobody's
        # missed frames; the run is judged between its own first and last.
        self.assertEqual(gaps([100, 101, 102]), [])

    def test_no_observations_is_not_a_claim_either_way(self):
        self.assertEqual(gaps([]), [])


if __name__ == "__main__":
    unittest.main()
