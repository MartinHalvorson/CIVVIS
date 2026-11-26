#!/usr/bin/env python3
"""An address follows the page's name in the site list.

Operator, 2026-11-23: "clean up the URL please. standardized. please remove
research from the url". Until then four pages answered at addresses that were
not their names. Win Analytics was at `/research`, what the page was called
the day it was written, three names ago. CIVVIS Performance and AI
Interpretability stood beside it at the top of the site, although the list
keeps them under Charts/. Auto-Tourneys was at `/tournaments`. On 2026-11-24
Auto-Tourneys took its first name back, Tournaments (operator: "change
auto-outnrye to Tournaments"), and with it `/tournaments`; `/auto-tourneys`
forwards there. The same day AI Interpretability was named Civ AI
Interpretability (operator: "please change AI interpretability to "Civ AI
Interpretability""), and `/charts/ai-interpretability` forwards to where it
is now. On 2026-11-25 Tournaments was named Experiments (operator: "rename
tournaments to experiments"), at `/experiments`, and on 2026-11-26 it took
its name and `/tournaments` back (operator: "change experiments back to
tournaments"); `/experiments` forwards there.

The rule, for every page the site list names: the address is the name as the
list shows it, in lower case with a hyphen for anything that is not a letter
or a digit, and a page listed under a folder (Charts/) sits in that folder.

    Home                       /home/
    Charts/
      AI Insights              /charts/ai-insights/
      Civ AI Interpretability  /charts/civ-ai-interpretability/
      Civ Analytics            /charts/civ-analytics/
      Win Analytics            /charts/win-analytics/
      ELO Rankings             /charts/elo-rankings/
      CIVVIS Performance       /charts/civvis-performance/
    Tournaments                /tournaments/

A page states its own address (its `<link rel="canonical">`), `publish.sh`
puts it there, and every link in it is relative to it. So this reads the
pages as they are written and holds each to the rule, and holds every link to
landing on a page: with pages at two depths, a link written for the wrong one
stays inside the lane and reaches nothing. It also holds the two tables of
the addresses pages have left (the router's, which forwards them, and the
publish check's) to each other and to the pages.

    python3 beta/addresses_test.py
"""

from __future__ import annotations

import pathlib
import posixpath
import re
import sys
import unittest

BETA = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(BETA))

import verify  # noqa: E402

PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")
WORKER = (BETA / "_worker.js").read_text(encoding="utf-8")
VIEWER = (BETA.parent / "web" / "index.html").read_text(encoding="utf-8")

#: The pages `publish.sh` publishes, found in the script rather than listed
#: here: a page added to a lane is held to the rule the day it is added.
SOURCES = sorted(set(re.findall(r'_page="\$source_tree/beta/([a-z_]+\.html)"', PUBLISH)))

STATED = re.compile(r'<link rel="canonical" href="https://civvis\.ai/([^"]*)/">')
LINK = re.compile(r'<a class="(site-link(?: sub-link)?)" href="([^"]*)"([^>]*)>([^<]+)</a>')

#: Names a lane already answers to, which no page may take.
TAKEN = {"test", "assets", "report", "beta", "build", "index"}


def slug(name: str) -> str:
    """A name of the site list as an address: `CIVVIS Performance` -> `civvis-performance`."""
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")


def read(source: str) -> str:
    return (BETA / source).read_text(encoding="utf-8")


def stated(source: str) -> list:
    return STATED.findall(read(source))


def site_list(text: str) -> list:
    """The site list of a page: (classes, href, is this page, name) per link."""
    if '<nav class="site-nav"' not in text:
        return []
    nav = text[text.index('<nav class="site-nav"'):]
    nav = nav[:nav.index("</nav>")]
    return [
        (classes, href, 'aria-current="page"' in rest, name)
        for classes, href, rest, name in LINK.findall(nav)
    ]


def lands(address: str, link: str) -> str:
    """Where a relative link of the page at `address` comes out, in the lane."""
    target = link.split("#", 1)[0].split("?", 1)[0]
    return posixpath.normpath(posixpath.join(address, target))


#: Every page by the address it states.
ADDRESS = {source: (stated(source) or [""])[0] for source in SOURCES}


