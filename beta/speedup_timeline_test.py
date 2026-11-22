#!/usr/bin/env python3
"""The home page's speed timeline: the ledger of readings, what
`speedup_timeline.py` works out from it, and the page that shows it."""

from __future__ import annotations

import contextlib
import io
import json
import pathlib
import re
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import home_rankings  # noqa: E402
import speedup_timeline  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[1]
LANDING = (ROOT / "beta/landing.html").read_text(encoding="utf-8")
LEDGER = json.loads((ROOT / speedup_timeline.LEDGER).read_text(encoding="utf-8"))
BENCHMARK = "data/performance/games_per_minute.json"


def tree(scratch: str, ledger: dict | None, runs: list[dict] | None = None) -> pathlib.Path:
    """A source tree holding this ledger and these benchmark runs."""
    source = pathlib.Path(scratch)
    (source / "data/performance").mkdir(parents=True, exist_ok=True)
    if ledger is not None:
        (source / speedup_timeline.LEDGER).write_text(json.dumps(ledger), encoding="utf-8")
    if runs is not None:
        (source / BENCHMARK).write_text(json.dumps({"runs": runs}), encoding="utf-8")
    return source


class Words(unittest.TestCase):
    def test_a_time_is_worded_by_its_size(self):
        for seconds, words in [
            (0.018, "18 ms"), (0.9, "900 ms"), (1, "1 s"), (3.273, "3.3 s"), (9.94, "9.9 s"), (9.96, "10 s"), (21.4, "21 s"),
            (99.4, "99 s"), (99.6, "1.7 min"), (600, "10 min"), (757, "12.6 min"), (5969, "99.5 min"),
            (5970, "1.7 h"), (18000, "5 h"), (19800, "5.5 h"),
        ]:
            self.assertEqual(speedup_timeline.clock(seconds), words, seconds)

    def test_a_multiplier_is_a_whole_number_with_its_thousands_marked(self):
        for multiplier, words in [
            (1, "1x"), (1.02, "1x"), (2.5, "2.5x"), (9.9, "9.9x"), (9.96, "10x"), (23.78, "24x"),
            (5498.9999, "5,499x"), (18000, "18,000x"), (1000000, "1,000,000x"),
        ]:
            self.assertEqual(speedup_timeline.times(multiplier), words, multiplier)


class Ledger(unittest.TestCase):
    def test_every_stop_is_measured(self):
        # A stop is a key and either its own dated readings or the ledger it
        # follows; a reading is a day, the seconds one game takes and how
        # that was measured.
        keys = [stop["key"] for stop in LEDGER["stops"]]
        self.assertEqual(len(keys), len(set(keys)))
        self.assertGreaterEqual(len(keys), 2)
        for stop in LEDGER["stops"]:
            self.assertNotEqual("readings" in stop, "follows" in stop, stop["key"])
            if "follows" in stop:
                self.assertIn(stop["follows"], speedup_timeline.FOLLOWED)
                self.assertTrue((ROOT / stop["follows"]).is_file(), stop["follows"])
                continue
            self.assertTrue(stop["readings"], stop["key"])
            days = [reading["taken"] for reading in stop["readings"]]
            self.assertEqual(days, sorted(days), f"{stop['key']}: the readings are kept oldest first")
            for reading in stop["readings"]:
                self.assertRegex(reading["taken"], r"^\d{4}-\d{2}-\d{2}$")
                self.assertGreater(reading["seconds"], 0)
                self.assertGreater(len(reading["from"]), 20, f"{stop['key']}: a reading says how it was measured")
        # Nothing derived is kept: a multiplier written here would be a copy.
        self.assertNotIn("multiplier", json.dumps(LEDGER["stops"]))
        self.assertGreater(LEDGER["goal"], 1)

    def test_the_first_stop_is_the_one_the_others_are_measured_against(self):
        shown = speedup_timeline.timeline(ROOT)
        first = shown["stops"][0]
        self.assertEqual((first["key"], first["times"], first["at"]), ("hand", "1x", 0.0))
        base = first["seconds"]
        for stop in shown["stops"]:
            self.assertAlmostEqual(stop["multiplier"], base / stop["seconds"], delta=0.001 * stop["multiplier"])
            self.assertEqual(stop["times"], speedup_timeline.times(stop["multiplier"]))
            self.assertEqual(stop["time"], speedup_timeline.clock(stop["seconds"]))

    def test_the_page_s_number_is_the_fastest_stop_s_multiplier(self):
        shown = speedup_timeline.timeline(ROOT)
        fastest = max(shown["stops"], key=lambda stop: stop["multiplier"])
        self.assertEqual(shown["speedup"], fastest["times"])
        self.assertEqual(shown["fastest"], fastest["key"])
        self.assertEqual(shown["reach"], fastest["at"])
        self.assertEqual(shown["goal"], speedup_timeline.times(LEDGER["goal"]))
        base = shown["stops"][0]["seconds"]
        self.assertEqual(shown["goal_time"], speedup_timeline.clock(base / LEDGER["goal"]))
        self.assertRegex(shown["speedup"], r"^\d{1,3}(,\d{3})*x$")

    def test_every_core_follows_the_games_per_minute_record(self):
        # The stop that plays on every core is the record of
        # data/performance/games_per_minute.json, so a new record there
        # moves the home page with no second edit.
        runs = json.loads((ROOT / BENCHMARK).read_text(encoding="utf-8"))["runs"]
        record = max(run["games_per_minute"] for run in runs)
        shown = {stop["key"]: stop for stop in speedup_timeline.timeline(ROOT)["stops"]}
        base = shown["hand"]["seconds"]
        self.assertAlmostEqual(shown["all_cores"]["seconds"], 60 / record, places=3)
        self.assertEqual(shown["all_cores"]["times"], speedup_timeline.times(base * record / 60))


