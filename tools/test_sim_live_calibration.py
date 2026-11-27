#!/usr/bin/env python3
"""Hermetic tests for `tools/sim_live_calibration.py`, and the facts
`docs/GENE_SCREEN.md` ("What the screen measures") states about the field the
gene ledger is priced on, each read from the file that makes it true."""

from __future__ import annotations

import io
import json
import re
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import sim_live_calibration as cal  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent

RATINGS = {
    "holy": {"elo": 1400.0, "margin95": 20.0},
    "moonshot": {"elo": 1100.0, "margin95": 20.0},
    "jack": {"elo": 1150.0, "margin95": 20.0},
    "civ6:prince": {"elo": 1000.0},
    "civ6:deity": {"elo": 1900.0},
}


def result(tag, outcome, difficulty="DIFFICULTY_DEITY", target="science", **extra):
    return {"tag": tag, "game": tag, "outcome": outcome, "difficulty": difficulty, "target": target,
            "players": 6, "map": "Continents.lua", "civ": "CIVILIZATION_ROME", **extra}


def header(**extra):
    return {"kind": "header", "players": 6, "difficulty": "deity", **extra}


def seat(seed, win, kind="game", **extra):
    return {"kind": kind, "seed": seed, "seat": 0, "win": win, "turn": 180, "rank": 6, **extra}


class LiveGames(unittest.TestCase):
    def test_a_game_counts_once_by_its_best_run_at_the_benchmark_table(self):
        rows = [
            result("civvis-t6-20261121T000000Z", "win"),                     # before the benchmark
            result("civvis-t7-20261123T000000Z", "unfinished"),
            result("civvis-t7-20261123T000000Z-cont1", "win"),               # its resume won
            result("civvis-t7-20261123T010000Z", "unfinished"),              # never reached a board
            result("civvis-t7-20261123T020000Z", "rival_victory", players=5),
            result("civvis-t7-20261123T030000Z", "defeat", map="Pangaea.lua"),
            result("civvis-t7-20261123T040000Z", "win", civ="CIVILIZATION_ROME+CIVILIZATION_CHINA"),
            result("civvis-t8-20261126T000000Z", "rival_victory"),
            result("civvis-t8-20261126T010000Z", "abandoned"),
        ]
        games = cal.live_games(rows)
        self.assertEqual([g["game"] for g in games],
                         ["civvis-t7-20261123T000000Z", "civvis-t8-20261126T000000Z",
                          "civvis-t8-20261126T010000Z"])
        self.assertEqual([g["outcome"] for g in games], ["win", "rival_victory", "abandoned"])

    def test_levels_read_both_spellings(self):
        self.assertEqual(cal.level_of("DIFFICULTY_DEITY"), "deity")
        self.assertEqual(cal.level_of("immortal"), "immortal")
        self.assertIsNone(cal.level_of("DIFFICULTY_NONSENSE"))


class EloModel(unittest.TestCase):
    def test_one_seat_among_equals_wins_one_table_in_six(self):
        self.assertAlmostEqual(cal.lone_seat_win(1500, 1500, 5), 1 / 6)
        # 400 points stronger is ten times the strength.
        self.assertAlmostEqual(cal.lone_seat_win(1900, 1500, 5), 10 / 15)

    def test_the_prediction_averages_each_game_s_twin(self):
        games = [result("a", "win", target="religious"), result("b", "defeat", target="science"),
                 result("c", "defeat", target=None)]
        expected = sum(cal.lone_seat_win(elo, 1900, 5) for elo in (1400, 1100, 1150)) / 3
        self.assertAlmostEqual(cal.elo_prediction(games, RATINGS, "deity"), expected)
        self.assertIsNone(cal.elo_prediction(games, RATINGS, "king"))  # no civ6:king rating

    def test_the_implied_rating_is_the_one_that_predicts_the_record(self):
        # Sixty games of an equal-strength twin winning ten: the fit's level
        # sits where the twin's expected wins meet them, near the twin.
        games = [result(f"g{i}", "win" if i < 10 else "rival_victory", target="religious")
                 for i in range(60)]
        fit = cal.implied_rival_elo(games, RATINGS)
        self.assertAlmostEqual(fit["elo"], 1400, delta=40)
        self.assertEqual(fit["bound"], "")
        always = cal.implied_rival_elo([result("w", "win", target="religious")] * 5, RATINGS)
        self.assertEqual(always["bound"], "upper")
        self.assertEqual(cal.rated_text(always), f"≤ {always['elo']:.0f}")


