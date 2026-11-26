#!/usr/bin/env python3
"""The Civ AI Interpretability page: one turn, the plan through a game, and every gene by what it is worth."""

from __future__ import annotations

import contextlib
import io
import json
import pathlib
import re
import tempfile
import unittest

import interpretability_page

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "interpretability.html").read_text(encoding="utf-8")
MARKUP, SCRIPT = PAGE.split('<script type="application/json" id="interpretability-data">', 1)
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")
RESEARCH = (BETA / "research.html").read_text(encoding="utf-8")
PERFORMANCE = (BETA / "performance.html").read_text(encoding="utf-8")
TOURNAMENTS = (BETA / "tournaments.html").read_text(encoding="utf-8")
VIEWER = (ROOT / "web" / "index.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")

HEADER = {
    "kind": "header",
    "speed": "online",
    "turns": 250,
    "players": 6,
    "map": "continents",
    "all_seats": True,
    "build": {"commit": "39c314e77f0000da86f136cb2221973742f9a197"},
}


def point(turn: int, posture: str, bottleneck: str | None = None) -> dict:
    return {"turn": turn, "posture": posture, "bottleneck": bottleneck}


def seat(seed: int, number: int, target: str, trace: list[dict], end: int = 200, win: bool = False, victory: str = "science",
         alive: bool = True, forecasting: bool = False, marks: dict | None = None) -> dict:
    """A seat's row as the gene screen writes it, with what this page reads."""
    return {
        "kind": "game", "seed": seed, "seat": number, "player_target": target, "turn": end, "win": win, "victory": victory, "alive": alive,
        "victory_portfolio": {"enabled": forecasting, "trace": trace},
        "trajectory": [{"turn": turn, "alive": True, **values} for turn, values in (marks or {}).items()],
    }


def batch(root: pathlib.Path, name: str, rows: list[dict], header: dict = HEADER, finished: bool = True) -> pathlib.Path:
    directory = root / name
    directory.mkdir(exist_ok=True)
    (directory / "rows-continuous.jsonl").write_text("".join(json.dumps(line) + "\n" for line in [header, *rows]), encoding="utf-8")
    if finished:
        (directory / "analysis.json").write_text("{}", encoding="utf-8")
    return directory


