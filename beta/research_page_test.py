#!/usr/bin/env python3
"""The Research page: when each victory lands, counted off both kinds of game."""

from __future__ import annotations

import contextlib
import io
import json
import pathlib
import re
import subprocess
import tempfile
import unittest

import research_page

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "research.html").read_text(encoding="utf-8")
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")
TOURNAMENTS = (BETA / "tournaments.html").read_text(encoding="utf-8")
VIEWER = (ROOT / "web" / "index.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")


def run(tag: str, outcome: str, turns: int, victory: str | None = None, **more) -> dict:
    """One line of the results file, as `civ6_ladder.result_row` writes it."""
    return {
        "tag": tag,
        "game": re.sub(r"-cont\d+$", "", tag),
        "utc": "2026-11-21T10:00:00Z",
        "civ": "CIVILIZATION_ROME",
        "difficulty": "DIFFICULTY_DEITY",
        "players": 6,
        "map": "Continents.lua",
        "outcome": outcome,
        "victory": victory,
        "turns": turns,
        **more,
    }


FULL = ("GAMESPEED_ONLINE", 250)


class DeityGames(unittest.TestCase):
    def test_a_game_counts_once_by_its_victory_and_turn(self):
        rows = [
            run("civvis-20261120T100000Z", "rival_victory", 185, "VICTORY_TECHNOLOGY"),
            run("civvis-20261120T110000Z", "win", 173, "VICTORY_DIPLOMATIC"),
            # A reloaded game: two runs, one game, ended the best way any run did.
            run("civvis-20261121T090000Z", "rival_victory", 160, "VICTORY_CULTURE"),
            run("civvis-20261121T090000Z-cont1", "win", 171, "VICTORY_RELIGIOUS"),
            # No victory, so no type and no turn: left out.
            run("civvis-20261121T100000Z", "defeat", 120),
            run("civvis-20261121T110000Z", "abandoned", 90),
            run("civvis-20261121T120000Z", "unfinished", 40),
            # Another table, another level: left out.
            run("civvis-20261121T130000Z", "win", 150, "VICTORY_CULTURE", players=4, map="Pangaea.lua"),
            run("civvis-20261121T140000Z", "win", 150, "VICTORY_CULTURE", difficulty="DIFFICULTY_PRINCE"),
        ]
        groups = research_page.deity_groups(rows, lambda tags: {tag: FULL for tag in tags})
        self.assertEqual([g["day"] for g in groups], ["2026-11-20", "2026-11-21"])
        self.assertEqual(groups[0]["games"], 2)
        self.assertEqual(groups[0]["turns"], {"science": {185: 1}, "diplomatic": {173: 1}})
        self.assertEqual(groups[0]["won"], {"diplomatic": {173: 1}})
        self.assertEqual(groups[1]["games"], 1)
        self.assertEqual(groups[1]["turns"], {"religious": {171: 1}})

    def test_a_game_played_short_of_250_turns_is_left_out(self):
        # The Score series of 2026-11-12 to 15 ran to 170, 160 and 120 turns:
        # their Score victories at the limit would read as a spike that the
        # 250-turn game never has. A run with no summary says nothing of its
        # limit and is left out too; a longer limit is the same game to 250.
        rows = [
            run("civvis-20261113T100000Z", "rival_victory", 170, "VICTORY_SCORE"),
            run("civvis-20261113T110000Z", "rival_victory", 150, "VICTORY_TECHNOLOGY"),
            run("civvis-20261113T120000Z", "rival_victory", 190, "VICTORY_TECHNOLOGY"),
            run("civvis-20261113T130000Z", "rival_victory", 200, "VICTORY_TECHNOLOGY"),
            run("civvis-20261113T140000Z", "rival_victory", 210, "VICTORY_TECHNOLOGY"),
        ]
        known = {
            "civvis-20261113T100000Z": ("GAMESPEED_ONLINE", 170),
            "civvis-20261113T110000Z": ("GAMESPEED_STANDARD", 500),
            "civvis-20261113T130000Z": ("GAMESPEED_ONLINE", 500),
            "civvis-20261113T140000Z": FULL,
        }
        asked = []
        groups = research_page.deity_groups(rows, lambda tags: asked.extend(tags) or known)
        self.assertEqual(groups[0]["turns"], {"science": {200: 1, 210: 1}})
        # One lookup for every game that ended in a victory.
        self.assertEqual(sorted(asked), sorted(row["tag"] for row in rows))

    def test_the_turn_limits_come_from_the_run_summaries_on_the_ledger(self):
        with tempfile.TemporaryDirectory() as scratch:
            repo = pathlib.Path(scratch)
            git = lambda *args: subprocess.run(["git", "-C", scratch, *args], check=True, capture_output=True, text=True)
            git("init", "-q")
            for tag, summary in (
                ("civvis-a", {"speed": "GAMESPEED_ONLINE", "max_turns": 250, "outcome": {"turn": 190}}),
                ("civvis-b-cont1", {"speed": "GAMESPEED_ONLINE", "max_turns": 170}),
            ):
                (repo / "runs" / tag).mkdir(parents=True)
                (repo / "runs" / tag / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
            (repo / "runs" / "civvis-c").mkdir()
            (repo / "runs" / "civvis-c" / "summary.json").write_text("not json", encoding="utf-8")
            git("add", "-A")
            git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "ledger")
            git("update-ref", "refs/remotes/origin/ledger", "HEAD")
            found = research_page.limits(repo, ["civvis-a", "civvis-missing", "civvis-b-cont1", "civvis-c"])
            self.assertEqual(found, {"civvis-a": FULL, "civvis-b-cont1": ("GAMESPEED_ONLINE", 170)})
            self.assertEqual(research_page.limits(repo / "nowhere", ["civvis-a"]), {})


