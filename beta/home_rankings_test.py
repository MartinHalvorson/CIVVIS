#!/usr/bin/env python3
"""The home page's rankings column is derived from this tree and fits its slot."""

from __future__ import annotations

import contextlib
import io
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile
import unittest

import home_rankings
import performance_page

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
# The two rankings stand on Win Analytics, the first page under Charts/, since
# 2026-11-22 (operator: "move 2 charts on home/ to charts/"); the home page is
# four videos.
CHARTS = (BETA / "research.html").read_text(encoding="utf-8")
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")
PAGES = {name: (BETA / f"{name}.html").read_text(encoding="utf-8") for name in ("landing", "research", "performance", "tournaments")}

sys.path.insert(0, str(ROOT / "tools"))
import civ6_ladder  # noqa: E402

#: A CI runner has no git identity; supplied through the environment so the
#: suite never writes to any repository's config.
PROBE = {
    "GIT_AUTHOR_NAME": "rankings probe",
    "GIT_AUTHOR_EMAIL": "probe@civvis.invalid",
    "GIT_COMMITTER_NAME": "rankings probe",
    "GIT_COMMITTER_EMAIL": "probe@civvis.invalid",
}


def git(repo: pathlib.Path, *args: str, stdin: str | None = None) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), *args], input=stdin, capture_output=True, text=True, check=True,
        env={**os.environ, **PROBE},
    ).stdout.strip()


def run(tag: str, outcome: str, civ: str | None = None, **more) -> dict:
    """One line of the results file, as `civ6_ladder.result_row` writes it:
    a Lv 8 game, on a day the civ table counts unless the line says another."""
    return {
        "tag": tag,
        "game": re.sub(r"-cont\d+$", "", tag),
        "utc": "2026-11-17T00:00:00Z",
        "civ": f"CIVILIZATION_{civ}" if civ else None,
        "difficulty": "DIFFICULTY_DEITY",
        "outcome": outcome,
        "victory": None,
        **more,
    }


def with_ledger(clone: pathlib.Path, rows: list[dict]) -> None:
    """A clone whose fetched `ledger` branch holds these result lines."""
    git(clone, "init", "-q")
    text = "".join(json.dumps(row) + "\n" for row in rows)
    blob = git(clone, "hash-object", "-w", "--stdin", stdin=text)
    tree = git(clone, "mktree", stdin=f"100644 blob {blob}\tresults.jsonl\n")
    git(clone, "update-ref", "refs/remotes/origin/ledger", git(clone, "commit-tree", tree, "-m", "ledger"))


