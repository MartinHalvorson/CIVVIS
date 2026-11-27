#!/usr/bin/env python3
"""The speed timeline of CIVVIS Performance: the ledger of readings, what
`speedup_timeline.py` works out from it, and the page that shows it. The line
was the home page's until 2026-11-24 (operator: "move the civvis performance
timeline thing from home/ to the civvis performance section. middle of the
page or so")."""

from __future__ import annotations

import contextlib
import html
import io
import json
import math
import pathlib
import re
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import home_rankings  # noqa: E402
import performance_page  # noqa: E402
import speedup_timeline  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[1]
PAGE = (ROOT / "beta/performance.html").read_text(encoding="utf-8")
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
            (0.000018, "18 µs"), (0.00099, "990 µs"), (0.001, "1 ms"), (0.018, "18 ms"), (0.9, "900 ms"), (1, "1 s"), (3.273, "3.3 s"), (9.94, "9.9 s"), (9.96, "10 s"), (21.4, "21 s"),
            (99.4, "99 s"), (99.6, "1.7 min"), (600, "10 min"), (757, "12.6 min"), (5969, "99.5 min"),
            (5970, "1.7 h"), (18000, "5 h"), (19800, "5.5 h"),
        ]:
            self.assertEqual(speedup_timeline.clock(seconds), words, seconds)

    def test_a_multiplier_is_a_whole_number_with_its_thousands_marked(self):
        for multiplier, words in [
            (1, "1x"), (1.02, "1x"), (2.5, "2.5x"), (9.9, "9.9x"), (9.96, "10x"), (23.78, "24x"),
            (5498.9999, "5,499x"), (18000, "18,000x"), (1000000, "1,000,000x"), (1000000000, "1,000,000,000x"),
        ]:
            self.assertEqual(speedup_timeline.times(multiplier), words, multiplier)

    def test_games_a_second_are_worded_by_their_size(self):
        for games, words in [
            (0.4055, "0.41 games/s"), (0.994, "0.99 games/s"), (0.999, "1 game/s"), (1, "1 game/s"), (2, "2 games/s"), (4.055, "4.1 games/s"),
            (9.94, "9.9 games/s"), (9.96, "10 games/s"), (55.56, "56 games/s"), (405.4, "405 games/s"), (1234.5, "1,234 games/s"),
        ]:
            self.assertEqual(speedup_timeline.per_second(games), words, games)

    def test_a_mark_of_the_scale_is_in_words_from_a_million_up(self):
        for multiplier, words in [
            (1, "1x"), (1000, "1,000x"), (100000, "100,000x"), (1000000, "1 million x"), (2500000, "2.5 million x"),
            (100000000, "100 million x"), (1000000000, "1 billion x"),
        ]:
            self.assertEqual(speedup_timeline.scale(multiplier), words, multiplier)

    def test_a_sum_of_money_is_a_price_to_the_cent_or_an_estimate_to_three_figures(self):
        for value, words in [
            (2.85, "$2.85"), (10.346, "$10.35"), (74.1, "$74"), (268.996, "$269"), (999.6, "$1,000"), (1336.7, "$1,340"), (90631, "$90,600"),
            (267351, "$267,000"), (999600, "$1 million"), (2356406, "$2.36 million"), (645200000, "$645 million"), (2342000000, "$2.34 billion"),
        ]:
            self.assertEqual(speedup_timeline.dollars(value), words, value)


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

    def test_the_goal_is_the_operator_s_million(self):
        # Operator 2026-11-22: "1,000,000x speed imrpove would be a huge
        # achievement". A billion stood beside it from later that day until
        # 2026-11-24: "Change the timeline to just scale to 1,000,000x (drop
        # the 1bX)."
        self.assertEqual([(goal["key"], goal["times"]) for goal in LEDGER["goals"]], [("million", 1000000)])
        self.assertEqual(speedup_timeline.marked(LEDGER), LEDGER["goals"])
        self.assertNotIn("goal", LEDGER)
        self.assertNotIn("billion", json.dumps(LEDGER["goals"]))
        self.assertNotIn('data-goal="billion"', PAGE)

    def test_the_first_stop_carries_the_operator_s_note_on_his_own_play(self):
        # Operator 2026-11-24: "By "Human Player" change it to "Human Player
        # (me)" with subtitle note: Some are faster, some are slower. I'm a
        # bit on the slower side but 5h seems reasonable."
        stop = re.search(r'<li class="speed-stop" data-stop="hand">(.*?)</li>', PAGE, re.S).group(1)
        asides = [" ".join(html.unescape(aside).split()) for aside in re.findall(r'<small class="speed-aside">(.*?)</small>', stop, re.S)]
        self.assertEqual(asides, ["Some are faster, some are slower. I’m a bit on the slower side but 5h seems reasonable."])
        # The note names his own figure, the one figure on the chart that
        # the slot does not write. It is held to the stop's reading here,
        # so the two cannot come apart unseen.
        hand = {row["key"]: row for row in speedup_timeline.timeline(ROOT)["stops"]}["hand"]
        self.assertEqual(re.findall(r"\d[\d,.]*\s*(?:x|s|min|h)\b", asides[0]), [hand["time"].replace(" ", "")])
        # It is his note on his own five hours and on nothing else: no
        # other entry of the chart has one.
        self.assertEqual(PAGE.count('class="speed-aside"'), 1)
        # Under the figures, where a stop's other small lines stand.
        self.assertLess(stop.index('class="speed-read"'), stop.index('class="speed-aside"'))
        # Shown, where a stop's sentence is only what a pointer held over
        # it says: it is not the hidden `speed-what`, and it has a place in
        # the list and on the line.
        self.assertIn('<small class="speed-what">One person plays one game of Civilization VI.</small>', stop)
        self.assertIn("  .speed-aside { grid-column: 1 / -1; grid-row: 4;", PAGE)
        wide = PAGE[PAGE.index("@media (min-width: 1120px) {"):]
        # On the line it wraps in the 150px every stop's words have: the
        # slot keeps two stops' words that far apart and no further.
        self.assertIn("    .speed-aside { display: block; margin: 2px 0 0; line-height: 1.3; }", wide[:wide.index("</style>")])
        self.assertNotRegex(PAGE, r"\.speed-aside \{[^}]*\bwidth:")
        # The room over and under the line is measured from every line of
        # words but the hidden sentence, so the note has its room.
        self.assertIn('filter((line) => !line.hidden && !line.classList.contains("speed-what"))', PAGE)

    def test_the_line_s_stops_are_the_five_the_operator_named(self):
        # Operator 2026-11-24: "we should have: human player 5 hrs, Firaxis
        # Computer control: 10-15mins but we should benchmark this with our
        # games, Firaxis self-play (test how long this take), CIVVIS
        # self-play, "Parallel games, M5 Max, all 18 cores"."
        shown = speedup_timeline.timeline(ROOT)
        self.assertEqual([stop["key"] for stop in shown["stops"]], ["hand", "control", "firaxis_self_play", "self_play", "all_cores"])
        names = re.findall(r'<li class="speed-stop" data-stop="\w+">.*?<b class="speed-name">(.*?)</b>', PAGE, re.S)
        # The control stop as he named it later that day: "lets also call
        # Firaxis computer Control inside CIVVIS->Firaxis computer control".
        # His arrow is drawn as one.
        # The first is his own play and says so, as he asked that day: "By
        # "Human Player" change it to "Human Player (me)"".
        self.assertEqual([html.unescape(re.sub(r"<[^>]+>", "", name)) for name in names],
                         ["Human Player (me)", "CIVVIS → Firaxis computer control", "Firaxis self-play", "CIVVIS self-play", "Parallel games, M5 Max, all 18 cores"])
        stops = {stop["key"]: stop for stop in shown["stops"]}
        self.assertEqual(stops["hand"]["time"], "5 h")
        # His "10-15mins", benchmarked with our own games: the reading
        # written down, and over it the computer control benchmark's own
        # record where this clone holds it.
        self.assertTrue(600 <= stops["control"]["seconds"] <= 900, stops["control"]["seconds"])
        written = next(stop for stop in LEDGER["stops"] if stop["key"] == "control")
        self.assertEqual(written.get("benchmark"), "control")
        self.assertIn("our own games", written["readings"][-1]["from"])
        self.assertTrue("our own games" in stops["control"]["from"] or "computer control benchmark" in stops["control"]["from"])
        # A stop kept off the line stays in the ledger for the tables.
        off = [stop["key"] for stop in LEDGER["stops"] if stop.get("line") is False]
        self.assertEqual(off, ["deity_sim"])
        self.assertNotIn("deity_sim", stops)
        self.assertIn("deity_sim", [row["key"] for row in speedup_timeline.worked(ROOT)["games"]])

    def test_a_timed_reading_takes_a_worked_out_one_s_place_and_did_not_move_from_it(self):
        # Operator 2026-11-24: "Firaxis self-play (test how long this take)".
        # The first reading was worked out from our own games; the lane then
        # played one such game for real.
        stop = next(stop for stop in LEDGER["stops"] if stop["key"] == "firaxis_self_play")
        self.assertEqual([bool(reading.get("worked")) for reading in stop["readings"]][:2], [True, False])
        newest = stop["readings"][-1]
        self.assertNotIn("worked", newest)
        self.assertTrue(newest["from"].startswith("Timed:"))
        shown = {row["key"]: row for row in speedup_timeline.timeline(ROOT)["stops"]}
        self.assertNotIn("worked", shown["firaxis_self_play"])
        self.assertEqual(shown["firaxis_self_play"]["seconds"], newest["seconds"])
        # The worked-out figure is not where the stop stood before.
        if not any(not reading.get("worked") for reading in stop["readings"][:-1]):
            self.assertNotIn("was", shown["firaxis_self_play"])
        # It stands between computer control and the simulator.
        self.assertLess(shown["control"]["multiplier"], shown["firaxis_self_play"]["multiplier"])
        self.assertLess(shown["firaxis_self_play"]["multiplier"], shown["self_play"]["multiplier"])
        # The rule, on readings of its own.
        timed, worked = {"taken": "2026-12-01", "seconds": 300}, {"taken": "2026-11-24", "seconds": 500, "worked": True}
        later = {"taken": "2026-12-09", "seconds": 250}
        self.assertIs(speedup_timeline.began([worked, timed]), timed)
        self.assertIs(speedup_timeline.began([worked, timed, later]), timed)
        self.assertIs(speedup_timeline.began([worked]), worked)
        self.assertIs(speedup_timeline.began([timed, later]), timed)

    def test_a_reading_that_was_worked_out_says_so(self):
        # No game has been timed yet with Firaxis's AI in every seat: the
        # reading is worked out from our own games, and the pages mark it.
        stop = next(stop for stop in LEDGER["stops"] if stop["key"] == "firaxis_self_play")
        newest = stop["readings"][-1]
        if not newest.get("worked"):
            self.skipTest("a timed reading has taken the worked-out one's place")
        self.assertIn("not timed", newest["from"])
        shown = {row["key"]: row for row in speedup_timeline.timeline(ROOT)["stops"]}
        self.assertIs(shown["firaxis_self_play"]["worked"], True)
        self.assertNotIn("worked", shown["control"])
        tabled = {row["key"]: row for row in speedup_timeline.worked(ROOT)["games"]}
        self.assertIs(tabled["firaxis_self_play"]["worked"], True)
        self.assertIn('put(entry, "worked", stop.worked ? "worked out, not yet timed" : "");', PAGE)
        self.assertIn('entry.classList.toggle("is-worked", !!stop.worked);', PAGE)
        self.assertIn(".speed-stop.is-worked .speed-dot { background: var(--panel); border: 2px dashed var(--accent);", PAGE)
        # It stands between computer control and the simulator.
        self.assertLess(shown["control"]["multiplier"], shown["firaxis_self_play"]["multiplier"])
        self.assertLess(shown["firaxis_self_play"]["multiplier"], shown["self_play"]["multiplier"])

    def test_the_stops_past_what_was_measured_are_the_operator_s_four(self):
        # Operator 2026-11-22: "add to out speedup timeline: 10x theoretical
        # scale up from local hardware ... hardware scaleup is to the left of
        # m5 max. potential distributed RL fleet: up to 1000+x". And two
        # guesses of his on 2026-11-24: "somewhere to the left of CIVVIS
        # self-play, through down Civ 6 Computationl Irreducability - ??? not
        # sure where this is. Maybe just guess a 10x improvement? Maybe
        # through down a "M5 Max, Theoretical Max Civ 6 Throughput" somewhere
        # like 10x to the left of Civ 6 Computational Irr...".
        listed = LEDGER["projected"]
        self.assertEqual([(stop["key"], stop["over"]) for stop in listed],
                         [("irreducible", "self_play"), ("m5_max", "irreducible"), ("scale_up", "m5_max"), ("fleet", "all_cores")])
        measured = {stop["key"] for stop in LEDGER["stops"]}
        known = set(measured)
        for stop in listed:
            # A stop stands on a measured stop or on a projected stop listed
            # before it.
            self.assertIn(stop["over"], known)
            known.add(stop["key"])
            self.assertNotIn(stop["key"], measured)
            days = [estimate["taken"] for estimate in stop["estimates"]]
            self.assertEqual(days, sorted(days), f"{stop['key']}: the estimates are kept oldest first")
            for estimate in stop["estimates"]:
                self.assertRegex(estimate["taken"], r"^\d{4}-\d{2}-\d{2}$")
                self.assertGreater(estimate["times"], 1)
                self.assertIn("The project owner's figure", estimate["from"])
                self.assertIn("not a run", estimate["from"])
        newest = {stop["key"]: stop["estimates"][-1] for stop in listed}
        self.assertEqual((newest["scale_up"]["times"], newest["scale_up"].get("or_more", False)), (10, False))
        self.assertEqual((newest["fleet"]["times"], newest["fleet"].get("or_more", False)), (1000, True))
        # Both of 2026-11-24 are tenfolds, and each says it is a guess.
        for key in ("irreducible", "m5_max"):
            self.assertEqual((newest[key]["times"], newest[key].get("or_more", False)), (10, False))
            self.assertIn("guess", newest[key]["from"].lower())
        # Nothing derived is kept: what a figure comes to is worked out.
        self.assertNotIn("multiplier", json.dumps(listed))
        self.assertNotIn("seconds", json.dumps(listed))
        # On the line each is that many times the M5 Max's stop, to its left
        # (a larger `at` is further left).
        shown = speedup_timeline.timeline(ROOT)
        stops = {stop["key"]: stop for stop in shown["stops"]}
        ahead = {stop["key"]: stop for stop in shown["projected"]}
        self.assertEqual(list(ahead), ["irreducible", "m5_max", "scale_up", "fleet"])
        # The irreducible game is ten times CIVVIS self-play, to its left,
        # and the most an M5 Max could finish is ten times that: a hundred
        # times CIVVIS self-play.
        one = stops["self_play"]["multiplier"]
        self.assertAlmostEqual(ahead["irreducible"]["multiplier"], 10 * one, delta=0.01 * one)
        self.assertAlmostEqual(ahead["m5_max"]["multiplier"], 100 * one, delta=0.1 * one)
        self.assertAlmostEqual(ahead["m5_max"]["multiplier"], 10 * ahead["irreducible"]["multiplier"], delta=0.1 * one)
        self.assertEqual((ahead["irreducible"]["by"], ahead["m5_max"]["by"]), ("10x", "10x"))
        self.assertLess(stops["self_play"]["at"], ahead["irreducible"]["at"])
        self.assertLess(ahead["irreducible"]["at"], ahead["m5_max"]["at"])
        self.assertAlmostEqual(ahead["m5_max"]["at"] - ahead["irreducible"]["at"], ahead["irreducible"]["at"] - stops["self_play"]["at"], places=3)
        # The scale-up is ten times the M5 Max's theoretical most, to its
        # left (operator, 2026-11-26: "reasonable local hardware scale-up
        # theoretical should be 10x left of m5 max theoretical max civ 6
        # thorugh").
        most = ahead["m5_max"]["multiplier"]
        self.assertAlmostEqual(ahead["scale_up"]["multiplier"], 10 * most, delta=0.01 * most)
        self.assertLess(ahead["m5_max"]["at"], ahead["scale_up"]["at"])
        self.assertAlmostEqual(ahead["scale_up"]["at"] - ahead["m5_max"]["at"], ahead["m5_max"]["at"] - ahead["irreducible"]["at"], places=3)
        under = stops["all_cores"]["multiplier"]
        self.assertAlmostEqual(ahead["fleet"]["multiplier"], 1000 * under, delta=under)
        self.assertEqual((ahead["scale_up"]["by"], ahead["fleet"]["by"]), ("10x", "1,000x+"))
        self.assertTrue(ahead["fleet"]["times"].endswith("x+"))
        self.assertLess(stops["all_cores"]["at"], ahead["scale_up"]["at"])
        self.assertLess(ahead["scale_up"]["at"], ahead["fleet"]["at"])
        # A thousand M5 Maxes are past a million, where the line ended until
        # the operator had it run out to ten million (2026-11-24: "maybe
        # extend timeline out to 10,000,000x"): the fleet stands on the line
        # at its own place, past the goal and short of the end. A stop past
        # the end of a line still stands at its end, marked as running on
        # (`test_the_line_runs_out_past_its_last_goal`).
        self.assertGreater(ahead["fleet"]["multiplier"], LEDGER["goals"][-1]["times"])
        self.assertLess(ahead["fleet"]["multiplier"], speedup_timeline.end(LEDGER))
        self.assertLess(shown["goals"][-1]["at"], ahead["fleet"]["at"])
        self.assertLess(ahead["fleet"]["at"], 1.0)
        for stop in ahead.values():
            self.assertNotIn("past", stop)
        self.assertIn('entry.classList.toggle("is-past", !!stop.past);', PAGE)
        # And neither is the page's number: that is the fastest measured stop.
        self.assertEqual(shown["fastest"], "all_cores")
        self.assertNotIn(shown["speedup"], [stop["times"] for stop in ahead.values()])

    def test_left_of_civvis_self_play_the_times_are_amortized(self):
        # Operator 2026-11-24: "starting left of CIVVIS self-play (but not
        # including that), we should note that estimates are amortized
        # games". The ledger says which entries play many games at once.
        self.assertEqual([stop["key"] for stop in LEDGER["stops"] if stop.get("amortized")], ["all_cores"])
        self.assertEqual([goal["key"] for goal in LEDGER["goals"] if goal.get("amortized")], ["million"])
        self.assertEqual([stop["key"] for stop in LEDGER["projected"] if stop.get("amortized")], ["m5_max"])
        shown = speedup_timeline.timeline(ROOT)
        entries = shown["stops"] + shown["projected"] + shown["goals"]
        edge = next(entry["at"] for entry in entries if entry["key"] == "self_play")
        # Nothing at CIVVIS self-play or right of it is amortized. Left of
        # it everything is but the irreducible game, which is one game on
        # one core and takes the time it says.
        self.assertEqual([entry["key"] for entry in entries if entry["at"] <= edge and entry.get("amortized")], [])
        self.assertEqual([entry["key"] for entry in entries if entry["at"] > edge and not entry.get("amortized")], ["irreducible"])
        self.assertEqual([entry["key"] for entry in entries if entry.get("amortized")], ["all_cores", "m5_max", "scale_up", "fleet", "million"])
        # A stop so many times an amortized stop is amortized with it.
        over = {stop["key"]: stop["over"] for stop in LEDGER["projected"]}
        self.assertEqual((over["scale_up"], over["fleet"]), ("m5_max", "all_cores"))

    def test_an_amortized_stop_leads_with_its_games_per_second(self):
        # Operator 2026-11-24: "on the left side of the timeline, at more
        # than 1 game / second, we should also provide the games per second
        # as the standard measure and (time per game amoritized as the
        # second measure)". And later that day, of the one amortized stop
        # under a game a second: "lest have m5 max blurb list amortiized
        # games per second as the primary unit. as this is really
        # amortiization at play". So every amortized entry leads with them.
        shown = speedup_timeline.timeline(ROOT)
        base = shown["stops"][0]["seconds"]
        entries = shown["stops"] + shown["projected"] + shown["goals"]
        for entry in entries:
            if not entry.get("amortized"):
                self.assertNotIn("rate", entry)
                self.assertNotIn("lead", entry)
                continue
            # Its games a second are its multiplier over the baseline's
            # seconds, and its time is still the time per game.
            games = entry["multiplier"] / base
            self.assertEqual(entry["rate"].replace("+", ""), speedup_timeline.per_second(games), entry["key"])
            # The multiplier is kept to the cent, so a time that falls on a
            # half (the scale-up's 34.5 ms) may be worded either side of it.
            near = {speedup_timeline.clock(base / (entry["multiplier"] + cent)) for cent in (-0.005, 0, 0.005)}
            self.assertIn(entry["time"], near, entry["key"])
            self.assertEqual(entry.get("lead"), "rate", entry["key"])
        led = [entry["key"] for entry in entries if entry.get("lead") == "rate"]
        self.assertEqual(led, ["all_cores", "m5_max", "scale_up", "fleet", "million"])
        # The M5 Max as it runs today is short of a game a second and
        # leads with its games a second all the same: its time per game is
        # the second measure. Its goal is in the measure it leads with,
        # the game a second the operator named from the first (2026-11-22:
        # "Can scale across M5 Max cores to 1 game per second").
        every = next(entry for entry in entries if entry["key"] == "all_cores")
        self.assertLess(every["multiplier"] / base, 1)
        self.assertRegex(every["rate"], r"^0\.\d\d games/s$")
        self.assertEqual(every["goal"], "goal 1 game/s, 18,000x")
        # One game on one core is no amortized time, and its goal is a
        # game's time.
        alone = next(entry for entry in entries if entry["key"] == "self_play")
        self.assertEqual(alone["goal"], "goal 10 s, 1,800x")
        # The page takes the lead from the slot and words the second
        # measure to match.
        self.assertIn('const fast = row.amortized && row.lead === "rate";', PAGE)
        self.assertIn('put(entry, "time", fast ? row.rate : row.time);', PAGE)
        self.assertIn('put(entry, "second", row.amortized ? "(" + (fast ? row.time + " a game" : row.rate) + ", amortized)" : "");', PAGE)
        # A figure open at its top is open in both measures.
        fleet = next(entry for entry in entries if entry["key"] == "fleet")
        self.assertRegex(fleet["rate"], r"^[\d,]+\+ games/s$")
        self.assertTrue(fleet["times"].endswith("x+"))

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
        # The goals are on the line smallest first, each at its own place:
        # the last a tenfold short of the line's end, six of its seven
        # tenfolds along (`test_the_line_runs_out_past_its_last_goal`).
        base = shown["stops"][0]["seconds"]
        self.assertEqual([(goal["key"], goal["times"], goal["time"]) for goal in shown["goals"]],
                         [(goal["key"], speedup_timeline.times(goal["times"]), speedup_timeline.clock(base / goal["times"])) for goal in LEDGER["goals"]])
        self.assertEqual([goal["at"] for goal in shown["goals"]], [round(6 / 7, 4)])
        self.assertEqual((shown["goals"][-1]["times"], shown["goals"][-1]["time"]), ("1,000,000x", "18 ms"))
        self.assertEqual(shown["ticks"][-1], {"times": "10 million x", "at": 1.0, "per_game": "1.8 ms/game", "per_time": "556 games/s"})
        self.assertRegex(shown["speedup"], r"^\d{1,3}(,\d{3})*x$")

    def test_each_tenfold_is_also_a_time_a_game_and_games_in_a_time(self):
        # Operator 2026-11-27: "we can also have a time / game measure along
        # the top and a games / time measure along the bottom". 1x is a game
        # by hand, five hours, which is 0.2 games an hour; each tenfold a
        # tenth of the time and ten times the games, in the unit that leaves
        # at least one.
        shown = speedup_timeline.timeline(ROOT)
        self.assertEqual([(tick["times"], tick["per_game"], tick["per_time"]) for tick in shown["ticks"]], [
            ("1x", "5 h/game", "0.2 games/h"), ("10x", "30 min/game", "2 games/h"), ("100x", "3 min/game", "20 games/h"),
            ("1,000x", "18 s/game", "3.3 games/min"), ("10,000x", "1.8 s/game", "33.3 games/min"),
            ("100,000x", "180 ms/game", "5.6 games/s"), ("1 million x", "18 ms/game", "56 games/s"),
            ("10 million x", "1.8 ms/game", "556 games/s")])
        self.assertEqual(speedup_timeline.per_time(1 / 3600), "1 game/h")
        self.assertEqual(speedup_timeline.fine_clock(0.0018), "1.8 ms")
        # The page draws them over the track and under it, at the tenfolds'
        # own places.
        section = PAGE[PAGE.index('<section class="res-section" id="perf-line"'):PAGE.index("</section>", PAGE.index('id="perf-line"'))]
        top, track, bottom = (section.index(piece) for piece in (
            '<div class="speed-scale is-top" data-scale="game"', '<div class="speed-track">', '<div class="speed-scale is-bottom" data-scale="rate"'))
        self.assertLess(top, track)
        self.assertLess(track, bottom)
        self.assertIn('for (const [name, words] of [["game", tick.per_game], ["rate", tick.per_time]]) {', PAGE)

    def test_a_stop_says_what_it_is_played_on_and_its_time_in_the_browser(self):
        # Operator 2026-11-27: "note that firaxis (native app) and civvis
        # (browser)", "if thats true", "if we have different speeds for
        # browser and native speeds too we should note that". Civilization
        # VI is a native app; CIVVIS's games are timed on its native
        # program, and the same games take longer in the browser's
        # WebAssembly build, by the ratio the ledger measured.
        shown = speedup_timeline.timeline(ROOT)
        stops = {stop["key"]: stop for stop in shown["stops"]}
        self.assertEqual(stops["control"]["where"], "native app")
        self.assertEqual(stops["firaxis_self_play"]["where"], "native app")
        self.assertEqual(stops["all_cores"]["where"], "native")
        self.assertNotIn("where", stops["hand"])
        ledger = {stop["key"]: stop for stop in LEDGER["stops"]}
        ratio = ledger["self_play"]["browser"]["ratio"]
        self.assertGreater(ratio, 1)
        browser = stops["self_play"]["seconds"] * ratio
        self.assertEqual(stops["self_play"]["where"], f"native · {speedup_timeline.clock(browser)} in the browser")
        self.assertEqual(stops["self_play"]["browser_seconds"], round(browser, 3))
        self.assertIn("WebAssembly", stops["self_play"]["where_from"])
        # Each measured stop has the line for it on the page, under its
        # figures and over its goal, and the page writes it in.
        blocks = dict(re.findall(r'<li class="speed-stop[^"]*" data-stop="(\w+)">(.*?)</li>', PAGE, re.S))
        for key in ("control", "firaxis_self_play", "self_play", "all_cores"):
            block = blocks[key]
            self.assertLess(block.index('data-speed="where"'), block.index('data-speed="goal"'), key)
        self.assertIn('put(entry, "where", stop.where);', PAGE)

    def test_the_line_runs_out_past_its_last_goal(self):
        # Operator 2026-11-24: "maybe extend timeline out to 10,000,000x".
        # The line's end is the ledger's `line_to`, and no goal: the one
        # goal is still the million he kept that day, costed as it was.
        self.assertEqual(LEDGER["line_to"], 10000000)
        self.assertEqual([goal["key"] for goal in LEDGER["goals"]], ["million"])
        self.assertEqual(speedup_timeline.end(LEDGER), 10000000)
        shown = speedup_timeline.timeline(ROOT)
        self.assertEqual([tick["times"] for tick in shown["ticks"]],
                         ["1x", "10x", "100x", "1,000x", "10,000x", "100,000x", "1 million x", "10 million x"])
        self.assertEqual([tick["at"] for tick in shown["ticks"]], [round(power / 7, 4) for power in range(8)])
        # Every entry stands at its multiplier's place on the longer line,
        # and nothing of it is past the end.
        for row in shown["stops"] + shown["projected"] + shown["goals"]:
            self.assertAlmostEqual(row["at"], math.log10(row["multiplier"]) / 7, places=3, msg=row["key"])
            self.assertNotIn("past", row)
        self.assertEqual([goal["key"] for goal in shown["goals"]], ["million"])
        self.assertIn("an hour in the cloud", shown["goals"][0]["cost"])
        # A ledger without it ends at its largest goal, as every ledger did
        # before, and so does one whose `line_to` is short of that goal. A
        # stop past the end of such a line stands at its end and says so.
        ledger = json.loads(json.dumps(LEDGER))
        del ledger["line_to"]
        self.assertEqual(speedup_timeline.end(ledger), 1000000)
        runs = json.loads((ROOT / BENCHMARK).read_text(encoding="utf-8"))["runs"]
        with tempfile.TemporaryDirectory() as scratch:
            short = speedup_timeline.timeline(tree(scratch, ledger, runs))
        self.assertEqual([goal["at"] for goal in short["goals"]], [1.0])
        self.assertEqual(short["ticks"][-1], {"times": "1 million x", "at": 1.0, "per_game": "18 ms/game", "per_time": "56 games/s"})
        fleet = {stop["key"]: stop for stop in short["projected"]}["fleet"]
        self.assertEqual((fleet["at"], fleet.get("past")), (1.0, True))
        self.assertEqual(speedup_timeline.end(dict(ledger, line_to=1000)), 1000000)
        self.assertEqual(speedup_timeline.end(dict(ledger, line_to=None)), 1000000)

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
        "goals": [{"key": "million", "times": 1000000}],
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

    def test_an_amortized_stop_has_both_measures_and_leads_with_its_games(self):
        ledger = json.loads(json.dumps(self.LEDGER))
        ledger["goals"][0]["amortized"] = True
        ledger["stops"][2]["amortized"] = True
        ledger["projected"] = [{"key": "scale_up", "over": "all_cores", "estimates": [{"taken": "2026-11-23", "times": 10, "or_more": True, "from": "ten"}]}]
        with tempfile.TemporaryDirectory() as scratch:
            shown = speedup_timeline.timeline(tree(scratch, ledger, self.RUNS))
        stops = {stop["key"]: stop for stop in shown["stops"]}
        # 30 games a minute is half a game a second, and the games lead
        # under one a second as they do past it (operator, 2026-11-24, of
        # the M5 Max's stop: "amortiized games per second as the primary
        # unit"). Its goal is worded in them.
        self.assertEqual((stops["all_cores"]["time"], stops["all_cores"]["rate"], stops["all_cores"].get("lead")), ("2 s", "0.50 games/s", "rate"))
        self.assertIs(stops["all_cores"]["amortized"], True)
        self.assertEqual(stops["all_cores"]["goal"], "goal 1 game/s, 18,000x")
        self.assertEqual(stops["self_play"]["goal"], "goal 10 s, 1,800x")
        # One game on one core is not amortized, whatever it takes.
        self.assertNotIn("amortized", stops["self_play"])
        self.assertNotIn("rate", stops["self_play"])
        # Ten times that is five games a second, and more: the games lead.
        (scale_up,) = shown["projected"]
        self.assertEqual((scale_up["time"], scale_up["rate"], scale_up["lead"]), ("200 ms", "5+ games/s", "rate"))
        # A million times a five hour game is 55.6 games a second.
        (million,) = shown["goals"]
        self.assertEqual((million["time"], million["rate"], million["lead"], million["amortized"]), ("18 ms", "56 games/s", "rate", True))
        # At any rate: a game a second, two, or one in a hundred seconds.
        for seconds, games in ((1.0, "1 game/s"), (0.5, "2 games/s"), (100.0, "0.01 games/s")):
            row = {}
            speedup_timeline.amortize(row, seconds)
            self.assertEqual(row, {"amortized": True, "rate": games, "lead": "rate"})
        # A ledger that marks nothing has no second measure anywhere.
        with tempfile.TemporaryDirectory() as scratch:
            plain = speedup_timeline.timeline(tree(scratch, self.LEDGER, self.RUNS))
        self.assertNotIn("amortized", json.dumps(plain))
        self.assertNotIn("rate", json.dumps(plain))

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
        self.assertEqual(shown["goals"], [{"key": "million", "times": "1,000,000x", "multiplier": 1000000, "time": "18 ms", "at": 1.0, "side": "above"}])
        # The line runs by tenfolds from 1x at one end to the goal at the other.
        self.assertEqual([tick["times"] for tick in shown["ticks"]],
                         ["1x", "10x", "100x", "1,000x", "10,000x", "100,000x", "1 million x"])
        self.assertEqual([tick["at"] for tick in shown["ticks"]], [0.0, 0.1667, 0.3333, 0.5, 0.6667, 0.8333, 1.0])
        self.assertAlmostEqual(stops["all_cores"]["at"], 0.659, places=3)
        # No estimate of a cloud's cost in the ledger: none on the page.
        self.assertNotIn("cloud", shown)

    def test_a_projected_stop_is_so_many_times_the_stop_it_names(self):
        ledger = json.loads(json.dumps(self.LEDGER))
        ledger["projected"] = [
            {"key": "scale_up", "over": "all_cores", "estimates": [
                {"taken": "2026-11-22", "times": 4, "from": "an older figure"},
                {"taken": "2026-11-23", "times": 10, "from": "ten times the machine"},
            ]},
            {"key": "fleet", "over": "self_play", "estimates": [{"taken": "2026-11-22", "times": 1000, "or_more": True, "from": "a fleet"}]},
        ]
        with tempfile.TemporaryDirectory() as scratch:
            shown = speedup_timeline.timeline(tree(scratch, ledger, self.RUNS))
        scale_up, fleet = shown["projected"]
        # The newest estimate, times the record that stands (30 games a
        # minute is 9,000x), with the sum that gives it.
        self.assertEqual((scale_up["key"], scale_up["over"], scale_up["by"], scale_up["times"], scale_up["time"]),
                         ("scale_up", "all_cores", "10x", "90,000x", "200 ms"))
        self.assertEqual((scale_up["sum"], scale_up["taken"], scale_up["from"]), ("10 × 9,000", "11-23", "ten times the machine"))
        self.assertAlmostEqual(done_again(scale_up["sum"]), scale_up["multiplier"])
        self.assertAlmostEqual(scale_up["at"], 0.8257, places=3)
        # A figure open at its top says so twice, and any stop can be named.
        self.assertEqual((fleet["over"], fleet["by"], fleet["times"], fleet["time"]), ("self_play", "1,000x+", "900,000x+", "20 ms"))
        # Each has a side of the line, and the page's number is still the
        # fastest stop that was measured.
        self.assertLessEqual({stop["side"] for stop in shown["projected"]}, {"above", "below"})
        self.assertEqual((shown["speedup"], shown["fastest"]), ("9,000x", "all_cores"))
        # A projected stop can stand on one listed before it: so many times
        # that stop's multiplier, which is a guess on a guess.
        ledger["projected"] = [
            {"key": "irreducible", "over": "self_play", "estimates": [{"taken": "2026-11-24", "times": 10, "from": "a guess"}]},
            {"key": "m5_max", "over": "irreducible", "amortized": True, "estimates": [{"taken": "2026-11-24", "times": 10, "from": "a guess on it"}]},
        ]
        with tempfile.TemporaryDirectory() as scratch:
            shown = speedup_timeline.timeline(tree(scratch, ledger, self.RUNS))
        one = {stop["key"]: stop for stop in shown["stops"]}["self_play"]["multiplier"]
        irreducible, m5_max = shown["projected"]
        self.assertAlmostEqual(irreducible["multiplier"], 10 * one, places=1)
        self.assertAlmostEqual(m5_max["multiplier"], 100 * one, places=0)
        self.assertAlmostEqual(done_again(m5_max["sum"]), m5_max["multiplier"], delta=0.001 * m5_max["multiplier"])
        # One game on one core is one game's time. Many at once is
        # amortized, where the ledger says so.
        self.assertNotIn("amortized", irreducible)
        self.assertIs(m5_max["amortized"], True)
        # Listed before the stop it stands on, it cannot be read.
        ledger["projected"].reverse()
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.timeline(tree(scratch, ledger, self.RUNS)))
        self.assertEqual(shown["reach"], {stop["key"]: stop for stop in shown["stops"]}["all_cores"]["at"])
        # A ledger with none shows none, and one that names a stop that is
        # not there cannot be read.
        with tempfile.TemporaryDirectory() as scratch:
            self.assertNotIn("projected", speedup_timeline.timeline(tree(scratch, self.LEDGER, self.RUNS)))
        ledger["projected"][0]["over"] = "nowhere"
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.timeline(tree(scratch, ledger, self.RUNS)))

    def test_a_second_goal_lengthens_the_line_and_the_first_keeps_its_place_on_it(self):
        ledger = json.loads(json.dumps(self.LEDGER))
        ledger["goals"] = [{"key": "billion", "times": 1000000000}, {"key": "million", "times": 1000000}]
        with tempfile.TemporaryDirectory() as scratch:
            shown = speedup_timeline.timeline(tree(scratch, ledger, self.RUNS))
        self.assertEqual([(goal["key"], goal["times"], goal["time"], goal["at"]) for goal in shown["goals"]],
                         [("million", "1,000,000x", "18 ms", 0.6667), ("billion", "1,000,000,000x", "18 µs", 1.0)])
        self.assertEqual([tick["times"] for tick in shown["ticks"]],
                         ["1x", "10x", "100x", "1,000x", "10,000x", "100,000x", "1 million x", "10 million x", "100 million x", "1 billion x"])
        stops = {stop["key"]: stop for stop in shown["stops"]}
        self.assertAlmostEqual(stops["all_cores"]["at"], 0.4394, places=3)
        self.assertEqual(shown["reach"], stops["all_cores"]["at"])

    def test_a_revision_with_no_ledger_shows_nothing(self):
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.timeline(tree(scratch, None)))
        # Nor does one whose followed ledger is missing, or whose ledger is
        # not the shape this reads.
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.timeline(tree(scratch, self.LEDGER)))
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.timeline(tree(scratch, {"goals": [{"key": "million", "times": 1000000}], "stops": []})))
        # Nor one with no goal for its line to end at.
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.timeline(tree(scratch, {**self.LEDGER, "goals": []}, self.RUNS)))

    def test_stops_that_stand_close_together_take_opposite_sides(self):
        # Self-play and the Deity sim are one engine and stand a few pixels
        # apart, so their words go one above the line and one below.
        self.assertEqual(speedup_timeline.sides([0.0, 0.23, 0.44, 0.45, 0.62]),
                         ["below", "above", "below", "above", "below"])
        # In whatever order the ledger lists them.
        self.assertEqual(speedup_timeline.sides([0.45, 0.0, 0.44]), ["below", "below", "above"])

    def test_words_that_would_meet_on_one_side_stand_a_row_out(self):
        # A line that runs to a far goal holds the stops in a short stretch
        # of it. Words are 150px wide on a line of at least 1000px.
        room = speedup_timeline.ROOM
        self.assertAlmostEqual(room, 150 / 1000)
        self.assertEqual(speedup_timeline.rows([0.0, 0.28, 0.42], ["below", "below", "below"]), [False, False, True])
        self.assertEqual(speedup_timeline.rows([0.0, 0.28, 0.42], ["below", "above", "below"]), [False, False, False])
        # In whatever order they are listed, and a third goes back to the
        # row next to the line once there is room for it there.
        self.assertEqual(speedup_timeline.rows([0.41, 0.1, 0.0, 0.25], ["above"] * 4), [False, True, False, False])
        # At an end of the line the words run inward from the point, so
        # their middle is half their width inside it: a stop at the very end
        # is nearer its neighbour than its point is.
        self.assertAlmostEqual(speedup_timeline.middle(0.0), room / 2)
        self.assertAlmostEqual(speedup_timeline.middle(1.0), 1 - room / 2)
        self.assertEqual(speedup_timeline.middle(0.5), 0.5)
        self.assertEqual(speedup_timeline.rows([0.81, 1.0], ["below", "below"]), [False, True])
        self.assertEqual(speedup_timeline.rows([0.7, 1.0], ["below", "below"]), [False, False])
        # The line is drawn from 1120px of window, in a panel of the page,
        # and is then no shorter than the words were given room for: the
        # window less a scrollbar, the page's own margins, and the panel's
        # border and padding, each as the page's rules have them.
        self.assertEqual(PAGE.count("@media (min-width: 1120px) {"), 1)
        shell = re.search(r"^  \.shell \{ width: min\((\d+)px, 100%\); margin: 0 auto; padding: 0 clamp\(\d+px, \d+vw, (\d+)px\); \}$", PAGE, re.M)
        panel = re.search(r"^  \.res-section \{ min-width: 0; padding: \d+px (\d+)px \d+px; border: (\d+)px solid var\(--border\);", PAGE, re.M)
        widest, margin, padding, border = int(shell[1]), int(shell[2]), int(panel[1]), int(panel[2])
        self.assertEqual(widest, 1120)
        scrollbar = 17
        shortest = widest - scrollbar - 2 * margin - 2 * padding - 2 * border
        self.assertGreaterEqual(shortest, 150 / room, "the line can be drawn shorter than its words were given room for")
        shown = speedup_timeline.timeline(ROOT)
        entries = shown["stops"] + shown["projected"] + shown["goals"]
        # Every entry takes its side in turn along the line, whatever kind
        # it is: ten of them do not fit one side, and the pairs that stand
        # close together (the irreducible game and the M5 Max, the M5 Max's
        # most and the scale-up, the fleet and the goal at the line's end)
        # go one over the line and one under it.
        self.assertEqual([entry["side"] for entry in entries], speedup_timeline.sides([entry["at"] for entry in entries]))
        along = sorted(entries, key=lambda entry: entry["at"])
        for one, other in zip(along, along[1:]):
            if other["at"] - one["at"] < room / 2:
                self.assertNotEqual(one["side"], other["side"], (one["key"], other["key"]))
        by_row: dict[tuple[str, bool], list[float]] = {}
        for entry in entries:
            by_row.setdefault((entry["side"], bool(entry.get("far"))), []).append(speedup_timeline.middle(entry["at"]))
        for (side, far), places in by_row.items():
            places.sort()
            for near, next_one in zip(places, places[1:]):
                self.assertGreaterEqual(next_one - near, room, f"two stops' words meet {side} the line{', a row out' if far else ''}")