class Engine(unittest.TestCase):
    def write(self, folder, name, rows):
        path = Path(folder) / name
        path.write_text("".join(json.dumps(row) + "\n" for row in rows))
        return str(path)

    def test_only_the_live_arrangement_is_read_and_rival_chairs_are_skipped(self):
        with tempfile.TemporaryDirectory() as folder:
            live = self.write(folder, "live.jsonl", [
                header(rivals="firaxis-faithful", handicap="rivals", rival_chairs=5, rival_rung="-2"),
                seat(1, True, difficulty="deity"), seat(1, False, kind="rival", difficulty="deity"),
                seat(2, False, difficulty="deity"), seat(3, False, difficulty="deity")])
            levels, fields = cal.engine_levels([live])
            self.assertEqual(levels["deity"]["wins"], 1)
            self.assertEqual(levels["deity"]["games"], 3)
            self.assertIn("rival_rung=-2", fields[0])
            selfplay = self.write(folder, "selfplay.jsonl", [
                header(all_seats=True), *[seat(1, i == 0) for i in range(6)]])
            with self.assertRaises(SystemExit) as refused:
                cal.engine_levels([selfplay])
            self.assertIn("by symmetry", str(refused.exception))

    def test_max_miss_fails_a_field_that_does_not_match_live(self):
        with tempfile.TemporaryDirectory() as folder:
            results = self.write(folder, "results.jsonl", [
                result(f"civvis-t7-20261123T0{i:02d}000Z", "win" if i < 5 else "rival_victory")
                for i in range(20)])
            elo = Path(folder) / "elo.json"
            elo.write_text(json.dumps({"ratings": RATINGS}))
            engine = self.write(folder, "engine.jsonl", [
                header(rivals="firaxis-faithful", handicap="rivals", rival_chairs=5),
                *[seat(s, s < 5, difficulty="deity") for s in range(20)]])
            out = io.StringIO()
            with redirect_stdout(out):
                self.assertEqual(cal.main(["--results", results, "--elo", str(elo), "--engine", engine,
                                           "--json"]), 0)
            row = json.loads(out.getvalue())["levels"][0]
            self.assertEqual((row["level"], row["live_wins"], row["live_games"]), ("deity", 5, 20))
            self.assertAlmostEqual(row["engine_miss_pp"], 0.0)
            self.assertLess(row["elo_miss_pp"], -20)  # the Elo table's stand-in is far too strong
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                self.assertEqual(cal.main(["--results", results, "--elo", str(elo), "--max-miss", "5"]), 1)
                self.assertEqual(cal.main(["--results", results, "--elo", str(elo), "--engine", engine,
                                           "--max-miss", "100"]), 0)
            table = io.StringIO()
            with redirect_stdout(table):
                cal.main(["--results", results, "--elo", str(elo), "--engine", engine])
            self.assertIn("| Lv 8 Deity | 5/20 |", table.getvalue())


class TheScreensField(unittest.TestCase):
    """What `docs/GENE_SCREEN.md` says the ledger's batches play, held to the
    files that decide it. If one of these fails, the batches have moved to a
    different field: relabel the ranking and the site, then change the test."""

    def test_prince_hands_no_seat_anything(self):
        rungs = json.loads((ROOT / "data" / "difficulties.json").read_text())
        handicaps = {key for key in rungs["prince"] if key.startswith(("ai_", "human_", "starting_"))}
        self.assertEqual(handicaps, set(), "Prince now carries a handicap; GENE_SCREEN.md says it carries none")
        self.assertTrue(rungs["deity"]["ai_yield_pct"] and rungs["deity"]["ai_bonus_units"])

    def test_the_screen_defaults_to_prince_with_deity_barbarians(self):
        game = (ROOT / "src" / "game.rs").read_text()
        self.assertRegex(game, r'pub fn default_difficulty\(\) -> String \{\s*"prince"\.to_string\(\)')
        screen = (ROOT / "src" / "bin" / "gene_screen.rs").read_text()
        self.assertIn('const SCREEN_BARBARIAN_DIFFICULTY: &str = "deity";', screen)
        self.assertIn('text(&args, "--difficulty").unwrap_or_else(civvis::game::default_difficulty)', screen)

    def test_the_continuous_batch_passes_no_field_flags(self):
        source = (ROOT / "tools" / "continuous_batch_scheduler.py").read_text()
        body = source[source.index("def run_segment("):]
        command = body[body.index("    command = ["):body.index("    ]", body.index("    command = ["))]
        flags = set(re.findall(r'"(--[a-z-]+)"', command))
        self.assertEqual(flags, {"--games", "--target-games", "--start-seed", "--jobs", "--out", "--append"})

    def test_every_reporting_batch_is_prince_self_play_and_says_so(self):
        ledger = json.loads((ROOT / "docs" / "gene_ledger.json").read_text())
        self.assertTrue(ledger["reporting_batches"])
        for batch in ledger["reporting_batches"]:
            profile = batch["profile"]
            self.assertTrue(profile["all_seats"], batch["path"])
            self.assertEqual(profile.get("difficulty", "prince"), "prince", batch["path"])
            self.assertFalse(profile.get("rivals"), batch["path"])
        ranking = (ROOT / "GENE_HEURISTIC_RANKING.md").read_text().splitlines()
        self.assertIn("Prince Self-Play", ranking[0])
        self.assertIn("not Deity evidence", ranking[1])
        self.assertTrue("## ⚠⚠ What the screen measures" in (ROOT / "docs" / "GENE_SCREEN.md").read_text(),
                        "docs/GENE_SCREEN.md lost the section the ranking's warning points at")


if __name__ == "__main__":
    unittest.main()
