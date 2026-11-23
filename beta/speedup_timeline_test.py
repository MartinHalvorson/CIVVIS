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


def tree(scratch: str, ledger: dict | None, runs: list[dict] | None = None, listed: dict | None = None) -> pathlib.Path:
    """A source tree holding this ledger, these benchmark runs and this list
    of processors."""
    source = pathlib.Path(scratch)
    (source / "data/performance").mkdir(parents=True, exist_ok=True)
    if ledger is not None:
        (source / speedup_timeline.LEDGER).write_text(json.dumps(ledger), encoding="utf-8")
    if runs is not None:
        (source / BENCHMARK).write_text(json.dumps({"runs": runs}), encoding="utf-8")
    if listed is not None:
        (source / speedup_timeline.PROCESSORS).write_text(json.dumps(listed), encoding="utf-8")
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


def done_again(said: str) -> float:
    """A sum as the page writes it, worked out: `18 cores × 18,000 s ÷ 57.1 s`
    with its words dropped and its signs read."""
    bare = re.sub(r"[A-Za-z]+", " ", re.sub(r"(?<=\d),(?=\d)", "", said)).replace("×", "*").replace("÷", "/")
    assert re.fullmatch(r"[\d\s.*/()]+", bare), said
    return eval(bare)  # noqa: S307 (digits and signs only, checked above)