class Windows(unittest.TestCase):
    def test_a_window_counts_back_from_the_newest_game(self):
        groups = [
            {"day": "2026-10-20", "games": 4, "turns": {"score": {250: 4}}},
            {"day": "2026-11-15", "games": 3, "turns": {"science": {200: 2, 210: 1}}},
            {"day": "2026-11-21", "games": 2, "turns": {"diplomatic": {185: 1, 205: 1}}, "won": {"diplomatic": {205: 1}}},
        ]
        week = research_page.window(groups, 7)
        self.assertEqual((week["from"], week["to"], week["games"]), ("2026-11-15", "2026-11-21", 5))
        self.assertEqual(week["victories"]["science"]["median"], 200)
        self.assertEqual(week["victories"]["diplomatic"]["median"], 185)
        self.assertEqual(week["victories"]["diplomatic"]["won"], 1)
        # CIVVIS's wins are placed on their turns too, so the page can draw
        # them apart from the Deity AI's; a victory CIVVIS never won carries
        # no row of zeros.
        won = week["victories"]["diplomatic"]["won_turns"]
        self.assertEqual((len(won), won[204], sum(won)), (250, 1, 1))
        self.assertNotIn("won_turns", week["victories"]["science"])
        self.assertNotIn("won_turns", week["victories"]["score"])
        self.assertEqual(week["victories"]["score"]["games"], 0)
        self.assertIsNone(week["victories"]["score"]["median"])
        turns = week["victories"]["diplomatic"]["turns"]
        self.assertEqual(len(turns), 250)
        self.assertEqual((turns[184], turns[204], sum(turns)), (1, 1, 2))
        every = research_page.window(groups, None)
        self.assertEqual((every["from"], every["games"]), ("2026-10-20", 9))
        self.assertEqual(every["victories"]["score"]["turns"][249], 4)
        # A victory found as the last turn ends is stamped the turn after; it
        # is the last turn's. Nothing else outside the game counts.
        self.assertEqual(research_page.per_turn({251: 2, 250: 1, 252: 5, 0: 3})[249], 3)
        self.assertEqual(sum(research_page.per_turn({252: 5, 0: 3})), 0)
        self.assertIsNone(research_page.window([], 7))

    def test_every_victory_is_listed_in_one_order(self):
        self.assertEqual(research_page.VICTORIES, ["science", "culture", "diplomatic", "religious", "domination", "score"])
        self.assertEqual(sorted(research_page.LEDGER.values()), sorted(research_page.VICTORIES))
        # The page colours them in that order, and the stack runs in it.
        colours = re.findall(r"--v-(\w+): #[0-9a-f]{6};", PAGE)
        self.assertEqual(colours, research_page.VICTORIES)


