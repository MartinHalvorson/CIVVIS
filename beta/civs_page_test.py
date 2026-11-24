#!/usr/bin/env python3
"""Civ Analytics: the tree of early build orders first, then the cities and
the faith under one row of filters, then the winningest civs.

    python3 beta/civs_page_test.py
"""

from __future__ import annotations

import contextlib
import gzip
import io
import json
import pathlib
import re
import subprocess
import tempfile
import unittest
from unittest import mock

import civs_page

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "civs.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")


def run(tag: str, outcome: str, difficulty: str = "DIFFICULTY_DEITY", **more) -> dict:
    """One line of the results file, as `civ6_ladder.result_row` writes it."""
    return {
        "tag": tag,
        "game": re.sub(r"-cont\d+$", "", tag),
        "utc": "2026-11-21T10:00:00Z",
        "civ": "CIVILIZATION_ROME",
        "difficulty": difficulty,
        "players": 6,
        "map": "Continents.lua",
        "outcome": outcome,
        **more,
    }


def ledger(scratch: str, rows: list[dict], runs: dict[str, tuple[list[dict], dict | None]]) -> pathlib.Path:
    """A repository whose `origin/ledger` holds `rows` as its results file and
    each run's events log and summary."""
    repo = pathlib.Path(scratch)
    git = lambda *args: subprocess.run(["git", "-C", scratch, *args], check=True, capture_output=True, text=True)
    git("init", "-q")
    (repo / "results.jsonl").write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
    for tag, (events, summary) in runs.items():
        (repo / "runs" / tag).mkdir(parents=True)
        if summary is not None:
            (repo / "runs" / tag / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
        if events:
            with gzip.open(repo / "runs" / tag / "events.jsonl.gz", "wt", encoding="utf-8") as log:
                log.write("".join(json.dumps(event) + "\n" for event in events))
    git("add", "-A")
    git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "ledger")
    git("update-ref", "refs/remotes/origin/ledger", "HEAD")
    return repo


def found(turn: int, x: int) -> dict:
    return {"kind": "found", "turn": turn, "x": x, "y": 1}


class TheOrderOfThePage(unittest.TestCase):
    def test_the_build_orders_come_first_and_the_civs_last(self):
        # Operator 2026-11-24: "move winngest civs to the bottom ... top chart
        # is build order"; between them the cities and the faith.
        record = PAGE[PAGE.index('<div class="civ-record" id="civ-record" hidden>'):]
        sections = re.findall(r'<section [^>]*id="([a-z0-9-]+)"', record)
        self.assertEqual(sections, [
            "res-side-builds",
            "civ-filtered", "civ-city2", "civ-city3", "civ-pantheons", "civ-founders", "civ-followers",
            "civ-civs", "rank-civs", "rank-civs-self",
        ])
        self.assertIn('<h2 class="res-side-title" id="res-title-builds">Early build order: win rate against Lv 8 Deity AI</h2>', PAGE)
        self.assertIn('<h2 class="res-side-title" id="civ-civs-title">Winningest civs</h2>', PAGE)
        # Two tables of civs: against Deity, and the self-play seats.
        self.assertIn('<h3 class="rank-title" id="rank-civs-title">Against Lv 8 Deity AI</h3>', PAGE)
        self.assertIn('<h3 class="rank-title" id="rank-civs-self-title">CIVVIS self-play</h3>', PAGE)
        # Operator: "add a note that winningest civs is just due to whatever
        # modifications and chosen for the civ and how well the ai straetgy
        # handles that".
        why = re.sub(r"\s+", " ", PAGE[PAGE.index('<p class="civ-why">'):PAGE.index("</p>", PAGE.index('<p class="civ-why">'))])
        self.assertIn("the bonuses Civilization VI gives it", why)
        self.assertIn("how well the AI&#8217;s strategy makes use of them", why)

    def test_one_row_of_filters_scopes_the_cities_and_the_faith(self):
        # Operator: "filterable by the typical stuff (lvs, etc)"; the cities
        # "by lv default lv 8".
        part = PAGE[PAGE.index('id="civ-filtered"'):PAGE.index('id="civ-civs"')]
        self.assertEqual(part.count('class="civ-filters"'), 1)
        self.assertLess(part.index('class="civ-filters"'), part.index('id="civ-city2"'))
        for box in ("civ-f-games", "civ-f-diplo", "civ-f-days"):
            self.assertIn(f'class="civ-seg" id="{box}"', part)
        self.assertIn("OPENING_LEVEL = 8", (BETA / "civs_page.py").read_text(encoding="utf-8"))
        self.assertIn("pick.games === \"self\"", PAGE)
        # The note says what a Settler first is.
        self.assertIn("A Settler can come first: the capital needs two citizens to train one", PAGE)

    def test_the_spans_of_turns_cover_every_turn_once(self):
        spans = re.search(r"const SPANS = \{\s*city2: (\[.*?\]\]),\s*city3: (\[.*?\]\]),", PAGE, re.S)
        for text in spans.groups():
            listed = json.loads(text)
            self.assertEqual(listed[-1], [0, 0])
            turns = [t for lo, hi in listed[:-1] for t in range(lo, hi + 1)]
            self.assertEqual(turns, list(range(1, 251)))

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


