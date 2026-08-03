"""Tests for the complete generated AI player Elo ranking."""

from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
ROOT = TOOLS.parent
sys.path.insert(0, str(TOOLS))

import update_ai_player_elo_rankings as rankings  # noqa: E402


def strategy(
    name: str,
    *,
    username: str | None = None,
    rating: float = 1500.0,
    rd: float = 80.0,
    table_games: int | None = 20,
    table_wins: int = 2,
    last_played: str | None = "2026-08-02",
    retired: bool = False,
    human: bool = False,
    league_only: bool = False,
    target: str | None = None,
) -> dict:
    leader_elo = {}
    if last_played is not None:
        leader_elo = {
            "Leader A": {
                "Civ A": {
                    "rating": rating,
                    "rd": rd,
                    "vol": 0.06,
                    "games": 1,
                    "wins": 0,
                    "last_played": last_played,
                }
            }
        }
    return {
        "name": name,
        "username": username if username is not None else name.title(),
        "kind": {"Advanced": {"weights": {}, "target": target}},
        "rating": rating,
        "rd": rd,
        "vol": 0.06,
        "games": table_games or 0,
        "wins": table_wins if table_games is not None else 0,
        "wins_by_table_size": (
            {"8": {"games": table_games, "wins": table_wins}}
            if table_games is not None
            else {}
        ),
        "retired": retired,
        "human": human,
        "league_only": league_only,
        "leader_elo": leader_elo,
    }


class RankingTests(unittest.TestCase):
    def test_only_current_live_strategies_are_ranked_on_table_win_evidence(self):
        league = {
            "round": 9,
            "strategies": [
                strategy(
                    "placement-spike",
                    username="Placement Spike",
                    rating=2200.0,
                    table_games=23,
                    table_wins=0,
                ),
                strategy(
                    "proven-winner",
                    username="Proven Winner",
                    rating=1400.0,
                    table_games=100,
                    table_wins=30,
                    target="religious",
                ),
                strategy(
                    "retired-winner",
                    table_games=100,
                    table_wins=90,
                    retired=True,
                ),
                strategy(
                    "human-winner", table_games=100, table_wins=90, human=True
                ),
                strategy(
                    "offline-winner",
                    table_games=100,
                    table_wins=90,
                    league_only=True,
                ),
            ],
        }

        round_number, rows, without_table_evidence, excluded = rankings.extract_rankings(
            league
        )

        self.assertEqual(round_number, 9)
        self.assertEqual(
            [row.strategy for row in rows], ["proven-winner", "placement-spike"]
        )
        self.assertEqual(rows[0].role, "religious specialist")
        self.assertEqual(without_table_evidence, [])
        self.assertEqual(excluded, 3)

        document = rankings.render_document(
            round_number,
            rows,
            without_table_evidence,
            excluded,
            ROOT / "data/league/league.json",
        )
        self.assertIn("Proven Winner (`proven-winner`)", document)
        self.assertNotIn("retired-winner", document)
        self.assertNotIn("human-winner", document)
        self.assertNotIn("offline-winner", document)
        self.assertNotIn("Civ A", document)
        self.assertIn("Placement Elo is retained only as", document)

    def test_current_strategy_without_table_evidence_is_not_mixed_into_ranking(self):
        league = {
            "round": 2,
            "strategies": [strategy("rated"), strategy("provisional", table_games=None)],
        }

        _, rows, without_table_evidence, excluded = rankings.extract_rankings(league)

        self.assertEqual([row.strategy for row in rows], ["rated"])
        self.assertEqual(
            [row.strategy for row in without_table_evidence], ["provisional"]
        )
        self.assertEqual(excluded, 0)

    def test_last_played_must_be_a_real_iso_calendar_date(self):
        league = {
            "round": 2,
            "strategies": [strategy("invalid-date", last_played="2026-02-29")],
        }

        with self.assertRaisesRegex(rankings.RankingError, "real calendar date"):
            rankings.extract_rankings(league)

    def test_table_wins_cannot_exceed_games(self):
        invalid = strategy("invalid-record", table_games=3, table_wins=4)
        invalid["wins"] = 0
        league = {
            "round": 2,
            "strategies": [invalid],
        }

        with self.assertRaisesRegex(rankings.RankingError, "more 8-player wins"):
            rankings.extract_rankings(league)

    def test_committed_artifact_contains_only_current_eight_player_ranking(self):
        league = rankings.load_object(ROOT / "data/league/league.json")
        round_number, rows, without_table_evidence, excluded = rankings.extract_rankings(
            league
        )
        artifact = ROOT / "AI_PLAYER_ELO_RANKINGS.md"
        current = {
            raw["name"]
            for raw in league["strategies"]
            if not raw.get("retired")
            and not raw.get("human")
            and not raw.get("league_only")
        }

        self.assertGreaterEqual(round_number, 0)
        self.assertEqual(
            {row.strategy for row in rows}
            | {row.strategy for row in without_table_evidence},
            current,
        )
        self.assertGreater(len(rows), 0)
        self.assertTrue(
            all(row.last_played is not None for row in rows),
            "every current ranked strategy retains a last-played date",
        )
        self.assertEqual(
            [row.win_bound for row in rows],
            sorted((row.win_bound for row in rows), reverse=True),
        )
        self.assertEqual(rows[0].strategy, "g24-26")
        self.assertEqual((rows[0].wins, rows[0].games), (189, 1069))
        former_page_leader = next(row for row in rows if row.strategy == "g56-48")
        self.assertEqual(
            (rows.index(former_page_leader) + 1, former_page_leader.wins, former_page_leader.games),
            (39, 0, 23),
        )
        self.assertEqual(excluded, 1)
        current_artifact = artifact.read_text(encoding="utf-8")
        self.assertNotIn("WildCard9 (`g56-48`) — Rome — Trajan", current_artifact)
        self.assertNotIn("active, retired, and human", current_artifact)
        self.assertEqual(
            current_artifact,
            rankings.render_document(
                round_number,
                rows,
                without_table_evidence,
                excluded,
                ROOT / "data/league/league.json",
            ),
        )

    def test_wilson_confidence_constant_matches_league_selection(self):
        league_source = (ROOT / "src/league.rs").read_text(encoding="utf-8")
        match = re.search(r"const SELECTION_Z: f64 = ([0-9.]+);", league_source)

        self.assertIsNotNone(match)
        self.assertEqual(float(match.group(1)), rankings.SELECTION_Z)


if __name__ == "__main__":
    unittest.main()