class Following(unittest.TestCase):
    LEDGER = {
        "goal": 1000000,
        "stops": [
            {"key": "hand", "readings": [{"taken": "2026-11-22", "seconds": 18000, "from": "a person's own games"}]},
            {"key": "self_play", "goal_seconds": 10, "readings": [
                {"taken": "2026-11-22", "seconds": 40, "from": "twelve games on one core"},
                {"taken": "2026-12-01", "seconds": 20, "from": "twelve games on one core"},
            ]},
            {"key": "all_cores", "goal_seconds": 1, "follows": BENCHMARK},
        ],
    }
    RUNS = [
        {"taken": "2026-11-22T05:32:38Z", "games_per_minute": 18.0},
        {"taken": "2026-11-23T05:00:00Z", "games_per_minute": 17.0},
        {"taken": "2026-11-25T05:00:00Z", "games_per_minute": 30.0},
        {"taken": "2026-11-26T05:00:00Z", "games_per_minute": 24.0},
    ]

    def test_a_followed_ledger_s_records_are_the_readings(self):
        # A run slower than one before it is not a reading, so the newest
        # reading is the record that stands.
        took = speedup_timeline.records(self.RUNS, speedup_timeline.FOLLOWED[BENCHMARK])
        self.assertEqual([reading["taken"] for reading in took], ["2026-11-22", "2026-11-25"])
        self.assertEqual(took[-1]["seconds"], 2.0)
        # A reading says how it was measured, as a kept one does.
        self.assertTrue(took[-1]["from"].startswith("30 games a minute, the record of `tools/games_per_minute.py --record`"))

    def test_a_stop_shows_its_newest_reading_its_goal_and_where_it_started(self):
        with tempfile.TemporaryDirectory() as scratch:
            shown = speedup_timeline.timeline(tree(scratch, self.LEDGER, self.RUNS))
        stops = {stop["key"]: stop for stop in shown["stops"]}
        self.assertEqual([stop["key"] for stop in shown["stops"]], ["hand", "self_play", "all_cores"])
        self.assertEqual((stops["self_play"]["time"], stops["self_play"]["times"]), ("20 s", "900x"))
        self.assertEqual(stops["self_play"]["was"], "450x on 11-22")
        self.assertEqual(stops["self_play"]["goal"], "goal 10 s, 1,800x")
        self.assertEqual(stops["self_play"]["taken"], "12-01")
        self.assertEqual((stops["all_cores"]["time"], stops["all_cores"]["times"]), ("2 s", "9,000x"))
        self.assertEqual(stops["all_cores"]["was"], "5,400x on 11-22")
        self.assertEqual(stops["all_cores"]["goal"], "goal 1 s, 18,000x")
        self.assertIn("30 games a minute", stops["all_cores"]["from"])
        # A stop with one reading has not moved, and one with no goal has none.
        self.assertNotIn("was", stops["hand"])
        self.assertNotIn("goal", stops["hand"])
        self.assertEqual((shown["speedup"], shown["fastest"], shown["was"]), ("9,000x", "all_cores", "5,400x on 11-22"))
        self.assertEqual((shown["goal"], shown["goal_time"]), ("1,000,000x", "18 ms"))
        # The line runs by tenfolds from 1x at one end to the goal at the other.
        self.assertEqual([tick["times"] for tick in shown["ticks"]],
                         ["1x", "10x", "100x", "1,000x", "10,000x", "100,000x", "1,000,000x"])
        self.assertEqual([tick["at"] for tick in shown["ticks"]], [0.0, 0.1667, 0.3333, 0.5, 0.6667, 0.8333, 1.0])
        self.assertAlmostEqual(stops["all_cores"]["at"], 0.659, places=3)

    def test_a_revision_with_no_ledger_shows_nothing(self):
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.timeline(tree(scratch, None)))
        # Nor does one whose followed ledger is missing, or whose ledger is
        # not the shape this reads.
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.timeline(tree(scratch, self.LEDGER)))
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.timeline(tree(scratch, {"goal": 1000000, "stops": []})))

    def test_stops_that_stand_close_together_take_opposite_sides(self):
        # Self-play and the Deity sim are one engine and stand a few pixels
        # apart, so their words go one above the line and one below.
        self.assertEqual(speedup_timeline.sides([0.0, 0.23, 0.44, 0.45, 0.62]),
                         ["below", "above", "below", "above", "below"])
        # In whatever order the ledger lists them.
        self.assertEqual(speedup_timeline.sides([0.45, 0.0, 0.44]), ["below", "below", "above"])
        shown = speedup_timeline.timeline(ROOT)
        by_side: dict[str, list[float]] = {"above": [], "below": []}
        for stop in shown["stops"]:
            by_side[stop["side"]].append(stop["at"])
        for side, places in by_side.items():
            places.sort()
            for near, far in zip(places, places[1:]):
                # Words are 150px wide on a line of at least 936px.
                self.assertGreaterEqual(far - near, 150 / 936, f"two stops' words meet {side} the line")


