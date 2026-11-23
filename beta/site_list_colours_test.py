#!/usr/bin/env python3
"""A button in the site list wears the ground of what it opens.

Operator, 2026-11-23: "make home, charts/ and auto-tourneys buttons all have
a blue background (like the pages do). civvis, ai debugger have a black
background (same as simulator)", and of the sixth button: "civ 6 can have the
steam color of blue. the background can be styled how you think steam is".

Until then a list was all one colour, the colour of the page it stood on:
six blue buttons on the pages, six black ones in the simulator. Now a button
says where it goes, and says the same thing on every page:

    Home, Charts/ and its pages, Auto-Tourneys   the pages' blue
    CIVVIS, AI Debugger, Doctor Strange's Civ 6  the simulator's black
    Civ 6                                        the blue of Steam's buttons

and the page on screen is still the one in gold.

The pages do it without a word in the markup, which a dozen tests pin as it
stands: a link that ends at the lane's own address (`../`, or two up from a
page of Charts/, bare for CIVVIS, with `?mode=play` for AI Debugger and with
`?mode=strange` for Doctor Strange's Civ 6, which is the same game shown as a
tree of two timelines) is a
link into the simulator, and a link to Steam's shop is Civ 6. So this reads
every page `publish.sh` publishes and holds each list to it: the three rules
are there, and they catch those three links and no other. The simulator's own
list names its links, and is held to the same colours by name.

    python3 beta/site_list_colours_test.py
"""

from __future__ import annotations

import pathlib
import re
import unittest

BETA = pathlib.Path(__file__).resolve().parent

PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")
VIEWER = (BETA.parent / "web" / "index.html").read_text(encoding="utf-8")

#: The pages `publish.sh` publishes, found in the script rather than listed
#: here: a page added to a lane is held to this the day it is added.
SOURCES = sorted(set(re.findall(r'_page="\$source_tree/beta/([a-z_]+\.html)"', PUBLISH)))

NAV = re.compile(r'<nav class="site-nav" aria-label="CIVVIS sites">(.*?)</nav>', re.S)
LINK = re.compile(r'<a class="(site-link(?: sub-link)?)" href="([^"]*)"([^>]*)>([^<]+)</a>')

#: The pages' blue, which was every button's and is still the list's own rule.
BLUE = (
    "    --action-bg: #274b66;\n"
    "    --action-fg: #a8d8f5;\n"
    "    --action-hover: #315d7e;\n"
)
#: The simulator's black is its ground (`--ink` in `web/index.html`), inside
#: the edge its own buttons have (`--line`).
BLACK = (
    '  .site-link[href$="../"],\n'
    '  .site-link[href$="?mode=play"],\n'
    '  .site-link[href$="?mode=strange"] { background: #050607; color: #c9cecb; box-shadow: inset 0 0 0 1px #30363a; }\n'
    '  .site-link[href$="../"]:hover, .site-link[href$="../"]:focus-visible,\n'
    '  .site-link[href$="?mode=play"]:hover, .site-link[href$="?mode=play"]:focus-visible,\n'
    '  .site-link[href$="?mode=strange"]:hover, .site-link[href$="?mode=strange"]:focus-visible { background: #181c1f; color: #fff; }\n'
)
#: Steam's button blue, the dark end under the name so the name reads.
STEAM = (
    '  .site-link[href^="https://store.steampowered.com/"] { background: linear-gradient(90deg, #1a44c2 35%, #47bfff); color: #fff; }\n'
    '  .site-link[href^="https://store.steampowered.com/"]:hover,\n'
    '  .site-link[href^="https://store.steampowered.com/"]:focus-visible { background: linear-gradient(90deg, #2352d6 35%, #73cfff); color: #fff; }\n'
)


def ground(href: str) -> str:
    """What the three rules make of a link, read as the browser reads them:
    `$=` is "ends with" and `^=` "starts with"."""
    if href.endswith("../") or href.endswith("?mode=play") or href.endswith("?mode=strange"):
        return "black"
    if href.startswith("https://store.steampowered.com/"):
        return "steam"
    return "blue"


def lists() -> dict[str, list[tuple[str, str]]]:
    """Every published page that carries the site list, with its links as
    (name, href) in the order they stand."""
    found = {}
    for source in SOURCES:
        page = (BETA / source).read_text(encoding="utf-8")
        nav = NAV.search(page)
        if nav:
            found[source] = [(name, href) for _, href, _, name in LINK.findall(nav.group(1))]
    return found


