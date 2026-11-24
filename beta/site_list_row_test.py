#!/usr/bin/env python3
"""The site list is one row across the top of every page, and Charts/ drops
its pages down.

Operator, 2026-11-24: "try putting the vertical menu from home... civ across
the top instad. charts/ can drop down folder style and the user can pick the
right chart". Until then the list stood in a column to the left of the title,
and the pages of Charts/ were listed under it only on those pages.

So every page `publish.sh` publishes with a header (Home, Tournaments and the
pages of Charts/) carries the list first in its header, over the title, and
in it Charts/ is a folder: its own link still opens the folder's first page,
and a list of all its pages, the same on every page, drops down from it when
it is pointed at, tabbed to or, on a touch screen, tapped. The page on screen
is ringed in white, and on a page of Charts/ so is the folder.

The simulator keeps its list behind a button in a column: the map has the
top of the screen. (So does Wizard Chess's page, which is set aside since
2026-11-24 and is not published.)

    python3 beta/site_list_row_test.py
"""

from __future__ import annotations

import pathlib
import re
import unittest

BETA = pathlib.Path(__file__).resolve().parent
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")
SOURCES = sorted(set(re.findall(r'_page="\$source_tree/beta/([a-z_]+\.html)"', PUBLISH)))

#: Charts/'s pages, in the order its list gives them, by address (operator,
#: 2026-11-24: "move ai interpretablity to the top of the charts/ then civ
#: analytics, win analytics. elo rankings.. civvis performanc"; and, later
#: that day, "we want a new ai insights", which is listed over them).
CHARTS = [
    ("ai-insights", "AI Insights"),
    ("ai-interpretability", "AI Interpretability"),
    ("civ-analytics", "Civ Analytics"),
    ("win-analytics", "Win Analytics"),
    ("elo-rankings", "ELO Rankings"),
    ("civvis-performance", "CIVVIS Performance"),
]
LINK = re.compile(r'<a class="(site-link(?: sub-link)?)" href="([^"]*)"([^>]*)>([^<]+)</a>')


def headed() -> dict[str, str]:
    """The published pages whose header carries the site list."""
    found = {}
    for source in SOURCES:
        page = (BETA / source).read_text(encoding="utf-8")
        if '<header class="shell site-header">' in page and '<nav class="site-nav"' in page:
            found[source] = page
    return found


def address(page: str) -> str:
    return re.search(r'<link rel="canonical" href="https://civvis\.ai/([^"]+)">', page).group(1)


class Row(unittest.TestCase):
    def test_the_pages_with_a_header_are_found(self):
        # Discovered, so an empty glob fails rather than passes.
        self.assertGreaterEqual(set(headed()), {
            "landing.html", "tournaments.html", "research.html", "elo.html",
            "civs.html", "performance.html", "interpretability.html", "insights.html",
        })

    def test_the_list_is_first_in_the_header_over_the_title(self):
        for source, page in headed().items():
            header = page[page.index('<header class="shell site-header">'):page.index("</header>")]
            with self.subTest(page=source):
                self.assertRegex(header, r'^<header class="shell site-header">\s*<!--[\s\S]*?-->\s*<nav class="site-nav"')
                self.assertLess(header.index('<nav class="site-nav"'), header.index('<div class="site-brand">'))
                self.assertLess(header.index('<div class="site-brand">'), header.index('<h1 class="site-title">'))
                self.assertIn("  .site-nav { display: flex; flex: 1 0 100%; flex-wrap: wrap; align-items: center; gap: 6px; }\n", page)
                # Every name on one line, and nothing left of the column.
                self.assertIn("    white-space: nowrap;\n", page[page.index("  .site-link {"):])
                self.assertNotIn(".site-nav { display: grid", page)
                self.assertNotIn("width: 0; min-width: 100%", page)

    def test_charts_drops_the_same_list_down_on_every_page(self):
        for source, page in headed().items():
            here = address(page)
            nav = page[page.index('<nav class="site-nav"'):page.index("</nav>")]
            with self.subTest(page=source):
                self.assertEqual(nav.count('<div class="site-folder">'), 1)
                folder = nav[nav.index('<div class="site-folder">'):]
                top = LINK.search(folder)
                self.assertEqual((top.group(1), top.group(4)), ("site-link", "Charts/"))
                menu = folder[folder.index('<div class="site-menu">'):folder.index("</div>")]
                listed = LINK.findall(menu)
                self.assertEqual([name for _, _, _, name in listed], [name for _, name in CHARTS])
                self.assertTrue(all(cls == "site-link sub-link" for cls, _, _, _ in listed))
                # Each entry is lane-relative and lands on its page: `./` on
                # the page itself, a sibling from a page of Charts/, and
                # down into charts/ from Home and Tournaments.
                for (_, href, rest, name), (slug, _) in zip(listed, CHARTS):
                    if here == f"charts/{slug}/":
                        self.assertEqual((href, rest), ("./", ' aria-current="page"'))
                    elif here.startswith("charts/"):
                        self.assertEqual(href, f"../{slug}/")
                    else:
                        self.assertEqual(href, f"../charts/{slug}/")
                # Charts/ itself opens the folder's first page.
                self.assertEqual(top.group(2), listed[0][1])
                # Nothing but the folder's pages is set in as a sub-link.
                self.assertEqual(nav.count("sub-link"), len(CHARTS))

    def test_the_list_opens_by_pointer_keyboard_and_touch(self):
        for source, page in headed().items():
            with self.subTest(page=source):
                self.assertIn("  .site-menu {\n    position: absolute;", page)
                self.assertIn("    display: none;", page[page.index("  .site-menu {"):])
                self.assertIn(
                    "  .site-folder:hover > .site-menu,\n"
                    "  .site-folder:focus-within > .site-menu,\n"
                    "  .site-folder.is-open > .site-menu { display: grid; }\n",
                    page,
                )
                # A strip over the gap keeps the list open on the way down.
                self.assertIn('  .site-menu::before { content: ""; position: absolute; left: 0; right: 0; top: -7px; height: 7px; }\n', page)
                # Touch: the first tap opens, a tap elsewhere closes; a
                # pointer that can hover is left to follow the link.
                script = page[page.index("<script data-site-folder>"):]
                script = script[:script.index("</script>")]
                self.assertIn('matchMedia("(hover: hover)").matches', script)
                self.assertIn('event.preventDefault();\n        folder.classList.add("is-open");', script)
                self.assertIn('if (!folder.contains(event.target)) folder.classList.remove("is-open");', script)
                self.assertEqual(page.count("<script data-site-folder>"), 1)
                self.assertLess(page.index("</nav>"), page.index("<script data-site-folder>"))

    def test_the_page_on_screen_is_ringed_and_so_is_its_folder(self):
        for source, page in headed().items():
            nav = page[page.index('<nav class="site-nav"'):page.index("</nav>")]
            with self.subTest(page=source):
                self.assertEqual(nav.count('aria-current="page"'), 1)
                self.assertIn(
                    '  .site-link[aria-current="page"],\n'
                    '  .site-folder:has([aria-current="page"]) > .site-link { box-shadow: inset 0 0 0 2px #fff; }\n',
                    page,
                )


if __name__ == "__main__":
    unittest.main()
