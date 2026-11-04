"""Recovery-request observation gaps, without causal cost attribution."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from live_turn_clock import decision_waits, rescue_stalls


def stamp(seconds: float) -> str:
    minute, second = divmod(seconds, 60)
    return f"2026-09-02T10:{int(minute):02d}:{second:06.3f}Z"


class JsonlTestCase(unittest.TestCase):
    def write(self, events) -> Path:
        handle = tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False)
        for event in events:
            handle.write(json.dumps(event) + "\n")
        handle.close()
        self.addCleanup(lambda: Path(handle.name).unlink(missing_ok=True))
        return Path(handle.name)


class RescueStallsTest(JsonlTestCase):
    """Unnumbered recovery requests retain their next-event gap for diagnosis."""

    def test_the_gap_after_each_ask_is_labeled_as_an_observation(self):
        path = self.write([
            {"kind": "state", "utc": stamp(0)},
            {"kind": "autoclose_desktop", "screen": "DiplomacyActionView", "utc": stamp(10)},
            {"kind": "await", "utc": stamp(33.5)},
            {"kind": "autoclose_stuck", "screen": "DiplomacyActionView", "utc": stamp(40)},
            {"kind": "await", "utc": stamp(63.5)},
        ])
        found = rescue_stalls(path)
        self.assertEqual(found["asks"], 2)
        self.assertEqual(found["metric"], "request_to_next_event_gap")
        self.assertAlmostEqual(found["seconds"], 47.0, places=3)
        self.assertAlmostEqual(found["worst"], 23.5, places=3)
        self.assertAlmostEqual(found["screens"]["DiplomacyActionView"], 47.0, places=3)

    def test_a_run_with_no_asks_reports_zero_rather_than_failing(self):
        path = self.write([{"kind": "state", "utc": stamp(0)},
                           {"kind": "turn", "utc": stamp(8)}])
        self.assertEqual(rescue_stalls(path)["asks"], 0)

    def test_unstamped_and_malformed_lines_are_skipped(self):
        handle = tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False)
        handle.write("not json\n")
        handle.write(json.dumps({"kind": "autoclose_desktop"}) + "\n")  # no utc
        handle.write(json.dumps({"kind": "state", "utc": stamp(0)}) + "\n")
        handle.write(json.dumps({"kind": "autoclose_desktop", "utc": stamp(1)}) + "\n")
        handle.write(json.dumps({"kind": "await", "utc": stamp(24.5)}) + "\n")
        handle.close()
        self.addCleanup(lambda: Path(handle.name).unlink(missing_ok=True))
        found = rescue_stalls(Path(handle.name))
        self.assertEqual(found["asks"], 1)
        self.assertAlmostEqual(found["seconds"], 23.5, places=3)

    def test_an_absurd_gap_is_not_counted(self):
        """A run that was killed and resumed leaves an hour between two lines;
        that is outside this diagnostic window."""
        path = self.write([
            {"kind": "autoclose_desktop", "screen": "X", "utc": "2026-09-02T10:00:00.000Z"},
            {"kind": "await", "utc": "2026-09-02T12:00:00.000Z"},
        ])
        self.assertEqual(rescue_stalls(path)["asks"], 0)

    def test_the_span_is_the_whole_run_so_a_share_can_be_taken(self):
        path = self.write([
            {"kind": "state", "utc": stamp(0)},
            {"kind": "autoclose_desktop", "screen": "X", "utc": stamp(10)},
            {"kind": "await", "utc": stamp(33.5)},
            {"kind": "turn", "utc": stamp(100)},
        ])
        found = rescue_stalls(path)
        self.assertAlmostEqual(found["span"], 100.0, places=3)
        self.assertAlmostEqual(100 * found["seconds"] / found["span"], 23.5, places=3)


class DecisionWaitsTest(JsonlTestCase):
    def test_replans_pair_with_their_own_state_and_preserve_unanswered_frames(self):
        path = self.write([
            {"kind": "state", "turn": 4, "utc": stamp(0)},
            {"kind": "orders", "turn": 4, "utc": stamp(0.125)},
            {"kind": "state", "turn": 4, "frame": 1, "utc": stamp(1)},
            {"kind": "orders", "turn": 4, "frame": 1, "utc": stamp(31)},
            {"kind": "state", "turn": 4, "frame": 2, "utc": stamp(32)},
            {"kind": "orders", "turn": 5, "frame": 2, "utc": stamp(33)},
        ])
        found = decision_waits(path)
        self.assertEqual(found["matched"], 2)
        self.assertEqual(found["unanswered"], 1)
        self.assertEqual(found["seconds"], 30.125)
        self.assertEqual(found["samples"][-1], {"turn": 4, "frame": 1, "seconds": 30.0})

    def test_interleaved_receipts_do_not_double_count_wall_time_or_duplicates(self):
        path = self.write([
            {"kind": "state", "turn": 4, "frame": 0, "utc": stamp(0)},
            {"kind": "state", "turn": 4, "frame": 1, "utc": stamp(1)},
            {"kind": "state", "turn": 4, "frame": 0, "utc": stamp(2)},
            {"kind": "orders", "turn": 4, "frame": 1, "utc": stamp(3)},
            {"kind": "orders", "turn": 4, "frame": 0, "utc": stamp(4)},
            {"kind": "orders", "turn": 4, "frame": 0, "utc": stamp(5)},
            {"kind": "state", "turn": 4, "frame": 0, "utc": stamp(6)},
        ])
        found = decision_waits(path)
        self.assertEqual(found["seconds"], 4)
        self.assertEqual(found["matched"], 2)
        self.assertEqual(found["unanswered"], 0)

    def test_invalid_and_backwards_receipts_do_not_make_negative_waits(self):
        path = self.write([
            {"kind": "state", "turn": 4, "utc": stamp(2)},
            {"kind": "orders", "turn": 4, "utc": stamp(1)},
            {"kind": "state", "turn": 5, "frame": "bad", "utc": stamp(3)},
            {"kind": "orders", "turn": 4, "utc": stamp(4)},
            {"kind": "state", "turn": 6},
        ])
        with path.open("a") as handle:
            handle.write('not json\n[]\n')
        found = decision_waits(path)
        self.assertEqual(found["seconds"], 2)
        self.assertEqual(found["matched"], 1)
        self.assertEqual(found["unanswered"], 0)


if __name__ == "__main__":
    unittest.main()
