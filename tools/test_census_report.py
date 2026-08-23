#!/usr/bin/env python3
"""The census reporter has to record what a census actually printed.

Twenty-two `#[ignore]`d censuses had no reader at all. Making them visible is
only worth anything if the reading that gets recorded is the reading — and two
separate bugs in the first drafts recorded an empty one while reporting success.
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import census_report as census  # noqa: E402


class Done:
    def __init__(self, stdout: str, returncode: int = 0):
        self.stdout = stdout
        self.stderr = ""
        self.returncode = returncode


NOCAPTURE = """   Compiling civvis v0.6.0 (/repo)
    Finished `ci` profile [optimized] target(s) in 6.17s
     Running unittests src/lib.rs (target/ci/deps/civvis-abc)

running 1 test
test ai::advanced::tests::belief_pressure_census ... belief-pressure census: 338/22680 city-turns with memory
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2161 filtered out; finished in 30.16s
"""

NOTHING_MATCHED = """     Running unittests src/lib.rs (target/ci/deps/civvis-abc)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2162 filtered out; finished in 0.00s
"""


class TheReadingIsWhatTheCensusPrinted(unittest.TestCase):
    def test_output_on_the_harness_line_is_kept(self):
        """`--nocapture` writes `test name ... ` with NO newline.

        The test's own output lands on the end of that line, so a filter that
        drops lines beginning "test " drops exactly the reading. The first draft
        did, and recorded "(printed nothing)" for a census that had printed a
        perfectly good number.
        """
        with mock.patch.object(census.subprocess, "run", lambda *a, **k: Done(NOCAPTURE)):
            reading = census.run_one("belief_pressure_census", 60)
        self.assertTrue(reading["ok"])
        self.assertEqual(
            reading["output"],
            ["belief-pressure census: 338/22680 city-turns with memory"],
        )

    def test_a_filter_that_matched_nothing_is_not_a_silent_pass(self):
        """`--exact` wants the full module path, and cargo exits 0 either way.

        Passing a bare function name to `--exact` matches nothing, cargo prints
        "running 0 tests" and returns success, and the census records an empty
        reading indistinguishable from a census that printed nothing.
        """
        with mock.patch.object(census.subprocess, "run",
                               lambda *a, **k: Done(NOTHING_MATCHED)):
            reading = census.run_one("belief_pressure_census", 60)
        self.assertFalse(reading["ok"])
        self.assertIn("no test matched", reading["output"][0])

    def test_durations_are_not_recorded_as_drift(self):
        noisy = NOCAPTURE.replace(
            "belief-pressure census: 338/22680 city-turns with memory",
            "census done in 12.5s")
        with mock.patch.object(census.subprocess, "run", lambda *a, **k: Done(noisy)):
            reading = census.run_one("x", 60)
        self.assertEqual(reading["output"], [],
                         "a wall-clock number changes on every machine and "
                         "would report as drift forever")

    def test_the_run_does_not_pass_exact(self):
        seen = {}

        def capture(argv, **kwargs):
            seen["argv"] = argv
            return Done(NOCAPTURE)

        with mock.patch.object(census.subprocess, "run", capture):
            census.run_one("belief_pressure_census", 60)
        self.assertNotIn("--exact", seen["argv"])
        self.assertIn("--ignored", seen["argv"])
        self.assertIn("--nocapture", seen["argv"])


class DriftIsTheSignal(unittest.TestCase):
    """A census is a reading. The number may move; it may not move silently."""

    def ledger(self, tmp: Path, readings: dict) -> None:
        (tmp / "census.json").write_text(json.dumps(readings))

    def check(self, tmp: Path, now: dict) -> int:
        with mock.patch.object(census, "LEDGER", tmp / "census.json"), \
             mock.patch.object(census, "MARKDOWN", tmp / "CENSUS.md"), \
             mock.patch.object(census, "take", lambda timeout, only: now):
            return census.main(["--check"])

    def test_an_unchanged_reading_passes(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            reading = {"a": {"ok": True, "output": ["n = 7"]}}
            self.ledger(tmp, reading)
            self.assertEqual(self.check(tmp, reading), 0)

    def test_a_changed_number_fails(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            self.ledger(tmp, {"a": {"ok": True, "output": ["n = 7"]}})
            self.assertEqual(self.check(tmp, {"a": {"ok": True, "output": ["n = 8"]}}), 1)

    def test_a_new_census_fails_until_it_is_recorded(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            self.ledger(tmp, {})
            self.assertEqual(self.check(tmp, {"a": {"ok": True, "output": ["n = 7"]}}), 1)

    def test_a_census_that_disappears_fails(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            self.ledger(tmp, {"a": {"ok": True, "output": ["n = 7"]},
                              "b": {"ok": True, "output": ["m = 1"]}})
            self.assertEqual(self.check(tmp, {"a": {"ok": True, "output": ["n = 7"]}}), 1)


class TheDiscoveryFindsThemAll(unittest.TestCase):
    def test_it_finds_the_censuses_in_this_repository(self):
        found = census.censuses()
        self.assertGreaterEqual(len(found), 15, found)
        names = {row["test"] for row in found}
        self.assertIn("belief_pressure_census", names)

    def test_a_plain_ignore_without_a_reason_is_not_a_census(self):
        """`#[ignore]` alone is an ordinary skipped test and not this tool's business."""
        self.assertIsNone(census.CENSUS_NOTE.search("    #[ignore]"))
        self.assertIsNone(
            census.CENSUS_NOTE.search('    #[ignore = "flaky on CI"]'))
        self.assertIsNotNone(
            census.CENSUS_NOTE.search('    #[ignore = "census, not an assertion"]'))