class Pages(unittest.TestCase):
    def test_the_list_is_found_on_the_pages_that_carry_it(self):
        # Discovered, so a glob that comes up empty is a failure and not a
        # pass: Home, Auto-Tourneys and the three pages of Charts/.
        carried = lists()
        self.assertGreaterEqual(len(carried), 5, sorted(carried))
        for source, links in carried.items():
            self.assertGreaterEqual(len(links), 7, source)
            # Every link in the list is one the rules were read against.
            page = (BETA / source).read_text(encoding="utf-8")
            self.assertEqual(len(links), NAV.search(page).group(1).count("<a "), source)

    def test_every_page_states_the_three_grounds(self):
        for source in lists():
            page = (BETA / source).read_text(encoding="utf-8")
            with self.subTest(page=source):
                self.assertIn(BLUE, page)
                self.assertIn("    background: var(--action-bg);\n    color: var(--action-fg);\n", page)
                self.assertEqual(page.count(BLACK), 1)
                self.assertEqual(page.count(STEAM), 1)
                # The page on screen is gold over whatever it would wear.
                self.assertIn('.site-link[aria-current="page"]:hover { background: var(--gold); color: #17130c; }', page)

    def test_a_button_wears_the_ground_of_what_it_opens(self):
        for source, links in lists().items():
            worn = {name: ground(href) for name, href in links}
            with self.subTest(page=source):
                self.assertEqual(
                    {name for name, kind in worn.items() if kind == "black"},
                    {"CIVVIS", "AI Debugger", "Doctor Strange's Civ 6"})
                self.assertEqual({name for name, kind in worn.items() if kind == "steam"}, {"Civ 6"})
                blue = {name for name, kind in worn.items() if kind == "blue"}
                self.assertLessEqual({"Home", "Charts/", "Auto-Tourneys"}, blue)
                # Whatever else is blue is a page of Charts/, set in under it.
                under = {name for kind, _, _, name in LINK.findall(NAV.search((BETA / source).read_text(encoding="utf-8")).group(1)) if kind.endswith("sub-link")}
                self.assertEqual(blue - {"Home", "Charts/", "Auto-Tourneys"}, under)

    def test_the_simulator_is_never_the_page_on_screen_here(self):
        # A black button is never also the gold one on these pages, so the
        # two rules, which weigh the same, never meet on one link.
        for source in lists():
            page = (BETA / source).read_text(encoding="utf-8")
            current = re.findall(r'href="([^"]*)" aria-current="page"', NAV.search(page).group(1))
            with self.subTest(page=source):
                self.assertTrue(current)
                for href in current:
                    self.assertEqual(ground(href), "blue", href)


class Simulator(unittest.TestCase):
    def test_its_list_names_the_links_the_rules_colour(self):
        for mark, name in (
            ("homelink", "Home"),
            ("chartslink", "Charts/"),
            ("tourlink", "Auto-Tourneys"),
            ("watchlink", "CIVVIS"),
            ("playlink", "AI Debugger"),
            ("strangelink", "Doctor Strange's Civ 6"),
            ("civ6link", "Civ 6"),
        ):
            self.assertRegex(VIEWER, rf'<a class="home-link" id="{mark}" [^>]*>{re.escape(name)}</a>')

    def test_the_pages_are_blue_and_civ_6_is_steam(self):
        # The same blue as the pages' own lists, and the same Steam blue.
        self.assertIn("  #homelink, #chartslink, #tourlink { color: #a8d8f5; border-color: #3b6788; background: #274b66; }\n", VIEWER)
        self.assertIn("  #homelink:hover, #chartslink:hover, #tourlink:hover { color: #fff; border-color: #66c0f4; background: #315d7e; }\n", VIEWER)
        self.assertIn(
            "  #civ6link {\n"
            "    color: #fff; border-color: transparent;\n"
            "    background: linear-gradient(90deg, #1a44c2 35%, #47bfff) border-box;\n"
            "  }\n",
            VIEWER,
        )

    def test_its_own_two_links_keep_the_black(self):
        # CIVVIS, AI Debugger and Doctor Strange's Civ 6 wear the list's own
        # rule, the simulator's panel inside its line, and nothing recolours
        # any of them but the gold of the page on screen.
        rule = VIEWER.split("  .home-link {\n", 1)[1].split("  }\n", 1)[0]
        self.assertIn("border: 1px solid var(--line); border-radius: 6px; background: var(--panel-2);", rule)
        self.assertIn("    --panel-2: #111416;\n", VIEWER)
        rules = [chunk.rpartition("{") for chunk in VIEWER.split("}")]
        for mark in ("watchlink", "playlink", "strangelink"):
            coloured = [
                selector for selector, _, body in rules
                if f"#{mark}" in selector and ("color" in body or "background" in body)
            ]
            self.assertEqual(len(coloured), 1, mark)
            self.assertIn(
                "body.watching-sim #watchlink,\n"
                "  html.seat-known:not(.strange) body:not(.watching-sim) #playlink,\n"
                "  html.seat-known.strange body:not(.watching-sim) #strangelink",
                coloured[0])
        # The ground the pages' black buttons are set in is this page's own.
        self.assertIn("    --ink: #050607;\n", VIEWER)
        self.assertIn("    --line: #30363a;\n", VIEWER)


if __name__ == "__main__":
    unittest.main()