class Addresses(unittest.TestCase):
    def test_the_script_names_its_pages(self):
        # The glob this whole file stands on. Six pages on 2026-11-23.
        self.assertGreaterEqual(len(SOURCES), 6, SOURCES)
        for source in SOURCES:
            self.assertTrue((BETA / source).is_file(), source)

    def test_every_page_states_one_address_and_no_two_the_same(self):
        for source in SOURCES:
            self.assertEqual(len(stated(source)), 1, f"{source} states {stated(source)}")
            address = ADDRESS[source]
            # What `publish.sh` and the router read as a page: one name, or a
            # folder and a name.
            self.assertRegex(address, r"^[a-z][a-z0-9-]*(/[a-z][a-z0-9-]*)?$", source)
            self.assertNotIn(address.split("/")[0], TAKEN, source)
        self.assertEqual(len(set(ADDRESS.values())), len(SOURCES), ADDRESS)
        # No page lives at a folder's own address: the folder opens its
        # first page.
        folders = {address.split("/")[0] for address in ADDRESS.values() if "/" in address}
        self.assertEqual(folders & set(ADDRESS.values()), set())

    def test_an_address_is_the_pages_name_in_the_site_list(self):
        named = 0
        for source in SOURCES:
            links = site_list(read(source))
            if not links:
                # A page the list does not name (the downloads page) keeps
                # the address it has.
                continue
            here = [index for index, link in enumerate(links) if link[2]]
            self.assertEqual(len(here), 1, f"{source} marks {len(here)} links as itself")
            classes, _, _, name = links[here[0]]
            if "sub-link" in classes:
                # Under the folder that stands over it in the list.
                over = [link for link in links[:here[0]] if "sub-link" not in link[0]][-1]
                self.assertTrue(over[3].endswith("/"), f"{source} stands under {over[3]!r}")
                expected = f"{slug(over[3])}/{slug(name)}"
            else:
                expected = slug(name)
            self.assertEqual(ADDRESS[source], expected, f"{source} is {name!r} in the site list")
            named += 1
        self.assertGreaterEqual(named, 5)

    def test_no_address_says_research(self):
        # The word left the site list on 2026-11-21, when the page became
        # Musings. The file names and the data slot keep it; an address a
        # reader sees does not.
        for source, address in ADDRESS.items():
            self.assertNotIn("research", address, source)
        self.assertNotIn('href="/research', VIEWER)

    def test_every_link_of_the_site_list_opens_the_page_it_names(self):
        # A name of the list to the address it opens: a page its own, a
        # folder its first page.
        by_name = {}
        for source in SOURCES:
            for classes, _, current, name in site_list(read(source)):
                if current:
                    by_name[name] = ADDRESS[source]
        firsts = {}
        for address in ADDRESS.values():
            if "/" in address:
                firsts.setdefault(address.split("/")[0], []).append(address)
        checked = 0
        for source in SOURCES:
            links = site_list(read(source))
            for index, (classes, href, current, name) in enumerate(links):
                if re.match(r"[a-z][a-z0-9+.-]*:", href):
                    continue  # Civ 6, on Steam
                where = lands(ADDRESS[source], href)
                if name in by_name:
                    self.assertEqual(where, by_name[name], f"{source}: {name} -> {href}")
                elif name.endswith("/"):
                    # The folder opens the first page listed under it.
                    under = [link[3] for link in links[index + 1:] if "sub-link" in link[0]]
                    opens = by_name[under[0]] if under else sorted(firsts[slug(name)])[0]
                    if under:
                        self.assertEqual(where, opens, f"{source}: {name} -> {href}")
                    else:
                        self.assertIn(where, firsts[slug(name)], f"{source}: {name} -> {href}")
                else:
                    # CIVVIS and AI Debugger: the viewer, at the lane's root.
                    self.assertEqual(where, ".", f"{source}: {name} -> {href}")
                checked += 1
        self.assertGreaterEqual(checked, 25)

    def test_the_folder_opens_the_same_page_from_everywhere(self):
        opens = set()
        for source in SOURCES:
            for classes, href, _, name in site_list(read(source)):
                if name.endswith("/"):
                    opens.add(lands(ADDRESS[source], href))
        # AI Insights, the folder's first page since 2026-11-24 (operator:
        # "we want a new ai insights").
        self.assertEqual(opens, {"charts/ai-insights"})
        self.assertIn('href="/charts/ai-insights" target="_top">Charts/</a>', VIEWER)

    def test_every_relative_link_of_a_page_lands_on_a_page_or_the_viewer(self):
        # The same reading `publish.sh` gives the published pages, given here
        # to the pages as written, so a pull request's own checks see it.
        plain = re.compile(r"[A-Za-z0-9._~/?#=&%+-]+")
        addresses = set(ADDRESS.values())
        checked = 0
        for source in SOURCES:
            text = read(source)
            self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', text), [], source)
            for link in sorted(set(re.findall(r'(?:href|src)="([^"]*)"', text))):
                if not plain.fullmatch(link):
                    continue
                where = lands(ADDRESS[source], link)
                if where.startswith(f"{ADDRESS['landing.html']}/assets/"):
                    self.assertTrue(
                        (BETA / "assets" / where.split("/assets/", 1)[1]).is_file(), f"{source}: {link}")
                else:
                    self.assertIn(
                        where, addresses | {"."},
                        f"{source}, at {ADDRESS[source]}/, links {link}, which is no page of the site")
                checked += 1
        self.assertGreaterEqual(checked, 30)

    def test_the_viewer_links_pages_where_they_are(self):
        # Root-absolute as written (the desktop build serves the viewer at
        # the root); `publish.sh` makes each lane-relative, and knows a link
        # only as the address of a page the lane carries.
        linked = re.findall(r'href="/([a-z][^"?#]*)"', VIEWER)
        self.assertGreaterEqual(len(linked), 3, linked)
        for address in linked:
            self.assertIn(address, set(ADDRESS.values()), f'the viewer links "/{address}"')

    def test_wizard_chess_is_set_aside_and_is_no_page_of_the_site(self):
        # Operator, 2026-11-24: "lets set aside wizard chess right now. please
        # remove from civvis.ai". It was the last name in the list, at
        # /wizard-chess/, from 2026-11-23. The game stays in the tree
        # (`beta/chess.html`, `beta/chess/`); `publish.sh` publishes nothing
        # of it, no published page or the viewer names or links it, and the
        # publish check knows no such page.
        self.assertNotIn("chess.html", SOURCES)
        # The one line of `publish.sh` that names it copies nothing: it is
        # where a revision from the days the page was on the site pointed
        # its viewer's link, which is made lane-relative like every other
        # (left root-absolute it stopped every publish, the stable lane
        # being pinned to such a revision the hour the page was set aside).
        self.assertEqual(
            [line for line in PUBLISH.splitlines() if "chess" in line.lower() and not line.lstrip().startswith("#")],
            ['set_aside = ["wizard-chess", "chaos-monkey"]'])
        self.assertNotIn("wizard-chess", set(ADDRESS.values()))
        self.assertNotIn("wizard-chess", verify.ADDRESSES)
        for source in SOURCES:
            self.assertNotRegex(read(source), r"(?i)wizard.chess", source)
        self.assertNotRegex(VIEWER, r"(?i)wizard.chess|chesslink")

    def test_chaos_monkey_is_off_the_site(self):
        # Operator, 2026-11-26: "remove chaos monkey for now". It stood
        # between Experiments and CIVVIS, at /chaos-monkey/, on 2026-11-25.
        # Its page is out of the tree, `publish.sh` publishes nothing of it,
        # no page or the viewer names or links it, and the publish check
        # knows no such page. The one line of `publish.sh` that names it is
        # where an older revision's viewer pointed its link (see above).
        self.assertFalse((BETA / "chaos.html").exists())
        self.assertNotIn("chaos.html", SOURCES)
        self.assertEqual(
            [line for line in PUBLISH.splitlines() if "chaos" in line.lower() and not line.lstrip().startswith("#")],
            ['set_aside = ["wizard-chess", "chaos-monkey"]'])
        self.assertNotIn("chaos-monkey", set(ADDRESS.values()))
        self.assertNotIn("chaos-monkey", verify.ADDRESSES)
        for source in SOURCES:
            self.assertNotRegex(read(source), r"(?i)chaos.monkey", source)
        self.assertNotRegex(VIEWER, r"(?i)chaos.monkey|chaoslink")


