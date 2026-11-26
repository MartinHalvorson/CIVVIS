#!/usr/bin/env python3
"""The Research page: when each victory lands, counted off both kinds of game."""

from __future__ import annotations

import contextlib
import datetime
import io
import json
import os
import pathlib
import re
import shutil
import subprocess
import tempfile
import unittest

import civs_page
import home_rankings
import research_page

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "research.html").read_text(encoding="utf-8")
#: Civ Analytics, where the tree of build orders stands since 2026-11-23.
CIVS = (BETA / "civs.html").read_text(encoding="utf-8")
#: ELO Rankings, the page after Win Analytics under Charts/.
ELO = (BETA / "elo.html").read_text(encoding="utf-8")
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")
TOURNAMENTS = (BETA / "tournaments.html").read_text(encoding="utf-8")
VIEWER = (ROOT / "web" / "index.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")


def run(tag: str, outcome: str, turns: int, victory: str | None = None, **more) -> dict:
    """One line of the results file, as `civ6_ladder.result_row` writes it."""
    return {
        "tag": tag,
        "game": re.sub(r"-cont\d+$", "", tag),
        "utc": "2026-11-21T10:00:00Z",
        "civ": "CIVILIZATION_ROME",
        "difficulty": "DIFFICULTY_DEITY",
        "players": 6,
        "map": "Continents.lua",
        "outcome": outcome,
        "victory": victory,
        "turns": turns,
        **more,
    }


FULL = ("GAMESPEED_ONLINE", 250)


