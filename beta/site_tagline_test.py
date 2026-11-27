#!/usr/bin/env python3
"""The site's tagline says what the project is for.

Operator, 2026-11-27: change "A civ VI simulator for ai reasoning insights"
to "A Civilization VI Simulator for fixing the Civ AI". The simulator's
lockup reads exactly that, two lines: "A Civilization VI Simulator" and "for
fixing the Civ AI". Every page with a header carries the same second line
under its own first line ("An open source Civilization VI Simulator"), and
the home page's description for search engines says the same.

No script writes the tagline: it is text in each page, so a page copied from
an old one brings the old words with it. This holds every page in `beta/`
and the simulator to the current words, and fails on any that still carries
a retired one.

    python3 beta/site_tagline_test.py
"""

from __future__ import annotations

import pathlib
import re
import unittest

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
SIMULATOR = ROOT / "web" / "index.html"

PURPOSE = "for fixing the Civ AI"
# Each wording the tagline has had, newest last: "for AI strategy
# development" until #5741 (2026-11-23), "for AI reasoning insights" until
# 2026-11-27, and "for interpretability & forecasting insights", asked for on
# 2026-11-26 (#5916) and overtaken before it was published.
RETIRED = (
    "for AI strategy development",
    "for AI reasoning insights",
    "for interpretability &amp; forecasting insights",
    "for interpretability & forecasting insights",
)


def pages() -> list[pathlib.Path]:
    return sorted(BETA.glob("*.html")) + [SIMULATOR]


class TheTaglineIsForFixingTheCivAI(unittest.TestCase):
    def test_the_simulator_reads_the_operators_words(self):
        page = SIMULATOR.read_text(encoding="utf-8")
        self.assertIn(
            '<div class="tagline"><span>A <i>Civilization VI</i> Simulator</span>'
            f"<span>{PURPOSE}</span></div>",
            page,
        )

    def test_every_page_header_carries_the_same_purpose(self):
        headed = 0
        for path in sorted(BETA.glob("*.html")):
            page = path.read_text(encoding="utf-8")
            for tagline in re.findall(r'<p class="site-tagline">(.*?)</p>', page):
                headed += 1
                self.assertEqual(
                    tagline,
                    f"<span>An open source Civilization VI Simulator</span> <span>{PURPOSE}</span>",
                    path.name,
                )
        # Home, Tournaments and the pages of Charts/ all have a header.
        self.assertGreaterEqual(headed, 9)

    def test_the_home_page_describes_itself_the_same_way(self):
        landing = (BETA / "landing.html").read_text(encoding="utf-8")
        self.assertIn(
            f'<meta name="description" content="An open source Civilization VI Simulator {PURPOSE}.">',
            landing,
        )

    def test_no_page_keeps_a_retired_wording(self):
        for path in pages():
            page = path.read_text(encoding="utf-8")
            for words in RETIRED:
                self.assertNotIn(words, page, f"{path.name} still says {words!r}")


if __name__ == "__main__":
    unittest.main()
