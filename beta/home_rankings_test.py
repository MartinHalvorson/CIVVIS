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

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")

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
        self.assertIn("Prince", [row["name"] for row in rungs])
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
        self.assertIn(f'row("is-rung", "", level(r) + "{template[1]}", "", elo(r), "",', LANDING)
        self.assertIn('return low + (high > low ? "\\u2013" + high : "") + " " + r.name;', LANDING)
        self.assertNotIn('"Civ VI "', LANDING)
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
            self.assertIn(line, LANDING)

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
        self.assertEqual(LANDING.count('<script type="application/json" id="rankings-data">null</script>'), 1)
        data = home_rankings.rankings(ROOT)
        filled = home_rankings.fill(LANDING, data)
        slot = home_rankings.SLOT.search(filled)
        self.assertEqual(json.loads(slot[2]), data)
        # Nothing but the slot moved.
        self.assertEqual(home_rankings.SLOT.sub("", filled), home_rankings.SLOT.sub("", LANDING))
        # The publish script is what runs it, against the lane's own revision.
        publish = (BETA / "publish.sh").read_text(encoding="utf-8")
        self.assertIn('home_rankings.py" "$source_tree" "$out/home/index.html"', publish)

    def test_the_civs_hold_the_left_and_the_strategies_the_right(self):
        # The rankings section is a two-column grid with two children, each a
        # card: the civs, then the strategies. Document order is the order
        # the columns read in, and the order a narrow window stacks them in.
        opening = '<section class="rankings" id="rankings" hidden aria-label="Rankings">'
        self.assertEqual(LANDING.count(opening), 1)
        section = LANDING[LANDING.index(opening) + len(opening):LANDING.index("</main>")]
        children = []
        depth = 0
        for tag in re.finditer(r"<(/?)(section|div)\b([^>]*)>", section):
            if tag[1]:
                depth -= 1
                if depth < 0:
                    break
            else:
                if depth == 0:
                    children.append(tag[0])
                depth += 1
        self.assertEqual(
            children,
            [
                '<section class="rank-card" id="rank-civs" hidden aria-labelledby="rank-civs-title">',
                '<section class="rank-card" id="rank-strategies" hidden aria-labelledby="rank-strategies-title">',
            ],
        )
        # Two columns and never three. The rule dates from when the
        # Tournaments table spanned every track above the lists (#5541, #5550)
        # and is kept now the table has its own page: it is read back and
        # counted the way the browser counts auto-fit tracks.
        rule = re.search(
            r"\.rankings \{ display: grid; grid-template-columns: repeat\(auto-fit, "
            r"minmax\(min\(100%, max\((\d+)px, calc\(50% - (\d+)px\)\)\), 1fr\)\); gap: (\d+)px;",
            LANDING,
        )
        self.assertIsNotNone(rule)
        floor, half_gap, gap = (int(figure) for figure in rule.groups())
        self.assertEqual(half_gap * 2, gap)

        def tracks(width: int) -> int:
            least = min(width, max(floor, width / 2 - half_gap))
            return max(1, int((width + gap) // (least + gap)))

        stacks_below = 2 * floor + gap
        self.assertEqual({tracks(width) for width in range(stacks_below, 2000)}, {2})
        self.assertEqual({tracks(width) for width in range(120, stacks_below)}, {1})
        for line in (
            "const civs = data.civs;",
            'shown = show("rank-civs", rows, note) || shown;',
            "const strategies = data.strategies;",
            'shown = show("rank-strategies", rows, note) || shown;',
            'if (shown) document.getElementById("rankings").hidden = false;',
        ):
            self.assertIn(line, LANDING)
        # The Tournaments table stood above the two lists until the operator
        # gave tournaments their own page (2026-11-21: "The tournament tables
        # on home right now can move to Tournaments"). Nothing of it is left
        # here but the link to that page (`tournaments_page_test.py`).
        for gone in ("rank-tournaments", "rank-table", "is-wide", "data.tournaments", "toursShown"):
            self.assertNotIn(gone, LANDING)
        self.assertIn('<a class="site-link" href="../tournaments/"', LANDING)
        # Every game behind the table is one click away, on the branch the
        # table is read from.
        self.assertIn(
            f'href="https://github.com/MartinHalvorson/CIVVIS/blob/{civ6_ladder.LEDGER_BRANCH}/{civ6_ladder.RESULTS_PATH}"',
            LANDING,
        )

    def test_the_page_lists_no_heuristic_genes(self):
        # The home page once carried the head of the gene ranking under the
        # strategies. Neither the page nor the slot it is filled from has
        # one now: no card, no reader, no style or script left behind.
        self.assertEqual(sorted(home_rankings.rankings(ROOT)), ["civs", "strategies", "tournaments"])
        self.assertFalse(hasattr(home_rankings, "genes"))
        self.assertEqual(re.findall(r'id="(rank-[a-z]+)"', LANDING), ["rank-civs", "rank-strategies"])
        page = LANDING.lower()
        for gone in ("heuristic gene", "gene_heuristic_ranking", "data.genes", "is-gene", "is-off", "rank-stack", "has-civs"):
            self.assertNotIn(gone, page)
        # A lane pinned to a revision whose page still has the gene card is
        # published by this script too, and its card stays empty and hidden:
        # that page shows a list only when the slot holds its key.
        self.assertNotIn("genes", json.loads(home_rankings.SLOT.search(
            home_rankings.fill(LANDING, home_rankings.rankings(ROOT)))[2]))

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

    def test_the_tournament_definitions_are_committed_and_read(self):
        specs = json.loads((ROOT / "data" / "tournaments.json").read_text(encoding="utf-8"))["tournaments"]
        self.assertTrue(specs)
        for spec in specs:
            self.assertRegex(spec["start"], r"^civvis-\d{8}T\d{6}Z$")
            if "end" in spec:
                self.assertRegex(spec["end"], r"^civvis-\d{8}T\d{6}Z$")
                self.assertGreater(spec["end"], spec["start"])
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
        filled = home_rankings.fill(LANDING, data)
        self.assertEqual(filled.count("</script>"), LANDING.count("</script>"))
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
            page.write_text(LANDING, encoding="utf-8")
            self.assertEqual(home_rankings.main(["home_rankings.py", empty, str(page)]), 0)
            self.assertEqual(json.loads(home_rankings.SLOT.search(page.read_text(encoding="utf-8"))[2]),
                             {"civs": None, "strategies": None})

    def test_only_a_page_with_the_tournaments_card_is_sent_its_rows(self):
        # The table lives on the Tournaments page now. A lane pinned to a home
        # page that still has the card is published by this script too and
        # keeps its table; head's page has no card and is not sent the rows.
        data = {"civs": None, "strategies": None, "tournaments": [{"id": 1}]}
        self.assertEqual(home_rankings.for_page(LANDING, data), {"civs": None, "strategies": None})
        older = LANDING.replace('id="rank-civs"', 'id="rank-tournaments"></section><section id="rank-civs"', 1)
        self.assertEqual(home_rankings.for_page(older, data), data)
        with tempfile.TemporaryDirectory() as empty:
            page = pathlib.Path(empty) / "index.html"
            page.write_text(older, encoding="utf-8")
            self.assertEqual(home_rankings.main(["home_rankings.py", empty, str(page)]), 0)
            self.assertIn("tournaments", json.loads(home_rankings.SLOT.search(page.read_text(encoding="utf-8"))[2]))


if __name__ == "__main__":
    unittest.main()