class Worked(unittest.TestCase):
    """The CIVVIS Performance page's tables (operator 2026-11-22: "at the top
    of civvis performance we want to calculate a multiplier on the
    theoretical and experimetnal throughput of the simualtor. and the
    calculations. baeline is 5 hrs for manually played game. please add the
    deity computer control, deity self-play, civvis self-play.")."""

    LEDGER = {
        "goal": 1000000,
        "stops": [
            {"key": "hand", "readings": [{"taken": "2026-11-22", "seconds": 18000, "from": "a person's own games"}]},
            {"key": "control", "readings": [{"taken": "2026-11-22", "seconds": 750, "from": "the middle verification game"}]},
            {"key": "deity_sim", "cores": 1, "readings": [{"taken": "2026-11-22", "seconds": 30, "from": "twelve games on one core"}]},
            {"key": "self_play", "cores": 1, "goal_seconds": 10,
             "alone": [
                 {"taken": "2026-11-20", "turns": 100, "seconds": 50, "from": "an older game alone"},
                 {"taken": "2026-11-22", "turns": 200, "seconds": 40, "from": "one game with the machine to itself"},
             ],
             "readings": [
                 {"taken": "2026-11-22", "seconds": 90, "from": "twelve games on one core"},
                 {"taken": "2026-12-01", "seconds": 60, "from": "twelve games on one core"},
             ]},
            {"key": "all_cores", "of": "self_play", "goal_seconds": 1, "follows": BENCHMARK},
        ],
    }
    RUNS = [
        {"taken": "2026-11-22T05:32:38Z", "games_per_minute": 6.0, "workers": 10, "mean_turns": 150.0},
        {"taken": "2026-11-25T05:00:00Z", "games_per_minute": 8.0, "workers": 10, "mean_turns": 160.0,
         "machine": {"chip": "Apple M5 Max", "cores": 10}, "conditions": {"workers_cpu_cores_sampled": 9.5}},
        {"taken": "2026-11-26T05:00:00Z", "games_per_minute": 7.0, "workers": 10, "mean_turns": 150.0},
    ]

    def shown(self, ledger: dict | None = None, runs: list[dict] | None = None) -> dict | None:
        with tempfile.TemporaryDirectory() as scratch:
            return speedup_timeline.worked(tree(scratch, ledger or self.LEDGER, self.RUNS if runs is None else runs))

    def test_a_rate_a_figure_and_a_share_are_worded_by_their_size(self):
        for value, words in [(0.2, "0.2"), (4.756, "4.8"), (27.0, "27"), (63.05, "63"), (99.94, "99.9"), (99.96, "100"), (1099.8, "1,100"), (200000, "200,000")]:
            self.assertEqual(speedup_timeline.rate(value), words, value)
        for value, words in [(757, "757"), (57.1, "57.1"), (29.1, "29.1"), (18.33, "18.33"), (194.7, "194.7"), (5, "5"), (18000, "18,000"), (999.4, "999.4")]:
            self.assertEqual(speedup_timeline.figure(value), words, value)
        for part, words in [(0.9692, "97%"), (0.679, "68%"), (0.3055, "31%"), (0.0057, "0.6%"), (0.005499, "0.5%"), (1.0, "100%")]:
            self.assertEqual(speedup_timeline.share(part), words, part)

    def test_one_game_at_a_time_is_every_stop_with_readings_of_its_own(self):
        shown = self.shown()
        self.assertEqual([row["key"] for row in shown["games"]], ["hand", "control", "deity_sim", "self_play"])
        hand, control, deity, self_play = shown["games"]
        self.assertEqual((hand["time"], hand["an_hour"], hand["times"], hand["sum"]), ("5 h", "0.2", "1x", "5 h = 18,000 s, the baseline"))
        self.assertEqual((control["time"], control["an_hour"], control["times"], control["sum"]), ("12.5 min", "4.8", "24x", "18,000 s ÷ 750 s"))
        self.assertEqual((deity["time"], deity["an_hour"], deity["times"], deity["sum"]), ("30 s", "120", "600x", "18,000 s ÷ 30 s"))
        # The newest reading is the one shown, with its goal and where the
        # stop started, and the day in full for the page to word.
        self.assertEqual((self_play["time"], self_play["an_hour"], self_play["times"], self_play["sum"]), ("60 s", "60", "300x", "18,000 s ÷ 60 s"))
        self.assertEqual((self_play["goal"], self_play["was"], self_play["taken"]), ("10 s, 1,800x", "200x on 11-22", "2026-12-01"))
        self.assertEqual(self_play["from"], "twelve games on one core")
        self.assertNotIn("goal", control)
        self.assertNotIn("was", control)
        self.assertEqual(shown["base"], {"time": "5 h", "seconds": "18,000", "an_hour": "0.2"})

    def test_experimental_is_the_run_and_theoretical_is_every_core_at_one_core_s_speed(self):
        shown = self.shown()
        self.assertEqual(shown["machine"], {"cores": 10, "chip": "Apple M5 Max"})
        rows = shown["throughput"]
        self.assertEqual(
            [(row["key"], row["kind"], row.get("basis", "")) for row in rows],
            [
                ("all_cores", "measured", ""),
                # The theory for the game the run plays stands under it ...
                ("self_play", "theory", "one_core"),
                ("self_play", "theory", "alone"),
                # ... then the theory for a game no run has played on every core,
                ("deity_sim", "theory", "one_core"),
                # and the goals: the run's own, then the whole line's.
                ("all_cores", "goal", ""),
                ("goal", "goal", ""),
            ],
        )
        run, one_core, alone, deity, own_goal, goal = rows
        # Experimental: the record run, 8 games a minute on 10 cores.
        self.assertEqual((run["of"], run["a_minute"], run["an_hour"], run["times"], run["time"]), ("self_play", "8", "480", "2,400x", "7.5 s"))
        self.assertEqual((run["sum"], run["had"], run["goal"], run["taken"]), ("8 games a minute × 60 × 5 h", "9.5", "1 s, 18,000x", "2026-11-25"))
        self.assertNotIn("reaches", run)
        # Theoretical: 10 cores, each a game in the 60 s one core was measured at.
        self.assertEqual((one_core["a_minute"], one_core["an_hour"], one_core["times"]), ("10", "600", "3,000x"))
        self.assertEqual((one_core["sum"], one_core["one_game"], one_core["reaches"]), ("10 cores × 18,000 s ÷ 60 s", "60 s", "80%"))
        # And each at the speed of the newest game that had the machine to
        # itself, 200 turns in 40 s, playing the run's 160-turn games: 32 s.
        self.assertEqual((alone["one_game"], alone["an_hour"], alone["times"], alone["reaches"]), ("32 s", "1,125", "5,625x", "43%"))
        self.assertEqual(alone["sum"], "10 cores × (200 turns ÷ 40 s) × 18,000 s ÷ 160 turns a game")
        self.assertEqual((alone["from"], alone["taken"]), ("one game with the machine to itself", "2026-11-22"))
        # A game no run has played on every core has its theory and no share.
        self.assertEqual((deity["times"], deity["sum"]), ("6,000x", "10 cores × 18,000 s ÷ 30 s"))
        self.assertNotIn("reaches", deity)
        # The goals, with the share of each the run reaches.
        self.assertEqual((own_goal["times"], own_goal["a_minute"], own_goal["an_hour"], own_goal["sum"], own_goal["reaches"]), ("18,000x", "60", "3,600", "18,000 s ÷ 1 s", "13%"))
        self.assertEqual((goal["times"], goal["a_minute"], goal["an_hour"], goal["sum"], goal["reaches"]), ("1,000,000x", "3,333", "200,000", "a game every 18 ms", "0.2%"))
        self.assertEqual(shown["goal"], {"times": "1,000,000x", "time": "18 ms", "a_minute": "3,333", "an_hour": "200,000"})
        # The opening sentence: self-play one game at a time, on every core
        # as measured, and the highest theory there is for it.
        self.assertEqual(
            shown["answer"],
            {"stop": "self_play", "one": "300x", "one_time": "60 s", "all": "2,400x", "all_time": "7.5 s", "all_an_hour": "480",
             "theory": "5,625x", "theory_basis": "alone", "reaches": "43%"},
        )

    def test_every_sum_shown_gives_the_figure_beside_it(self):
        for shown in (self.shown(), speedup_timeline.worked(ROOT)):
            rows = [row for row in shown["games"][1:] + shown["throughput"] if row["key"] != "goal"]
            self.assertGreaterEqual(len(rows), 5)
            for row in rows:
                # The figures of a sum are written to four figures.
                self.assertAlmostEqual(done_again(row["sum"]), row["multiplier"], delta=0.002 * row["multiplier"], msg=row["sum"])
                self.assertEqual(row["times"], speedup_timeline.times(row["multiplier"]))

    def test_without_an_every_core_run_there_is_one_game_at_a_time_alone(self):
        # A ledger whose every-core stop names no game, or whose run does
        # not say its cores, has no machine to multiply by.
        ledger = json.loads(json.dumps(self.LEDGER))
        ledger["stops"] = ledger["stops"][:-1]
        shown = self.shown(ledger)
        self.assertEqual(len(shown["games"]), 4)
        self.assertEqual(shown["throughput"], [])
        self.assertNotIn("answer", shown)
        self.assertNotIn("machine", shown)
        shown = self.shown(runs=[{"taken": "2026-11-22T05:32:38Z", "games_per_minute": 6.0}])
        self.assertEqual((shown["throughput"], "answer" in shown), ([], False))
        # No game alone on the machine: the theory is the one-core reading's.
        ledger = json.loads(json.dumps(self.LEDGER))
        del ledger["stops"][3]["alone"]
        shown = self.shown(ledger)
        self.assertEqual([row.get("basis") for row in shown["throughput"] if row["kind"] == "theory"], ["one_core", "one_core"])
        self.assertEqual((shown["answer"]["theory"], shown["answer"]["theory_basis"]), ("3,000x", "one_core"))
        # No ledger, or one that is not the shape this reads: nothing.
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.worked(tree(scratch, None)))
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.worked(tree(scratch, {"goal": 1000000, "stops": []})))

    def test_the_two_pages_read_one_ledger_and_agree(self):
        shown, line = speedup_timeline.worked(ROOT), speedup_timeline.timeline(ROOT)
        stops = {stop["key"]: stop for stop in line["stops"]}
        for row in shown["games"] + [row for row in shown["throughput"] if row["kind"] == "measured"]:
            self.assertEqual((row["times"], row["time"]), (stops[row["key"]]["times"], stops[row["key"]]["time"]), row["key"])
        # The home page's number is the run's.
        self.assertEqual(shown["answer"]["all"], line["speedup"])
        self.assertEqual(shown["goal"]["times"], line["goal"])
        # The operator's baseline and his three ways of playing, in the
        # order he listed them, then the theory and the run for self-play.
        self.assertEqual([row["key"] for row in shown["games"]], ["hand", "control", "deity_sim", "self_play"])
        self.assertEqual(shown["base"]["time"], "5 h")
        self.assertEqual(shown["answer"]["stop"], "self_play")
        kinds = [(row["key"], row["kind"]) for row in shown["throughput"]]
        self.assertEqual(kinds[0], ("all_cores", "measured"))
        self.assertIn(("self_play", "theory"), kinds)
        self.assertIn(("deity_sim", "theory"), kinds)
        # The theory is over the run, and the run reaches a share of it.
        measured = shown["throughput"][0]
        for row in shown["throughput"]:
            if row["kind"] == "theory" and row["key"] == "self_play":
                self.assertGreater(row["multiplier"], measured["multiplier"])
                self.assertRegex(row["reaches"], r"^\d{1,2}%$")

    def test_the_ledger_s_keys_for_the_tables_are_whole(self):
        keys = {stop["key"]: stop for stop in LEDGER["stops"]}
        for stop in LEDGER["stops"]:
            if "of" in stop:
                self.assertIn("follows", stop)
                self.assertEqual(keys[stop["of"]].get("cores"), 1, "a run plays the game of a one-core stop")
            for reading in stop.get("alone", []):
                self.assertEqual(stop.get("cores"), 1)
                self.assertRegex(reading["taken"], r"^\d{4}-\d{2}-\d{2}$")
                self.assertGreater(reading["turns"], 0)
                self.assertGreater(reading["seconds"], 0)
                self.assertGreater(len(reading["from"]), 20)


