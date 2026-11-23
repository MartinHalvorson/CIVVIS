#!/usr/bin/env python3
"""The Research page: when each victory lands, counted off both kinds of game."""

from __future__ import annotations

import contextlib
import io
import json
import pathlib
import re
import subprocess
import tempfile
import unittest

import home_rankings
import research_page

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "research.html").read_text(encoding="utf-8")
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

    def ledger(self, scratch: str, rows: list[dict]) -> pathlib.Path:
        """A clone whose ledger holds these result lines, every run played to
        the full length."""
        repo = pathlib.Path(scratch)
        git = lambda *args: subprocess.run(["git", "-C", scratch, *args], check=True, capture_output=True, text=True)
        git("init", "-q")
        (repo / "results.jsonl").write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
        for row in rows:
            (repo / "runs" / row["tag"]).mkdir(parents=True)
            summary = {"speed": "GAMESPEED_ONLINE", "max_turns": 250}
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
    our win rate against lv 4... lv 8 deity ai in tournament games. please
    list sample sizes, wins, losses in the table too. table should default to
    last 30 days"."""

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
        # The ladder, by level, and a table of four players it does not count.
        run("civvis-t6-20261121T210000Z", "win", 224, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_PRINCE"),
        run("civvis-t6-20261121T220000Z", "win", 230, "VICTORY_CULTURE", difficulty="DIFFICULTY_PRINCE"),
        run("civvis-t6-20261121T230000Z", "rival_victory", 241, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_KING"),
        run("civvis-t6-20261122T010000Z", "win", 250, "VICTORY_SCORE", difficulty="DIFFICULTY_EMPEROR", players=4),
        # A level under the table's lowest: a tournament's game, not a row.
        run("civvis-t6-20261122T020000Z", "win", 210, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_WARLORD"),
        # The indefinite benchmark.
        run("civvis-t7-20261122T030000Z", "rival_victory", 233, "VICTORY_TECHNOLOGY", difficulty="DIFFICULTY_IMMORTAL"),
        run("civvis-t7-20261122T040000Z", "rival_victory", 236, "VICTORY_CULTURE"),
    ]

    def test_the_games_are_the_tournaments_finished_games_by_level(self):
        games = sorted(research_page.tournament_games(self.ROWS, self.SPECS))
        self.assertEqual(
            games,
            [
                ("2026-10-20", 8, False),
                ("2026-11-17", 8, True),
                ("2026-11-18", 8, False),
                ("2026-11-19", 8, False),
                ("2026-11-21", 4, True),
                ("2026-11-21", 4, True),
                ("2026-11-21", 5, False),
                ("2026-11-22", 7, False),
                ("2026-11-22", 8, False),
            ],
        )
        # They are the games the Auto-Tourneys page counts, level by level:
        # the same series, the same wins and losses.
        series = home_rankings.tournament_series(self.ROWS, self.SPECS, detail=True)
        listed = [
            ending for row in series for _, ending in row["played"]
            if home_rankings.level(ending.get("difficulty")) in research_page.RATE_LEVELS
        ]
        self.assertEqual(len(listed), len(games))
        self.assertEqual(sum(ending["outcome"] == "win" for ending in listed), sum(won for _, _, won in games))
        self.assertEqual(research_page.tournament_games(self.ROWS, []), [])

    def test_a_window_lists_every_level_with_its_games_wins_and_losses(self):
        self.assertEqual(research_page.RATE_LEVELS, [4, 5, 6, 7, 8])
        rates = research_page.win_rates(research_page.tournament_games(self.ROWS, self.SPECS))
        self.assertEqual(list(rates), ["windows"])
        self.assertEqual(list(rates["windows"]), [key for key, _ in research_page.WINDOWS])
        row = lambda level, games, wins: {"level": level, "games": games, "wins": wins, "losses": games - wins}
        # The last 30 days count back from the newest tournament game at any
        # level, 11-22: the loss of 10-20 is outside them.
        month = rates["windows"]["30"]
        self.assertEqual((month["from"], month["to"], month["games"]), ("2026-11-17", "2026-11-22", 8))
        self.assertEqual(month["levels"], [row(4, 2, 2), row(5, 1, 0), row(6, 0, 0), row(7, 1, 0), row(8, 4, 1)])
        # Every game holds it, and a level nobody has played is still a row.
        whole = rates["windows"]["all"]
        self.assertEqual((whole["from"], whole["to"], whole["games"]), ("2026-10-20", "2026-11-22", 9))
        self.assertEqual(whole["levels"], [row(4, 2, 2), row(5, 1, 0), row(6, 0, 0), row(7, 1, 0), row(8, 5, 1)])
        # One window is the same days for every level: Lv 4, last played on
        # 11-21, and Lv 8 are counted over the same last week.
        week = rates["windows"]["7"]
        self.assertEqual((week["from"], week["to"], week["games"]), ("2026-11-17", "2026-11-22", 8))
        for window in rates["windows"].values():
            self.assertEqual(window["games"], sum(level["games"] for level in window["levels"]))
            for level in window["levels"]:
                self.assertEqual(level["wins"] + level["losses"], level["games"])
        self.assertIsNone(research_page.win_rates([]))

    def test_the_record_holds_them_off_the_ledger_and_the_definitions(self):
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()):
            source = GamesAgainstTheAI().ledger(scratch, self.ROWS)
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
        self.assertEqual(record["tournament_rates"], research_page.win_rates(research_page.tournament_games(self.ROWS, self.SPECS)))
        self.assertEqual(written["tournament_rates"], record["tournament_rates"])
        self.assertIn("9 tournament games into", said.getvalue())
        self.assertEqual(bare["levels"], record["levels"])

    def test_the_committed_definitions_are_where_the_record_reads_them(self):
        defined = json.loads((ROOT / research_page.TOURNAMENTS).read_text(encoding="utf-8"))
        self.assertTrue(defined["tournaments"])
        # The same file the Auto-Tourneys page counts its tournaments from.
        self.assertIn('path = source / "data" / "tournaments.json"', (BETA / "tournaments_page.py").read_text(encoding="utf-8"))
        self.assertEqual(research_page.TOURNAMENTS, pathlib.Path("data/tournaments.json"))


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

    def test_the_committed_snapshot_is_the_one_table(self):
        snap = json.loads((ROOT / research_page.SELF_PLAY).read_text(encoding="utf-8"))
        self.assertTrue(snap["batches"])
        self.assertRegex(snap["taken"], r"^\d{4}-\d{2}-\d{2}$")
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
        rates = {"windows": {"7": {"games": 2}, "30": {"games": 2}, "all": {"games": 2}}}
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
        self.assertIs(research_page.for_page(PAGE, data), data)
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
        # The two kinds of game, the two charts and the tables.
        for piece in ('id="res-kind-self_play"', 'id="res-kind-ai"', 'id="res-timing"', 'id="res-summary"', 'id="res-spans"'):
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
        # The script draws its column charts, and the groups of the table of
        # spans, off one list in that order. The three charts of the games
        # against the AI are named for the level chosen.
        charts = re.findall(r'\{ id: "([\w-]+)", kind: "(\w+)", side: "(\w+)", label: "([^"]+)"', PAGE)
        self.assertEqual(
            charts,
            [
                ("ai", "ai", "all", "CIVVIS vs {level} AI: every victory"),
                ("ai-rival", "ai", "rival", "CIVVIS vs {level} AI: {rung} AI wins"),
                ("ai-civvis", "ai", "civvis", "CIVVIS vs {level} AI: CIVVIS wins"),
                ("self_play", "self_play", "all", "CIVVIS self-play"),
            ],
        )
        # The charts of one kind share a scale, so the two sides' charts
        # stand against the first; self-play has its own.
        self.assertIn("for (const chart of CHARTS) drawStacks(chart, ymax[chart.kind]);", PAGE)
        self.assertIn("for (const chart of CHARTS) {\n        const th = el(\"th\", \"is-group\", worded(chart.label));", PAGE)
        # On the page a chart stands under its kind's name, so its title
        # there is the short one, `head`; `label`, the whole name, heads its
        # columns in the table of spans.
        heads = re.findall(r'\{ id: "([\w-]+)", kind: "\w+", side: "\w+", label: "[^"]+", head: "([^"]+)"', PAGE)
        self.assertEqual(
            heads,
            [("ai", "Every victory"), ("ai-rival", "{rung} AI wins"), ("ai-civvis", "CIVVIS wins"), ("self_play", "Every victory")],
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
        places = [graphs, own, at["self_play"], ai, at["ai"], at["ai-rival"], at["ai-civvis"], PAGE.index('<aside class="res-lists"')]
        self.assertEqual(places, sorted(places))
        self.assertEqual(PAGE.index('class="res-kind"'), at["self_play"] + len("<div "))
        # One thing stands over the graphs besides the page's own opening
        # lines, the table of win rates over the tournaments' games (operator
        # later on 2026-11-22: "to the top of win analytics roughly, please
        # list our win rate against lv 4... lv 8 deity ai in tournament
        # games"; `test_the_win_rate_at_each_level_stands_over_the_graphs`).
        # It is the first block of the graphs' column, so nothing is over
        # the column and the rankings still stand to its right. The key to
        # the colours and the span of a column are over the first chart, in
        # its panel.
        opening = PAGE[PAGE.index('<div class="res-record" id="res-record" hidden>') : own]
        self.assertEqual((opening.count("res-section"), opening.count("<h2"), opening.count("<section")), (1, 1, 1))
        self.assertIn('<section class="res-block" id="res-side-tourney"', opening)
        self.assertNotIn("res-kind", opening)
        over = PAGE[PAGE.index('<div class="res-record" id="res-record" hidden>') : graphs]
        self.assertNotIn("<section", over)
        self.assertNotIn("<h2", over)
        key = PAGE[own : at["self_play"]]
        self.assertIn('<ul class="res-legend" id="res-legend" aria-label="Victory types"></ul>', key)
        self.assertIn('<div class="res-seg" id="res-span">', key)
        # The graphs' column is the wide one, twice the rankings', and under
        # 860px the page is one column, the graphs first.
        self.assertIn(".res-layout { display: grid; grid-template-columns: minmax(0, 2fr) minmax(0, 1fr); gap: 16px; align-items: start; min-width: 0; }", PAGE)
        self.assertIn("@media (max-width: 859px) { .res-layout { grid-template-columns: minmax(0, 1fr); } }", PAGE)
        for gone in ("res-sides", 'class="res-side"', "res-side-body", 'id="res-when"', "subgrid"):
            self.assertNotIn(gone, PAGE)
        # The two sides' charts are the first one split by who won: what is
        # not CIVVIS's win on a turn is the AI's, and every column is a
        # share of all the games counted, so the two add up to the first.
        self.assertIn('if (side === "civvis") return won;', PAGE)
        self.assertIn("return all.map((n, i) => Math.max(0, (Number(n) || 0) - (Number(won[i]) || 0)));", PAGE)
        self.assertIn("const h = y(0) - y(s.bins[v][i] / s.w.games);", PAGE)

    def test_the_games_against_the_ai_are_listed_before_the_self_play_games(self):
        # Operator 2026-11-21: "list the civvis vs deity ai stuff before the
        # civvis self-play. still side by side, just swap the ordering".
        # The script lists per kind in this order: the counts, the figures
        # above each small chart, the hover rows and both tables' rows.
        # (The column charts at the top of the page are the other way about
        # since 2026-11-22, self-play's first: see the test above.)
        self.assertIn('const KINDS = ["ai", "self_play"];', PAGE)
        # And the markup the script fills, in the same order: the line key,
        # the notes and their sources.
        for first, second in (
            (">CIVVIS vs Lv 8 Deity AI</span></li>", "</span>CIVVIS self-play (in the victory"),
            ('id="res-note-ai"', 'id="res-note-self"'),
            (">Games against the AI</a>", ">Self-play games</a>"),
        ):
            self.assertEqual((PAGE.count(first), PAGE.count(second)), (1, 1), (first, second))
            self.assertLess(PAGE.index(first), PAGE.index(second), (first, second))
        # The figures table keeps the two side by side, the AI's columns on
        # the left with the two columns self-play has no use for: who won,
        # the AI before CIVVIS as the charts stand.
        self.assertIn(
            '<tr><th></th><th class="is-group" colspan="6" data-say="CIVVIS vs {level} AI">CIVVIS vs Lv 8 Deity AI</th>'
            '<th class="is-group" colspan="4">CIVVIS self-play</th></tr>',
            PAGE,
        )
        self.assertRegex(
            PAGE,
            r'<th scope="col">Victory</th>\s*'
            r'<th scope="col">Games</th><th scope="col">Share</th><th scope="col" data-say="\{rung\} AI won">Deity AI won</th><th scope="col">CIVVIS won</th>'
            r'<th scope="col">Median turn</th><th scope="col">Busiest span</th>\s*'
            r'<th scope="col">Games</th><th scope="col">Share</th>'
            r'<th scope="col">Median turn</th><th scope="col">Busiest span</th>\s*</tr>',
        )

    def test_each_kind_of_game_counts_a_window_of_its_own_and_the_ai_games_open_on_30_days(self):
        # Operator 2026-11-22: "default civvis vs ai stuff to last 30 days.
        # higher levels finish faster". The games against the AI open on
        # their last 30 days; self-play, thousands of games a week, stays on
        # its last 7. So each kind has its own control, in its own head, and
        # the script counts each kind by its own choice.
        windows = re.findall(
            r'<div class="res-seg" id="res-window-(\w+)">\s*'
            r'<button type="button" data-window="7" aria-pressed="(true|false)">Last 7 days</button>\s*'
            r'<button type="button" data-window="30" aria-pressed="(true|false)">Last 30 days</button>\s*'
            r'<button type="button" data-window="all" aria-pressed="(true|false)">Every game</button>\s*</div>',
            PAGE,
        )
        # The table of win rates over the tournaments' games, the first
        # thing on the page, has a third, on 30 days too (operator later
        # that day: "table should default to last 30 days").
        self.assertEqual(
            windows,
            [("tourney", "false", "true", "false"), ("self_play", "true", "false", "false"), ("ai", "false", "true", "false")],
        )
        self.assertNotIn('id="res-window"', PAGE)
        self.assertIn("return games && games.windows ? games.windows[state.windows[kind]] : null;", PAGE)
        self.assertIn('choose(group.id, "data-window", (value) => { state.windows[kind] = value; });', PAGE)
        self.assertNotIn("state.window ", PAGE)
        self.assertNotIn("state.window]", PAGE)
        # Each control stands in the head of the kind it counts, after the
        # kind's name and before its charts.
        for kind, name in (("tourney", "res-title-tourney"), ("self_play", "res-title-self_play"), ("ai", "res-title-ai")):
            head = PAGE[PAGE.index(f'<section class="res-block" id="res-side-{kind}"') :]
            head = head[: head.index("</header>")]
            self.assertLess(head.index(f'id="{name}"'), head.index(f'id="res-window-{kind}"'), kind)
        # The record holds every window a control can ask for, for each kind
        # and at every level (`research_page.kind`).
        self.assertEqual([key for key, _ in research_page.WINDOWS], ["7", "30", "all"])
        self.assertEqual(sorted(set(re.findall(r'data-window="(\w+)"', PAGE))), ["30", "7", "all"])

    def test_the_win_rate_at_each_level_stands_over_the_graphs(self):
        # Operator 2026-11-22: "to the top of win analytics roughly, please
        # list our win rate against lv 4... lv 8 deity ai in tournament
        # games. please list sample sizes, wins, losses in the table too.
        # table should default to last 30 days".
        # The table is the first block of the graphs' column: over the
        # self-play graph, with the rankings to its right as before.
        graphs = PAGE.index('<div class="res-graphs">')
        side = PAGE.index('<section class="res-block" id="res-side-tourney" aria-labelledby="res-title-tourney">')
        self.assertEqual(PAGE[graphs:side].strip(), '<div class="res-graphs">')
        self.assertLess(side, PAGE.index('<section class="res-block" id="res-side-self_play"'))
        block = PAGE[side : PAGE.index("</section>", side)]
        self.assertIn('<h2 class="res-side-title" id="res-title-tourney">Win rate vs the Lv 4 to Lv 8 AI</h2>', block)
        # Its columns: the level, the win rate, then the sample size, the
        # wins and the losses.
        self.assertIn(
            '<table class="rank-table res-rates" id="res-rates" aria-labelledby="res-title-tourney">\n'
            "                <thead>\n"
            '                  <tr><th scope="col">AI level</th><th scope="col">Win rate</th><th scope="col">Games</th>'
            '<th scope="col">Wins</th><th scope="col">Losses</th></tr>\n'
            "                </thead>\n"
            "                <tbody></tbody>",
            block,
        )
        # It opens on the last 30 days, by a control of its own in its head.
        self.assertRegex(
            block,
            r'<div class="res-seg" id="res-window-tourney">\s*'
            r'<button type="button" data-window="7" aria-pressed="false">Last 7 days</button>\s*'
            r'<button type="button" data-window="30" aria-pressed="true">Last 30 days</button>\s*'
            r'<button type="button" data-window="all" aria-pressed="false">Every game</button>',
        )
        self.assertIn('tourney: "30" }', PAGE)
        # The script draws a row per level of the window chosen, off the
        # record's `tournament_rates`: the rate is the wins as a share of the
        # row's games, and the games, wins and losses follow it.
        self.assertIn("const RATES = data.tournament_rates && data.tournament_rates.windows ? data.tournament_rates.windows : null;", PAGE)
        self.assertIn("const w = RATES ? RATES[state.windows.tourney] : null;", PAGE)
        self.assertIn('tr.append(el("td", games ? "is-rate" : "is-zero", games ? pct(wins / games) : "—"));', PAGE)
        self.assertIn('for (const n of [games, wins, losses]) tr.append(el("td", games ? "" : "is-zero", fmt(n)));', PAGE)
        self.assertIn("const render = () => { notes(); drawRates(); draw(); drawTables(); };", PAGE)
        # A record with no tournament games hides the block, and a hidden
        # block is not drawn (a block is a grid, which `hidden` alone would
        # not hide).
        self.assertIn('document.getElementById("res-side-tourney").hidden = !w;', PAGE)
        self.assertIn(".res-record[hidden], .res-empty[hidden], .res-kind[hidden], .res-block[hidden] { display: none; }", PAGE)
        # The table's own rules stand after the shared table's, which they
        # adjust at the same weight.
        self.assertLess(PAGE.index(".rank-table td { padding:"), PAGE.index(".res-rates td { padding:"))
        # What it counts is said under it, with the page its games are
        # listed on, and what a loss is, as `tournament_games` counts one:
        # every finished game that CIVVIS did not win.
        self.assertIn('Every finished game of the tournaments on <a href="../../auto-tourneys/">Auto-Tourneys</a>', block)
        self.assertIn("A win is a CIVVIS victory; a loss is a rival&#8217;s victory, our elimination, or a game conceded early.", block)
        self.assertEqual(home_rankings.OUTCOMES, ["win", "defeat", "rival_victory", "abandoned", "unfinished"])

    def test_a_level_picks_the_games_against_the_ai(self):
        # Operator 2026-11-21: "on the civvis vs deity ai chart, lets have an
        # option to select by level too. so i could for example go to level 4
        # and see the win rate of civvis against level 4. broken down by
        # victor type and timeline. and i can filter for civvis wins vs Deity
        # AI wins like before."
        # And 2026-11-22: "civvis vs lv 8 deity ai chart should have a lv 1
        # to lv 8 slider at the top. defaults to 8. but people can change the
        # level if they want."
        # The picker is a slider over the game's eight levels, standing on
        # the level the record opens on, Deity.
        last = len(home_rankings.CIV6_RUNGS)
        self.assertEqual((last, research_page.OPENING_LEVEL), (8, 8))
        self.assertIn(
            f'<input type="range" id="res-level" min="1" max="{last}" step="1" value="{research_page.OPENING_LEVEL}" '
            f'aria-valuetext="Lv {research_page.OPENING_LEVEL} {home_rankings.CIV6_RUNGS[-1].title()}">',
            PAGE,
        )
        self.assertEqual(PAGE.count('id="res-level"'), 1)
        self.assertIn('<label for="res-level">AI level</label>', PAGE)
        # Under it, its scale: the eight levels under the site's names for
        # them, in order, the opening one lit.
        stops = re.findall(
            r'<span data-level="(\d)" data-rung="(\w+)"( data-on)?><b>Lv (\d)</b> <small>(\w+)</small></span>',
            PAGE,
        )
        self.assertEqual(
            stops,
            [
                (str(at), rung.title(), " data-on" if at == research_page.OPENING_LEVEL else "", str(at), rung.title())
                for at, rung in enumerate(home_rankings.CIV6_RUNGS, 1)
            ],
        )
        scale = PAGE[PAGE.index('<div class="res-level-stops" id="res-level-stops" aria-hidden="true">') :]
        self.assertEqual(scale[: scale.index("</div>")].count("<span data-level="), last)
        # The thumb stands over the level it is on: the track runs from the
        # middle of the first of the eight names to the middle of the last.
        self.assertIn("width: calc(87.5% + 18px); height: 20px; margin: 0 calc(6.25% - 9px); padding: 0;", PAGE)
        self.assertIn(".res-level-stops { display: grid; grid-template-columns: repeat(8, minmax(0, 1fr)); }", PAGE)
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
            'const state = { windows: { self_play: "7", ai: "30", tourney: "30" }, span: 5, level: RUNGS[data.level] ? String(data.level) : "8" };',
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
            ["CIVVIS vs {level} AI", "CIVVIS vs {level} AI", "CIVVIS vs {level} AI", "{rung} AI won"],
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

    def test_every_page_lists_charts_between_home_and_auto_tourneys(self):
        # Operator 2026-11-21: "between home and tournaments, we should add a
        # "Research"." This page lists the site's pages as the others do,
        # with itself in gold. It was Musings later that day (operator:
        # "musings (rename research)") and is Charts since 2026-11-22
        # (operator: "Change musings to "Charts". Move performance from under
        # civvis to charts. Charts is where almost all our charts can go");
        # the file and the data kept theirs, and the address did until
        # 2026-11-23, when it took the page's name. Later that day it
        # became a folder of pages, named with its slash (operator: "in
        # charts, we should have at least win analytics, CIVVIS Performance
        # ... We should change just "Charts" to "Charts/" with the
        # intentional slash"): this page is Win Analytics, the first under
        # it and the one Charts/ opens, and the branch is open here with
        # this page in gold.
        self.assertIn(
            '<nav class="site-nav" aria-label="CIVVIS sites">\n'
            '        <a class="site-link" href="../../home/">Home</a>\n'
            '        <a class="site-link" href="./" '
            'title="What the games show: charts read off the CIVVIS AI agents&#8217; games.">Charts/</a>\n'
            '        <a class="site-link sub-link" href="./" aria-current="page">Win Analytics</a>\n'
            '        <a class="site-link sub-link" href="../civvis-performance/" '
            'title="How fast the simulator plays: the fastest game on video and how long a self-play game takes.">CIVVIS Performance</a>\n'
            '        <a class="site-link sub-link" href="../ai-interpretability/" '
            'title="How a CIVVIS AI agent decides: one turn step by step, its plan through a game, and what each rule is worth in wins.">AI Interpretability</a>\n'
            '        <a class="site-link" href="../../auto-tourneys/" ',
            PAGE,
        )
        nav = PAGE[PAGE.index('<nav class="site-nav"'):PAGE.index("</nav>")]
        self.assertEqual(nav.count('aria-current="page"'), 1)
        self.assertIn(".site-link.sub-link { margin-left: 10px; padding: 2px 6px 3px; font-size: 10px; letter-spacing: 0.03em; }", PAGE)
        self.assertIn('<a href="../civvis-performance/">CIVVIS Performance</a>.', PAGE)
        self.assertRegex(PAGE, r'<div class="site-brand">\s*<!--[\s\S]*?-->\s*<nav class="site-nav"[^>]*>[\s\S]*?</nav>\s*<div class="site-lockup">')
        self.assertIn("<title>Win Analytics · CIVVIS.ai</title>", PAGE)
        for page in (PAGE, LANDING, TOURNAMENTS, VIEWER):
            self.assertNotIn(">Research</a>", page)
            self.assertNotIn(">Musings</a>", page)
            self.assertNotIn(">Charts</a>", page)
        for page, pattern in (
            (LANDING, r'aria-current="page">Home</a>\s*<a class="site-link" href="\.\./charts/win-analytics/"[^>]*>Charts/</a>'
                      r'\s*<a class="site-link" href="\.\./auto-tourneys/"'),
            (TOURNAMENTS, r'href="\.\./home/">Home</a>\s*<a class="site-link" href="\.\./charts/win-analytics/"[^>]*>Charts/</a>'
                          r'\s*<a class="site-link" href="\./" aria-current="page">Auto-Tourneys</a>'),
            (VIEWER, r'id="homelink" href="/home" target="_top">Home</a>\s*'
                     r'<a class="home-link" id="chartslink" href="/charts/win-analytics" target="_top">Charts/</a>\s*'
                     r'<a class="home-link" id="tourlink" href="/auto-tourneys" target="_top">Auto-Tourneys</a>'),
        ):
            self.assertRegex(page, pattern)


if __name__ == "__main__":
    unittest.main()
