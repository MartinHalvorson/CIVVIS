#!/usr/bin/env python3
"""The Tournaments page: its record is counted off the ledger, and it fits its slot."""

from __future__ import annotations

import contextlib
import io
import json
import pathlib
import re
import tempfile
import unittest

import home_rankings
import tournaments_page

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "tournaments.html").read_text(encoding="utf-8")
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")
VIEWER = (ROOT / "web" / "index.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")

ON = {"players": 6, "map": "Continents.lua"}
MATCH = {"players": 6, "map": "Continents.lua"}


def run(tag: str, outcome: str, civ: str | None, minute: int, **more) -> dict:
    """One line of the results file, as `civ6_ladder.result_row` writes it."""
    return {
        "tag": tag,
        "game": re.sub(r"-cont\d+$", "", tag),
        "utc": f"2026-11-21T10:{minute:02d}:00Z",
        "civ": civ,
        "difficulty": "DIFFICULTY_DEITY",
        "outcome": outcome,
        "victory": None,
        "turns": 100 + minute,
        **ON,
        **more,
    }


class TournamentsPage(unittest.TestCase):
    def record(self) -> dict:
        """Four definitions: one played out, one being played, one waiting
        behind a tag prefix of its own, and one whose time came and went."""
        specs = [
            {"name": "First idea", "idea": "Does the opening matter?", "conditions": "Lv 8 6P Continents Online",
             "start": "civvis-20261121T100000Z", "end": "civvis-20261121T102000Z", "series_size": 2,
             "match": {**MATCH, "difficulty": "DIFFICULTY_DEITY"}, "note": "Kept."},
            {"name": "Second idea", "conditions": "Lv 8 6P Continents Online",
             "start": "civvis-20261121T102000Z", "series_size": 5,
             "match": {**MATCH, "difficulty": "DIFFICULTY_DEITY"}},
            {"name": "Never played", "conditions": "c", "start": "civvis-20261121T090000Z",
             "end": "civvis-20261121T093000Z", "series_size": 3, "match": {"players": 8}},
            {"name": "Level ladder", "idea": "Two agents at one table, level by level.",
             "conditions": "Lv 4-8 6P Continents Online, 2 CIVVIS + 4 AI",
             "start": "civvis-t6-", "end": "civvis-t7-", "series_size": 50, "match": MATCH},
        ]
        rows = [
            run("civvis-20261121T100000Z", "win", "CIVILIZATION_ROME", 5, victory="VICTORY_DIPLOMATIC"),
            run("civvis-20261121T101000Z", "rival_victory", "CIVILIZATION_GRAN_COLOMBIA", 15, victory="VICTORY_TECHNOLOGY"),
            run("civvis-20261121T102000Z", "defeat", "CIVILIZATION_MALI", 25),
            run("civvis-20261121T103000Z", "unfinished", None, 31),                      # never reached a board
            run("civvis-20261121T103500Z", "rival_victory", "CIVILIZATION_GAUL", 40),
            run("civvis-20261121T103500Z-cont1", "win", "CIVILIZATION_GAUL", 45, victory="VICTORY_RELIGIOUS"),
            run("civvis-20261121T105000Z", "abandoned", "CIVILIZATION_KOREA", 55),
        ]
        return tournaments_page.record(rows, specs, ["Rome", "Gran Colombia", "Mali", "Gaul", "Korea"])

    def test_the_record_is_what_runs_what_waits_and_what_has_finished(self):
        record = self.record()
        self.assertEqual([(t["id"], t["name"]) for t in record["finished"]], [(1, "First idea")])
        self.assertEqual([(t["id"], t["name"]) for t in record["running"]], [(2, "Second idea")])
        # A definition with no finished game waits, numbered on from the
        # last tournament begun. One whose `end` the games have passed never
        # ran and is not listed.
        self.assertEqual(record["scheduled"], [{
            "id": 3, "name": "Level ladder", "idea": "Two agents at one table, level by level.",
            "conditions": "Lv 4-8 6P Continents Online, 2 CIVVIS + 4 AI", "note": "", "size": 50,
        }])
        first, second = record["finished"][0], record["running"][0]
        self.assertEqual((first["games"], first["size"], first["wins"], first["losses"]), (2, 2, 1, 1))
        self.assertEqual((first["start"], first["end"]), ("2026-11-21 10:00Z", "2026-11-21 10:15Z"))
        self.assertEqual((first["idea"], first["note"]), ("Does the opening matter?", "Kept."))
        self.assertEqual((second["games"], second["size"], second["wins"], second["losses"]), (3, 5, 1, 2))
        self.assertIsNone(second["end"])
        self.assertEqual(second["idea"], "")
        # The totals count every game of every tournament begun.
        self.assertEqual(record["totals"], {"tournaments": 2, "games": 5, "wins": 2, "losses": 3})
        self.assertEqual(record["through"], "2026-11-21 10:55Z")

    def test_a_series_carries_its_games_in_the_order_played(self):
        first, second = self.record()["finished"][0], self.record()["running"][0]
        self.assertEqual(first["results"], [
            {"civ": "Rome", "won": True, "how": "Diplomatic victory", "turns": 105, "level": 8},
            {"civ": "Gran Colombia", "won": False, "how": "Rival Science victory", "turns": 115, "level": 8},
        ])
        # A game reloaded from its autosave is one game, by its best run.
        self.assertEqual([(g["civ"], g["how"]) for g in second["results"]],
                         [("Mali", "Eliminated"), ("Gaul", "Religious victory"), ("Korea", "Conceded")])
        # Wins by victory and losses by how they were lost, most first.
        self.assertEqual(first["victories"], [["Diplomatic", 1]])
        self.assertEqual(first["endings"], [["Rival Science victory", 1]])
        self.assertEqual(second["victories"], [["Religious", 1]])
        self.assertEqual(second["endings"], [["Conceded", 1], ["Eliminated", 1]])
        # One level throughout: no record per level.
        self.assertEqual(first["levels"], [])

    def test_a_table_of_two_agents_and_mixed_levels_reads_plainly(self):
        # Tournament 6 (operator 2026-11-21): hotseat tables with two CIVVIS
        # agents, ten games at each level from 4 to 8. The run records both
        # civs joined by `+`, and each game its own difficulty.
        spec = {"name": "Level ladder", "conditions": "c", "start": "civvis-t6-", "end": "civvis-t7-",
                "series_size": 50, "match": MATCH}
        rows = [
            run("civvis-t6-20261121T100000Z", "win", "CIVILIZATION_PHOENICIA+CIVILIZATION_AUSTRALIA", 5,
                difficulty="DIFFICULTY_PRINCE", victory="VICTORY_TECHNOLOGY"),
            run("civvis-t6-20261121T101000Z", "rival_victory", "CIVILIZATION_ROME+CIVILIZATION_MALI", 15,
                difficulty="DIFFICULTY_PRINCE", victory="VICTORY_CULTURE"),
            run("civvis-t6-20261121T102000Z", "win", "CIVILIZATION_ROME+CIVILIZATION_GAUL", 25,
                difficulty="DIFFICULTY_KING", victory="VICTORY_TECHNOLOGY"),
            run("civvis-20261121T103000Z", "win", "CIVILIZATION_ROME", 35),   # not this table's tag
        ]
        record = tournaments_page.record(rows, [spec], ["Phoenicia", "Australia", "Rome", "Mali", "Gaul"])
        (ladder,) = record["running"]
        self.assertEqual(ladder["games"], 3)
        self.assertEqual(ladder["results"][0]["civ"], "Phoenicia & Australia")
        self.assertEqual([g["level"] for g in ladder["results"]], [4, 4, 5])
        self.assertEqual(ladder["levels"], [[4, 1, 1], [5, 1, 0]])
        self.assertEqual(ladder["victories"], [["Science", 2]])
        self.assertEqual(record["scheduled"], [])

    def test_the_counting_is_the_home_tables_own(self):
        # The page, the live mirror's Mode line and an older home page's
        # table count one set of series: the detail the page asks for adds
        # keys to `tournament_series` rows and changes none of them.
        specs = json.loads((ROOT / "data" / "tournaments.json").read_text(encoding="utf-8"))["tournaments"]
        rows = [run("civvis-20261119T190000Z", "win", "CIVILIZATION_ROME", 5),
                run("civvis-20261120T050000Z", "defeat", "CIVILIZATION_MALI", 15)]
        plain = home_rankings.tournament_series(rows, specs)
        detailed = home_rankings.tournament_series(rows, specs, detail=True)
        self.assertEqual(
            [{key: value for key, value in row.items() if key not in ("definition", "played")} for row in detailed],
            plain,
        )
        self.assertTrue(all("played" not in row and "definition" not in row for row in plain))
        record = tournaments_page.record(rows, specs, [])
        self.assertEqual(
            sorted(t["id"] for t in record["running"] + record["finished"]), sorted(row["id"] for row in plain))
        # Every committed definition's idea, where it has one, is a sentence.
        for spec in specs:
            if "idea" in spec:
                self.assertIsInstance(spec["idea"], str)
                self.assertTrue(spec["idea"].strip())

    def test_the_page_carries_one_empty_slot_and_publishing_fills_it(self):
        self.assertEqual(PAGE.count('<script type="application/json" id="tournaments-data">null</script>'), 1)
        record = self.record()
        record["running"][0]["note"] = "</script><b>x"   # a sentence cannot close the slot
        filled = tournaments_page.fill(PAGE, record)
        self.assertEqual(filled.count("</script>"), PAGE.count("</script>"))
        self.assertEqual(json.loads(tournaments_page.SLOT.search(filled)[2]), record)
        # Nothing but the slot moved.
        self.assertEqual(tournaments_page.SLOT.sub("", filled), tournaments_page.SLOT.sub("", PAGE))
        # The publish script is what runs it: the lane's own page (head's for
        # a revision that predates it), filled from head's definitions.
        for line in (
            'tournaments_page="$source_tree/beta/tournaments.html"',
            '[ -f "$tournaments_page" ] || tournaments_page="$repo_root/beta/tournaments.html"',
            'cp "$tournaments_page" "$out/tournaments/index.html"',
            'tournaments_page.py" "$repo_root" "$out/tournaments/index.html"',
            # And its links are held inside the lane like every page's.
            'out / "tournaments" / "index.html": [],',
        ):
            self.assertIn(line, PUBLISH)
        # The page is authored lane-relative: nothing in it links the site root.
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])

    def test_without_a_ledger_the_page_says_it_has_no_record(self):
        with tempfile.TemporaryDirectory() as empty:
            source = pathlib.Path(empty)
            self.assertIsNone(tournaments_page.tournaments(source))
            page = source / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(tournaments_page.main(["tournaments_page.py", empty, str(page)]), 0)
            self.assertEqual(tournaments_page.SLOT.search(page.read_text(encoding="utf-8"))[2], "null")
            # A page with no slot is left byte for byte.
            page.write_text("<html>no slot</html>", encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(tournaments_page.main(["tournaments_page.py", empty, str(page)]), 0)
            self.assertEqual(page.read_text(encoding="utf-8"), "<html>no slot</html>")
        # The unfilled page shows one sentence and keeps the record hidden.
        self.assertIn('<p class="tour-empty" id="tour-empty">This copy of the page has no tournament record.', PAGE)
        self.assertIn('<div class="tour-record" id="tour-record" hidden>', PAGE)
        self.assertIn("if (!data) return;", PAGE)

    def test_a_definition_it_cannot_count_costs_the_record_not_the_publish(self):
        with tempfile.TemporaryDirectory() as scratch:
            source = pathlib.Path(scratch)
            (source / "data").mkdir()
            (source / "data" / "tournaments.json").write_text('{"tournaments": [{"series_size": "many"}]}', encoding="utf-8")
            rows = [run("civvis-20261121T100000Z", "win", "CIVILIZATION_ROME", 5)]
            results = home_rankings.results
            home_rankings.results = lambda _source: rows
            try:
                said = io.StringIO()
                with contextlib.redirect_stderr(said):
                    self.assertIsNone(tournaments_page.tournaments(source))
                self.assertIn("no record", said.getvalue())
            finally:
                home_rankings.results = results

    def test_every_page_lists_tournaments_between_home_and_civvis(self):
        # Operator 2026-11-21: "Between Home and CIVVIS, we want to add a
        # Tournaments view." This page lists the site's pages as the others
        # do, with itself in gold.
        self.assertIn(
            '<nav class="site-nav" aria-label="CIVVIS sites">\n'
            '        <a class="site-link" href="../home/">Home</a>\n'
            '        <a class="site-link" href="./" aria-current="page">Tournaments</a>\n'
            '        <a class="site-link" href="../" title="Every civilization is played by a CIVVIS AI agent.">CIVVIS</a>\n'
            '        <a class="site-link" href="../?mode=play" '
            'title="You play one civilization. CIVVIS AI agents play the rest.">AI Debugger</a>\n'
            '        <a class="site-link" href="https://store.steampowered.com/app/289070/Sid_Meiers_Civilization_VI/" '
            'target="_blank" rel="noopener" title="Buy Civilization VI on Steam">Civ 6</a>\n'
            "      </nav>",
            PAGE,
        )
        self.assertIn('.site-link[aria-current="page"]:hover { background: var(--gold); color: #17130c; }', PAGE)
        # The home page and the viewer name it second, each in its own markup.
        for page, pattern in (
            (LANDING, r'aria-current="page">Home</a>\s*<a class="site-link" href="\.\./tournaments/"[^>]*>Tournaments</a>'
                      r'\s*<a class="site-link" href="\.\./"[^>]*>CIVVIS</a>'),
            (VIEWER, r'id="homelink" href="/home" target="_top">Home</a>\s*'
                     r'<a class="home-link" id="tourlink" href="/tournaments" target="_top">Tournaments</a>\s*'
                     r'<a class="home-link" id="watchlink" href="\./" target="_top">CIVVIS</a>'),
        ):
            self.assertRegex(page, pattern)
        # The viewer's link is root-absolute as written and lane-relative as published.
        self.assertIn("""('href="/tournaments"', 'href="tournaments/"')""", PUBLISH)

    def test_the_page_draws_three_lists_and_trusts_nothing_as_markup(self):
        for piece in (
            '<section class="tour-section" id="tour-running" hidden aria-labelledby="tour-running-title">',
            '<section class="tour-section" id="tour-scheduled" aria-labelledby="tour-scheduled-title">',
            '<section class="tour-section" id="tour-finished" hidden aria-labelledby="tour-finished-title">',
            # The table that stood on the home page, with the win rate added.
            '<th scope="col">Tournament</th><th scope="col">Start</th><th scope="col">End</th><th scope="col">Games</th>'
            '<th scope="col">Record</th><th scope="col">Won</th><th scope="col">Conditions</th><th scope="col">Notes</th>',
            "const running = list(data.running);",
            "const scheduled = list(data.scheduled);",
            "const finished = list(data.finished);",
            # Scheduling is adding a definition, and the page says where.
            'href="https://github.com/MartinHalvorson/CIVVIS/blob/main/data/tournaments.json"',
            'href="https://github.com/MartinHalvorson/CIVVIS/blob/ledger/results.jsonl"',
            # The publish check finds the page by its slot (`beta/verify.py`).
            'id="tournaments-data"',
        ):
            self.assertIn(piece, PAGE)
        for unsafe in ("innerHTML", "insertAdjacentHTML", "document.write"):
            self.assertNotIn(unsafe, PAGE)
        verify = (BETA / "verify.py").read_text(encoding="utf-8")
        self.assertIn('f"{lane}tournaments/index.html",', verify)
        self.assertIn("""b'id="tournaments-data"' not in get("/tournaments/").read()""", verify)


if __name__ == "__main__":
    unittest.main()
