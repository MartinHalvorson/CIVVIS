#!/usr/bin/env python3
"""The Tournaments page: its record is counted off the ledger, and it fits its slot."""

from __future__ import annotations

import contextlib
import io
import json
import pathlib
import re
import tempfile
import unittest

import home_rankings
import tournaments_page

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "tournaments.html").read_text(encoding="utf-8")
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")
VIEWER = (ROOT / "web" / "index.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")

ON = {"players": 6, "map": "Continents.lua"}
MATCH = {"players": 6, "map": "Continents.lua"}


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
        # The totals count every game of every tournament begun.
        self.assertEqual(record["totals"], {"tournaments": 2, "games": 5, "wins": 2, "losses": 3})
        self.assertEqual(record["through"], "2026-11-21 10:55")

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
        # 2026-11-23: "clean up the URL please. standardized.").
        self.assertIn('<link rel="canonical" href="https://civvis.ai/auto-tourneys/">', PAGE)
        # The page is authored lane-relative: nothing in it links the site root.
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])

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

    def test_every_page_lists_auto_tourneys_between_charts_and_civvis(self):
        # Operator 2026-11-21: "Between Home and CIVVIS, we want to add a
        # Tournaments view." This page lists the site's pages as the others
        # do, with itself in gold, and is named Auto-Tourneys in the list
        # and on the tab since 2026-11-22 ("make sure Tournaments changes to
        # Auto-Tourneys"); its address was /tournaments/ until 2026-11-23
        # and is /auto-tourneys/, the name in the list, since. As on the home
        # page, the list stands to the left of the title (operator
        # 2026-11-21, of the home page: "on the left side, to the left of
        # civvis.ai").
        self.assertRegex(PAGE, r'<div class="site-brand">\s*<!--[\s\S]*?-->\s*<nav class="site-nav"[^>]*>[\s\S]*?</nav>\s*<div class="site-lockup">')
        self.assertRegex(LANDING, r'<div class="site-brand">\s*<!--[\s\S]*?-->\s*<nav class="site-nav"[^>]*>[\s\S]*?</nav>\s*<div class="site-lockup">')
        self.assertIn(
            '<nav class="site-nav" aria-label="CIVVIS sites">\n'
            '        <a class="site-link" href="../home/">Home</a>\n'
            '        <a class="site-link" href="../charts/win-analytics/" '
            'title="What the games show: charts read off the CIVVIS AI agents&#8217; games.">Charts/</a>\n'
            '        <a class="site-link" href="./" aria-current="page">Auto-Tourneys</a>\n'
            '        <a class="site-link" href="../" title="Every civilization is played by a CIVVIS AI agent.">CIVVIS</a>\n'
            '        <a class="site-link" href="../?mode=strange" '
            'title="One world, two timelines: the same game played two ways, side by side, on a timeline that branches.">Doctor Strange\'s Civ 6</a>\n'
            '        <a class="site-link" href="../?mode=play" '
            'title="You play one civilization. CIVVIS AI agents play the rest.">AI Debugger</a>\n'
            '        <a class="site-link" href="https://store.steampowered.com/app/289070/Sid_Meiers_Civilization_VI/" '
            'target="_blank" rel="noopener" title="Buy Civilization VI on Steam">Civ 6</a>\n'
            "      </nav>",
            PAGE,
        )
        self.assertIn('.site-link[aria-current="page"]:hover { background: var(--gold); color: #17130c; }', PAGE)
        self.assertIn("<title>Auto-Tourneys · CIVVIS.ai</title>", PAGE)
        # The Debugger is AI Debugger in this list again (operator
        # 2026-11-21: "AI Debugger (rename plz)", after a day as Strategy
        # Debugger and Strat Debugger), and every name stands on one line
        # ("make sure it fits on one line"): the column is as wide as its
        # longest name, AUTO-TOURNEYS, at every width, and the rule that
        # narrowed it on a phone under 380px is still gone.
        for page in (PAGE, LANDING, VIEWER):
            self.assertNotIn("Strategy Debugger", page)
            self.assertNotIn("Strat Debugger", page)
        for page in (PAGE, LANDING):
            self.assertNotIn("min-content", page)
        # The home page and the viewer name it third, after Charts (which
        # joined between Home and Tournaments the same day as Research; its
        # own test is `research_page_test.py`), each in its own markup.
        for page, pattern in (
            (LANDING, r'aria-current="page">Home</a>\s*<a class="site-link" href="\.\./charts/win-analytics/"[^>]*>Charts/</a>'
                      r'\s*<a class="site-link" href="\.\./auto-tourneys/"[^>]*>Auto-Tourneys</a>'
                      r'\s*<a class="site-link" href="\.\./"[^>]*>CIVVIS</a>'),
            (VIEWER, r'id="homelink" href="/home" target="_top">Home</a>\s*'
                     r'<a class="home-link" id="chartslink" href="/charts/win-analytics" target="_top">Charts/</a>\s*'
                     r'<a class="home-link" id="tourlink" href="/auto-tourneys" target="_top">Auto-Tourneys</a>\s*'
                     r'<a class="home-link" id="watchlink" href="\./" target="_top">CIVVIS</a>'),
        ):
            self.assertRegex(page, pattern)
        for page in (PAGE, LANDING, VIEWER):
            self.assertNotIn(">Tournaments</a>", page)
        # The viewer's link is root-absolute as written and lane-relative as
        # published: `publish.sh` turns a root link to any page the lane
        # carries into the lane's own.
        self.assertIn('href="/auto-tourneys"', VIEWER)
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
        # out. The seventh is his name for it (2026-11-23: "Call our
        # Indefinite Tourney the "Indefinite Benchmarking Tourney"").
        defined = json.loads((ROOT / "data" / "tournaments.json").read_text(encoding="utf-8"))
        self.assertEqual(defined["descriptions"], {
            "1": "Proof of concept, 10 game series",
            "2": "Abandoned",
            "3": "30 game series",
            "4": "50 game series",
            "5": "Repeat 50 game series but with 2/6 CIVVIS AI Agents (2nd half no Diplo allowed)",
            "6": "Benchmarking across levels: 5 games at each level (no Diplo allowed)",
            "7": "Indefinite Benchmarking Tourney",
        })
        self.assertEqual(list(defined["descriptions"]), [str(n) for n in range(1, len(defined["descriptions"]) + 1)])
        self.assertGreaterEqual(len(defined["descriptions"]), len(defined["tournaments"]))

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
        self.assertGreaterEqual(len(defined["hyperparameters"]), len(defined["tournaments"]) + 1)   # T4 and T5 share one
        for number, given in defined["hyperparameters"].items():
            self.assertEqual(tuple(given), tournaments_page.HYPERPARAMETERS, number)
            self.assertEqual(tournaments_page.hyperparameters(defined["hyperparameters"], int(number), {}),
                             [[name, value] for name, value in given.items()], number)
        # What the ledger shows, spelled out where it differs from the rest.
        hp = defined["hyperparameters"]
        self.assertEqual({n: hp[n]["Map"] for n in hp}, {n: "Continents, Small, 9 city-states, a new map each game" for n in hp})
        self.assertEqual({hp[n]["Game speed"] for n in hp} | {hp[n]["Turn limit"] for n in hp}, {"Online", "250"})
        self.assertIn("all but Diplomatic", hp["5"]["Victory types"])
        self.assertTrue(hp["6"]["Victory types"].startswith("All but Diplomatic"))
        self.assertIn("hotseat for games 1-7", hp["6"]["Played in"])
        self.assertIn("Lv 8 Deity 1 game in 2", hp["7"]["AI level"])
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
        self.assertEqual(PAGE.count("const settings = hyper(t);"), 3)   # running, scheduled and finished

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
            # What is scheduled stands in a column of its own on the right
            # (operator 2026-11-21: "lets have schedule in a col off on the
            # right"), from the width at which the table still fits beside
            # it; narrower, the three lists are one under another.
            "  @media (min-width: 1420px) {\n"
            "    .tour-record { grid-template-columns: minmax(0, 1fr) 300px; align-items: start; }\n"
            "    .tour-record > * { grid-column: 1; }\n"
            "    .tour-record > #tour-scheduled { grid-column: 2; grid-row: 1 / span 3; }\n"
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
        self.assertIn("""    "auto-tourneys": b'id="tournaments-data"',\n""", verify)
        self.assertIn("""    "tournaments": "auto-tourneys",\n""", verify)


if __name__ == "__main__":
    unittest.main()