class Page(unittest.TestCase):
    def test_the_page_has_an_entry_for_every_stop_and_no_figure_of_its_own(self):
        entries = re.findall(r'<li class="speed-stop" data-stop="(\w+)">', LANDING)
        self.assertEqual(entries, [stop["key"] for stop in LEDGER["stops"]])
        self.assertEqual(LANDING.count('<script type="application/json" id="home-speedup">null</script>'), 1)
        # The timeline is over the videos, and hidden until it is filled.
        section = LANDING[LANDING.index('<section class="speed"') : LANDING.index('<section class="watch"')]
        self.assertIn('<section class="speed" id="speed" aria-label="How many times faster than a person a game is played" hidden>', section)
        self.assertIn("section.hidden = false;", LANDING)
        for place in ("speedup", "was", "target", "target-time"):
            self.assertEqual(section.count(f'data-speed="{place}"'), 1, place)
        # The goal is the line's last entry, at its left end.
        self.assertLess(section.rindex('data-stop="'), section.index('<li class="speed-stop speed-finish is-above is-start">'))
        self.assertIn(".speed-finish { right: 100%; }", LANDING)
        for place in ("time", "times", "goal"):
            self.assertEqual(section.count(f'data-speed="{place}"'), len(entries), place)
        # No figure is written into the page: every one is the slot's.
        text = re.sub(r"<!--.*?-->", "", section, flags=re.S)
        self.assertIsNone(re.search(r"\d[\d,.]*\s*(x|s|min|h)\b", re.sub(r"<[^>]+>", " ", text)))
        # 1x is at the right end of the line and the goal at the left.
        self.assertIn(".speed-stop { position: absolute; right: calc(var(--at) * 100%);", LANDING)
        self.assertIn("width: calc(var(--reach, 0) * 100%);", LANDING)
        self.assertIn('entry.style.setProperty("--at", stop.at);', LANDING)
        self.assertNotIn("innerHTML", LANDING)

    def test_publishing_writes_the_timeline_into_its_slot(self):
        shown = speedup_timeline.timeline(ROOT)
        with tempfile.TemporaryDirectory() as scratch:
            page = pathlib.Path(scratch) / "index.html"
            page.write_text(LANDING, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()) as said:
                self.assertEqual(home_rankings.main(["home_rankings.py", str(ROOT), str(page)]), 0)
            self.assertIn("the speed timeline", said.getvalue())
            filled = page.read_text(encoding="utf-8")
            self.assertEqual(json.loads(speedup_timeline.SLOT.search(filled)[2]), shown)
            # Nothing but the page's two slots moved.
            bare = lambda text: home_rankings.VIDEOS_SLOT.sub("", speedup_timeline.SLOT.sub("", text))  # noqa: E731
            self.assertEqual(bare(filled), bare(LANDING))
            # A revision with no ledger: the slot says so and the section
            # stays hidden.
            page.write_text(LANDING, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()) as said:
                self.assertEqual(home_rankings.main(["home_rankings.py", scratch, str(page)]), 0)
            self.assertIn("no speed timeline", said.getvalue())
            self.assertIn('<script type="application/json" id="home-speedup">null</script>', page.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
