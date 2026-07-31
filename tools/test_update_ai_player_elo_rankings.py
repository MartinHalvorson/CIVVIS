"""Tests for the complete generated AI player Elo ranking."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
ROOT = TOOLS.parent
sys.path.insert(0, str(TOOLS))

import update_ai_player_elo_rankings as rankings  # noqa: E402


def pair_rating(
    rating: float,
    *,
    games: int = 6,
    wins: int = 1,
    rd: float = 80.0,
) -> dict:
    return {
        "rating": rating,
        "rd": rd,
        "vol": 0.06,
        "games": games,
        "wins": wins,
    }


def strategy(
    name: str,
    leader_elo: dict[str, dict[str, dict]],
    *,
    username: str | None = None,
    rating: float = 1500.0,
    rd: float = 350.0,
    games: int = 0,
    wins: int = 0,
    retired: bool = False,
    human: bool = False,
) -> dict:
    return {
        "name": name,
        "username": username if username is not None else name.title(),
        "rating": rating,
        "rd": rd,
        "vol": 0.06,
        "games": games,
        "wins": wins,
        "retired": retired,
        "human": human,
        "leader_elo": leader_elo,
    }


class RankingTests(unittest.TestCase):
    def test_every_pair_is_kept_sorted_and_retired_entries_are_not_dropped(self):
        league = {
            "round": 9,
            "strategies": [
                strategy(
                    "alpha",
                    {"Leader A": {"Civ A": pair_rating(1700.0)}},
                    username="Alpha Player",
                ),
                strategy(
                    "retired-beta",
                    {"Leader B": {"Civ B": pair_rating(2100.0)}},
                    username="Beta Player",
                    retired=True,
                ),
                strategy(
                    "unseated",
                    {},
                    username="Unseated Player",
                    rating=1950.0,
                    games=4,
                    wins=3,
                ),
            ],
        }

        round_number, rows, without_pair_ratings = rankings.extract_rankings(league)

        self.assertEqual(round_number, 9)
        self.assertEqual(
            [(row.elo, row.strategy, row.status) for row in rows],
            [(2100.0, "retired-beta", "retired"), (1700.0, "alpha", "active")],
        )
        self.assertEqual(
            [(row.elo, row.strategy, row.status) for row in without_pair_ratings],
            [(1950.0, "unseated", "active")],
        )

        document = rankings.render_document(
            round_number, rows, without_pair_ratings, ROOT / "data/league/league.json"
        )
        self.assertIn(
            "Beta Player (`retired-beta`) — Civ B — Leader B", document
        )
        self.assertIn("Strategies without a civilization/leader Elo", document)
        self.assertIn("Unseated Player (`unseated`)", document)

    def test_pair_rating_without_a_game_is_rejected(self):
        league = {
            "round": 2,
            "strategies": [
                strategy(
                    "invalid",
                    {"Leader A": {"Civ A": pair_rating(1600.0, games=0)}},
                )
            ],
        }

        with self.assertRaisesRegex(rankings.RankingError, "without a game"):
            rankings.extract_rankings(league)

    def test_committed_artifact_covers_the_entire_committed_roster(self):
        league = rankings.load_object(ROOT / "data/league/league.json")
        round_number, rows, without_pair_ratings = rankings.extract_rankings(league)
        artifact = ROOT / "AI_PLAYER_ELO_RANKINGS.md"
        source_pair_count = sum(
            len(civilizations)
            for raw_strategy in league["strategies"]
            for civilizations in raw_strategy["leader_elo"].values()
        )

        self.assertGreaterEqual(round_number, 0)
        self.assertEqual(len(rows), source_pair_count)
        self.assertGreater(len(rows), 0)
        self.assertEqual([row.elo for row in rows], sorted((row.elo for row in rows), reverse=True))
        self.assertEqual(
            len({row.strategy for row in rows} | {row.strategy for row in without_pair_ratings}),
            len(league["strategies"]),
        )
        self.assertEqual(
            artifact.read_text(encoding="utf-8"),
            rankings.render_document(round_number, rows, without_pair_ratings, ROOT / "data/league/league.json"),
        )


if __name__ == "__main__":
    unittest.main()
