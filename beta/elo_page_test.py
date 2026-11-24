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
        self.assertEqual(sections, ["rank-strategies", "elo-levels", "elo-over-time"])
        self.assertIn('<h2 class="rank-title" id="rank-strategies-title">Player strategies</h2>', PAGE)
        self.assertIn('<h2 class="rank-title" id="elo-over-time-title">Elo over time</h2>', PAGE)

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
