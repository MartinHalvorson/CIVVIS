"""Tests for the generated README leader/strategy ranking."""

from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
ROOT = TOOLS.parent
sys.path.insert(0, str(TOOLS))

import update_readme_rankings as rankings  # noqa: E402


def civ_rating(rating: float, games: int = 6, rd: float = 80.0) -> dict:
    return {
        "rating": rating,
        "rd": rd,
        "vol": 0.06,
        "games": games,
        "wins": 1,
    }


def strategy(
    name: str,
    ratings: dict[str, dict[str, dict]],
    *,
    retired: bool = False,
    human: bool = False,
) -> dict:
    return {
        "name": name,
        "username": name.title(),
        "retired": retired,
        "human": human,
        "leader_elo": ratings,
    }


class OfficialRosterTests(unittest.TestCase):
    def test_roster_is_the_canonical_civ_vi_pool_not_the_expanded_pool(self):
        roster = rankings.official_roster(ROOT)

        self.assertEqual(len(roster), 50)
        self.assertEqual(len(set(roster)), 50)
        self.assertIn(("India", "Gandhi"), roster)
        self.assertIn(("Portugal", "João III"), roster)
        self.assertIn(("America", "Abraham Lincoln"), roster)
        self.assertNotIn("Denmark", {civilization for civilization, _ in roster})
        self.assertNotIn("Ashoka", {leader for _, leader in roster})

    def test_committed_readme_has_every_official_pair_once_in_elo_order(self):
        roster = rankings.official_roster(ROOT)
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        block = readme.split(rankings.START_MARKER, 1)[1].split(
            rankings.END_MARKER, 1
        )[0]
        parsed = []
        row_pattern = re.compile(
            r"^\| (\d+) \| ([^|]+) \| ([^|]+) \| [^|]+ \| "
            r"([+-]?\d+(?:\.\d+)?) \(±[^)]+\) \| [^|]+ \|$"
        )
        for line in block.splitlines():
            match = row_pattern.match(line)
            if match:
                parsed.append(
                    (
                        int(match.group(1)),
                        match.group(2).strip(),
                        match.group(3).strip(),
                        float(match.group(4)),
                    )
                )

        self.assertEqual([rank for rank, *_ in parsed], list(range(1, 51)))
        self.assertEqual(
            {(civilization, leader) for _, civilization, leader, _ in parsed},
            set(roster),
        )
        elos = [elo for *_, elo in parsed]
        self.assertEqual(elos, sorted(elos, reverse=True))


class RankingTests(unittest.TestCase):
    def test_best_active_exact_pair_wins_and_rows_sort_by_elo(self):
        league = {
            "round": 12,
            "strategies": [
                strategy(
                    "retired-top",
                    {"Leader One": {"Civ One": civ_rating(2500.0)}},
                    retired=True,
                ),
                strategy(
                    "human-top",
                    {"Leader One": {"Civ One": civ_rating(2400.0)}},
                    human=True,
                ),
                strategy(
                    "wrong-leader",
                    {"Other Leader": {"Civ One": civ_rating(2300.0)}},
                ),
                strategy(
                    "provisional-top",
                    {"Leader One": {"Civ One": civ_rating(2200.0, games=1)}},
                ),
                strategy(
                    "alpha",
                    {
                        "Leader One": {"Civ One": civ_rating(1600.0)},
                        "Leader Two": {"Civ Two": civ_rating(1700.0)},
                    },
                ),
                strategy(
                    "beta",
                    {
                        "Leader One": {"Civ One": civ_rating(1650.0)},
                        "Leader Two": {"Civ Two": civ_rating(1500.0)},
                    },
                ),
            ],
        }

        round_number, rows = rankings.rank_leaders(
            [("Civ One", "Leader One"), ("Civ Two", "Leader Two")],
            league,
            minimum_games=5,
        )

        self.assertEqual(round_number, 12)
        self.assertEqual(
            [(row.civilization, row.strategy, row.elo) for row in rows],
            [("Civ Two", "alpha", 1700.0), ("Civ One", "beta", 1650.0)],
        )

    def test_incomplete_league_is_rejected_instead_of_inventing_a_best(self):
        league = {
            "round": 3,
            "strategies": [
                strategy(
                    "alpha",
                    {"Leader One": {"Civ One": civ_rating(1600.0)}},
                )
            ],
        }

        with self.assertRaisesRegex(
            rankings.RankingError, "no active settled rating.*Leader Two / Civ Two"
        ):
            rankings.rank_leaders(
                [("Civ One", "Leader One"), ("Civ Two", "Leader Two")],
                league,
                minimum_games=5,
            )

    def test_generated_block_is_inserted_after_title_and_replaced_in_place(self):
        original = "# CIVVIS\n\nExisting introduction.\n"
        first_block = (
            f"{rankings.START_MARKER}\nfirst block\n{rankings.END_MARKER}"
        )
        second_block = (
            f"{rankings.START_MARKER}\nsecond block\n{rankings.END_MARKER}"
        )
        first = rankings.with_ranking(original, first_block)
        second = rankings.with_ranking(first, second_block)

        self.assertEqual(
            first,
            "# CIVVIS\n\n"
            f"{rankings.START_MARKER}\nfirst block\n{rankings.END_MARKER}\n\n"
            "Existing introduction.\n",
        )
        self.assertEqual(
            second,
            "# CIVVIS\n\n"
            f"{rankings.START_MARKER}\nsecond block\n{rankings.END_MARKER}\n\n"
            "Existing introduction.\n",
        )


if __name__ == "__main__":
    unittest.main()
