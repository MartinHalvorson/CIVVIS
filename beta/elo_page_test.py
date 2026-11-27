#!/usr/bin/env python3
"""ELO Rankings: the agents over every published fit, and Civilization VI's
own AI at each level rated from the benchmark's games against it.

    python3 beta/elo_page_test.py
"""

from __future__ import annotations

import contextlib
import datetime
import io
import json
import math
import pathlib
import re
import sys
import tempfile
import unittest

import elo_page
import home_rankings as ranks

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "elo.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")

sys.path.insert(0, str(ROOT / "tools"))
import strategy_elo_history  # noqa: E402


def at(text: str) -> datetime.datetime:
    return datetime.datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ")


def point(when: str, **ratings) -> dict:
    return {"at": when, "ratings": {key: list(value) for key, value in ratings.items()}, "ladder": {}}


class Fit(unittest.TestCase):
    def test_a_seat_that_wins_one_game_in_six_is_level_with_each_rival(self):
        # Six seats of one strength: each wins one table in six, and the
        # level is rated exactly where our seat is.
        games = [(1000.0, 0.0, i % 6 == 0) for i in range(600)]
        fitted = elo_page.fit_level(games)
        self.assertAlmostEqual(fitted["elo"], 1000.0, delta=0.05)
        self.assertEqual((fitted["games"], fitted["wins"], fitted["bound"]), (600, 100, ""))

    def test_the_rating_is_the_tables_reading_of_the_win_rate(self):
        # Many games: the imaginary pair no longer moves it, and the level
        # sits where a table of one seat and five rivals puts it,
        # rival - seat = 400 log10((1 - p) / (5 p)).
        for p in (0.05, 0.145, 0.4, 0.7):
            n = 20000
            games = [(1100.0, 0.0, i < round(p * n)) for i in range(n)]
            expected = 1100 + 400 * math.log10((1 - p) / (5 * p))
            self.assertAlmostEqual(elo_page.fit_level(games)["elo"], expected, delta=1.0, msg=p)

    def test_more_wins_rate_the_level_lower_and_the_margin_narrows_with_games(self):
        few = elo_page.fit_level([(1000.0, 0.0, i < 3) for i in range(20)])
        more = elo_page.fit_level([(1000.0, 0.0, i < 8) for i in range(20)])
        self.assertGreater(few["elo"], more["elo"])
        wide = elo_page.fit_level([(1000.0, 0.0, i < 6) for i in range(36)])
        narrow = elo_page.fit_level([(1000.0, 0.0, i < 60) for i in range(360)])
        self.assertGreater(wide["margin"], narrow["margin"])

    def test_the_twins_margin_is_added_to_the_fits_own(self):
        alone = elo_page.fit_level([(1000.0, 0.0, i < 30) for i in range(180)])
        twinned = elo_page.fit_level([(1000.0, 40.0, i < 30) for i in range(180)])
        self.assertEqual(twinned["margin"], round(math.hypot(alone["margin"], 40.0)))

    def test_a_level_never_lost_to_or_never_beaten_is_a_bound_and_still_finite(self):
        swept = elo_page.fit_level([(1000.0, 0.0, True)] * 17)
        self.assertEqual(swept["bound"], "upper")
        shut_out = elo_page.fit_level([(1000.0, 0.0, False)] * 17)
        self.assertEqual(shut_out["bound"], "lower")
        for fitted in (swept, shut_out):
            self.assertTrue(math.isfinite(fitted["elo"]))
        self.assertLess(swept["elo"], 1000)
        self.assertGreater(shut_out["elo"], 1000)
        self.assertIsNone(elo_page.fit_level([]))


