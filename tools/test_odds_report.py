"""Outcome scoring must not bless leakage, duplicate games or invalid odds."""
import copy
import gzip
import json
import math
from pathlib import Path
import tempfile
import unittest

from odds_report import at_phase, interval, load, reliability, report, score


def fixture():
    return {"schema": "civvis-odds-v1", "seed": 1, "players": 2,
            "max_turns": 100, "end_turn": 90, "winners": [1],
            "samples": [{"turn": turn, "terminal": False,
                         "seats": [{"pid": 0, "start": .5, "now": .2},
                                   {"pid": 1, "start": .5, "now": .8}]}
                        for turn in (10, 50, 90)]}


class OddsReportTests(unittest.TestCase):
    def read(self, *games):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "run.jsonl"
            path.write_text("".join(json.dumps(game) + "\n" for game in games))
            return load([path])

    def test_scores_actual_winner_and_all_seats(self):
        brier, loss, correct = score([.2, .8], [0, 1], 1)
        self.assertAlmostEqual(brier, .04)
        self.assertAlmostEqual(loss, -math.log(.8))
        self.assertEqual(correct, 1)
        brier, loss, correct = score([.2, .8], [0, 1], 0)
        self.assertAlmostEqual(brier, .64)
        self.assertAlmostEqual(loss, -math.log(.2))
        self.assertEqual(correct, 0)

    def test_phase_never_looks_ahead_or_carries_a_finished_game(self):
        game = fixture()
        self.assertEqual(at_phase(game, .25)["turn"], 10)
        self.assertEqual(at_phase(game, .5)["turn"], 50)
        self.assertIsNone(at_phase(game, .95))
        # A turn-opening forecast can precede a result within that turn.
        self.assertEqual(at_phase(game, .9)["turn"], 90)

    def test_terminal_snapshots_are_rejected(self):
        game = fixture()
        game["samples"][-1]["terminal"] = True
        with self.assertRaisesRegex(ValueError, "terminal"):
            self.read(game)

    def test_missing_results_and_repeated_maps_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "duplicate seed"):
            self.read(fixture(), fixture())
        game = fixture()
        game["winners"] = []
        with self.assertRaisesRegex(ValueError, "terminal outcome"):
            self.read(game)

    def test_invalid_probabilities_and_incomplete_tables_are_rejected(self):
        for values in ([.1, .1], [float("nan"), .5], [float("inf"), .5], [-.1, 1.1]):
            game = fixture()
            for seat, value in zip(game["samples"][0]["seats"], values):
                seat["now"] = value
            with self.assertRaises(ValueError):
                self.read(game)
        game = fixture()
        game["samples"][0]["seats"].pop()
        with self.assertRaisesRegex(ValueError, "per major"):
            self.read(game)

    def test_unsorted_or_repeated_snapshots_are_rejected(self):
        game = fixture()
        game["samples"].append(copy.deepcopy(game["samples"][0]))
        with self.assertRaisesRegex(ValueError, "unique and increasing"):
            self.read(game)

    def test_report_compares_on_identical_games(self):
        games = self.read(fixture())
        result = report(games)
        self.assertEqual(result["games"], 1)
        metrics = result["phases"][0]["metrics"]
        self.assertAlmostEqual(metrics["brier"]["now"], .04)
        self.assertAlmostEqual(metrics["brier"]["start"], .25)
        self.assertGreater(metrics["log_loss"]["difference"], 0)
        self.assertEqual(interval([.25, .25], 100), [.25, .25])

    def test_reliability_keeps_zero_and_one_in_valid_bins(self):
        bins = reliability([(0., 1.), (1., 0.)])
        self.assertEqual([row["low"] for row in bins], [0., .9])
        self.assertEqual([row["observed"] for row in bins], [1., 0.])

    def test_actual_winner_assigned_zero_is_visible(self):
        game = fixture()
        for sample in game["samples"]:
            sample["seats"][0]["now"] = 1.
            sample["seats"][1]["now"] = 0.
        result = report(self.read(game), phases=(.5,))
        self.assertEqual(result["phases"][0]["zero_probability_winners"], {"now": 1, "start": 0})

    def test_archived_evidence_reads_the_same_as_plain_jsonl(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "run.jsonl.gz"
            with gzip.open(path, "wt") as stream:
                stream.write(json.dumps(fixture()) + "\n")
            self.assertEqual(load([path]), [fixture()])

    def test_empty_forecast_series_is_not_evidence(self):
        game = fixture()
        game["samples"] = []
        with self.assertRaisesRegex(ValueError, "at least one"):
            self.read(game)

    def test_parallel_completion_order_does_not_change_the_report(self):
        first, second = fixture(), fixture()
        second["seed"] = 2
        self.assertEqual(self.read(first, second), self.read(second, first))


if __name__ == "__main__":
    unittest.main()
