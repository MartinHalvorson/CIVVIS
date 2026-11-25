#!/usr/bin/env python3
"""Chaos Monkey: the page of worlds played under a very high frequency of
natural disasters, its place in the site list, and the dial it turns.

    python3 beta/chaos_page_test.py
"""

from __future__ import annotations

import contextlib
import io
import json
import pathlib
import re
import tempfile
import unittest

import chaos_page
import verify

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "chaos.html").read_text(encoding="utf-8")
EXPERIMENTS = (BETA / "tournaments.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")
SHIM = (BETA / "shim.js").read_text(encoding="utf-8")
VIEWER = (ROOT / "web" / "index.html").read_text(encoding="utf-8")
SETUP = (ROOT / "web" / "assets" / "app_setup.js").read_text(encoding="utf-8")
APP = (ROOT / "web" / "assets" / "app.js").read_text(encoding="utf-8")
RECORD = json.loads((ROOT / "data" / "chaos_monkey.json").read_text(encoding="utf-8"))

#: The pages that carry the site list, found rather than listed: every page
#: in beta/ with the row of sites.
LISTED = {path.name: path.read_text(encoding="utf-8") for path in sorted(BETA.glob("*.html")) if '<nav class="site-nav"' in path.read_text(encoding="utf-8")}


def site_list(page: str) -> list[tuple[str, str]]:
    """A page's top-level sites, in order: (address as written, name)."""
    nav = page[page.index('<nav class="site-nav"'):]
    nav = nav[:nav.index("</nav>")]
    return [(href, name) for cls, href, name in re.findall(r'<a class="(site-link)" href="([^"]+)"[^>]*>([^<]+)</a>', nav)]


class TheRecord(unittest.TestCase):
    def test_the_trees_record_is_one_the_page_can_draw(self):
        data = chaos_page.record(ROOT)
        self.assertEqual(data, RECORD)
        doses = data["doses"]
        self.assertGreaterEqual(len(doses), 2)
        self.assertEqual(doses[0]["frequency"], 1)
        # Every frequency has the five shares and the three figures a game
        # the page draws, and every one past 1x its range and whether the
        # games settle it.
        for dose in doses:
            for key in ("games", "disasters", "kept", "wars", "captures", "won"):
                self.assertIn(key, dose)
            measured = [*dose["kept"].values(), dose["wars"], dose["captures"], dose["won"]]
            self.assertEqual(len(measured), 8)
            for figure in measured:
                self.assertIsInstance(figure["value"], (int, float))
                if dose is not doses[0]:
                    self.assertLessEqual(figure["low"], figure["high"])
                    self.assertIsInstance(figure["sure"], bool)
        # Every lesson has its parts, its range among them.
        for lesson in data["lessons"]:
            for key in ("figure", "of", "range", "title"):
                self.assertTrue(lesson[key])
        self.assertEqual(data["tested"]["figures"], 8 * (len(doses) - 1))

    def test_no_record_is_no_figures_and_never_an_error(self):
        err = io.StringIO()
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(err):
            tree = pathlib.Path(scratch)
            self.assertIsNone(chaos_page.record(tree))
            (tree / "data").mkdir()
            (tree / "data" / "chaos_monkey.json").write_text("{not json", encoding="utf-8")
            self.assertIsNone(chaos_page.record(tree))
            # One frequency is nothing to compare.
            (tree / "data" / "chaos_monkey.json").write_text(json.dumps({"doses": [RECORD["doses"][0]]}), encoding="utf-8")
            self.assertIsNone(chaos_page.record(tree))
        self.assertIn("is not there", err.getvalue())
        self.assertIn("cannot be read", err.getvalue())
        self.assertIn("fewer than two frequencies", err.getvalue())

    def test_the_record_is_written_into_the_pages_slot(self):
        with tempfile.TemporaryDirectory() as scratch:
            page = pathlib.Path(scratch) / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                self.assertEqual(chaos_page.main(["chaos_page.py", str(ROOT), str(page)]), 0)
            written = json.loads(chaos_page.SLOT.search(page.read_text(encoding="utf-8"))[2])
            self.assertEqual(written, RECORD)
            # Nothing but the slot moved.
            self.assertEqual(chaos_page.SLOT.sub(r"\1null\3", page.read_text(encoding="utf-8")), PAGE)
        self.assertEqual(out.getvalue(), "")
        self.assertIn("chaos_page: wrote the Chaos Monkey games (1x ", err.getvalue())
        # A name or a sentence cannot close the script element.
        self.assertIn("\\u003c", chaos_page.fill(PAGE, {"lessons": "</script>"}))
        self.assertIsNone(chaos_page.fill("<p>no slot</p>", {}))
        self.assertEqual(PAGE.count('<script type="application/json" id="chaos-data">null</script>'), 1)


