#!/usr/bin/env python3
"""The site's pages are pictures, charts and tables first, and words second.

Operator, 2026-11-24: "go through civvis website and clean up junk text.
there is too much text on pages, particualrly home/ and charts/. should be a
more visual experience. some text ok. just not too much."

Until that day a page opened on a paragraph, had a paragraph under every
heading and another under every chart. Now a page opens on a line or two, a
heading has at most a line under it, and what a chart counts is under a
disclosure (`<details class="fine">`, "How it is counted") that is closed
until a visitor presses it.

This holds that, of every page that opens on an intro, as written in the
repository: the words a page's script writes in when it is filled are not
read here. The pages are found, not listed, so a new page of the site is held
to it the day it is added.
"""
from __future__ import annotations

import pathlib
import re
import unittest

HERE = pathlib.Path(__file__).resolve().parent

# The most words a page may open on, a heading may have under it, and any
# other paragraph may run to before it belongs in a disclosure.
INTRO = 30
SUB = 35
PARAGRAPH = 45

PAGES = {path.name: path.read_text() for path in sorted(HERE.glob("*.html")) if '<p class="intro">' in path.read_text()}


def body(page: str) -> str:
    """The page's markup as a visitor is given it: no comments, styles or
    scripts, and no dialog (it is shut until its button is pressed)."""
    page = page[page.index("<body"):]
    for block in (r"<!--.*?-->", r"<style\b.*?</style>", r"<script\b.*?</script>", r"<dialog\b.*?</dialog>"):
        page = re.sub(block, "", page, flags=re.S)
    return page


def words(markup: str) -> int:
    return len(re.sub(r"&#?\w+;", "x", re.sub(r"<[^>]+>", " ", markup)).split())


def paragraphs(markup: str) -> list[tuple[str, str]]:
    return re.findall(r"<p\b([^>]*)>(.*?)</p>", markup, flags=re.S)


class PageWords(unittest.TestCase):
    def test_the_pages_are_found(self):
        # Home, Tournaments and the five pages of Charts/ on the day this was
        # written; fewer means the glob or the intro's markup moved.
        self.assertGreaterEqual(len(PAGES), 7, sorted(PAGES))
        for name in ("landing.html", "research.html", "civs.html", "elo.html", "performance.html", "interpretability.html", "tournaments.html"):
            self.assertIn(name, PAGES)

    def test_a_page_opens_on_a_line_or_two(self):
        for name, page in PAGES.items():
            intros = [text for attrs, text in paragraphs(body(page)) if 'class="intro"' in attrs]
            self.assertEqual(len(intros), 1, name)
            self.assertLessEqual(words(intros[0]), INTRO, f"{name} opens on {words(intros[0])} words")
            # It names no other page: the list over it does.
            self.assertNotIn("<a ", intros[0], name)

    def test_a_heading_has_at_most_a_line_under_it(self):
        for name, page in PAGES.items():
            for attrs, text in paragraphs(body(page)):
                if re.search(r'class="[^"]*\b(rank-sub|int-kind-sub|civ-step-why|civ-why)\b', attrs):
                    self.assertLessEqual(words(text), SUB, f"{name}: {words(text)} words under a heading: {' '.join(text.split())[:80]}")

    def test_what_a_chart_counts_is_closed_until_it_is_pressed(self):
        for name, page in PAGES.items():
            markup = body(page)
            self.assertNotRegex(markup, r'<details class="fine"[^>]*\bopen\b', name)
            # Out in the open, no paragraph runs long.
            shown = re.sub(r'<details class="fine"[^>]*>.*?</details>', "", markup, flags=re.S)
            for attrs, text in paragraphs(shown):
                self.assertLessEqual(words(text), PARAGRAPH, f"{name}: {words(text)} words in the open: {' '.join(text.split())[:80]}")
            # A disclosure says what it holds and holds something.
            for inner in re.findall(r'<details class="fine"[^>]*>(.*?)</details>', markup, flags=re.S):
                self.assertRegex(inner, r"<summary>How it is (counted|worked out)</summary>", name)
                self.assertRegex(inner, r'<(p|div)\b', name)

    def test_every_page_with_a_disclosure_styles_it(self):
        for name, page in PAGES.items():
            if '<details class="fine"' in page:
                self.assertIn("  .fine > summary {", page, name)


if __name__ == "__main__":
    unittest.main()