class TheRecord(unittest.TestCase):
    def test_the_record_is_the_tree_the_games_the_self_play_and_the_civs(self):
        tree = {"level": 8, "games": 2, "tree": [[0, 2, 1]], "items": ["Scout"]}
        civs = {"rows": [{"name": "Greece", "wins": 1, "losses": 1}], "games": 2}
        games = {"rows": [[8, 1, 1, 0, 12, 20, 1, 2, 3]]}
        with mock.patch.object(civs_page.research_page, "research", return_value={"build_orders": tree, "levels": {}}), \
                mock.patch.object(civs_page.ranks, "civs", return_value=civs), \
                mock.patch.object(civs_page.ranks, "results", return_value=[{}]), \
                mock.patch.object(civs_page, "game_rows", return_value=games), \
                contextlib.redirect_stderr(io.StringIO()):
            record = civs_page.record(BETA.parent)
        self.assertEqual((record["build_orders"], record["civs"], record["games"]), (tree, civs, games))
        # What cannot be read costs the page that figure, never the publish.
        with mock.patch.object(civs_page.research_page, "research", side_effect=ValueError("no ledger")), \
                mock.patch.object(civs_page.ranks, "civs", return_value=None), \
                mock.patch.object(civs_page.ranks, "results", return_value=None), \
                mock.patch.object(civs_page, "SELF_PLAY", pathlib.Path("no/such/file.json")), \
                contextlib.redirect_stderr(io.StringIO()) as said:
            self.assertIsNone(civs_page.record(BETA.parent))
        self.assertIn("civs_page: no tree of build orders (no ledger)", said.getvalue())
        self.assertIn("civs_page: no self-play seats", said.getvalue())

    def test_a_belief_is_named_as_the_game_names_it(self):
        self.assertEqual(civs_page.belief_name("BELIEF_GOD_OF_THE_FORGE"), "God of the Forge")
        self.assertEqual(civs_page.belief_name("BELIEF_LADY_OF_THE_REEDS_AND_MARSHES"), "Lady of the Reeds and Marshes")
        self.assertEqual(civs_page.belief_name("BELIEF_DIVINE_SPARK"), "Divine Spark")
        self.assertEqual(civs_page.belief_name("BELIEF_CROSS_CULTURAL_DIALOGUE"), "Cross-Cultural Dialogue")

    def test_a_games_runs_are_read_as_one_game(self):
        first = {"cities": [1, 14], "pantheon": None, "religion": None}
        later = {"cities": [12, 14, 30, 41], "pantheon": "BELIEF_DIVINE_SPARK",
                 "religion": {"turn": 40, "founder": "BELIEF_TITHE", "follower": "BELIEF_WORK_ETHIC"}}
        # A reloaded run plays a few turns over: only its cities past the
        # first run's last count.
        self.assertEqual(civs_page.merged([first, later]), {
            "cities": [1, 14, 30, 41], "pantheon": "BELIEF_DIVINE_SPARK", "religion": later["religion"]})
        # No first run, no opening: the game is not read.
        self.assertIsNone(civs_page.merged([None, later]))
        held = civs_page.held_marks({"games": [["g", [1, 9, 20], "BELIEF_DIVINE_SPARK", [30, "BELIEF_TITHE", "BELIEF_WORK_ETHIC"]],
                                               ["h", [1], None, None]]})
        self.assertEqual(held["g"]["religion"], {"turn": 30, "founder": "BELIEF_TITHE", "follower": "BELIEF_WORK_ETHIC"})
        self.assertEqual(held["h"], {"cities": [1], "pantheon": None, "religion": None})

    def test_a_row_per_game_with_its_level_switch_result_day_cities_and_faith(self):
        games = [("a", ["a"], True, "2026-11-20", 8), ("b", ["b"], False, "2026-11-22", 8),
                 ("c", ["c"], False, "2026-11-21", 6), ("d", ["d"], False, "2026-11-21", 6)]
        held = {
            "a": {"cities": [1, 12, 25], "pantheon": "BELIEF_DIVINE_SPARK",
                  "religion": {"founder": "BELIEF_TITHE", "follower": "BELIEF_WORK_ETHIC"}},
            "b": {"cities": [1, 30], "pantheon": "BELIEF_DIVINE_SPARK", "religion": None},
            "c": {"cities": [1], "pantheon": None, "religion": None},
        }
        with mock.patch.object(civs_page, "finished_games", return_value=games), \
                mock.patch.object(civs_page, "summary_marks", return_value={}), \
                mock.patch.object(civs_page.research_page, "switches", return_value={"a": True, "b": False}):
            out = civs_page.game_rows(BETA.parent, [{}], held)
        self.assertEqual(out["fields"], ["level", "diplo", "won", "day", "city2", "city3", "pantheon", "founder", "follower"])
        self.assertEqual(out["first"], "2026-11-20")
        self.assertEqual(out["names"], ["Divine Spark", "Tithe", "Work Ethic"])
        # Oldest first; "d" has no reading and is only counted as finished.
        self.assertEqual(out["rows"], [[8, 1, 1, 0, 12, 25, 1, 2, 3], [6, 2, 0, 1, 0, 0, 0, 0, 0], [8, 0, 0, 2, 30, 0, 1, 0, 0]])
        self.assertEqual(out["finished"], {"8": 2, "6": 2})
        self.assertEqual(out["level"], 8)


