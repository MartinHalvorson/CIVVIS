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
import unittest.mock

import home_rankings
import performance_page
import research_page

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
# The two rankings stood on Win Analytics, the first page under Charts/, from
# 2026-11-22 (operator: "move 2 charts on home/ to charts/") and moved on
# 2026-11-23: the player strategies to ELO Rankings, the civ records to Civ
# Analytics (operator: "add that to a new page: Charts/ELO Rankings ... at
# the top of this page is the player straetgies, pulled from win analytics,
# add another page: Civ Analytics. First can be the civ records (pulled from
# win analytics), then early build order"). The home page is its videos,
# with the Firaxis Civ 6 AI benchmark copied over them from Win Analytics
# and, over that since 2026-11-27, ELO Rankings (`elo_table_test.py`).
CHARTS = (BETA / "research.html").read_text(encoding="utf-8")
ELO = (BETA / "elo.html").read_text(encoding="utf-8")
CIVS = (BETA / "civs.html").read_text(encoding="utf-8")
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")
PAGES = {
    name: (BETA / f"{name}.html").read_text(encoding="utf-8")
    for name in ("landing", "research", "elo", "civs", "performance", "tournaments")
}
#: A page with the rankings' slot, as Win Analytics carried it from
#: 2026-11-22 to 2026-11-23: no page of head's has it any more, and a lane
#: pinned to a revision of those days is still filled by this script.
SLOTTED = (
    '<section class="res-section rank-card" id="rank-strategies" hidden></section>\n'
    '<section class="res-section rank-card" id="rank-civs" hidden></section>\n'
    '<script type="application/json" id="rankings-data">null</script>\n'
    "<script>/* the two lists */</script>\n"
)

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
        self.assertIn('" ± is the 95% margin of error."', ELO)

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
        # ELO Rankings names each level the same way, among the agents.
        self.assertIn(f'const levelName = (l) => l.level + " " + l.name + "{template[1]}";', ELO)
        self.assertNotIn('"Civ VI "', ELO)
        named = [name for row in rungs for name in row["name"].split(" · ")]
        self.assertEqual(sorted(named), sorted(rung.title() for rung in published["ladder"]))
        # A level carries its 95% margin as an agent does: the widest of the
        # rungs sharing its row, and none on the rung the scale is pinned at
        # unless it shares its row.
        for row in rungs:
            margins = [published["ratings"][f"civ6:{name.lower()}"]["margin95"] for name in row["name"].split(" · ")]
            self.assertEqual(row["margin"], max(margins), row["name"])
        self.assertGreater(max(row["margin"] for row in rungs), 0)

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
        # beside it, and says which games were counted: the level in the
        # card's title ("Against Lv 8 Deity AI"), the day under it. The line
        # under the title said both, at length, until 2026-11-24 (operator:
        # "there is too much text on pages, particualrly home/ and charts/").
        for line in (
            'const figure = el("span", "rank-value", (100 * wins / (wins + losses)).toFixed(1) + "%");',
            'figure.append(el("small", "", fmt(wins) + "\\u2013" + fmt(losses)));',
            'const item = civRow(i, c.name, c.wins, c.losses,',
            'card.querySelector(".rank-title").textContent = civs.level ? "Against " + levelName(civs.level) + " AI" : "Against Civilization VI\\u2019s AI";',
            'sub.textContent = "Win rate as each civ" + (civs.since ? ", since " : "");',
            'if (civs.since) sub.append(el("span", "rank-day", civs.since));',
        ):
            self.assertIn(line, CIVS)

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

    def test_a_page_with_the_slot_is_filled_and_head_s_pages_have_none(self):
        # No page of head's carries the slot: the lists are drawn from
        # their own pages' records (`elo_page.py`, `civs_page.py`), which
        # take them from here.
        for name, page in PAGES.items():
            self.assertNotIn('id="rankings-data"', page, name)
        data = home_rankings.rankings(ROOT)
        filled = home_rankings.fill(SLOTTED, data)
        slot = home_rankings.SLOT.search(filled)
        self.assertEqual(json.loads(slot[2]), data)
        # Nothing but the slot moved.
        self.assertEqual(home_rankings.SLOT.sub("", filled), home_rankings.SLOT.sub("", SLOTTED))
        # The publish script is what runs it, against the lane's own revision:
        # on Win Analytics and on the home page, which had the slot until
        # 2026-11-23 and 2026-11-22 and still have it in a lane pinned to an
        # older revision.
        publish = (BETA / "publish.sh").read_text(encoding="utf-8")
        self.assertIn('fill home_rankings.py "$out/$research_at/index.html"', publish)
        self.assertIn('fill home_rankings.py "$out/$home_at/index.html"', publish)
        # `fill` runs the lane's own script against the lane's own tree.
        self.assertIn('local script="$source_tree/beta/$1" page="$2"', publish)
        self.assertIn('python3 "$script" "$source_tree" "$page"', publish)
        self.assertNotIn('"$repo_root/beta/home_rankings', publish)

    def test_the_two_rankings_moved_to_elo_rankings_and_civ_analytics(self):
        # Operator 2026-11-22: "move 2 charts on home/ to charts/", then of
        # Win Analytics: "player strategies ranking should be off to the
        # right side of the graphs mentioned above. The winningest civs on
        # the right below the other table". On 2026-11-23 the two lists moved
        # again, each to a page of its own under Charts/: the player
        # strategies first on ELO Rankings, the civ records first on Civ
        # Analytics. Win Analytics keeps its graphs, now across the page.
        for gone in ("rank-strategies", "rank-civs", "rankings-data", "res-lists", "rank-list"):
            self.assertNotIn(gone, CHARTS)
        at = [
            CHARTS.index(piece)
            for piece in ('<div class="res-layout">', '<div class="res-graphs">', 'id="res-kind-self_play"',
                          'id="res-kind-ai-civvis"')
        ]
        self.assertEqual(at, sorted(at))
        self.assertIn(".res-layout { display: grid; grid-template-columns: minmax(0, 1fr); gap: 16px; align-items: start; min-width: 0; }", CHARTS)
        # The player strategies stand first in ELO Rankings' record. The civ
        # records stood first in Civ Analytics', before the tree of build
        # orders, until 2026-11-24 (operator: "move winngest civs to the
        # bottom"): now last, the tree first.
        record = ELO[ELO.index('<div class="elo-record" id="elo-record" hidden>'):]
        self.assertLess(record.index('id="rank-strategies"'), record.index('id="elo-over-time"'))
        self.assertEqual(re.findall(r'<section [^>]*id="([a-z-]+)"', record)[0], "rank-strategies")
        record = CIVS[CIVS.index('<div class="civ-record" id="civ-record" hidden>'):]
        sections = re.findall(r'<section [^>]*id="([a-z0-9-]+)"', record)
        self.assertEqual((sections[0], sections[-3:]), ("res-side-builds", ["civ-civs", "rank-civs", "rank-civs-self"]))
        # Since 2026-11-24 the list has two readings, the Diplomatic victory
        # on and off, each a record of the same fields (`elo_page.record`).
        self.assertIn("const rowsOf = (view) => (view && view.strategies && Array.isArray(view.strategies.rows) ? view.strategies.rows : []);", ELO)
        self.assertIn("const civs = data.civs;", CIVS)
        # The Tournaments table stood above the two lists until the operator
        # gave tournaments their own page (2026-11-21: "The tournament tables
        # on home right now can move to Tournaments"). Nothing of it came
        # along but the link to that page (`tournaments_page_test.py`), which
        # was named Experiments from 2026-11-25 to 2026-11-26.
        for gone in ("rank-tournaments", "is-wide", "data.tournaments", "toursShown"):
            for name, page in PAGES.items():
                self.assertNotIn(gone, page, name)
        for page in (CHARTS, ELO, CIVS):
            self.assertIn('<a class="site-link" href="../../tournaments/"', page)
        # Every game behind the civ table is one click away, on the branch
        # the table is read from.
        self.assertIn(
            f'href="https://github.com/MartinHalvorson/CIVVIS/blob/{civ6_ladder.LEDGER_BRANCH}/{civ6_ladder.RESULTS_PATH}"',
            CIVS,
        )
        # And the home page has neither list left, nor their slot. Since
        # 2026-11-27 it has ELO Rankings over its benchmark, drawn from ELO
        # Rankings' own record (`elo_table_test.py`): a table of its own, not
        # the list that moved.
        for gone in ("rankings-data", "rank-civs", "rank-strategies", 'id="rankings"'):
            self.assertNotIn(gone, LANDING)

    def test_the_home_page_is_eight_videos_two_to_a_row(self):
        # Operator 2026-11-26: "swap video 5 on home for
        # https://youtu.be/U0iwoVTK5q8. vid 5 (ai debugger) can be vid 6. 7 is
        # https://youtu.be/kcDyO_tOpmQ. 8 is https://youtu.be/11wYuujdddk."
        # Those are CIVVIS.ai, CIVVIS: Edge of Tomorrow and a newer Doctor
        # Strange's CIVVIS, titled as YouTube titles them.
        #
        # Six of them stood there from 2026-11-24: "on home, first video can be "Fastest recorded
        # CIVVIS self-play game" subtitle 4P Tennis Ball, Online. Single
        # Player POV is great as is. Space 3 we can insert a Lv 8 scienec win
        # - https://www.youtube.com/watch?v=rVDor7Xb4Mg. space 4 is culture
        # win. titles can be Lv 8 Deity Win - Culture, Maya. and Lv 8 Deity
        # Win - France, Science for vid 3 (first title for vid 4). vid 5 can
        # be our AI Debugger - Multiverse, Time Travel. vid 6 can be Doctor
        # Strange's Civ 6 https://www.youtube.com/watch?v=H50wfMantkY."
        #
        # Four of them stood there from 2026-11-22: "On home, we can have our
        # fastest game youtube video embedded to watch in the upper left. in
        # upper right, we can have a single player pov game (should be under
        # performance). Maybe the AI Debugger - Multiverse, Time Travel video
        # in the lower right. lower left please save for a lv 8 victory video
        # lv 8 maya cluture from the youtube." And later that day: "swap the
        # positions of the bottom 2 videos to the Maya Culture win is lower
        # right", "Make the video Titles above the video and bigger or more
        # readable", "have the home upper right video start at 6 seconds in".
        tiles = re.findall(
            r'<figure class="watch-tile" id="(watch-[a-z]+)" data-video="([\w-]{11})"( data-start="\d+")?(?: data-tool="[^"]+")?>\s*'
            r"<figcaption><b>([^<]+(?:<span data-watch-time>[^<]+</span>)?)</b> (?:<span data-watch-title>([^<]+)</span> )?<a [^>]+>(?:<svg [^>]+><path [^>]+/></svg>)?[^<]+</a></figcaption>\s*"
            r'<a class="watch-poster" href="([^"]+)"((?: target="_blank" rel="noreferrer")?)>\s*'
            r'<img alt="" width="1280" height="720" decoding="async" referrerpolicy="no-referrer" '
            r'src="https://i\.ytimg\.com/vi_webp/([\w-]{11})/hq720\.webp">',
            LANDING,
        )
        # Two columns, so document order reads left then right, row by
        # row: his spaces 1 to 8. A tile's title is the first thing in it,
        # over its video, which the pattern above reads in that order. A
        # title that says everything has no second line: the two victories
        # are titled as he wrote them, and so is CIVVIS.ai. Edge of
        # Tomorrow's and Doctor Strange's second lines say what they are for
        # (operator 2026-11-27: "to home/ edge of tomorrow - add
        # desecription: play and replay sequences throughout a Civ game with
        # top CIVVIS AI strategies to discover and experiment with optimal
        # play", and "to home/ doctor strange's civvis, change to: Enter
        # Doctor Strange's fifth dimension ..."). The
        # culture win is still at the right (2026-11-22), of the second row.
        # Since 2026-11-25 the fastest game's title ends with its time
        # ("throw a time on fastest recorded civvis game home/.:: <6s") and
        # the culture win names the civ first ("swap culture, maya to Maya,
        # Culture"), as the science win does.
        self.assertEqual(
            [(ident, video, label, title) for ident, video, _, label, title, _, _, _ in tiles],
            [
                ("watch-fastest", "iHlUWPIqr5g", "Fastest recorded CIVVIS self-play game:&#160;<span data-watch-time>&lt;6s</span>", "4P Tennis Ball, Online"),
                ("watch-pov", "9c8m87e3rCg", "Single player POV", "A full Civ 6 match in 14 seconds"),
                ("watch-science", "rVDor7Xb4Mg", "Lv 8 Deity Win - France, Science", ""),
                ("watch-culture", "6PCd4R08UPM", "Lv 8 Deity Win - Maya, Culture", ""),
                ("watch-civvis", "U0iwoVTK5q8", "CIVVIS.ai", ""),
                ("watch-debugger", "6W5t0hcR_N4", "CIVVIS AI Debugger", "Multiverse, Time Travel"),
                ("watch-tomorrow", "kcDyO_tOpmQ", "CIVVIS: Edge of Tomorrow",
                 "Play and replay sequences throughout a Civ game with top CIVVIS AI strategies to discover and experiment with optimal play"),
                ("watch-strange", "11wYuujdddk", "Doctor Strange's CIVVIS",
                 "Enter Doctor Strange's fifth dimension to visually explore alternate paths through time. Watch 12 different CIVVIS strategies simultaneously play out 12 copies of the same full Civ game in 60s and analyze the outcomes."),
            ],
        )
        for _, video, _, _, _, _, _, picture in tiles:
            self.assertEqual(picture, video)
        # Pressing a video: the four of the site's own tools open the tool,
        # CIVVIS.ai the simulator and the debugger, Edge of Tomorrow and
        # Doctor Strange's CIVVIS the modes themselves, as the site list
        # names them, in the same tab; the other four play, or open on
        # YouTube in a new tab (operator 2026-11-27: "for civvis.ai, ai
        # debugger, edge of tomorrow, doctor strange, can we have clicking on
        # the video link to the right tool and a Youtube link maybe where the
        # existing link is, upper right of the video").
        youtube = "https://www.youtube.com/watch?v="
        new_tab = ' target="_blank" rel="noreferrer"'
        self.assertEqual([(ident, poster, tab) for ident, _, _, _, _, poster, tab, _ in tiles], [
            ("watch-fastest", youtube + "iHlUWPIqr5g", new_tab),
            ("watch-pov", youtube + "9c8m87e3rCg&amp;t=6s", new_tab),
            ("watch-science", youtube + "rVDor7Xb4Mg", new_tab),
            ("watch-culture", youtube + "6PCd4R08UPM", new_tab),
            ("watch-civvis", "../", ""),
            ("watch-debugger", "../debugger", ""),
            ("watch-tomorrow", "../edge-of-tomorrow", ""),
            ("watch-strange", "../doctor-strange", ""),
        ])
        tools = re.findall(r'<figure class="watch-tile" id="(watch-[a-z]+)" data-video="[\w-]{11}" data-tool="([^"]+)">', LANDING)
        self.assertEqual(tools, [(ident, poster) for ident, _, _, _, _, poster, tab, _ in tiles if not tab])
        # The link at the upper right of a video: the two records lead to
        # CIVVIS Performance, the two victories to the playlist of them, and
        # a tool's video to itself on YouTube, with YouTube's mark.
        more = re.findall(r'<figure class="watch-tile" id="(watch-[a-z]+)".*?<figcaption>.*?<a href="([^"]+)"[^>]*>(<svg [^>]+><path [^>]+/></svg>)?([^<]+)</a></figcaption>', LANDING, re.S)
        self.assertEqual([(ident, href, text, bool(mark)) for ident, href, mark, text in more], [
            ("watch-fastest", "../charts/civvis-performance/", "CIVVIS Performance", False),
            ("watch-pov", "../charts/civvis-performance/", "CIVVIS Performance", False),
            ("watch-science", "https://www.youtube.com/playlist?list=PLWcKlP8e8iy4", "Lv 8 Deity Wins", False),
            ("watch-culture", "https://www.youtube.com/playlist?list=PLWcKlP8e8iy4", "Lv 8 Deity Wins", False),
            ("watch-civvis", youtube + "U0iwoVTK5q8", "YouTube", True),
            ("watch-debugger", youtube + "6W5t0hcR_N4", "YouTube", True),
            ("watch-tomorrow", youtube + "kcDyO_tOpmQ", "YouTube", True),
            ("watch-strange", youtube + "11wYuujdddk", "YouTube", True),
        ])
        # A tool's player takes clicks for itself, so a link to the tool lies
        # over it once it plays, and pressing the picture before then is
        # never turned into playing it here.
        self.assertIn('const tool = tile.dataset.tool || "";', LANDING)
        self.assertIn("        if (tool) return;\n", LANDING)
        self.assertIn('go.className = "watch-go";', LANDING)
        self.assertIn('(tool ? "&controls=0" : "")', LANDING)
        self.assertIn(".watch-go { position: absolute; inset: 0; z-index: 1; border-radius: inherit; }", LANDING)
        for href, name in (
            ("../", "CIVVIS"),
            ("../debugger", "CIVVIS AI Debugger"),
            ("../edge-of-tomorrow", "CIVVIS: Edge of Tomorrow"),
            ("../doctor-strange", "Doctor Strange's CIVVIS"),
        ):
            self.assertRegex(LANDING, rf'<a class="site-link" href="{re.escape(href)}"[^>]*>{re.escape(name)}</a>')
        # The single player's game starts 6 seconds in, on the page and on
        # YouTube; the other seven start at their start.
        self.assertEqual(
            [(start, poster.endswith("&amp;t=6s")) for _, _, start, _, _, poster, _, _ in tiles],
            [("", False), (' data-start="6"', True)] + [("", False)] * 6,
        )
        # A tile with no second line is named by its title alone.
        self.assertIn('const second = tile.querySelector("[data-watch-title]");', LANDING)
        self.assertIn('const title = tile.querySelector("b").textContent + (second ? ": " + second.textContent : "");', LANDING)
        self.assertIn('const start = /^[1-9]\\d*$/.test(tile.dataset.start || "") ? tile.dataset.start : "";', LANDING)
        self.assertIn('        poster.href = "https://www.youtube.com/watch?v=" + id + (start ? "&t=" + start + "s" : "");', LANDING)
        # The titles are the largest type under the page's name, and whole.
        self.assertIn(".watch-tile figcaption b { color: #fff; font-size: 20px; font-weight: 700; line-height: 1.3; }", LANDING)
        self.assertIn(".watch-tile figcaption span { order: 3; flex-basis: 100%; min-width: 0; color: var(--text); font-size: 15px; line-height: 1.4; }", LANDING)
        self.assertNotIn("text-overflow", LANDING[LANDING.index(".watch-grid {") : LANDING.index("</style>")])
        # The section has no heading over it (operator: "Drop "watch" title
        # or whatever from this section").
        self.assertIn('<section class="watch" id="watch" aria-label="Videos">\n      <div class="watch-grid">', LANDING)
        for gone in ('id="watch-title"', 'class="watch-title"', "aria-labelledby=\"watch", "watch-head", ">Watch<"):
            self.assertNotIn(gone, LANDING)
        self.assertEqual(LANDING.count('class="watch-tile"'), 8)
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
        self.assertIn('const PLAYER = "https://www.youtube-nocookie.com";', LANDING)
        # Every video loops for as long as the page is open (operator
        # 2026-11-23: "please have the vido on the website all loop
        # indefinitely"): YouTube loops a video that is its own playlist.
        self.assertIn(
            'player.src = PLAYER + "/embed/" + id + "?autoplay=1&rel=0&playsinline=1&loop=1&playlist=" + id'
            ' + (muted ? "&mute=1" : "") + (tool ? "&controls=0" : "") + (start ? "&start=" + start + "&enablejsapi=1" : "");',
            LANDING,
        )
        # Its loop comes round to 0 s, so a tile with a start sends its
        # player back there each time round, through the player's messages,
        # and only once it plays again (a seek while it reloads is not taken).
        self.assertIn("if (start) keepStart(player, Number(start));", LANDING)
        keep = LANDING[LANDING.index("const keepStart = "):LANDING.index("const starts = new Map();")]
        self.assertIn('send({ event: "listening", id: player.title, channel: "widget" })', keep)
        self.assertIn("if (event.origin !== PLAYER || event.source !== player.contentWindow) return;", keep)
        self.assertIn("if (info.playerState === -1 || info.playerState === 0) round = true;", keep)
        self.assertIn("if (round && playing && Date.now() - asked > 1000) {", keep)
        self.assertIn('send({ event: "command", func: "seekTo", args: [start, true] });', keep)
        self.assertNotIn("innerHTML", keep)
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
        # With it, the game the record played, from its line in the list:
        # the tile's second line (operator 2026-11-24: "first video can be
        # "Fastest recorded CIVVIS self-play game" subtitle 4P Tennis Ball,
        # Online").
        fastest = {"fastest": {**{key: records[0][key] for key in ("id", "title", "time")}, "short": "<6s", "setup": "4P Tennis Ball, Online"}}
        self.assertEqual(home_rankings.videos(ROOT), fastest)
        listed = json.loads((ROOT / performance_page.VIDEOS).read_text(encoding="utf-8"))["videos"]
        self.assertEqual([entry["video"][-11:] for entry in listed if entry.get("setup")], [records[0]["id"]])
        self.assertIn("<span data-watch-title>4P Tennis Ball, Online</span>", LANDING)
        # And its time, at the end of the tile's title: the record's, as
        # the page is written with it ("under 6 seconds").
        self.assertEqual(records[0]["time"], "under 6 seconds")
        self.assertIn("<b>Fastest recorded CIVVIS self-play game:&#160;<span data-watch-time>&lt;6s</span></b>", LANDING)
        self.assertIn(".watch-tile figcaption b [data-watch-time] { color: inherit; font-size: inherit; line-height: inherit; }", LANDING)
        self.assertEqual(records[0]["seconds"], min(v["seconds"] for v in records))
        with tempfile.TemporaryDirectory() as scratch:
            page = pathlib.Path(scratch) / "index.html"
            page.write_text(LANDING, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()) as said:
                self.assertEqual(home_rankings.main(["home_rankings.py", str(ROOT), str(page)]), 0)
            self.assertIn("the fastest game's video", said.getvalue())
            filled = page.read_text(encoding="utf-8")
            self.assertEqual(json.loads(home_rankings.VIDEOS_SLOT.search(filled)[2]), fastest)
            # Nothing but the page's slots moved: this one and the Firaxis
            # AI benchmark's.
            bare = lambda text: home_rankings.BENCHMARK_SLOT.sub("", home_rankings.VIDEOS_SLOT.sub("", text))  # noqa: E731
            self.assertEqual(bare(filled), bare(LANDING))
            # A record that does not say what game it played: no setup in
            # the slot, and the tile's second line is its video's title.
            (pathlib.Path(scratch) / performance_page.VIDEOS).parent.mkdir(parents=True)
            (pathlib.Path(scratch) / performance_page.VIDEOS).write_text(
                json.dumps({"videos": [{"video": "https://www.youtube.com/watch?v=" + "a" * 11, "seconds": 5, "title": "A faster one"}]}), encoding="utf-8")
            self.assertEqual(home_rankings.videos(pathlib.Path(scratch))["fastest"], {"id": "a" * 11, "title": "A faster one", "time": "5 seconds", "short": "5s"})
            (pathlib.Path(scratch) / performance_page.VIDEOS).unlink()
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
            'if (fastest.setup || fastest.title) tile.querySelector("[data-watch-title]").textContent = fastest.setup || fastest.title;',
            'if (fastest.short) tile.querySelector("[data-watch-time]").textContent = fastest.short;',
        ):
            self.assertIn(line, LANDING)

    def test_the_firaxis_benchmark_stands_over_the_videos_as_win_analytics_has_it(self):
        # Operator 2026-11-24: "above the videos, lets copy the Firaxis Civ 6
        # Benchmarks showing our win rate (both diplo on and off), same as
        # the chart is now. then the videos below".
        # It was the page's first part from the day the speed timeline
        # that stood over it went to CIVVIS Performance, later that day
        # ("move the civvis performance timeline thing from home/ to the
        # civvis performance section"; `beta/speedup_timeline_test.py`),
        # until 2026-11-27, when ELO Rankings went over it ("put the elo
        # chart at the top of home/ too, above the benchmarks").
        places = [LANDING.index(mark) for mark in (
            '<p class="intro">',
            '<section class="res-block home-elo" id="elo" aria-labelledby="home-elo-title" hidden>',
            '<section class="res-block home-bench" id="bench" aria-labelledby="res-title-tourney" hidden>',
            '<section class="watch"',
        )]
        self.assertEqual(places, sorted(places))
        main = LANDING[LANDING.index('<main class="shell">'):LANDING.index("</main>")]
        self.assertEqual(re.findall(r"<section [^>]*\bid=\"([\w-]+)\"", main), ["elo", "bench", "watch"])
        section = LANDING[places[2]:places[3]]
        bare = lambda text: [line.strip() for line in text.strip().split("\n")]  # noqa: E731
        # Its title and the line under it are Win Analytics' own, and so is
        # everything from the pair's opening to the end of the two tables:
        # the games that allowed the Diplomatic victory at the left, the
        # games with it off at the right, each titled with the games it
        # counts, the four windows as columns, and under the left one the
        # Deity games with the Religious victory off as well (operator,
        # 2026-11-27: "add a third category to the tournament for lv 8 deity
        # no diplo no religion").
        for line in (
            '<h2 class="res-side-title" id="res-title-tourney">Firaxis Civ 6 AI benchmark: win rate by level</h2>',
            '<p class="res-counted" id="res-counted-tourney">Tournament games against Civilization VI&#8217;s own AI.</p>',
        ):
            self.assertIn(line, section)
            self.assertIn(line, CHARTS)
        pair = '<div class="res-bench-pair">'
        theirs = CHARTS[CHARTS.index(pair) : CHARTS.index('<details class="fine">', CHARTS.index(pair))]
        ours = section[section.index(pair) : section.index("</section>")]
        self.assertEqual(bare(ours), bare(theirs))
        # The note of the 1/6 par is gone from the copy too (operator,
        # 2026-11-26: "drop the expected win rate thing warning message").
        for page in (LANDING, CHARTS):
            self.assertNotIn("res-bench-par", page)
            self.assertNotIn("Expected win rate", page)
        # And the titles are only the games they count (operator, later on
        # 2026-11-26: "remove the diplo vicotry alloewd and diplo vicotry off
        # parts of the title. leave diplo off at the end of the second").
        self.assertNotIn("Diplomatic victory allowed ·", ours)
        self.assertNotIn("Diplomatic victory off ·", ours)
        self.assertIn(">6P Continents Online <small", ours)
        self.assertIn(">6P Continents Online, Diplo Off <small", ours)
        self.assertIn(">6P Continents Online, Diplo Off, Religion Off <small", ours)
        self.assertEqual(ours.count("<table"), 3)
        self.assertLess(ours.index('id="res-rates-on"'), ours.index('id="res-rates-off"'))
        self.assertLess(ours.index('id="res-rates-off"'), ours.index('id="res-rates-religion_off"'))
        # How the games are counted stays on Win Analytics, a link away.
        self.assertIn('<p class="home-bench-more"><a href="../charts/win-analytics/">Win Analytics</a></p>', section)
        self.assertNotIn("<details", section)
        # The look is that page's too: every rule of the copy is a line of
        # its stylesheet, and no rule of its benchmark is missing here.
        style = LANDING[LANDING.index("<style>") : LANDING.index("</style>")]
        copied = style[style.index("  .res-block { display: grid;") : style.index("  /* This page's own: the room under the benchmark")]
        theirs = CHARTS[CHARTS.index("<style>") : CHARTS.index("</style>")]
        rules = [line for line in copied.split("\n") if line.strip()]
        self.assertGreater(len(rules), 30)
        for line in rules:
            if line.strip() == ".res-block[hidden] { display: none; }":
                self.assertIn(".res-block[hidden] { display: none; }", theirs)
                continue
            self.assertIn("\n" + line + "\n", theirs, line.strip())
        for line in theirs.split("\n"):
            if re.match(r"\s*(@media [^{]+\{ )?\.res-(rates|bench)", line):
                self.assertIn("\n" + line + "\n", style, line.strip())
        # And the drawing: the shade of a cell against the 1/6 par, the
        # words of a rate, and the loop that fills the three tables are that
        # page's own lines.
        def cut(page: str, start: str, end: str) -> list[str]:
            at = page.index(start)
            return bare(page[at : page.index(end, at) + len(end)])
        for start, end in (
            ("const PAR = 1 / 6,", '"#fff" : "#101820" };\n    };'),
            ("const el = (tag, cls, text) => {", "return node;\n    };"),
            ("const fmt = (n) =>", ";"),
            ("const pct = (x) =>", '"%");'),
            ("const day = (iso) =>", "};"),
            ("const count = (n, one, many) =>", '.replace("Z", " UTC");'),
            ('for (const name of ["on", "off", "religion_off"]) {', "body.append(tr);"),
        ):
            self.assertEqual(cut(LANDING, start, end), cut(CHARTS, start, end), start)
        self.assertNotIn("innerHTML", LANDING)
        # Shown only when the page was given the record.
        self.assertIn("if (!section || !BENCH || !BENCH.tables) return;", LANDING)
        self.assertIn("section.hidden = false;", LANDING[LANDING.index('document.getElementById("home-benchmark")'):LANDING.index("// The video tiles.")])

    def test_the_benchmark_s_record_is_the_one_win_analytics_is_given(self):
        self.assertEqual(LANDING.count('<script type="application/json" id="home-benchmark">null</script>'), 1)
        rates = research_page.tournament_rates(ROOT, home_rankings.results(ROOT))
        shown = home_rankings.benchmark(ROOT)
        if rates is None:
            # A clone without the run ledger: no record, and the section
            # stays hidden.
            self.assertIsNone(shown)
        else:
            self.assertEqual({key: value for key, value in shown.items() if key != "rungs"}, rates)
            self.assertEqual(sorted(shown["tables"]), ["off", "on", "religion_off"])
        # Whatever the ledger holds: the record as it is, and with it the
        # levels' names, which Win Analytics has from its level picker.
        record = {"to": "2026-11-24 20:18Z", "days": [1, 3, 7, 30], "games": 4, "tables": {
            name: {"games": games, "levels": [{"level": level, "windows": [{"days": days, "games": games, "wins": 1} for days in (1, 3, 7, 30)]} for level in levels]}
            for name, games, levels in (("on", 2, (8, 7, 6, 5, 4)), ("off", 1, (8, 7, 6, 5, 4)), ("religion_off", 1, (8,)))
        }}
        with unittest.mock.patch.object(research_page, "tournament_rates", return_value=record):
            shown = home_rankings.benchmark(ROOT)
            self.assertEqual(shown, {**record, "rungs": {"8": "Deity", "7": "Immortal", "6": "Emperor", "5": "King", "4": "Prince"}})
            for level, name in shown["rungs"].items():
                self.assertRegex(CHARTS, rf'data-level="{level}" data-rung="{name}"')
            with tempfile.TemporaryDirectory() as scratch:
                page = pathlib.Path(scratch) / "index.html"
                page.write_text(LANDING, encoding="utf-8")
                with contextlib.redirect_stderr(io.StringIO()) as said:
                    self.assertEqual(home_rankings.main(["home_rankings.py", str(ROOT), str(page)]), 0)
                self.assertIn("the Firaxis AI benchmark", said.getvalue())
                self.assertEqual(json.loads(home_rankings.BENCHMARK_SLOT.search(page.read_text(encoding="utf-8"))[2]), shown)
        with unittest.mock.patch.object(research_page, "tournament_rates", return_value=None):
            self.assertIsNone(home_rankings.benchmark(ROOT))
            with tempfile.TemporaryDirectory() as scratch:
                page = pathlib.Path(scratch) / "index.html"
                page.write_text(LANDING, encoding="utf-8")
                with contextlib.redirect_stderr(io.StringIO()) as said:
                    self.assertEqual(home_rankings.main(["home_rankings.py", str(ROOT), str(page)]), 0)
                self.assertIn("no Firaxis AI benchmark", said.getvalue())
                self.assertIsNone(json.loads(home_rankings.BENCHMARK_SLOT.search(page.read_text(encoding="utf-8"))[2]))
        # Win Analytics is given the same function's answer.
        source = (BETA / "research_page.py").read_text(encoding="utf-8")
        self.assertIn('out["tournament_rates"] = tournament_rates(source, rows)', source)

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
        self.assertEqual(re.findall(r'id="(rank-[a-z]+)"', CHARTS), [])
        self.assertEqual(re.findall(r'id="(rank-[a-z]+)"', ELO), ["rank-strategies"])
        self.assertEqual(re.findall(r'id="(rank-[a-z]+)"', CIVS), ["rank-civs"])
        for page in (CHARTS.lower(), ELO.lower(), CIVS.lower()):
            for gone in ("heuristic gene", "gene_heuristic_ranking", "data.genes", "is-gene", "is-off", "rank-stack", "has-civs"):
                self.assertNotIn(gone, page)
        # A lane pinned to a revision whose page still has the gene card is
        # published by this script too, and its card stays empty and hidden:
        # that page shows a list only when the slot holds its key.
        self.assertNotIn("genes", json.loads(home_rankings.SLOT.search(
            home_rankings.fill(SLOTTED, home_rankings.rankings(ROOT)))[2]))

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
        filled = home_rankings.fill(SLOTTED, data)
        self.assertEqual(filled.count("</script>"), SLOTTED.count("</script>"))
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
            page.write_text(SLOTTED, encoding="utf-8")
            self.assertEqual(home_rankings.main(["home_rankings.py", empty, str(page)]), 0)
            self.assertEqual(json.loads(home_rankings.SLOT.search(page.read_text(encoding="utf-8"))[2]),
                             {"civs": None, "strategies": None})

    def test_only_a_page_with_the_tournaments_card_is_sent_its_rows(self):
        # The table lives on the Tournaments page now. A lane pinned to a home
        # page that still has the card is published by this script too and
        # keeps its table; head's page has no card and is not sent the rows.
        data = {"civs": None, "strategies": None, "tournaments": [{"id": 1}]}
        self.assertEqual(home_rankings.for_page(SLOTTED, data), {"civs": None, "strategies": None})
        older = SLOTTED.replace('id="rank-civs"', 'id="rank-tournaments"></section><section id="rank-civs"', 1)
        self.assertEqual(home_rankings.for_page(older, data), data)
        with tempfile.TemporaryDirectory() as empty:
            page = pathlib.Path(empty) / "index.html"
            page.write_text(older, encoding="utf-8")
            self.assertEqual(home_rankings.main(["home_rankings.py", empty, str(page)]), 0)
            self.assertIn("tournaments", json.loads(home_rankings.SLOT.search(page.read_text(encoding="utf-8"))[2]))


if __name__ == "__main__":
    unittest.main()