class HomeRankings(unittest.TestCase):
    def test_each_agent_carries_its_95_percent_margin_and_the_window_its_runs(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = pathlib.Path(scratch)
            (root / "data").mkdir()
            (root / "src").mkdir()
            (root / "src" / "strategies.rs").write_text(
                (ROOT / "src" / "strategies.rs").read_text(encoding="utf-8"), encoding="utf-8"
            )
            rating = {"elo": 1200.0, "sd": 40.0, "games": 100, "wins": 30, "bound": ""}
            published = {
                "tables": 2640,
                "players": 4,
                "turns": 250,
                "runs": [{"run": "", "tables": 1320}, {"run": "2026-11-23-15125c4", "tables": 1320}],
                "ratings": {"jack": rating, "holy": dict(rating, margin95=50.0), "stock": dict(rating, elo=1000.0)},
                "ladder": {},
            }
            (root / "data" / "strategy_elo.json").write_text(json.dumps(published), encoding="utf-8")
            ranking = home_rankings.strategies(root)
        margins = {row["id"]: row["margin"] for row in ranking["rows"] if row["kind"] == "agent"}
        # A file without `margin95` (written before it existed) gets 1.96 sd.
        self.assertEqual(margins["jack"], 78)
        self.assertEqual(margins["holy"], 50.0)
        self.assertEqual((ranking["runs"], ranking["latest_run"]), (2, "2026-11-23"))
        self.assertIn('" \\u00b1 is the 95% margin of error."', CHARTS)

    def test_the_agents_are_ranked_by_published_elo_under_their_handles(self):
        published = json.loads((ROOT / "data" / "strategy_elo.json").read_text(encoding="utf-8"))
        ranking = home_rankings.strategies(ROOT)
        rows = ranking["rows"]
        self.assertEqual([row["elo"] for row in rows], sorted((row["elo"] for row in rows), reverse=True))
        agents = [row for row in rows if row["kind"] == "agent"]
        rated = {key for key in published["ratings"] if not key.startswith("civ6:")}
        self.assertEqual({row["id"] for row in agents}, rated)
        for row in agents:
            # The roster is read out of Rust source by pattern. If its shape
            # moves, an agent falls back to its bare id: caught here, at head,
            # rather than on the published page.
            self.assertNotEqual(row["name"], row["id"], f"{row['id']} lost its handle")
            self.assertTrue(row["tilt"], f"{row['id']} lost its tilt")
            self.assertEqual(row["elo"], published["ratings"][row["id"]]["elo"])
        rungs = [row for row in rows if row["kind"] == "rung"]
        # Rungs the tables cannot tell apart share a row ("Prince · King").
        self.assertIn("Prince", [name for row in rungs for name in row["name"].split(" · ")])
        # A rung's level number is its place on the engine's own ladder.
        strategies_rs = (ROOT / "src" / "strategies.rs").read_text(encoding="utf-8")
        ladder = re.search(r"CIV6_RUNGS: \[.*?\] = \[(.*?)\];", strategies_rs, re.S)
        self.assertEqual(re.findall(r'\("(\w+)",', ladder[1]), home_rankings.CIV6_RUNGS)
        levels = {name: lv for row in rungs for name, lv in zip(row["name"].split(" · "), row["levels"])}
        self.assertEqual((levels["Settler"], levels["Prince"], levels["King"], levels["Deity"]), (1, 4, 5, 8))
        self.assertEqual(sorted(levels.values()), list(range(1, 9)))
        # Civilization VI's own AI at a level is named here as the viewer
        # names it beside a live game: number, name, then whose AI it is
        # ("8 Deity AI Firaxis"). The words after the level are read out of
        # the engine's template, so the two cannot come to disagree.
        template = re.search(r'format!\("\{\} \{\}( [^"]+)", at \+ 1, CIV6_RUNGS\[at\]\.1\)', strategies_rs)
        self.assertEqual(template[1], " AI Firaxis")
        self.assertIn(f'row("is-rung", "", level(r) + "{template[1]}", "", elo(r), "",', CHARTS)
        self.assertIn('return low + (high > low ? "\\u2013" + high : "") + " " + r.name;', CHARTS)
        self.assertNotIn('"Civ VI "', CHARTS)
        named = [name for row in rungs for name in row["name"].split(" · ")]
        self.assertEqual(sorted(named), sorted(rung.title() for rung in published["ladder"]))

    def test_the_civs_are_ranked_by_win_rate_over_the_recorded_games(self):
        # How a game is counted, over every level and every day; which games
        # the page's table counts is the next test.
        names = ["Greece", "Korea", "Gran Colombia", "Ottomans", "Babylon", "Rome"]
        rows = [
            # Greece: two wins at two levels, and a loss of each kind.
            run("g1", "win", "GREECE", victory="VICTORY_CULTURE", utc="2026-11-18T05:00:00Z"),
            run("g2", "win", "GREECE", victory="VICTORY_DIPLOMATIC", difficulty="DIFFICULTY_KING"),
            run("g3", "rival_victory", "GREECE", victory="VICTORY_TECHNOLOGY"),
            run("g4", "defeat", "GREECE"),
            run("g5", "abandoned", "GREECE"),
            # A game somebody stopped is not a result, however many there are.
            run("g6", "unfinished", "GREECE", utc="2026-12-25T00:00:00Z"),
            run("g7", "unfinished", "GREECE"),
            # Korea: one game in three runs. The first two were parked and
            # reloaded, the first before the run recorded its civ; the game
            # is the win its last run ended on, once.
            run("k1", "unfinished"),
            run("k1-cont1", "unfinished", "KOREA"),
            run("k1-cont2", "win", "KOREA", victory="VICTORY_TECHNOLOGY"),
            run("k2", "win", "KOREA", victory="VICTORY_TECHNOLOGY"),
            # Two wins each: the better win rate stands higher, and Korea's
            # two wins without a loss stand above one win without a loss.
            run("c1", "win", "GRAN_COLOMBIA", victory="VICTORY_CONQUEST", difficulty="DIFFICULTY_EMPEROR"),
            run("c2", "win", "GRAN_COLOMBIA", victory="VICTORY_CONQUEST", difficulty="DIFFICULTY_EMPEROR"),
            run("c3", "abandoned", "GRAN_COLOMBIA"),
            # One win each, no losses: by name. Neither type title-cases
            # into the engine's name for the civ.
            run("o1", "win", "OTTOMAN", victory="VICTORY_DIPLOMATIC", difficulty="DIFFICULTY_IMMORTAL"),
            run("b1", "win", "BABYLON_STK", victory="VICTORY_TECHNOLOGY"),
            # Played and never won: counted as played, not listed.
            run("r1", "rival_victory", "ROME"),
            # A win whose game recorded no civ belongs to no row.
            run("x1", "win", victory="VICTORY_SCORE", difficulty="DIFFICULTY_SETTLER"),
            # A line in words this reader does not know is not a result.
            run("z1", "forfeit", "ROME"),
        ]
        ranking = home_rankings.civ_ranking(rows, names, civ6_level=None, since=None)
        self.assertEqual(
            [(row["name"], row["wins"], row["losses"]) for row in ranking["rows"]],
            [("Korea", 2, 0), ("Babylon", 1, 0), ("Ottomans", 1, 0), ("Gran Colombia", 2, 1), ("Greece", 2, 3)],
        )
        by_name = {row["name"]: row for row in ranking["rows"]}
        self.assertEqual(by_name["Greece"]["won"], ["Lv 8 Culture", "Lv 5 Diplomatic"])
        self.assertEqual(by_name["Gran Colombia"]["won"], ["Lv 6 Domination \u00d72"])
        self.assertEqual(by_name["Korea"]["won"], ["Lv 8 Science \u00d72"])
        self.assertEqual([by_name[name]["best"] for name in ("Greece", "Gran Colombia", "Ottomans")], [8, 6, 7])
        self.assertEqual(by_name["Ottomans"]["civ"], "CIVILIZATION_OTTOMAN")
        self.assertEqual(
            {key: value for key, value in ranking.items() if key != "rows"},
            {
                "level": None, "since": None,
                "games": 14, "wins": 9, "unfinished": 2, "unattributed_wins": 1,
                "played": 6, "winners": 5,
                # The last finished game's day; a stopped game does not move it.
                "through": "2026-11-18",
            },
        )
        # Nothing finished is nothing to show.
        self.assertIsNone(home_rankings.civ_ranking([run("g6", "unfinished", "GREECE")], names))

    def test_the_civ_table_counts_the_lv_8_games_from_2026_11_16(self):
        # Operator 2026-11-19: "our winningest civs ranking should be by win %.
        # and just limited to lv 8 games, starting from 11/16".
        self.assertEqual((home_rankings.CIVS_LEVEL, home_rankings.CIVS_SINCE), (8, "2026-11-16"))
        names = ["Greece", "Korea", "Rome", "Mali"]
        rows = [
            # Greece, 1-1: a game launched in the day's first minutes counts.
            run("civvis-20261116T000500Z", "win", "GREECE", victory="VICTORY_CULTURE", utc="2026-11-16T00:20:00Z"),
            run("civvis-20261117T120000Z", "rival_victory", "GREECE", utc="2026-11-17T12:15:00Z"),
            # One launched the evening before does not, though it ended on the day.
            run("civvis-20261115T235500Z", "win", "GREECE", victory="VICTORY_CULTURE", utc="2026-11-16T00:10:00Z"),
            # Nor does an older game reloaded since: a game begins once.
            run("civvis-20261114T100000Z", "unfinished", "GREECE", utc="2026-11-14T10:30:00Z"),
            run("civvis-20261114T100000Z-cont1", "win", "GREECE", victory="VICTORY_SCORE", utc="2026-11-18T09:00:00Z"),
            # A stopped game counts as stopped only inside the window.
            run("civvis-20261117T140000Z", "unfinished", "GREECE", utc="2026-11-17T14:05:00Z"),
            run("civvis-20261115T140000Z", "unfinished", "GREECE", utc="2026-11-15T14:05:00Z"),
            # Rome, 0-1: a win below Lv 8 is not counted, whenever it was.
            run("civvis-20261118T100000Z", "win", "ROME", victory="VICTORY_SCORE", difficulty="DIFFICULTY_IMMORTAL",
                utc="2026-11-18T10:20:00Z"),
            run("civvis-20261118T110000Z", "defeat", "ROME", utc="2026-11-18T11:20:00Z"),
            # Korea, 1-0: the best rate leads, on one win.
            run("civvis-20261118T120000Z", "win", "KOREA", victory="VICTORY_TECHNOLOGY", utc="2026-11-18T12:20:00Z"),
            # Mali, 2-2: Greece's rate on more wins, so above Greece. A tag
            # with no stamp is placed by the day its first run was recorded.
            run("probe-a", "win", "MALI", victory="VICTORY_SCORE", utc="2026-11-16T08:00:00Z"),
            run("probe-b", "win", "MALI", victory="VICTORY_SCORE", utc="2026-11-15T23:00:00Z"),
            run("civvis-20261119T090000Z", "win", "MALI", victory="VICTORY_TECHNOLOGY", utc="2026-11-19T09:20:00Z"),
            run("civvis-20261119T100000Z", "abandoned", "MALI", utc="2026-11-19T10:05:00Z"),
            run("civvis-20261119T110000Z", "defeat", "MALI", utc="2026-11-19T11:20:00Z"),
        ]
        ranking = home_rankings.civ_ranking(rows, names)
        self.assertEqual(
            [(row["name"], row["wins"], row["losses"]) for row in ranking["rows"]],
            [("Korea", 1, 0), ("Mali", 2, 2), ("Greece", 1, 1)],
        )
        self.assertEqual(
            {key: value for key, value in ranking.items() if key != "rows"},
            {
                "level": 8, "since": "2026-11-16",
                "games": 8, "wins": 4, "unfinished": 1, "unattributed_wins": 0,
                "played": 4, "winners": 3, "through": "2026-11-19",
            },
        )
        # The level and the day are each their own bound.
        immortal = home_rankings.civ_ranking(rows, names, civ6_level=7, since=None)
        self.assertEqual([(row["name"], row["wins"], row["losses"]) for row in immortal["rows"]], [("Rome", 1, 0)])
        every_day = home_rankings.civ_ranking(rows, names, since=None)
        self.assertEqual(
            [(row["name"], row["wins"], row["losses"]) for row in every_day["rows"]],
            [("Korea", 1, 0), ("Greece", 3, 1), ("Mali", 3, 2)],
        )
        # The page leads a row with the rate it is ranked by, the record
        # beside it, and says under the title which games were counted.
        for line in (
            '(100 * c.wins / (c.wins + c.losses)).toFixed(1) + "%", count(c.wins) + "\\u2013" + count(c.losses),',
            'sub.textContent = "Our AI\\u2019s win rate as each civ, in " + (civs.level ? "Lv " + civs.level + " " : "")',
            '+ "verification games against Civilization VI\\u2019s own AI" + (civs.since ? " since " : "");',
            'if (civs.since) sub.append(el("span", "rank-day", civs.since));',
        ):
            self.assertIn(line, CHARTS)

    def test_the_civ_table_shows_the_winners_and_stops_at_its_length(self):
        rows = [run(f"w{n}", "win", f"CIV_{n:02}", victory="VICTORY_SCORE") for n in range(home_rankings.TOP_CIVS + 3)]
        rows += [run(f"w{n}-again", "win", f"CIV_{n:02}", victory="VICTORY_SCORE") for n in range(2)]
        ranking = home_rankings.civ_ranking(rows, [])
        self.assertEqual(len(ranking["rows"]), home_rankings.TOP_CIVS)
        self.assertEqual(ranking["winners"], home_rankings.TOP_CIVS + 3)
        self.assertEqual([row["name"] for row in ranking["rows"][:3]], ["Civ 00", "Civ 01", "Civ 02"])

    def test_a_civ_reads_under_the_engines_own_name(self):
        names = list(json.loads((ROOT / "data" / "civs.json").read_text(encoding="utf-8")))
        for civ, name in (
            ("CIVILIZATION_GREECE", "Greece"),
            ("CIVILIZATION_GRAN_COLOMBIA", "Gran Colombia"),
            ("CIVILIZATION_OTTOMAN", "Ottomans"),
            ("CIVILIZATION_BABYLON_STK", "Babylon"),
        ):
            self.assertEqual(home_rankings.civ_name(civ, names), name)
            self.assertIn(name, names)
        # A type the engine has no civ for keeps its plain reading.
        self.assertEqual(home_rankings.civ_name("CIVILIZATION_LOST_ATLANTIS", names), "Lost Atlantis")
        self.assertEqual([home_rankings.level(f"DIFFICULTY_{rung.upper()}") for rung in ("settler", "king", "deity")], [1, 5, 8])
        self.assertIsNone(home_rankings.level(None))

    def test_the_results_are_read_off_the_fetched_ledger_branch(self):
        rows = [
            run("g1", "win", "GREECE", victory="VICTORY_CULTURE"),
            run("g2", "rival_victory", "GREECE", victory="VICTORY_TECHNOLOGY"),
        ]
        with tempfile.TemporaryDirectory() as scratch:
            clone = pathlib.Path(scratch) / "clone"
            (clone / "data").mkdir(parents=True)
            (clone / "data" / "civs.json").write_text(json.dumps({"Greece": {}}), encoding="utf-8")
            with_ledger(clone, rows)
            self.assertEqual(home_rankings.results(clone), rows)
            ranking = home_rankings.rankings(clone)["civs"]
            self.assertEqual([(row["name"], row["wins"], row["losses"]) for row in ranking["rows"]], [("Greece", 1, 1)])
            # A clone that never fetched the branch, and a ledger from before
            # the results file, both read as no record: the table stays off.
            bare = pathlib.Path(scratch) / "bare"
            bare.mkdir()
            git(bare, "init", "-q")
            self.assertIsNone(home_rankings.civs(bare))
            tree = git(bare, "mktree", stdin="")
            git(bare, "update-ref", "refs/remotes/origin/ledger", git(bare, "commit-tree", tree, "-m", "runs only"))
            self.assertIsNone(home_rankings.civs(bare))
            # A line this reader cannot count costs the page its civ table
            # and is said; it never stops the script that publishes the site.
            odd = pathlib.Path(scratch) / "odd"
            odd.mkdir()
            with_ledger(odd, [run("g1", "win", "GREECE"), {**run("g2", "win", "GREECE"), "game": ["not", "a", "tag"]}])
            said = io.StringIO()
            with contextlib.redirect_stderr(said):
                self.assertIsNone(home_rankings.civs(odd))
                self.assertEqual(home_rankings.rankings(odd), {"civs": None, "strategies": None, "tournaments": None})
            self.assertIn("could not be ranked", said.getvalue())
        # The publish workflow is what fetches it: every branch, on checkout.
        workflow = (ROOT / ".github" / "workflows" / "to-test-auto-30.yml").read_text(encoding="utf-8")
        self.assertIn("fetch-depth: 0", workflow)
        self.assertEqual(home_rankings.RESULTS_REF, f"refs/remotes/origin/{civ6_ladder.LEDGER_BRANCH}:{civ6_ladder.RESULTS_PATH}")

    def test_the_reader_counts_in_the_ladders_own_words(self):
        # `tools/civ6_ladder.py` writes the lines; this file only counts them.
        self.assertEqual(home_rankings.OUTCOMES, list(civ6_ladder.OUTCOMES))
        self.assertLess(home_rankings.LOSSES, set(home_rankings.OUTCOMES))
        line = civ6_ladder.result_row(civ6_ladder.result_entry({
            "tag": "civvis-1-cont2", "finished_utc": "2026-11-19T10:00:00Z", "difficulty": "DIFFICULTY_DEITY",
            "outcome": {"kind": "victory", "won": True, "victory": 3},
            "seat": {"civ": "CIVILIZATION_GREECE", "victory_types": [{"index": 3, "type": "VICTORY_CULTURE"}]},
        }))
        ranking = home_rankings.civ_ranking([line], ["Greece"])
        self.assertEqual(ranking["rows"], [{
            "civ": "CIVILIZATION_GREECE", "name": "Greece", "wins": 1, "losses": 0, "best": 8, "won": ["Lv 8 Culture"],
        }])
        self.assertEqual(ranking["through"], "2026-11-19")

    def test_the_page_carries_one_empty_slot_and_publishing_fills_it(self):
        self.assertEqual(CHARTS.count('<script type="application/json" id="rankings-data">null</script>'), 1)
        data = home_rankings.rankings(ROOT)
        filled = home_rankings.fill(CHARTS, data)
        slot = home_rankings.SLOT.search(filled)
        self.assertEqual(json.loads(slot[2]), data)
        # Nothing but the slot moved.
        self.assertEqual(home_rankings.SLOT.sub("", filled), home_rankings.SLOT.sub("", CHARTS))
        # The publish script is what runs it, against the lane's own revision:
        # on this page, and on the home page, which had the slot until
        # 2026-11-22 and still has it in a lane pinned to an older revision.
        publish = (BETA / "publish.sh").read_text(encoding="utf-8")
        self.assertIn('home_rankings.py" "$source_tree" "$out/research/index.html"', publish)
        self.assertIn('home_rankings.py" "$source_tree" "$out/home/index.html"', publish)

    def test_the_two_rankings_stand_to_the_right_of_the_graphs_strategies_over_civs(self):
        # Operator 2026-11-22: "move 2 charts on home/ to charts/", and later
        # that day, of Win Analytics: "player strategies ranking should be
        # off to the right side of the graphs mentioned above. The winningest
        # civs on the right below the other table. other tables can move up
        # accordingly". The two lists that were the home page's two columns
        # are one column of Win Analytics, to the right of its graphs: the
        # Civ AI Agents by Elo, and under them the winningest civs. What
        # follows the graphs, each victory's timing and the figures, comes
        # straight after.
        at = [
            CHARTS.index(piece)
            for piece in (
                '<div class="res-layout">',
                '<div class="res-graphs">',
                'id="res-kind-self_play"',
                'id="res-kind-ai-civvis"',
                '<aside class="res-lists" aria-label="Rankings">',
                '<section class="res-section rank-card" id="rank-strategies" hidden aria-labelledby="rank-strategies-title">',
                '<section class="res-section rank-card" id="rank-civs" hidden aria-labelledby="rank-civs-title">',
                "</aside>",
                'id="res-each"',
                'id="res-table"',
            )
        ]
        self.assertEqual(at, sorted(at))
        # Nothing stands between the first screen and the sections after it.
        self.assertRegex(CHARTS, r"</aside>\s*</div>\s*<section class=\"res-section\" id=\"res-each\"")
        # Two columns, the graphs' twice the lists' width, both starting at
        # the top; one column, the graphs first, under 860px.
        self.assertIn(".res-layout { display: grid; grid-template-columns: minmax(0, 2fr) minmax(0, 1fr); gap: 16px; align-items: start; min-width: 0; }", CHARTS)
        self.assertIn("@media (max-width: 859px) { .res-layout { grid-template-columns: minmax(0, 1fr); } }", CHARTS)
        self.assertIn(".res-graphs, .res-lists { display: grid; align-content: start; gap: 16px; min-width: 0; }", CHARTS)
        self.assertNotIn("res-sides", CHARTS)
        for line in (
            "const civs = data.civs;",
            'show("rank-civs", rows, note);',
            "const strategies = data.strategies;",
            'show("rank-strategies", rows, note);',
        ):
            self.assertIn(line, CHARTS)
        # The Tournaments table stood above the two lists until the operator
        # gave tournaments their own page (2026-11-21: "The tournament tables
        # on home right now can move to Tournaments"). Nothing of it came
        # along but the link to that page (`tournaments_page_test.py`).
        for gone in ("rank-tournaments", "is-wide", "data.tournaments", "toursShown"):
            self.assertNotIn(gone, CHARTS)
            self.assertNotIn(gone, LANDING)
        self.assertIn('<a class="site-link" href="../tournaments/"', CHARTS)
        # Every game behind the table is one click away, on the branch the
        # table is read from.
        self.assertIn(
            f'href="https://github.com/MartinHalvorson/CIVVIS/blob/{civ6_ladder.LEDGER_BRANCH}/{civ6_ladder.RESULTS_PATH}"',
            CHARTS,
        )
        # And the home page has neither list left, nor their slot.
        for gone in ("rankings-data", "rank-civs", "rank-strategies", "rank-list", 'id="rankings"'):
            self.assertNotIn(gone, LANDING)

    def test_the_home_page_is_four_videos_two_by_two(self):
        # Operator 2026-11-22: "On home, we can have our fastest game youtube
        # video embedded to watch in the upper left. in upper right, we can
        # have a single player pov game (should be under performance). Maybe
        # the AI Debugger - Multiverse, Time Travel video in the lower right.
        # lower left please save for a lv 8 victory video lv 8 maya cluture
        # from the youtube." And later that day: "swap the positions of the
        # bottom 2 videos to the Maya Culture win is lower right", "Make the
        # video Titles above the video and bigger or more readable", "have
        # the home upper right video start at 6 seconds in".
        tiles = re.findall(
            r'<figure class="watch-tile" id="(watch-[a-z]+)" data-video="([\w-]{11})"( data-start="\d+")?>\s*'
            r"<figcaption><b>([^<]+)</b> <span data-watch-title>([^<]+)</span> <a [^>]+>[^<]+</a></figcaption>\s*"
            r'<a class="watch-poster" href="https://www\.youtube\.com/watch\?v=([\w-]{11})((?:&amp;t=\d+s)?)" target="_blank" rel="noreferrer">\s*'
            r'<img alt="" width="1280" height="720" decoding="async" referrerpolicy="no-referrer" '
            r'src="https://i\.ytimg\.com/vi_webp/([\w-]{11})/hq720\.webp">',
            LANDING,
        )
        # Two columns, so document order reads upper left, upper right,
        # lower left, lower right. A tile's title is the first thing in it,
        # over its video, which the pattern above reads in that order.
        self.assertEqual(
            [(ident, video, label, title) for ident, video, _, label, title, _, _, _ in tiles],
            [
                ("watch-fastest", "iHlUWPIqr5g", "Fastest game", "A complete Civ 6 match in under 6 seconds, real-time simulation"),
                ("watch-pov", "9c8m87e3rCg", "Single player POV", "A full Civ 6 match in 14 seconds"),
                ("watch-debugger", "6W5t0hcR_N4", "AI Debugger", "Multiverse, Time Travel"),
                ("watch-victory", "6PCd4R08UPM", "Lv 8 Deity win", "Maya Culture"),
            ],
        )
        for _, video, _, _, _, link, _, picture in tiles:
            self.assertEqual((link, picture), (video, video))
        # The single player's game starts 6 seconds in, on the page and on
        # YouTube; the other three start at their start.
        self.assertEqual(
            [(start, at) for _, _, start, _, _, _, at, _ in tiles],
            [("", ""), (' data-start="6"', "&amp;t=6s"), ("", ""), ("", "")],
        )
        self.assertIn('const start = /^[1-9]\\d*$/.test(tile.dataset.start || "") ? tile.dataset.start : "";', LANDING)
        self.assertIn('poster.href = "https://www.youtube.com/watch?v=" + id + (start ? "&t=" + start + "s" : "");', LANDING)
        # The titles are the largest type under the page's name, and whole.
        self.assertIn(".watch-tile figcaption b { color: #fff; font-size: 20px; font-weight: 700; line-height: 1.3; }", LANDING)
        self.assertIn(".watch-tile figcaption span { order: 3; flex-basis: 100%; min-width: 0; color: var(--text); font-size: 15px; line-height: 1.4; }", LANDING)
        self.assertNotIn("text-overflow", LANDING[LANDING.index(".watch-grid {") : LANDING.index("</style>")])
        # The section has no heading over it (operator: "Drop "watch" title
        # or whatever from this section").
        self.assertIn('<section class="watch" id="watch" aria-label="Videos">\n      <div class="watch-grid">', LANDING)
        for gone in ('id="watch-title"', 'class="watch-title"', "aria-labelledby=\"watch", "watch-head", ">Watch<"):
            self.assertNotIn(gone, LANDING)
        self.assertEqual(LANDING.count('class="watch-tile"'), 4)
        self.assertIn(".watch-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr));", LANDING)
        self.assertIn("@media (max-width: 639px) { .watch-grid { grid-template-columns: minmax(0, 1fr); } }", LANDING)
        # The single player's game is one of the Performance page's records.
        records = performance_page.videos(json.loads((ROOT / performance_page.VIDEOS).read_text(encoding="utf-8"))["videos"])
        self.assertIn("Single Player POV", {v["id"]: v["title"] for v in records}["9c8m87e3rCg"])
        # The videos stay YouTube's (operator 2026-11-21: "website is a fast
        # and light surface layer"): as written a tile is a picture and a
        # link, and the script puts the video's player in the picture's place
        # when the tile is on screen, playing without sound (operator
        # 2026-11-22: "please have the videos autoplay if possible (no
        # audio)"). A picture pressed first plays with its sound.
        markup = LANDING[: LANDING.index('<script type="application/json" id="home-videos">')]
        self.assertNotIn("youtube-nocookie", markup)
        self.assertNotIn("<iframe", markup)
        self.assertIn(
            'player.src = "https://www.youtube-nocookie.com/embed/" + id + "?autoplay=1&rel=0&playsinline=1"'
            ' + (muted ? "&mute=1" : "") + (start ? "&start=" + start : "");',
            LANDING,
        )
        self.assertIn("starts.get(entry.target)(true);", LANDING)
        self.assertIn("if (seen) { starts.set(tile, play); seen.observe(tile); }", LANDING)
        self.assertIn("play(false);", LANDING)
        # Whoever asked their system for less motion keeps the pictures.
        self.assertIn('const still = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;', LANDING)
        self.assertIn('const seen = !still && "IntersectionObserver" in window && new IntersectionObserver(', LANDING)
        self.assertIn("if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button) return;", LANDING)
        self.assertNotIn("innerHTML", LANDING)

    def test_the_fastest_game_follows_the_record_list(self):
        # The upper left tile is whichever record leads
        # data/performance/fastest_game_videos.json, the list the Performance
        # page shows: publishing writes it into the page's own slot, so a new
        # record reaches the home page from its one line in that file.
        self.assertEqual(LANDING.count('<script type="application/json" id="home-videos">null</script>'), 1)
        records = performance_page.videos(json.loads((ROOT / performance_page.VIDEOS).read_text(encoding="utf-8"))["videos"])
        fastest = {"fastest": {key: records[0][key] for key in ("id", "title", "time")}}
        self.assertEqual(home_rankings.videos(ROOT), fastest)
        self.assertEqual(records[0]["seconds"], min(v["seconds"] for v in records))
        with tempfile.TemporaryDirectory() as scratch:
            page = pathlib.Path(scratch) / "index.html"
            page.write_text(LANDING, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()) as said:
                self.assertEqual(home_rankings.main(["home_rankings.py", str(ROOT), str(page)]), 0)
            self.assertIn("the fastest game's video", said.getvalue())
            filled = page.read_text(encoding="utf-8")
            self.assertEqual(json.loads(home_rankings.VIDEOS_SLOT.search(filled)[2]), fastest)
            # Nothing but the page's slots moved: this one, and the speed
            # timeline's (`beta/speedup_timeline_test.py` reads that one).
            bare = lambda text: home_rankings.speedup_timeline.SLOT.sub("", home_rankings.VIDEOS_SLOT.sub("", text))  # noqa: E731
            self.assertEqual(bare(filled), bare(LANDING))
            # A revision with no list of records: the slot says so, and the
            # tile stays the video it is written with.
            self.assertIsNone(home_rankings.videos(pathlib.Path(scratch)))
            page.write_text(LANDING, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(home_rankings.main(["home_rankings.py", scratch, str(page)]), 0)
            self.assertIsNone(json.loads(home_rankings.VIDEOS_SLOT.search(page.read_text(encoding="utf-8"))[2]))
        for line in (
            "const fastest = data && data.fastest;",
            "if (fastest && VIDEO_ID.test(fastest.id)) {",
            "tile.dataset.video = fastest.id;",
            'if (fastest.title) tile.querySelector("[data-watch-title]").textContent = fastest.title;',
        ):
            self.assertIn(line, LANDING)

    def test_github_and_youtube_stand_out_and_the_top_playlists_are_listed_under_youtube(self):
        # Operator 2026-11-22: "make the GitHub, dev progress stand out a bit
        # more. at (everything open source, free) to the github part. Dev
        # Progress button can change to "YouTube". We should have direct
        # links to watching our top playlists: Lv 8 Deity Wins, Lv 1 - 7
        # Archive." And later that day: "list top playlists under youtube.
        # maybe in a vertical list".
        for name, page in PAGES.items():
            self.assertIn("<span>GitHub <small>Everything open source, free</small></span>", page, name)
            self.assertRegex(
                page,
                r'<a class="utility-link is-github" href="https://github\.com/MartinHalvorson/CIVVIS" target="_blank" rel="noreferrer">',
                name,
            )
            self.assertRegex(
                page,
                r'<a class="utility-link is-youtube" href="https://www\.youtube\.com/@civvis" target="_blank" rel="noreferrer">'
                r"\s*<svg[^>]*>.*?</svg>\s*YouTube\s*</a>",
                name,
            )
            self.assertNotIn("Dev Progress", page, name)
            self.assertIn(".utility-link.is-github { border-color: var(--accent); background: var(--action-bg); }", page, name)
            self.assertIn(".utility-link.is-youtube { border-color: var(--yt-edge); background: var(--yt); }", page, name)
            # The playlists are a list under the YouTube button, one to a
            # line, in the header of every page: the button and its list are
            # one group of the row, and the list is all the group holds
            # after the button.
            group = re.search(
                r'<div class="utility-group">\s*<a class="utility-link is-youtube" [^>]+>.*?</a>\s*'
                r'<ul class="utility-lists" aria-label="Top playlists">(.*?)</ul>\s*</div>',
                page,
                re.S,
            )
            self.assertIsNotNone(group, name)
            lists = re.findall(
                r'<li><a class="utility-list" href="https://www\.youtube\.com/playlist\?list=([\w-]+)" target="_blank" rel="noreferrer">'
                r"<svg[^>]*>.*?</svg>([^<]+)</a></li>",
                group[1],
            )
            self.assertEqual(lists, [("PLWcKlP8e8iy4", "Lv 8 Deity Wins"), ("PLecirlzIUSSo", "Lv 1 - 7 Archive")], name)
            self.assertEqual(group[1].count("<li>"), 2, name)
            self.assertIn(".utility-group { display: grid; gap: 5px; }", page, name)
            self.assertIn(".utility-lists { display: grid; gap: 1px; margin: 0; padding: 0; list-style: none; }", page, name)
            # The row's links keep their tops in a line, so the list hangs
            # under its own button and stretches nothing.
            self.assertIn(
                ".utility-nav { display: flex; flex-wrap: wrap; align-items: flex-start; justify-content: flex-end; gap: 10px; margin-left: auto; }",
                page,
                name,
            )
            self.assertNotIn("watch-list", page, name)

    def test_the_page_lists_no_heuristic_genes(self):
        # The home page once carried the head of the gene ranking under the
        # strategies. Neither the page nor the slot it is filled from has
        # one now: no card, no reader, no style or script left behind.
        self.assertEqual(sorted(home_rankings.rankings(ROOT)), ["civs", "strategies", "tournaments"])
        self.assertFalse(hasattr(home_rankings, "genes"))
        self.assertEqual(re.findall(r'id="(rank-[a-z]+)"', CHARTS), ["rank-strategies", "rank-civs"])
        page = CHARTS.lower()
        for gone in ("heuristic gene", "gene_heuristic_ranking", "data.genes", "is-gene", "is-off", "rank-stack", "has-civs"):
            self.assertNotIn(gone, page)
        # A lane pinned to a revision whose page still has the gene card is
        # published by this script too, and its card stays empty and hidden:
        # that page shows a list only when the slot holds its key.
        self.assertNotIn("genes", json.loads(home_rankings.SLOT.search(
            home_rankings.fill(CHARTS, home_rankings.rankings(ROOT)))[2]))

    def test_a_tournament_is_series_of_finished_games_newest_first(self):
        spec = {"name": "Lv 8 Deity series", "conditions": "Lv 8 6P Continents Online",
                "start": "civvis-20261119T184500Z", "series_size": 3,
                "match": {"difficulty": "DIFFICULTY_DEITY", "players": 6, "map": "Continents.lua"}}
        on = {"players": 6, "map": "Continents.lua"}
        rows = [
            run("civvis-20261119T180000Z", "win", "ROME", **on),            # before the start
            run("civvis-20261119T190000Z", "abandoned", "ROME", utc="2026-11-19T19:10:00Z", **on),
            run("civvis-20261119T191500Z", "rival_victory", "GAUL", utc="2026-11-19T19:25:00Z", **on),
            run("civvis-20261119T191500Z-cont1", "win", "GAUL", utc="2026-11-19T19:40:00Z", **on),  # best run counts
            run("civvis-20261119T194500Z", "unfinished", None, players=None, map=None),           # never seated
            run("civvis-20261119T195000Z", "win", "KOREA", utc="2026-11-19T20:05:00Z", **on),
            run("civvis-20261119T201000Z", "defeat", "MALI", utc="2026-11-19T20:20:00Z", **on),
            run("civvis-20261119T202000Z", "win", "MALI", utc="2026-11-19T20:30:00Z", players=4, map="Pangaea.lua"),
        ]
        series = home_rankings.tournament_series(rows, [spec])
        self.assertEqual([(s["id"], s["series"], s["games"], s["wins"], s["losses"]) for s in series],
                         [(2, 2, 1, 0, 1), (1, 1, 3, 2, 1)])
        self.assertEqual(series[1]["start"], "2026-11-19 19:00Z")
        self.assertEqual(series[1]["end"], "2026-11-19 20:05Z")
        self.assertIsNone(series[0]["end"])                    # a series still being played
        self.assertEqual(series[0]["conditions"], "Lv 8 6P Continents Online")
        self.assertEqual([s["note"] for s in series], ["", ""])  # no note, an empty Notes cell
        self.assertIsNone(home_rankings.tournament_series([], [spec]))

    def test_an_end_closes_a_definition_and_the_next_one_counts_on(self):
        # Operator 2026-11-19 ~21:20Z: "Turn the ongoing tournament into a 100
        # game tournament": the 10-game definition ends where the 100-game one
        # starts, and the ids run on across both.
        match = {"difficulty": "DIFFICULTY_DEITY", "players": 6, "map": "Continents.lua"}
        tens = {"name": "tens", "conditions": "c", "start": "civvis-20261119T180000Z",
                "end": "civvis-20261119T200000Z", "series_size": 2, "match": match,
                "note": "Scrapped: random civ, random strategy."}
        hundreds = {"name": "hundreds", "conditions": "c", "start": "civvis-20261119T200000Z",
                    "series_size": 5, "match": match}
        on = {"players": 6, "map": "Continents.lua"}
        rows = [run(f"civvis-20261119T{h:02d}{m:02d}00Z", outcome, "ROME", utc=f"2026-11-19T{h:02d}:{m + 5:02d}:00Z", **on)
                for h, m, outcome in ((18, 0, "win"), (18, 30, "defeat"), (19, 0, "defeat"),
                                      (20, 0, "rival_victory"), (20, 30, "win"), (21, 0, "defeat"))]
        series = home_rankings.tournament_series(rows, [tens, hundreds])
        self.assertEqual([(s["id"], s["tournament"], s["games"], s["size"], s["wins"], s["losses"], s["end"] is None)
                          for s in series],
                         [(3, "hundreds", 3, 5, 1, 2, True), (2, "tens", 1, 2, 0, 1, False), (1, "tens", 2, 2, 1, 1, False)])
        # The short series the `end` closed has an end date: its last game's.
        self.assertEqual(series[1]["end"], "2026-11-19 19:05Z")
        # Operator 2026-11-20: "tournament 2 was scraped. (can add a notes
        # col)": a definition's note rides on every one of its series.
        self.assertEqual([s["note"] for s in series], ["", "Scrapped: random civ, random strategy.",
                                                        "Scrapped: random civ, random strategy."])

    def test_a_table_owning_a_tag_prefix_does_not_close_the_series_it_interrupts(self):
        # Operator 2026-11-21: Tournament 6 is a hotseat table tagged
        # `civvis-t6-...`; the single-seat definition ends at "civvis-t" and
        # stays open until a game past that end is played after its last one.
        match = {"players": 6, "map": "Continents.lua"}
        seat = {"name": "seat", "conditions": "c", "start": "civvis-20261120T000000Z",
                "end": "civvis-t", "series_size": 3, "match": match}
        table = {"name": "table", "conditions": "c", "start": "civvis-t6-", "end": "civvis-t7-",
                 "series_size": 2, "match": match}
        on = {"players": 6, "map": "Continents.lua"}
        rows = [run("civvis-20261120T010000Z", "win", "ROME", utc="2026-11-20T01:10:00Z", **on),
                run("civvis-20261120T020000Z", "defeat", "ROME", utc="2026-11-20T02:10:00Z", **on)]
        series = home_rankings.tournament_series(rows, [seat, table])
        self.assertEqual([(s["tournament"], s["games"], s["end"]) for s in series], [("seat", 2, None)])
        rows.append(run("civvis-t6-20261120T030000Z", "win", "GAUL", utc="2026-11-20T03:10:00Z", **on))
        series = home_rankings.tournament_series(rows, [seat, table])
        self.assertEqual([(s["id"], s["tournament"], s["games"], s["end"] is None) for s in series],
                         [(2, "table", 1, True), (1, "seat", 2, False)])

    def test_an_indefinite_tournament_never_fills_and_a_tournament_between_its_games_pauses_it(self):
        # Operator 2026-11-22: "please add an indefinite tourney - whenever we
        # have no other tourney, we run lv 6, lv 7, lv 8 (x 2) games".
        on = {"players": 6, "map": "Continents.lua"}
        bench = {"name": "bench", "conditions": "c", "start": "civvis-t7-", "end": "civvis-t8-",
                 "indefinite": True, "match": on}
        later = {"name": "later", "conditions": "c", "start": "civvis-t8-", "end": "civvis-t9-",
                 "series_size": 2, "match": on}
        rows = [run(f"civvis-t7-20261122T{h:02d}0000Z", "win" if h % 3 else "rival_victory", "ROME",
                    utc=f"2026-11-22T{h:02d}:40:00Z", **on) for h in range(12)]
        rows.append(run("civvis-t8-20261122T120000Z", "win", "GAUL", utc="2026-11-22T12:40:00Z", **on))
        rows.append(run("civvis-t7-20261122T130000Z", "win", "MALI", utc="2026-11-22T13:40:00Z", **on))
        series = home_rankings.tournament_series(rows, [bench, later])
        self.assertEqual([(s["id"], s["tournament"], s["games"], s["size"], s["end"]) for s in series],
                         [(2, "later", 1, 2, None), (1, "bench", 13, None, None)])
        self.assertEqual(series[1]["wins"], 9)
        # Before its first game it is one tournament still to come, not ten.
        self.assertIsNone(home_rankings.tournament_series([], [bench]))

    def test_the_tournament_definitions_are_committed_and_read(self):
        specs = json.loads((ROOT / "data" / "tournaments.json").read_text(encoding="utf-8"))["tournaments"]
        self.assertTrue(specs)
        for spec in specs:
            # A run tag (a hotseat table's carries its tournament's prefix,
            # `civvis-t6-<utc>`), or a tag prefix a whole table owns (`civvis-t6-`).
            tag = r"^civvis-((t\d+-)?\d{8}T\d{6}Z|t\d*-?)$"
            self.assertRegex(spec["start"], tag)
            if "end" in spec:
                self.assertRegex(spec["end"], tag)
                self.assertGreater(spec["end"], spec["start"])
            if spec.get("indefinite"):
                # One series that never fills: a size would contradict it.
                self.assertNotIn("series_size", spec)
            else:
                self.assertGreater(spec["series_size"], 0)
            self.assertTrue(spec["conditions"])
            if "note" in spec:
                self.assertIsInstance(spec["note"], str)
                self.assertTrue(spec["note"])
        # Operator 2026-11-20: Tournament 2 (the 30-game series closed at
        # civvis-20261119T223800Z) was scrapped.
        scrapped = [spec for spec in specs if spec.get("end") == "civvis-20261119T223800Z"]
        self.assertEqual(len(scrapped), 1)
        self.assertTrue(scrapped[0]["note"].startswith("Scrapped"))

    def test_a_sentence_cannot_close_the_slot(self):
        data = {"civs": None, "strategies": {"rows": [{"tilt": "</script><b>x"}]}}
        filled = home_rankings.fill(CHARTS, data)
        self.assertEqual(filled.count("</script>"), CHARTS.count("</script>"))
        self.assertEqual(json.loads(home_rankings.SLOT.search(filled)[2]), data)

    def test_an_older_revision_publishes_without_either_ranking(self):
        with tempfile.TemporaryDirectory() as empty:
            data = home_rankings.rankings(pathlib.Path(empty))
            self.assertEqual(data, {"civs": None, "strategies": None, "tournaments": None})
            page = pathlib.Path(empty) / "index.html"
            # A landing page from before the column is left byte for byte.
            page.write_text("<html>no slot</html>", encoding="utf-8")
            self.assertEqual(home_rankings.main(["home_rankings.py", empty, str(page)]), 0)
            self.assertEqual(page.read_text(encoding="utf-8"), "<html>no slot</html>")
            page.write_text(CHARTS, encoding="utf-8")
            self.assertEqual(home_rankings.main(["home_rankings.py", empty, str(page)]), 0)
            self.assertEqual(json.loads(home_rankings.SLOT.search(page.read_text(encoding="utf-8"))[2]),
                             {"civs": None, "strategies": None})

    def test_only_a_page_with_the_tournaments_card_is_sent_its_rows(self):
        # The table lives on the Tournaments page now. A lane pinned to a home
        # page that still has the card is published by this script too and
        # keeps its table; head's page has no card and is not sent the rows.
        data = {"civs": None, "strategies": None, "tournaments": [{"id": 1}]}
        self.assertEqual(home_rankings.for_page(CHARTS, data), {"civs": None, "strategies": None})
        older = CHARTS.replace('id="rank-civs"', 'id="rank-tournaments"></section><section id="rank-civs"', 1)
        self.assertEqual(home_rankings.for_page(older, data), data)
        with tempfile.TemporaryDirectory() as empty:
            page = pathlib.Path(empty) / "index.html"
            page.write_text(older, encoding="utf-8")
            self.assertEqual(home_rankings.main(["home_rankings.py", empty, str(page)]), 0)
            self.assertIn("tournaments", json.loads(home_rankings.SLOT.search(page.read_text(encoding="utf-8"))[2]))


if __name__ == "__main__":
    unittest.main()