class ATransientFailureIsRetriedBeforeItIsBelieved(unittest.TestCase):
    """These run for minutes on a machine that is also playing Civilization VI.

    A cargo invocation that loses a build lock returns nonzero without the
    census having failed. Recording that bakes `ok: false` into the baseline and
    every later run reports drift when the census simply passes again — measured
    on the first full run here, where `expansion_funnel_blocker_census` recorded
    a failure and passed on every attempt afterwards.
    """

    def take(self, results):
        calls = {"n": 0}

        def flaky(test, timeout):
            calls["n"] += 1
            return results[min(calls["n"], len(results)) - 1]

        entry = [{"test": "a", "file": "x.rs", "line": 1, "note": "census"}]
        with mock.patch.object(census, "run_one", flaky), \
             mock.patch.object(census, "censuses", lambda: entry):
            readings = census.take(60, None)
        return readings["a"], calls["n"]

    def test_a_failure_that_passes_on_retry_is_recorded_as_a_pass(self):
        reading, calls = self.take([{"ok": False, "output": ["boom"]},
                                    {"ok": True, "output": ["n = 7"]}])
        self.assertTrue(reading["ok"])
        self.assertEqual(reading["output"], ["n = 7"])
        self.assertEqual(calls, 2)

    def test_a_failure_twice_is_believed(self):
        reading, calls = self.take([{"ok": False, "output": ["boom"]},
                                    {"ok": False, "output": ["boom"]}])
        self.assertFalse(reading["ok"])
        self.assertEqual(calls, 2, "two attempts, not more")

    def test_a_pass_is_not_run_twice(self):
        _, calls = self.take([{"ok": True, "output": ["n = 7"]}])
        self.assertEqual(calls, 1, "a census that passed must not run again")


