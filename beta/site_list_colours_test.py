#!/usr/bin/env python3
"""A button in the site list wears the ground of what it opens.

Operator, 2026-11-23: "make home, charts/ and auto-tourneys buttons all have
a blue background (like the pages do). civvis, ai debugger have a black
background (same as simulator)", and of the sixth button: "civ 6 can have the
steam color of blue. the background can be styled how you think steam is".
Later the same day, of Doctor Strange's Civ 6: "highlight dr strange civ 6
button in dr strange colors". On 2026-11-24, of CIVVIS: "lets give civvis a
gold background on the button, rather than black".

Until then a list was all one colour, the colour of the page it stood on:
six blue buttons on the pages, six black ones in the simulator. Now a button
says where it goes, and says the same thing on every page:

    Home, Charts/ and its pages, Tournaments     the pages' blue
    CIVVIS                                       gold
    AI Debugger                                  the simulator's black
    Doctor Strange's Civ 6                       Doctor Strange's blue and
                                                 crimson, in an orange glow
    Civ 6                                        the blue of Steam's buttons

and the page on screen keeps its colour and is ringed in white (it was gold
until gold became CIVVIS's).

The pages do it without a word in the markup, which a dozen tests pin as it
stands: a link that ends at the lane's own address (`../`, or two up from a
page of Charts/, bare for CIVVIS and with `?mode=play` for AI Debugger) is a
link into the simulator, one that ends `?mode=strange` is Doctor Strange's
Civ 6 (the same game shown as a tree of two timelines), and a link to Steam's
shop is Civ 6. So this reads every page `publish.sh` publishes and holds each
list to it: the four rules are there, and they catch those links and no other. The simulator's own
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
#: CIVVIS, the simulation at the lane's own address, is the gold the
#: simulator lit the page on screen in: the same gold on every page.
GOLD = (
    '  .site-link[href$="../"] { background: linear-gradient(145deg, #dec176, #aa7d30); color: #17130c; box-shadow: inset 0 0 0 1px #efd184; }\n'
    '  .site-link[href$="../"]:hover, .site-link[href$="../"]:focus-visible { background: linear-gradient(145deg, #f0d68e, #c0913c); color: #000; }\n'
)
#: AI Debugger is the simulator's black, its ground (`--ink` in
#: `web/index.html`), inside the edge its own buttons have (`--line`).
BLACK = (
    '  .site-link[href$="?mode=play"] { background: #050607; color: #c9cecb; box-shadow: inset 0 0 0 1px #30363a; }\n'
    '  .site-link[href$="?mode=play"]:hover, .site-link[href$="?mode=play"]:focus-visible { background: #181c1f; color: #fff; }\n'
)
#: The page on screen: its own colour, ringed in white.
RING = '  .site-link[aria-current="page"]'
#: Doctor Strange's own colours: the deep blue of his tunic into the crimson
#: of his cloak, ringed and lit by the orange of his spells' sparks, the name
#: in their pale gold. The simulator states the same colours by name.
TUNIC, CLOAK, SPARK, NAME = "#1c2a7a", "#a3122f", "#ff9a2e", "#ffe3a3"
STRANGE = (
    '  .site-link[href$="?mode=strange"] {\n'
    f'    background: linear-gradient(135deg, {TUNIC} 30%, {CLOAK}); color: {NAME};\n'
    f'    box-shadow: inset 0 0 0 1px {SPARK}, 0 0 6px rgba(255, 138, 30, 0.6);\n'
    '  }\n'
    '  .site-link[href$="?mode=strange"]:hover,\n'
    '  .site-link[href$="?mode=strange"]:focus-visible {\n'
    '    background: linear-gradient(135deg, #2638a0 30%, #c8193d); color: #fff;\n'
    '    box-shadow: inset 0 0 0 1px #ffb85c, 0 0 10px rgba(255, 150, 40, 0.85);\n'
    '  }\n'
)
#: Steam's button blue, the dark end under the name so the name reads.
STEAM = (
    '  .site-link[href^="https://store.steampowered.com/"] { background: linear-gradient(90deg, #1a44c2 35%, #47bfff); color: #fff; }\n'
    '  .site-link[href^="https://store.steampowered.com/"]:hover,\n'
    '  .site-link[href^="https://store.steampowered.com/"]:focus-visible { background: linear-gradient(90deg, #2352d6 35%, #73cfff); color: #fff; }\n'
)
#: Wizard Chess stood last in the list from 2026-11-23, on a chessboard of a
#: button, and was set aside on 2026-11-24 (operator: "lets set aside wizard
#: chess right now. please remove from civvis.ai"). No list names it and no
#: rule colours it: neither its name nor its address is on a page.
SET_ASIDE = re.compile(r"(?i)wizard.chess|chesslink")


def ground(href: str) -> str:
    """What the three rules make of a link, read as the browser reads them:
    `$=` is "ends with" and `^=` "starts with"."""
    if href.endswith("../"):
        return "gold"
    if href.endswith("?mode=play"):
        return "black"
    if href.endswith("?mode=strange"):
        return "strange"
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
        # pass: Home, Tournaments and the pages of Charts/.
        carried = lists()
        self.assertGreaterEqual(len(carried), 5, sorted(carried))
        for source, links in carried.items():
            self.assertGreaterEqual(len(links), 7, source)
            # Every link in the list is one the rules were read against.
            page = (BETA / source).read_text(encoding="utf-8")
            self.assertEqual(len(links), NAV.search(page).group(1).count("<a "), source)

    def test_every_page_states_the_four_grounds(self):
        for source in lists():
            page = (BETA / source).read_text(encoding="utf-8")
            with self.subTest(page=source):
                self.assertIn(BLUE, page)
                self.assertIn("    background: var(--action-bg);\n    color: var(--action-fg);\n", page)
                self.assertEqual(page.count(GOLD), 1)
                self.assertEqual(page.count(BLACK), 1)
                self.assertEqual(page.count(STRANGE), 1)
                self.assertEqual(page.count(STEAM), 1)
                self.assertNotRegex(page, SET_ASIDE)
                # The strange rule comes after the black, so nothing from
                # the simulator's black is left on it at equal weight.
                self.assertLess(page.index(GOLD), page.index(BLACK))
                self.assertLess(page.index(BLACK), page.index(STRANGE))
                # The page on screen keeps what it would wear, ringed in
                # white, and the ring is the last rule, so it wins.
                self.assertNotIn("var(--gold)", page[page.index("  .site-link {"):page.index(RING)])
                ring = page[page.index(RING):]
                self.assertIn("{ box-shadow: inset 0 0 0 2px #fff; }\n", ring[:ring.index("}") + 2])

    def test_a_button_wears_the_ground_of_what_it_opens(self):
        for source, links in lists().items():
            worn = {name: ground(href) for name, href in links}
            with self.subTest(page=source):
                self.assertEqual({name for name, kind in worn.items() if kind == "gold"}, {"CIVVIS"})
                self.assertEqual({name for name, kind in worn.items() if kind == "black"}, {"AI Debugger"})
                self.assertEqual({name for name, kind in worn.items() if kind == "strange"}, {"Doctor Strange's Civ 6"})
                self.assertEqual({name for name, kind in worn.items() if kind == "steam"}, {"Civ 6"})
                # The list ends at Civ 6: Wizard Chess, which stood below
                # it, is set aside.
                self.assertEqual(list(worn)[-1], "Civ 6")
                self.assertNotIn("Wizard Chess", worn)
                blue = {name for name, kind in worn.items() if kind == "blue"}
                self.assertLessEqual({"Home", "Charts/", "Tournaments"}, blue)
                # Whatever else is blue is a page of Charts/, set in under it.
                under = {name for kind, _, _, name in LINK.findall(NAV.search((BETA / source).read_text(encoding="utf-8")).group(1)) if kind.endswith("sub-link")}
                self.assertEqual(blue - {"Home", "Charts/", "Tournaments"}, under)

    def test_the_simulator_is_never_the_page_on_screen_here(self):
        # The page on screen here is always one of the blue pages.
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
            ("tourlink", "Tournaments"),
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

    def test_wizard_chess_is_set_aside(self):
        # The list is the seven names above and ends at Civ 6; nothing in
        # the simulator names Wizard Chess or the link it had.
        nav = VIEWER[VIEWER.index('<nav class="head-links" id="headlinks"'):]
        nav = nav[:nav.index("</nav>")]
        self.assertEqual(
            re.findall(r'<a class="home-link" id="([a-z0-9]+)"', nav),
            ["homelink", "chartslink", "tourlink", "watchlink", "strangelink", "playlink", "civ6link"])
        self.assertNotRegex(VIEWER, SET_ASIDE)

    def test_doctor_strange_is_in_his_colours(self):
        # The same colours as on the pages, by name; the gold of the page on
        # screen, which weighs more, still lights it in strange mode.
        self.assertIn(
            "  #strangelink {\n"
            f"    color: {NAME}; border-color: {SPARK};\n"
            f"    background: linear-gradient(135deg, {TUNIC} 30%, {CLOAK}) border-box;\n"
            "    box-shadow: 0 0 6px rgba(255, 138, 30, 0.6);\n"
            "  }\n",
            VIEWER,
        )
        self.assertIn("  #strangelink:hover { color: #fff; border-color: #ffb85c; "
                      "background: linear-gradient(135deg, #2638a0 30%, #c8193d) border-box; "
                      "box-shadow: 0 0 10px rgba(255, 150, 40, 0.85); }\n", VIEWER)
        rules = [chunk.rpartition("{") for chunk in VIEWER.split("}")]
        coloured = [selector.strip() for selector, _, body in rules
                    if "#strangelink" in selector and ("color" in body or "background" in body)]
        self.assertEqual(len(coloured), 3, coloured)
        self.assertIn("html.seat-known.strange body:not(.watching-sim) #strangelink", coloured[-1])

    def test_civvis_is_gold(self):
        # The same gold as on the pages, by name.
        self.assertIn("  #watchlink { color: #17130c; border-color: #efd184; "
                      "background: linear-gradient(145deg, #dec176, #aa7d30); }\n", VIEWER)
        self.assertIn("  #watchlink:hover { color: #000; border-color: #fff3c9; "
                      "background: linear-gradient(145deg, #f0d68e, #c0913c); }\n", VIEWER)
        self.assertIn(GOLD.split(" { ", 1)[1].split(";", 1)[0], VIEWER)

    def test_the_page_on_screen_is_ringed_in_white(self):
        # Whichever of its three pages this is keeps its own colour and is
        # ringed in white; nothing here is lit gold for being on screen.
        self.assertIn(
            "  body.watching-sim #watchlink,\n"
            "  html.seat-known:not(.strange) body:not(.watching-sim) #playlink,\n"
            "  html.seat-known.strange body:not(.watching-sim) #strangelink {\n"
            "    border-color: #fff; box-shadow: inset 0 0 0 1px #fff;\n"
            "  }\n",
            VIEWER,
        )

    def test_ai_debugger_keeps_the_black(self):
        # AI Debugger wears the list's own rule, the simulator's panel
        # inside its line, and nothing recolours it.
        rule = VIEWER.split("  .home-link {\n", 1)[1].split("  }\n", 1)[0]
        self.assertIn("border: 1px solid var(--line); border-radius: 6px; background: var(--panel-2);", rule)
        self.assertIn("    --panel-2: #111416;\n", VIEWER)
        rules = [chunk.rpartition("{") for chunk in VIEWER.split("}")]
        coloured = [
            selector for selector, _, body in rules
            if "#playlink" in selector and ("background" in body or "color:" in body.replace("border-color:", ""))
        ]
        self.assertEqual(coloured, [], coloured)
        # The ground the pages' black buttons are set in is this page's own.
        self.assertIn("    --ink: #050607;\n", VIEWER)
        self.assertIn("    --line: #30363a;\n", VIEWER)


if __name__ == "__main__":
    unittest.main()