class Twins(unittest.TestCase):
    def test_a_lane_is_rated_as_the_agent_that_plays_it(self):
        self.assertEqual(elo_page.twin("science"), "moonshot")
        self.assertEqual(elo_page.twin("religious"), "holy")
        self.assertEqual(elo_page.twin("culture"), "tourist")
        self.assertEqual(elo_page.twin("diplomatic"), "swing")
        self.assertEqual(elo_page.twin("domination"), "siege")
        for adaptive in (None, "", "civvis", "score"):
            self.assertEqual(elo_page.twin(adaptive), "jack")
        # Every twin is an agent of the roster.
        roster = re.findall(r'id: "([a-z]+)",\s*handle:', (ROOT / "src" / "strategies.rs").read_text(encoding="utf-8"))
        for agent in {*elo_page.TWINS.values(), elo_page.ADAPTIVE}:
            self.assertIn(agent, roster)

    def test_a_game_is_rated_against_the_fit_published_when_it_began(self):
        points = [point("2026-11-21T00:00:00Z", moonshot=(1100, 50, "")),
                  point("2026-11-22T12:00:00Z", moonshot=(1050, 30, ""))]
        self.assertEqual(elo_page.in_force(points, at("2026-11-20T00:00:00Z"))["at"], "2026-11-21T00:00:00Z")
        self.assertEqual(elo_page.in_force(points, at("2026-11-22T11:59:59Z"))["at"], "2026-11-21T00:00:00Z")
        self.assertEqual(elo_page.in_force(points, at("2026-11-22T12:00:00Z"))["at"], "2026-11-22T12:00:00Z")
        games = [{"at": at("2026-11-22T06:00:00Z"), "level": 8, "won": True, "twin": "moonshot"},
                 {"at": at("2026-11-23T06:00:00Z"), "level": 8, "won": False, "twin": "moonshot"},
                 # A twin the fit has no rating for is rated as JackOfAllTrades.
                 {"at": at("2026-11-23T07:00:00Z"), "level": 8, "won": False, "twin": "nobody"}]
        points[1]["ratings"]["jack"] = [1140, 28, ""]
        self.assertEqual(elo_page.rated(games, points), {8: [(1100.0, 50.0, True), (1050.0, 30.0, False), (1140.0, 28.0, False)]})


