#!/usr/bin/env python3
"""Unreviewed Gen AI Charts: the charts an agent added to the other pages of Charts/
on its own, moved to one page at the end of Charts/ (operator, 2026-11-26:
"please add another chart to the end of charts/ called random ai charts.
please move all the ai generated charts that i havent approved of or we have
discussed to here").

A chart here is the one that stood on its page, with its markup, its rules
and its script, and it reads the same record: the page carries the slot of
each page its charts came from, and the publish fills them with that page's
own script.

    python3 beta/random_page_test.py
"""

from __future__ import annotations

import html.parser
import importlib
import pathlib
import re
import unittest

BETA = pathlib.Path(__file__).resolve().parent
PAGE = (BETA / "random.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")

#: The groups, in the order of Charts/: the group's key, the slot its
#: page's record is written into, the script that writes it, the page it
#: came from (file, address under charts/, name), and the charts that moved,
#: by the id each is found by. Those ids are on this page and on no other.
GROUPS = [
    ("civs", "civ-data", "civs_page", "civs.html", "civ-analytics", "Civ Analytics",
     ["civ-victories"]),
    ("research", "research-data", "research_page", "research.html", "win-analytics", "Win Analytics",
     ["res-each", "res-table"]),
    ("elo", "elo-data", "elo_page", "elo.html", "elo-rankings", "ELO Rankings",
     ["elo-levels"]),
    ("perf", "performance-data", "performance_page", "performance.html", "civvis-performance", "CIVVIS Performance",
     ["perf-path-steps", "perf-spans-plot", "perf-batches"]),
]


def group(key: str) -> str:
    """The markup of one group, its opening tag to the next group's."""
    start = PAGE.index(f'<section class="rnd-group rnd-{key}"')
    later = [PAGE.find('<section class="rnd-group', start + 1), PAGE.find("</main>", start)]
    return PAGE[start:min(i for i in later if i > 0)]


def style() -> str:
    return PAGE[PAGE.index("<style>"):PAGE.index("</style>")]


class ThePage(unittest.TestCase):
    def test_it_is_under_charts_before_project_chronology_and_ringed(self):
        self.assertIn("<title>Unreviewed Gen AI Charts · CIVVIS.ai</title>", PAGE)
        self.assertIn('<link rel="canonical" href="https://civvis.ai/charts/unreviewed-gen-ai-charts/">', PAGE)
        nav = PAGE[PAGE.index('<nav class="site-nav"'):PAGE.index("</nav>")]
        menu = nav[nav.index('<div class="site-menu">'):nav.index("</div>", nav.index('<div class="site-menu">'))]
        names = re.findall(r'<a class="site-link sub-link" href="([^"]+)"[^>]*>([^<]+)</a>', menu)
        # The last page under Charts/ until 2026-11-28, when Project
        # Chronology was put after it (operator: "add a project chronology
        # to charts/ at the end for now").
        self.assertEqual(names[-2:], [("./", "Unreviewed Gen AI Charts"), ("../project-chronology/", "Project Chronology")])
        self.assertEqual(
            [name for _, name in names],
            ["AI Insights", "Civ AI Interpretability", "Civ Analytics", "Win Analytics", "ELO Rankings",
             "CIVVIS Performance", "Unreviewed Gen AI Charts", "Project Chronology"],
        )
        self.assertIn('<a class="site-link sub-link" href="./" aria-current="page">Unreviewed Gen AI Charts</a>', nav)
        # Charts/ itself opens the folder's first page, as on every page.
        self.assertIn('<a class="site-link" href="../ai-insights/" ', nav)

    def test_it_opens_on_one_line(self):
        intro = re.search(r'<p class="intro">\s*(.*?)\s*</p>', PAGE, re.S)[1]
        self.assertEqual(intro, "Charts the AI agents building this site added on their own, gathered here until they are asked for.")

    def test_the_publish_places_it_last_under_charts_and_fills_its_slots(self):
        self.assertIn('random_page="$source_tree/beta/random.html"', PUBLISH)
        self.assertIn('random_at="$(address "$random_page" charts/unreviewed-gen-ai-charts)"', PUBLISH)
        # Its block follows CIVVIS Performance's, so the lane lists it last
        # under Charts/ and `/charts` still opens AI Insights.
        self.assertLess(PUBLISH.index('pages+=("$performance_at")'), PUBLISH.index('pages+=("$random_at")'))
        loop = re.search(r'for filler in ([a-z_. ]+); do\n    fill "\$filler" "\$out/\$random_at/index.html"', PUBLISH)
        self.assertIsNotNone(loop)
        self.assertEqual(sorted(loop[1].split()), sorted(f"{script}.py" for _, _, script, *_ in GROUPS))


