#!/usr/bin/env python3
"""The Tournaments page: its record is counted off the ledger, and it fits its slot.

The last class here opens the page in a browser, filled with a record, and
reads the row of game squares as it is drawn: the labels at its two ends and
where they stand. Set `CIVVIS_TEST_CHROME` to run it with a particular
browser binary.
"""

from __future__ import annotations

import contextlib
import copy
import io
import json
import os
import pathlib
import re
import shutil
import subprocess
import tempfile
import time
import unittest
import urllib.request

import elo_page
import home_rankings
import tournaments_page
from verify import Devtools, find_chrome, free_port, fresh_profile, tether

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "tournaments.html").read_text(encoding="utf-8")
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")
VIEWER = (ROOT / "web" / "index.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")

ON = {"players": 6, "map": "Continents.lua"}
MATCH = {"players": 6, "map": "Continents.lua"}


def rated(elo: float, margin: float, games: int, wins: int, **more) -> dict:
    return {"elo": elo, "margin": margin, "games": games, "wins": wins, **more}


def elo_record() -> dict:
    """ELO Rankings' record (`beta/elo_page.py`) as the ELO Rankings table on
    this page reads it: two agents, two Firaxis levels and one person, in
    each reading. Diplo off and Diplo on order them differently."""
    def reading(holy, jack, deity, king, dev):
        return {
            "strategies": {"rows": [
                {"kind": "agent", "id": "holy", "name": "HolyRoller", "tilt": "Religion", **rated(holy, 30, 400, 250)},
                {"kind": "agent", "id": "jack", "name": "JackOfAllTrades", "tilt": "Balanced", **rated(jack, 29, 400, 200)},
                {"kind": "rung", "name": "Deity", "elo": 1955},
            ], "tables": 900, "players": 4, "turns": 250, "runs": 3, "latest_run": "2026-11-27"},
            "firaxis": {"days": 7, "games": 40, "to": "2026-11-27T10:00:00Z", "levels": [
                {"level": 8, "name": "Deity", "now": rated(deity, 90, 30, 9)},
                {"level": 5, "name": "King", "now": rated(king, 150, 10, 7)},
                {"level": 4, "name": "Prince", "now": None},
            ]},
            "people": [{"name": "dev", **rated(dev, 180, 3, 2)}],
        }
    on = reading(holy=1547, jack=1155, deity=1042, king=595, dev=1210)
    on["off"] = reading(holy=1543, jack=1191, deity=1452, king=799, dev=1100)
    # Two of the named setups' fits (`strategy_elo --setup`): the agents
    # alone, no Firaxis level or person read beside them.
    def agents_only(holy, jack):
        rows = [{"kind": "agent", "id": "holy", "name": "HolyRoller", "tilt": "Religion", **rated(holy, 40, 100, 40)}]
        if jack is not None:
            rows.append({"kind": "agent", "id": "jack", "name": "JackOfAllTrades", "tilt": "Balanced", **rated(jack, 40, 100, 50)})
        return {"strategies": {"rows": rows, "tables": 120, "players": 6, "turns": 250, "runs": 1}, "firaxis": None, "people": []}
    on["setups"] = {"continents6_science": agents_only(1300, 1400), "pangaea2": agents_only(1200, None)}
    return on


def run(tag: str, outcome: str, civ: str | None, minute: int, **more) -> dict:
    """One line of the results file, as `civ6_ladder.result_row` writes it."""
    return {
        "tag": tag,
        "game": re.sub(r"-cont\d+$", "", tag),
        "utc": f"2026-11-21T10:{minute:02d}:00Z",
        "civ": civ,
        "difficulty": "DIFFICULTY_DEITY",
        "outcome": outcome,
        "victory": None,
        "turns": 100 + minute,
        **ON,
        **more,
    }