class OneTurn(unittest.TestCase):
    def test_a_step_is_sorted_by_what_its_name_says_it_does(self):
        for name, kind in (
            ("assess", "plan"),
            ("maintain_war_plan", "plan"),
            ("record_culture_curves", "plan"),
            ("advanced_research", "govern"),
            ("strategic_policies", "govern"),
            ("advanced_diplomacy", "govern"),
            ("emergency_city_defense_purchase", "spend"),
            ("buy_culture_shrine", "spend"),
            ("upgrade_units_preserving_air_wing", "spend"),
            ("advanced_gold_spending", "spend"),
            ("claim_capital_campus", "production"),
            ("reserve_government_plaza", "production"),
            ("advanced_production", "production"),
            ("advanced_units", "units"),
            ("close_unit_orders", "units"),
        ):
            self.assertEqual(interpretability_page.kind(name), kind, name)
        self.assertEqual({label for label, _ in interpretability_page.KINDS} | {"plan"}, set(interpretability_page.KIND_ORDER))

    def test_the_turn_is_read_off_this_tree_and_every_kind_has_steps(self):
        # The page's sentence is that the agent plans before it spends, fills
        # the queues after that and moves its units last: the steps of this
        # tree's own turn must still say so, or the sentence is to be reread.
        turn = interpretability_page.turn(ROOT)
        self.assertIsNotNone(turn)
        self.assertEqual(turn["kinds"], ["plan", "govern", "spend", "production", "units"])
        kinds = [step[2] for step in turn["steps"]]
        self.assertGreater(len(kinds), 100)
        for kind in turn["kinds"]:
            self.assertIn(kind, kinds)
        names = [step[0] for step in turn["steps"]]
        self.assertEqual(len(names), len(set(names)))
        for step in turn["steps"]:
            self.assertRegex(step[1], r"^[\w./-]+\.rs$")
        middle = lambda kind: sorted(i for i, k in enumerate(kinds) if k == kind)[kinds.count(kind) // 2]
        self.assertLess(middle("plan"), middle("spend"))
        self.assertLess(middle("spend"), middle("production"))
        self.assertLess(middle("production"), middle("units"))
        # The page names this step as the one that moves every unit.
        self.assertIn("advanced_units", names)
        self.assertIn("<code>advanced_units</code>", PAGE)
        self.assertGreater(turn["code"]["lines"], 50_000)
        self.assertGreater(turn["code"]["files"], 50)

    def test_a_tree_without_the_source_has_no_turn_and_says_so(self):
        with tempfile.TemporaryDirectory() as tmp, contextlib.redirect_stderr(io.StringIO()) as said:
            self.assertIsNone(interpretability_page.turn(pathlib.Path(tmp)))
        self.assertIn("no turn order", said.getvalue())


RANKING = """## A Ranking

| Rank | Gene | Description | Best version \\| Total versions | Default | P(>0) | Last | Prior | Third | Total (on) | Total (off) | Diff | Compute | Time |
|---:|---|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | `settler-never-idles` | A Settler always has somewhere to go: exhaustion asks wider questions instead of holding. | 1 \\| 1 | **on** | 90.9% | +42 | +61 | -16 | 17.10% (n=15,656) | 15.36% (n=5,176) | 1.74% | -1.67% ±0.89% | -1.82% ±1.08% |
| 2 | `muster-walks-the-road` | Opt-in gene `muster-walks-the-road`; see `Self::muster_walks_the_road`. | 1 \\| 1 | off | 51.6% | +1 | -7 | +10 | 16.71% (n=5,200) | 16.66% (n=15,632) | 0.05% | -0.70% ±1.16% | -0.71% ±1.41% |
| 3 | `campus-before-the-army-3` | `campus-before-the-army-3`: version 2, on through the University and the Research Lab. | 2 \\| 3 | off | 0.0% | -38 | -27 | -18 | v2 17.64% (n=1,763) · v1 15.03% (n=1,730) | v2 16.58% (n=19,069) · v1 16.81% (n=19,102) | -3.53% | -1.78% ±1.33% | -1.85% ±1.67% |
| 4 | `culture-keeps-pingala-home` | `culture-keeps-pingala-home`: see `advanced/culture_pingala_first.rs`. | 1 \\| 1 | off | 2.0% | -1 | -2 | -3 | 15.00% (n=5,000) | 17.00% (n=15,832) | -4.00% | +0.00% ±1.00% | +0.00% ±1.00% |
"""


class TheGenePool(unittest.TestCase):
    def ranked(self, text: str = RANKING) -> dict | None:
        with tempfile.TemporaryDirectory() as tmp:
            (pathlib.Path(tmp) / "GENE_HEURISTIC_RANKING.md").write_text(text, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                return interpretability_page.genes(pathlib.Path(tmp))

    def test_a_row_is_the_gene_its_default_its_difference_and_how_sure(self):
        genes = self.ranked()
        self.assertEqual(
            genes["rows"],
            [
                ["settler-never-idles", 1, 1.74, 90.9, "A Settler always has somewhere to go: exhaustion asks wider questions instead of holding."],
                # A description that says only where the code is, is left out.
                ["muster-walks-the-road", 0, 0.05, 51.6, ""],
                # A description that opens on the gene's own name loses it.
                ["campus-before-the-army-3", 0, -3.53, 0.0, "Version 2, on through the University and the Research Lab."],
                ["culture-keeps-pingala-home", 0, -4.0, 2.0, ""],
            ],
        )
        self.assertEqual(genes["on"], 1)
        # The most seats any one gene was priced on: on and off together.
        self.assertEqual(genes["seats"], 20_832)

    def test_a_long_description_is_cut_at_a_word(self):
        said = interpretability_page.described("x", "word " * 80)
        self.assertLessEqual(len(said), interpretability_page.DESCRIPTION_LIMIT + 1)
        self.assertTrue(said.endswith("word…"))

    def test_no_ranking_is_no_genes(self):
        with tempfile.TemporaryDirectory() as tmp, contextlib.redirect_stderr(io.StringIO()) as said:
            self.assertIsNone(interpretability_page.genes(pathlib.Path(tmp)))
        self.assertIn("no gene ranking", said.getvalue())
        self.assertIsNone(self.ranked("# nothing ranked\n"))

    def test_this_trees_ranking_is_read_whole(self):
        # The table is generated by `tools/genes.py write`; were its columns
        # to move, the page would lose its genes without a word.
        genes = interpretability_page.genes(ROOT)
        ranked = len(re.findall(r"^\| \d+ \|", (ROOT / "GENE_HEURISTIC_RANKING.md").read_text(encoding="utf-8"), re.M))
        self.assertGreater(ranked, 100)
        self.assertEqual(len(genes["rows"]), ranked)
        self.assertTrue(0 < genes["on"] < ranked)
        self.assertGreater(genes["seats"], 1000)
        self.assertGreater(sum(1 for row in genes["rows"] if row[4]), ranked // 4)


class ThePlans(unittest.TestCase):
    def snapshot(self, rows: list[dict], **more) -> dict:
        with tempfile.TemporaryDirectory() as tmp:
            batch(pathlib.Path(tmp), "20261121T184151Z-b41a", rows, **more)
            return interpretability_page.snapshot(pathlib.Path(tmp))

    def test_a_seat_holds_a_posture_until_it_writes_the_next(self):
        rows = [
            seat(1, 0, "science", [point(1, "expansion"), point(101, "science", "research")], end=110, win=True, forecasting=True),
            seat(1, 1, "culture", [point(1, "expansion"), point(11, "recovery")], end=110),
        ]
        taken = self.snapshot(rows)
        self.assertEqual((taken["games"], taken["seats"], taken["commit"], taken["began"]), (1, 2, "39c314e", "2026-11-21"))
        science, culture, everyone = (taken["targets"][name] for name in ("science", "culture", "all"))
        # Turns 1 to 100 on Expansion, 101 to 110 on Science: five turns a column.
        self.assertEqual(science["posture"]["expansion"][:21], [5] * 20 + [0])
        self.assertEqual(science["posture"]["science"][19:23], [0, 5, 5, 0])
        self.assertEqual(science["playing"][:23], [5] * 22 + [0])
        self.assertEqual(culture["posture"]["recovery"][:4], [0, 0, 5, 5])
        self.assertEqual(everyone["playing"][:23], [10] * 22 + [0])
        # The bottleneck is counted in the turns it was named on.
        self.assertEqual((science["bottleneck"], science["named"], science["turns"]), ({"research": 10}, 10, 110))
        self.assertEqual((science["forecasting"], culture["forecasting"], everyone["forecasting"]), (1, 0, 1))
        self.assertEqual((science["wins"], science["won_by"]["science"], culture["wins"]), (1, 1, 0))
        self.assertEqual(everyone["won_by"], {"science": 1, "culture": 0, "religious": 0, "diplomatic": 0, "domination": 0, "score": 0})

    def test_a_seat_that_did_not_live_is_counted_to_its_last_point(self):
        rows = [seat(1, 0, "score", [point(1, "expansion"), point(18, "recovery"), point(30, "recovery")], end=200, alive=False)]
        score = self.snapshot(rows)["targets"]["score"]
        self.assertEqual(score["turns"], 30)
        self.assertEqual(sum(score["playing"]), 30)
        self.assertEqual(sum(score["posture"]["recovery"]), 13)

    def test_the_marks_are_the_middle_seat_of_those_still_playing(self):
        rows = [
            seat(1, n, "culture", [point(1, "expansion")], marks={25: {"cities": cities, "science": 5.0, "culture": 4.0, "military": 50.0}})
            for n, cities in enumerate((2, 3, 9))
        ]
        culture = self.snapshot(rows)["targets"]["culture"]
        self.assertEqual(culture["marks"]["cities"][0], 3.0)
        self.assertEqual(culture["at_marks"][:2], [3, 0])
        self.assertIsNone(culture["marks"]["cities"][1])

    def test_the_newest_finished_batch_of_the_standard_table_is_the_one_read(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            one = [seat(1, 0, "science", [point(1, "expansion")])]
            batch(root, "20261120T000000Z-aaaa", one)
            batch(root, "20261121T000000Z-bbbb", one * 1 + [seat(2, 0, "culture", [point(1, "culture")])])
            # Newer still, but another table, and one still being played.
            batch(root, "20261122T000000Z-cccc", one, header={**HEADER, "turns": 500})
            batch(root, "20261123T000000Z-dddd", one, finished=False)
            taken = interpretability_page.snapshot(root)
            self.assertEqual((taken["batch"], taken["seats"]), ("20261121T000000Z-bbbb", 2))
            written = interpretability_page.written(taken)
            self.assertEqual(json.loads(written), taken)
            # A line a kind of seat, so a refresh's diff is the kinds.
            self.assertEqual(len(re.findall(r'^  "\w+": \{.*\},?$', written, re.M)), len(taken["targets"]))
        with tempfile.TemporaryDirectory() as tmp, self.assertRaises(SystemExit):
            interpretability_page.snapshot(pathlib.Path(tmp))

    def test_the_committed_snapshot_is_one_this_script_reads(self):
        with contextlib.redirect_stderr(io.StringIO()) as said:
            plans = interpretability_page.plans(ROOT)
        self.assertEqual(said.getvalue(), "")
        self.assertNotIn("about", plans)
        everyone = plans["targets"]["all"]
        self.assertEqual(everyone["seats"], sum(plans["targets"][name]["seats"] for name in interpretability_page.TARGETS))
        self.assertEqual(everyone["wins"], plans["games"])
        self.assertEqual(len(everyone["playing"]), interpretability_page.TURNS // interpretability_page.SPAN)
        for name, counted in plans["targets"].items():
            self.assertEqual(set(counted["posture"]), set(interpretability_page.POSTURES), name)
            self.assertEqual(sum(counted["won_by"].values()), counted["wins"], name)
            for column, playing in enumerate(counted["playing"]):
                self.assertEqual(sum(counted["posture"][p][column] for p in interpretability_page.POSTURES), playing, name)
        # The page's sentence about the chart, said of the snapshot it draws.
        share = lambda column: everyone["posture"]["expansion"][column] / everyone["playing"][column]
        self.assertGreater(share(0), 0.5)
        self.assertLess(share(29), share(19))
        self.assertIn("Most seats open on Expansion; the push for the victory starts near turn 100.", PAGE)
        # The page said until 2026-11-24 that a Culture seat gives up science
        # for culture. The sentence went with most of the page's words
        # (operator: "there is too much text on pages, particualrly home/ and
        # charts/"); the small charts still show it, of this snapshot.
        culture, science = plans["targets"]["culture"]["marks"], plans["targets"]["science"]["marks"]
        self.assertGreater(culture["culture"][5], science["culture"][5])
        self.assertLess(culture["science"][5], science["science"][5])
        self.assertNotIn("gives up science for culture", PAGE)

    def test_a_snapshot_taken_with_another_span_is_not_read(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "data" / "interpretability").mkdir(parents=True)
            taken = json.loads((ROOT / interpretability_page.PLANS).read_text(encoding="utf-8"))
            (root / interpretability_page.PLANS).write_text(json.dumps({**taken, "span": 10}), encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()) as said:
                self.assertIsNone(interpretability_page.plans(root))
            self.assertIn("rebuild it", said.getvalue())


class ThePage(unittest.TestCase):
    def test_the_record_is_written_into_the_slot_and_cannot_close_it(self):
        self.assertIn('<script type="application/json" id="interpretability-data">null</script>', PAGE)
        filled = interpretability_page.fill(PAGE, {"genes": {"rows": [["x", 1, 0.5, 60.0, "</script><b>"]]}})
        slot = interpretability_page.SLOT.search(filled)[2]
        self.assertNotIn("<", slot)
        self.assertEqual(json.loads(slot)["genes"]["rows"][0][4], "</script><b>")
        self.assertIsNone(interpretability_page.fill("<p>no slot</p>", {}))
        # A copy with no figures says so, and the record stays hidden.
        self.assertIn('<p class="res-empty" id="int-empty">This copy of the page has no figures.', PAGE)
        self.assertIn('<div class="res-record" id="int-record" hidden>', PAGE)

    def test_this_trees_record_has_its_three_parts(self):
        with contextlib.redirect_stderr(io.StringIO()):
            record = interpretability_page.interpretability(ROOT)
        self.assertEqual(sorted(key for key, value in record.items() if value), ["genes", "plans", "turn"])
        with tempfile.TemporaryDirectory() as tmp, contextlib.redirect_stderr(io.StringIO()):
            self.assertIsNone(interpretability_page.interpretability(pathlib.Path(tmp)))

    def test_the_script_draws_from_data_never_markup(self):
        self.assertNotIn("innerHTML", SCRIPT)
        self.assertNotIn("insertAdjacentHTML", SCRIPT)
        self.assertNotIn("document.write", SCRIPT)

    def test_every_name_the_snapshot_can_hold_has_its_colour_and_its_words(self):
        for posture in interpretability_page.POSTURES:
            self.assertIn(f"--p-{posture}: #", PAGE)
            self.assertRegex(SCRIPT, rf'\b{posture}: \["')
        for victory in interpretability_page.VICTORIES + ["civvis"]:
            self.assertIn(f"--v-{victory}: #", PAGE)
        for kind in interpretability_page.KIND_ORDER:
            self.assertIn(f"--k-{kind}: #", PAGE)
            self.assertIn(f".k-{kind} {{ background: var(--k-{kind}); }}", PAGE)
            self.assertRegex(SCRIPT, rf'\b{kind}: \["')
        for target in interpretability_page.TARGETS:
            self.assertRegex(SCRIPT, rf'\b{target}: "')
        for measure in interpretability_page.MEASURES:
            self.assertRegex(SCRIPT, rf'\b{measure}: "')
        # Every bottleneck the committed snapshot names is worded for a person.
        with contextlib.redirect_stderr(io.StringIO()):
            named = interpretability_page.plans(ROOT)["targets"]["all"]["bottleneck"]
        self.assertTrue(named)
        for bottleneck in named:
            self.assertRegex(SCRIPT, rf'\b{bottleneck}: "')


class TheSiteList(unittest.TestCase):
    def test_the_page_is_the_second_under_charts_ringed(self):
        # Operator 2026-11-22: "Add a new chart to the bottom of Charts/
        # called AI Interpretablity"; 2026-11-24: "move ai interpretablity to
        # the top of the charts/", so Charts/ opened it, until later that day
        # ("we want a new ai insights") AI Insights was put over it and
        # Charts/ opens that.
        nav = PAGE[PAGE.index('<nav class="site-nav"'):PAGE.index("</nav>")]
        self.assertEqual(
            re.findall(r'<a class="([^"]+)" href="([^"]+)"[^>]*>([^<]+)</a>', nav),
            [
                ("site-link", "../../home/", "Home"),
                ("site-link", "../ai-insights/", "Charts/"),
                ("site-link sub-link", "../ai-insights/", "AI Insights"),
                ("site-link sub-link", "./", "Civ AI Interpretability"),
                ("site-link sub-link", "../civ-analytics/", "Civ Analytics"),
                ("site-link sub-link", "../win-analytics/", "Win Analytics"),
                ("site-link sub-link", "../elo-rankings/", "ELO Rankings"),
                ("site-link sub-link", "../civvis-performance/", "CIVVIS Performance"),
                ("site-link sub-link", "../random-ai-charts/", "Random AI Charts"),
                ("site-link", "../../tournaments/", "Tournaments"),
                ("site-link", "../../", "CIVVIS"),
                ("site-link", "../../doctor-strange", "Doctor Strange's CIVVIS"),
                ("site-link", "../../edge-of-tomorrow", "CIVVIS: Edge of Tomorrow"),
                ("site-link", "../../debugger", "CIVVIS AI Debugger"),
                ("site-link", "https://store.steampowered.com/app/289070/Sid_Meiers_Civilization_VI/", "Civ 6"),
            ],
        )
        self.assertEqual(nav.count('aria-current="page"'), 1)
        self.assertIn('<a class="site-link sub-link" href="./" aria-current="page">Civ AI Interpretability</a>', nav)
        self.assertIn("  .site-folder:hover > .site-menu,\n  .site-folder:focus-within > .site-menu,\n  .site-folder.is-open > .site-menu { display: grid; }\n", PAGE)
        self.assertIn("<title>Civ AI Interpretability · CIVVIS.ai</title>", PAGE)
        # Every link of the page stays in its lane.
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])

    def test_the_header_is_the_one_every_page_carries(self):
        header = lambda page: re.sub(r"<!--.*?-->", "", page[page.index('<nav class="utility-nav"'):page.index("</header>")], flags=re.S)
        self.assertEqual(header(PAGE), header(PERFORMANCE))

    def test_charts_drops_it_down_second_on_every_page(self):
        # The second of Charts/'s pages in the list it drops down, after AI
        # Insights, on every page (operator 2026-11-24: "charts/ can drop
        # down folder style and the user can pick the right chart", then
        # "move ai interpretablity to the top of the charts/", then "we want
        # a new ai insights"); the viewer lists none.
        listed = ('        <div class="site-menu">\n'
                  '          <a class="site-link sub-link" href="../ai-insights/" title="What the games show about AI at large: '
                  'smarter AI picks up speed.">AI Insights</a>\n'
                  '          <a class="site-link sub-link" href="../civ-ai-interpretability/" title="How a CIVVIS AI agent decides: one turn '
                  'step by step, its plan through a game, and what each rule is worth in wins.">Civ AI Interpretability</a>\n')
        for page in (RESEARCH, PERFORMANCE):
            self.assertIn(listed, page)
        for page in (LANDING, TOURNAMENTS):
            self.assertIn(listed.replace('href="../', 'href="../charts/'), page)
        self.assertNotIn("Interpretability</a>", VIEWER)

    def test_publishing_builds_the_page_into_a_lane_whose_revision_has_it(self):
        # The lane's own page, filled by the lane's own script: a lane is one
        # revision's site, so a revision from before this page is published
        # without it (operator 2026-11-22: "1 test set of everything and 1
        # real prod set of everything"). It was given head's copy until then,
        # and this page answered on the front page the day it was written,
        # with nobody having promoted it.
        for line in (
            'interpretability_page="$source_tree/beta/interpretability.html"',
            'if [ -f "$interpretability_page" ]; then',
            # At the address the page states itself (operator 2026-11-23:
            # "clean up the URL please. standardized."), which is its name
            # in the site list, under Charts/; `interpretability` is where a
            # revision from before pages said so had it.
            'interpretability_at="$(address "$interpretability_page" interpretability)"',
            'mkdir -p "$out/$interpretability_at"',
            'cp "$interpretability_page" "$out/$interpretability_at/index.html"',
            'fill interpretability_page.py "$out/$interpretability_at/index.html"',
            'pages+=("$interpretability_at")',
        ):
            self.assertIn(line, PUBLISH)
        self.assertNotIn('"$repo_root/beta/interpretability', PUBLISH)
        self.assertIn('<link rel="canonical" href="https://civvis.ai/charts/civ-ai-interpretability/">', PAGE)
        verify = (BETA / "verify.py").read_text(encoding="utf-8")
        self.assertIn("""    "charts/civ-ai-interpretability": b'id="interpretability-data"',\n""", verify)
        # Two moves: out of the top of the site on 2026-11-23, and to the
        # name Civ AI Interpretability on 2026-11-24.
        self.assertIn("""    "interpretability": "charts/ai-interpretability",\n""", verify)
        self.assertIn("""    "charts/ai-interpretability": "charts/civ-ai-interpretability",\n""", verify)


if __name__ == "__main__":
    unittest.main()