class TheCharts(unittest.TestCase):
    def test_the_groups_stand_in_the_order_of_charts(self):
        self.assertEqual(re.findall(r'<section class="rnd-group rnd-([a-z]+)"', PAGE), [g[0] for g in GROUPS])

    def test_each_group_is_named_for_its_page_and_links_back(self):
        for key, slot, _, _, at, name, _ in GROUPS:
            with self.subTest(group=key):
                markup = group(key)
                self.assertIn(f'data-slot="{slot}"', markup)
                self.assertIn(f'<p class="rnd-from" id="rnd-{key}-from">From <a href="../{at}/">{name}</a></p>', markup)
                # Folded away until its record arrives.
                self.assertIn(" hidden>", markup[:markup.index(">") + 1])

    def test_each_group_closes_what_it_opens(self):
        # A chart's markup is pasted in from its page: one closing tag too
        # many there (a row's `</div>` it once stood in) would close the
        # group and the record around it, and the next group would fall out
        # of the page's grid.
        class Tags(html.parser.HTMLParser):
            VOID = {"meta", "link", "br", "img", "input", "hr", "source", "col", "wbr"}

            def __init__(self):
                super().__init__()
                self.open, self.wrong = [], []

            def handle_starttag(self, tag, attrs):
                if tag not in self.VOID:
                    self.open.append(tag)

            def handle_endtag(self, tag):
                if tag in self.VOID:
                    return
                if self.open and self.open[-1] == tag:
                    self.open.pop()
                else:
                    self.wrong.append((tag, self.getpos(), self.open[-1:] or None))

        for key, *_ in GROUPS:
            with self.subTest(group=key):
                markup = group(key)
                tags = Tags()
                tags.feed(markup[:markup.rindex("</section>") + len("</section>")])
                self.assertEqual((tags.wrong, tags.open), ([], []))

    def test_each_moved_chart_is_here_and_not_on_its_page(self):
        for key, _, _, source, _, _, moved in GROUPS:
            page = (BETA / source).read_text(encoding="utf-8")
            for chart in moved:
                with self.subTest(chart=chart):
                    self.assertIn(f'id="{chart}"', group(key))
                    self.assertNotIn(f'id="{chart}"', page)

    def test_each_slot_is_the_one_its_pages_script_fills(self):
        # The record is the page's own: a chart reads here what it read there.
        for key, slot, script, *_ in GROUPS:
            with self.subTest(group=key):
                self.assertEqual(PAGE.count(f'<script type="application/json" id="{slot}">null</script>'), 1)
                filler = importlib.import_module(script)
                filled = filler.fill(PAGE, {"marker": key})
                self.assertIsNotNone(filled, f"{script}.fill finds no slot here")
                self.assertIn(f'<script type="application/json" id="{slot}">{{"marker":"{key}"}}</script>', filled)

    def test_each_groups_rules_are_held_to_the_group(self):
        # Two pages may give one class two looks (`.rank-title`, `.res-section`):
        # each group's rules name its group first, so they cannot meet.
        css = style()
        blocks = re.split(r"\n  /\* From ([A-Za-z ]+)\. \*/\n", css)
        named = dict(zip(blocks[1::2], blocks[2::2]))
        self.assertEqual(list(named), [g[5] for g in GROUPS])
        for key, *_, name, _ in GROUPS:
            with self.subTest(group=key):
                body = re.sub(r"/\*.*?\*/", "", named[name], flags=re.S)
                for selectors in re.findall(r"([^{}]+)\{[^{}]*\}", re.sub(r"@media[^{]*\{", "", body)):
                    for selector in selectors.split(","):
                        selector = selector.strip()
                        if not selector or selector.startswith(("from", "to", "@")) or re.fullmatch(r"[\d.]+%", selector):
                            continue
                        self.assertTrue(selector.startswith(f".rnd-{key}"), f"{name}: {selector!r}")

    def test_each_groups_script_reads_its_own_slot(self):
        scripts = re.findall(r"<script>\n(.*?)\n  </script>", PAGE, re.S)
        for key, slot, *_ in GROUPS:
            with self.subTest(group=key):
                self.assertEqual(sum(f'document.getElementById("{slot}")' in s for s in scripts), 1)

    def test_a_group_is_shown_before_its_charts_are_drawn(self):
        # A chart is drawn at the width its card is given; a hidden card has none.
        shower = PAGE.index('document.querySelectorAll(".rnd-group[data-slot]")')
        for key, slot, *_ in GROUPS:
            self.assertLess(shower, PAGE.rindex(f'document.getElementById("{slot}")'))


class TheEloLevels(unittest.TestCase):
    def test_each_reading_has_its_benchmark_and_simulator_side_by_side(self):
        # As it stood on ELO Rankings beside the strategies until 2026-11-26.
        levels = PAGE[PAGE.index('id="elo-levels-table"'):PAGE.index("</thead>", PAGE.index('id="elo-levels-table"'))]
        self.assertEqual(re.findall(r"<span>(Diplo o\w+)</span>", levels), ["Diplo on", "Diplo off"])
        self.assertEqual(re.findall(r'<th scope="col"[^>]*>(\w+)</th>', levels),
                         ["Level", "Benchmark", "Simulator", "Benchmark", "Simulator"])
        self.assertIn('<h2 class="rank-title" id="elo-levels-title">Firaxis Civ 6 AI: Elo by level</h2>', PAGE)


if __name__ == "__main__":
    unittest.main()
