"""The run clock: how long a game takes with CIVVIS at the controls, end to end.

Operator 2026-11-24: "we should be benchmarking our civvis computer control
games time end to end and that number show flow through to a performance
chart". `civ6_play.RUN_CLOCK` is one monotonic clock for a run, started as
`play` is entered and read as the summary is written, with a mark where setup
ends and one where play does. These hold what the summary says of it, and
that the clock is actually wired into the run and both summaries: a reading
nothing starts is not a benchmark.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402
import civ6_play  # noqa: E402

SOURCE = (Path(__file__).resolve().parent / "civ6_play.py").read_text(encoding="utf-8")

CONFIG = {"Difficulty": "DIFFICULTY_DEITY", "MapSize": "MAPSIZE_SMALL",
          "GameSpeed": "GAMESPEED_ONLINE", "MaxTurns": 250}


def body(name: str) -> str:
    """The source of one top-level function of `civ6_play`."""
    start = SOURCE.index(f"\ndef {name}(")
    end = SOURCE.find("\ndef ", start + 1)
    return SOURCE[start:end if end > 0 else len(SOURCE)]


class Clock:
    """A monotonic clock the test moves by hand."""

    def __init__(self, at: float = 1000.0) -> None:
        self.at = at

    def __call__(self) -> float:
        return self.at


class RunClock(unittest.TestCase):
    def setUp(self) -> None:
        self.saved = dict(civ6_play.RUN_CLOCK)
        self.clock = Clock()
        patcher = mock.patch.object(civ6_play.time, "monotonic", self.clock)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.addCleanup(lambda: civ6_play.RUN_CLOCK.update(self.saved))

    def test_a_run_that_never_started_its_clock_has_no_reading(self):
        civ6_play.RUN_CLOCK.update(t0=None, started_utc=None, marks={})
        self.assertIsNone(civ6_play.run_timing())
        civ6_play.run_clock_mark("configured")
        self.assertEqual(civ6_play.RUN_CLOCK["marks"], {})

    def test_the_whole_run_is_split_at_the_two_marks(self):
        civ6_play.run_clock_start()
        self.assertRegex(civ6_play.RUN_CLOCK["started_utc"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
        self.clock.at += 48.26  # Civilization VI launched, the game set up
        civ6_play.run_clock_mark("configured")
        self.clock.at += 680.11  # the game played
        civ6_play.run_clock_mark("ended")
        self.clock.at += 28.04  # the final screen held, the game quit
        timing = civ6_play.run_timing()
        self.assertEqual(timing, {"seconds": 756.4, "setup_seconds": 48.3,
                                  "play_seconds": 680.1, "end_seconds": 28.0})
        # The parts are the whole, to the rounding of each.
        parts = timing["setup_seconds"] + timing["play_seconds"] + timing["end_seconds"]
        self.assertAlmostEqual(parts, timing["seconds"], delta=0.15)

    def test_a_mark_is_the_first_time_it_is_reached(self):
        civ6_play.run_clock_start()
        self.clock.at += 40
        civ6_play.run_clock_mark("configured")
        self.clock.at += 500
        civ6_play.run_clock_mark("configured")
        self.assertEqual(civ6_play.run_timing()["setup_seconds"], 40.0)

    def test_a_run_stopped_early_says_only_what_it_reached(self):
        civ6_play.run_clock_start()
        self.clock.at += 12
        # Stopped while Civilization VI was still loading.
        self.assertEqual(civ6_play.run_timing(), {"seconds": 12.0})
        civ6_play.run_clock_mark("configured")
        self.clock.at += 300
        # Stopped mid-game: the setup is known, the play never ended.
        self.assertEqual(civ6_play.run_timing(), {"seconds": 312.0, "setup_seconds": 12.0})

    def test_a_new_run_starts_from_nothing(self):
        civ6_play.run_clock_start()
        civ6_play.run_clock_mark("configured")
        self.clock.at += 5
        civ6_play.run_clock_start()
        self.assertEqual(civ6_play.run_timing(), {"seconds": 0.0})

    def test_a_reading_that_cannot_be_worked_out_is_none_and_never_raises(self):
        civ6_play.RUN_CLOCK.update(t0="not a time", started_utc=None, marks=None)
        self.assertIsNone(civ6_play.run_timing())

    def test_a_stopped_run_s_summary_carries_the_clock(self):
        civ6_play.run_clock_start()
        self.clock.at += 90.5
        stopped = civ6_play.partial_summary("civvis-x", CONFIG, {"turn": 3, "score": 7})
        self.assertEqual(stopped["timing"], {"seconds": 90.5})
        self.assertEqual(stopped["started_utc"], civ6_play.RUN_CLOCK["started_utc"])
        # And its line in the results file says so.
        row = civ6_ladder.result_row(civ6_ladder.result_entry(stopped))
        self.assertEqual(row["seconds"], 90.5)
        self.assertEqual(row["outcome"], "unfinished")


class TheClockIsWiredIn(unittest.TestCase):
    """A clock nothing starts measures nothing. These read the source: the
    run itself needs Civilization VI and cannot be played in a test."""

    def test_the_clock_starts_as_the_run_does(self):
        play = body("play")
        self.assertIn("run_clock_start()", play)
        # Before anything that takes time: the first statement of `play`.
        first = next(line.strip() for line in play.splitlines()[2:] if line.strip() and not line.strip().startswith("#"))
        self.assertEqual(first, "run_clock_start()")

    def test_setup_ends_where_the_game_is_configured_and_play_where_the_loop_does(self):
        run = body("_play")
        configured = run.index('setup_mark("configured", stop=True)')
        self.assertIn('run_clock_mark("configured")', run[configured:configured + 120])
        ended = run.index("end_t0 = time.monotonic()")
        self.assertIn('run_clock_mark("ended")', run[ended:ended + 120])
        self.assertLess(configured, ended)

    def test_the_summary_of_a_played_game_carries_the_reading(self):
        run = body("_play")
        ended = run.index('run_clock_mark("ended")')
        summary = run.index("summary = {", ended)
        written = run.index('(run_dir / "summary.json").write_text', summary)
        block = run[summary:written]
        self.assertIn('"timing": run_timing()', block)
        self.assertIn('"started_utc": RUN_CLOCK["started_utc"]', block)

    def test_the_ledger_reads_the_names_the_harness_writes(self):
        civ6_play.RUN_CLOCK.update(t0=0.0, started_utc="2026-11-24T15:00:35Z",
                                   marks={"configured": 40.0, "ended": 700.0})
        try:
            timing = civ6_play.run_timing(now=730.0)
        finally:
            civ6_play.RUN_CLOCK.update(t0=None, started_utc=None, marks={})
        self.assertLessEqual(set(civ6_ladder.RUN_SECONDS), set(timing))
        self.assertEqual(civ6_ladder.run_seconds(timing),
                         {"seconds": 730.0, "setup_seconds": 40.0, "play_seconds": 660.0})


if __name__ == "__main__":
    unittest.main()
