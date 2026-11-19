#!/usr/bin/env python3
"""The home page's rankings column is derived from this tree and fits its slot."""

from __future__ import annotations

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
    """One line of the results file, as `civ6_ladder.result_row` writes it."""
    return {
        "tag": tag,
        "game": re.sub(r"-cont\d+$", "", tag),
        "utc": "2026-11-01T00:00:00Z",
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
        ladder = re.search(r"CIV6_RUNGS: \[.*?\] = \[(.*?)\];", (ROOT / "src" / "strategies.rs").read_text(encoding="utf-8"), re.S)
        self.assertEqual(re.findall(r'\("(\w+)",', ladder[1]), home_rankings.CIV6_RUNGS)
        levels = {name: lv for row in rungs for name, lv in zip(row["name"].split(" · "), row["levels"])}
        self.assertEqual((levels["Settler"], levels["Prince"], levels["King"], levels["Deity"]), (1, 4, 5, 8))
        self.assertEqual(sorted(levels.values()), list(range(1, 9)))
        self.assertIn('"Civ VI " + level(r) + r.name', LANDING)
        named = [name for row in rungs for name in row["name"].split(" · ")]
        self.assertEqual(sorted(named), sorted(rung.title() for rung in published["ladder"]))

    def test_the_civs_are_ranked_by_wins_over_the_recorded_games(self):
        names = ["Greece", "Korea", "Gran Colombia", "Ottomans", "Babylon", "Rome"]
        rows = [
            # Greece: two wins at two levels, and a loss of each kind.
            run("g1", "win", "GREECE", victory="VICTORY_CULTURE", utc="2026-11-03T05:00:00Z"),
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
            # Two wins each: the better win rate stands higher.
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
        ranking = home_rankings.civ_ranking(rows, names)
        self.assertEqual(
            [(row["name"], row["wins"], row["losses"]) for row in ranking["rows"]],
            [("Korea", 2, 0), ("Gran Colombia", 2, 1), ("Greece", 2, 3), ("Babylon", 1, 0), ("Ottomans", 1, 0)],
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
                "games": 14, "wins": 9, "unfinished": 2, "unattributed_wins": 1,
                "played": 6, "winners": 5,
                # The last finished game's day; a stopped game does not move it.
                "through": "2026-11-03",
            },
        )
        # Nothing finished is nothing to show.
        self.assertIsNone(home_rankings.civ_ranking([run("g6", "unfinished", "GREECE")], names))

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

    def test_the_genes_are_the_head_of_the_committed_ranking(self):
        ranking = home_rankings.genes(ROOT)
        rows = ranking["rows"]
        self.assertEqual([row["rank"] for row in rows], list(range(1, home_rankings.TOP_GENES + 1)))
        table = (ROOT / "GENE_HEURISTIC_RANKING.md").read_text(encoding="utf-8")
        for row in rows:
            self.assertIn(f"| {row['rank']} | `{row['tag']}` |", table)
            self.assertIsInstance(row["diff"], float)
            self.assertTrue(row["description"])
        self.assertGreaterEqual(ranking["total"], len(rows))
        self.assertTrue(0 < ranking["on"] <= ranking["total"])
        self.assertEqual(ranking["total"], len(re.findall(r"(?m)^\| \d+ \| `", table)))

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

    def test_a_sentence_cannot_close_the_slot(self):
        data = {"strategies": None, "genes": {"rows": [{"description": "</script><b>x"}]}}
        filled = home_rankings.fill(LANDING, data)
        self.assertEqual(filled.count("</script>"), LANDING.count("</script>"))
        self.assertEqual(json.loads(home_rankings.SLOT.search(filled)[2]), data)

    def test_an_older_revision_publishes_without_either_ranking(self):
        with tempfile.TemporaryDirectory() as empty:
            data = home_rankings.rankings(pathlib.Path(empty))
            self.assertEqual(data, {"civs": None, "strategies": None, "genes": None})
            page = pathlib.Path(empty) / "index.html"
            # A landing page from before the column is left byte for byte.
            page.write_text("<html>no slot</html>", encoding="utf-8")
            self.assertEqual(home_rankings.main(["home_rankings.py", empty, str(page)]), 0)
            self.assertEqual(page.read_text(encoding="utf-8"), "<html>no slot</html>")
            page.write_text(LANDING, encoding="utf-8")
            self.assertEqual(home_rankings.main(["home_rankings.py", empty, str(page)]), 0)
            self.assertEqual(json.loads(home_rankings.SLOT.search(page.read_text(encoding="utf-8"))[2]), data)


if __name__ == "__main__":
    unittest.main()