class GamesAgainstTheAI(unittest.TestCase):
    def test_a_game_counts_once_by_its_victory_and_turn(self):
        rows = [
            run("civvis-20261120T100000Z", "rival_victory", 185, "VICTORY_TECHNOLOGY"),
            run("civvis-20261120T110000Z", "win", 173, "VICTORY_DIPLOMATIC"),
            # A reloaded game: two runs, one game, ended the best way any run did.
            run("civvis-20261121T090000Z", "rival_victory", 160, "VICTORY_CULTURE"),
            run("civvis-20261121T090000Z-cont1", "win", 171, "VICTORY_RELIGIOUS"),
            # No victory, so no type and no turn: left out.
            run("civvis-20261121T100000Z", "defeat", 120),
            run("civvis-20261121T110000Z", "abandoned", 90),
            run("civvis-20261121T120000Z", "unfinished", 40),
            # Another table: left out.
            run("civvis-20261121T130000Z", "win", 150, "VICTORY_CULTURE", players=4, map="Pangaea.lua"),
            # Another level: counted, under its own level.
            run("civvis-20261121T140000Z", "win", 150, "VICTORY_CULTURE", difficulty="DIFFICULTY_PRINCE"),
            run("civvis-20261121T150000Z", "rival_victory", 201, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_PRINCE"),
            # Two CIVVIS agents at one table (the row names both): either
            # one winning is a win there, so it is no level's game.
            run("civvis-t6-20261121T131243Z", "win", 224, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_PRINCE",
                civ="CIVILIZATION_GREECE+CIVILIZATION_VIETNAM"),
            # A level under the page's lowest (operator 2026-11-23: "ai level
            # in charts can go from lv 4 to lv 8"): left out.
            run("civvis-20261121T153000Z", "win", 150, "VICTORY_CULTURE", difficulty="DIFFICULTY_WARLORD"),
            # A level the game does not have: left out.
            run("civvis-20261121T160000Z", "win", 150, "VICTORY_CULTURE", difficulty="DIFFICULTY_NIGHTMARE"),
            run("civvis-20261121T170000Z", "win", 150, "VICTORY_CULTURE", difficulty=None),
        ]
        levels = research_page.level_groups(rows, lambda tags: {tag: FULL for tag in tags})
        # A level per level played, lowest first, and no other.
        self.assertEqual(list(levels), [4, 8])
        self.assertEqual(len(levels[4]), 1)
        self.assertEqual(levels[4][0]["games"], 2)
        self.assertEqual(levels[4][0]["turns"], {"culture": {150: 1}, "science": {201: 1}})
        self.assertEqual(levels[4][0]["won"], {"culture": {150: 1}})
        groups = levels[8]
        self.assertEqual([g["day"] for g in groups], ["2026-11-20", "2026-11-21"])
        self.assertEqual(groups[0]["games"], 2)
        self.assertEqual(groups[0]["turns"], {"science": {185: 1}, "diplomatic": {173: 1}})
        self.assertEqual(groups[0]["won"], {"diplomatic": {173: 1}})
        self.assertEqual(groups[1]["games"], 1)
        self.assertEqual(groups[1]["turns"], {"religious": {171: 1}})

    def test_a_game_played_short_of_250_turns_is_left_out(self):
        # The Score series of 2026-11-12 to 15 ran to 170, 160 and 120 turns:
        # their Score victories at the limit would read as a spike that the
        # 250-turn game never has. A run with no summary says nothing of its
        # limit and is left out too; a longer limit is the same game to 250.
        rows = [
            run("civvis-20261113T100000Z", "rival_victory", 170, "VICTORY_SCORE"),
            run("civvis-20261113T110000Z", "rival_victory", 150, "VICTORY_TECHNOLOGY"),
            run("civvis-20261113T120000Z", "rival_victory", 190, "VICTORY_TECHNOLOGY"),
            run("civvis-20261113T130000Z", "rival_victory", 200, "VICTORY_TECHNOLOGY"),
            run("civvis-20261113T140000Z", "rival_victory", 210, "VICTORY_TECHNOLOGY"),
        ]
        known = {
            "civvis-20261113T100000Z": ("GAMESPEED_ONLINE", 170),
            "civvis-20261113T110000Z": ("GAMESPEED_STANDARD", 500),
            "civvis-20261113T130000Z": ("GAMESPEED_ONLINE", 500),
            "civvis-20261113T140000Z": FULL,
        }
        asked = []
        groups = research_page.level_groups(rows, lambda tags: asked.extend(tags) or known)[8]
        self.assertEqual(groups[0]["turns"], {"science": {200: 1, 210: 1}})
        # One lookup for every game that ended in a victory.
        self.assertEqual(sorted(asked), sorted(row["tag"] for row in rows))

    def test_the_turn_limits_come_from_the_run_summaries_on_the_ledger(self):
        with tempfile.TemporaryDirectory() as scratch:
            repo = pathlib.Path(scratch)
            git = lambda *args: subprocess.run(["git", "-C", scratch, *args], check=True, capture_output=True, text=True)
            git("init", "-q")
            for tag, summary in (
                ("civvis-a", {"speed": "GAMESPEED_ONLINE", "max_turns": 250, "outcome": {"turn": 190}}),
                ("civvis-b-cont1", {"speed": "GAMESPEED_ONLINE", "max_turns": 170}),
            ):
                (repo / "runs" / tag).mkdir(parents=True)
                (repo / "runs" / tag / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
            (repo / "runs" / "civvis-c").mkdir()
            (repo / "runs" / "civvis-c" / "summary.json").write_text("not json", encoding="utf-8")
            git("add", "-A")
            git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "ledger")
            git("update-ref", "refs/remotes/origin/ledger", "HEAD")
            found = research_page.limits(repo, ["civvis-a", "civvis-missing", "civvis-b-cont1", "civvis-c"])
            self.assertEqual(found, {"civvis-a": FULL, "civvis-b-cont1": ("GAMESPEED_ONLINE", 170)})
            self.assertEqual(research_page.limits(repo / "nowhere", ["civvis-a"]), {})

    def ledger(self, scratch: str, rows: list[dict], more: dict | None = None) -> pathlib.Path:
        """A clone whose ledger holds these result lines, every run played to
        the full length. `more` is what else a run's summary says, by tag."""
        repo = pathlib.Path(scratch)
        git = lambda *args: subprocess.run(["git", "-C", scratch, *args], check=True, capture_output=True, text=True)
        git("init", "-q")
        (repo / "results.jsonl").write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
        for row in rows:
            (repo / "runs" / row["tag"]).mkdir(parents=True)
            summary = {"speed": "GAMESPEED_ONLINE", "max_turns": 250, **(more or {}).get(row["tag"], {})}
            (repo / "runs" / row["tag"] / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
        git("add", "-A")
        git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "ledger")
        git("update-ref", "refs/remotes/origin/ledger", "HEAD")
        return repo

    def test_the_record_holds_a_kind_per_level_and_opens_on_deity(self):
        # Operator 2026-11-21: "an option to select by level too. so i could
        # for example go to level 4 and see the win rate of civvis against
        # level 4. broken down by victor type and timeline."
        rows = [
            run("civvis-20261120T100000Z", "rival_victory", 185, "VICTORY_TECHNOLOGY"),
            run("civvis-20261120T110000Z", "win", 173, "VICTORY_DIPLOMATIC"),
            run("civvis-t6-20261121T210000Z", "win", 224, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_PRINCE"),
            run("civvis-t6-20261122T010000Z", "win", 230, "VICTORY_CULTURE", difficulty="DIFFICULTY_PRINCE"),
            run("civvis-t6-20261122T050000Z", "rival_victory", 241, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_PRINCE"),
            run("civvis-20261101T050000Z", "rival_victory", 233, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_KING"),
        ]
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()):
            record = research_page.research(self.ledger(scratch, rows))
        # No self-play snapshot in that clone; the games against the AI stand alone.
        self.assertIsNone(record["self_play"])
        self.assertNotIn("deity", record)
        self.assertEqual(list(record["levels"]), ["4", "5", "8"])
        self.assertEqual(record["level"], 8)
        self.assertEqual(
            [kind["name"] for kind in record["levels"].values()],
            ["CIVVIS vs Lv 4 Prince AI", "CIVVIS vs Lv 5 King AI", "CIVVIS vs Lv 8 Deity AI"],
        )
        prince = record["levels"]["4"]["windows"]
        self.assertEqual(sorted(prince), ["30", "7", "all"])
        # The win rate the page prints is these two: 2 of 3.
        week = prince["7"]
        self.assertEqual((week["from"], week["to"], week["games"]), ("2026-11-21", "2026-11-22", 3))
        self.assertEqual(sum(v["won"] for v in week["victories"].values()), 2)
        self.assertEqual(week["victories"]["science"]["won_turns"][223], 1)
        # A level's window counts back from that level's own newest game, so
        # a level last played weeks ago still has a last week.
        king = record["levels"]["5"]["windows"]["7"]
        self.assertEqual((king["from"], king["to"], king["games"]), ("2026-11-01", "2026-11-01", 1))
        # The page as written opens on the level the record opens on.
        self.assertEqual(research_page.OPENING_LEVEL, 8)
        self.assertEqual(research_page.level_name(4), "Lv 4 Prince")

    def test_without_deity_games_the_record_opens_on_the_highest_level_played(self):
        rows = [
            run("civvis-20261101T050000Z", "win", 233, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_KING"),
            run("civvis-t6-20261121T210000Z", "win", 224, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_PRINCE"),
        ]
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()):
            record = research_page.research(self.ledger(scratch, rows))
        self.assertEqual((list(record["levels"]), record["level"]), (["4", "5"], 5))


class TournamentWinRates(unittest.TestCase):
    """Operator 2026-11-22: "to the top of win analytics roughly, please list
    our win rate against lv 4... lv 8 deity ai in tournament games". And
    2026-11-23: "at the top of win analytics, we should have a last 1 day win
    rate col, last 3 days, last 7 days, last 30 days. this will help us track
    win rates. order the cols lv 8 down to level 4. this will be our primary
    Firaxis Civ 6 AI benchmark - win rate on each level. half the games can
    be diplo, half no diplo. we should have a table on the left the is with
    diplo excluded and a table on the right that allows diplo wins. we can
    drop the wins, losses. games (n) should be listed after each value. so
    like 100% (5) or 14% (181). at the top, we should have win rate (total
    games)". Later that day he reversed the sides: "put diplo victory allowed
    to the left of no diplo victory"."""

    SPECS = [
        {"name": "Deity series", "start": "civvis-20261001T000000Z", "end": "civvis-t", "series_size": 2,
         "match": {"difficulty": "DIFFICULTY_DEITY", "players": 6, "map": "Continents.lua"}},
        {"name": "Level ladder", "start": "civvis-t6-20261121T204000Z", "end": "civvis-t7-", "series_size": 25,
         "match": {"players": 6, "map": "Continents.lua"}},
        {"name": "Indefinite", "start": "civvis-t7-", "end": "civvis-t8-", "indefinite": True,
         "match": {"players": 6, "map": "Continents.lua"}},
    ]
    ROWS = [
        # Before any tournament began: a verification game, not a tournament's.
        run("civvis-20260920T100000Z", "win", 200, "VICTORY_TECHNOLOGY"),
        # The Deity series: a loss on 10-20, then a game that took two runs
        # and was won by the second, which is one win.
        run("civvis-20261020T100000Z", "rival_victory", 185, "VICTORY_TECHNOLOGY"),
        run("civvis-20261117T100000Z", "abandoned", 90),
        run("civvis-20261117T100000Z-cont1", "win", 240, "VICTORY_CULTURE"),
        # An elimination and a game conceded early are losses; a launch that
        # never reached a board is not a game.
        run("civvis-20261118T100000Z", "defeat", 120),
        run("civvis-20261119T100000Z", "abandoned", 60),
        run("civvis-20261119T110000Z", "unfinished", 0),
        # A King game in the Deity series' days is not that tournament's.
        run("civvis-20261119T120000Z", "win", 230, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_KING"),
        # A game whose summary says nothing of its Diplomatic victory.
        run("civvis-20261119T130000Z", "rival_victory", 199, "VICTORY_TECHNOLOGY"),
        # The ladder, by level, and a table of four players it does not count.
        run("civvis-t6-20261121T210000Z", "win", 224, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_PRINCE"),
        run("civvis-t6-20261121T220000Z", "win", 230, "VICTORY_CULTURE", difficulty="DIFFICULTY_PRINCE"),
        run("civvis-t6-20261121T230000Z", "rival_victory", 241, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_KING"),
        run("civvis-t6-20261122T010000Z", "win", 250, "VICTORY_SCORE", difficulty="DIFFICULTY_EMPEROR", players=4),
        # A level under the tables' lowest: a tournament's game, not a row.
        run("civvis-t6-20261122T020000Z", "win", 210, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_WARLORD"),
        # The indefinite benchmark.
        run("civvis-t7-20261122T030000Z", "rival_victory", 233, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_IMMORTAL"),
        run("civvis-t7-20261122T040000Z", "rival_victory", 236, "VICTORY_CULTURE"),
    ]
    #: Each run's Diplomatic victory, as its summary says it. The game of
    #: 11-17 says it only in its first run, the one of 11-19 13:00 nowhere.
    SWITCH = {
        "civvis-20260920T100000Z": True,
        "civvis-20261020T100000Z": True,
        "civvis-20261117T100000Z": True,
        "civvis-20261118T100000Z": False,
        "civvis-20261119T100000Z": True,
        "civvis-20261119T120000Z": True,
        "civvis-t6-20261121T210000Z": False,
        "civvis-t6-20261121T220000Z": False,
        "civvis-t6-20261121T230000Z": False,
        "civvis-t6-20261122T010000Z": False,
        "civvis-t6-20261122T020000Z": False,
        "civvis-t7-20261122T030000Z": False,
        "civvis-t7-20261122T040000Z": True,
    }

    def switch(self, tags: list[str]) -> dict[str, bool]:
        return {tag: self.SWITCH[tag] for tag in tags if tag in self.SWITCH}

    def games(self) -> list[tuple]:
        return research_page.tournament_games(self.ROWS, self.SPECS, self.switch)

    def test_the_games_are_the_tournaments_finished_games_by_level_and_switch(self):
        at = lambda day, hour: datetime.datetime(2026, int(day[:2]), int(day[3:]), hour)
        self.assertEqual(
            sorted(self.games()),
            [
                (at("10-20", 10), 8, False, True),
                (at("11-17", 10), 8, True, True),
                (at("11-18", 10), 8, False, False),
                (at("11-19", 10), 8, False, True),
                (at("11-21", 21), 4, True, False),
                (at("11-21", 22), 4, True, False),
                (at("11-21", 23), 5, False, False),
                (at("11-22", 3), 7, False, False),
                (at("11-22", 4), 8, False, True),
            ],
        )
        # They are the games the Tournaments page counts, level by level,
        # the same series, the same wins and losses, less the one game whose
        # summary does not say which table it stands in.
        series = home_rankings.tournament_series(self.ROWS, self.SPECS, detail=True)
        listed = [
            ending for row in series for _, ending in row["played"]
            if home_rankings.level(ending.get("difficulty")) in research_page.RATE_LEVELS
        ]
        self.assertEqual(len(listed), len(self.games()) + 1)
        self.assertEqual(sum(ending["outcome"] == "win" for ending in listed), sum(won for _, _, won, _ in self.games()))
        self.assertEqual(research_page.tournament_games(self.ROWS, [], self.switch), [])
        # The summaries are asked for once, for the run each game ended in
        # and for its first: the game of 11-17 ended in its second run.
        asked = []
        research_page.tournament_games(self.ROWS, self.SPECS, lambda tags: asked.append(tags) or {})
        self.assertEqual(len(asked), 1)
        self.assertIn("civvis-20261117T100000Z-cont1", asked[0])
        self.assertIn("civvis-20261117T100000Z", asked[0])
        self.assertEqual(len(asked[0]), len(set(asked[0])))
        # No summary read: no game in either table.
        self.assertEqual(research_page.tournament_games(self.ROWS, self.SPECS), [])

    def test_a_game_began_at_its_tags_stamp(self):
        began = research_page.started
        self.assertEqual(began("civvis-t7-20261123T042112Z", {}), datetime.datetime(2026, 11, 23, 4, 21, 12))
        # A tag with no stamp: the moment the ending was recorded.
        self.assertEqual(began("civvis-table-a", {"utc": "2026-11-21T10:00:00Z"}), datetime.datetime(2026, 11, 21, 10))
        self.assertIsNone(began("civvis-table-a", {}))
        self.assertIsNone(began("civvis-20261340T000000Z", {}))

    def test_a_summary_says_whether_the_diplomatic_victory_was_on(self):
        said = research_page.diplomatic
        # A single-player game reports its own switches once the board is up.
        self.assertIs(said({"seat": {"victories": {"diplomatic": False, "culture": True}}}), False)
        self.assertIs(said({"seat": {"victories": {"diplomatic": True}}}), True)
        # A hotseat table names what it was asked to switch off.
        self.assertIs(said({"victories_off": ["VICTORY_DIPLOMATIC"], "seat": {}}), False)
        self.assertIs(said({"victories_off": ["VICTORY_RELIGIOUS"]}), True)
        self.assertIs(said({"victories_off": []}), True)
        # Neither: not known, which is not "on".
        for summary in ({}, {"seat": None}, {"seat": {}}, {"seat": {"victories": {}}}, {"seat": {"victories": {"diplomatic": "no"}}}):
            self.assertIsNone(said(summary), summary)

    def test_the_two_tables_list_every_level_in_every_window(self):
        self.assertEqual(research_page.RATE_LEVELS, [8, 7, 6, 5, 4])
        self.assertEqual(research_page.RATE_DAYS, [1, 3, 7, 30])
        self.assertEqual(research_page.RATE_TABLES, [("on", True), ("off", False)])
        rates = research_page.win_rates(self.games())
        self.assertEqual(list(rates), ["to", "days", "games", "tables"])
        # Every window counts back from the newest tournament game in either
        # table, the Deity loss begun 11-22 04:00.
        self.assertEqual((rates["to"], rates["days"]), ("2026-11-22 04:00Z", [1, 3, 7, 30]))
        self.assertEqual(list(rates["tables"]), ["on", "off"])
        cells = lambda table, level: [
            (w["days"], w["games"], w["wins"])
            for row in rates["tables"][table]["levels"] if row["level"] == level
            for w in row["windows"]
        ]
        for table in rates["tables"].values():
            self.assertEqual([row["level"] for row in table["levels"]], [8, 7, 6, 5, 4])
        # Diplomatic off: the ladder's and the benchmark's games are in the
        # last day, the Deity elimination of 11-18 only in the last week.
        self.assertEqual(cells("off", 8), [(1, 0, 0), (3, 0, 0), (7, 1, 0), (30, 1, 0)])
        self.assertEqual(cells("off", 7), [(1, 1, 0), (3, 1, 0), (7, 1, 0), (30, 1, 0)])
        # A level nobody has played is still a row.
        self.assertEqual(cells("off", 6), [(1, 0, 0), (3, 0, 0), (7, 0, 0), (30, 0, 0)])
        self.assertEqual(cells("off", 5), [(1, 1, 0), (3, 1, 0), (7, 1, 0), (30, 1, 0)])
        self.assertEqual(cells("off", 4), [(1, 2, 2), (3, 2, 2), (7, 2, 2), (30, 2, 2)])
        # Diplomatic on: the newest game, then the game conceded on 11-19,
        # then the win of 11-17; the loss of 10-20 is outside the 30 days.
        self.assertEqual(cells("on", 8), [(1, 1, 0), (3, 2, 0), (7, 3, 1), (30, 3, 1)])
        for level in (7, 6, 5, 4):
            self.assertEqual(cells("on", level), [(1, 0, 0), (3, 0, 0), (7, 0, 0), (30, 0, 0)])
        # A table's games, and the record's, are what the widest window holds.
        self.assertEqual((rates["tables"]["off"]["games"], rates["tables"]["on"]["games"], rates["games"]), (5, 3, 8))
        self.assertIsNone(research_page.win_rates([]))

    def test_a_day_is_24_hours_back_from_the_newest_game(self):
        newest = datetime.datetime(2026, 11, 23, 4, 21, 12)
        ago = lambda **back: newest - datetime.timedelta(**back)
        games = [
            (newest, 8, True, False),
            (ago(hours=23, minutes=59), 8, False, False),
            # A day to the second is outside the last day and inside the last 3.
            (ago(days=1), 8, False, False),
            (ago(days=2, hours=23), 8, True, False),
            (ago(days=3), 8, False, False),
            (ago(days=29, hours=23), 8, False, False),
            (ago(days=30), 8, True, False),
        ]
        deity = research_page.win_rates(games)["tables"]["off"]["levels"][0]
        self.assertEqual(deity["level"], 8)
        self.assertEqual([(w["days"], w["games"], w["wins"]) for w in deity["windows"]], [(1, 2, 1), (3, 4, 2), (7, 5, 2), (30, 6, 2)])

    def test_the_record_holds_them_off_the_ledger_and_the_definitions(self):
        said = lambda on: {"seat": {"victories": {"diplomatic": on}}}
        more = {tag: said(on) for tag, on in self.SWITCH.items()}
        # One of the ladder's games was a hotseat table.
        more["civvis-t6-20261121T230000Z"] = {"victories_off": ["VICTORY_DIPLOMATIC"]}
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()):
            source = GamesAgainstTheAI().ledger(scratch, self.ROWS, more)
            self.assertEqual(research_page.switches(source, [row["tag"] for row in self.ROWS] + ["civvis-missing"]), self.SWITCH)
            # No definitions in the clone: no table, and the charts' games
            # are counted as before.
            bare = research_page.research(source)
            self.assertIsNone(bare["tournament_rates"])
            self.assertIn("8", bare["levels"])
            (source / "data").mkdir()
            (source / research_page.TOURNAMENTS).write_text(json.dumps({"tournaments": self.SPECS}), encoding="utf-8")
            record = research_page.research(source)
            said = io.StringIO()
            page = source / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            with contextlib.redirect_stderr(said):
                self.assertEqual(research_page.main(["research_page.py", scratch, str(page)]), 0)
            written = json.loads(research_page.SLOT.search(page.read_text(encoding="utf-8"))[2])
        self.assertEqual(record["tournament_rates"], research_page.win_rates(self.games()))
        self.assertEqual(written["tournament_rates"], record["tournament_rates"])
        self.assertIn("8 tournament games into", said.getvalue())
        self.assertEqual(bare["levels"], record["levels"])

    def test_the_committed_definitions_are_where_the_record_reads_them(self):
        defined = json.loads((ROOT / research_page.TOURNAMENTS).read_text(encoding="utf-8"))
        self.assertTrue(defined["tournaments"])
        # The same file the Tournaments page counts its tournaments from.
        self.assertIn('path = source / "data" / "tournaments.json"', (BETA / "tournaments_page.py").read_text(encoding="utf-8"))
        self.assertEqual(research_page.TOURNAMENTS, pathlib.Path("data/tournaments.json"))


class BuildOrders(unittest.TestCase):
    """Operator 2026-11-23: "add a tree for the early build order win rates
    for deity games. meaning, when scout was built first, what percentage of
    those games won? when settler was secnod, what percentage of
    scout-settler won? a person should be able to click thourgh and see the
    win rates of all the units. we should sort the tree by win rates
    descending. so the very first thing is: the winngest first build,
    underneath the second winnest first bulid. if etierh is clicked, then the
    winninest combinations of first build second build. etc... add this chart
    to the top of win analytics"."""

    SCOUT, SLINGER, SETTLER, BUILDER = "UNIT_SCOUT", "UNIT_SLINGER", "UNIT_SETTLER", "UNIT_BUILDER"

    #: The games: a tag's stamp, how it ended, and what its capital built.
    PLAYED = [
        ("20261120T010000Z", "win", [SCOUT, SETTLER, SLINGER]),
        ("20261120T020000Z", "rival_victory", [SCOUT, SETTLER, BUILDER]),
        ("20261120T030000Z", "defeat", [SCOUT, SLINGER, SETTLER]),
        ("20261120T040000Z", "abandoned", [SCOUT, SLINGER]),
        ("20261121T050000Z", "win", [SLINGER, SCOUT, SETTLER]),
        ("20261121T060000Z", "rival_victory", [SCOUT, SLINGER, SETTLER]),
        ("20261122T070000Z", "win", [SETTLER]),
    ]

    def rows(self) -> list[dict]:
        return [run(f"civvis-{stamp}", outcome, 200) for stamp, outcome, _ in self.PLAYED]

    def known(self) -> dict[str, list[str]]:
        return {f"civvis-{stamp}": builds for stamp, _, builds in self.PLAYED}

    def test_a_game_is_a_finished_game_against_deity_with_one_agent_in_it(self):
        rows = self.rows() + [
            # Somebody stopped it: no result.
            run("civvis-20261122T080000Z", "unfinished", 40),
            # Another level, and a table of two CIVVIS agents.
            run("civvis-20261122T090000Z", "win", 200, difficulty="DIFFICULTY_KING"),
            run("civvis-20261122T100000Z", "win", 200, civ="CIVILIZATION_ROME+CIVILIZATION_KOREA"),
            # A game reloaded: one game, ended the best way a run did, its
            # first run first, where the opening is.
            run("civvis-20261122T110000Z-cont2", "win", 210),
            run("civvis-20261122T110000Z", "unfinished", 90),
            run("civvis-20261122T110000Z-cont1", "rival_victory", 200),
        ]
        games = research_page.build_games(rows)
        self.assertEqual(len(games), len(self.PLAYED) + 1)
        self.assertEqual(
            games[-1],
            ("civvis-20261122T110000Z", ["civvis-20261122T110000Z", "civvis-20261122T110000Z-cont1", "civvis-20261122T110000Z-cont2"], True, "2026-11-22"),
        )
        self.assertEqual([won for _, _, won, _ in games[:7]], [True, False, False, False, True, False, True])
        self.assertEqual(research_page.build_games(rows, level=5), [("civvis-20261122T090000Z", ["civvis-20261122T090000Z"], True, "2026-11-22")])
        self.assertEqual(research_page.BUILD_LEVEL, home_rankings.CIV6_RUNGS.index("deity") + 1)

    def test_the_tree_lists_each_build_best_win_rate_first(self):
        tree = research_page.build_orders(research_page.build_games(self.rows()), self.known())
        self.assertEqual({key: tree[key] for key in ("level", "games", "wins", "finished", "from", "to", "depth")}, {
            "level": 8, "games": 7, "wins": 3, "finished": 7, "from": "2026-11-20", "to": "2026-11-22", "depth": research_page.BUILD_DEPTH,
        })
        named = lambda nodes: [(tree["items"][node[0]], node[1], node[2]) for node in nodes]
        # The first builds: the winningest at the top, the next under it.
        first = tree["tree"]
        self.assertEqual(named(first), [("Settler", 1, 1), ("Slinger", 1, 1), ("Scout", 5, 1)])
        # One game opened with a Settler and one with a Slinger: a line of
        # one game is not opened further.
        self.assertEqual([len(node) for node in first], [3, 3, 4])
        # Scout first: then the Settler (1 of 2) over the Slinger (0 of 3).
        second = first[2][3]
        self.assertEqual(named(second), [("Settler", 2, 1), ("Slinger", 3, 0)])
        # Scout, Settler: the two third builds, the one that won first.
        self.assertEqual(named(second[0][3]), [("Slinger", 1, 1), ("Builder", 1, 0)])
        # Scout, Slinger: one game built nothing more, so the builds under
        # it hold two of its three games.
        self.assertEqual(named(second[1][3]), [("Settler", 2, 0)])
        self.assertEqual(tree["items"], ["Settler", "Slinger", "Scout", "Builder"])

    def test_a_tie_goes_to_the_build_more_games_played_then_to_its_name(self):
        played = [("a", True, [self.SCOUT]), ("b", False, [self.SCOUT]), ("c", True, [self.SLINGER]), ("d", False, [self.SLINGER]),
                  ("e", True, [self.SETTLER]), ("f", False, [self.SETTLER]), ("g", True, [self.SETTLER]), ("h", False, [self.SETTLER])]
        games = [(key, [key], won, "2026-11-20") for key, won, _ in played]
        tree = research_page.build_orders(games, {key: builds for key, _, builds in played})
        self.assertEqual([(tree["items"][node[0]], node[1], node[2]) for node in tree["tree"]], [("Settler", 4, 2), ("Scout", 2, 1), ("Slinger", 2, 1)])

    def test_the_tree_is_no_deeper_than_its_depth_and_a_game_without_an_opening_is_left_out(self):
        games = [("a", ["a", "a-cont1"], True, "2026-11-20"), ("b", ["b"], False, "2026-11-21"), ("c", ["c"], False, "2026-11-22")]
        # A game's opening is asked of its first run, then of the rest.
        known = {"a-cont1": [self.SCOUT, self.SLINGER, self.SETTLER], "b": [self.SCOUT, self.SLINGER, self.BUILDER]}
        tree = research_page.build_orders(games, known, depth=2)
        self.assertEqual((tree["games"], tree["wins"], tree["finished"], tree["from"], tree["to"]), (2, 1, 3, "2026-11-20", "2026-11-21"))
        self.assertEqual(tree["tree"], [[0, 2, 1, [[1, 2, 1]]]])
        self.assertEqual(tree["items"], ["Scout", "Slinger"])
        self.assertIsNone(research_page.build_orders(games, {}))
        self.assertIsNone(research_page.build_orders([], known))

    def test_a_build_is_named_as_the_game_names_it(self):
        for item, name in (
            ("UNIT_SCOUT", "Scout"),
            ("UNIT_HEAVY_CHARIOT", "Heavy Chariot"),
            ("BUILDING_MONUMENT", "Monument"),
            ("BUILDING_WALLS", "Ancient Walls"),
            ("DISTRICT_HOLY_SITE", "Holy Site"),
            ("DISTRICT_THEATER", "Theater Square"),
            ("DISTRICT_GOVERNMENT", "Government Plaza"),
            ("BUILDING_GOV_WIDE", "Ancestral Hall"),
            # A civ's own unit: the type leads with the civ, the name does not.
            ("UNIT_GREEK_HOPLITE", "Hoplite"),
            ("UNIT_GAUL_GAESATAE", "Gaesatae"),
            ("UNIT_AZTEC_EAGLE_WARRIOR", "Eagle Warrior"),
            ("UNIT_SUMERIAN_WAR_CART", "War-Cart"),
            ("BUILDING_TAJ_MAHAL", "Taj Mahal"),
            # A project, by the name the game gives it.
            ("PROJECT_ENHANCE_DISTRICT_HOLY_SITE", "Holy Site Prayers"),
            ("PROJECT_REPAIR_OUTER_DEFENSES", "Repair Outer Defenses"),
            # An item the capital dropped unfinished (operator 2026-11-24).
            ("UNIT_SCOUT:dropped", "Scout, dropped"),
            ("DISTRICT_HOLY_SITE:dropped", "Holy Site, dropped"),
        ):
            self.assertEqual(research_page.build_name(item), name)

    def test_an_opening_comes_from_the_snapshot_or_the_runs_summary(self):
        first, second, third, fourth = (f"civvis-{stamp}" for stamp, _, _ in self.PLAYED[:4])
        rows = self.rows()[:4]
        reading = research_page.ladder().BUILD_ORDER_READING
        more = {
            # The harness writes the builds with their turns, and how it read them.
            second: {"build_order": [[self.SCOUT, 4], [self.SETTLER, 12], [self.BUILDER, 16]], "build_order_reading": reading},
            # Held by the snapshot too, which is not asked twice.
            first: {"build_order": [[self.BUILDER, 9]], "build_order_reading": reading},
            third: {"build_order": [], "build_order_reading": reading},
            # Read another way than the reader reads now: not taken.
            fourth: {"build_order": [[self.SLINGER, 6]], "build_order_reading": reading - 1},
        }
        snapshot = {"reading": reading, "items": [self.SCOUT, self.SETTLER, self.SLINGER], "games": [[first, [0, 1, 2]]]}
        self.assertEqual(research_page.held_openings(snapshot), {first: [self.SCOUT, self.SETTLER, self.SLINGER]})
        # A snapshot read another way holds nothing the tree takes.
        self.assertEqual(research_page.held_openings({**snapshot, "reading": reading - 1}), {})
        self.assertEqual(research_page.held_openings({key: value for key, value in snapshot.items() if key != "reading"}), {})
        self.assertEqual(research_page.builds_named([[self.SCOUT, 4], ["UNIT_SETTLER", 12]]), [self.SCOUT, self.SETTLER])
        # A step the capital dropped is marked.
        self.assertEqual(
            research_page.builds_named([[self.SCOUT, 4, "dropped"], ["UNIT_SETTLER", 12]]),
            [self.SCOUT + research_page.DROPPED, self.SETTLER],
        )
        for none in (None, [], "UNIT_SCOUT", [[4]]):
            self.assertIsNone(research_page.builds_named(none))
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()):
            source = GamesAgainstTheAI().ledger(scratch, rows, more)
            self.assertEqual(
                research_page.openings(source, [first, second, third, fourth, "civvis-missing"]),
                {first: [self.BUILDER], second: [self.SCOUT, self.SETTLER, self.BUILDER]},
            )
            # No snapshot in the clone: the summaries alone, two games that
            # opened differently, so neither is opened further.
            bare = research_page.research(source)["build_orders"]
            self.assertEqual((bare["games"], bare["finished"], bare["items"]), (2, 4, ["Builder", "Scout"]))
            self.assertEqual(bare["tree"], [[0, 1, 1], [1, 1, 0]])
            (source / research_page.BUILD_ORDERS).parent.mkdir(parents=True)
            (source / research_page.BUILD_ORDERS).write_text(json.dumps(snapshot), encoding="utf-8")
            record = research_page.research(source)["build_orders"]
            said = io.StringIO()
            page = source / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            civs = source / "civs.html"
            civs.write_text(CIVS, encoding="utf-8")
            with contextlib.redirect_stderr(said):
                self.assertEqual(research_page.main(["research_page.py", scratch, str(page)]), 0)
                self.assertEqual(civs_page.main(["civs_page.py", scratch, str(civs)]), 0)
            # Win Analytics has had no tree since 2026-11-23 and is not sent
            # it; Civ Analytics is sent the record built here.
            self.assertNotIn("build_orders", json.loads(research_page.SLOT.search(page.read_text(encoding="utf-8"))[2]))
            written = json.loads(civs_page.SLOT.search(civs.read_text(encoding="utf-8"))[2])
        # Two of the four games have an opening: one win of two, Scout then
        # Settler, then the Slinger of the game won over the Builder.
        self.assertEqual((record["games"], record["wins"], record["finished"]), (2, 1, 4))
        self.assertEqual(record["tree"], [[0, 2, 1, [[1, 2, 1, [[2, 1, 1], [3, 1, 0]]]]]])
        self.assertEqual(record["items"], ["Scout", "Settler", "Slinger", "Builder"])
        self.assertEqual(written["build_orders"], record)
        self.assertIn("civs_page: wrote 2 build orders", said.getvalue())

    def test_the_snapshot_is_read_off_the_event_logs_on_the_ledger(self):
        import gzip

        scout = lambda turn, **more: {"kind": "state", "turn": turn, "units": more.pop("units", []), "cities": [{
            "id": 1, "capital": True, "x": 5, "y": 5, "buildings": [], "districts": [], "wonders": [],
            "buildable": [{"t": self.SCOUT, "c": 15, "pr": more.get("production_progress", 0)}], **more}]}
        opening = [
            scout(1, producing_hash=7, production_cost=15, production_progress=11, production_turns=1),
            {"kind": "host_move", "turn": 1, "unit": 9},
            scout(2, producing_hash=0, production_cost=-1, production_progress=-1, units=[{"id": 2, "kind": self.SCOUT, "x": 5, "y": 5}]),
        ]
        first, second, third = "civvis-20261120T010000Z", "civvis-20261120T020000Z", "civvis-20261120T030000Z"
        fourth = "civvis-20261120T040000Z"
        reading = research_page.ladder().BUILD_ORDER_READING
        rows = [run(first, "win", 200), run(second + "-cont1", "rival_victory", 200), run(third, "defeat", 150),
                run(fourth, "defeat", 160)]
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()):
            repo = pathlib.Path(scratch)
            git = lambda *args: subprocess.run(["git", "-C", scratch, *args], check=True, capture_output=True, text=True)
            git("init", "-q")
            (repo / "results.jsonl").write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
            for tag, events, summary in (
                (first, opening, {}),
                # A game reloaded from a save: its one published run opens
                # mid-game, so its opening is in no log.
                (second + "-cont1", [scout(140, producing_hash=7, production_cost=15, production_progress=3)], {}),
                # Its summary carries the opening, read the way the reader
                # reads now: left to the summary.
                (third, opening, {"build_order": [[self.SETTLER, 9]], "build_order_reading": reading}),
                # One read another way is read again off the log.
                (fourth, opening, {"build_order": [[self.SETTLER, 9]]}),
            ):
                (repo / "runs" / tag).mkdir(parents=True)
                (repo / "runs" / tag / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
                with gzip.open(repo / "runs" / tag / "events.jsonl.gz", "wt", encoding="utf-8") as log:
                    log.write("".join(json.dumps(event) + "\n" for event in opening if events is opening) or json.dumps(events[0]) + "\n")
            git("add", "-A")
            git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "ledger")
            git("update-ref", "refs/remotes/origin/ledger", "HEAD")
            self.assertEqual([event["kind"] for event in research_page.run_events(repo, first)], ["state", "state"])
            self.assertEqual(list(research_page.run_events(repo, "civvis-missing")), [])
            out = repo / "out" / "build_orders.json"
            self.assertEqual(research_page.main(["research_page.py", "--build-orders", scratch, str(out)]), 0)
            taken = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual((taken["level"], taken["reading"], taken["items"], taken["games"]),
                             (8, reading, [self.SCOUT], [[first, [0]], [fourth, [0]]]))
            # A game per line, and a second run adds nothing it holds.
            self.assertIn('\n["civvis-20261120T010000Z",[0]],\n', out.read_text(encoding="utf-8"))
            held = {**taken, "items": ["UNIT_WARRIOR", self.SCOUT], "games": [[first, [1, 0]], [fourth, [1]]]}
            again = research_page.openings_snapshot(repo, held)
            self.assertEqual((again["items"], again["games"]), (held["items"], held["games"]))
            # A snapshot read another way is read again whole.
            anew = research_page.openings_snapshot(repo, {**held, "reading": reading - 1})
            self.assertEqual((anew["items"], anew["games"]), ([self.SCOUT], [[first, [0]], [fourth, [0]]]))
            # Read afresh (`--again`, for a change to how an opening is read):
            # the snapshot's own readings are dropped, and the game whose
            # summary carries an opening is read off its log too.
            self.assertEqual(research_page.main(["research_page.py", "--build-orders", scratch, str(out), "--again"]), 0)
            fresh = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual((fresh["items"], fresh["games"]), ([self.SCOUT], [[first, [0]], [third, [0]], [fourth, [0]]]))
            # With no snapshot, a publish reads off their logs the openings
            # no summary holds the way the reader reads now.
            self.assertEqual(research_page.research(repo)["build_orders"]["games"], 3)

    def test_the_committed_snapshot_holds_openings_the_tree_can_list(self):
        taken = json.loads((ROOT / research_page.BUILD_ORDERS).read_text(encoding="utf-8"))
        self.assertEqual(research_page.BUILD_ORDERS, pathlib.Path("data/research/build_orders.json"))
        self.assertEqual(taken["level"], research_page.BUILD_LEVEL)
        self.assertGreater(len(taken["games"]), 1000)
        keys = [key for key, _ in taken["games"]]
        self.assertEqual(keys, sorted(set(keys)))
        for key, builds in taken["games"]:
            self.assertTrue(builds and len(builds) <= 10, key)
            self.assertTrue(all(isinstance(at, int) and 0 <= at < len(taken["items"]) for at in builds), key)
        self.assertEqual(len(taken["items"]), len(set(taken["items"])))
        # Written a game to a line, as `written_openings` writes it.
        self.assertEqual((ROOT / research_page.BUILD_ORDERS).read_text(encoding="utf-8"), research_page.written_openings(taken))
        # Every build in it has a name the page can print.
        for item in taken["items"]:
            self.assertRegex(research_page.build_name(item), r"^[A-Z][A-Za-z\u2019' -]+(, dropped)?$", item)
        # Read the way the reader reads now: the page takes it.
        self.assertEqual(taken["reading"], research_page.ladder().BUILD_ORDER_READING)
        self.assertEqual(research_page.held_openings(taken).keys(), {key for key, _ in taken["games"]})


class Windows(unittest.TestCase):
    def test_a_window_counts_back_from_the_newest_game(self):
        groups = [
            {"day": "2026-10-20", "games": 4, "turns": {"score": {250: 4}}},
            {"day": "2026-11-15", "games": 3, "turns": {"science": {200: 2, 210: 1}}},
            {"day": "2026-11-21", "games": 2, "turns": {"diplomatic": {185: 1, 205: 1}}, "won": {"diplomatic": {205: 1}}},
        ]
        week = research_page.window(groups, 7)
        self.assertEqual((week["from"], week["to"], week["games"]), ("2026-11-15", "2026-11-21", 5))
        self.assertEqual(week["victories"]["science"]["median"], 200)
        self.assertEqual(week["victories"]["diplomatic"]["median"], 185)
        self.assertEqual(week["victories"]["diplomatic"]["won"], 1)
        # CIVVIS's wins are placed on their turns too, so the page can draw
        # them apart from the Deity AI's; a victory CIVVIS never won carries
        # no row of zeros.
        won = week["victories"]["diplomatic"]["won_turns"]
        self.assertEqual((len(won), won[204], sum(won)), (250, 1, 1))
        self.assertNotIn("won_turns", week["victories"]["science"])
        self.assertNotIn("won_turns", week["victories"]["score"])
        self.assertEqual(week["victories"]["score"]["games"], 0)
        self.assertIsNone(week["victories"]["score"]["median"])
        turns = week["victories"]["diplomatic"]["turns"]
        self.assertEqual(len(turns), 250)
        self.assertEqual((turns[184], turns[204], sum(turns)), (1, 1, 2))
        every = research_page.window(groups, None)
        self.assertEqual((every["from"], every["games"]), ("2026-10-20", 9))
        self.assertEqual(every["victories"]["score"]["turns"][249], 4)
        # A victory found as the last turn ends is stamped the turn after; it
        # is the last turn's. Nothing else outside the game counts.
        self.assertEqual(research_page.per_turn({251: 2, 250: 1, 252: 5, 0: 3})[249], 3)
        self.assertEqual(sum(research_page.per_turn({252: 5, 0: 3})), 0)
        self.assertIsNone(research_page.window([], 7))

    def test_every_victory_is_listed_in_one_order(self):
        self.assertEqual(research_page.VICTORIES, ["science", "culture", "diplomatic", "religious", "domination", "score"])
        self.assertEqual(sorted(research_page.LEDGER.values()), sorted(research_page.VICTORIES))
        # The page colours them in that order, and the stack runs in it.
        colours = re.findall(r"--v-(\w+): #[0-9a-f]{6};", PAGE)
        self.assertEqual(colours, research_page.VICTORIES)


class SelfPlay(unittest.TestCase):
    HEADER = {"kind": "header", "speed": "online", "turns": 250, "players": 6, "map": "continents", "all_seats": True}

    def batch(self, root: pathlib.Path, name: str, header: dict, games: list[tuple[int, str, int]]) -> None:
        directory = root / name
        directory.mkdir(parents=True)
        lines = [json.dumps(header)]
        for seed, victory, turn in games:
            for seat in range(6):  # every seat names the same ending
                lines.append(json.dumps({"kind": "game", "seed": seed, "seat": seat, "victory": victory, "turn": turn}))
        lines.append("{torn")
        (directory / "rows-continuous.jsonl").write_text("\n".join(lines) + "\n", encoding="utf-8")

    def test_the_snapshot_holds_a_game_per_seed_and_only_the_one_table(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = pathlib.Path(scratch)
            self.batch(root, "20261120T000256Z-6ad9", self.HEADER, [(1, "science", 204), (2, "diplomatic", 205), (3, "science", 204)])
            self.batch(root, "20261121T083532Z-db65", self.HEADER, [(4, "score", 250)])
            self.batch(root, "20261119T000000Z-aaaa", {**self.HEADER, "turns": 125}, [(5, "science", 120)])
            (root / "not-a-batch").mkdir()
            snap = research_page.snapshot(root)
            self.assertEqual([b["batch"] for b in snap["batches"]], ["20261120T000256Z-6ad9", "20261121T083532Z-db65"])
            first = snap["batches"][0]
            self.assertEqual((first["began"], first["games"]), ("2026-11-20", 3))
            self.assertEqual(first["turns"], {"science": {"204": 2}, "diplomatic": {"205": 1}})
            self.assertGreaterEqual(first["ended"], first["began"])
            # The file is a line per batch and reads back as it was written.
            text = research_page.written(snap)
            self.assertEqual(json.loads(text), snap)
            self.assertEqual(sum(line.startswith('{"batch"') for line in text.splitlines()), 2)
            # And the page reads it as a group per batch.
            groups = research_page.self_play_groups(snap)
            self.assertEqual(groups[0]["turns"]["science"], {204: 2})

    #: A moment for the newest game of the batches below to finish at.
    NOW = 1795546800.0  # 2026-11-24T19:00:00Z

    def part(self, root: pathlib.Path, batch: str, name: str, games: list[tuple[int, float]], written: float) -> None:
        """One worker's games of a batch, in the order it played them, each
        with how long it took, in a file last written at `written`."""
        directory = root / batch / "parts"
        directory.mkdir(parents=True, exist_ok=True)
        lines = [json.dumps(self.HEADER)]
        for seed, secs in games:
            for seat in range(2):  # every seat's row carries the game's length
                lines.append(json.dumps({"kind": "game", "seed": seed, "seat": seat, "secs": secs}))
        (directory / name).write_text("\n".join(lines) + "\n", encoding="utf-8")
        os.utime(directory / name, (written, written))

    def rows_written(self, root: pathlib.Path, batch: str, written: float) -> None:
        os.utime(root / batch / "rows-continuous.jsonl", (written, written))

    def test_a_game_finished_when_its_part_was_last_written_less_the_games_after_it(self):
        # A row has no clock, only how long its game took, and a part is one
        # worker's games end to end: the last finished when the file was last
        # written, and each one before it that game's length earlier.
        hour = 3600.0
        with tempfile.TemporaryDirectory() as scratch:
            root = pathlib.Path(scratch)
            self.part(root, "20261124T000000Z-aaaa", "part-1.jsonl", [(1, 2 * hour), (2, hour), (3, 3 * hour)], self.NOW)
            self.part(root, "20261124T000000Z-aaaa", "part-4.jsonl", [(4, hour), (5, 0)], self.NOW - 10 * hour)
            first = research_page.part_finishes(root / "20261124T000000Z-aaaa" / "parts" / "part-1.jsonl")
            self.assertEqual(first, {3: self.NOW, 2: self.NOW - 3 * hour, 1: self.NOW - 4 * hour})
            # A row with no length is dated with the game after it; a batch's
            # games are its parts' together.
            every = research_page.batch_finishes(root / "20261124T000000Z-aaaa")
            self.assertEqual(every[4], self.NOW - 10 * hour)
            self.assertEqual(every[5], self.NOW - 10 * hour)
            self.assertEqual(sorted(every), [1, 2, 3, 4, 5])
            self.assertEqual(research_page.batch_finishes(root / "no-such-batch"), {})
        self.assertEqual(research_page.stamp(self.NOW), "2026-11-24T19:00:00Z")

    def test_the_last_day_is_the_24_hours_before_the_newest_game_finished(self):
        # Operator 2026-11-24: "to win analytics civvis self play, add 3 days
        # and 1 day options please. 1 day covers last 24 hrs". A batch runs
        # for six hours to a day and a half, so a day is counted game by
        # game, back from the moment the newest finished.
        self.assertEqual(research_page.RECENT, [("1", 1), ("3", 3)])
        hour = 3600.0
        now = self.NOW
        with tempfile.TemporaryDirectory() as scratch:
            root = pathlib.Path(scratch)
            # A batch from a week ago: in neither window, and its parts are
            # not opened (this one's would put a game at this very hour).
            self.batch(root, "20261117T000000Z-0001", self.HEADER, [(1, "science", 200)])
            self.part(root, "20261117T000000Z-0001", "part-1.jsonl", [(1, hour)], now)
            self.rows_written(root, "20261117T000000Z-0001", now - 7 * 24 * hour)
            # A batch with no parts, whose rows were last written 50 hours
            # ago: its games count from then.
            self.batch(root, "20261122T000000Z-0002", self.HEADER, [(2, "culture", 180), (3, "culture", 181)])
            self.rows_written(root, "20261122T000000Z-0002", now - 50 * hour)
            # The running batch, two workers. The first played games 4 to 7,
            # finishing 30 h, 24 h, 23 h and 1 h ago; the second game 8,
            # finishing now, and game 9, which reached no victory.
            self.batch(root, "20261123T120000Z-0003", self.HEADER, [
                (4, "science", 204), (5, "science", 205), (6, "diplomatic", 205), (7, "science", 204), (8, "score", 250),
            ])
            self.part(root, "20261123T120000Z-0003", "part-4.jsonl",
                      [(4, hour), (5, 6 * hour), (6, hour), (7, 22 * hour)], now - hour)
            self.part(root, "20261123T120000Z-0003", "part-8.jsonl", [(9, hour), (8, 5 * hour)], now)
            self.rows_written(root, "20261123T120000Z-0003", now - 60)
            snap = research_page.snapshot(root)
            last = snap["last"]
            self.assertEqual(last["to"], "2026-11-24T19:00:00Z")
            self.assertEqual(list(last["windows"]), ["1", "3"])
            day, three = last["windows"]["1"], last["windows"]["3"]
            # 24 hours: the games that finished 23 h and 1 h ago and now. The
            # one that finished exactly 24 hours before the newest is out.
            self.assertEqual((day["hours"], day["games"], day["from"]), (24, 3, "2026-11-23T20:00:00Z"))
            self.assertEqual(day["turns"], {"science": {"204": 1}, "diplomatic": {"205": 1}, "score": {"250": 1}})
            # 72 hours: those, the two before them and the batch of 50 hours ago.
            self.assertEqual((three["hours"], three["games"], three["from"]), (72, 7, "2026-11-22T17:00:00Z"))
            self.assertEqual(three["turns"]["science"], {"204": 2, "205": 1})
            self.assertEqual(three["turns"]["culture"], {"180": 1, "181": 1})
            # The batches are held as they were, whole.
            self.assertEqual([b["games"] for b in snap["batches"]], [1, 2, 5])
            # The file keeps a line per window after the batches and reads
            # back as it was written.
            text = research_page.written(snap)
            self.assertEqual(json.loads(text), snap)
            self.assertEqual(sum(line.startswith('   "') for line in text.splitlines()), 2)
            self.assertLess(text.index('"batches"'), text.index('"last"'))

            # As the page reads them: a window like any other, with the hours
            # it covers and the moment they run back from, before the longer
            # windows.
            recent = research_page.recent_windows(snap)
            self.assertEqual(list(recent), ["1", "3"])
            self.assertEqual(
                {key: recent["1"][key] for key in ("from", "to", "games", "hours", "until")},
                {"from": "2026-11-23", "to": "2026-11-24", "games": 3, "hours": 24, "until": "2026-11-24T19:00:00Z"},
            )
            self.assertEqual(recent["1"]["victories"]["science"]["games"], 1)
            self.assertEqual(recent["1"]["victories"]["science"]["median"], 204)
            self.assertEqual(recent["1"]["victories"]["score"]["turns"][249], 1)
            self.assertEqual((recent["3"]["from"], recent["3"]["games"], recent["3"]["hours"]), ("2026-11-22", 7, 72))
            kind = research_page.kind("CIVVIS self-play", research_page.self_play_groups(snap), recent)
            self.assertEqual(list(kind["windows"]), ["1", "3", "7", "30", "all"])
            # The longer windows are counted as before, by whole batches.
            self.assertEqual(kind["windows"]["7"]["games"], 7)
            self.assertEqual(kind["windows"]["all"]["games"], 8)
            # A snapshot from before these windows were kept has none, and
            # neither has a kind that is not self-play.
            self.assertEqual(research_page.recent_windows({"batches": snap["batches"]}), {})
            self.assertEqual(research_page.recent_windows({**snap, "last": {"to": "soon", "windows": last["windows"]}}), {})
            plain = research_page.kind("CIVVIS self-play", research_page.self_play_groups(snap))
            self.assertEqual(list(plain["windows"]), ["7", "30", "all"])
        self.assertIsNone(research_page.last_windows([]))

    def test_the_committed_snapshot_is_the_one_table(self):
        snap = json.loads((ROOT / research_page.SELF_PLAY).read_text(encoding="utf-8"))
        self.assertTrue(snap["batches"])
        self.assertRegex(snap["taken"], r"^\d{4}-\d{2}-\d{2}$")
        # It holds the last 24 and 72 hours already counted, and the page's
        # record offers them.
        last = snap["last"]
        self.assertRegex(last["to"], research_page.STAMPED.pattern)
        self.assertEqual(list(last["windows"]), [key for key, _ in research_page.RECENT])
        every = sum(batch["games"] for batch in snap["batches"])
        counts = []
        for (key, days), held in zip(research_page.RECENT, last["windows"].values()):
            self.assertEqual(held["hours"], 24 * days)
            self.assertEqual(sum(n for turns in held["turns"].values() for n in turns.values()), held["games"])
            self.assertLessEqual(set(held["turns"]), set(research_page.VICTORIES))
            span = research_page.stamped(last["to"]) - research_page.stamped(held["from"])
            self.assertTrue(datetime.timedelta(0) <= span < datetime.timedelta(hours=24 * days), (key, span))
            counts.append(held["games"])
        self.assertTrue(0 < counts[0] <= counts[1] <= every, counts)
        self.assertEqual(list(research_page.recent_windows(snap)), ["1", "3"])
        for batch in snap["batches"]:
            self.assertRegex(batch["batch"], research_page.BATCH.pattern)
            counted = sum(n for turns in batch["turns"].values() for n in turns.values())
            self.assertEqual(counted, batch["games"])
            self.assertLessEqual(set(batch["turns"]), set(research_page.VICTORIES))
            # 251 is a victory found as turn 250 ended; `per_turn` counts it on 250.
            self.assertTrue(all(1 <= int(turn) <= 251 for turns in batch["turns"].values() for turn in turns))


class Page(unittest.TestCase):
    def test_the_page_carries_one_empty_slot_and_publishing_fills_it(self):
        self.assertEqual(PAGE.count('<script type="application/json" id="research-data">null</script>'), 1)
        data = {"turns": 250, "victories": ["science"], "self_play": {"name": "</script><b>x"}, "levels": {}, "level": 8}
        filled = research_page.fill(PAGE, data)
        self.assertEqual(filled.count("</script>"), PAGE.count("</script>"))
        self.assertEqual(json.loads(research_page.SLOT.search(filled)[2]), data)
        self.assertEqual(research_page.SLOT.sub("", filled), research_page.SLOT.sub("", PAGE))
        for line in (
            'research_page="$source_tree/beta/research.html"',
            'if [ -f "$research_page" ]; then',
            # At the address the page states itself, which is where its
            # relative links are right; `research` is where a revision from
            # before pages said so had it.
            'research_at="$(address "$research_page" research)"',
            'cp "$research_page" "$out/$research_at/index.html"',
            'fill research_page.py "$out/$research_at/index.html"',
            'pages+=("$research_at")',
        ):
            self.assertIn(line, PUBLISH)
        # The address follows the page's name in the site list (operator
        # 2026-11-23: "clean up the URL please. standardized. please remove
        # research from the url"): Win Analytics, under Charts/.
        self.assertIn('<link rel="canonical" href="https://civvis.ai/charts/win-analytics/">', PAGE)
        # Authored lane-relative: nothing in it links the site root.
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])

    def test_a_page_from_before_the_level_picker_is_sent_the_deity_games_where_it_reads_them(self):
        # Until 2026-11-22 the stable lane served its own revision's page
        # with head's figures (`publish.sh` took the page from `$source_tree`
        # and the script from `$repo_root`), and a page that read the Deity
        # games under `deity` (up to 2026-11-21), sent only `levels`, would
        # have shown its Deity charts empty until the next promotion. A lane
        # is now filled by its own revision's script, so the two always
        # agree; `for_page` still sends an older page what it reads.
        self.assertIn('research_page="$source_tree/beta/research.html"', PUBLISH)
        self.assertIn('fill research_page.py "$out/$research_at/index.html"', PUBLISH)
        self.assertNotIn('"$repo_root/beta/research', PUBLISH)
        deity = {"name": "CIVVIS vs Lv 8 Deity AI", "windows": {"7": {"games": 3}, "30": {"games": 3}, "all": {"games": 3}}}
        king = {"name": "CIVVIS vs Lv 5 King AI", "windows": {"7": {"games": 1}, "30": {"games": 1}, "all": {"games": 1}}}
        rates = {"to": "2026-11-22 04:00Z", "days": [1, 3, 7, 30], "games": 2, "tables": {"off": {"games": 2, "levels": []}, "on": {"games": 0, "levels": []}}}
        data = {"turns": 250, "victories": ["science"], "self_play": None, "self_play_taken": "", "levels": {"5": king, "8": deity}, "level": 8,
                "tournament_rates": rates}
        old = '<html><script type="application/json" id="research-data">null</script><script>data["deity"]</script></html>'
        sent = research_page.for_page(old, data)
        self.assertEqual(sent["deity"], {"name": "CIVVIS vs Deity AI", "windows": deity["windows"]})
        self.assertNotIn("levels", sent)
        self.assertNotIn("level", sent)
        # Nor the table of win rates, which it has no place for either.
        self.assertNotIn("tournament_rates", sent)
        self.assertEqual((sent["turns"], sent["victories"], sent["self_play"]), (250, ["science"], None))
        # No Deity games: the old page is told so, as it always was.
        self.assertIsNone(research_page.for_page(old, {**data, "levels": {"5": king}, "level": 5})["deity"])
        self.assertIsNone(research_page.for_page(old, None))
        # This revision's page has the picker and is sent the record whole.
        self.assertEqual(PAGE.count(research_page.LEVEL_PICKER), 1)
        self.assertEqual(research_page.for_page(PAGE, data), data)
        # And that is what publishing writes into each: the same record, in
        # the shape the page on disk reads.
        with tempfile.TemporaryDirectory() as scratch:
            rows = [
                run("civvis-20261120T100000Z", "rival_victory", 185, "VICTORY_TECHNOLOGY"),
                run("civvis-20261101T050000Z", "win", 233, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_KING"),
            ]
            source = GamesAgainstTheAI().ledger(scratch, rows)
            said = io.StringIO()
            for name, page in (("old.html", old), ("new.html", PAGE)):
                (source / name).write_text(page, encoding="utf-8")
                with contextlib.redirect_stderr(said):
                    self.assertEqual(research_page.main(["research_page.py", scratch, str(source / name)]), 0)
            sent_old = json.loads(research_page.SLOT.search((source / "old.html").read_text(encoding="utf-8"))[2])
            sent_new = json.loads(research_page.SLOT.search((source / "new.html").read_text(encoding="utf-8"))[2])
        self.assertEqual(sent_old["deity"]["windows"], sent_new["levels"]["8"]["windows"])
        self.assertNotIn("levels", sent_old)
        self.assertEqual((list(sent_new["levels"]), sent_new["level"], "deity" in sent_new), (["5", "8"], 8, False))
        self.assertIn("wrote 1 CIVVIS vs Deity AI games into", said.getvalue())
        self.assertIn("wrote 1 CIVVIS vs Lv 5 King AI games and 1 CIVVIS vs Lv 8 Deity AI games into", said.getvalue())

    def test_without_figures_the_page_says_so(self):
        with tempfile.TemporaryDirectory() as empty:
            source = pathlib.Path(empty)
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertIsNone(research_page.research(source))
            page = source / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(research_page.main(["research_page.py", empty, str(page)]), 0)
            self.assertEqual(research_page.SLOT.search(page.read_text(encoding="utf-8"))[2], "null")
            page.write_text("<html>no slot</html>", encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(research_page.main(["research_page.py", empty, str(page)]), 0)
            self.assertEqual(page.read_text(encoding="utf-8"), "<html>no slot</html>")
        self.assertIn('<p class="res-empty" id="res-empty">This copy of the page has no figures.', PAGE)
        self.assertIn('<div class="res-record" id="res-record" hidden>', PAGE)
        self.assertIn("if (!data) return;", PAGE)

    def test_the_page_draws_from_data_and_trusts_none_of_it_as_markup(self):
        self.assertNotIn("innerHTML", PAGE)
        self.assertNotIn("insertAdjacentHTML", PAGE)
        # The two kinds of game's charts. Each victory's timing and the
        # figures moved to Random AI Charts on 2026-11-26
        # (`test_each_victorys_timing_and_the_figures_moved_to_random_ai_charts`).
        for piece in ('id="res-kind-self_play"', 'id="res-kind-ai"'):
            self.assertIn(piece, PAGE)

    def worded(self, text: str, level: int = research_page.OPENING_LEVEL) -> str:
        """A sentence of the page for one level, as its script's `worded`
        writes it: `{level}` is "Lv 8 Deity" and `{rung}` is "Deity"."""
        rung = home_rankings.CIV6_RUNGS[level - 1].title()
        return text.replace("{level}", f"Lv {level} {rung}").replace("{rung}", rung)

    def test_the_games_against_the_ai_are_drawn_by_every_victory_then_by_each_sides_wins(self):
        # Operator 2026-11-21: "on civvis vs deity ai, start with all
        # victories by type along timeline, then just the deity ai wins, then
        # the civvis wins, then the civvis self play stuff".
        # The script draws its column charts off one list in that order. The
        # three charts of the games against the AI are named for the level
        # chosen.
        charts = re.findall(r'\{ id: "([\w-]+)", kind: "(\w+)", side: "(\w+)", label: "([^"]+)"', PAGE)
        self.assertEqual(
            charts,
            [
                ("ai", "ai", "all", "CIVVIS vs {level} AI: all games"),
                ("ai-rival", "ai", "rival", "CIVVIS vs {level} AI: {rung} AI wins"),
                ("ai-civvis", "ai", "civvis", "CIVVIS vs {level} AI: CIVVIS wins"),
                ("self_play", "self_play", "all", "CIVVIS self-play"),
            ],
        )
        # The charts of one kind share a scale, so the two sides' charts
        # stand against the first; self-play has its own.
        self.assertIn("for (const chart of CHARTS) drawStacks(chart, ymax[chart.kind]);", PAGE)
        # On the page a chart stands under its kind's name, so its title
        # there is the short one, `head`; `label` is the whole name.
        heads = re.findall(r'\{ id: "([\w-]+)", kind: "\w+", side: "\w+", label: "[^"]+", head: "([^"]+)"', PAGE)
        self.assertEqual(
            heads,
            [
                ("ai", "Turn Length and Victory Type Histogram of All Games"),
                ("ai-rival", "{rung} AI wins"),
                ("ai-civvis", "CIVVIS wins against {rung} AI"),
                ("self_play", "Every victory"),
            ],
        )
        self.assertIn('box.querySelector("[data-title]").textContent = worded(chart.head);', PAGE)
        # Each chart of the markup the script fills stands under the title
        # the list gives it for the level the page opens on.
        at = {}
        for name, head in heads:
            box = (
                f'<div class="res-kind" id="res-kind-{name}">\n              <h3 class="res-kind-title">'
                f"<span data-title>{self.worded(head)}</span> <small data-count></small></h3>"
            )
            self.assertEqual(PAGE.count(box), 1, name)
            at[name] = PAGE.index(box)
        self.assertEqual(PAGE.count('<div class="res-kind" id="res-kind-'), len(charts))
        # On the page the graphs are one wide column in this order (operator
        # 2026-11-22: "at the very top, we should have a wide civvis self
        # play graph. below it, the civvis vs lv 8 deity ai. then deity ai
        # wins, and civvis wins"): self-play's chart of every victory is the
        # first chart of the page, and the three of the games against the AI
        # follow it as the list has them.
        graphs = PAGE.index('<div class="res-graphs">')
        own, ai = PAGE.index('<section class="res-block" id="res-side-self_play"'), PAGE.index('<section class="res-block" id="res-side-ai"')
        places = [graphs, own, at["self_play"], ai, at["ai"], at["ai-rival"], at["ai-civvis"]]
        self.assertEqual(places, sorted(places))
        self.assertEqual(PAGE.index('class="res-kind"'), at["self_play"] + len("<div "))
        # One thing stands over the graphs and the rankings besides the
        # page's own opening lines, the benchmark against the game's own AI
        # (operator 2026-11-23: "at the top of win analytics, we should have
        # a last 1 day win rate col, last 3 days, last 7 days, last 30 days
        # ... a table on the left the is with diplo excluded and a table on
        # the right that allows diplo wins";
        # `test_the_benchmark_stands_over_the_graphs_as_two_tables`): one
        # block across the page, its two tables side by side. The tree of
        # build orders stood over it on 2026-11-23 and is on Civ Analytics
        # since (`test_the_build_orders_stand_as_a_tree_a_reader_opens_on_civ_analytics`).
        # Nothing else is over the graphs' column, whose first block is
        # self-play's. The key to the colours and the span of a column are
        # over the first chart, in its panel.
        over = PAGE[PAGE.index('<div class="res-record" id="res-record" hidden>') : graphs]
        self.assertEqual((over.count('class="res-section"'), over.count("<h2"), over.count("<section")), (2, 1, 1))
        self.assertNotIn("res-side-builds", PAGE)
        self.assertNotIn("res-kind", over)
        self.assertEqual(PAGE[graphs:own].strip(), '<div class="res-graphs">')
        key = PAGE[own : at["self_play"]]
        self.assertIn('<ul class="res-legend" id="res-legend" aria-label="Victory types"></ul>', key)
        self.assertIn('<div class="res-seg" id="res-span">', key)
        # The graphs' column is the page's width: the rankings that stood to
        # its right are on ELO Rankings and Civ Analytics since 2026-11-23.
        self.assertIn(".res-layout { display: grid; grid-template-columns: minmax(0, 1fr); gap: 16px; align-items: start; min-width: 0; }", PAGE)
        self.assertNotIn("res-lists", PAGE)
        for gone in ("res-sides", 'class="res-side"', "res-side-body", 'id="res-when"', "subgrid"):
            self.assertNotIn(gone, PAGE)
        # The two sides' charts are the first one split by who won: what is
        # not CIVVIS's win on a turn is the AI's, and every column is a
        # share of all the games counted, so the two add up to the first.
        self.assertIn('if (side === "civvis") return won;', PAGE)
        self.assertIn("return all.map((n, i) => Math.max(0, (Number(n) || 0) - (Number(won[i]) || 0)));", PAGE)
        self.assertIn("const h = y(0) - y(s.bins[v][i] / s.w.games);", PAGE)

    def test_each_victorys_timing_and_the_figures_moved_to_random_ai_charts(self):
        # Operator 2026-11-25: "please add another chart to the end of
        # charts/ called random ai charts. please move all the ai generated
        # charts that i havent approved of or we have discussed to here."
        # Each victory's timing (a small chart per victory, the games against
        # the AI and self-play as two lines) and the figures (a table per
        # victory, every span of turns as a table, and how they are counted)
        # stood under the graphs here; they are on Random AI Charts now, read
        # off the same record (`research_page.py` still writes it whole).
        # Nothing of them is left on Win Analytics: not their markup, their
        # rules or their script.
        for gone in (
            'id="res-each"', 'id="res-timing"', "Each victory&#8217;s timing", "res-panel", "res-cross", "is-wash",
            'id="res-table"', 'id="res-summary"', 'id="res-spans"', 'id="res-note-ai"', 'id="res-note-self"',
            "res-details", "Every span of turns as a table", "Median turn", "Busiest span",
            "drawTiming", "drawTables", "const KINDS", "const LABEL", "busiest",
        ):
            self.assertNotIn(gone, PAGE)
        # The column charts no longer point a reader at tables below them.
        self.assertNotIn("in the tables below", PAGE)
        # What the graphs read still counts what the moved charts read.
        for kept in ("const win = (kind) => {", "const binned = (turns, span) => {", "const sided = (vic, side) => {"):
            self.assertIn(kept, PAGE)

    def test_each_kind_of_game_counts_a_window_of_its_own_and_the_ai_games_open_on_30_days(self):
        # Operator 2026-11-22: "default civvis vs ai stuff to last 30 days.
        # higher levels finish faster". The games against the AI open on
        # their last 30 days; self-play, thousands of games a week, stays on
        # its last 7. So each kind has its own control, in its own head, and
        # the script counts each kind by its own choice.
        windows = re.findall(
            r'<div class="res-seg" id="res-window-(\w+)">\s*(?:<!--.*?-->\s*'
            r'<button type="button" data-window="1" aria-pressed="false">(?:Last |<span class="res-wide">Last </span>)1 day</button>\s*'
            r'<button type="button" data-window="3" aria-pressed="false">(?:Last |<span class="res-wide">Last </span>)3 days</button>\s*)?'
            r'<button type="button" data-window="7" aria-pressed="(true|false)">(?:Last |<span class="res-wide">Last </span>)7 days</button>\s*'
            r'<button type="button" data-window="30" aria-pressed="(true|false)">(?:Last |<span class="res-wide">Last </span>)30 days</button>\s*'
            r'<button type="button" data-window="all" aria-pressed="(true|false)">Every game</button>\s*</div>',
            PAGE,
            re.S,
        )
        # The benchmark over them, the first thing on the page, has none
        # since 2026-11-23: its tables list their windows side by side.
        self.assertEqual(
            windows,
            [("self_play", "true", "false", "false"), ("ai", "false", "true", "false")],
        )
        self.assertNotIn('id="res-window"', PAGE)
        self.assertIn("return games && games.windows ? games.windows[state.windows[kind]] : null;", PAGE)
        self.assertIn('choose(group.id, "data-window", (value) => { state.windows[kind] = value; });', PAGE)
        self.assertNotIn("state.window ", PAGE)
        self.assertNotIn("state.window]", PAGE)
        # Each control stands in the head of the kind it counts, after the
        # kind's name and before its charts.
        for kind, name in (("self_play", "res-title-self_play"), ("ai", "res-title-ai")):
            head = PAGE[PAGE.index(f'<section class="res-block" id="res-side-{kind}"') :]
            head = head[: head.index("</header>")]
            self.assertLess(head.index(f'id="{name}"'), head.index(f'id="res-window-{kind}"'), kind)
        # The record holds every window a control can ask for, for each kind
        # and at every level (`research_page.kind`).
        self.assertEqual([key for key, _ in research_page.WINDOWS], ["7", "30", "all"])
        self.assertEqual(sorted(set(re.findall(r'data-window="(\w+)"', PAGE))), ["1", "3", "30", "7", "all"])

    def test_self_play_alone_offers_its_last_day_and_its_last_three(self):
        # Operator 2026-11-24: "to win analytics civvis self play, add 3 days
        # and 1 day options please. 1 day covers last 24 hrs". Two more
        # choices in self-play's control, shortest first, and none in the
        # control of the games against the AI.
        def offered(kind: str) -> list:
            control = PAGE[PAGE.index(f'<div class="res-seg" id="res-window-{kind}">') :]
            control = control[: control.index("</div>")]
            found = re.findall(r'<button type="button" data-window="(\w+)" aria-pressed="(?:true|false)">(.+?)</button>', control)
            # Self-play's five lose their "Last" on a phone, so it stands in
            # a span of its own there; read, a choice says the same.
            return [(window, said.replace('<span class="res-wide">Last </span>', "Last ")) for window, said in found]

        self.assertEqual(
            offered("self_play"),
            [("1", "Last 1 day"), ("3", "Last 3 days"), ("7", "Last 7 days"), ("30", "Last 30 days"), ("all", "Every game")],
        )
        self.assertEqual(offered("ai"), [("7", "Last 7 days"), ("30", "Last 30 days"), ("all", "Every game")])
        # The record holds what the control offers, under the same names.
        self.assertEqual(
            [key for key, _ in research_page.RECENT] + [key for key, _ in research_page.WINDOWS],
            [key for key, _ in offered("self_play")],
        )
        # Self-play still opens on its last 7 days.
        control = PAGE[PAGE.index('id="res-window-self_play"') :]
        control = control[: control.index("</div>")]
        self.assertIn('data-window="7" aria-pressed="true"><span class="res-wide">Last </span>7 days</button>', control)
        self.assertEqual(control.count('aria-pressed="true"'), 1)
        # Five choices are wider than a phone: there each loses its "Last",
        # and narrower still the control scrolls in its own width.
        self.assertEqual(control.count('<span class="res-wide">Last </span>'), 4)
        self.assertIn("#res-window-self_play { max-width: 100%; overflow-x: auto; }", PAGE)
        self.assertRegex(
            PAGE,
            r"@media \(max-width: 479px\) \{\s*#res-window-self_play \.res-wide \{ display: none; \}\s*"
            r"#res-window-self_play button \{ padding: 0 8px; \}\s*\}",
        )
        # Under the title the page says what a day is: the hours, and the
        # moment they run back from.
        self.assertIn(
            'if (w.hours && w.until) return w.hours + " hours to " + day(w.to) + ", " + String(w.until).slice(11, 16) + " UTC";',
            PAGE,
        )
        # A window the record does not hold (a snapshot from before these
        # were counted) is not offered.
        self.assertIn(
            'if (kind === "self_play") button.hidden = !(data.self_play && data.self_play.windows '
            "&& data.self_play.windows[button.dataset.window]);",
            PAGE,
        )

    def test_the_benchmark_tables_name_their_games_and_no_par_note_stands_over_them(self):
        # Operator 2026-11-26: "drop the expected win rate thing warning
        # message and just add to the title: 6P Continents Online for the
        # first. 2nd is 6P Continents Online, Diplo Off". The big note of the
        # 1/6 par (operator 2026-11-24: "add a big note to firaxis civ 6
        # benchmark that expected win rate for a 6P game is 1/6 or 16.7%")
        # is gone, rule and all; nothing stands between the benchmark's head
        # and its two tables, and each table's title ends with the games it
        # counts. The par still shades every cell (the test after this one).
        side = PAGE.index('<section class="res-block" id="res-side-tourney" aria-labelledby="res-title-tourney">')
        block = PAGE[side : PAGE.index("</section>", side)]
        between = block[block.index("</header>") + len("</header>") : block.index('<div class="res-bench-pair">')]
        self.assertEqual(re.sub(r"<!--.*?-->", "", between, flags=re.S).strip(), "")
        for gone in ("res-bench-par", "Expected win rate", "beats the Firaxis AI"):
            self.assertNotIn(gone, PAGE)
        for name, title in (
            ("on", "Diplomatic victory allowed · 6P Continents Online"),
            ("off", "Diplomatic victory off · 6P Continents Online, Diplo Off"),
        ):
            self.assertIn(f'<h3 class="res-bench-title" id="res-title-rates-{name}">{title} <small id="res-rates-count-{name}"></small></h3>', block)

    def test_a_benchmark_cell_is_shaded_by_its_rate_against_the_par(self):
        # Operator 2026-11-24: "we should highlight the winr ate by level.
        # white for 16.7% - dark green for 100% win rare. white to red for
        # 16.7 -> 0. bxakground of the cells please. 16.7 is 1/6 expected
        # win rate." Every cell with games in it takes the shade of its
        # exact rate as its background, and the text that reads on it; a
        # cell with no game keeps the page's dark and its dash.
        self.assertIn("const PAR = 1 / 6, PAR_WHITE = [255, 255, 255], PAR_GREEN = [22, 101, 52], PAR_RED = [220, 38, 38];", PAGE)
        self.assertIn(
            '            if (games) {\n'
            '              td.append(" ", el("small", "", "(" + fmt(games) + ")"));\n'
            "              Object.assign(td.style, shade(wins / games));\n"
            "            }\n",
            PAGE,
        )
        # Tiles stand apart, the games in brackets take the tile's text
        # colour, and the rules stay the benchmark's own.
        self.assertIn(".res-rates { border-collapse: separate; border-spacing: 3px; }", PAGE)
        self.assertIn(".res-rates td.is-rate small { color: inherit; opacity: 0.75; }", PAGE)
        self.assertIn("Its shade is read against the 1/6 par: white at 16.7%, deepening to dark green at 100% and to red at 0%.", PAGE)

    @unittest.skipUnless(shutil.which("node"), "needs node to run the page's shade")
    def test_the_shade_is_white_at_the_par_dark_green_at_100_and_red_at_0(self):
        start = PAGE.index("    const PAR = 1 / 6,")
        script = PAGE[start : PAGE.index("    const BENCH", start)]
        rates = [0, 0.05, 1 / 6, 0.4, 1]
        out = subprocess.run(
            ["node", "-e", script + f"console.log(JSON.stringify({rates}.map(shade)));"],
            check=True, capture_output=True, text=True,
        ).stdout
        red, pink, par, green, deep = json.loads(out)
        self.assertEqual(red, {"background": "rgb(220, 38, 38)", "color": "#fff"})
        self.assertEqual(par, {"background": "rgb(255, 255, 255)", "color": "#101820"})
        self.assertEqual(deep, {"background": "rgb(22, 101, 52)", "color": "#fff"})
        # Between, the mix is straight: 5% is 70% of the way to red, 40%
        # 28% of the way to dark green, and both still read in dark text.
        self.assertEqual(pink, {"background": "rgb(231, 103, 103)", "color": "#101820"})
        self.assertEqual(green, {"background": "rgb(190, 212, 198)", "color": "#101820"})

    def test_the_self_play_count_is_highlighted(self):
        # Operator 2026-11-24: "hightlight the civvis self play game count
        # in the chart. 13k is a very solid number". Self-play's chart puts
        # its count of games in a highlighted figure, its days after it;
        # the charts of the games against the AI keep the plain count.
        self.assertIn(
            'if (s && chart.kind === "self_play") {\n'
            '        tally.replaceChildren(el("b", "res-big-count", fmt(s.w.games) + (s.w.games === 1 ? " game" : " games")), " · " + days(s.w));',
            PAGE,
        )
        self.assertIn(".res-kind-title .res-big-count { display: inline-block;", PAGE)
        self.assertRegex(PAGE, r"\.res-kind-title \.res-big-count \{[^}]*color: #fff; font-size: 20px; font-weight: 800;")

    def test_the_benchmark_stands_over_the_graphs_as_two_tables(self):
        # Operator 2026-11-23: "at the top of win analytics, we should have a
        # last 1 day win rate col, last 3 days, last 7 days, last 30 days.
        # this will help us track win rates. order the cols lv 8 down to
        # level 4. this will be our primary Firaxis Civ 6 AI benchmark - win
        # rate on each level. half the games can be diplo, half no diplo. we
        # should have a table on the left the is with diplo excluded and a
        # table on the right that allows diplo wins. we can drop the wins,
        # losses. games (n) should be listed after each value. so like 100%
        # (5) or 14% (181). at the top, we should have win rate (total
        # games)".
        # The benchmark is over the graphs, the width of the page, and the
        # first thing in the record: the tree of build orders, which stood
        # over it later that day, is on Civ Analytics since.
        record = PAGE.index('<div class="res-record" id="res-record" hidden>')
        side = PAGE.index('<section class="res-block" id="res-side-tourney" aria-labelledby="res-title-tourney">')
        layout = PAGE.index('<div class="res-layout">')
        self.assertLess(record, side)
        self.assertLess(side, layout)
        self.assertEqual(re.findall(r'<section class="res-block" id="([^"]+)"', PAGE[record:side]), [])
        block = PAGE[side : PAGE.index("</section>", side)]
        self.assertEqual(PAGE[side + len(block) + len("</section>") : layout].strip(), "")
        self.assertIn('<h2 class="res-side-title" id="res-title-tourney">Firaxis Civ 6 AI benchmark: win rate by level</h2>', block)
        # Two tables side by side, each named and with the count of its
        # games after its name. The games that allowed the Diplomatic
        # victory are at the left and the games with it off at the right
        # (operator, later on 2026-11-23, reversing the sides he first asked
        # for: "put diplo victory allowed to the left of no diplo victory"),
        # and nothing stands between the pair's opening and the left table.
        self.assertEqual(block.count("<table"), 2)
        pair = block.index('<div class="res-bench-pair">')
        off, on = block.index('<div class="res-section" id="res-bench-off">'), block.index('<div class="res-section" id="res-bench-on">')
        self.assertLess(pair, on)
        self.assertLess(on, off)
        self.assertEqual(block[pair + len('<div class="res-bench-pair">') : on].strip(), "")
        self.assertIn(".res-bench-pair { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; min-width: 0; }", PAGE)
        self.assertIn("@media (max-width: 859px) { .res-bench-pair { grid-template-columns: minmax(0, 1fr); } }", PAGE)
        for name, title in (
            ("on", "Diplomatic victory allowed · 6P Continents Online"),
            ("off", "Diplomatic victory off · 6P Continents Online, Diplo Off"),
        ):
            self.assertIn(
                f'<h3 class="res-bench-title" id="res-title-rates-{name}">{title} <small id="res-rates-count-{name}"></small></h3>', block
            )
            table = block[block.index(f'<table class="rank-table res-rates" id="res-rates-{name}" aria-labelledby="res-title-rates-{name}">') :]
            table = table[: table.index("</table>")]
            # Over the columns: "Win rate (total games)". Under it a column
            # per window, the last 1, 3, 7 and 30 days, in the order the
            # record lists a row's cells. No column of wins or of losses.
            self.assertIn('<th scope="col" rowspan="2">AI level</th><th scope="colgroup" colspan="4" class="is-group">Win rate (total games)</th>', table)
            self.assertEqual([int(days) for days in re.findall(r'<th scope="col" data-days="(\d+)">', table)], research_page.RATE_DAYS)
            self.assertEqual(
                re.findall(r'data-days="\d+"><span class="res-wide">Last </span>([^<]+)</th>', table),
                ["1 day", "3 days", "7 days", "30 days"],
            )
            self.assertEqual(len(re.findall(r"<th[ >]", table)), 2 + len(research_page.RATE_DAYS))
            for gone in ("Wins", "Losses", ">Games<"):
                self.assertNotIn(gone, table)
            self.assertIn("<tbody></tbody>", table)
        # The windows stand side by side, so there is none to choose.
        self.assertNotIn("res-window-tourney", PAGE)
        self.assertIn('const state = { windows: { self_play: "7", ai: "30" }, span: 5,', PAGE)
        # The script draws both tables off the record's `tournament_rates`,
        # in the record's order of rows, Lv 8 Deity first: a cell is the wins
        # as a share of its games and then the games in brackets, "14% (181)".
        self.assertIn("const BENCH = data.tournament_rates && data.tournament_rates.tables ? data.tournament_rates : null;", PAGE)
        self.assertIn('for (const name of ["on", "off"]) {', PAGE)
        self.assertEqual([name for name, _ in research_page.RATE_TABLES], ["on", "off"])
        self.assertIn('const body = document.querySelector("#res-rates-" + name + " tbody");', PAGE)
        self.assertIn('const td = el("td", games ? "is-rate" : "is-zero", games ? pct(wins / games) : "—");', PAGE)
        self.assertIn('td.append(" ", el("small", "", "(" + fmt(games) + ")"));', PAGE)
        self.assertIn("const render = () => { notes(); drawRates(); draw(); };", PAGE)
        # A record with no tournament games hides the block, and a hidden
        # block is not drawn (a block is a grid, which `hidden` alone would
        # not hide).
        self.assertIn('document.getElementById("res-side-tourney").hidden = !BENCH;', PAGE)
        self.assertIn(".res-record[hidden], .res-empty[hidden], .res-kind[hidden], .res-block[hidden] { display: none; }", PAGE)
        # The table's own rules stand after the shared table's, which they
        # adjust at the same weight.
        self.assertLess(PAGE.index(".rank-table td { padding:"), PAGE.index(".res-rates td { padding:"))
        self.assertLess(PAGE.index(".rank-table th.is-group {"), PAGE.index(".res-rates th.is-group {"))
        # What it counts is said under it, with the page its games are
        # listed on, which table a game stands in, and what a loss is, as
        # `tournament_games` counts one: every finished game that CIVVIS did
        # not win.
        self.assertIn('Every finished game of the tournaments on the <a href="../../tournaments/">Tournaments</a> page', block)
        self.assertIn("The left table holds the games that allowed the Diplomatic victory, the right table the games set up with it switched off.", block)
        self.assertIn("A win is a CIVVIS victory; a loss is a rival&#8217;s victory, our elimination, or a game conceded early.", block)
        self.assertIn("A day is 24 hours, counted back from the newest tournament game.", block)
        self.assertEqual(home_rankings.OUTCOMES, ["win", "defeat", "rival_victory", "abandoned", "unfinished"])

    def test_the_build_orders_stand_as_a_tree_a_reader_opens_on_civ_analytics(self):
        # Operator 2026-11-23: "add a tree for the early build order win
        # rates for deity games ... a person should be able to click thourgh
        # and see the win rates of all the units. we should sort the tree by
        # win rates descending. so the very first thing is: the winngest
        # first build, underneath the second winnest first bulid. if etierh
        # is clicked, then the winninest combinations of first build second
        # build. etc... add this chart to the top of win analytics". Later
        # that day it moved to a page of its own (operator: "add another
        # page: Civ Analytics. First can be the civ records (pulled from win
        # analytics), then early build order"), and on 2026-11-24 to its top
        # ("top chart is build order"): the first block of Civ Analytics'
        # record, the cities and the faith after it.
        PAGE = CIVS
        record = PAGE.index('<div class="civ-record" id="civ-record" hidden>')
        side = PAGE.index('<section class="res-block civ-builds" id="res-side-builds" aria-labelledby="res-title-builds" hidden>')
        self.assertEqual(re.findall(r'<section [^>]*id="([a-z-]+)"', PAGE[record:side]), [])
        block = PAGE[side : PAGE.index("</section>", side)]
        after = PAGE[side + len(block) + len("</section>") :]
        self.assertEqual(re.search(r'<section [^>]*id="([a-z-]+)"', after)[1], "civ-filtered")
        self.assertIn('<h2 class="res-side-title" id="res-title-builds">Early build order: win rate against Lv 8 Deity AI</h2>', block)
        self.assertIn('<p class="res-counted" id="res-counted-builds">', block)
        # One panel: the two headings, then the list the script fills.
        self.assertEqual(block.count('class="res-section"'), 1)
        self.assertIn('<div class="res-builds-head" aria-hidden="true"><span>Build order</span><span>Win rate (total games)</span></div>', block)
        self.assertIn('<ul class="res-builds" id="res-builds" aria-labelledby="res-title-builds"></ul>', block)
        # The note says what a step is, which games count and what a row is
        # (operator 2026-11-24: an item dropped unfinished is a step, marked).
        for sentence in (
            "CIVVIS&#8217;s capital&#8217;s build order: what it worked on, in order.",
            "or one it put production into and dropped unfinished, marked &#8220;dropped&#8221;",
            "so the first step is always one the capital could start on turn 1.",
            "The first steps are listed best win rate first; press one to list the steps after it, then the third,",
            "A row is CIVVIS&#8217;s win rate over the games whose build order opened with exactly the steps down to it and, in brackets, those games.",
            "An item bought with Gold or Faith, or handed over (a pantheon&#8217;s free Settler or Builder), is not a step.",
            "Every finished game against the Deity AI on the run ledger counts, in a tournament or not: a win is a CIVVIS victory; a loss is a rival&#8217;s victory, our elimination, or a game conceded early.",
            "A build order only one game played is not opened further.",
        ):
            self.assertIn(sentence, block)
        self.assertIn(f'<span id="res-builds-depth">{["zero", "one", "two", "three", "four", "five", "six", "seven", "eight"][research_page.BUILD_DEPTH]}</span>', block)
        self.assertIn('<span id="res-builds-left"></span>', block)
        # Hidden until the record holds a tree, and drawn once: no control
        # changes its games, and a second drawing would close what a reader
        # has opened.
        self.assertIn("const BUILDS = data.build_orders && Array.isArray(data.build_orders.tree) && data.build_orders.tree.length ? data.build_orders : null;", PAGE)
        self.assertIn('    if (BUILDS) {\n      document.getElementById("res-side-builds").hidden = false;', PAGE)
        self.assertEqual(PAGE.count('document.getElementById("res-builds").replaceChildren('), 1)
        draw = PAGE[PAGE.index("    if (BUILDS) {") : PAGE.index("    // ---- The cities and the faith")]
        # The record's order is the page's: best win rate first, as sent.
        self.assertIn('document.getElementById("res-builds").replaceChildren(...BUILDS.tree.map((node) => row(node, [])));', draw)
        self.assertNotIn(".sort(", draw)
        # A row: its place in the order, its name, then `14% (181)`.
        self.assertIn('el("span", "res-build-place", String(path.length)),', draw)
        self.assertIn('el("span", "res-build-name", name(item)),', draw)
        self.assertIn('figures.append(el("span", "res-build-rate", n ? pct((Number(won) || 0) / n) : "\\u2014"), el("span", "res-build-games", "(" + fmt(n) + ")"));', draw)
        # A build with builds after it is a button that opens them under it
        # and closes them again; one with none is a plain row.
        self.assertIn('const line = el(opens ? "button" : "div", "res-build");', draw)
        self.assertIn('line.setAttribute("aria-expanded", "false");', draw)
        self.assertIn('const open = line.getAttribute("aria-expanded") !== "true";', draw)
        self.assertIn("below.append(...next.map((child) => row(child, path)));", draw)
        self.assertIn("if (below) below.hidden = !open;", draw)
        # The whole order down to a row is its title and what a reader hears.
        self.assertIn('const said = path.join(", then ") + ": CIVVIS won "', draw)
        # The games left out are counted in the note.
        self.assertIn('document.getElementById("res-builds-left").textContent = left > 0', draw)
        # The two figures keep their widths, so they stand in columns down
        # the tree, and the builds after a build are set in under a rule.
        self.assertIn(".res-build { display: grid; grid-template-columns: 12px auto minmax(0, 1fr) 52px 58px;", PAGE)
        self.assertIn(".res-builds ul { margin-left: 6px; padding-left: 14px; border-left: 1px solid var(--border-strong); }", PAGE)
        self.assertIn(".res-builds ul[hidden] { display: none; }", PAGE)
        self.assertIn('.res-build[aria-expanded="true"] > .res-build-mark { border-left-color: var(--accent); transform: rotate(90deg); }', PAGE)
        self.assertIn("button.res-build:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }", PAGE)
        # A phone: a step in is half as deep, and under 360px the figures
        # stand one over the other and a name may wrap.
        self.assertIn(".res-builds ul { margin-left: 5px; padding-left: 6px; }", PAGE)
        self.assertIn(".res-build-figures { display: grid; justify-items: end; min-width: 40px; line-height: 1.25; }", PAGE)
        # A page from before the tree is not sent it, nor is Win Analytics
        # since the tree left it.
        old = '<html><script type="application/json" id="research-data">null</script></html>'
        self.assertNotIn("build_orders", research_page.for_page(old, {"levels": {}, "build_orders": {"tree": []}}))
        self.assertNotIn("build_orders", research_page.for_page(globals()["PAGE"], {"levels": {}, "build_orders": {"tree": []}}))
        self.assertIn("build_orders", research_page.for_page(
            research_page.LEVEL_PICKER + research_page.BUILD_TREE, {"levels": {}, "build_orders": {"tree": []}}))
        # The page's filler takes it from the record Win Analytics' script builds.
        self.assertIn('(research_page.research(source) or {}).get("build_orders")', (BETA / "civs_page.py").read_text(encoding="utf-8"))
        self.assertIn('fill civs_page.py "$out/$civs_at/index.html"', PUBLISH)

    def test_a_level_picks_the_games_against_the_ai(self):
        # Operator 2026-11-21: "on the civvis vs deity ai chart, lets have an
        # option to select by level too. so i could for example go to level 4
        # and see the win rate of civvis against level 4. broken down by
        # victor type and timeline. and i can filter for civvis wins vs Deity
        # AI wins like before."
        # And 2026-11-22: "civvis vs lv 8 deity ai chart should have a lv 1
        # to lv 8 slider at the top. defaults to 8. but people can change the
        # level if they want."
        # And 2026-11-23: "ai level in charts can go from lv 4 to lv 8".
        # The picker is a slider over the levels the page lists, Prince to
        # Deity, the game's highest, standing on the level the record opens
        # on, Deity.
        first, last = research_page.LEVELS[0], len(home_rankings.CIV6_RUNGS)
        self.assertEqual(research_page.LEVELS, list(range(4, last + 1)))
        self.assertEqual((last, research_page.OPENING_LEVEL), (8, 8))
        self.assertIn(
            f'<input type="range" id="res-level" min="{first}" max="{last}" step="1" value="{research_page.OPENING_LEVEL}" '
            f'aria-valuetext="Lv {research_page.OPENING_LEVEL} {home_rankings.CIV6_RUNGS[-1].title()}">',
            PAGE,
        )
        self.assertEqual(PAGE.count('id="res-level"'), 1)
        self.assertIn('<label for="res-level">AI level</label>', PAGE)
        # Under it, its scale: the five levels under the site's names for
        # them, in order, the opening one lit, and no level under Prince.
        stops = re.findall(
            r'<span data-level="(\d)" data-rung="(\w+)"( data-on)?><b>Lv (\d)</b> <small>(\w+)</small></span>',
            PAGE,
        )
        self.assertEqual(
            stops,
            [
                (str(at), rung.title(), " data-on" if at == research_page.OPENING_LEVEL else "", str(at), rung.title())
                for at, rung in enumerate(home_rankings.CIV6_RUNGS, 1)
                if at in research_page.LEVELS
            ],
        )
        self.assertEqual([stop[4] for stop in stops], ["Prince", "King", "Emperor", "Immortal", "Deity"])
        scale = PAGE[PAGE.index('<div class="res-level-stops" id="res-level-stops" aria-hidden="true">') :]
        self.assertEqual(scale[: scale.index("</div>")].count("<span data-level="), len(research_page.LEVELS))
        # The thumb stands over the level it is on: the track runs from the
        # middle of the first of the five names to the middle of the last.
        self.assertIn("width: calc(80% + 18px); height: 20px; margin: 0 calc(10% - 9px); padding: 0;", PAGE)
        self.assertIn(".res-level-stops { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); }", PAGE)
        # The page says the same range in words.
        self.assertIn("one level at a time, Lv 4 Prince to Lv 8 Deity, by the slider over them,", PAGE)
        self.assertNotIn("Lv 1 Settler", PAGE)
        # It is at the top of the games against the AI, the first thing
        # in their head, over their name. The buttons it replaces are gone.
        side = PAGE.index('<section class="res-block" id="res-side-ai"')
        name = PAGE.index('<h2 class="res-side-title" id="res-title-ai"')
        self.assertLess(side, PAGE.index('id="res-level"'))
        self.assertLess(PAGE.index('id="res-level-stops"'), name)
        self.assertLess(name, PAGE.index('id="res-kind-ai"'))
        self.assertNotIn("res-kind", PAGE[side:name])
        for gone in ('<div class="res-seg" id="res-level">', "levelButtons", "#res-level button", 'choose("res-level"'):
            self.assertNotIn(gone, PAGE)
        # The level chosen is the kind drawn wherever the games against the
        # AI are: the record holds a kind per level under `levels`.
        self.assertIn('const games = kind === "ai" ? LEVELS[state.level] : data[kind];', PAGE)
        self.assertIn(
            'const state = { windows: { self_play: "7", ai: "30" }, span: 5, level: RUNGS[data.level] ? String(data.level) : "8" };',
            PAGE,
        )
        self.assertIn('const RUNGS = Object.fromEntries(levelStops.map((stop) => [stop.dataset.level, stop.dataset.rung]));', PAGE)
        self.assertIn('levelSlider.addEventListener("input", () => picked(levelSlider.value));', PAGE)
        self.assertIn("if (stop) picked(stop.dataset.level);", PAGE)
        # A page with the slider is still a page with a level picker, so it
        # is sent the record with a kind per level.
        self.assertIn(research_page.LEVEL_PICKER, PAGE)
        # Every heading and key that names those games is a sentence the
        # script words for the level chosen, and as the page is written each
        # reads for the level it opens on.
        said = re.findall(r'data-say="([^"]+)">([^<]+)<', PAGE)
        self.assertEqual(
            [template for template, _ in said],
            ["CIVVIS vs {level} AI"],
        )
        for template, text in said:
            self.assertEqual(text, self.worded(template))
        self.assertEqual(self.worded("CIVVIS vs {level} AI: {rung} AI wins", 4), "CIVVIS vs Lv 4 Prince AI: Prince AI wins")
        self.assertIn(
            'String(text).replaceAll("{level}", "Lv " + state.level + " " + RUNGS[state.level])'
            '.replaceAll("{rung}", RUNGS[state.level]);',
            PAGE,
        )
        self.assertIn('for (const node of document.querySelectorAll("[data-say]")) node.textContent = worded(node.dataset.say);', PAGE)
        # The win rate: CIVVIS's wins as a share of the level's games, on the
        # line under the side's name.
        self.assertIn('<h2 class="res-side-title" id="res-title-ai" data-say="CIVVIS vs {level} AI">CIVVIS vs Lv 8 Deity AI</h2>', PAGE)
        self.assertIn('const rate = document.getElementById("res-counted-ai"), w = win("ai");', PAGE)
        self.assertIn('el("b", "", "CIVVIS won " + fmt(won) + " (" + pct(won / w.games) + ")")', PAGE)
        # No name of one level is left where the level chosen belongs: the
        # charts' script names Deity only in the operator's own words. (The
        # script of the two lists, after it, is not about a level chosen.)
        script = PAGE[PAGE.index("<script>") : PAGE.index("</script>", PAGE.index("<script>"))]
        self.assertIn("const CHARTS = [", script)
        self.assertEqual(re.findall(r'"[^"\n]*Deity[^"\n]*"', script), [])

    def test_every_page_lists_charts_between_home_and_tournaments(self):
        # Operator 2026-11-21: "between home and tournaments, we should add a
        # "Research"." This page lists the site's pages as the others do,
        # with itself marked. It was Musings later that day (operator:
        # "musings (rename research)") and is Charts since 2026-11-22
        # (operator: "Change musings to "Charts". Move performance from under
        # civvis to charts. Charts is where almost all our charts can go");
        # the file and the data kept theirs, and the address did until
        # 2026-11-23, when it took the page's name. Later that day it
        # became a folder of pages, named with its slash (operator: "in
        # charts, we should have at least win analytics, CIVVIS Performance
        # ... We should change just "Charts" to "Charts/" with the
        # intentional slash"): this page is Win Analytics, the first under
        # it and the one Charts/ opens. Since 2026-11-24 the list is a row
        # across the top and Charts/ drops its pages down (operator: "charts/
        # can drop down folder style and the user can pick the right
        # chart"), with this page ringed in it. AI Insights is the first
        # page under it, and the one Charts/ opens, since later that day
        # (operator: "we want a new ai insights").
        self.assertIn(
            '<nav class="site-nav" aria-label="CIVVIS sites">\n'
            '      <a class="site-link" href="../../home/">Home</a>\n'
            '      <div class="site-folder">\n'
            '        <a class="site-link" href="../ai-insights/" '
            'title="What the games show: charts read off the CIVVIS AI agents&#8217; games.">Charts/</a>\n'
            '        <div class="site-menu">\n'
            '          <a class="site-link sub-link" href="../ai-insights/" '
            'title="What the games show about AI at large: smarter AI picks up speed.">AI Insights</a>\n'
            '          <a class="site-link sub-link" href="../civ-ai-interpretability/" '
            'title="How a CIVVIS AI agent decides: one turn step by step, its plan through a game, and what each rule is worth in wins.">Civ AI Interpretability</a>\n'
            '          <a class="site-link sub-link" href="../civ-analytics/" '
            'title="How CIVVIS does as each civ, and the early build orders that win against Deity.">Civ Analytics</a>\n'
            '          <a class="site-link sub-link" href="./" aria-current="page">Win Analytics</a>\n'
            '          <a class="site-link sub-link" href="../elo-rankings/" '
            'title="The Civ AI Agents and Civilization VI&#8217;s own AI on one Elo scale, now and over time.">ELO Rankings</a>\n'
            '          <a class="site-link sub-link" href="../civvis-performance/" '
            'title="How fast the simulator plays: the fastest game on video and how long a self-play game takes.">CIVVIS Performance</a>\n'
            '          <a class="site-link sub-link" href="../random-ai-charts/" '
            'title="Charts the AI agents added on their own, kept here until they are asked for or talked over.">Random AI Charts</a>\n'
            '        </div>\n'
            '      </div>\n'
            '      <a class="site-link" href="../../tournaments/" ',
            PAGE,
        )
        nav = PAGE[PAGE.index('<nav class="site-nav"'):PAGE.index("</nav>")]
        self.assertEqual(nav.count('aria-current="page"'), 1)
        self.assertIn("  .site-folder:hover > .site-menu,\n  .site-folder:focus-within > .site-menu,\n  .site-folder.is-open > .site-menu { display: grid; }\n", PAGE)
        # The page's opening words are one line and name no other page: the
        # list over them does (operator, 2026-11-24: "there is too much text
        # on pages, particualrly home/ and charts/ ... some text ok. just
        # not too much").
        intro = PAGE[PAGE.index('<p class="intro">'):]
        intro = intro[:intro.index("</p>")]
        self.assertNotIn("<a ", intro)
        self.assertLess(len(re.sub(r"<[^>]+>", "", intro).split()), 20)
        # The two pages after it under Charts/ (operator 2026-11-23: "add
        # that to a new page: Charts/ELO Rankings. this should be after win
        # analytics in the ordering ... add another page: Civ Analytics")
        # list the same branch with themselves in gold, at the addresses
        # their names make.
        for page, name, address in ((ELO, "ELO Rankings", "elo-rankings"), (CIVS, "Civ Analytics", "civ-analytics")):
            nav = page[page.index('<nav class="site-nav"'):page.index("</nav>")]
            self.assertEqual(
                re.findall(r'<a class="site-link sub-link" href="([^"]+)"[^>]*>([^<]+)</a>', nav),
                [(("./" if listed == name else f"../{at}/"), listed) for at, listed in (
                    ("ai-insights", "AI Insights"),
                    ("civ-ai-interpretability", "Civ AI Interpretability"), ("civ-analytics", "Civ Analytics"),
                    ("win-analytics", "Win Analytics"), ("elo-rankings", "ELO Rankings"),
                    ("civvis-performance", "CIVVIS Performance"), ("random-ai-charts", "Random AI Charts"))],
            )
            self.assertEqual(nav.count('aria-current="page"'), 1)
            self.assertIn(f'<a class="site-link sub-link" href="./" aria-current="page">{name}</a>', nav)
            self.assertIn(f'<link rel="canonical" href="https://civvis.ai/charts/{address}/">', page)
            self.assertIn(f"<title>{name} · CIVVIS.ai</title>", page)
            self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', page), [])
        self.assertRegex(PAGE, r'<header class=\"shell site-header\">\s*<!--[\s\S]*?-->\s*<nav class=\"site-nav\"[^>]*>[\s\S]*?</nav>\s*<script data-site-folder>[\s\S]*?</script>\s*<div class=\"site-brand\">\s*<div class=\"site-lockup\">')
        self.assertIn("<title>Win Analytics · CIVVIS.ai</title>", PAGE)
        for page in (PAGE, LANDING, TOURNAMENTS, VIEWER):
            self.assertNotIn(">Research</a>", page)
            self.assertNotIn(">Musings</a>", page)
            self.assertNotIn(">Charts</a>", page)
        for page, pattern in (
            (LANDING, r'aria-current="page">Home</a>\s*<div class="site-folder">\s*<a class="site-link" href="\.\./charts/ai-insights/"[^>]*>Charts/</a>'
                      r'[\s\S]*?</div>\s*</div>\s*<a class="site-link" href="\.\./tournaments/"'),
            (TOURNAMENTS, r'href="\.\./home/">Home</a>\s*<div class="site-folder">\s*<a class="site-link" href="\.\./charts/ai-insights/"[^>]*>Charts/</a>'
                          r'[\s\S]*?</div>\s*</div>\s*<a class="site-link" href="\./" aria-current="page">Tournaments</a>'),
            (VIEWER, r'id="homelink" href="/home" target="_top">Home</a>\s*'
                     r'<a class="home-link" id="chartslink" href="/charts/ai-insights" target="_top">Charts/</a>\s*'
                     r'<a class="home-link" id="tourlink" href="/tournaments" target="_top">Tournaments</a>'),
        ):
            self.assertRegex(page, pattern)


if __name__ == "__main__":
    unittest.main()
