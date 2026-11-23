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