class Moved(unittest.TestCase):
    """The addresses pages have left forward to where the pages are."""

    def router_table(self) -> dict:
        block = WORKER[WORKER.index("const MOVED = {"):]
        block = block[:block.index("};")]
        return dict(re.findall(r'^\s*"?([a-z][a-z0-9/-]*)"?: "([^"]+)",$', block, re.M))

    def test_the_router_and_the_publish_check_keep_one_table(self):
        moved = self.router_table()
        self.assertEqual(moved, verify.FORMER)
        self.assertGreaterEqual(len(moved), 5)
        # And one table of the pages back at an address they once left.
        block = WORKER[WORKER.index("const BETWEEN = {"):]
        block = block[:block.index("};")]
        between = {back: re.findall(r'"([^"]+)"', away)
                   for back, away in re.findall(r'^\s*"?([a-z][a-z0-9/-]*)"?: \[([^\]]*)\],$', block, re.M)}
        self.assertEqual(between, verify.BETWEEN)
        self.assertEqual(between, {"tournaments": ["auto-tourneys", "experiments"]})

    def test_a_page_back_at_an_address_it_left_forwards_from_the_one_in_between(self):
        # Tournaments was at /auto-tourneys/ from 2026-11-23 to 2026-11-24,
        # and at /experiments/ from 2026-11-25 to 2026-11-26. A lane pinned
        # to a revision of those days still has it there, and /tournaments/
        # forwards to it, as it did then; in a lane from after them both
        # forward to /tournaments/.
        for back, between in verify.BETWEEN.items():
            self.assertIn(back, verify.ADDRESSES)
            for away in between:
                self.assertEqual(verify.FORMER[away], back)
        self.assertIn("Object.hasOwn(BETWEEN, now)", WORKER)

    def test_a_page_renamed_again_is_found_from_every_address_it_had(self):
        # Operator, 2026-11-25: "rename tournaments to experiments", and
        # 2026-11-26: "change experiments back to tournaments". The page has
        # been at /tournaments/, /auto-tourneys/, /tournaments/ again,
        # /experiments/ and /tournaments/ once more, and a lane has it
        # wherever its revision did.
        self.assertEqual(ADDRESS["tournaments.html"], "tournaments")
        self.assertEqual(verify.FORMER["experiments"], "tournaments")
        self.assertEqual(verify.FORMER["auto-tourneys"], "tournaments")
        self.assertNotIn("tournaments", verify.FORMER)
        self.assertEqual(verify.left_by("tournaments"), ["experiments", "auto-tourneys"])
        back = ["home", "tournaments"]           # a lane with the name back, or one from before either
        named = ["home", "experiments"]          # one pinned while it was Experiments: the front page, until a promotion
        between = ["home", "auto-tourneys"]      # one pinned to the days it was Auto-Tourneys
        asked = {address: [verify.forwards_to(address, lane) if address not in lane else address
                           for lane in (back, named, between)]
                 for address in ("tournaments", "experiments", "auto-tourneys")}
        self.assertEqual(asked, {
            "tournaments": ["tournaments", "experiments", "auto-tourneys"],
            "experiments": ["tournaments", "experiments", "auto-tourneys"],
            "auto-tourneys": ["tournaments", "experiments", "auto-tourneys"],
        })
        # The router asks the same way: an address that has moved on is
        # followed first, and where the lane has the page at none of those,
        # an address the page had in between two stays where they end is.
        walk = WORKER[WORKER.index("  if (Object.hasOwn(MOVED, name)) {"):]
        walk = walk[:walk.index("  if (now !== name) return { absent: true };")]
        self.assertIn("  if (Object.hasOwn(BETWEEN, now)) {\n"
                      "    const away = BETWEEN[now].find((page) => pages.includes(page));\n", walk)
        self.assertLess(walk.index("now = MOVED[now];"), walk.index("Object.hasOwn(BETWEEN, now)"))
        # A bundle from before lanes listed their pages had it at its first
        # address, which is where it is again.
        self.assertEqual(verify.first_address("tournaments"), "tournaments")
        self.assertIn("tournaments", verify.lane_pages({}))
        self.assertNotIn("auto-tourneys", verify.lane_pages({}))
        self.assertNotIn("experiments", verify.lane_pages({}))

    def test_a_page_that_moved_twice_is_followed_move_by_move(self):
        # Each line of the table is one move, and a page can have made more
        # than one: Civ AI Interpretability was at /interpretability, then at
        # /charts/ai-interpretability, and is at /charts/civ-ai-interpretability
        # since it took that name on 2026-11-24. A lane pinned to a revision
        # from before the name still has it at the second, and there
        # /interpretability forwards to the second, as it did then.
        self.assertEqual(
            verify.moves_from("interpretability"),
            ["charts/ai-interpretability", "charts/civ-ai-interpretability"])
        self.assertEqual(verify.page_at("interpretability"), "charts/civ-ai-interpretability")
        self.assertEqual(verify.page_at("charts/ai-interpretability"), "charts/civ-ai-interpretability")
        named = ["home", "charts/civ-ai-interpretability"]
        before_the_name = ["home", "charts/ai-interpretability"]
        self.assertEqual(verify.forwards_to("interpretability", named), "charts/civ-ai-interpretability")
        self.assertEqual(verify.forwards_to("charts/ai-interpretability", named), "charts/civ-ai-interpretability")
        self.assertEqual(verify.forwards_to("interpretability", before_the_name), "charts/ai-interpretability")
        self.assertIsNone(verify.forwards_to("interpretability", ["home"]))
        # The address a page left last is listed before the one it left
        # first, and a bundle from before lanes listed their pages had each
        # page at its first.
        self.assertEqual(
            verify.left_by("charts/civ-ai-interpretability"),
            ["charts/ai-interpretability", "interpretability"])
        self.assertIn("interpretability", verify.lane_pages({}))
        self.assertNotIn("charts/ai-interpretability", verify.lane_pages({}))
        # No address leads back to itself, so following the moves ends, well
        # inside the count the router allows itself.
        allowed = int(re.search(r"Object\.hasOwn\(MOVED, now\) && moves < (\d+);", WORKER).group(1))
        for old in verify.FORMER:
            went = verify.moves_from(old)
            self.assertEqual(len(went), len(set(went)), old)
            self.assertNotIn(old, went)
            self.assertLess(len(went), allowed)
        # The router walks the same way: to the first address the lane lists.
        self.assertIn("now = MOVED[now];\n      if (pages.includes(now)) return { moved: `${lane}/${now}/` };", WORKER)

    def test_every_left_address_forwards_to_a_page_that_is_there(self):
        addresses = set(ADDRESS.values())
        for old, new in self.router_table().items():
            there = verify.page_at(old)
            self.assertIn(there, addresses, f"/{old} forwards in the end to /{there}, which no page states")
            # A move lands on a page, or on an address the page has left since.
            self.assertTrue(new in addresses or new in verify.FORMER, f"/{old} forwards to /{new}")
            # An address in use is not one a page has left.
            self.assertNotIn(old, addresses, old)
            self.assertNotIn(old, {address.split("/")[0] for address in addresses}, old)

    def test_the_publish_check_knows_every_page_by_its_address(self):
        self.assertEqual(set(verify.PAGES), set(ADDRESS.values()))
        # Each by the bytes that prove the page answering is that page.
        for source, address in ADDRESS.items():
            self.assertIn(verify.PAGES[address], read(source).encode("utf-8"), source)

    def test_publish_puts_every_page_where_it_says(self):
        for source in SOURCES:
            name = source[:-len(".html")]
            # The second word is where the page was before pages stated
            # their addresses; a page written since has only the one it
            # states, folder and all.
            self.assertRegex(
                PUBLISH, rf'\n[a-z]+_at="\$\(address "\${name}_page" [a-z]+(/[a-z-]+)?\)"\n', source)


if __name__ == "__main__":
    unittest.main()
