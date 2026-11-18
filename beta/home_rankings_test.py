#!/usr/bin/env python3
"""The home page's rankings column is derived from this tree and fits its slot."""

from __future__ import annotations

import json
import pathlib
import re
import tempfile
import unittest

import home_rankings

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")


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
            self.assertEqual(data, {"strategies": None, "genes": None})
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