class TournamentsPage(unittest.TestCase):
    def record(self, *more: dict) -> dict:
        """Four definitions: one played out, one being played, one waiting
        behind a tag prefix of its own, and one whose time came and went."""
        specs = [
            {"name": "First idea", "idea": "Does the opening matter?", "conditions": "Lv 8 6P Continents Online",
             "start": "civvis-20261121T100000Z", "end": "civvis-20261121T102000Z", "series_size": 2,
             "match": {**MATCH, "difficulty": "DIFFICULTY_DEITY"}, "note": "Kept."},
            {"name": "Second idea", "conditions": "Lv 8 6P Continents Online",
             "start": "civvis-20261121T102000Z", "series_size": 5,
             "match": {**MATCH, "difficulty": "DIFFICULTY_DEITY"}},
            {"name": "Never played", "conditions": "c", "start": "civvis-20261121T090000Z",
             "end": "civvis-20261121T093000Z", "series_size": 3, "match": {"players": 8}},
            {"name": "Level ladder", "idea": "Two agents at one table, level by level.",
             "conditions": "Lv 4-8 6P Continents Online, 2 CIVVIS + 4 AI",
             "start": "civvis-t6-", "end": "civvis-t7-", "series_size": 50, "match": MATCH},
        ]
        rows = [
            run("civvis-20261121T100000Z", "win", "CIVILIZATION_ROME", 5, victory="VICTORY_DIPLOMATIC"),
            run("civvis-20261121T101000Z", "rival_victory", "CIVILIZATION_GRAN_COLOMBIA", 15, victory="VICTORY_TECHNOLOGY"),
            run("civvis-20261121T102000Z", "defeat", "CIVILIZATION_MALI", 25),
            run("civvis-20261121T103000Z", "unfinished", None, 31),                      # never reached a board
            run("civvis-20261121T103500Z", "rival_victory", "CIVILIZATION_GAUL", 40),
            run("civvis-20261121T103500Z-cont1", "win", "CIVILIZATION_GAUL", 45, victory="VICTORY_RELIGIOUS"),
            run("civvis-20261121T105000Z", "abandoned", "CIVILIZATION_KOREA", 55),
            # Runs from the ledger's older tag families sort after every
            # prefix a scheduled table owns and were played months before.
            run("settler-20260730T101433Z", "win", None, 0, utc="2026-07-30T10:20:00Z", players=None, map=None),
            run("civvis-tinywar-20260731T231249Z", "unfinished", None, 0, utc="2026-07-31T23:30:00Z", players=None, map=None),
        ]
        return tournaments_page.record(rows + list(more), specs, ["Rome", "Gran Colombia", "Mali", "Gaul", "Korea"],
                                       self.videos, self.descriptions, self.listed)

    #: The definitions file's `videos`, `descriptions` and `hyperparameters`, as `record` is handed them.
    videos: dict | None = None
    descriptions: dict | None = None
    listed: dict | None = None

    def test_the_record_is_what_runs_what_waits_and_what_has_finished(self):
        record = self.record()
        self.assertEqual([(t["id"], t["name"]) for t in record["finished"]], [(1, "First idea")])
        self.assertEqual([(t["id"], t["name"]) for t in record["running"]], [(2, "Second idea")])
        # A definition with no finished game waits, numbered on from the
        # last tournament begun. One whose `end` the games have passed never
        # ran and is not listed.
        self.assertEqual(record["scheduled"], [{
            "id": 3, "description": "", "name": "Level ladder", "idea": "Two agents at one table, level by level.",
            "hyperparameters": [], "conditions": "Lv 4-8 6P Continents Online, 2 CIVVIS + 4 AI", "note": "", "size": 50,
        }])
        first, second = record["finished"][0], record["running"][0]
        self.assertEqual((first["games"], first["size"], first["wins"], first["losses"]), (2, 2, 1, 1))
        self.assertEqual((first["start"], first["end"]), ("2026-11-21 10:00", "2026-11-21 10:15"))
        self.assertEqual((first["idea"], first["note"]), ("Does the opening matter?", "Kept."))
        self.assertEqual((second["games"], second["size"], second["wins"], second["losses"]), (3, 5, 1, 2))
        self.assertIsNone(second["end"])
        self.assertEqual(second["idea"], "")
        # The queue moving on is what retires a definition that never ran: a
        # table tagged past the ladder's `end`, played after the games ahead
        # of it. The old runs that merely sort past it retire nothing (above).
        later = run("civvis-t7-20261121T120000Z", "unfinished", None, 0, utc="2026-11-21T12:10:00Z")
        self.assertEqual(self.record(later)["scheduled"], [])
        # The totals count every game of every tournament begun, through
        # the last of them. No definition here is the benchmark.
        self.assertEqual(record["totals"], {"tournaments": 2, "games": 5, "wins": 2, "losses": 3, "through": "2026-11-21 10:55"})
        self.assertEqual(record["through"], "2026-11-21 10:55")
        self.assertIsNone(record["benchmark"])

    def test_dates_are_plain_and_a_series_carries_its_wall_time_and_run_time(self):
        # Operator 2026-11-21: "for all start and end dates, lets use
        # YYYY-MM-DD 24 hr time with no z. add a wall time and run time
        # duration cols."
        record = self.record()
        first, second = record["finished"][0], record["running"][0]
        dated = re.compile(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}")
        for stamp in (first["start"], first["end"], second["start"], record["through"]):
            self.assertTrue(dated.fullmatch(stamp), stamp)
        self.assertEqual(second["start"], "2026-11-21 10:20")
        # The home page's count is not touched: it still writes its `Z`, for
        # the live mirror's Mode line and a lane pinned to the older page.
        series = tournaments_page.ranks.tournament_series(
            [run("civvis-20261121T100000Z", "win", "CIVILIZATION_ROME", 5)],
            [{"name": "n", "start": "civvis-20261121T100000Z", "series_size": 1, "match": MATCH}])
        self.assertEqual((series[0]["start"], series[0]["end"]), ("2026-11-21 10:00Z", "2026-11-21 10:05Z"))
        # The first idea's two games ran 10:00 to 10:05 and 10:10 to 10:15:
        # a quarter of an hour on the clock, ten minutes of it in play.
        self.assertEqual((first["wall"], first["run"]), ("15m", "10m"))
        # The running one's three: 10:20 to 10:25, 10:35 to the 10:45 result
        # of the resumed run that counted, and 10:50 to 10:55. Both run to
        # its last finished game.
        self.assertEqual((second["wall"], second["run"]), ("35m", "20m"))
        # Hours and minutes from an hour up, to the nearest minute.
        self.assertEqual([tournaments_page.lasted(seconds) for seconds in (0, 59 * 60 + 29, 59 * 60 + 30, 79380, 100 * 3600)],
                         ["0m", "59m", "1h 00m", "22h 03m", "100h 00m"])
        # A night the machine was off is wall time and not run time.
        night = [("civvis-20261120T230000Z", {"utc": "2026-11-20T23:12:00Z"}),
                 ("civvis-20261121T090000Z", {"utc": "2026-11-21T09:15:00Z"})]
        self.assertEqual(tournaments_page.durations(night), ("10h 15m", "27m"))
        # A game with no time in its tag, or no result time, is not measured,
        # and a series with none that can be has neither figure.
        self.assertEqual(tournaments_page.durations(night + [("civvis-t6-", {"utc": "2026-11-21T09:30:00Z"}),
                                                             ("civvis-20261121T093000Z", {"utc": None})]),
                         ("10h 15m", "27m"))
        self.assertEqual(tournaments_page.durations([("settler-old", {"utc": "2026-07-30T10:20:00Z"})]), ("", ""))
        # A start that is not a date is shown as it is, and no end stays none.
        self.assertEqual(tournaments_page.plain("settler-old"), "settler-old")
        self.assertIsNone(tournaments_page.plain(None))

    def test_a_series_carries_its_games_in_the_order_played(self):
        first, second = self.record()["finished"][0], self.record()["running"][0]
        self.assertEqual(first["results"], [
            {"civ": "Rome", "won": True, "how": "Diplomatic victory", "turns": 105, "level": 8},
            {"civ": "Gran Colombia", "won": False, "how": "Rival Science victory", "turns": 115, "level": 8},
        ])
        # A game reloaded from its autosave is one game, by its best run.
        self.assertEqual([(g["civ"], g["how"]) for g in second["results"]],
                         [("Mali", "Eliminated"), ("Gaul", "Religious victory"), ("Korea", "Conceded")])
        # Wins by victory and losses by how they were lost, most first.
        self.assertEqual(first["victories"], [["Diplomatic", 1]])
        self.assertEqual(first["endings"], [["Rival Science victory", 1]])
        self.assertEqual(second["victories"], [["Religious", 1]])
        self.assertEqual(second["endings"], [["Conceded", 1], ["Eliminated", 1]])
        # One level throughout: no record per level.
        self.assertEqual(first["levels"], [])

    def test_a_table_of_two_agents_and_mixed_levels_reads_plainly(self):
        # Tournament 6 (operator 2026-11-21): hotseat tables with two CIVVIS
        # agents, ten games at each level from 4 to 8. The run records both
        # civs joined by `+`, and each game its own difficulty.
        spec = {"name": "Level ladder", "conditions": "c", "start": "civvis-t6-", "end": "civvis-t7-",
                "series_size": 50, "match": MATCH}
        rows = [
            run("civvis-t6-20261121T100000Z", "win", "CIVILIZATION_PHOENICIA+CIVILIZATION_AUSTRALIA", 5,
                difficulty="DIFFICULTY_PRINCE", victory="VICTORY_TECHNOLOGY"),
            run("civvis-t6-20261121T101000Z", "rival_victory", "CIVILIZATION_ROME+CIVILIZATION_MALI", 15,
                difficulty="DIFFICULTY_PRINCE", victory="VICTORY_CULTURE"),
            run("civvis-t6-20261121T102000Z", "win", "CIVILIZATION_ROME+CIVILIZATION_GAUL", 25,
                difficulty="DIFFICULTY_KING", victory="VICTORY_TECHNOLOGY"),
            run("civvis-20261121T103000Z", "win", "CIVILIZATION_ROME", 35),   # not this table's tag
        ]
        record = tournaments_page.record(rows, [spec], ["Phoenicia", "Australia", "Rome", "Mali", "Gaul"])
        (ladder,) = record["running"]
        self.assertEqual(ladder["games"], 3)
        self.assertEqual(ladder["results"][0]["civ"], "Phoenicia & Australia")
        self.assertEqual([g["level"] for g in ladder["results"]], [4, 4, 5])
        self.assertEqual(ladder["levels"], [[4, 1, 1], [5, 1, 0]])
        self.assertEqual(ladder["victories"], [["Science", 2]])
        self.assertEqual(record["scheduled"], [])

    def test_an_indefinite_tournament_benchmarks_its_win_rate_by_level_day_by_day(self):
        # Operator 2026-11-22: "whenever we have no other tourney, we run lv 6,
        # lv 7, lv 8 (x 2) games. diplomacy off. benchmark our win rates
        # roughly for these games over time."
        spec = {"name": "Bench", "conditions": "c", "start": "civvis-t7-", "end": "civvis-t8-",
                "indefinite": True, "match": MATCH}
        turn = ["DIFFICULTY_EMPEROR", "DIFFICULTY_IMMORTAL", "DIFFICULTY_DEITY", "DIFFICULTY_DEITY"]
        rows = []
        for i in range(12):
            day = 21 + i // 8
            rows.append(run(f"civvis-t7-202611{day}T{i:02d}0000Z", "win" if i % 4 != 3 else "rival_victory",
                            "CIVILIZATION_ROME", 0, utc=f"2026-11-{day}T{i:02d}:30:00Z", difficulty=turn[i % 4]))
        record = tournaments_page.record(rows, [spec], ["Rome"])
        (bench,) = record["running"]
        self.assertEqual((bench["games"], bench["size"], bench["end"]), (12, None, None))
        self.assertEqual(bench["benchmark"]["days"], ["11-21", "11-22"])
        self.assertEqual(
            [(lv["level"], lv["wins"], lv["games"], lv["days"]) for lv in bench["benchmark"]["levels"]],
            [(6, 3, 3, [[2, 2], [1, 1]]), (7, 3, 3, [[2, 2], [1, 1]]), (8, 3, 6, [[2, 4], [1, 2]])])
        # The 95% range: Wilson's, honest at 100% on three games.
        self.assertEqual(bench["benchmark"]["levels"][0]["range"], [44, 100])
        self.assertEqual(tournaments_page.wilson(50, 100), [40, 60])
        self.assertIsNone(tournaments_page.wilson(0, 0))
        # A series of a fixed size carries no benchmark.
        self.assertNotIn("benchmark", self.record()["running"][0])
        # Scheduled, it has no size to show.
        waiting = tournaments_page.record([], [spec], ["Rome"])
        self.assertEqual([(t["name"], t["size"]) for t in waiting["scheduled"]], [("Bench", None)])
        for piece in ('t.size == null ? " · indefinite" : " of " + t.size',
                      't.size == null ? "Indefinite" : plural(t.size, "game", "games")',
                      'if (bench(t.benchmark)) body.append(bench(t.benchmark));',
                      'el("p", "tour-latest-title", "Win rate by level")'):
            self.assertIn(piece, PAGE)

    def test_the_benchmark_is_not_an_experiment(self):
        # Operator 2026-11-25: "tourney 7 is benchmark. please remove from
        # experiments and make the official benchmark for now." The
        # definition marked `benchmark` is drawn on its own, with no number;
        # the tournaments are numbered without it, so one begun after it and
        # one scheduled after that are 3 and 4, not 4 and 5.
        specs = [
            {"name": "First idea", "conditions": "c", "start": "civvis-20261121T100000Z",
             "end": "civvis-20261121T102000Z", "series_size": 2, "match": MATCH},
            {"name": "Bench", "benchmark": True, "description": "Official Benchmark", "conditions": "c",
             "start": "civvis-t7-", "end": "civvis-t8-", "indefinite": True, "match": MATCH,
             "hyperparameters": {"Games": "Indefinite"}},
            {"name": "After the bench", "conditions": "c", "start": "civvis-t8-", "end": "civvis-t9-",
             "series_size": 2, "match": MATCH},
            {"name": "Waiting", "conditions": "c", "start": "civvis-t9-", "series_size": 3, "match": MATCH},
        ]
        rows = [
            run("civvis-20261121T100000Z", "win", "CIVILIZATION_ROME", 5),
            run("civvis-20261121T101000Z", "defeat", "CIVILIZATION_ROME", 15),
            run("civvis-t7-20261121T110000Z", "win", "CIVILIZATION_ROME", 0, utc="2026-11-21T11:30:00Z"),
            run("civvis-t7-20261121T120000Z", "defeat", "CIVILIZATION_ROME", 0, utc="2026-11-21T12:30:00Z"),
            run("civvis-t8-20261121T130000Z", "win", "CIVILIZATION_ROME", 0, utc="2026-11-21T13:30:00Z"),
        ]
        # The file's lines are by a tournament's number: "2" is the second
        # tournament's, never the benchmark's, though the benchmark began
        # second.
        descriptions = {"1": "One", "2": "Two", "3": "Three"}
        listed = {"2": {"Games": "Two's"}}
        record = tournaments_page.record(rows, specs, ["Rome"], {"2": "https://youtu.be/an7y2IlGhU0"}, descriptions, listed)
        tournaments = record["finished"] + record["running"]
        self.assertEqual(sorted((t["id"], t["name"], t["description"]) for t in tournaments),
                         [(1, "First idea", "One"), (2, "After the bench", "Two")])
        self.assertEqual(record["running"][0]["hyperparameters"], [["Games", "Two's"]])
        self.assertEqual(record["running"][0]["video"], "https://www.youtube.com/watch?v=an7y2IlGhU0")
        self.assertEqual([(t["id"], t["name"], t["description"]) for t in record["scheduled"]], [(3, "Waiting", "Three")])
        bench = record["benchmark"]
        self.assertEqual((bench["id"], bench["name"], bench["description"], bench["games"], bench["size"]),
                         (None, "Bench", "Official Benchmark", 2, None))
        self.assertEqual((bench["hyperparameters"], bench["video"], bench["last"]), ([["Games", "Indefinite"]], "", "2026-11-21 12:30"))
        self.assertIn("benchmark", bench)   # its win rate by level
        # The totals are the tournaments' own games, through the last of
        # them; the page's `through` is every game's.
        self.assertEqual(record["totals"], {"tournaments": 2, "games": 3, "wins": 2, "losses": 1, "through": "2026-11-21 13:30"})
        # It is never scheduled, games or none.
        self.assertIsNone(tournaments_page.record([], specs[1:2], ["Rome"])["benchmark"])
        self.assertEqual(tournaments_page.record([], specs[1:2], ["Rome"])["scheduled"], [])
        # The committed definitions: one is the benchmark, with its own
        # title line and its own hyperparameters; the five before it are
        # Tournaments 1 to 6 (the fourth is Tournaments 4 and 5).
        defined = json.loads((ROOT / "data" / "tournaments.json").read_text(encoding="utf-8"))
        marked = [spec for spec in defined["tournaments"] if spec.get("benchmark")]
        self.assertEqual([(spec["name"], spec["description"], spec["start"]) for spec in marked],
                         [("Lv 4 to Lv 8 against the Firaxis AI, 1 CIVVIS agent + 5 AI", "Official Benchmark", "civvis-t7-")])
        # Operator 2026-11-26: "please add a tournament with 100 lv 8 deity
        # games, no diplo." The definition after the benchmark's is
        # Tournament 7: its line in `descriptions` and `hyperparameters` is
        # by that number, every one of the thirteen hyperparameters listed.
        self.assertEqual(defined["descriptions"]["7"], "100 Lv 8 Deity games, no Diplo allowed")
        self.assertEqual(list(defined["hyperparameters"]["7"]), list(tournaments_page.HYPERPARAMETERS))
        self.assertNotIn("8", defined["descriptions"])
        # One game in each committed tournament's range (51 in the fourth
        # definition's, so it is Tournaments 4 and 5) and one benchmark game.
        def game(tag: str, **more) -> dict:
            stamp = re.search(r"(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})", tag)
            return run(tag, "rival_victory", "CIVILIZATION_ROME", 0,
                       utc=f"{stamp[1]}-{stamp[2]}-{stamp[3]}T{stamp[4]}:{stamp[5]}:30Z", **more)
        committed = defined["tournaments"]
        rows = [game("civvis-20261119T190000Z"), game("civvis-20261119T210000Z"), game("civvis-20261120T000000Z")]
        rows += [game(f"civvis-20261120T06{minute:02d}00Z") for minute in range(51)]
        rows += [game("civvis-t6-20261122T010000Z", difficulty="DIFFICULTY_KING"),
                 game("civvis-t7-20261126T010000Z", difficulty="DIFFICULTY_PRINCE")]
        listed = lambda rows: tournaments_page.record(rows, committed, ["Rome"], defined["videos"],
                                                      defined["descriptions"], defined["hyperparameters"])
        before = listed(rows)
        self.assertEqual(sorted(t["id"] for t in before["running"] + before["finished"]), [1, 2, 3, 4, 5, 6])
        self.assertEqual(before["benchmark"]["games"], 1)
        self.assertEqual([(t["id"], t["description"], t["size"]) for t in before["scheduled"]],
                         [(7, "100 Lv 8 Deity games, no Diplo allowed", 100)])
        self.assertEqual([name for name, _ in before["scheduled"][0]["hyperparameters"]],
                         list(tournaments_page.HYPERPARAMETERS))
        # Its first game (tagged `civvis-t8-`, the benchmark's `end`) makes
        # it the tournament running, still number 7, and the benchmark's
        # count does not take it; a game below Deity is not one of its own.
        during = listed(rows + [game("civvis-t8-20261126T070000Z"),
                                game("civvis-t8-20261126T080000Z", difficulty="DIFFICULTY_IMMORTAL")])
        self.assertEqual(during["scheduled"], [])
        self.assertEqual([(t["id"], t["description"], t["games"], t["size"]) for t in during["running"]],
                         [(7, "100 Lv 8 Deity games, no Diplo allowed", 1, 100)])
        self.assertEqual(during["benchmark"]["games"], 1)
        # The page draws it in the right-hand column, under Scheduled.
        side = PAGE[PAGE.index('<div class="tour-side">'):PAGE.index('<p class="tour-legend">')]
        self.assertLess(side.index('id="tour-scheduled"'), side.index('id="tour-benchmark"'))
        for piece in (
            '<section class="tour-section" id="tour-benchmark" hidden aria-labelledby="tour-benchmark-title">',
            '<h2 class="rank-title" id="tour-benchmark-title">Benchmark</h2>',
            "const benchmark = data.benchmark;",
            'benchBox.querySelector("[data-cards]").append(playing(benchmark));',
            'line.append(el("span", "tour-no", t.description || "Benchmark"));',
            '"Our official benchmark against the Firaxis AI, played whenever no tournament is"',
            '"Nothing is scheduled. Until something is, the agents play the benchmark."',
        ):
            self.assertIn(piece, PAGE)

    def test_the_counting_is_the_home_tables_own(self):
        # The page, the live mirror's Mode line and an older home page's
        # table count one set of series: the detail the page asks for adds
        # keys to `tournament_series` rows and changes none of them.
        specs = json.loads((ROOT / "data" / "tournaments.json").read_text(encoding="utf-8"))["tournaments"]
        rows = [run("civvis-20261119T190000Z", "win", "CIVILIZATION_ROME", 5),
                run("civvis-20261120T050000Z", "defeat", "CIVILIZATION_MALI", 15)]
        plain = home_rankings.tournament_series(rows, specs)
        detailed = home_rankings.tournament_series(rows, specs, detail=True)
        self.assertEqual(
            [{key: value for key, value in row.items() if key not in ("definition", "played")} for row in detailed],
            plain,
        )
        self.assertTrue(all("played" not in row and "definition" not in row for row in plain))
        record = tournaments_page.record(rows, specs, [])
        self.assertEqual(
            sorted(t["id"] for t in record["running"] + record["finished"]), sorted(row["id"] for row in plain))
        # Every committed definition's idea, where it has one, is a sentence.
        for spec in specs:
            if "idea" in spec:
                self.assertIsInstance(spec["idea"], str)
                self.assertTrue(spec["idea"].strip())

    def test_the_page_carries_one_empty_slot_and_publishing_fills_it(self):
        self.assertEqual(PAGE.count('<script type="application/json" id="tournaments-data">null</script>'), 1)
        record = self.record()
        record["running"][0]["note"] = "</script><b>x"   # a sentence cannot close the slot
        filled = tournaments_page.fill(PAGE, record)
        self.assertEqual(filled.count("</script>"), PAGE.count("</script>"))
        self.assertEqual(json.loads(tournaments_page.SLOT.search(filled)[2]), record)
        # Nothing but the slot moved.
        self.assertEqual(tournaments_page.SLOT.sub("", filled), tournaments_page.SLOT.sub("", PAGE))
        # The publish script is what runs it: the lane's own page, filled by
        # the lane's own script from the lane's own definitions. (Both lanes
        # had head's definitions until 2026-11-22; a tournament scheduled on
        # main now reaches the front page's list with a promotion.)
        self.assertNotIn('"$repo_root/beta/tournaments', PUBLISH)
        for line in (
            'tournaments_page="$source_tree/beta/tournaments.html"',
            'if [ -f "$tournaments_page" ]; then',
            # At the address the page states itself; `tournaments` is where
            # a revision from before pages said so had it.
            'tournaments_at="$(address "$tournaments_page" tournaments)"',
            'cp "$tournaments_page" "$out/$tournaments_at/index.html"',
            'fill tournaments_page.py "$out/$tournaments_at/index.html"',
            'pages+=("$tournaments_at")',
            # And its links are held inside the lane like every page's: the
            # guard runs over every page the lane carries.
            'for address in carried:',
        ):
            self.assertIn(line, PUBLISH)
        # The address follows the page's name in the site list (operator
        # 2026-11-23: "clean up the URL please. standardized."), and the
        # page is Tournaments again (operator 2026-11-26: "change
        # experiments back to tournaments"; it was Experiments from
        # 2026-11-25, "rename tournaments to experiments").
        self.assertIn('<link rel="canonical" href="https://civvis.ai/tournaments/">', PAGE)
        self.assertNotIn("civvis.ai/experiments/", PAGE)
        # The page is authored lane-relative: nothing in it links the site root.
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])

    def test_elo_rankings_stands_across_the_top_with_a_column_a_setup_and_publishing_fills_it(self):
        # Operator 2026-11-27: "lets add victory type cols to elo rankings
        # and move elo rankings to the top full wdith of tournaments. 6p
        # Continents Online (Diplo off, Diplo on, science, culture, religion,
        # diplomacy, domination, score) , 2P Pangaea (religion off, religion
        # on). we can elo for each, sortable by score". It stood on the right
        # over the benchmark from earlier that day ("list Diplo off first (and
        # default for sorting)"): now it is the record's first child, before
        # both columns, so it spans the page.
        record = PAGE[PAGE.index('<div class="tour-record" id="tour-record" hidden>'):PAGE.index('<p class="tour-legend">')]
        self.assertLess(record.index('id="tour-elo"'), record.index('<div class="tour-main">'))
        self.assertNotIn('id="tour-elo"', record[record.index('<div class="tour-side">'):])
        self.assertIn(".tour-record > :not(.tour-main):not(.tour-side) { grid-column: 1 / -1; }", PAGE)
        self.assertIn('>ELO Rankings</a></h2>', record)
        # A column for each setup, in the operator's order, under its game;
        # Diplo off first and pressed, the order it opens in.
        # Two tables (operator, the same evening: "we should have two
        # tables: one on the left for computer control firaxis civ 6 games
        # and one on the right for CIVVIS self-play"): Civilization VI's
        # Diplo off and on, then CIVVIS self-play's ten. Both Diplo off heads
        # stand pressed: the page opens on the one-scale ranking by Diplo
        # off, wherever a player's figure was measured.
        heads = re.findall(r'class="rank-sort" data-sort="([\w:]+)" aria-pressed="(\w+)"[^>]*>([^<]+)<', record)
        self.assertEqual(heads, [
            ("control:continents6_no_diplo", "true", "Diplo off"), ("control:continents6", "false", "Diplo on"),
            ("self:continents6_no_diplo", "true", "Diplo off"), ("self:continents6", "false", "Diplo on"),
            ("self:continents6_science", "false", "Science"), ("self:continents6_culture", "false", "Culture"),
            ("self:continents6_religion", "false", "Religion"), ("self:continents6_diplomacy", "false", "Diplomacy"),
            ("self:continents6_domination", "false", "Domination"), ("self:continents6_score", "false", "Score"),
            ("self:pangaea2_no_religion", "false", "Religion off"), ("self:pangaea2", "false", "Religion on")])
        pair = record[record.index('<div class="elo-pair">'):]
        self.assertLess(pair.index('data-elo-side="control"'), pair.index('data-elo-side="self"'))
        self.assertIn('>Civilization VI &#183; computer control</th>', pair)
        self.assertIn('>CIVVIS self-play</th>', pair)
        self.assertIn('colspan="8" class="is-group is-split">6P Continents Online</th>', record)
        self.assertIn('colspan="2" class="is-group is-split">2P Pangaea</th>', record)
        self.assertEqual(record.count('class="is-group is-split">6P Continents Online</th>'), 2)
        self.assertIn("show(OPEN);", PAGE)
        # A row is one fixed height in both tables, so the two line up.
        self.assertIn("#tour-elo .elo-pair tbody tr { height: 28px; }", PAGE)
        self.assertIn("#tour-elo .elo-pair thead tr { height: 24px; }", PAGE)
        # Each kind faintly its colour ("we can highlight dev blue, civvis
        # gold, and firaxis red in the elo table. faint"), with a key.
        for kind, ink in (("agent", "var(--gold) 8%"), ("level", "#e5534b 10%"), ("person", "var(--accent) 11%")):
            self.assertIn(f"#tour-elo tbody tr.is-{kind} {{ background: color-mix(in srgb, {ink}, transparent); }}", PAGE)
            self.assertIn(f'<span class="elo-kind is-{kind}">', record)
        # Its figures are ELO Rankings' own record, written by the same
        # script into a slot of the same name, once.
        self.assertEqual(PAGE.count('<script type="application/json" id="elo-data">null</script>'), 1)
        self.assertLess(PUBLISH.index('fill tournaments_page.py "$out/$tournaments_at/index.html"'),
                        PUBLISH.index('fill elo_page.py "$out/$tournaments_at/index.html"'))
        filled = elo_page.fill(tournaments_page.fill(PAGE, self.record()), elo_record())
        self.assertEqual(json.loads(elo_page.SLOT.search(filled)[2]), elo_record())
        self.assertEqual(json.loads(tournaments_page.SLOT.search(filled)[2]), self.record())
        self.assertEqual(filled.count("</script>"), PAGE.count("</script>"))
        # Its rules are its own card's: the page's own `.rank-*` are not touched.
        style = re.sub(r"/\*.*?\*/", "", PAGE[:PAGE.index("</style>")], flags=re.S)
        rules = [part.rsplit("{", 1)[0].split("{")[-1].strip() for part in style.split("}") if "{" in part]
        for rule in rules:
            if any(name in rule for name in (".rank-sort", ".elo-kind", "tr.is-agent", "tr.is-level", "tr.is-person",
                                             "td.is-value", "td.is-sorted", "td.is-split")):
                self.assertTrue(all(part.strip().startswith("#tour-elo") for part in rule.split(",")), rule)

    def test_without_a_ledger_the_page_says_it_has_no_record(self):
        with tempfile.TemporaryDirectory() as empty:
            source = pathlib.Path(empty)
            self.assertIsNone(tournaments_page.tournaments(source))
            page = source / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(tournaments_page.main(["tournaments_page.py", empty, str(page)]), 0)
            self.assertEqual(tournaments_page.SLOT.search(page.read_text(encoding="utf-8"))[2], "null")
            # A page with no slot is left byte for byte.
            page.write_text("<html>no slot</html>", encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(tournaments_page.main(["tournaments_page.py", empty, str(page)]), 0)
            self.assertEqual(page.read_text(encoding="utf-8"), "<html>no slot</html>")
        # The unfilled page shows one sentence and keeps the record hidden.
        self.assertIn('<p class="tour-empty" id="tour-empty">This copy of the page has no tournament record.', PAGE)
        self.assertIn('<div class="tour-record" id="tour-record" hidden>', PAGE)
        self.assertIn("if (!data) return;", PAGE)

    def test_a_definition_it_cannot_count_costs_the_record_not_the_publish(self):
        with tempfile.TemporaryDirectory() as scratch:
            source = pathlib.Path(scratch)
            (source / "data").mkdir()
            (source / "data" / "tournaments.json").write_text('{"tournaments": [{"series_size": "many"}]}', encoding="utf-8")
            rows = [run("civvis-20261121T100000Z", "win", "CIVILIZATION_ROME", 5)]
            results = home_rankings.results
            home_rankings.results = lambda _source: rows
            try:
                said = io.StringIO()
                with contextlib.redirect_stderr(said):
                    self.assertIsNone(tournaments_page.tournaments(source))
                self.assertIn("no record", said.getvalue())
            finally:
                home_rankings.results = results

    def test_every_page_lists_tournaments_between_charts_and_civvis(self):
        # Operator 2026-11-21: "Between Home and CIVVIS, we want to add a
        # Tournaments view." This page lists the site's pages as the others
        # do, with itself marked. It was named Auto-Tourneys in the list
        # and on the tab from 2026-11-22 ("make sure Tournaments changes to
        # Auto-Tourneys") and at /auto-tourneys/ from 2026-11-23, and took
        # its first name and address back on 2026-11-24 ("change
        # auto-outnrye to Tournaments. Auto-tourney suggests some self
        # imrpvoing tourney thing"): Tournaments, at /tournaments/. On
        # 2026-11-25 it was named Experiments ("rename tournaments to
        # experiments"), in the list, on the tab and in its address,
        # /experiments/, and each series on it "Experiment 4: 50 game
        # series"; on 2026-11-26 it took its name, its address and the
        # series' titles back ("change experiments back to tournaments"):
        # "Tournament 4: 50 game series". As on the home
        # page, the list is a row across the top, over the title (operator
        # 2026-11-24: "try putting the vertical menu from home... civ across
        # the top instead"; from 2026-11-21 it stood to the left of the
        # title), and Charts/ drops its pages down.
        self.assertRegex(PAGE, r'<header class=\"shell site-header\">\s*<!--[\s\S]*?-->\s*<nav class=\"site-nav\"[^>]*>[\s\S]*?</nav>\s*<script data-site-folder>[\s\S]*?</script>\s*<div class=\"site-brand\">\s*<div class=\"site-lockup\">')
        self.assertRegex(LANDING, r'<header class=\"shell site-header\">\s*<!--[\s\S]*?-->\s*<nav class=\"site-nav\"[^>]*>[\s\S]*?</nav>\s*<script data-site-folder>[\s\S]*?</script>\s*<div class=\"site-brand\">\s*<div class=\"site-lockup\">')
        self.assertIn(
            '      </div>\n'
            '      <a class="site-link" href="./" aria-current="page">Tournaments</a>\n'
            '      <a class="site-link" href="../" title="Every civilization is played by a CIVVIS AI agent.">CIVVIS</a>\n'
            '      <a class="site-link" href="../doctor-strange" '
            'title="One world, two timelines: the same game played two ways, side by side, on a timeline that branches.">Doctor Strange\'s CIVVIS</a>\n'
            '      <a class="site-link" href="../edge-of-tomorrow" '
            'title="You play one civilization, with a jump back: the world returns to the turn you woke on, as it stood.">CIVVIS: Edge of Tomorrow</a>\n'
            '      <a class="site-link" href="../debugger" '
            'title="You play one civilization. CIVVIS AI agents play the rest.">CIVVIS AI Debugger</a>\n'
            '      <a class="site-link" href="https://store.steampowered.com/app/289070/Sid_Meiers_Civilization_VI/" '
            'target="_blank" rel="noopener" title="Buy Civilization VI on Steam">Civ 6</a>\n'
            "    </nav>",
            PAGE,
        )
        self.assertIn(
            '<nav class="site-nav" aria-label="CIVVIS sites">\n'
            '      <a class="site-link" href="../home/">Home</a>\n'
            '      <div class="site-folder">\n'
            '        <a class="site-link" href="../charts/ai-insights/" '
            'title="What the games show: charts read off the CIVVIS AI agents&#8217; games.">Charts/</a>\n'
            '        <div class="site-menu">\n',
            PAGE,
        )
        self.assertIn('  .site-link[aria-current="page"],\n  .site-folder:has([aria-current="page"]) > .site-link { box-shadow: inset 0 0 0 2px #fff; }\n', PAGE)
        self.assertIn("<title>Tournaments · CIVVIS.ai</title>", PAGE)
        self.assertIn('<meta name="description" content="CIVVIS tournaments: series of verification games ', PAGE)
        # The Debugger is AI Debugger in this list again (operator
        # 2026-11-21: "AI Debugger (rename plz)", after a day as Strategy
        # Debugger and Strat Debugger), and every name stands on one line
        # ("make sure it fits on one line"): the column is as wide as
        # AUTO-TOURNEYS was, at every width, and the rule that
        # narrowed it on a phone under 380px is still gone.
        for page in (PAGE, LANDING, VIEWER):
            self.assertNotIn("Strategy Debugger", page)
            self.assertNotIn("Strat Debugger", page)
        for page in (PAGE, LANDING):
            self.assertNotIn("min-content", page)
        # The home page and the viewer name it third, after Charts (which
        # joined between Home and this page the same day as Research; its
        # own test is `research_page_test.py`), each in its own markup.
        for page, pattern in (
            (LANDING, r'aria-current="page">Home</a>\s*<div class="site-folder">\s*<a class="site-link" href="\.\./charts/ai-insights/"[^>]*>Charts/</a>'
                      r'[\s\S]*?</div>\s*</div>\s*<a class="site-link" href="\.\./tournaments/"[^>]*>Tournaments</a>'
                      r'\s*<a class="site-link" href="\.\./"[^>]*>CIVVIS</a>'),
            (VIEWER, r'id="homelink" href="/home" target="_top">Home</a>\s*'
                     r'<a class="home-link" id="chartslink" href="/charts/ai-insights" target="_top">Charts/</a>\s*'
                     r'<a class="home-link" id="tourlink" href="/tournaments" target="_top">Tournaments</a>\s*'
                     r'<a class="home-link" id="watchlink" href="\./" target="_top">CIVVIS</a>'),
        ):
            self.assertRegex(page, pattern)
        for page in (PAGE, LANDING, VIEWER):
            self.assertNotIn(">Auto-Tourneys</a>", page)
            self.assertNotIn("auto-tourneys/", page)
            # Nor is it listed by the name it had from 2026-11-25 to
            # 2026-11-26, or linked at the address it had.
            self.assertNotIn(">Experiments</a>", page)
            self.assertEqual(re.findall(r'href="[./]*/?experiments/?"', page), [])
        # The viewer's link is root-absolute as written and lane-relative as
        # published: `publish.sh` turns a root link to any page the lane
        # carries into the lane's own.
        self.assertIn('href="/tournaments"', VIEWER)
        self.assertIn("""needle, replacement = f'href="/{address}"', f'href="{address}/"'""", PUBLISH)

    def test_a_tournament_links_its_result_video_on_youtube(self):
        # Operator 2026-11-21: "on our new tournaments page we are building
        # out. we can link to the youtube tournament result videos."
        self.assertEqual([t["video"] for t in self.record()["finished"] + self.record()["running"]], ["", ""])
        # A video is listed by its tournament's number, in either address
        # YouTube gives, and is carried as the watch page.
        self.videos = {"1": "https://youtu.be/an7y2IlGhU0?si=abc", "2": "https://www.youtube.com/watch?v=HgqxiWQKCqc&t=5s"}
        record = self.record()
        self.assertEqual(record["finished"][0]["video"], "https://www.youtube.com/watch?v=an7y2IlGhU0")
        self.assertEqual(record["running"][0]["video"], "https://www.youtube.com/watch?v=HgqxiWQKCqc")
        self.assertNotIn("video", record["scheduled"][0])
        # Anything that is not a YouTube video is left out, and said.
        for elsewhere in ("https://example.com/watch?v=an7y2IlGhU0", "javascript:alert(1)",
                          "http://www.youtube.com/watch?v=an7y2IlGhU0", "https://www.youtube.com/@civvis", 7):
            self.videos = {"1": elsewhere}
            said = io.StringIO()
            with contextlib.redirect_stderr(said):
                self.assertEqual(self.record()["finished"][0]["video"], "")
            self.assertIn("Tournament 1's video is not a YouTube link", said.getvalue())
        self.videos = ["not", "a", "map"]
        self.assertEqual(self.record()["finished"][0]["video"], "")
        # The page links it with the tournament's games, in a tab of its
        # own, and checks the address again before it does.
        for piece in (
            'const link = el("a", "tour-video", "Result video");',
            r'if (!/^https:\/\/www\.youtube\.com\/watch\?v=[\w-]{11}$/.test(t.video || "")) return null;',
            'link.target = "_blank";',
            "if (video(t)) cell.append(video(t));",
            "if (video(t)) about.append(video(t));",
        ):
            self.assertIn(piece, PAGE)
        # The definitions name the three videos the channel has, by number,
        # and the record made from them carries each on its own tournament.
        defined = json.loads((ROOT / "data" / "tournaments.json").read_text(encoding="utf-8"))
        self.assertEqual(defined["videos"], {
            "1": "https://www.youtube.com/watch?v=an7y2IlGhU0",
            "3": "https://www.youtube.com/watch?v=HgqxiWQKCqc",
            "4": "https://www.youtube.com/watch?v=4pQAMeV5W2Q",
        })
        for link in defined["videos"].values():
            self.assertTrue(tournaments_page.VIDEO.fullmatch(link), link)

    def test_a_tournament_has_a_short_description_on_its_title_line(self):
        # Operator 2026-11-21: "for each tournament, we should have a short
        # description. T1: Proof of concept, 10 game series. T2: Abandoned.
        # T3: 30 game series. T4: 50 game series T5: Repeat 50 game series
        # but with 2/6 CIVVIS AI Agents (2nd half no Diplo allowed). T6:
        # Benchmarking across levels: 5 games at each level (no Diplo
        # allowed). the descriptions can be on the title line. please make
        # title line bigger too."
        every = lambda record: {t["id"]: t["description"] for t in record["finished"] + record["running"] + record["scheduled"]}
        self.assertEqual(every(self.record()), {1: "", 2: "", 3: ""})
        # A description is listed by its tournament's number, as a video
        # is, whether the tournament has finished, is running or waits.
        self.descriptions = {"1": "Proof of concept", "2": "  Repeat it,\n over 5 games ", "3": "Benchmarking across levels"}
        self.assertEqual(every(self.record()), {1: "Proof of concept", 2: "Repeat it, over 5 games", 3: "Benchmarking across levels"})
        # A definition's own description is used for a number with no line,
        # and the line listed for a number is the one shown.
        spec = {"name": "n", "description": "From the definition", "conditions": "c", "start": "civvis-t6-", "series_size": 2, "match": MATCH}
        rows = [run("civvis-t6-20261121T100000Z", "win", "CIVILIZATION_ROME", 5)]
        own = lambda listed: tournaments_page.record(rows, [spec], ["Rome"], None, listed)["running"][0]["description"]
        self.assertEqual(own(None), "From the definition")
        self.assertEqual(own({"2": "Another tournament's"}), "From the definition")
        self.assertEqual(own({"1": "From the list"}), "From the list")
        # Anything that is not a few words is not a description.
        for odd in ({"1": ""}, {"1": "   "}, {"1": 7}, {"1": None}, ["not", "a", "map"]):
            self.assertEqual(own(odd), "From the definition")
        # The page writes it on the title line after the number, the same
        # line in all three lists, with the definition's name under it.
        for piece in (
            'line.append(el("span", "tour-no", "Tournament " + t.id + (t.description ? ":" : "")));',
            'if (t.description) line.append(" " + t.description);',
            'ident.append(titled(t, "h3"), el("p", "tour-name", t.name));',
            # The largest line a tournament has, the number in gold.
            "  .tour-title { margin: 0; color: #fff; font-size: 19px; font-weight: 700; line-height: 1.3; }\n"
            "  .tour-title .tour-no { color: var(--gold); }",
            "  .tour-queue .tour-title { font-size: 16px; }",
            # It is read whole on a phone: under it, not beside it, the
            # record.
            "  @media (max-width: 600px) {\n"
            "    .tour-head { display: block; }\n"
            "    .tour-figure { margin-top: 6px; text-align: left; }\n"
            "  }",
        ):
            self.assertIn(piece, PAGE)
        # One title line for all three lists, written by one function.
        self.assertEqual(PAGE.count("titled(t, "), 1)
        self.assertEqual(PAGE.count("identity(t, "), 3)   # running, scheduled and finished
        # Bigger than the lines it replaced: the 13px number in the table
        # and the 11.5px number over a 17px name in the two lists.
        for gone in ("tour-number", 'el("b", "", "Tournament " + t.id)', "td.is-name b"):
            self.assertNotIn(gone, PAGE)
        # The definitions carry the operator's descriptions, word for word,
        # and no tournament from the first to the last described is left
        # out. The benchmark, which was the seventh ("Indefinite
        # Benchmarking Tourney", 2026-11-23), is not a tournament since
        # 2026-11-25 and carries its own on its definition. Tournament 7 is
        # the one the operator scheduled on 2026-11-26.
        defined = json.loads((ROOT / "data" / "tournaments.json").read_text(encoding="utf-8"))
        self.assertEqual(defined["descriptions"], {
            "1": "Proof of concept, 10 game series",
            "2": "Abandoned",
            "3": "30 game series",
            "4": "50 game series",
            "5": "Repeat 50 game series but with 2/6 CIVVIS AI Agents (2nd half no Diplo allowed)",
            "6": "Benchmarking across levels: 5 games at each level (no Diplo allowed)",
            "7": "100 Lv 8 Deity games, no Diplo allowed",
        })
        self.assertEqual(list(defined["descriptions"]), [str(n) for n in range(1, len(defined["descriptions"]) + 1)])
        tournaments = [spec for spec in defined["tournaments"] if not spec.get("benchmark")]
        self.assertGreaterEqual(len(defined["descriptions"]), len(tournaments))

    def test_every_tournament_spells_out_its_hyperparameters(self):
        # Operator 2026-11-23: "please spell out the tourney hyperparameters
        # for each tourney clearly."
        every = lambda record: {t["id"]: t["hyperparameters"] for t in record["finished"] + record["running"] + record["scheduled"]}
        self.assertEqual(every(self.record()), {1: [], 2: [], 3: []})
        # Listed by the tournament's number, as a description is, and put in
        # the one order every tournament shares, whatever the file's order.
        # A name the list does not know follows, in the file's order.
        self.listed = {
            "1": {"Turn limit": "250", "Extra": "  last,\n  in file order ", "Games": "2", "AI level": "Lv 8 Deity"},
            "3": {"Games": "50", "Blank": "  ", "Number": 7},
        }
        self.assertEqual(every(self.record()), {
            1: [["Games", "2"], ["AI level", "Lv 8 Deity"], ["Turn limit", "250"], ["Extra", "last, in file order"]],
            2: [],
            3: [["Games", "50"]],   # scheduled, and anything not a few words left out
        })
        # A definition's own are used for a number with no line.
        spec = {"name": "n", "hyperparameters": {"Games": "From the definition"}, "conditions": "c",
                "start": "civvis-t6-", "series_size": 2, "match": MATCH}
        rows = [run("civvis-t6-20261121T100000Z", "win", "CIVILIZATION_ROME", 5)]
        own = lambda given: tournaments_page.record(rows, [spec], ["Rome"], None, None, given)["running"][0]["hyperparameters"]
        self.assertEqual(own(None), [["Games", "From the definition"]])
        self.assertEqual(own({"2": {"Games": "Another tournament's"}}), [["Games", "From the definition"]])
        self.assertEqual(own({"1": {"Games": "From the list"}}), [["Games", "From the list"]])
        for odd in ({"1": {}}, {"1": "Games: 2"}, {"1": {"Games": ""}}, ["not", "a", "map"]):
            self.assertEqual(own(odd), [["Games", "From the definition"]])
        # The definitions file spells out all thirteen for every tournament
        # begun, in the shared order, each a few words.
        defined = json.loads((ROOT / "data" / "tournaments.json").read_text(encoding="utf-8"))
        self.assertEqual(tournaments_page.HYPERPARAMETERS, (
            "Games", "AI level", "Players", "Victory types", "Civ", "Strategy", "Map", "Game speed",
            "Turn limit", "Ruleset", "Played in", "Game end", "CIVVIS agent",
        ))
        self.assertEqual(list(defined["hyperparameters"]), [str(n) for n in range(1, len(defined["hyperparameters"]) + 1)])
        tournaments = [spec for spec in defined["tournaments"] if not spec.get("benchmark")]
        self.assertGreaterEqual(len(defined["hyperparameters"]), len(tournaments) + 1)   # 4 and 5 share one
        for number, given in defined["hyperparameters"].items():
            self.assertEqual(tuple(given), tournaments_page.HYPERPARAMETERS, number)
            self.assertEqual(tournaments_page.hyperparameters(defined["hyperparameters"], int(number), {}),
                             [[name, value] for name, value in given.items()], number)
        # The benchmark's are on its definition, all thirteen.
        (bench,) = [spec for spec in defined["tournaments"] if spec.get("benchmark")]
        self.assertEqual(tuple(bench["hyperparameters"]), tournaments_page.HYPERPARAMETERS)
        # What the ledger shows, spelled out where it differs from the rest.
        hp = {**defined["hyperparameters"], "benchmark": bench["hyperparameters"]}
        self.assertEqual({n: hp[n]["Map"] for n in hp}, {n: "Continents, Small, 9 city-states, a new map each game" for n in hp})
        self.assertEqual({hp[n]["Game speed"] for n in hp} | {hp[n]["Turn limit"] for n in hp}, {"Online", "250"})
        self.assertIn("all but Diplomatic", hp["5"]["Victory types"])
        self.assertTrue(hp["6"]["Victory types"].startswith("All but Diplomatic"))
        self.assertIn("hotseat for games 1-7", hp["6"]["Played in"])
        self.assertIn("Lv 8 Deity 1 game in 2", hp["benchmark"]["AI level"])
        # The page draws them in all three lists, a name and its value to a
        # line, as text.
        for piece in (
            "const hyper = (t) => {",
            'div.append(el("dt", "", name), el("dd", "", value));',
            'box.append(el("p", "tour-latest-title", "Hyperparameters"), dl);',
            "  .tour-hyper { display: grid; grid-template-columns: max-content minmax(0, 1fr);",
            "  .tour-hyper > div { display: contents; }",
            "Hyperparameters are the settings every game of a tournament was played under.",
        ):
            self.assertIn(piece, PAGE)
        self.assertEqual(PAGE.count("const settings = hyper(t);"), 3)   # a series being played, scheduled and finished

    def test_a_tournament_is_its_name_on_the_left_third_and_the_rest_on_the_right(self):
        # Operator 2026-11-22: "lets have The tournaments structure to be
        # tournament number, name, and description on the left third. and
        # everything else moved to the right two thirds."
        for piece in (
            # The left third: the title line (number and description) and
            # the definition's name, the same in all three lists.
            '    const identity = (t, tag) => {\n'
            '      const ident = el(tag, "tour-ident");\n'
            '      ident.append(titled(t, "h3"), el("p", "tour-name", t.name));\n'
            '      return ident;\n'
            '    };',
            # Running and Scheduled: two parts side by side once the list
            # is wide enough, the third measured as the table's is, so the
            # parts line up from one list to the next. The list is the
            # container, so Scheduled in its own column stands them one
            # under the other.
            "container: tour / inline-size;",
            "  @container tour (min-width: 640px) {\n"
            "    .tour-split { grid-template-columns: 33.333% minmax(0, 1fr); }\n"
            "    .tour-split > .tour-ident { padding-right: 24px; }\n"
            "  }",
            'const card = el("article", "tour-card tour-split");',
            "card.append(identity(t, \"div\"), body);",
            'const item = el("li", "tour-split");',
            "item.append(identity(t, \"div\"), body);",
            # Finished: the left third is the cell that runs down beside a
            # tournament's figures and its games, and everything else, the
            # notes too, is to its right.
            '<th scope="col" class="is-ident">Tournament</th>',
            "  .rank-table .is-ident { width: 33.333%; padding-right: 24px; white-space: normal; }",
            "const ident = identity(t, \"td\");",
            "ident.rowSpan = 2;",
            "cell.colSpan = columns - 1;",
            'if (settings) cell.append(settings);\n'
            '        cell.append(strip(t, false), counts(t, [t.idea, t.note].filter(Boolean).join(" ")));',
            'if (note) line("Notes", el("dd", "is-note", note));',
            "body.append(tr, games);",
            # Too narrow for that, each figure stands under its own label.
            "  @container tour (max-width: 899px) {",
            "    .rank-table td[data-label]::before { content: attr(data-label); display: block;",
            "td.dataset.label = label;",
        ):
            self.assertIn(piece, PAGE)
        # The title row across the table and the Notes column are gone.
        for gone in ('el("tr", "is-title")', "tr.is-title", '<th scope="col">Notes</th>', '"is-note", [t.idea'):
            self.assertNotIn(gone, PAGE)

    def test_a_row_of_games_says_where_it_starts_and_ends(self):
        # Operator 2026-11-24: "in tourneys we should label the first game
        # and last game in the chart. just like a first and last label to
        # make the flow clear". `Drawn` below reads the labels off the page
        # as a browser draws it; these are the page's own lines, held on a
        # machine with no browser too.
        for piece in (
            # A row that starts later than the tournament's first game (an
            # indefinite one's latest 100) says which game it starts at.
            '        ended(squares[0], "first", skipped ? "Game " + (skipped + 1) : "First");\n'
            '        ended(squares[squares.length - 1], "last", "Last");',
            'square.append(el("span", "tour-end is-" + side, text));',
            # A label stands in its own square's margin, so a wrapped row
            # never parts the two.
            "  .tour-game.is-first { margin-left: var(--lead, 28px); }",
            "  .tour-game.is-last { margin-right: var(--trail, 28px); }",
            "  .tour-end.is-first { left: calc(-1 * var(--lead, 28px)); }",
            # The first square's margin is whole squares, 14px each.
            '(first ? Math.ceil((wide + 4) / 14) * 14 : Math.ceil(wide + 5)) + "px");',
        ):
            self.assertIn(piece, PAGE)
        # Both lists that draw a row of games draw it through `strip`.
        self.assertEqual(PAGE.count("strip(t, "), 2)

    def test_the_page_draws_three_lists_and_trusts_nothing_as_markup(self):
        for piece in (
            '<section class="tour-section" id="tour-running" hidden aria-labelledby="tour-running-title">',
            '<section class="tour-section" id="tour-scheduled" aria-labelledby="tour-scheduled-title">',
            '<section class="tour-section" id="tour-finished" hidden aria-labelledby="tour-finished-title">',
            # The table that stood on the home page, with the win rate added
            # and, after the two dates, the two durations.
            '<th scope="col" class="is-ident">Tournament</th><th scope="col">Start</th><th scope="col">End</th>'
            '<th scope="col" title="From the first game starting to the last game ending">Wall time</th>'
            '<th scope="col" title="The time the games were in play, added up">Run time</th>'
            '<th scope="col">Games</th>'
            '<th scope="col">Record</th><th scope="col">Won</th><th scope="col">Conditions</th></tr>',
            'figure("Wall time", t.wall || ""),\n          figure("Run time", t.run || ""),',
            't.wall ? "wall time " + t.wall : "", t.run ? "run time " + t.run : ""',
            # What the bare dates and the two durations mean, said once.
            "Dates are UTC, on the 24-hour clock.",
            "Wall time is from a tournament&#8217;s first game starting to its last game ending.",
            "Run time is the time its games were in play, added up, without the time between them.",
            # The tournaments are the left 60%, and what is scheduled and
            # the benchmark the right 40% (operator 2026-11-25: "put the
            # experiments list on the left 60%. the scheduled games &
            # benchmark can be on the right"); narrower, one column.
            "  @media (min-width: 1100px) {\n"
            "    .tour-record { grid-template-columns: minmax(0, 3fr) minmax(0, 2fr); align-items: start; }\n"
            "    .tour-record > :not(.tour-main):not(.tour-side) { grid-column: 1 / -1; }\n"
            "  }",
            "const running = list(data.running);",
            "const scheduled = list(data.scheduled);",
            "const finished = list(data.finished);",
            # Scheduling is adding a definition, and the page says where.
            'href="https://github.com/MartinHalvorson/CIVVIS/blob/main/data/tournaments.json"',
            'href="https://github.com/MartinHalvorson/CIVVIS/blob/ledger/results.jsonl"',
            # The publish check finds the page by its slot (`beta/verify.py`).
            'id="tournaments-data"',
        ):
            self.assertIn(piece, PAGE)
        for unsafe in ("innerHTML", "insertAdjacentHTML", "document.write"):
            self.assertNotIn(unsafe, PAGE)
        verify = (BETA / "verify.py").read_text(encoding="utf-8")
        self.assertIn("""    "tournaments": b'id="tournaments-data"',\n""", verify)
        self.assertIn("""    "auto-tourneys": "tournaments",\n""", verify)
        self.assertIn("""    "experiments": "tournaments",\n""", verify)


#: Read off the page as it is drawn: every row of game squares, with where
#: its squares and its two labels stand, measured from the row's own left
#: edge. `step` is how far two squares in one line are apart.
ROWS = """(() => [...document.querySelectorAll(".tour-strip")].map((row) => {
  const box = row.getBoundingClientRect();
  const squares = [...row.children].map((square) => square.getBoundingClientRect());
  const label = (side) => {
    const node = row.querySelector(".tour-end.is-" + side);
    if (!node) return null;
    const at = node.getBoundingClientRect(), square = node.parentNode;
    return { text: node.textContent, left: at.left - box.left, right: at.right - box.left, top: at.top, bottom: at.bottom,
      square: [...row.children].indexOf(square), open: square.classList.contains("is-open") };
  };
  return {
    title: row.closest(".tour-card, tbody") ? (row.closest(".tour-card") || row.closest("tr").previousElementSibling).querySelector(".tour-name").textContent : "",
    width: box.width,
    squares: squares.map((at) => ({ left: at.left - box.left, right: at.right - box.left, top: at.top, bottom: at.bottom })),
    lines: new Set(squares.map((at) => Math.round(at.top))).size,
    first: label("first"), last: label("last"),
    labels: row.querySelectorAll(".tour-end").length,
  };
}))()"""


class Drawn(unittest.TestCase):
    """The row of game squares, drawn in a browser from a filled page."""

    @classmethod
    def setUpClass(cls):
        cls.stage = pathlib.Path(tempfile.mkdtemp(prefix="civvis-tournaments-"))
        cls.profile = fresh_profile("civvis-tournaments-profile-")
        cls.chrome = None
        # A series of each kind the page draws a row for. The benchmark, in
        # the right-hand column: indefinite, with more games than its row
        # draws. Running: a tournament of a fixed size with games still to
        # play. Finished: one played out, and one that ended after a single
        # game.
        rows = [run(f"civvis-t7-2026112{1 + i // 100}T{i % 100 // 4:02d}{i % 4 * 15:02d}00Z",
                    "win" if i % 3 == 0 else "rival_victory", "CIVILIZATION_ROME", 0,
                    utc=f"2026-11-2{1 + i // 100}T{i % 100 // 4:02d}:{i % 4 * 15 + 10:02d}:00Z") for i in range(240)]
        bench = tournaments_page.record(rows, [{"name": "Bench", "benchmark": True, "conditions": "c", "start": "civvis-t7-",
                                                "end": "civvis-t8-", "indefinite": True, "match": MATCH}], ["Rome"])
        record = TournamentsPage().record()
        single = copy.deepcopy(record["finished"][0])
        single.update(name="One game", results=single["results"][:1], games=1, wins=1, losses=0)
        record["benchmark"] = bench["benchmark"]
        record["finished"] = record["finished"] + [single]
        cls.record = record
        page = cls.stage / "index.html"
        page.write_text(elo_page.fill(tournaments_page.fill(PAGE, record), elo_record()), encoding="utf-8")
        cls.address = page.as_uri()
        chrome = os.environ.get("CIVVIS_TEST_CHROME") or find_chrome()
        if not pathlib.Path(chrome).exists() and not shutil.which(chrome):
            return
        port = free_port()
        cls.chrome = subprocess.Popen(
            [chrome, "--headless=new", f"--remote-debugging-port={port}", f"--user-data-dir={cls.profile}",
             "--no-first-run", "--disable-gpu", "--window-size=1500,1200", "about:blank"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        tether(cls.chrome)
        target, deadline = None, time.time() + 60
        while time.time() < deadline and target is None:
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=2) as answer:
                    pages = [page for page in json.load(answer) if page.get("type") == "page"]
                target = next((page for page in pages if page.get("webSocketDebuggerUrl")), None)
            except OSError:
                time.sleep(0.2)
        if target is None:
            cls.chrome.terminate()
            cls.chrome = None
            raise AssertionError("Chrome never offered a debuggable page within 60s")
        cls.dev = Devtools(target["webSocketDebuggerUrl"])
        cls.dev.call("Page.enable")
        cls.dev.call("Runtime.enable")

    @classmethod
    def tearDownClass(cls):
        if cls.chrome:
            cls.chrome.terminate()
            try:
                cls.chrome.wait(timeout=5)
            except subprocess.TimeoutExpired:
                cls.chrome.kill()
        shutil.rmtree(cls.stage, ignore_errors=True)
        shutil.rmtree(cls.profile, ignore_errors=True)

    def rows(self, width: int) -> dict:
        """The page opened in a window `width` wide: its rows, by the name
        of the tournament each is of."""
        if not self.chrome:
            self.skipTest("no Chrome on this machine to open the page in")
        self.dev.call("Emulation.setDeviceMetricsOverride",
                      {"width": width, "height": 1200, "deviceScaleFactor": 1, "mobile": False})
        drawn = "document.readyState === 'complete' && location.href === %s && !!document.querySelector('.tour-end')" % json.dumps(self.address)
        for attempt in range(1, 5):
            self.dev.call("Page.navigate", {"url": self.address})
            deadline = time.time() + 15
            while time.time() < deadline:
                try:
                    if self.dev.evaluate(drawn):
                        return {row["title"]: row for row in self.dev.evaluate(ROWS)}
                except Exception:  # noqa: BLE001 - the page may be between two documents
                    pass
                time.sleep(0.1)
            print(f"attempt {attempt}: the page had not drawn its rows after 15s; opening it again.")
        self.fail(f"the page never drew its rows ({self.dev.console[-5:]})")

    def test_elo_rankings_ranks_everyone_together_across_the_page_a_column_a_setup(self):
        # Two tables, a row for each player across both (operator
        # 2026-11-27: "two tables: one on the left for computer control
        # firaxis civ 6 games and one on the right for CIVVIS self-play. 2
        # separate tables but scrolling horizontally in one should move the
        # other as well. rows should cross over all the way. any col in
        # either table should sort all rows in both tables").
        self.rows(1500)
        read = """(() => { const card = document.getElementById('tour-elo');
          const side = name => card.querySelector('[data-elo-side="' + name + '"]');
          const rows = name => [...side(name).querySelectorAll('tbody tr')].map(row => [...row.children].map(cell => cell.firstChild ? cell.firstChild.textContent : ''));
          const tops = name => [...side(name).querySelectorAll('tbody tr')].map(row => { const r = row.getBoundingClientRect(); return [Math.round(r.top), Math.round(r.height)]; });
          const kinds = [...card.querySelectorAll('tbody tr')].map(row => [row.className.replace(' is-hover', ''), getComputedStyle(row).backgroundColor]);
          const box = id => document.getElementById(id).getBoundingClientRect();
          const record = box('tour-record'), elo = box('tour-elo');
          return {hidden: card.hidden, left: rows('control'), right: rows('self'), lefts: tops('control'), rights: tops('self'),
            heads: ['control', 'self'].map(name => Math.round(side(name).querySelector('thead').getBoundingClientRect().height)),
            kinds, pressed: [...card.querySelectorAll('.rank-sort')].filter(b => b.getAttribute('aria-pressed') === 'true').map(b => b.dataset.sort),
            top: [elo.bottom <= box('tour-running').top || box('tour-running').height === 0, elo.bottom <= box('tour-scheduled').top, Math.abs(elo.width - record.width) <= 2],
            note: card.querySelector('[data-rank-note]').textContent}; })()"""
        names = lambda drawn: [row[1] for row in drawn["left"]]
        drawn = self.dev.evaluate(read)
        self.assertFalse(drawn["hidden"])
        self.assertEqual(drawn["top"], [True, True, True])
        # It opens on the one-scale ranking by Diplo off, both Diplo off
        # heads pressed: every row numbered, wherever its figure was measured.
        self.assertEqual(drawn["pressed"], ["control:continents6_no_diplo", "self:continents6_no_diplo"])
        # Left, Civilization VI: the Firaxis levels and the person; right,
        # CIVVIS self-play: the agents, a column a setup. Diplo off and Diplo
        # on read the four-player fits until the six-player ones are
        # published; a column without a figure is a dash.
        dash = ["–"] * 5
        self.assertEqual(drawn["left"], [
            ["1", "HolyRoller", "–", "–"],
            ["2", "Lv 8 Deity", "1452", "1042"],
            ["3", "JackOfAllTrades", "–", "–"],
            ["4", "dev", "1100", "1210"],
            ["5", "Lv 5 King", "799", "595"],
        ])
        self.assertEqual(drawn["right"], [
            ["1543", "1547", "1300", *dash, "–", "1200"],
            ["–"] * 10,
            ["1191", "1155", "1400", *dash, "–", "–"],
            ["–"] * 10,
            ["–"] * 10,
        ])
        # A row runs across: each row of the left stands level with its row
        # on the right, and the two heads are one height.
        self.assertEqual(drawn["lefts"], drawn["rights"])
        self.assertEqual(drawn["heads"][0], drawn["heads"][1])
        # Each kind faintly its own colour, and the three apart.
        kinds = {cls: ink for cls, ink in drawn["kinds"]}
        self.assertEqual(sorted(kinds), ["is-agent", "is-level", "is-person"])
        self.assertEqual(len(set(kinds.values())), 3, kinds)
        self.assertTrue(all(ink not in ("rgba(0, 0, 0, 0)", "transparent") for ink in kinds.values()), kinds)
        self.assertIn("Diplo off and Diplo on read the four-player Pangaea tables until the six-player Continents fit is published.", drawn["note"])
        self.assertIn("Science 120", drawn["note"])
        # Any head of either table sorts both: Science (right) puts those
        # rated there first and the rest in the Diplo off ranking.
        self.dev.evaluate("document.querySelector('#tour-elo [data-sort=\"self:continents6_science\"]').click(), true")
        drawn = self.dev.evaluate(read)
        self.assertEqual(drawn["pressed"], ["self:continents6_science"])
        self.assertEqual(names(drawn), ["JackOfAllTrades", "HolyRoller", "Lv 8 Deity", "dev", "Lv 5 King"])
        self.assertEqual(drawn["right"][0][2], "1400")
        # Civilization VI's Diplo on (left) sorts both by it.
        self.dev.evaluate("document.querySelector('#tour-elo [data-sort=\"control:continents6\"]').click(), true")
        drawn = self.dev.evaluate(read)
        self.assertEqual(names(drawn), ["dev", "Lv 8 Deity", "Lv 5 King", "HolyRoller", "JackOfAllTrades"])
        self.assertEqual(drawn["right"][3][1], "1547")
        self.assertEqual(drawn["lefts"], drawn["rights"])
        # Pointing at a row in one table lights it in the other.
        lit = self.dev.evaluate("""(() => { const card = document.getElementById('tour-elo');
          card.querySelectorAll('[data-elo-side="control"] tbody tr')[1].children[1].dispatchEvent(new MouseEvent('mouseover', {bubbles: true}));
          return [...card.querySelectorAll('[data-elo-side="self"] tbody tr')].map(row => row.classList.contains('is-hover')); })()""")
        self.assertEqual(lit, [False, True, False, False, False])
        self.assertEqual(self.dev.console, [])

    def test_the_two_elo_tables_scroll_sideways_together(self):
        # On a phone both tables are wider than their room: scrolling one
        # sideways scrolls the other the same share of its width.
        self.rows(390)
        moved = self.dev.evaluate("""new Promise(done => {
          const card = document.getElementById('tour-elo');
          const [left, right] = ['control', 'self'].map(name => card.querySelector('[data-elo-side="' + name + '"]'));
          const span = pane => pane.scrollWidth - pane.clientWidth;
          right.scrollLeft = span(right);
          setTimeout(() => {
            const after = [span(left), span(right), left.scrollLeft, right.scrollLeft];
            left.scrollLeft = 0;
            setTimeout(() => done([...after, left.scrollLeft, right.scrollLeft]), 300);
          }, 300);
        })""")
        span_left, span_right, left_at, right_at, left_back, right_back = moved
        self.assertGreater(span_left, 0, moved)
        self.assertGreater(span_right, 0, moved)
        self.assertEqual(right_at, span_right, moved)
        self.assertLessEqual(abs(left_at - span_left), 1, moved)
        self.assertEqual((left_back, right_back), (0, 0), moved)
        self.assertEqual(self.dev.console, [])

    def test_a_row_of_games_is_labelled_first_and_last(self):
        # Operator 2026-11-24: "in tourneys we should label the first game
        # and last game in the chart. just like a first and last label to
        # make the flow clear".
        rows = self.rows(1500)
        self.assertEqual(sorted(rows), ["Bench", "First idea", "One game", "Second idea"])
        for name, row in rows.items():
            self.assertEqual(row["labels"], 2, name)
            self.assertEqual((row["first"]["square"], row["last"]["square"]), (0, len(row["squares"]) - 1), name)
            self.assertEqual(row["last"]["text"], "Last", name)
        # A tournament played out: its first game and its last.
        self.assertEqual((rows["First idea"]["first"]["text"], len(rows["First idea"]["squares"])), ("First", 2))
        # One still being played: "Last" is on its last game, not yet played.
        running = rows["Second idea"]
        self.assertEqual((running["first"]["text"], len(running["squares"])), ("First", 5))
        self.assertEqual((running["first"]["open"], running["last"]["open"]), (False, True))
        # One game is both.
        self.assertEqual((rows["One game"]["first"]["text"], rows["One game"]["first"]["square"],
                          rows["One game"]["last"]["square"]), ("First", 0, 0))
        # An indefinite tournament's row is its latest 100 games, so it
        # does not start at the first: it says which game it starts at.
        self.assertEqual(self.record["benchmark"]["games"], 240)
        self.assertEqual((rows["Bench"]["first"]["text"], len(rows["Bench"]["squares"])), ("Game 141", 100))

    def test_a_label_has_its_room_in_the_row_and_the_squares_keep_their_columns(self):
        for width in (1500, 700, 420):
            rows = self.rows(width)
            for name, row in rows.items():
                where = f"{name} at {width}px"
                squares, first, last = row["squares"], row["first"], row["last"]
                start, end = squares[0], squares[-1]
                # Each label is in its row, on its square's line, clear of
                # the square it names by at least the gap between two.
                self.assertGreaterEqual(first["left"], -0.01, where)
                self.assertLessEqual(first["right"], start["left"] - 3, where)
                self.assertGreaterEqual(last["left"], end["right"] + 3, where)
                self.assertLessEqual(last["right"], row["width"] + 0.01, where)
                for label, square in ((first, start), (last, end)):
                    self.assertLess(abs((label["top"] + label["bottom"]) - (square["top"] + square["bottom"])) / 2, 1.5, where)
                # The first label starts the row, and takes no more room
                # than the whole squares it needs.
                self.assertLess(start["left"] - first["right"], 14 + 4, where)
                # Every square is in a column of the same grid, 14px apart,
                # in whichever line of a wrapped row it falls.
                for square in squares:
                    self.assertAlmostEqual(square["left"] / 14, round(square["left"] / 14), places=2, msg=where)
                # No square is under a label.
                for label in (first, last):
                    for square in squares:
                        apart = (square["right"] <= label["left"] or square["left"] >= label["right"]
                                 or square["bottom"] <= label["top"] or square["top"] >= label["bottom"])
                        self.assertTrue(apart, where)
            # The long row is wrapped at every one of these widths.
            self.assertGreater(rows["Bench"]["lines"], 1, width)


if __name__ == "__main__":
    unittest.main()