class FilteringNarrowsBothSides(unittest.TestCase):
    def test_only_compares_the_censuses_it_ran(self):
        """Comparing a filtered run against the whole ledger is 21 false alarms."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            (tmp / "census.json").write_text(json.dumps({
                "wanted": {"ok": True, "output": ["n = 7"]},
                "other": {"ok": True, "output": ["m = 1"]},
            }))
            with mock.patch.object(census, "LEDGER", tmp / "census.json"), \
                 mock.patch.object(census, "MARKDOWN", tmp / "CENSUS.md"), \
                 mock.patch.object(census, "take",
                                   lambda timeout, only: {"wanted": {"ok": True,
                                                                     "output": ["n = 7"]}}):
                code = census.main(["--check", "--only", "wanted"])
        self.assertEqual(code, 0)


class AStopwatchIsNotADeterminismReading(unittest.TestCase):
    """The reason the gate could not go green even with a fresh baseline.

    `.github/workflows/census.yml` compares a macOS baseline on Linux and calls
    a difference a determinism break. `sphere_distance_cache_order_benchmark`
    prints `median_elapsed_ns` straight off `Instant::elapsed()`, which differs
    between two runs on one machine. Pinned, it is a red X on every run forever,
    standing next to twenty-seven that would mean something.
    """

    TIMED = "preregistered microbenchmark; run explicitly with --nocapture"
    COUNTED = "census, not an assertion; run explicitly with --nocapture"

    def check(self, tmp: Path, now: dict) -> int:
        with mock.patch.object(census, "LEDGER", tmp / "census.json"), \
             mock.patch.object(census, "MARKDOWN", tmp / "CENSUS.md"), \
             mock.patch.object(census, "take", lambda timeout, only: now):
            return census.main(["--check"])

    def reading(self, note: str, ns: int, ok: bool = True) -> dict:
        return {"a": {"ok": ok, "note": note, "file": "src/sphere.rs", "line": 1,
                      "output": [f"cold_local: median_elapsed_ns={ns}"]}}

    def test_the_note_is_what_says_a_reading_is_a_stopwatch(self):
        self.assertTrue(census.is_stopwatch(self.TIMED))
        self.assertFalse(census.is_stopwatch(self.COUNTED))
        self.assertFalse(census.is_stopwatch(""))
        self.assertFalse(census.is_stopwatch(None))

    def test_the_live_repository_has_one_and_it_is_the_sphere_benchmark(self):
        """Pinned on the repository, not on a fixture: if this stops matching,
        the workflow silently goes back to asserting nanoseconds."""
        timed = [row["test"] for row in census.censuses()
                 if census.is_stopwatch(row["note"])]
        self.assertEqual(timed, ["sphere_distance_cache_order_benchmark"])

    def test_a_timing_reading_that_moved_does_not_fail_the_gate(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            (tmp / "census.json").write_text(
                json.dumps(self.reading(self.TIMED, 26_225_791)))
            self.assertEqual(self.check(tmp, self.reading(self.TIMED, 31_004_112)), 0)

    def test_the_same_movement_in_a_counted_census_still_fails(self):
        """The exemption is the note, not the shape of the number."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            (tmp / "census.json").write_text(
                json.dumps(self.reading(self.COUNTED, 26_225_791)))
            self.assertEqual(self.check(tmp, self.reading(self.COUNTED, 31_004_112)), 1)

    def test_a_timing_census_that_starts_failing_still_fails_the_gate(self):
        """Its own assertions are the signal that survives. The sphere benchmark
        asserts that eight distinct long queries admit the reused source row, so
        the regression it exists to watch still turns this red."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            (tmp / "census.json").write_text(
                json.dumps(self.reading(self.TIMED, 26_225_791)))
            self.assertEqual(
                self.check(tmp, self.reading(self.TIMED, 26_225_791, ok=False)), 1)

    def test_a_timing_census_is_still_recorded_before_it_is_exempt(self):
        """Skipping the comparison must not become a way to never appear."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            (tmp / "census.json").write_text(json.dumps({}))
            self.assertEqual(self.check(tmp, self.reading(self.TIMED, 1)), 1)

    def test_the_rendered_page_says_the_numbers_are_not_compared(self):
        page = census.render(self.reading(self.TIMED, 26_225_791))
        self.assertIn("26225791", page, "the numbers stay visible")
        self.assertIn("ran and passed", page)
        self.assertNotIn(
            "ran and passed", census.render(self.reading(self.COUNTED, 1)))


if __name__ == "__main__":
    unittest.main()