class ThePage(unittest.TestCase):
    def test_it_is_chaos_monkey_at_the_address_its_name_makes(self):
        self.assertIn("<title>Chaos Monkey · CIVVIS.ai</title>", PAGE)
        self.assertIn('<link rel="canonical" href="https://civvis.ai/chaos-monkey/">', PAGE)
        # Every link of the page stays in its lane.
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])
        # It opens on one line that names no other page: the list over it does.
        intro = PAGE[PAGE.index('<p class="intro">'):]
        intro = intro[:intro.index("</p>")]
        self.assertNotIn("<a ", intro)
        self.assertLessEqual(len(re.sub(r"<[^>]+>", "", intro).split()), 30)

    def test_it_opens_on_what_the_worlds_teach(self):
        # Operator 2026-11-25: "what can we learn about avoiding horrible
        # outcomes?" The lessons come first, then what they are read off.
        order = [PAGE.index(f'id="{section}"') for section in ("cm-learned", "cm-gives", "cm-war", "cm-method")]
        self.assertEqual(order, sorted(order))
        self.assertIn('<h2 class="cm-title" id="cm-learned-title">What the broken worlds teach</h2>', PAGE)
        self.assertIn('<h2 class="cm-title" id="cm-war-title">War and victory</h2>', PAGE)

    def test_it_draws_only_what_the_statistics_back(self):
        # Operator 2026-11-25: "clean up the chaos engineering stuf ffor now
        # please. the charts specifically. only things that are highly
        # backed by statistics". Gone: the dial with its tiles and kinds of
        # disaster, the world turn by turn, the outcomes no test could tell
        # from chance (lost half their people, wiped out, nuclear strikes,
        # the largest civilization's share, climate change), the six ways the
        # games ended, and who comes through.
        for gone in ("cm-dose", "cm-dial", "cm-tiles", "cm-kinds", "cm-time", "cm-people-plot", "cm-wreck-plot",
                     "cm-bad", "cm-ends", "cm-who", "cm-factors"):
            self.assertNotIn(f'id="{gone}"', PAGE)
        for gone in ("collapsed", "eliminated", "nukes", "climate", "same_winner", "factors"):
            self.assertNotIn(gone, PAGE)
        # Every figure under chaos is drawn with its 95% range, and hollow
        # where the games do not settle it.
        self.assertIn("const counts = (dose, measured) => dose === calm || Boolean(measured && measured.sure);", PAGE)
        self.assertIn('{ class: "cm-dot is-hollow", cx: px(at), cy: y(measured.value), r: 4, style: `stroke: ${line.colour}` }', PAGE)
        self.assertIn(': { fill: "transparent", stroke: colourOf(dose), "stroke-width": 1.5, d: shape }));', PAGE)
        self.assertEqual(PAGE.count('class: "cm-whisker"'), 6)
        self.assertIn('item.append(el("p", "cm-lesson-range", `95% range: ${lesson.range}`));', PAGE)

    def test_the_white_button_opens_a_world_under_the_storm(self):
        # "Here we can run civvis sims with a very high frequency of natural
        # disasters": the white button opens the lane's own simulator with
        # the dial already turned.
        self.assertIn('<a class="cm-run-link" id="cm-run-link" href="../?chaos=100">Run a 100x world</a>', PAGE)
        # The page's frequencies are the ones the simulator's row offers.
        offered = json.loads(re.search(r"const CHAOS_FREQUENCIES = (\[[^\]]+\]);", SETUP)[1])
        self.assertEqual([dose["frequency"] for dose in RECORD["doses"]], offered)

    def test_a_page_with_no_record_says_so_once(self):
        self.assertIn('<p class="cm-empty" id="cm-empty">This copy of the page has no figures. They are written in when the site is published.</p>', PAGE)
        self.assertIn('<div class="cm-record" id="cm-record" hidden>', PAGE)
        self.assertIn("if (!data || !Array.isArray(data.doses) || data.doses.length < 2) return;", PAGE)
        # Names and figures reach the page as text, never as markup.
        script = PAGE[PAGE.index('<script type="application/json" id="chaos-data">'):]
        self.assertNotIn("innerHTML", script)

    def test_the_figures_are_there_without_a_pointer(self):
        # Every chart's figures are also a table, closed until asked for.
        self.assertIn("<summary>The figures as a table</summary>", PAGE)
        self.assertIn("<summary>How it is counted</summary>", PAGE)
        self.assertNotIn(" open>", PAGE)