class SelfPlay(unittest.TestCase):
    HEADER = {"kind": "header", "speed": "online", "turns": 250, "players": 6, "map": "continents", "all_seats": True}

    def batch(self, root: pathlib.Path, name: str, header: dict, games: list[tuple[int, str, int]]) -> None:
        directory = root / name
        directory.mkdir(parents=True)
        lines = [json.dumps(header)]
        for seed, victory, turn in games:
            for seat in range(6):  # every seat names the same ending
                lines.append(json.dumps({"kind": "game", "seed": seed, "seat": seat, "victory": victory, "turn": turn}))
        lines.append("{torn")
        (directory / "rows-continuous.jsonl").write_text("\n".join(lines) + "\n", encoding="utf-8")

    def test_the_snapshot_holds_a_game_per_seed_and_only_the_one_table(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = pathlib.Path(scratch)
            self.batch(root, "20261120T000256Z-6ad9", self.HEADER, [(1, "science", 204), (2, "diplomatic", 205), (3, "science", 204)])
            self.batch(root, "20261121T083532Z-db65", self.HEADER, [(4, "score", 250)])
            self.batch(root, "20261119T000000Z-aaaa", {**self.HEADER, "turns": 125}, [(5, "science", 120)])
            (root / "not-a-batch").mkdir()
            snap = research_page.snapshot(root)
            self.assertEqual([b["batch"] for b in snap["batches"]], ["20261120T000256Z-6ad9", "20261121T083532Z-db65"])
            first = snap["batches"][0]
            self.assertEqual((first["began"], first["games"]), ("2026-11-20", 3))
            self.assertEqual(first["turns"], {"science": {"204": 2}, "diplomatic": {"205": 1}})
            self.assertGreaterEqual(first["ended"], first["began"])
            # The file is a line per batch and reads back as it was written.
            text = research_page.written(snap)
            self.assertEqual(json.loads(text), snap)
            self.assertEqual(sum(line.startswith('{"batch"') for line in text.splitlines()), 2)
            # And the page reads it as a group per batch.
            groups = research_page.self_play_groups(snap)
            self.assertEqual(groups[0]["turns"]["science"], {204: 2})

    def test_the_committed_snapshot_is_the_one_table(self):
        snap = json.loads((ROOT / research_page.SELF_PLAY).read_text(encoding="utf-8"))
        self.assertTrue(snap["batches"])
        self.assertRegex(snap["taken"], r"^\d{4}-\d{2}-\d{2}$")
        for batch in snap["batches"]:
            self.assertRegex(batch["batch"], research_page.BATCH.pattern)
            counted = sum(n for turns in batch["turns"].values() for n in turns.values())
            self.assertEqual(counted, batch["games"])
            self.assertLessEqual(set(batch["turns"]), set(research_page.VICTORIES))
            # 251 is a victory found as turn 250 ended; `per_turn` counts it on 250.
            self.assertTrue(all(1 <= int(turn) <= 251 for turns in batch["turns"].values() for turn in turns))


class Page(unittest.TestCase):
    def test_the_page_carries_one_empty_slot_and_publishing_fills_it(self):
        self.assertEqual(PAGE.count('<script type="application/json" id="research-data">null</script>'), 1)
        data = {"turns": 250, "victories": ["science"], "self_play": {"name": "</script><b>x"}, "deity": None}
        filled = research_page.fill(PAGE, data)
        self.assertEqual(filled.count("</script>"), PAGE.count("</script>"))
        self.assertEqual(json.loads(research_page.SLOT.search(filled)[2]), data)
        self.assertEqual(research_page.SLOT.sub("", filled), research_page.SLOT.sub("", PAGE))
        for line in (
            'research_page="$source_tree/beta/research.html"',
            '[ -f "$research_page" ] || research_page="$repo_root/beta/research.html"',
            'cp "$research_page" "$out/research/index.html"',
            'research_page.py" "$repo_root" "$out/research/index.html"',
            'out / "research" / "index.html": [],',
            """('href="/research"', 'href="research/"'),""",
        ):
            self.assertIn(line, PUBLISH)
        # Authored lane-relative: nothing in it links the site root.
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])

    def test_without_figures_the_page_says_so(self):
        with tempfile.TemporaryDirectory() as empty:
            source = pathlib.Path(empty)
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertIsNone(research_page.research(source))
            page = source / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(research_page.main(["research_page.py", empty, str(page)]), 0)
            self.assertEqual(research_page.SLOT.search(page.read_text(encoding="utf-8"))[2], "null")
            page.write_text("<html>no slot</html>", encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(research_page.main(["research_page.py", empty, str(page)]), 0)
            self.assertEqual(page.read_text(encoding="utf-8"), "<html>no slot</html>")
        self.assertIn('<p class="res-empty" id="res-empty">This copy of the page has no figures.', PAGE)
        self.assertIn('<div class="res-record" id="res-record" hidden>', PAGE)
        self.assertIn("if (!data) return;", PAGE)

    def test_the_page_draws_from_data_and_trusts_none_of_it_as_markup(self):
        self.assertNotIn("innerHTML", PAGE)
        self.assertNotIn("insertAdjacentHTML", PAGE)
        # The two kinds of game, the two charts and the tables.
        for piece in ('id="res-kind-self_play"', 'id="res-kind-deity"', 'id="res-timing"', 'id="res-summary"', 'id="res-spans"'):
            self.assertIn(piece, PAGE)

    def test_the_deity_games_are_drawn_by_every_victory_then_by_each_sides_wins(self):
        # Operator 2026-11-21: "on civvis vs deity ai, start with all
        # victories by type along timeline, then just the deity ai wins, then
        # the civvis wins, then the civvis self play stuff".
        # The script draws its column charts, and the groups of the table of
        # spans, off one list in that order.
        charts = re.findall(r'\{ id: "([\w-]+)", kind: "(\w+)", side: "(\w+)", label: "([^"]+)"', PAGE)
        self.assertEqual(
            charts,
            [
                ("deity", "deity", "all", "CIVVIS vs Deity AI: every victory"),
                ("deity-ai", "deity", "rival", "CIVVIS vs Deity AI: Deity AI wins"),
                ("deity-civvis", "deity", "civvis", "CIVVIS vs Deity AI: CIVVIS wins"),
                ("self_play", "self_play", "all", "CIVVIS self-play"),
            ],
        )
        self.assertIn("for (const chart of CHARTS) drawStacks(chart, ymax);", PAGE)
        self.assertIn("for (const chart of CHARTS) {\n        const th = el(\"th\", \"is-group\", chart.label);", PAGE)
        # The markup the script fills stands in the same order, each chart
        # under the title the list gives it.
        at = []
        for name, _, _, label in charts:
            box = f'<div class="res-kind" id="res-kind-{name}">\n          <h3 class="res-kind-title">{label} <small data-count></small></h3>'
            self.assertEqual(PAGE.count(box), 1, name)
            at.append(PAGE.index(box))
        self.assertEqual(at, sorted(at))
        self.assertEqual(PAGE.count('<div class="res-kind" id="res-kind-'), len(charts))
        # The two sides' charts are the first one split by who won: what is
        # not CIVVIS's win on a turn is the Deity AI's, and every column is a
        # share of all the games counted, so the two add up to the first.
        self.assertIn('if (side === "civvis") return won;', PAGE)
        self.assertIn("return all.map((n, i) => Math.max(0, (Number(n) || 0) - (Number(won[i]) || 0)));", PAGE)
        self.assertIn("const h = y(0) - y(s.bins[v][i] / s.w.games);", PAGE)

    def test_the_deity_games_are_listed_before_the_self_play_games(self):
        # Operator 2026-11-21: "list the civvis vs deity ai stuff before the
        # civvis self-play. still side by side, just swap the ordering".
        # The script lists per kind in this order: the counts, the figures
        # above each small chart, the hover rows and both tables' rows.
        self.assertIn('const KINDS = ["deity", "self_play"];', PAGE)
        # And the markup the script fills, in the same order: the two charts,
        # the line key, the notes and their sources.
        for first, second in (
            ('id="res-kind-deity"', 'id="res-kind-self_play"'),
            ("</span>CIVVIS vs Deity AI</li>", "</span>CIVVIS self-play (in the victory"),
            ('id="res-note-deity"', 'id="res-note-self"'),
            (">Deity games</a>", ">Self-play games</a>"),
        ):
            self.assertEqual((PAGE.count(first), PAGE.count(second)), (1, 1), (first, second))
            self.assertLess(PAGE.index(first), PAGE.index(second), (first, second))
        # The figures table keeps the two side by side, the Deity columns on
        # the left with the two columns self-play has no use for: who won,
        # the Deity AI before CIVVIS as the charts stand.
        self.assertIn(
            '<tr><th></th><th class="is-group" colspan="6">CIVVIS vs Deity AI</th>'
            '<th class="is-group" colspan="4">CIVVIS self-play</th></tr>',
            PAGE,
        )
        self.assertRegex(
            PAGE,
            r'<th scope="col">Victory</th>\s*'
            r'<th scope="col">Games</th><th scope="col">Share</th><th scope="col">Deity AI won</th><th scope="col">CIVVIS won</th>'
            r'<th scope="col">Median turn</th><th scope="col">Busiest span</th>\s*'
            r'<th scope="col">Games</th><th scope="col">Share</th>'
            r'<th scope="col">Median turn</th><th scope="col">Busiest span</th>\s*</tr>',
        )

    def test_every_page_lists_research_between_home_and_tournaments(self):
        # Operator 2026-11-21: "between home and tournaments, we should add a
        # "Research"." This page lists the site's pages as the others do,
        # with itself in gold. Its name in the list and on the tab is
        # Musings since later that day (operator: "musings (rename
        # research)"); the address, the file and the data kept theirs.
        self.assertIn(
            '<nav class="site-nav" aria-label="CIVVIS sites">\n'
            '        <a class="site-link" href="../home/">Home</a>\n'
            '        <a class="site-link" href="./" aria-current="page">Musings</a>\n'
            '        <a class="site-link" href="../tournaments/" ',
            PAGE,
        )
        self.assertRegex(PAGE, r'<div class="site-brand">\s*<!--[\s\S]*?-->\s*<nav class="site-nav"[^>]*>[\s\S]*?</nav>\s*<div class="site-lockup">')
        self.assertIn("<title>Musings · CIVVIS.ai</title>", PAGE)
        for page in (PAGE, LANDING, TOURNAMENTS, VIEWER):
            self.assertNotIn(">Research</a>", page)
        for page, pattern in (
            (LANDING, r'aria-current="page">Home</a>\s*<a class="site-link" href="\.\./research/"[^>]*>Musings</a>'
                      r'\s*<a class="site-link" href="\.\./tournaments/"'),
            (TOURNAMENTS, r'href="\.\./home/">Home</a>\s*<a class="site-link" href="\.\./research/"[^>]*>Musings</a>'
                          r'\s*<a class="site-link" href="\./" aria-current="page">Tournaments</a>'),
            (VIEWER, r'id="homelink" href="/home" target="_top">Home</a>\s*'
                     r'<a class="home-link" id="researchlink" href="/research" target="_top">Musings</a>\s*'
                     r'<a class="home-link" id="tourlink" href="/tournaments" target="_top">Tournaments</a>'),
        ):
            self.assertRegex(page, pattern)


if __name__ == "__main__":
    unittest.main()
