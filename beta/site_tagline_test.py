#!/usr/bin/env python3
"""Every page says what the simulator is for in the same words.

Operator, 2026-11-26: "change 'for AI reasoning insights' to 'for
interpretability & forecasting insights'". It was "for AI strategy
development" until 2026-11-23 (#5741), and "for AI reasoning insights" from
then until this change.

The phrase is written by hand into each page's header, so a new page copied
from an old one brings along whatever wording the old one had. This reads
every page `publish.sh` publishes (found in the script, not listed here) and
the simulator's own lockup. A tagline must carry today's phrase, and no page
in `beta/` or `web/` may still carry an earlier one.

    python3 beta/site_tagline_test.py
"""

from __future__ import annotations

import pathlib
import re
import unittest

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")
VIEWER = (ROOT / "web" / "index.html").read_text(encoding="utf-8")

#: What the simulator is for, as HTML writes it (`&` is `&amp;`).
PURPOSE = "for interpretability &amp; forecasting insights"
#: The wordings it replaced, which no page is to carry again.
RETIRED = ("for AI reasoning insights", "for AI strategy development")

SOURCES = sorted(set(re.findall(r'_page="\$source_tree/beta/([a-z_]+\.html)"', PUBLISH)))
PAGE_TAGLINE = re.compile(r'<p class="site-tagline">(.*?)</p>', re.S)
VIEWER_TAGLINE = re.compile(r'<div class="tagline">(.*?)</div>', re.S)


class SiteTaglineTest(unittest.TestCase):
    def test_the_published_pages_are_found(self) -> None:
        self.assertGreater(len(SOURCES), 5, "publish.sh's page list was not found")

    def test_every_page_tagline_says_the_purpose(self) -> None:
        seen = 0
        for name in SOURCES:
            text = (BETA / name).read_text(encoding="utf-8")
            for tagline in PAGE_TAGLINE.findall(text):
                seen += 1
                with self.subTest(page=name):
                    self.assertIn(f"<span>{PURPOSE}</span>", tagline)
        self.assertGreater(seen, 5, "no page carries a site-tagline")

    def test_the_landing_description_says_the_purpose(self) -> None:
        landing = (BETA / "landing.html").read_text(encoding="utf-8")
        self.assertIn(f'content="An open source Civilization VI Simulator {PURPOSE}."', landing)

    def test_the_simulator_lockup_says_the_purpose(self) -> None:
        taglines = VIEWER_TAGLINE.findall(VIEWER)
        self.assertEqual(len(taglines), 1)
        self.assertIn(f"<span>{PURPOSE}</span>", taglines[0])

    def test_no_page_keeps_an_earlier_wording(self) -> None:
        pages = sorted(BETA.glob("*.html")) + [ROOT / "web" / "index.html"]
        for path in pages:
            text = path.read_text(encoding="utf-8")
            for old in RETIRED:
                with self.subTest(page=path.name, wording=old):
                    self.assertNotIn(old, text)


if __name__ == "__main__":
    unittest.main()
