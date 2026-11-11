#!/usr/bin/env python3
"""The paired strength test scores a pair the way its header says, and holds its bar."""

from __future__ import annotations

import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import site_pair  # noqa: E402

REPO = Path(__file__).resolve().parent.parent

REPORT = """[31.204s]
[turns 1-10: 10 frames, 19.8 ms a frame, 0 KB a document; majors 0.155s, \
city-states and barbarians 0.043s, documents 0.000s]
[all: majors 28.000s, city-states and barbarians 3.000s, documents 0.000s]
[city yields: 134122 answered from what was kept, 66660 derived for all seats]
[major seats, player id and seconds: 0=6.000 1=6.000 2=8.000 3=8.000 | minors]
checkpoint 151: save 6707716ae958c941, document b300a127d421924d
{ending}
  1. Rome  score 512
"""


def a_report(ending: str) -> str:
    return REPORT.format(ending=ending)


WON_BY_2 = a_report("Winner: Rome (player 2) by science on turn 171")
NOBODY = a_report("No winner: turn 301 of 500, and no enabled victory was achieved")


class AReportIsRead(unittest.TestCase):
    def test_the_winner_the_turn_and_each_majors_seconds(self):
        game = site_pair.parse_game(WON_BY_2)
        self.assertEqual((game["winner"], game["victory"], game["turn"]),
                         (2, "science", 171))
        self.assertEqual(game["seconds"], {0: 6.0, 1: 6.0, 2: 8.0, 3: 8.0})

    def test_a_game_nobody_won_has_no_winner_and_still_has_its_turn(self):
        game = site_pair.parse_game(NOBODY)
        self.assertIsNone(game["winner"])
        self.assertEqual(game["turn"], 301)

    def test_a_binary_without_the_seat_line_is_refused(self):
        with self.assertRaises(ValueError) as refused:
            site_pair.parse_game("[1.0s]\nWinner: Rome (player 0) by score on turn 9\n")
        self.assertIn("predates", str(refused.exception))

    def test_the_engine_prints_the_line_this_reads(self):
        """The other half of that contract, read from the engine's source."""
        source = (REPO / "src" / "main.rs").read_text(encoding="utf-8")
        self.assertIn('"[major seats, player id and seconds: {seats} | minors]"', source)
        self.assertIn('format!("{seat}={seconds:.3}")', source)


class APairIsScored(unittest.TestCase):
    def test_the_chairs_are_the_two_halves_of_the_majors(self):
        self.assertEqual(site_pair.split_majors([3, 0, 2, 1]), ([0, 1], [2, 3]))
        self.assertEqual(site_pair.split_majors([0, 1, 2, 3, 4, 5]), ([0, 1, 2], [3, 4, 5]))
        with self.assertRaises(ValueError):
            site_pair.split_majors([0])

    def test_a_shortcut_that_changes_nothing_scores_zero(self):
        """The same civilization wins both games and took the shortcut once."""
        game = site_pair.parse_game(WON_BY_2)
        pair = site_pair.treated_score(game, [0, 1]) + site_pair.treated_score(game, [2, 3])
        self.assertEqual(pair - 1.0, 0.0)

    def test_a_game_nobody_won_counts_half_to_each_side(self):
        self.assertEqual(site_pair.treated_score(site_pair.parse_game(NOBODY), [0, 1]), 0.5)

    def test_the_time_ratio_is_the_shortcuts_seats_over_the_rest(self):
        game = site_pair.parse_game(WON_BY_2)
        self.assertAlmostEqual(site_pair.time_ratio(game, [0, 1]), 0.75)
        self.assertAlmostEqual(site_pair.time_ratio(game, [2, 3]), 8.0 / 6.0)
        self.assertIsNone(site_pair.time_ratio(game, []))

    def test_the_command_names_the_shortcut_and_its_seats(self):
        command = site_pair.game_command(Path("/x/civvis"), 7, 400, "envelopes-across-own-moves",
                                         [2, 3])
        self.assertEqual(command[:2], ["/x/civvis", "site-bench"])
        self.assertEqual(command[command.index("--shortcut-seats") + 1], "2,3")
        self.assertEqual(command[command.index("--shortcuts") + 1], "envelopes-across-own-moves")
        self.assertIn("--no-documents", command)