class TheSiteList(unittest.TestCase):
    def test_chaos_monkey_stands_between_experiments_and_civvis_on_every_page(self):
        # Operator 2026-11-25: "above civvis add "Chaos Monkey"".
        self.assertGreaterEqual(len(LISTED), 9, sorted(LISTED))
        for name, page in LISTED.items():
            names = [site for _, site in site_list(page)]
            at = names.index("Chaos Monkey")
            self.assertEqual(names[at - 1:at + 2], ["Experiments", "Chaos Monkey", "CIVVIS"], name)
            address = dict((site, href) for href, site in site_list(page))
            # It is beside Experiments, wherever the page stands.
            if name == "chaos.html":
                self.assertEqual(address["Chaos Monkey"], "./")
            elif name == "tournaments.html":
                self.assertEqual(address["Chaos Monkey"], "../chaos-monkey/")
            else:
                self.assertEqual(address["Chaos Monkey"], address["Experiments"].replace("experiments/", "chaos-monkey/"), name)

    def test_this_page_is_the_one_ringed(self):
        nav = PAGE[PAGE.index('<nav class="site-nav"'):PAGE.index("</nav>")]
        self.assertEqual(nav.count('aria-current="page"'), 1)
        self.assertIn('<a class="site-link" href="./" aria-current="page">Chaos Monkey</a>', nav)
        # Its list is the Experiments page's, one level down like it.
        mine = [(href, name) for href, name in site_list(PAGE) if name not in ("Experiments", "Chaos Monkey")]
        theirs = [(href, name) for href, name in site_list(EXPERIMENTS) if name not in ("Experiments", "Chaos Monkey")]
        self.assertEqual(mine, theirs)

    def test_the_header_is_the_one_every_page_carries(self):
        header = lambda page: re.sub(r"<!--.*?-->", "", page[page.index('<nav class="utility-nav"'):page.index("</header>")], flags=re.S)
        self.assertEqual(header(PAGE), header(EXPERIMENTS))

    def test_the_simulator_lists_it_too(self):
        self.assertIn(
            '        <a class="home-link" id="tourlink" href="/experiments" target="_top">Experiments</a>\n'
            '        <a class="home-link" id="chaoslink" href="/chaos-monkey" target="_top">Chaos Monkey</a>\n'
            '        <a class="home-link" id="watchlink" href="./" target="_top">CIVVIS</a>\n',
            VIEWER,
        )
        # It opens a page, so it wears the pages' blue.
        self.assertIn("  #homelink, #chartslink, #tourlink, #chaoslink { color: #a8d8f5; border-color: #3b6788; background: #274b66; }\n", VIEWER)

    def test_publishing_builds_the_page(self):
        for line in (
            'chaos_page="$source_tree/beta/chaos.html"',
            'if [ -f "$chaos_page" ]; then',
            'chaos_at="$(address "$chaos_page" chaos-monkey)"',
            'mkdir -p "$out/$chaos_at"',
            'cp "$chaos_page" "$out/$chaos_at/index.html"',
            'fill chaos_page.py "$out/$chaos_at/index.html"',
            'pages+=("$chaos_at")',
            "without+=(chaos-monkey)",
        ):
            self.assertIn(line, PUBLISH)
        # And the publish check knows the page by the slot that is filled.
        self.assertEqual(verify.PAGES["chaos-monkey"], b'id="chaos-data"')


class TheDialInTheSimulator(unittest.TestCase):
    def test_the_turn_box_has_the_row(self):
        self.assertIn("function hudChaosRow() {", SETUP)
        self.assertIn("<span>Chaos Monkey:</span>", SETUP)
        self.assertIn("<select data-hud-chaos ", SETUP)
        # app.js only places the row; the row, its label and its listener
        # are app_setup.js's, because app.js is held under 40,000 lines.
        self.assertIn("  const fogHonestRow = hudFogHonestRow(), chaosRow = hudChaosRow(); // app_setup.js\n", APP)
        self.assertIn("    fogHonestRow + chaosRow +\n", APP)
        self.assertIn('  if (chaos) { chooseChaos(Number(chaos.value)); chaos.blur(); }\n', SETUP)
        self.assertNotIn("chooseChaos", APP)

    def test_the_row_asks_the_engine_and_the_published_engine_answers(self):
        self.assertIn('const answer = await fetchJSON("/chaos", {method: "POST",', SETUP)
        # A path the shim does not know goes to the network, where nothing
        # answers: the engine's routes are listed.
        routes = SHIM[SHIM.index("const ENGINE_ROUTES = new Set(["):]
        routes = routes[:routes.index("]);")]
        self.assertIn('"/chaos"', routes)

    def test_an_address_can_ask_for_the_dial(self):
        self.assertIn('asked = Number(new URLSearchParams(location.search).get("chaos"));', SETUP)
        self.assertIn("  askAddressChaos(); // `?chaos=N`, once, of the first watched world drawn\n", SETUP)


if __name__ == "__main__":
    unittest.main()