class Benchmark(unittest.TestCase):
    def source(self, rows: list[dict]) -> pathlib.Path:
        root = pathlib.Path(tempfile.mkdtemp())
        (root / "data").mkdir()
        (root / "data" / "tournaments.json").write_text(json.dumps({"tournaments": [
            {"name": "bench", "indefinite": True, "start": "civvis-t7-", "match": {"players": 6}},
        ]}), encoding="utf-8")
        return root

    @staticmethod
    def row(stamp: str, difficulty: str, outcome: str, target: str | None = "science", **more) -> dict:
        tag = f"civvis-t7-{stamp}"
        return {"tag": tag, "game": tag, "utc": "2026-11-23T00:00:00Z", "difficulty": difficulty,
                "outcome": outcome, "players": 6, "target": target, "civ": "CIVILIZATION_GREECE", **more}

    def test_the_benchmark_games_are_the_tournaments_finished_games_at_lv_4_to_8(self):
        rows = [
            self.row("20261122T010000Z", "DIFFICULTY_DEITY", "win"),
            self.row("20261122T020000Z", "DIFFICULTY_DEITY", "rival_victory", target="religious"),
            # A launch that never reached a board is no game.
            self.row("20261122T030000Z", "DIFFICULTY_DEITY", "unfinished"),
            # Below Prince the benchmark does not deal.
            self.row("20261122T040000Z", "DIFFICULTY_WARLORD", "win"),
            # Two of our agents at one table: either winning was a win.
            self.row("20261122T050000Z", "DIFFICULTY_KING", "win", civ="CIVILIZATION_GREECE+CIVILIZATION_ROME"),
            self.row("20261122T060000Z", "DIFFICULTY_PRINCE", "defeat", target=None),
        ]
        games = elo_page.benchmark_games(self.source(rows), rows)
        self.assertEqual(
            [(g["at"].hour, g["level"], g["won"], g["twin"]) for g in games],
            [(1, 8, True, "moonshot"), (2, 8, False, "holy"), (6, 4, False, "jack")],
        )

    def test_each_game_carries_its_diplomatic_switch_from_its_run(self):
        rows = [
            self.row("20261122T010000Z", "DIFFICULTY_DEITY", "win"),
            self.row("20261122T020000Z", "DIFFICULTY_DEITY", "defeat"),
            # A run whose summary does not say is in neither reading.
            self.row("20261122T030000Z", "DIFFICULTY_KING", "win"),
        ]
        asked = []

        def switch(tags):
            asked.extend(tags)
            return {"civvis-t7-20261122T010000Z": True, "civvis-t7-20261122T020000Z": False}

        games = elo_page.benchmark_games(self.source(rows), rows, switch)
        self.assertEqual([g["diplo"] for g in games], [True, False, None])
        self.assertIn("civvis-t7-20261122T030000Z", asked)

    def test_each_level_is_rated_now_and_a_point_a_day_over_its_window(self):
        points = [point("2026-11-01T00:00:00Z", moonshot=(1000, 0, ""), jack=(1000, 0, ""))]
        games = []
        start = at("2026-11-10T00:00:00Z")
        for day in range(10):
            for k in range(6):
                games.append({"at": start + datetime.timedelta(days=day, hours=k), "level": 8, "won": k == 0, "twin": "moonshot"})
        # Two Prince games only: never enough for a point on the line.
        games += [{"at": start + datetime.timedelta(days=9, hours=7 + k), "level": 4, "won": True, "twin": "jack"} for k in range(2)]
        games.sort(key=lambda g: g["at"])
        record = elo_page.firaxis(games, points)
        self.assertEqual(record["days"], elo_page.WINDOW_DAYS)
        self.assertEqual(record["to"], "2026-11-19T08:00:00Z")
        deity, prince = record["levels"]
        self.assertEqual((deity["level"], deity["name"], prince["level"]), (8, "Deity", 4))
        # A seat that wins one table in six against five rivals: level with them.
        self.assertAlmostEqual(deity["now"]["elo"], 1000.0, delta=0.05)
        self.assertEqual(deity["now"]["games"], 7 * 6)
        # A point at the end of every day the benchmark played, the last at
        # the newest game, each over the days before it.
        self.assertEqual([p[0] for p in deity["series"]][:2], ["2026-11-11T00:00:00Z", "2026-11-12T00:00:00Z"])
        self.assertEqual(deity["series"][-1][0], "2026-11-19T08:00:00Z")
        self.assertEqual([p[4] for p in deity["series"]], [6, 12, 18, 24, 30, 36, 42, 42, 42, 42])
        self.assertEqual(prince["series"], [])
        self.assertEqual((prince["now"]["games"], prince["now"]["bound"]), (2, "upper"))
        self.assertIsNone(elo_page.firaxis([], points))