class TheBarIsHeld(unittest.TestCase):
    def test_level_pairs_are_not_worse(self):
        summary = site_pair.summarise([0.0] * 80 + [1.0] * 10 + [-1.0] * 10)
        self.assertEqual(summary["verdict"], "NOT WORSE")
        self.assertEqual((summary["up"], summary["level"], summary["down"]), (10, 80, 10))
        self.assertAlmostEqual(summary["mean"], 0.0)

    def test_the_august_reading_would_not_have_passed(self):
        """43.8% of decided games is a mean pair difference of about -0.125,
        and over 40 pairs its interval reaches far below the margin."""
        differences = [-1.0] * 12 + [1.0] * 7 + [0.0] * 21
        summary = site_pair.summarise(differences)
        self.assertLess(summary["mean"], 0.0)
        self.assertNotEqual(summary["verdict"], "NOT WORSE")

    def test_a_clear_loss_is_called_one(self):
        summary = site_pair.summarise([-1.0] * 60 + [0.0] * 40)
        self.assertEqual(summary["verdict"], "WORSE")

    def test_too_few_pairs_to_say_is_unresolved_not_a_pass(self):
        summary = site_pair.summarise([0.0, -1.0, 1.0, 0.0, -1.0, 0.0])
        self.assertEqual(summary["verdict"], "UNRESOLVED")

    def test_the_margin_in_the_header_is_the_margin_in_the_code(self):
        self.assertEqual(site_pair.MARGIN, 0.15)
        self.assertIn("0.15 by default", site_pair.__doc__)

    def test_one_pair_is_not_a_verdict(self):
        with self.assertRaises(ValueError):
            site_pair.summarise([0.0])


class TheRunPlaysEverySeedInBothChairs(unittest.TestCase):
    def test_each_seed_is_played_twice_and_only_a_pass_exits_zero(self):
        asked = []

        def fake_play(command):
            seats = command[command.index("--shortcut-seats") + 1]
            turns = command[command.index("--turns") + 1]
            asked.append((command[command.index("--seed") + 1], seats, turns))
            return site_pair.parse_game(WON_BY_2)

        with tempfile.TemporaryDirectory() as scratch:
            binary = Path(scratch) / "civvis"
            binary.write_text("")
            out = Path(scratch) / "games.jsonl"
            with mock.patch.object(site_pair, "play", fake_play), \
                    mock.patch("sys.stdout", new_callable=io.StringIO) as printed:
                code = site_pair.main(["--binary", str(binary), "--shortcuts",
                                       "envelopes-across-own-moves", "--pairs", "3",
                                       "--seeds", "100", "--jobs", "1",
                                       "--out", str(out)])
            rows = out.read_text(encoding="utf-8").splitlines()
        self.assertEqual(code, 0, printed.getvalue())
        self.assertIn("bar, fixed before the first game", printed.getvalue())
        self.assertIn("NOT WORSE", printed.getvalue())
        games = [ask for ask in asked if ask[2] != "1"]
        self.assertEqual(sorted(games), sorted(
            (str(seed), seats, "400") for seed in (100, 101, 102)
            for seats in ("0,1", "2,3")))
        self.assertEqual(len(rows), 6)

    def test_a_stopped_run_is_read_back_and_a_half_played_pair_is_left_out(self):
        game = site_pair.parse_game(WON_BY_2)
        rows = [{"seed": seed, "shortcut_seats": chairs, **game}
                for seed in (1, 2, 3) for chairs in ([0, 1], [2, 3])]
        rows.append({"seed": 4, "shortcut_seats": [0, 1], **game})
        with tempfile.TemporaryDirectory() as scratch:
            played = Path(scratch) / "games.jsonl"
            played.write_text("".join(json.dumps(row) + "\n" for row in rows),
                              encoding="utf-8")
            out = io.StringIO()
            summary = site_pair.verdict_from(site_pair.read_rows(played),
                                             "envelopes-across-own-moves", 0.15, out=out)
        self.assertEqual(summary["pairs"], 3, "seed 4 has one chair played")
        self.assertIn("won 3 of 6 decided games", out.getvalue())


if __name__ == "__main__":
    unittest.main()