def done_again(said: str) -> float:
    """A sum as the page writes it, worked out: `18 cores × 18,000 s ÷ 57.1 s`
    with its words dropped and its signs read."""
    bare = re.sub(r"[A-Za-z$]+", " ", re.sub(r"(?<=\d),(?=\d)", "", said)).replace("×", "*").replace("÷", "/").replace("−", "-")
    assert re.fullmatch(r"[\d\s.*/()-]+", bare), said
    return eval(bare)  # noqa: S307 (digits and signs only, checked above)


class Worked(unittest.TestCase):
    """The CIVVIS Performance page's tables (operator 2026-11-22: "at the top
    of civvis performance we want to calculate a multiplier on the
    theoretical and experimetnal throughput of the simualtor. and the
    calculations. baeline is 5 hrs for manually played game. please add the
    deity computer control, deity self-play, civvis self-play.")."""

    LEDGER = {
        "goals": [{"key": "million", "times": 1000000}],
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
        for part, words in [(0.9692, "97%"), (0.679, "68%"), (0.3055, "31%"), (0.0057, "0.6%"), (0.005499, "0.5%"), (1.0, "100%"),
                            (0.0024, "0.2%"), (0.000005499, "0.0005%"), (0.0, "0%")]:
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
                ("million", "goal", ""),
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
        self.assertEqual(("whole" in own_goal, goal["whole"]), (False, True))
        self.assertEqual(shown["goals"], [{"key": "million", "times": "1,000,000x", "words": "1 million x", "time": "18 ms", "a_minute": "3,333", "an_hour": "200,000"}])
        self.assertNotIn("cloud", shown)
        # The opening sentence: self-play one game at a time, on every core
        # as measured, and the highest theory there is for it.
        self.assertEqual(
            shown["answer"],
            {"stop": "self_play", "one": "300x", "one_time": "60 s", "all": "2,400x", "all_time": "7.5 s", "all_an_hour": "480",
             "theory": "5,625x", "theory_basis": "alone", "reaches": "43%"},
        )

    def test_every_sum_shown_gives_the_figure_beside_it(self):
        for shown in (self.shown(), speedup_timeline.worked(ROOT)):
            rows = [row for row in shown["games"][1:] + shown["throughput"] if not row.get("whole")]
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
            self.assertIsNone(speedup_timeline.worked(tree(scratch, {"goals": [{"key": "million", "times": 1000000}], "stops": []})))

    def test_the_line_and_the_tables_read_one_ledger_and_agree(self):
        shown, line = speedup_timeline.worked(ROOT), speedup_timeline.timeline(ROOT)
        stops = {stop["key"]: stop for stop in line["stops"]}
        tabled = shown["games"] + [row for row in shown["throughput"] if row["kind"] == "measured"]
        for row in tabled:
            # A stop kept off the line is in the tables alone.
            if row["key"] in stops:
                self.assertEqual((row["times"], row["time"]), (stops[row["key"]]["times"], stops[row["key"]]["time"]), row["key"])
        self.assertLessEqual(set(stops), {row["key"] for row in tabled})
        # The line's number is the run's.
        self.assertEqual(shown["answer"]["all"], line["speedup"])
        self.assertEqual([(goal["key"], goal["times"], goal["time"]) for goal in shown["goals"]],
                         [(goal["key"], goal["times"], goal["time"]) for goal in line["goals"]])
        # Every goal of the line is a row of the throughput table, the run
        # held against it.
        whole = [row for row in shown["throughput"] if row.get("whole")]
        self.assertEqual([(row["key"], row["times"], row["sum"]) for row in whole], [("million", "1,000,000x", "a game every 18 ms")])
        self.assertRegex(whole[0]["reaches"], r"^\d(\.\d)?%$")
        # The baseline and the ways of playing one game at a time, in the
        # ledger's order, then the theory and the run for self-play.
        self.assertEqual([row["key"] for row in shown["games"]], ["hand", "control", "firaxis_self_play", "deity_sim", "self_play"])
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


class Rented(unittest.TestCase):
    """What renting the rest of the way to a goal is estimated to cost
    (operator 2026-11-22: "Addd 1 billion x to the scaling scale as well. we
    should also estimate the cloud cost in $ to fill i nthe remaining amount
    (note that this is a Fable 5.2 best estimate of the numbers)")."""

    CLOUD = [
        {"taken": "2026-11-23", "by": "A Model", "machine": "Rented Hundred", "cores": 100, "of": "self_play", "single_thread": 2000,
         "dollars_an_hour": 5.0, "spot_dollars_an_hour": 2.0, "from": "an estimate from a list price and a score"},
        {"taken": "2026-11-01", "by": "An Older Model", "machine": "Rented Ten", "cores": 10, "of": "self_play", "single_thread": 1000,
         "dollars_an_hour": 1.0, "from": "an older estimate"},
    ]

    def ledger(self, **changed) -> dict:
        ledger = json.loads(json.dumps(Worked.LEDGER))
        ledger["goals"] = [{"key": "million", "times": 1000000}, {"key": "billion", "times": 1000000000}]
        ledger["cloud"] = json.loads(json.dumps(self.CLOUD))
        ledger.update(changed)
        return ledger

    def both(self, ledger: dict, listed: dict | None = Processors.LISTED) -> tuple[dict, dict]:
        with tempfile.TemporaryDirectory() as scratch:
            source = tree(scratch, ledger, Worked.RUNS, listed)
            return speedup_timeline.worked(source), speedup_timeline.timeline(source)

    def test_a_rented_machine_is_its_cores_at_one_core_s_speed_by_the_two_scores(self):
        cloud = self.both(self.ledger())[0]["cloud"]
        # The newest estimate is the one shown, and it says whose it is.
        self.assertEqual((cloud["by"], cloud["taken"], cloud["machine"], cloud["cores"], cloud["of"]), ("A Model", "2026-11-23", "Rented Hundred", 100, "self_play"))
        self.assertEqual(cloud["from"], "an estimate from a list price and a score")
        # One game on one core: the 60 s one-core reading, longer by the
        # measured processor's 4,000 over the rented one's 2,000.
        self.assertEqual((cloud["seconds"], cloud["one_game"], cloud["one_game_sum"]), (120.0, "2 min", "60 s × 4,000 ÷ 2,000"))
        # The machine: a hundred cores of that.
        self.assertEqual((cloud["times"], cloud["times_sum"], cloud["a_minute"], cloud["an_hour"]), ("15,000x", "100 cores × 18,000 s ÷ 120 s", "50", "3,000"))
        self.assertEqual((cloud["price"], cloud["spot"], cloud["a_year"], cloud["spot_a_year"], cloud["hours_a_year"]), ("$5.00", "$2.00", "$43,800", "$17,500", "8,760"))
        # A million games at 3,000 an hour.
        self.assertEqual((cloud["a_million"], cloud["spot_a_million"]), ("$1,670", "$667"))
        # What the fastest stop reaches is not rented: the every-core run.
        self.assertEqual((cloud["have"], cloud["have_stop"]), ("2,400x", "all_cores"))

    def test_the_rest_of_the_way_to_each_goal_is_whole_machines_at_their_price(self):
        shown, line = self.both(self.ledger())
        million, billion = shown["cloud"]["rows"]
        self.assertEqual((million["key"], million["times"], million["rest"], million["rest_sum"]), ("million", "1,000,000x", "997,600x", "1,000,000 − 2,400"))
        self.assertEqual((million["machines"], million["machines_sum"], million["cores"]), ("67", "997,600 ÷ 15,000 = 66.5, in whole machines", "6,700"))
        self.assertEqual((million["an_hour"], million["an_hour_sum"], million["a_year"]), ("$335", "67 × $5.00", "$2.93 million"))
        self.assertEqual((million["spot_an_hour"], million["spot_an_hour_sum"], million["spot_a_year"]), ("$134", "67 × $2.00", "$1.17 million"))
        self.assertEqual((billion["key"], billion["rest"], billion["machines"], billion["cores"]), ("billion", "999,997,600x", "66,667", "6,666,700"))
        self.assertEqual((billion["machines_sum"], billion["an_hour"], billion["a_year"]), ("999,997,600 ÷ 15,000 = 66,666.5, in whole machines", "$333,000", "$2.92 billion"))
        # Each goal carries its cost on both pages, and the home page says
        # whose estimate it is and of what.
        self.assertEqual([goal["cost"] for goal in shown["goals"]], ["$335", "$333,000"])
        self.assertEqual([goal["cost"] for goal in line["goals"]], ["about $335 an hour in the cloud", "about $333,000 an hour in the cloud"])
        self.assertEqual(line["cloud"], {"by": "A Model", "machine": "Rented Hundred", "price": "$5.00", "taken": "11-23"})

    def test_measured_scaling_takes_the_share_of_its_cores_our_own_run_reached(self):
        # The every-core run: 8 games a minute on 10 cores, whose one core
        # plays a game in 60 s, so ten would play 10 a minute: it reached
        # eight tenths. A rented machine is taken to reach the same share.
        cloud = json.loads(json.dumps(self.CLOUD))
        cloud[0]["scaling"] = "measured"
        scaled = self.both(self.ledger(cloud=cloud))[0]["cloud"]
        whole = self.both(self.ledger())[0]["cloud"]
        self.assertEqual((scaled["scaling"], scaled["scaling_sum"]), ("80%", "8 games a minute ÷ (10 cores × 60 s ÷ 60 s)"))
        self.assertAlmostEqual(done_again(scaled["scaling_sum"]), 0.8)
        self.assertEqual((scaled["times"], scaled["times_sum"]), ("12,000x", "100 cores × 18,000 s ÷ 120 s × 0.8"))
        self.assertEqual((scaled["a_minute"], scaled["an_hour"], scaled["a_million"]), ("40", "2,400", "$2,080"))
        self.assertNotIn("scaling", whole)
        # Fewer games from a machine is more machines for the same goal.
        self.assertEqual((whole["rows"][0]["machines"], scaled["rows"][0]["machines"]), ("67", "84"))
        self.assertEqual(scaled["rows"][0]["machines_sum"], "997,600 ÷ 12,000 = 83.1, in whole machines")

    def test_a_goal_already_reached_rents_nothing_and_an_estimate_may_have_no_spot_price(self):
        ledger = self.ledger(goals=[{"key": "near", "times": 2000}], cloud=[self.CLOUD[1]])
        cloud = self.both(ledger)[0]["cloud"]
        self.assertEqual((cloud["by"], cloud["rows"][0]["rest"], cloud["rows"][0]["machines"], cloud["rows"][0]["an_hour"]), ("An Older Model", "0x", "0", "$0.00"))
        for key in ("spot", "spot_a_million", "spot_a_year"):
            self.assertNotIn(key, cloud)
        self.assertNotIn("spot_an_hour", cloud["rows"][0])

    def test_without_an_estimate_or_the_measured_processor_s_score_no_cost_is_shown(self):
        for ledger, listed in ((self.ledger(cloud=[]), Processors.LISTED), (self.ledger(), None),
                               (self.ledger(cloud=[{**self.CLOUD[0], "of": "nothing"}]), Processors.LISTED)):
            shown, line = self.both(ledger, listed)
            self.assertNotIn("cloud", shown)
            self.assertNotIn("cloud", line)
            self.assertTrue(all("cost" not in goal for goal in shown["goals"] + line["goals"]))
            # The goals and the tables stand without it.
            self.assertEqual(len(line["goals"]), 2)
            self.assertEqual(len(shown["games"]), 4)

    def test_every_sum_shown_gives_the_figure_beside_it(self):
        for cloud in (self.both(self.ledger())[0]["cloud"], speedup_timeline.worked(ROOT)["cloud"]):
            self.assertAlmostEqual(done_again(cloud["one_game_sum"]), cloud["seconds"], delta=0.002 * cloud["seconds"])
            self.assertAlmostEqual(done_again(cloud["times_sum"]), cloud["multiplier"], delta=0.002 * cloud["multiplier"])
            self.assertEqual(cloud["times"], speedup_timeline.times(cloud["multiplier"]))
            price = float(cloud["price"].lstrip("$"))
            for row in cloud["rows"]:
                rest = done_again(row["rest_sum"])
                self.assertEqual(row["rest"], speedup_timeline.times(rest))
                # The machines are the rest over one machine, a whole one
                # for any part of one.
                said, machines = row["machines_sum"].split(" = ")[0], int(row["machines"].replace(",", ""))
                self.assertAlmostEqual(done_again(said), rest / cloud["multiplier"], delta=0.002 * machines)
                self.assertLessEqual(rest / cloud["multiplier"], machines + 0.002 * machines)
                self.assertGreater(rest / cloud["multiplier"], machines - 1 - 0.002 * machines)
                self.assertEqual(int(row["cores"].replace(",", "")), machines * cloud["cores"])
                self.assertEqual(row["an_hour_sum"], f"{row['machines']} × {cloud['price']}")
                self.assertEqual(row["an_hour"], speedup_timeline.dollars(machines * price))

    def test_the_committed_estimate_is_whole_and_says_whose_it_is(self):
        # A new estimate is a new entry after the ones there.
        days = [entry["taken"] for entry in LEDGER["cloud"]]
        self.assertEqual(days, sorted(days))
        one_core = {stop["key"] for stop in LEDGER["stops"] if stop.get("cores") == 1}
        for entry in LEDGER["cloud"]:
            self.assertRegex(entry["taken"], r"^\d{4}-\d{2}-\d{2}$")
            self.assertIn(entry["of"], one_core, "a rented core plays the game of a one-core stop")
            for key in ("cores", "single_thread", "dollars_an_hour"):
                self.assertGreater(entry[key], 0, key)
            self.assertLessEqual(entry.get("spot_dollars_an_hour", 0), entry["dollars_an_hour"])
            self.assertTrue(entry["by"] and entry["machine"])
            # It says for itself that it is an estimate and not a quote.
            self.assertIn("estimate", entry["from"])
            self.assertIn("Not a quote", entry["from"])
        # The operator: "note that this is a Fable 5.2 best estimate of the
        # numbers".
        self.assertEqual(LEDGER["cloud"][-1]["by"], "Claude Fable 5.2")
        # Nothing derived is kept: an entry is the machine, its score and
        # its prices, and no cost is written in the ledger.
        for entry in LEDGER["cloud"]:
            self.assertLessEqual(set(entry), {"taken", "by", "machine", "cores", "of", "single_thread", "dollars_an_hour", "spot_dollars_an_hour", "scaling", "from"})
            self.assertIn(entry.get("scaling", "measured"), ("measured",))
        shown, line = speedup_timeline.worked(ROOT), speedup_timeline.timeline(ROOT)
        self.assertEqual(shown["cloud"]["by"], "Claude Fable 5.2")
        self.assertEqual(line["cloud"]["by"], "Claude Fable 5.2")
        self.assertEqual([row["key"] for row in shown["cloud"]["rows"]], ["million"])
        # The estimate worked out again on 2026-11-24 (operator: "reclauclate
        # the cloud costs to make sure they are accurate") takes a rented
        # machine to reach the share of its cores that our own every-core
        # run reached, and shows the sum for that share.
        self.assertEqual((LEDGER["cloud"][-1]["taken"], LEDGER["cloud"][-1].get("scaling")), ("2026-11-24", "measured"))
        self.assertRegex(shown["cloud"]["scaling"], r"^\d{2}%$")
        part = done_again(shown["cloud"]["scaling_sum"])
        self.assertTrue(0.3 < part <= 1.0, part)
        self.assertAlmostEqual(part, int(shown["cloud"]["scaling"].rstrip("%")) / 100, delta=0.006)
        # It is the run that is short of the goals, and a larger goal costs more.
        self.assertEqual(shown["cloud"]["have"], shown["answer"]["all"])
        machines = [int(row["machines"].replace(",", "")) for row in shown["cloud"]["rows"]]
        self.assertEqual(machines, sorted(machines))
        self.assertGreater(machines[0], 0)
        for goal in line["goals"]:
            self.assertRegex(goal["cost"], r"^about \$[\d,.]+( million| billion)? an hour in the cloud$")


class History(unittest.TestCase):
    """Each way of playing over time, for the Performance page's charts
    (operator 2026-11-24: "in civvis performance, we should have charts
    studying the improvements rates of each of these over time")."""

    def test_a_pace_is_how_many_times_faster_over_how_long(self):
        pace = speedup_timeline.pace([{"taken": "2026-10-01", "seconds": 400}, {"taken": "2026-10-15", "seconds": 300}, {"taken": "2026-10-29", "seconds": 100}])
        self.assertEqual((pace["readings"], pace["days"], pace["over"], pace["times"], pace["factor"]), (3, 28, "4 weeks", "4x", 4.0))
        # Four times faster in 28 days is a halving every 14.
        self.assertEqual((pace["halves"], pace["first"], pace["last"]), ("2 weeks", "6.7 min", "1.7 min"))
        self.assertNotIn("steady", pace)
        self.assertNotIn("slower", pace)

    def test_a_short_span_has_no_halving_time_and_one_reading_no_pace(self):
        pace = speedup_timeline.pace([{"taken": "2026-11-22", "seconds": 57.1}, {"taken": "2026-11-24", "seconds": 34.5}])
        self.assertEqual((pace["days"], pace["over"], pace["times"]), (2, "2 days", "1.66x"))
        self.assertNotIn("halves", pace)
        self.assertEqual(speedup_timeline.pace([{"taken": "2026-11-22", "seconds": 57.1}]), {"readings": 1})
        self.assertEqual(speedup_timeline.pace([]), {"readings": 0})
        # Two readings on one day are no span of time.
        self.assertEqual(speedup_timeline.pace([{"taken": "2026-11-22", "seconds": 9}, {"taken": "2026-11-22", "seconds": 8}]), {"readings": 2})

    def test_a_pace_that_has_not_moved_is_steady_and_one_that_slowed_says_so(self):
        steady = speedup_timeline.pace([{"taken": "2026-11-01", "seconds": 757}, {"taken": "2026-11-15", "seconds": 753}])
        self.assertIs(steady["steady"], True)
        slowed = speedup_timeline.pace([{"taken": "2026-11-01", "seconds": 100}, {"taken": "2026-11-15", "seconds": 200}])
        self.assertEqual((slowed["slower"], slowed["doubles"], slowed["times"]), ("2x", "2 weeks", "0.5x"))
        self.assertNotIn("halves", slowed)
        self.assertNotIn("steady", slowed)

    def test_a_stop_s_line_is_every_reading_and_a_follower_s_pace_is_over_its_records(self):
        ledger = json.loads(json.dumps(Following.LEDGER))
        ledger["stops"].insert(2, {"key": "aside", "line": False, "readings": [{"taken": "2026-11-22", "seconds": 5, "from": "kept for the tables alone"}]})
        with tempfile.TemporaryDirectory() as scratch:
            shown = speedup_timeline.history(tree(scratch, ledger, Following.RUNS))
        # The baseline and a stop kept off the line have no chart.
        self.assertEqual([row["key"] for row in shown["rows"]], ["self_play", "all_cores"])
        self.assertEqual((shown["base"], shown["base_time"], shown["from"], shown["to"]), (18000, "5 h", "2026-11-22", "2026-12-01"))
        own, run = shown["rows"]
        self.assertEqual([line["kind"] for line in own["lines"]], ["readings"])
        self.assertEqual([(point["taken"], point["seconds"], point["time"]) for point in own["lines"][0]["points"]],
                         [("2026-11-22", 40, "40 s"), ("2026-12-01", 20, "20 s")])
        self.assertEqual((own["lines"][0]["pace"]["times"], own["lines"][0]["pace"]["days"], own["goal"], own["goal_seconds"]), ("2x", 9, "10 s", 10))
        # Every run is drawn, the slower ones too, and the pace is from the
        # first record to the one that stands: 18 a minute to 30.
        self.assertEqual([point["seconds"] for point in run["lines"][0]["points"]], [round(60 / n, 3) for n in (18.0, 17.0, 30.0, 24.0)])
        self.assertEqual((run["lines"][0]["pace"]["readings"], run["lines"][0]["pace"]["days"], run["lines"][0]["pace"]["times"]), (2, 3, "1.67x"))

    def test_the_tournament_s_batches_stand_beside_the_stop_whose_game_they_play(self):
        ledger = json.loads(json.dumps(Following.LEDGER))
        ledger["stops"][1]["tournament"] = True
        batches = [
            {"batch": "20261101T000000Z-aaaa", "began": "2026-10-20", "ended": "2026-11-01", "games": 900, "p": [50, 80, 300.0, 600, 900]},
            {"batch": "20261122T000000Z-bbbb", "began": "2026-11-21", "ended": "2026-11-22", "games": 1000, "p": [20, 40, 75.0, 150, 300]},
        ]
        with tempfile.TemporaryDirectory() as scratch:
            source = tree(scratch, ledger, Following.RUNS)
            without = speedup_timeline.history(source)
            (source / speedup_timeline.SELF_PLAY).write_text(json.dumps({"batches": batches}), encoding="utf-8")
            shown = speedup_timeline.history(source)
        # A tree with no snapshot of the batches draws the readings alone.
        self.assertEqual([line["kind"] for line in without["rows"][0]["lines"]], ["readings"])
        lines = shown["rows"][0]["lines"]
        self.assertEqual([line["kind"] for line in lines], ["readings", "tournament"])
        # The middle game of each batch, on the day the batch ended.
        self.assertEqual([(point["taken"], point["seconds"], point["games"]) for point in lines[1]["points"]],
                         [("2026-11-01", 300.0, 900), ("2026-11-22", 75.0, 1000)])
        self.assertEqual((lines[1]["pace"]["times"], lines[1]["pace"]["over"]), ("4x", "3 weeks"))
        self.assertEqual(shown["from"], "2026-11-01")
        # The other stop has no such line.
        self.assertEqual([line["kind"] for line in shown["rows"][1]["lines"]], ["readings"])

    RECORD = {
        "days": [
            {"day": "2026-08-01", "games": 5, "p": [1500, 2000, 2600]},
            {"day": "2026-08-02", "games": 8, "p": [1400, 1800, 2500]},
            {"day": "2026-12-05", "games": 60, "p": [400, 480, 700]},
        ],
        "first": {"from": "2026-08-01", "to": "2026-08-07", "games": 40, "p": [1450, 2000, 2550]},
        "week": {"from": "2026-11-29", "to": "2026-12-05", "games": 400, "p": [420, 500, 690]},
    }

    def patched(self, record, ledger):
        """What the module works out with the benchmark's record as given."""
        real = speedup_timeline.control_times.control
        try:
            speedup_timeline.control_times.control = lambda source: record
            with tempfile.TemporaryDirectory() as scratch:
                source = tree(scratch, ledger, Worked.RUNS)
                return speedup_timeline.timeline(source), speedup_timeline.worked(source), speedup_timeline.history(source)
        finally:
            speedup_timeline.control_times.control = real

    def test_a_stop_that_reads_a_benchmark_shows_its_record_and_falls_back_to_its_readings(self):
        # Operator 2026-11-24: "we should be benchmarking our civvis computer
        # cnotrol games time end to end and that number show flow through".
        ledger = json.loads(json.dumps(Worked.LEDGER))
        ledger["stops"][1]["benchmark"] = "control"
        line, tables, charts = self.patched(self.RECORD, ledger)
        stop = next(stop for stop in line["stops"] if stop["key"] == "control")
        # The middle game of the record's last seven days is the stop.
        self.assertEqual((stop["seconds"], stop["time"], stop["times"], stop["taken"]), (500, "8.3 min", "36x", "12-05"))
        self.assertIn("400 games", stop["from"])
        self.assertIn("A tenth took under 7 min and a tenth over 11.5 min", stop["from"])
        # It moved from the reading written down, and both pages say so.
        self.assertEqual(stop["was"], "24x on 11-22")
        self.assertEqual(next(row for row in tables["games"] if row["key"] == "control")["times"], "36x")
        # Its chart is the record's days, and its pace is first week to last.
        row = next(row for row in charts["rows"] if row["key"] == "control")
        self.assertEqual([one["kind"] for one in row["lines"]], ["days"])
        self.assertEqual([(point["taken"], point["seconds"], point["games"]) for point in row["lines"][0]["points"]],
                         [("2026-08-01", 2000, 5), ("2026-08-02", 1800, 8), ("2026-12-05", 480, 60)])
        pace = row["lines"][0]["pace"]
        self.assertEqual((pace["times"], pace["days"], pace["readings"]), ("4x", 120, 3))
        self.assertEqual(row["now"], {"time": "8.3 min"})
        # A clone without the record shows the reading written down.
        for record in (None, {}, {"week": {"to": "2026-12-05", "games": 0, "p": [0, 0, 0]}}):
            line, _, charts = self.patched(record, ledger)
            stop = next(stop for stop in line["stops"] if stop["key"] == "control")
            self.assertEqual((stop["seconds"], stop["taken"]), (750, "11-22"))
            row = next(row for row in charts["rows"] if row["key"] == "control")
            self.assertEqual([one["kind"] for one in row["lines"]], ["readings"])
        # And a record older than the newest reading written down does not
        # take its place.
        stale = json.loads(json.dumps(self.RECORD))
        stale["week"]["to"] = "2026-11-01"
        line, _, _ = self.patched(stale, ledger)
        self.assertEqual(next(stop for stop in line["stops"] if stop["key"] == "control")["seconds"], 750)
        # A stop that names no benchmark never reads one.
        line, _, _ = self.patched(self.RECORD, Worked.LEDGER)
        self.assertEqual(next(stop for stop in line["stops"] if stop["key"] == "control")["seconds"], 750)

    def test_the_benchmark_hours_are_a_line_on_self_play_and_on_every_core(self):
        # The hours are on the `ledger` branch, read by `self_play_hours`;
        # here they are handed over as that reader gives them.
        ledger = json.loads(json.dumps(Worked.LEDGER))
        ledger["stops"][3]["tournament"] = True
        hours = [
            {"taken": "2026-11-24T18:00:00Z", "games": 90, "games_an_hour": 90.0, "cpu_a_game": 50.0},
            {"taken": "2026-11-25T00:00:00Z", "games": 120, "games_an_hour": 120.0, "cpu_a_game": 40.0},
            {"taken": "2026-11-25T06:00:00Z", "games": 0, "games_an_hour": 0, "cpu_a_game": None},
        ]
        real = speedup_timeline.self_play_hours.self_play
        try:
            speedup_timeline.self_play_hours.self_play = lambda source: {"hours": hours}
            with tempfile.TemporaryDirectory() as scratch:
                shown = speedup_timeline.history(tree(scratch, ledger, Worked.RUNS))
            speedup_timeline.self_play_hours.self_play = lambda source: None
            with tempfile.TemporaryDirectory() as scratch:
                without = speedup_timeline.history(tree(scratch, ledger, Worked.RUNS))
        finally:
            speedup_timeline.self_play_hours.self_play = real
        rows = {row["key"]: row for row in shown["rows"]}
        # On self-play: the processor seconds one game took, each hour at
        # its own moment. An hour that finished no game is no point.
        own = {line["kind"]: line for line in rows["self_play"]["lines"]}
        self.assertEqual([(point["taken"], point["at"], point["seconds"], point["games"]) for point in own["hours"]["points"]],
                         [("2026-11-24", "2026-11-24T18:00:00Z", 50.0, 90), ("2026-11-25", "2026-11-25T00:00:00Z", 40.0, 120)])
        self.assertEqual(own["hours"]["pace"]["times"], "1.25x")
        # On every core: the time from one finished game to the next.
        run = {line["kind"]: line for line in rows["all_cores"]["lines"]}
        self.assertEqual([(point["seconds"], point["time"]) for point in run["production"]["points"]], [(40.0, "40 s"), (30.0, "30 s")])
        # The other stops have neither line, and nor has a clone without the hours.
        for key in ("control", "deity_sim"):
            self.assertEqual([line["kind"] for line in rows[key]["lines"]], ["readings"])
        for row in without["rows"]:
            self.assertEqual([line["kind"] for line in row["lines"]], ["readings"])

    def test_the_committed_ledger_has_a_chart_for_every_stop_after_the_baseline(self):
        shown, line = speedup_timeline.history(ROOT), speedup_timeline.timeline(ROOT)
        self.assertEqual([row["key"] for row in shown["rows"]], [stop["key"] for stop in line["stops"][1:]])
        for row, stop in zip(shown["rows"], line["stops"][1:]):
            own = row["lines"][0]
            # Its readings, or its benchmark's days where the clone holds
            # that record.
            self.assertIn(own["kind"], ("readings", "days"))
            self.assertEqual(own["kind"] == "days", speedup_timeline.benchmark(ROOT, next(s for s in LEDGER["stops"] if s["key"] == row["key"])) is not None)
            days = [point["taken"] for point in own["points"]]
            self.assertEqual(days, sorted(days))
            # The figure over the chart is the stop's on the line.
            self.assertEqual(row["now"]["time"], stop["time"])
            self.assertEqual(bool(row["now"].get("worked")), bool(stop.get("worked")))
            if own["kind"] == "readings":
                self.assertLessEqual(stop["seconds"], min(point["seconds"] for point in own["points"]) + 1e-9)
            self.assertEqual(own["pace"]["readings"] >= 1, True)
        tournament = [row["key"] for row in shown["rows"] if any(line["kind"] == "tournament" for line in row["lines"])]
        self.assertEqual(tournament, ["self_play"])
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(speedup_timeline.history(pathlib.Path(scratch)))


class Gap(unittest.TestCase):
    """Why one stop is so many times faster than another (operator,
    2026-11-24: "what is the main reason civvis is so much faster than
    firaxis civ 6? please try to explain that gap in the chart/timeline")."""

    #: A real game of 300 s (30 s to its first turn, 250 s of play over 100
    #: turns, 20 s of ending) whose game thread is on a processor for three
    #: fifths of its play, and a simulator game of 25 s over 125 turns.
    LEDGER = {
        "goals": [{"key": "million", "times": 1000000}],
        "gap": {"slow": "real", "fast": "sim", "threads": {"play_seconds": 200.0, "cpu_seconds": 640.0, "game_seconds": 120.0, "note": "How the threads were read.", "from": "The whole account."}},
        "stops": [
            {"key": "hand", "readings": [{"taken": "2026-11-22", "seconds": 18000, "from": "By hand."}]},
            {"key": "real", "readings": [{"taken": "2026-11-24", "seconds": 300, "turns": 100, "parts": {"setup": 30, "play": 250, "ending": 20}, "from": "Timed."}]},
            {"key": "sim", "readings": [{"taken": "2026-11-24", "seconds": 25, "turns": 125, "cpu_seconds": 24.5, "from": "Twelve games."}]},
        ],
    }

    def shown(self, change=None) -> dict | None:
        ledger = json.loads(json.dumps(self.LEDGER))
        if change:
            change(ledger)
        with tempfile.TemporaryDirectory() as scratch:
            return speedup_timeline.timeline(tree(scratch, ledger))

    def test_the_gap_is_the_slower_stop_s_time_over_the_faster_s_between_their_places(self):
        line = self.shown()
        between = line["gap"]
        self.assertEqual((between["slow"], between["fast"], between["times"]), ("real", "sim", "12x"))
        places = {stop["key"]: stop["at"] for stop in line["stops"]}
        self.assertEqual((between["from"], between["to"]), (places["real"], places["sim"]))
        self.assertLess(between["from"], between["to"])

    def test_the_slower_game_is_in_the_parts_its_time_goes_to_and_they_multiply_to_the_gap(self):
        between = self.shown()["gap"]
        slow, fast = between["bars"]
        self.assertEqual((slow["key"], slow["time"], slow["share"]), ("real", "5 min", 1))
        # From the bars' shared start: the game thread at work (three fifths
        # of 250 s), the same thread waiting, and the time outside the turns.
        self.assertEqual(slow["parts"], [
            {"key": "works", "time": "2.5 min", "share": 0.5, "times": "6x"},
            {"key": "waits", "time": "1.7 min", "share": 0.3333, "times": "1.7x"},
            {"key": "outside", "time": "50 s", "share": 0.1667, "times": "1.2x"},
        ])
        self.assertAlmostEqual(sum(part["share"] for part in slow["parts"]), 1, places=3)
        # 300 s to 250 s, to 150 s, to 25 s: the three factors are the gap.
        self.assertAlmostEqual((300 / 250) * (250 / 150) * (150 / 25), 300 / 25)
        # The faster game is one part, at work, and as long as its share.
        self.assertEqual(fast, {"key": "sim", "time": "25 s", "share": 0.0833, "parts": [{"key": "works", "time": "25 s", "share": 0.0833}]})

    def test_the_two_games_stand_side_by_side_as_pairs_of_figures(self):
        between = self.shown()["gap"]
        self.assertEqual(between["facts"], [
            # 250 s over 100 turns, and 25 s over 125.
            {"key": "turn", "slow": "2.5 s", "fast": "200 ms", "times": "12x"},
            # 640 processor seconds in 200 s of play, and 24.5 in 25.
            {"key": "cores", "slow": "3.2", "fast": "1"},
            # The game thread's 120 s of those 200, and the simulator's one
            # thread's 24.5 s of its 25.
            {"key": "busy", "slow": "60%", "fast": "98%"},
        ])
        for said in ("5 min", "30 s", "4.2 min", "20 s", "100 turns", "125", "60%", "3.2 processors", "How the threads were read."):
            self.assertIn(said, between["counted"])
        # The page is given the few words, not the whole account.
        self.assertNotIn("The whole account.", between["counted"])

    def test_without_the_threads_the_play_is_one_part(self):
        def bare(ledger):
            del ledger["gap"]["threads"]
        between = self.shown(bare)["gap"]
        self.assertEqual([part["key"] for part in between["bars"][0]["parts"]], ["play", "outside"])
        self.assertEqual([part["times"] for part in between["bars"][0]["parts"]], ["10x", "1.2x"])
        self.assertEqual(between["bars"][1]["parts"][0]["key"], "play")
        self.assertEqual([fact["key"] for fact in between["facts"]], ["turn"])

    def test_without_the_parts_there_is_the_gap_and_nothing_under_it(self):
        def bare(ledger):
            del ledger["stops"][1]["readings"][0]["parts"]
        between = self.shown(bare)["gap"]
        self.assertEqual(set(between), {"slow", "fast", "times", "from", "to"})

    def test_a_newer_reading_of_the_slower_stop_is_the_one_it_is_drawn_from(self):
        def newer(ledger):
            ledger["stops"][1]["readings"].append({"taken": "2026-11-26", "seconds": 400, "turns": 150, "parts": {"setup": 30, "play": 350, "ending": 20}, "from": "Timed again."})
        between = self.shown(newer)["gap"]
        self.assertEqual((between["times"], between["bars"][0]["time"]), ("16x", "6.7 min"))
        # The threads' share of the play is the ledger's one measurement of
        # it, whichever game's play it is taken of.
        self.assertEqual(between["bars"][0]["parts"][0], {"key": "works", "time": "3.5 min", "share": 0.525, "times": "8.4x"})

    def test_a_gap_the_line_cannot_draw_is_not_shown(self):
        def unknown(ledger):
            ledger["gap"]["fast"] = "nobody"
        def backwards(ledger):
            ledger["gap"]["slow"], ledger["gap"]["fast"] = "sim", "real"
        def off_the_line(ledger):
            ledger["stops"][2]["line"] = False
        def none(ledger):
            del ledger["gap"]
        def broken(ledger):
            ledger["gap"]["threads"] = {"play_seconds": 0}
        for change in (unknown, backwards, off_the_line, none, broken):
            line = self.shown(change)
            self.assertIsNotNone(line, change.__name__)
            self.assertNotIn("gap", line, change.__name__)

    def test_the_committed_ledger_explains_civvis_self_play_over_firaxis_self_play(self):
        asked = LEDGER["gap"]
        self.assertEqual((asked["slow"], asked["fast"]), ("firaxis_self_play", "self_play"))
        stops = {stop["key"]: stop for stop in LEDGER["stops"]}
        slow, fast = stops[asked["slow"]]["readings"][-1], stops[asked["fast"]]["readings"][-1]
        # The slower game's parts are its whole time, and its turns are said.
        self.assertAlmostEqual(sum(slow["parts"].values()), slow["seconds"], places=1)
        self.assertEqual(set(slow["parts"]), {"setup", "play", "ending"})
        self.assertGreater(slow["turns"], 0)
        # The faster game's processor time is no more than its one core's.
        self.assertGreater(fast["turns"], 0)
        self.assertLessEqual(fast["cpu_seconds"], fast["seconds"])
        self.assertGreater(fast["cpu_seconds"], 0.9 * fast["seconds"])
        # The threads were read of a real game: a thread cannot be on a
        # processor for longer than the clock ran, the process's time is at
        # least its named threads', and the reading says how it was taken.
        threads = asked["threads"]
        self.assertRegex(threads["taken"], r"^\d{4}-\d{2}-\d{2}$")
        self.assertRegex(threads["run"], r"^firaxis-selfplay-\d{8}T\d{6}Z$")
        for key in ("game_seconds", "window_seconds"):
            self.assertLessEqual(threads[key], threads["play_seconds"], key)
        self.assertGreaterEqual(threads["cpu_seconds"], threads["game_seconds"] + threads["window_seconds"] + threads["workers_seconds"])
        self.assertLess(threads["from_turn"], threads["to_turn"])
        self.assertIn("tools/civ6_thread_time.py", threads["from"])
        self.assertLessEqual(len(threads["note"].split()), 60)
        # The samples of the stacks: a thread is parked or passing its turn
        # in no more of them than were taken.
        stacks = threads["stacks"]
        self.assertLessEqual(stacks["game_parked"] + stacks["game_yielding"], stacks["samples"])
        self.assertLessEqual(stacks["window_yielding"], stacks["samples"])
        self.assertTrue(all(threads["from_turn"] <= turn <= threads["to_turn"] for turn in stacks["turns"]))
        # What the page is given of it.
        line = speedup_timeline.timeline(ROOT)
        between = line["gap"]
        places = {stop["key"]: stop["at"] for stop in line["stops"]}
        self.assertEqual((between["from"], between["to"]), (places["firaxis_self_play"], places["self_play"]))
        self.assertEqual(between["times"], speedup_timeline.times(slow["seconds"] / fast["seconds"]))
        self.assertEqual([part["key"] for part in between["bars"][0]["parts"]], ["works", "waits", "outside"])
        self.assertEqual([fact["key"] for fact in between["facts"]], ["turn", "cores", "busy"])
        # Nothing derived is kept in the ledger: no share, no factor.
        def keys(value):
            if isinstance(value, dict):
                return set(value) | {key for inner in value.values() for key in keys(inner)}
            return set()
        self.assertFalse(keys(asked) & {"share", "times", "multiplier", "busy", "cores"})


class Page(unittest.TestCase):
    SECTION = PAGE[PAGE.index('<section class="res-section" id="perf-line"') : PAGE.index('<section class="res-section" id="perf-control"')]
    SCRIPT = PAGE[PAGE.index("// ---- The same figures as a line"):PAGE.index("// ---- Each way of playing over time")]

    def test_the_page_has_an_entry_for_every_stop_and_no_figure_of_its_own(self):
        entries = re.findall(r'<li class="speed-stop" data-stop="(\w+)">', PAGE)
        self.assertEqual(entries, [stop["key"] for stop in LEDGER["stops"] if stop.get("line") is not False])
        # Its figures are the page's one slot's, the record's `timeline`.
        self.assertEqual(PAGE.count('<script type="application/json" id="performance-data">null</script>'), 1)
        self.assertIn('const line = data.timeline && typeof data.timeline === "object" ? data.timeline : null;', PAGE)
        self.assertIn("const fitLine = !line || !Array.isArray(line.stops) ? null : (() => {", PAGE)
        # The timeline is a part of CIVVIS Performance, between the tables
        # of sums and each way of playing over time, and hidden until it is
        # filled.
        section = self.SECTION
        # It has the operator's title over it (2026-11-24: "great new chart
        # for lets title "Civ Game Speedup - Synthetic Data Pipeline""), and
        # the title is the section's name, set as every part's is here.
        self.assertIn('<section class="res-section" id="perf-line" aria-labelledby="perf-line-title" hidden>\n'
                      '        <h2 class="rank-title" id="perf-line-title">Civ Game Speedup - Synthetic Data Pipeline</h2>\n'
                      '        <div class="speed">\n'
                      '          <div class="speed-head">', section)
        self.assertEqual(PAGE.count("Civ Game Speedup - Synthetic Data Pipeline</h2>"), 1)
        self.assertIn("section.hidden = false;", self.SCRIPT)
        for place in ("speedup", "was", "cloud-by"):
            self.assertEqual(section.count(f'data-speed="{place}"'), 1, place)
        # The machine rented and its price are the cloud's table's to say,
        # two tables up, where the note's link goes: the line says whose
        # estimate it is and no more (operator, 2026-11-24: "there is too
        # much text on pages, particualrly home/ and charts/").
        for place in ("cloud-machine", "cloud-price"):
            self.assertNotIn(f'data-speed="{place}"', PAGE)
        # The goals are the line's last entries, one for each in the ledger,
        # smallest first, and each stands at its own place as a stop does:
        # the largest is the line's left end.
        goals = re.findall(r'<li class="speed-stop speed-finish" data-goal="(\w+)">', PAGE)
        self.assertEqual(goals, [goal["key"] for goal in speedup_timeline.marked(LEDGER)])
        self.assertLess(section.rindex('data-stop="'), section.index('data-goal="'))
        # On this page each of them is named Theoretical (operator,
        # 2026-11-23: "replace "goal" on home for 1Bx and 1Mx with
        # "theoretical""): nothing has run at a million or a billion times.
        # A stop's own goal, the small line under its time, keeps the word.
        named = re.findall(r'<li class="speed-stop speed-finish" data-goal="\w+">.*?<b class="speed-name">([^<]+)</b>', section, re.S)
        self.assertEqual(named, ["Theoretical"] * len(goals))
        self.assertNotIn('<b class="speed-name">Goal</b>', PAGE)
        self.assertNotIn("to a goal", re.sub(r"<!--.*?-->", "", section, flags=re.S))
        self.assertNotIn(".speed-finish { right:", PAGE)
        self.assertIn('entry.style.setProperty("--at", goal.at);', PAGE)
        # Between the stops and the goals, the stops past what was measured
        # (operator, 2026-11-22: "hardware scaleup is to the left of m5
        # max"): one for each in the ledger, a ring like a goal's, and how
        # many times the M5 Max it is comes from the slot.
        ahead = re.findall(r'<li class="speed-stop speed-ahead" data-ahead="(\w+)">', PAGE)
        self.assertEqual(ahead, [stop["key"] for stop in LEDGER["projected"]])
        self.assertLess(section.rindex('data-stop="'), section.index('data-ahead="'))
        self.assertLess(section.rindex('data-ahead="'), section.index('data-goal="'))
        # Named in the operator's words: the two guesses of 2026-11-24
        # ("Civ 6 Computationl Irreducability", "M5 Max, Theoretical Max Civ
        # 6 Throughput") and the scale-up as he renamed it that day ("lets
        # change hardware scale-up to "Reasonable Local Hardware Scale-up
        # Theoretical"). A name of several parts is several lines on the line.
        names = re.findall(r'<li class="speed-stop speed-ahead" data-ahead="\w+">.*?<b class="speed-name">(.*?)</b>', section, re.S)
        self.assertEqual([re.sub(r"<[^>]+>", "", name) for name in names],
                         ["Civ 6 Computational Irreducibility", "M5 Max, Theoretical Max Civ 6 Throughput",
                          "Reasonable Local Hardware Scale-up Theoretical", "Distributed RL fleet"])
        self.assertNotIn("Hardware scale-up", section)
        self.assertIn('<small class="speed-goal">a guess, <span data-speed="by"></span> self-play</small>', section)
        self.assertIn('<small class="speed-goal">a guess, <span data-speed="by"></span> irreducibility</small>', section)
        self.assertIn('<small class="speed-goal"><span data-speed="by"></span> M5 Max theoretical</small>', section)
        self.assertIn('<small class="speed-goal">potential, up to <span data-speed="by"></span></small>', section)
        self.assertEqual(section.count('data-speed="by"'), len(ahead))
        # A ring is hollow in the colour of the panel it stands on.
        self.assertIn(".speed-ahead .speed-dot { background: var(--panel); border: 2px solid var(--accent);", PAGE)
        self.assertIn(".speed-finish .speed-dot { background: var(--panel); border: 2px solid var(--gold);", PAGE)
        rules = PAGE[PAGE.index("  .speed { margin:"):PAGE.index("  /* One tooltip for both charts")]
        self.assertNotIn("var(--bg)", rules)
        self.assertIn('entry.style.setProperty("--at", stop.at);', PAGE)
        # A goal stands on the side the slot gives it, as every entry does.
        self.assertIn('entry.classList.add(goal.side === "below" ? "is-below" : "is-above");', PAGE)
        for place in ("time", "times"):
            self.assertEqual(section.count(f'data-speed="{place}"'), len(entries) + len(ahead) + len(goals), place)
        self.assertEqual(section.count('data-speed="goal"'), len(entries))
        self.assertEqual(section.count('data-speed="cost"'), len(goals))
        # An amortized entry has a second measure under its figures, in
        # brackets, which says so (operator 2026-11-24: "we should note that
        # estimates are amortized games", and "provide the games per second
        # as the standard measure and (time per game amoritized as the
        # second measure)"). Every entry the slot marks has the line for it,
        # right under its figures, and no other entry has one.
        shown = speedup_timeline.timeline(ROOT)
        marked = {entry["key"] for entry in shown["stops"] + shown["projected"] + shown["goals"] if entry.get("amortized")}
        blocks = re.findall(r'<li class="speed-stop[^"]*" data-(?:stop|ahead|goal)="(\w+)">(.*?)</li>', section, re.S)
        self.assertEqual({key for key, block in blocks if 'data-speed="second"' in block}, marked)
        for key, block in blocks:
            if key in marked:
                self.assertIn('<span class="speed-read"><b data-speed="time"></b> <i data-speed="times"></i></span>\n'
                              '                  <small class="speed-second" data-speed="second" hidden></small>', block, key)
        self.assertIn('const fast = row.amortized && row.lead === "rate";', PAGE)
        self.assertIn('put(entry, "time", fast ? row.rate : row.time);', PAGE)
        self.assertIn('put(entry, "second", row.amortized ? "(" + (fast ? row.time + " a game" : row.rate) + ", amortized)" : "");', PAGE)
        # And once under the line, what the word means: shown when any
        # entry is amortized.
        self.assertIn('<p class="speed-note" id="speed-amortized" hidden>Amortized: many games play at once, and the time is from one finished game to the next.</p>', section)
        self.assertIn('document.getElementById("speed-amortized").hidden = !amortized;', PAGE)
        # Under the line, whose estimate a goal's cloud cost is (operator
        # 2026-11-22: "note that this is a Fable 5.2 best estimate of the
        # numbers"): hidden until the slot names one, and the name is the
        # slot's.
        self.assertIn('<p class="speed-note" id="speed-note" hidden>', section)
        self.assertIn('Cloud cost: a best estimate by <span data-speed="cloud-by"></span>, not a quote.', section)
        self.assertIn('<a href="#perf-cloud">How it is worked out</a>', section)
        self.assertEqual(PAGE.count('id="perf-cloud"'), 1)
        self.assertIn('document.getElementById("speed-note").hidden = false;', PAGE)
        self.assertNotIn("Fable", section)
        # Words that would meet their neighbour's stand a row out, and how
        # far out, with the room each side of the line takes, is measured
        # from the words as they are written in: a name is one line or
        # three, and a stop may have a second measure and a goal.
        self.assertIn('entry.classList.toggle("is-far", !!stop.far);', PAGE)
        self.assertIn('entry.classList.toggle("is-far", !!goal.far);', PAGE)
        self.assertIn(".speed-stop.is-far.is-above .speed-label { bottom: var(--out, 69px); }", PAGE)
        self.assertIn(".speed-stop.is-far.is-below .speed-label { top: var(--out, 87px); }", PAGE)
        self.assertIn('own.entry.style.setProperty("--out", out + "px");', PAGE)
        self.assertIn('track.style.setProperty(side === "above" ? "--over" : "--under", Math.ceil(reach + 6) + "px");', PAGE)
        # The words are measured, so the first fit waits for the record to
        # show: the part's script gives `fit` back and the page calls it
        # once nothing is hidden, and again when the window changes.
        for again in ("return fit;", 'wide.addEventListener("change", fit);', 'window.addEventListener("resize", fit);'):
            self.assertIn(again, self.SCRIPT)
        shown_at = PAGE.index('document.getElementById("perf-record").hidden = false;')
        self.assertIn("if (fitLine) fitLine();", PAGE[shown_at:])
        self.assertNotIn("is-tall-", PAGE)
        # The narrow layout is a list, slowest first, whatever kind an entry
        # is: a stop nothing has played can be slower than a measured one.
        self.assertIn("for (const entry of [...list.children].sort((one, other) => pace(one) - pace(other))) list.append(entry);", PAGE)
        # No figure is written into the page: every one is the slot's. But
        # for the operator's note on the first stop, his words as he wrote
        # them, which name his own five hours: the note's own test holds
        # that figure to the reading.
        text = re.sub(r"<!--.*?-->", "", section, flags=re.S)
        text, notes = re.subn(r'<small class="speed-aside">.*?</small>', "", text, flags=re.S)
        self.assertEqual(notes, 1)
        self.assertIsNone(re.search(r"\d[\d,.]*\s*(x|s|min|h)\b", re.sub(r"<[^>]+>", " ", text)))
        # 1x is at the right end of the line and the largest goal at the left.
        self.assertIn(".speed-stop { position: absolute; right: calc(var(--at) * 100%);", PAGE)
        self.assertIn("width: calc(var(--reach, 0) * 100%);", PAGE)
        self.assertIn('entry.style.setProperty("--at", stop.at);', PAGE)
        for unsafe in ("innerHTML", "insertAdjacentHTML", "outerHTML"):
            self.assertNotIn(unsafe, self.SCRIPT)

    def test_the_gap_between_two_stops_is_marked_on_the_line_and_explained_under_it(self):
        # Operator 2026-11-24: "what is the main reason civvis is so much
        # faster than firaxis civ 6? please try to explain that gap in the
        # chart/timeline". On the line: the stretch between the two stops,
        # drawn over the scale and under the stops' points, with how many
        # times faster on it, and it is a link to the part under the line.
        section, script = self.SECTION, self.SCRIPT
        self.assertIn('            <div class="speed-axis" aria-hidden="true"></div>\n'
                      '            <a class="speed-between" id="speed-between" href="#speed-gap" hidden></a>\n'
                      '            <ol class="speed-line">', section)
        self.assertIn(".speed-between { display: none; }", PAGE)
        self.assertIn("display: block; position: absolute; right: calc(var(--from) * 100%); width: calc((var(--to) - var(--from)) * 100%);", PAGE)
        self.assertIn('mark.style.setProperty("--from", between.from);', script)
        self.assertIn('mark.style.setProperty("--to", between.to);', script)
        self.assertIn('mark.append(el("b", "", text(between.times)));', script)
        # Under the line, after the line's own notes: the two games as bars
        # to one scale, what each colour is, and pairs of figures. It is
        # hidden until the record has the bars, and with it the mark.
        self.assertLess(section.index('id="speed-note"'), section.index('<div class="speed-gap" id="speed-gap" hidden>'))
        for place in ("speed-gap-times", "speed-gap-title", "speed-gap-bars", "speed-gap-keys", "speed-gap-facts", "speed-gap-counted"):
            self.assertEqual(section.count(f'id="{place}"'), 1, place)
        self.assertIn("&& Array.isArray(line.gap.bars) && line.gap.bars.length ? line.gap : null;", script)
        self.assertIn('document.getElementById("speed-gap").hidden = false;', script)
        # The two stops are the record's, named as the tables name them.
        self.assertIn('"Why " + way(between.fast) + " is that much faster than " + way(between.slow)', script)
        # A bar is as long as its share of the slower game, less the room
        # its time is written in, and a part as wide as its own share, with
        # the panel showing between two parts.
        self.assertIn("flex: none; width: calc((100% - 66px) * var(--share) - 2px);", PAGE)
        self.assertIn(".speed-gap-track { display: flex; align-items: center; gap: 2px;", PAGE)
        self.assertIn('piece.style.setProperty("--share", part.share);', script)
        # The parts the page can name, each in one of the page's three chart
        # colours, the work in the first: the faster game's bar is that one.
        for key, colour in (("works", "var(--series)"), ("waits", "var(--series-2)"), ("outside", "var(--series-3)"), ("play", "var(--series)")):
            self.assertRegex(script, rf'{key}: \["[^"]+", "{re.escape(colour)}", "[^"]*"\],')
        self.assertIn(".speed-gap-part.is-waits { background: var(--series-2); }", PAGE)
        self.assertIn(".speed-gap-part.is-outside { background: var(--series-3); }", PAGE)
        # Every part and every pair the record can give has a name here.
        between = speedup_timeline.timeline(ROOT)["gap"]
        for bar in between["bars"]:
            for part in bar["parts"]:
                self.assertRegex(script, rf'\b{part["key"]}: \["')
        for fact in between["facts"]:
            self.assertRegex(script, rf'\b{fact["key"]}: "')
        # A part's time is written on it only where it fits, measured again
        # whenever the page is fitted, at any width; a part can be reached
        # by the keyboard and says what it is under a pointer or on focus.
        self.assertIn("const fit = () => {\n        fitGap();\n        if (!wide.matches) return;", script)
        self.assertIn("if (piece.scrollWidth > piece.clientWidth) {", script)
        for heard in ('"pointermove"', '"pointerleave"', '"focus"', '"blur"'):
            self.assertIn(f"piece.addEventListener({heard},", script)
        self.assertIn("piece.tabIndex = 0;", script)
        # What the figures are counted from is closed until it is pressed.
        self.assertIn('<details class="fine">\n              <summary>How it is counted</summary>\n'
                      '              <p class="rank-note" id="speed-gap-counted"></p>', section)
        # The one sentence over the bars has no figure in it and is short.
        why = re.search(r'<p class="speed-gap-why">(.*?)</p>', section, re.S)[1]
        self.assertLessEqual(len(re.sub(r"&#?\w+;", "x", why).split()), 35)
        self.assertIsNone(re.search(r"\d", re.sub(r"&#\d+;", "", why)))

    def test_it_tops_civvis_performance_and_home_has_none_of_it(self):
        # Operator 2026-11-24: "move the civvis performance timeline thing
        # from home/ to the civvis performance section. middle of the page
        # or so"; then 2026-11-27: "move civ game speedup - synthetic data
        # pipeline up to the top of civvis.ai performance". It is the
        # page's first part, over the computer control chart and the tables
        # that work its figures out.
        parts = re.findall(r'<section class="res-section" id="([\w-]+)"', PAGE)
        self.assertEqual(parts[:3], ["perf-line", "perf-control", "perf-speedup"])
        record = PAGE.index('<div class="res-record" id="perf-record" hidden>')
        self.assertLess(record, PAGE.index('<section class="res-section" id="perf-line"'))
        # The tables' link to the line is to this page's own part.
        self.assertIn('<a href="#perf-line">The same figures as a line, at the top</a>', PAGE)
        self.assertNotIn("home/#speed", PAGE)
        # Home has none of it left: no entry, no rule, no script, no slot.
        for gone in ('class="speed', "speed-stop", "speed-track", "data-speed=", "home-speedup", "Civ Game Speedup"):
            self.assertNotIn(gone, LANDING, gone)
        self.assertFalse(hasattr(speedup_timeline, "SLOT"))
        self.assertFalse(hasattr(home_rankings, "speedup_timeline"))

    def test_publishing_writes_the_timeline_into_the_page_s_record(self):
        shown = speedup_timeline.timeline(ROOT)
        self.assertTrue(shown["stops"])
        with tempfile.TemporaryDirectory() as scratch:
            page = pathlib.Path(scratch) / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(performance_page.main(["performance_page.py", str(ROOT), str(page)]), 0)
            filled = page.read_text(encoding="utf-8")
            self.assertEqual(json.loads(performance_page.SLOT.search(filled)[2])["timeline"], shown)
            # Nothing but the page's slot moved.
            bare = lambda text: performance_page.SLOT.sub("", text)  # noqa: E731
            self.assertEqual(bare(filled), bare(PAGE))
            # Home's script no longer writes it: a page with a slot of the
            # old name is left as it is.
            old = pathlib.Path(scratch) / "old.html"
            old.write_text('<script type="application/json" id="home-speedup">null</script>', encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(home_rankings.main(["home_rankings.py", str(ROOT), str(old)]), 0)
            self.assertEqual(old.read_text(encoding="utf-8"), '<script type="application/json" id="home-speedup">null</script>')
        # A revision with no ledger has no line: a record it has for
        # another reason says so, and the part stays hidden.
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()):
            self.assertIsNone(speedup_timeline.timeline(pathlib.Path(scratch)))
            self.assertIsNone(performance_page.performance(pathlib.Path(scratch)))
            (pathlib.Path(scratch) / performance_page.VIDEOS).parent.mkdir(parents=True)
            (pathlib.Path(scratch) / performance_page.VIDEOS).write_text((ROOT / performance_page.VIDEOS).read_text(encoding="utf-8"), encoding="utf-8")
            self.assertIsNone(performance_page.performance(pathlib.Path(scratch))["timeline"])


if __name__ == "__main__":
    unittest.main()
