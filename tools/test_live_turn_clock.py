"""What the desktop rescue costs a run, reported by the tool built to find it."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from live_turn_clock import rescue_stalls


def stamp(seconds: float) -> str:
    minute, second = divmod(seconds, 60)
    return f"2026-09-02T10:{int(minute):02d}:{second:06.3f}Z"


class RescueStallsTest(unittest.TestCase):
    """★ THE ONE NUMBER THE PER-TURN TABLE HIDES.

    An `autoclose` event carries no turn, so a stall lands inside whichever
    turn spans it and reads as a slow turn. Measured directly it was 25.8 min
    of run civvis-20260902T095330Z's 68.6 -- 37.6 %.
    """

    def write(self, events) -> Path:
        handle = tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False)
        for event in events:
            handle.write(json.dumps(event) + "\n")
        handle.close()
        self.addCleanup(lambda: Path(handle.name).unlink(missing_ok=True))
        return Path(handle.name)

    def test_the_gap_after_each_ask_is_the_cost(self):
        path = self.write([
            {"kind": "state", "utc": stamp(0)},
            {"kind": "autoclose_desktop", "screen": "DiplomacyActionView", "utc": stamp(10)},
            {"kind": "await", "utc": stamp(33.5)},
            {"kind": "autoclose_stuck", "screen": "DiplomacyActionView", "utc": stamp(40)},
            {"kind": "await", "utc": stamp(63.5)},
        ])
        found = rescue_stalls(path)
        self.assertEqual(found["asks"], 2)
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

    def test_an_absurd_gap_is_not_counted_as_rescue_time(self):
        """A run that was killed and resumed leaves an hour between two lines;
        that is not what the rescue cost."""
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


if __name__ == "__main__":
    unittest.main()