class Processors(unittest.TestCase):
    """The Performance page's third table (operator 2026-11-22: "Add in a
    theoretical 9980, performance and m5 ultra, m6. we should have a table
    in civvis performance that has our benchmarks (and age) across each
    processor. we should also list the single thread score and multithread
    score from the website")."""

    LISTED = {
        "scores": {"site": "Chartsite", "address": "https://charts.example/", "single_thread": "One Thread", "multithread": "Every Thread",
                   "from": "the website's page for each processor"},
        "measured": "ours",
        "processors": [
            {"key": "ours", "name": "Ours", "cores": 10, "threads": 10, "chip": "Apple M5 Max", "fleet": "mbp-m5-max-128", "scores": [
                {"read": "2026-11-01", "single_thread": 9999, "multithread": 9999, "samples": 3, "page": "https://charts.example/ours"},
                {"read": "2026-11-23", "single_thread": 4000, "multithread": 40000, "samples": 200, "margin": "low", "page": "https://charts.example/ours"},
            ]},
            {"key": "wide", "name": "Wide", "cores": 64, "threads": 128, "fleet": "martbot-9985", "scores": [
                {"read": "2026-11-22", "single_thread": 3200, "multithread": 100000, "samples": 4, "margin": "high", "page": "https://elsewhere.example/wide"},
            ]},
            {"key": "quick", "name": "Quick", "cores": 12, "threads": 12, "chip": "Quick Chip", "scores": [
                {"read": "2026-11-20", "single_thread": 5000, "multithread": 20000, "samples": 80, "page": "https://charts.example/quick"},
            ]},
        ],
    }

    def shown(self, listed: dict | None = None, runs: list[dict] | None = None, ledger: dict | None = None) -> dict | None:
        with tempfile.TemporaryDirectory() as scratch:
            source = tree(scratch, ledger or Worked.LEDGER, Worked.RUNS if runs is None else runs, self.LISTED if listed is None else listed)
            return speedup_timeline.worked(source)

    def test_the_processor_the_readings_were_taken_on_shows_them_as_measured(self):
        across = self.shown()["processors"]
        self.assertEqual(across["site"], {
            "name": "Chartsite", "address": "https://charts.example/", "single_thread": "One Thread", "multithread": "Every Thread",
            "from": "the website's page for each processor", "read": "2026-11-23",
        })
        self.assertEqual([row["key"] for row in across["rows"]], ["ours", "wide", "quick"])
        ours = across["rows"][0]
        # The newest reading of the website is the one shown.
        self.assertEqual((ours["single_thread"], ours["multithread"], ours["samples"], ours["margin"], ours["read"]), ("4,000", "40,000", 200, "low", "2026-11-23"))
        self.assertEqual((ours["name"], ours["cores"], ours["threads"], ours["fleet"], ours["page"]), ("Ours", 10, 10, "mbp-m5-max-128", "https://charts.example/ours"))
        # Its one-core games are the stops' newest readings, and its
        # every-core figure the record among the runs on its own chip: the
        # only run that names it.
        self.assertEqual(ours["kind"], "measured")
        self.assertEqual([(cell["key"], cell["kind"], cell["time"], cell["taken"]) for cell in ours["games"]],
                         [("deity_sim", "measured", "30 s", "2026-11-22"), ("self_play", "measured", "60 s", "2026-12-01")])
        self.assertTrue(all("sum" not in cell for cell in ours["games"]))
        self.assertEqual(ours["every_core"], {"kind": "measured", "of": "self_play", "games_a_minute": 8.0, "a_minute": "8", "an_hour": "480", "taken": "2026-11-25"})
        self.assertEqual((ours["times"], ours["times_sum"]), ("2,400x", "8 games a minute × 60 × 5 h"))
        # The age is counted from the oldest reading its figures rest on.
        self.assertEqual(ours["taken"], "2026-11-22")

    def test_any_other_processor_is_worked_out_from_the_two_scores(self):
        across = self.shown()["processors"]
        wide, quick = across["rows"][1:]
        self.assertEqual(wide["kind"], "theory")
        # One game on one core: the measured time, times the measured
        # processor's single thread score, over the row's own.
        self.assertEqual([(cell["key"], cell["kind"], cell["time"], cell["sum"]) for cell in wide["games"]],
                         [("deity_sim", "theory", "38 s", "30 s × 4,000 ÷ 3,200"), ("self_play", "theory", "75 s", "60 s × 4,000 ÷ 3,200")])
        # Every core: the measured record, times the row's multithread
        # score, over the measured processor's.
        self.assertEqual(wide["every_core"], {
            "kind": "theory", "of": "self_play", "games_a_minute": 20.0, "a_minute": "20", "an_hour": "1,200", "taken": "2026-11-25",
            "sum": "8 games a minute × 100,000 ÷ 40,000",
        })
        self.assertEqual((wide["times"], wide["times_sum"], wide["taken"]), ("6,000x", "20 games a minute × 60 × 5 h", "2026-11-22"))
        self.assertEqual((wide["single_thread"], wide["multithread"], wide["samples"], wide["margin"], wide["fleet"]), ("3,200", "100,000", 4, "high", "martbot-9985"))
        # A page that is not the website's own is not linked.
        self.assertNotIn("page", wide)
        # A slower many-core processor plays one game slower and more games.
        self.assertEqual((quick["games"][1]["time"], quick["every_core"]["a_minute"], quick["times"]), ("48 s", "4", "1,200x"))
        self.assertNotIn("margin", quick)
        self.assertNotIn("fleet", quick)

    def test_a_run_on_a_processor_s_own_chip_is_its_measured_figure(self):
        # Once the benchmark is run on a listed chip its every-core figure
        # is the count and not the calculation, the record among its runs.
        runs = Worked.RUNS + [
            {"taken": "2026-11-27T05:00:00Z", "games_per_minute": 3.0, "machine": {"chip": "Quick Chip", "cores": 12}},
            {"taken": "2026-11-28T05:00:00Z", "games_per_minute": 5.0, "machine": {"chip": "Quick Chip", "cores": 12}},
            {"taken": "2026-11-29T05:00:00Z", "games_per_minute": 4.5, "machine": {"chip": "Quick Chip", "cores": 12}},
        ]
        across = self.shown(runs=runs)["processors"]
        ours, wide, quick = across["rows"]
        self.assertEqual(quick["every_core"], {"kind": "measured", "of": "self_play", "games_a_minute": 5.0, "a_minute": "5", "an_hour": "300", "taken": "2026-11-28"})
        self.assertEqual((quick["kind"], quick["times"]), ("measured", "1,500x"))
        # Its one-core games were not run there, so they stay worked out.
        self.assertEqual([cell["kind"] for cell in quick["games"]], ["theory", "theory"])
        # The others still rest on the measured processor's own record.
        self.assertEqual((ours["every_core"]["a_minute"], wide["every_core"]["sum"]), ("8", "8 games a minute × 100,000 ÷ 40,000"))

    def test_every_sum_shown_gives_the_figure_beside_it(self):
        base = 18000
        for across in (self.shown()["processors"], speedup_timeline.worked(ROOT)["processors"]):
            worked_out = 0
            for row in across["rows"]:
                every = row["every_core"]
                self.assertAlmostEqual(done_again(row["times_sum"]), row["multiplier"], delta=0.002 * row["multiplier"], msg=row["times_sum"])
                self.assertEqual(row["times"], speedup_timeline.times(row["multiplier"]))
                self.assertEqual(every["a_minute"], speedup_timeline.rate(every["games_a_minute"]))
                self.assertAlmostEqual(row["multiplier"], every["games_a_minute"] * base / 60, delta=0.001 * row["multiplier"])
                for cell in [every] + row["games"]:
                    self.assertEqual("sum" in cell, cell["kind"] == "theory", row["key"])
                    if cell["kind"] == "theory":
                        worked_out += 1
                        figure = cell.get("games_a_minute") or cell["seconds"]
                        self.assertAlmostEqual(done_again(cell["sum"]), figure, delta=0.002 * figure, msg=cell["sum"])
                for cell in row["games"]:
                    self.assertEqual(cell["time"], speedup_timeline.clock(cell["seconds"]))
            self.assertGreaterEqual(worked_out, 6)

    def test_without_a_list_or_a_run_the_table_shows_what_it_can(self):
        # A revision with no list of processors has its two tables and no
        # third, and so has one whose list cannot be read.
        with tempfile.TemporaryDirectory() as scratch:
            shown = speedup_timeline.worked(tree(scratch, Worked.LEDGER, Worked.RUNS))
        self.assertNotIn("processors", shown)
        self.assertEqual(len(shown["games"]), 4)
        for broken in ({"processors": []}, {**self.LISTED, "measured": "nobody"}):
            self.assertNotIn("processors", self.shown(listed=broken))
        # No run on the measured processor's chip: the one-core games are
        # shown, and no every-core figure is made up for anyone.
        across = self.shown(runs=[{"taken": "2026-11-22T05:32:38Z", "games_per_minute": 6.0}])["processors"]
        for row in across["rows"]:
            self.assertNotIn("every_core", row)
            self.assertNotIn("times", row)
            self.assertEqual(len(row["games"]), 2)
        self.assertEqual([row["kind"] for row in across["rows"]], ["measured", "theory", "theory"])

    def test_the_committed_list_is_whole(self):
        listed = json.loads((ROOT / speedup_timeline.PROCESSORS).read_text(encoding="utf-8"))
        site = listed["scores"]
        self.assertRegex(site["address"], r"^https://[\w.-]+/$")
        for key in ("site", "single_thread", "multithread"):
            self.assertTrue(site[key], key)
        self.assertGreater(len(site["from"]), 40, "the list says how a score is read")
        keys = [entry["key"] for entry in listed["processors"]]
        names = [entry["name"] for entry in listed["processors"]]
        self.assertEqual(len(keys), len(set(keys)))
        self.assertEqual(len(names), len(set(names)), "two rows of the table would read alike")
        self.assertIn(listed["measured"], keys)
        machines = (ROOT / "docs/MACHINES.md").read_text(encoding="utf-8")
        for entry in listed["processors"]:
            self.assertGreaterEqual(entry["threads"], entry["cores"], entry["key"])
            self.assertGreater(entry["cores"], 0)
            days = [reading["read"] for reading in entry["scores"]]
            self.assertTrue(days, entry["key"])
            self.assertEqual(days, sorted(days), f"{entry['key']}: the readings are kept oldest first")
            for reading in entry["scores"]:
                self.assertRegex(reading["read"], r"^\d{4}-\d{2}-\d{2}$")
                self.assertGreater(reading["single_thread"], 0)
                self.assertGreater(reading["multithread"], reading["single_thread"])
                self.assertGreater(reading["samples"], 0)
                self.assertIn(reading["margin"], ("low", "medium", "high"))
                self.assertTrue(reading["page"].startswith(site["address"]), reading["page"])
            # A machine of the fleet is named as the registry names it.
            if "fleet" in entry:
                self.assertIn(f"`{entry['fleet']}`", machines)
        # Nothing derived is kept.
        self.assertNotIn("multiplier", json.dumps(listed["processors"]))
        # The measured processor is the one the benchmark's runs name, so
        # its row is the record and every other row has a figure to be
        # worked out from.
        measured = next(entry for entry in listed["processors"] if entry["key"] == listed["measured"])
        runs = json.loads((ROOT / BENCHMARK).read_text(encoding="utf-8"))["runs"]
        self.assertIn(measured["chip"], {(run.get("machine") or {}).get("chip") for run in runs})
        self.assertEqual(measured["cores"], speedup_timeline.worked(ROOT)["machine"]["cores"])

    def test_the_operator_s_processors_are_listed_and_only_ours_is_measured(self):
        across = speedup_timeline.worked(ROOT)["processors"]
        rows = {row["key"]: row for row in across["rows"]}
        # The machine the benchmarks run on, then the three he asked for.
        self.assertEqual(across["rows"][0]["key"], "m5_max")
        for key in ("threadripper_9980x", "m5_ultra_36", "m6"):
            self.assertEqual(rows[key]["kind"], "theory", key)
        self.assertEqual([row["key"] for row in across["rows"] if row["kind"] == "measured"], ["m5_max"])
        # Our row is the run the rest of the page shows.
        run = speedup_timeline.worked(ROOT)["throughput"][0]
        self.assertEqual((rows["m5_max"]["times"], rows["m5_max"]["every_core"]["a_minute"]), (run["times"], run["a_minute"]))
        for row in across["rows"]:
            self.assertRegex(row["single_thread"], r"^\d{1,3}(,\d{3})*$")
            self.assertRegex(row["multithread"], r"^\d{1,3}(,\d{3})*$")
            self.assertRegex(row["taken"], r"^\d{4}-\d{2}-\d{2}$")
            self.assertEqual([cell["key"] for cell in row["games"]], ["deity_sim", "self_play"])


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