class Agents(unittest.TestCase):
    def test_the_top_ten_by_the_newest_fit_each_over_every_fit(self):
        points = strategy_elo_history.read(ROOT)["points"]
        agents = elo_page.agents(ROOT, elo_page.history(ROOT))
        self.assertLessEqual(len(agents), elo_page.TOP_AGENTS)
        newest = points[-1]["ratings"]
        elos = [newest[a["id"]][0] for a in agents]
        self.assertEqual(elos, sorted(elos, reverse=True))
        self.assertFalse(any(a["id"].startswith("civ6:") for a in agents))
        for agent in agents:
            self.assertNotEqual(agent["name"], agent["id"])
            self.assertEqual([p[0] for p in agent["series"]], [p["at"] for p in points if agent["id"] in p["ratings"]])
        # The simulator's dial at the levels rated: the stand-in's ladder.
        dial = elo_page.simulator(elo_page.history(ROOT))
        published = json.loads((ROOT / "data" / "strategy_elo.json").read_text(encoding="utf-8"))
        self.assertEqual([d["level"] for d in dial], [8, 7, 6, 5, 4])
        self.assertEqual(dial[0]["elo"], round(published["ladder"]["deity"], 1))

    def test_a_published_fit_the_history_does_not_end_at_yet_is_its_newest_point(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = pathlib.Path(scratch)
            (root / "data").mkdir()
            (root / "data" / "strategy_elo_history.json").write_text(json.dumps(
                {"points": [point("2026-11-20T00:00:00Z", jack=(1100, 30, ""))]}), encoding="utf-8")
            (root / "data" / "strategy_elo.json").write_text(json.dumps(
                {"tables": 10, "ratings": {"jack": {"elo": 1150.0, "sd": 10.0, "margin95": 20.0}}}), encoding="utf-8")
            points = elo_page.history(root)
        self.assertEqual([p["ratings"]["jack"][0] for p in points], [1100, 1150.0])


class Readings(unittest.TestCase):
    """Operator 2026-11-24: "add a second elo for games with diplo off. we
    should maintain this because diplo on or off makes a big difference"."""

    @staticmethod
    def tree(on: float, off: float | None) -> pathlib.Path:
        root = pathlib.Path(tempfile.mkdtemp())
        (root / "data").mkdir()
        fits = [(elo_page.PUBLISHED, elo_page.HISTORY, on)]
        if off is not None:
            fits.append((elo_page.NO_DIPLO_PUBLISHED, elo_page.NO_DIPLO_HISTORY, off))
        for published, kept, jack in fits:
            (root / published).write_text(json.dumps({"tables": 100, "players": 4, "ratings": {
                "jack": {"elo": jack, "sd": 10.0, "margin95": 20.0, "games": 100, "wins": 30}}}), encoding="utf-8")
            (root / kept).write_text(json.dumps({"points": [point("2026-11-20T00:00:00Z", jack=(jack, 20, ""))]}),
                                     encoding="utf-8")
        return root

    @staticmethod
    def games(diplo) -> list[dict]:
        # Six Deity games a day for three days, our seat winning one in six:
        # level with the twin.
        start = at("2026-11-21T00:00:00Z")
        return [{"at": start + datetime.timedelta(days=d, hours=k), "level": 8, "won": k == 0, "twin": "jack",
                 "diplo": diplo} for d in range(3) for k in range(6)]

    def record(self, root: pathlib.Path, games: list[dict]) -> dict:
        real = elo_page.benchmark_games
        elo_page.benchmark_games = lambda source: games
        try:
            return elo_page.record(root)
        finally:
            elo_page.benchmark_games = real

    def test_each_reading_rates_its_own_games_against_its_own_fit(self):
        # The same games, played both ways: each reading counts only its
        # own, and rates our seat at the twin's rating in that reading.
        games = sorted(self.games(True) + self.games(False) + self.games(None)[:2], key=lambda g: g["at"])
        record = self.record(self.tree(1000.0, 1300.0), games)
        on, off = record["firaxis"]["levels"][0]["now"], record["off"]["firaxis"]["levels"][0]["now"]
        self.assertEqual((on["games"], off["games"]), (18, 18))
        self.assertAlmostEqual(on["elo"], 1000.0, delta=0.5)
        self.assertAlmostEqual(off["elo"], 1300.0, delta=0.5)
        self.assertEqual(record["unswitched"], 2)
        self.assertEqual([r["elo"] for r in record["strategies"]["rows"]], [1000.0])
        self.assertEqual([r["elo"] for r in record["off"]["strategies"]["rows"]], [1300.0])
        self.assertEqual(record["off"]["agents"][0]["series"], [["2026-11-20T00:00:00Z", 1300.0, 20, ""]])
        # The diplo-off reading carries every field the stock one does.
        self.assertEqual(set(record["off"]), set(record) - {"off", "unswitched", "setups"})

    def test_each_published_setup_is_a_reading_of_its_own(self):
        # Operator 2026-11-27: "6p Continents Online (Diplo off, Diplo on,
        # science, culture, religion, diplomacy, domination, score) , 2P
        # Pangaea (religion off, religion on). we can elo for each". Each
        # published setup fit is a reading of its own fit; the 6P Continents
        # Diplo on and off ones carry the benchmark's Firaxis levels played
        # their way, rated against their own twin, and the others none.
        root = self.tree(1000.0, 1300.0)
        for name, jack in (("continents6", 1100.0), ("continents6_no_diplo", 1400.0), ("continents6_science", 900.0)):
            (root / f"data/strategy_elo_{name}.json").write_text(json.dumps({"tables": 60, "players": 6, "setup": name, "ratings": {
                "jack": {"elo": jack, "sd": 10.0, "margin95": 20.0, "games": 60, "wins": 10}}}), encoding="utf-8")
            (root / f"data/strategy_elo_{name}_history.json").write_text(
                json.dumps({"points": [point("2026-11-20T00:00:00Z", jack=(jack, 20, ""))]}), encoding="utf-8")
        record = self.record(root, sorted(self.games(True) + self.games(False), key=lambda g: g["at"]))
        self.assertEqual(sorted(record["setups"]), ["continents6", "continents6_no_diplo", "continents6_science"])
        on, off, science = (record["setups"][name] for name in ("continents6", "continents6_no_diplo", "continents6_science"))
        self.assertEqual([r["elo"] for r in on["strategies"]["rows"]], [1100.0])
        self.assertAlmostEqual(on["firaxis"]["levels"][0]["now"]["elo"], 1100.0, delta=0.5)
        self.assertAlmostEqual(off["firaxis"]["levels"][0]["now"]["elo"], 1400.0, delta=0.5)
        self.assertEqual([r["elo"] for r in science["strategies"]["rows"]], [900.0])
        self.assertIsNone(science["firaxis"])
        self.assertEqual(science["people"], [])
        # A revision with none of them published has an empty set.
        self.assertEqual(self.record(self.tree(1000.0, None), [])["setups"], {})

    def test_a_revision_from_before_the_diplo_off_fit_has_no_second_reading(self):
        record = self.record(self.tree(1000.0, None), self.games(False))
        self.assertIsNone(record["off"])
        # Its diplo-off games are not read as diplo-on ones.
        self.assertIsNone(record["firaxis"])

    def test_the_repository_publishes_both_readings(self):
        # The diplo-off fit is kept up as the stock one is: it must not go
        # missing from a revision once it has been published.
        self.assertTrue((ROOT / elo_page.NO_DIPLO_PUBLISHED).is_file())
        off = json.loads((ROOT / elo_page.NO_DIPLO_PUBLISHED).read_text(encoding="utf-8"))
        self.assertIs(off["diplomatic"], False)
        points = elo_page.history(ROOT, elo_page.NO_DIPLO_PUBLISHED, elo_page.NO_DIPLO_HISTORY)
        self.assertTrue(points)
        self.assertTrue(elo_page.agents(ROOT, points))
        self.assertEqual(elo_page.simulator(points)[0]["level"], 8)


class People(unittest.TestCase):
    """Operator 2026-11-27: "start posting elo on the website. use the name
    dev for me". A person who plays the agents in Civilization VI is rated
    on their scale, from the people's record on the ledger branch."""

    @staticmethod
    def line(tag: str, when: str, winner: str, agents=("holy",), person="dev", **more) -> str:
        return json.dumps({"tag": tag, "at": when, "person": person, "agents": list(agents),
                           "winner": winner, "diplomatic": True, **more})

    def test_one_game_won_rates_a_person_a_little_above_the_agent_and_lost_a_little_below(self):
        # The imaginary pair makes it two wins in three against the agent
        # (or one): 400 log10(2) either side, a bound until they have both
        # won and lost.
        won = elo_page.fit_person([([1446.0], 21.0, True)])
        lost = elo_page.fit_person([([1446.0], 21.0, False)])
        self.assertAlmostEqual(won["elo"], 1446 + 400 * math.log10(2), delta=0.1)
        self.assertAlmostEqual(lost["elo"], 1446 - 400 * math.log10(2), delta=0.1)
        self.assertEqual((won["bound"], won["games"], won["wins"]), ("lower", 1, 1))
        self.assertEqual((lost["bound"], lost["games"], lost["wins"]), ("upper", 1, 0))
        # One game pins little down, and the agent's own margin is in it.
        self.assertGreater(won["margin"], 400)
        self.assertGreater(won["margin"], elo_page.fit_person([([1446.0], 0.0, True)])["margin"])
        self.assertIsNone(elo_page.fit_person([]))

    def test_the_rating_is_the_tables_reading_of_the_win_rate(self):
        # Many duels: the pair no longer moves it, and the person sits
        # 400 log10(p / (1 - p)) from the agent.
        for p in (0.1, 0.5, 0.8):
            n = 20000
            fitted = elo_page.fit_person([([1200.0], 0.0, i < round(p * n)) for i in range(n)])
            self.assertAlmostEqual(fitted["elo"], 1200 + 400 * math.log10(p / (1 - p)), delta=1.0, msg=p)
            self.assertEqual(fitted["bound"], "")
        # A table of four: winning one in four is level with three equal agents.
        level = elo_page.fit_person([([1200.0] * 3, 0.0, i % 4 == 0) for i in range(4000)])
        self.assertAlmostEqual(level["elo"], 1200.0, delta=1.0)
        # More games narrow the margin.
        few = elo_page.fit_person([([1200.0], 0.0, i % 2 == 0) for i in range(4)])
        many = elo_page.fit_person([([1200.0], 0.0, i % 2 == 0) for i in range(40)])
        self.assertLess(many["margin"], few["margin"])

    def test_the_record_is_read_a_game_a_line_and_a_line_that_is_not_a_game_is_left_out(self):
        text = "\n".join([
            self.line("b", "2026-11-28T01:00:00Z", "holy"),
            self.line("a", "2026-11-27T06:30:00Z", "person", diplomatic=False),
            "not json",
            json.dumps({"tag": "no-winner", "at": "2026-11-27T06:30:00Z", "person": "dev", "agents": ["holy"]}),
            json.dumps({"tag": "nobody", "at": "2026-11-27T06:30:00Z", "agents": ["holy"], "winner": "holy"}),
            self.line("no-agents", "2026-11-27T06:30:00Z", "person", agents=()),
            self.line("unswitched", "2026-11-29T00:00:00Z", "person", diplomatic=None),
        ])
        games = elo_page.people_games(ROOT, text)
        self.assertEqual([(g["tag"], g["person"], g["agents"], g["won"], g["diplo"]) for g in games], [
            ("a", "dev", ["holy"], True, False),
            ("b", "dev", ["holy"], False, True),
            ("unswitched", "dev", ["holy"], True, None),
        ])
        self.assertEqual(games[0]["at"], at("2026-11-27T06:30:00Z"))
        # A clone with no ledger has no record, which is not an empty one.
        self.assertIsNone(elo_page.people_games(pathlib.Path(tempfile.mkdtemp())))

    def test_each_game_is_rated_against_the_agents_fit_when_it_began(self):
        points = [point("2026-11-20T00:00:00Z", holy=(1400.0, 20, ""), jack=(1100.0, 20, "")),
                  point("2026-11-28T00:00:00Z", holy=(1500.0, 20, ""), jack=(1100.0, 20, ""))]
        text = "\n".join([
            self.line("a", "2026-11-27T06:30:00Z", "person"),
            self.line("b", "2026-11-28T06:30:00Z", "holy"),
            self.line("c", "2026-11-28T07:30:00Z", "person", agents=("nobody-rated",)),
            self.line("d", "2026-11-28T08:30:00Z", "person", person="ada", agents=("jack",)),
        ])
        rated = elo_page.people(elo_page.people_games(ROOT, text), points)
        self.assertEqual([p["name"] for p in rated], ["dev", "ada"], "strongest first")
        dev = rated[0]
        self.assertEqual(dev, {**elo_page.fit_person([([1400.0], 20.0, True), ([1500.0], 20.0, False)]),
                               "name": "dev", "met": ["holy"], "last": "2026-11-28T06:30:00Z"})
        self.assertEqual((dev["games"], dev["wins"], dev["bound"]), (2, 1, ""))
        self.assertEqual((rated[1]["games"], rated[1]["met"]), (1, ["jack"]))
        self.assertEqual(elo_page.people([], points), [])

    def test_a_game_stands_in_the_reading_its_diplomatic_switch_puts_it_in(self):
        text = "\n".join([
            self.line("a", "2026-11-27T06:30:00Z", "person", agents=("jack",)),
            self.line("b", "2026-11-27T07:30:00Z", "jack", agents=("jack",), diplomatic=False),
            self.line("c", "2026-11-27T08:30:00Z", "person", agents=("jack",), diplomatic=None),
        ])
        root = Readings.tree(1000.0, 1300.0)
        real = elo_page.people_games, elo_page.benchmark_games
        elo_page.people_games = lambda source: real[0](source, text)
        elo_page.benchmark_games = lambda source: []
        try:
            record = elo_page.record(root)
        finally:
            elo_page.people_games, elo_page.benchmark_games = real
        on, off = record["people"], record["off"]["people"]
        self.assertEqual([(p["name"], p["games"], p["wins"], p["bound"]) for p in on], [("dev", 1, 1, "lower")])
        self.assertEqual([(p["name"], p["games"], p["wins"], p["bound"]) for p in off], [("dev", 1, 0, "upper")])
        self.assertAlmostEqual(on[0]["elo"], 1000 + 400 * math.log10(2), delta=0.1)
        self.assertAlmostEqual(off[0]["elo"], 1300 - 400 * math.log10(2), delta=0.1)

    def test_a_person_stands_in_the_player_strategies_among_the_agents(self):
        strategies = PAGE[PAGE.index("// ---- Player strategies"):PAGE.index("// ---- Elo over time")]
        self.assertIn('entry("person:" + p.name, { kind: "person", name: p.name, tilt: "Person" })[side] = reading(p);',
                      strategies)
        # Numbered with the agents (a Firaxis level is a rung, not a place), in gold.
        self.assertIn('item.append(el("span", "rank-n", r.kind === "level" ? "" : String(++place)));', strategies)
        self.assertIn('r.kind === "person" ? " is-person"', strategies)
        self.assertIn(".rank-row.is-person .rank-name { color: var(--gold); }", PAGE)
        self.assertIn("--gold:", PAGE)
        # What their figure counts is said on the row and under the list.
        self.assertIn('" against the Civ AI Agents in Civilization VI"', strategies)
        self.assertIn('" against the agents in Civilization VI itself, each agent at its Elo here when the game began."',
                      strategies)

    def test_the_record_is_the_one_the_recorder_writes(self):
        sys.path.insert(0, str(ROOT / "tools"))
        import people_elo

        self.assertEqual(elo_page.PEOPLE_REF, f"refs/remotes/origin/{people_elo.LEDGER_BRANCH}:{people_elo.PEOPLE_PATH}")
        self.assertEqual(elo_page.PERSON, people_elo.PERSON)
        self.assertEqual(elo_page.PEOPLE_REF.split(":")[0], ranks.RESULTS_REF.split(":")[0],
                         "read off the ledger the site build already fetches")


class Page(unittest.TestCase):
    def test_the_page_carries_one_empty_slot_and_publishing_fills_it(self):
        self.assertEqual(PAGE.count('<script type="application/json" id="elo-data">null</script>'), 1)
        with contextlib.redirect_stderr(io.StringIO()):
            data = elo_page.record(ROOT)
        filled = elo_page.fill(PAGE, data)
        self.assertEqual(json.loads(elo_page.SLOT.search(filled)[2]), data)
        self.assertEqual(elo_page.SLOT.sub("", filled), elo_page.SLOT.sub("", PAGE))
        # A sentence cannot close the slot.
        sneaky = elo_page.fill(PAGE, {"agents": [{"name": "</script><b>"}]})
        self.assertEqual(sneaky.count("</script>"), PAGE.count("</script>"))
        for line in (
            'elo_page="$source_tree/beta/elo.html"',
            'elo_at="$(address "$elo_page" elo)"',
            'cp "$elo_page" "$out/$elo_at/index.html"',
            'fill elo_page.py "$out/$elo_at/index.html"',
            'pages+=("$elo_at")',
        ):
            self.assertIn(line, PUBLISH)
        self.assertIn('<link rel="canonical" href="https://civvis.ai/charts/elo-rankings/">', PAGE)
        self.assertIn("""    "charts/elo-rankings": b'id="elo-data"',\n""", (BETA / "verify.py").read_text(encoding="utf-8"))

    def test_a_revision_with_no_ratings_publishes_the_page_without_figures(self):
        with tempfile.TemporaryDirectory() as empty:
            self.assertIsNone(elo_page.record(pathlib.Path(empty)))
            page = pathlib.Path(empty) / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(elo_page.main(["elo_page.py", empty, str(page)]), 0)
            self.assertIsNone(json.loads(elo_page.SLOT.search(page.read_text(encoding="utf-8"))[2]))

    def test_the_player_strategies_stand_at_the_top_then_the_graph(self):
        # Operator 2026-11-23: "at the top of this page is the player
        # straetgies, pulled from win analytics".
        record = PAGE[PAGE.index('<div class="elo-record" id="elo-record" hidden>'):]
        sections = re.findall(r'<section [^>]*id="([a-z-]+)"', record)
        # The two readings of each Firaxis level stood beside the strategies
        # until 2026-11-26, and are on Unreviewed Gen AI Charts since (operator:
        # "please move all the ai generated charts that i havent approved of
        # or we have discussed to here"); `random_page_test.py` holds them.
        self.assertEqual(sections, ["rank-strategies", "elo-over-time"])
        self.assertNotIn('id="elo-levels"', PAGE)
        self.assertIn('<h2 class="rank-title" id="rank-strategies-title">Player strategies</h2>', PAGE)
        self.assertIn('<h2 class="rank-title" id="elo-over-time-title">Elo over time</h2>', PAGE)

    def test_every_figure_is_shown_with_the_diplomatic_victory_on_and_off(self):
        # Player strategies: a column for each, its head sorting the list.
        # It opens sorted by Diplo off (operator 2026-11-26: "player
        # strategies should default diplo off"), by Diplo on only while the
        # record has no diplo-off fit.
        strategies = PAGE[PAGE.index('id="rank-strategies"'):PAGE.index('id="elo-over-time"')]
        self.assertEqual(re.findall(r'data-sort="(\w+)" aria-pressed="(\w+)"[^>]*>([^<]+)<', strategies),
                         [("on", "false", "Diplo on"), ("off", "true", "Diplo off")])
        self.assertIn('show(VIEWS.off ? "off" : "on");', PAGE)
        self.assertNotIn('show("on");', PAGE)
        self.assertIn("item.append(label, value(r, \"on\"), value(r, \"off\"));", PAGE)
        # The graph: one reading at a time, switched over the key.
        graph = PAGE[PAGE.index('id="elo-over-time"'):PAGE.index('id="elo-legend"')]
        self.assertEqual(re.findall(r'data-diplo="(\w+)" aria-pressed="(\w+)">([^<]+)<', graph),
                         [("on", "true", "Diplo on"), ("off", "false", "Diplo off")])
        self.assertIn("const VIEWS = { on: data, off: data.off || null };", PAGE)

    def test_every_agents_colour_follows_the_agent(self):
        # Colour follows the entity, never its place: every agent of the
        # roster has its own, and the palette's eight are each used once
        # solid before any is repeated dotted.
        roster = re.findall(r'id: "([a-z]+)",\s*handle:', (ROOT / "src" / "strategies.rs").read_text(encoding="utf-8"))
        table = PAGE[PAGE.index("const COLOURS = {"):PAGE.index("};", PAGE.index("const COLOURS = {"))]
        colours = {k: (c, d) for k, c, d in re.findall(r'(\w+): \["(#[0-9a-f]{6})", "([^"]*)"\]', table)}
        self.assertEqual(sorted(colours), sorted(roster))
        solid = [c for c, d in colours.values() if not d]
        self.assertEqual(len(solid), len(set(solid)))
        self.assertEqual(len(solid), 8)
        self.assertEqual(len(set(colours.values())), len(colours))

    def test_the_graph_is_drawn_after_the_record_is_shown(self):
        # A hidden card has no width: the record is shown before the graph
        # is measured.
        script = PAGE[PAGE.index('id="elo-data">null</script>'):]
        self.assertLess(script.index('document.getElementById("elo-record").hidden = false;'), script.index("const draw = () => {"))
        self.assertNotIn("innerHTML", PAGE)
        self.assertNotIn("Date.now()", PAGE)
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])


if __name__ == "__main__":
    unittest.main()
