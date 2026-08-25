"""Regression tests for the one supported continuous-screen status reader."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import continuous_screen_status as status


def header(first: int = 100, games: int = 1, players: int = 6) -> dict:
    return {
        "kind": "header",
        "players": players,
        "all_seats": True,
        "design": "independent",
        "batch": {
            "target_games": games,
            "target_seats": games * players,
            "seed_first": first,
            "seed_last": first + games - 1,
        },
    }


def game(seed: int, ordinal: int = 0, players: int = 6) -> list[dict]:
    winner = players - 1
    return [
        {
            "kind": "game",
            "seed": seed,
            "arm": 0,
            "game": ordinal,
            "seat": seat,
            "winner": winner,
            "win": seat == winner,
        }
        for seat in range(players)
    ]


def write(path: Path, records: list[dict]) -> None:
    path.write_text("\n".join(json.dumps(record) for record in records) + "\n",
                    encoding="utf-8")


class ContinuousScreenStatus(unittest.TestCase):
    def status(self, records: list[dict]) -> dict:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "rows.jsonl"
            write(path, records)
            return status.summarize(path)

    def test_six_seat_records_are_one_game_not_six(self):
        summary = self.status([header(), *game(100)])
        self.assertEqual(summary["complete_games"], 1)
        self.assertEqual(summary["complete_seats"], 6)
        self.assertEqual(summary["wins"], 1)
        self.assertEqual(summary["records"], 7)
        self.assertEqual(summary["header_records"], 1)
        self.assertEqual(summary["seat_records"], 6)
        self.assertNotEqual(summary["records"], summary["complete_games"],
                            "physical JSONL records are never reported as games")

    def test_segments_sum_targets_but_group_games_by_seed(self):
        summary = self.status([
            header(100), *game(100, 0),
            header(200), *game(200, 0),
        ])
        self.assertEqual(summary["complete_games"], 2)
        self.assertEqual(summary["complete_seats"], 12)
        self.assertEqual(summary["target_games"], 2)
        self.assertEqual(summary["target_seats"], 12)
        self.assertEqual(summary["played_seed_window"], [100, 200])

    def test_incomplete_game_fails_instead_of_becoming_a_smaller_count(self):
        records = [header(), *game(100)[:-1]]
        with self.assertRaisesRegex(status.LedgerError, "has 5 seat rows"):
            self.status(records)

    def test_duplicate_seat_fails_instead_of_double_counting(self):
        records = [header(), *game(100)]
        records[-1]["seat"] = 4
        with self.assertRaisesRegex(status.LedgerError, "has seats"):
            self.status(records)

    def test_multiple_or_missing_winners_fail(self):
        records = [header(), *game(100)]
        records[1]["win"] = True
        with self.assertRaisesRegex(status.LedgerError, "winning seats"):
            self.status(records)

    def test_analyzer_must_agree_with_the_validated_ledger(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            rows = root / "rows.jsonl"
            analysis = root / "analysis.json"
            write(rows, [header(), *game(100)])
            summary = status.summarize(rows)
            analysis.write_text(json.dumps({
                "kind": "gene_screen_analysis",
                "games": 1,
                "seats": 6,
                "batch": {
                    "complete_games": 1,
                    "complete_seats": 6,
                    "target_games": 1,
                    "target_seats": 6,
                },
            }), encoding="utf-8")
            status.validate_analysis(summary, analysis)
            bad = json.loads(analysis.read_text(encoding="utf-8"))
            bad["games"] = 7
            analysis.write_text(json.dumps(bad), encoding="utf-8")
            with self.assertRaisesRegex(status.LedgerError, "games=7"):
                status.validate_analysis(summary, analysis)


if __name__ == "__main__":
    unittest.main()