class TheSnapshots(unittest.TestCase):
    def test_the_marks_are_read_off_the_event_logs_or_the_summaries(self):
        first, second, third = "civvis-20261120T010000Z", "civvis-20261120T020000Z", "civvis-20261120T030000Z"
        rows = [run(first, "win"), run(first + "-cont1", "win"), run(second, "defeat", "DIFFICULTY_EMPEROR"),
                run(third, "rival_victory"), run("civvis-20261120T040000Z-cont1", "defeat")]
        pantheon = {"kind": "order_verified", "order_kind": "pantheon", "turn": 18, "verb": "BELIEF_DIVINE_SPARK"}
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()):
            repo = ledger(scratch, rows, {
                first: ([found(1, 1), found(14, 2), {"kind": "state", "turn": 15}], {}),
                # The game's second run founds its third city and its pantheon.
                first + "-cont1": ([found(14, 2), found(31, 3), pantheon], {}),
                second: ([found(1, 1), found(20, 2)], {}),
                # Its summaries carry the reading: left to them.
                third: ([found(1, 1)], {"founding_marks": {"cities": [1, 9, 15], "pantheon": None, "religion": None}}),
                # A game whose first run was never published.
                "civvis-20261120T040000Z-cont1": ([found(150, 9)], {}),
            })
            out = repo / "out" / "game_marks.json"
            self.assertEqual(civs_page.main(["civs_page.py", "--marks", scratch, str(out)]), 0)
            taken = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(taken["games"], [[first, [1, 14, 31], "BELIEF_DIVINE_SPARK", None], [second, [1, 20], None, None]])
            self.assertEqual(out.read_text(encoding="utf-8"), civs_page.written_marks(taken))
            # On the page: the snapshot's games and the summary's.
            (repo / civs_page.MARKS).parent.mkdir(parents=True, exist_ok=True)
            (repo / civs_page.MARKS).write_text(out.read_text(encoding="utf-8"), encoding="utf-8")
            record = civs_page.record(repo)["games"]
        self.assertEqual(sorted(row[0] for row in record["rows"]), [6, 8, 8])
        self.assertEqual(sorted((row[4], row[5]) for row in record["rows"]), [(9, 15), (14, 31), (20, 0)])
        self.assertEqual(record["finished"], {"6": 1, "8": 3})

    def test_the_self_play_seats_are_counted_batch_by_batch(self):
        header = {"kind": "header", **civs_page.research_page.SETUP}
        seat = lambda win, civ, two=None, three=None, religion=True: {
            "kind": "game", "win": win, "civ": civ, "founded_religion": religion,
            **({"city_two_turn": two} if two else {}), **({"city_three_turn": three} if three else {})}
        with tempfile.TemporaryDirectory() as scratch:
            batches = pathlib.Path(scratch)
            old, new, other = (batches / name for name in ("20261115T000000Z-aaaa", "20261121T000000Z-bbbb", "20261122T000000Z-cccc"))
            for directory, rows in (
                # Before the rows wrote their cities: none counted, not "never".
                (old, [header, seat(True, "Rome"), seat(False, "Greece", religion=False)]),
                (new, [header, seat(True, "Rome", 12, 20), seat(False, "Rome", 12), seat(False, "Greece", 30, 41, False)]),
                (other, [{**header, "players": 8}, seat(True, "Rome", 9, 15)]),
            ):
                directory.mkdir()
                (directory / "rows-continuous.jsonl").write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
            (batches / "not-a-batch").mkdir()
            out = batches / "seats.json"
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(civs_page.main(["civs_page.py", "--self-play", scratch, str(out)]), 0)
            taken = json.loads(out.read_text(encoding="utf-8"))
        self.assertEqual([b["batch"] for b in taken["batches"]], [old.name, new.name])
        self.assertNotIn("city2", taken["batches"][0])
        self.assertEqual(taken["batches"][1]["city2"], {"12": [2, 1], "30": [1, 0]})
        self.assertEqual(taken["batches"][1]["city3"], {"0": [1, 0], "20": [1, 1], "41": [1, 0]})
        self.assertEqual(taken["batches"][0]["religion"], {"0": [1, 0], "1": [1, 1]})
        drawn = civs_page.self_play(taken)
        self.assertEqual([b["from"] for b in drawn["batches"]], ["2026-11-15", "2026-11-21"])
        # The civs over every batch, best win rate first.
        self.assertEqual(drawn["civs"]["rows"], [["Rome", 3, 2], ["Greece", 2, 0]])
        self.assertEqual((drawn["civs"]["seats"], drawn["civs"]["batches"]), (5, 2))

    def test_the_committed_snapshots_hold_what_the_page_reads(self):
        marks = json.loads((ROOT / civs_page.MARKS).read_text(encoding="utf-8"))
        self.assertEqual((ROOT / civs_page.MARKS).read_text(encoding="utf-8"), civs_page.written_marks(marks))
        self.assertGreater(len(marks["games"]), 3000)
        keys = [game[0] for game in marks["games"]]
        self.assertEqual(keys, sorted(set(keys)))
        for key, cities, pantheon, religion in marks["games"]:
            self.assertTrue(len(cities) <= 3 and cities == sorted(cities), key)
            self.assertTrue(pantheon is None or pantheon.startswith("BELIEF_"), key)
            self.assertTrue(religion is None or len(religion) == 3, key)
        seats = json.loads((ROOT / civs_page.SELF_PLAY).read_text(encoding="utf-8"))
        self.assertEqual((ROOT / civs_page.SELF_PLAY).read_text(encoding="utf-8"), civs_page.written_self_play(seats))
        self.assertTrue(any("city2" in b for b in seats["batches"]))
        self.assertTrue(civs_page.self_play(seats)["civs"]["rows"])


if __name__ == "__main__":
    unittest.main()
