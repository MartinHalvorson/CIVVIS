#!/usr/bin/env python3
"""Civ Analytics: the civ records, then the tree of early build orders, both
as Win Analytics showed them until 2026-11-23.

    python3 beta/civs_page_test.py
"""

from __future__ import annotations

import contextlib
import io
import json
import pathlib
import re
import tempfile
import unittest
from unittest import mock

import civs_page

BETA = pathlib.Path(__file__).resolve().parent
PAGE = (BETA / "civs.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")


class CivAnalytics(unittest.TestCase):
    def test_the_civ_records_come_first_then_the_build_orders(self):
        # Operator 2026-11-23: "add another page: Civ Analytics. First can be
        # the civ records (pulled from win analytics), then early build order".
        record = PAGE[PAGE.index('<div class="civ-record" id="civ-record" hidden>'):]
        self.assertEqual(re.findall(r'<section [^>]*id="([a-z-]+)"', record), ["rank-civs", "res-side-builds"])
        self.assertIn('<h2 class="rank-title" id="rank-civs-title">Winningest civs</h2>', PAGE)
        self.assertIn('<h2 class="res-side-title" id="res-title-builds">Early build order: win rate against Lv 8 Deity AI</h2>', PAGE)
        # Side by side on a wide window, the records at the left; one under
        # the other, records first, on a narrow one.
        self.assertIn(".civ-record { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1.25fr);", PAGE)
        self.assertIn("@media (max-width: 859px) { .civ-record { grid-template-columns: minmax(0, 1fr); } }", PAGE)

    def test_the_record_is_the_civ_table_and_win_analytics_tree(self):
        tree = {"level": 8, "games": 2, "tree": [[0, 2, 1]], "items": ["Scout"]}
        civs = {"rows": [{"name": "Greece", "wins": 1, "losses": 1}], "games": 2}
        with mock.patch.object(civs_page.research_page, "research", return_value={"build_orders": tree, "levels": {}}), \
                mock.patch.object(civs_page.ranks, "civs", return_value=civs):
            self.assertEqual(civs_page.record(BETA.parent), {"civs": civs, "build_orders": tree})
        # What cannot be read costs the page that figure, never the publish.
        with mock.patch.object(civs_page.research_page, "research", side_effect=ValueError("no ledger")), \
                mock.patch.object(civs_page.ranks, "civs", return_value=None), \
                contextlib.redirect_stderr(io.StringIO()) as said:
            self.assertIsNone(civs_page.record(BETA.parent))
        self.assertIn("civs_page: no tree of build orders (no ledger)", said.getvalue())

    def test_the_page_carries_one_empty_slot_and_publishing_fills_it(self):
        self.assertEqual(PAGE.count('<script type="application/json" id="civ-data">null</script>'), 1)
        data = {"civs": None, "build_orders": {"tree": [], "note": "</script><b>"}}
        filled = civs_page.fill(PAGE, data)
        self.assertEqual(json.loads(civs_page.SLOT.search(filled)[2]), data)
        self.assertEqual(filled.count("</script>"), PAGE.count("</script>"))
        with tempfile.TemporaryDirectory() as empty:
            page = pathlib.Path(empty) / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(civs_page.main(["civs_page.py", empty, str(page)]), 0)
            self.assertIsNone(json.loads(civs_page.SLOT.search(page.read_text(encoding="utf-8"))[2]))
        for line in (
            'civs_page="$source_tree/beta/civs.html"',
            'civs_at="$(address "$civs_page" civs)"',
            'cp "$civs_page" "$out/$civs_at/index.html"',
            'fill civs_page.py "$out/$civs_at/index.html"',
            'pages+=("$civs_at")',
        ):
            self.assertIn(line, PUBLISH)
        self.assertIn('<link rel="canonical" href="https://civvis.ai/charts/civ-analytics/">', PAGE)
        self.assertIn("""    "charts/civ-analytics": b'id="civ-data"',\n""", (BETA / "verify.py").read_text(encoding="utf-8"))
        self.assertNotIn("innerHTML", PAGE)
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])


if __name__ == "__main__":
    unittest.main()
