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


def civ_rating(
    rating: float, games: int = 6, rd: float = 80.0, wins: int = 1
) -> dict:
    return {
        "rating": rating,
        "rd": rd,
        "vol": 0.06,
        "games": games,
        "wins": wins,
    }


# The bound the README prints must be the bound `src/league.rs` selects on. These
# are the shared golden values; the identical table is asserted in Rust by
# `civ_rating_strength_bound_matches_the_readme_tool` in `src/league.rs`. If either
# implementation's formula drifts, exactly one side of this pair starts failing.
GOLDEN_BOUNDS = [
    # (wins, games, lower, upper)
    (0, 5, 0.0, 0.43449149475208104),
    (1, 6, 0.030052585871730285, 0.56350943656364605),
    (3, 27, 0.038519647894987241, 0.28058182543972948),
    (161, 314, 0.45763314963452978, 0.56753662046479014),
    (230, 625, 0.33110414128153626, 0.40650863751681299),
    (8, 43, 0.097416355801059812, 0.32617293235305961),
]


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

    def test_committed_readme_accounts_for_every_official_pair_exactly_once(self):
        """Every pair is either ranked or explicitly reported as unresolved.

        The old version of this test asserted 50 ranked rows in descending Elo. It
        passed on a table that ranked on the wrong statistic, because it only ever
        checked that the ordering was *self-consistent* — never that the number it
        ordered on was the one that answers the question.
        """
        roster = rankings.official_roster(ROOT)
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        block = readme.split(rankings.START_MARKER, 1)[1].split(
            rankings.END_MARKER, 1
        )[0]
        ranked = re.compile(
            r"^\| (\d+) \| ([^|]+) \| ([^|]+) \| [^|]+ \| \d+/\d+ \| (\d\.\d+) \|$"
        )
        covered = re.compile(
            r"^\| ([^|]+) \| ([^|]+) \| (\d+) \| \d+ \| [^|]+ \| [^|]+ \|$"
        )
        ranked_rows, covered_rows = [], []
        for line in block.splitlines():
            match = ranked.match(line)
            if match:
                ranked_rows.append(
                    (
                        int(match.group(1)),
                        match.group(2).strip(),
                        match.group(3).strip(),
                        float(match.group(4)),
                    )
                )
                continue
            match = covered.match(line)
            if match:
                covered_rows.append((match.group(1).strip(), match.group(2).strip()))

        # Pairs nobody has rated are named in prose rather than given a table row,
        # but they must still be named — a gap may not hide behind a count.
        # The list is wrapped prose, so a pair can straddle a line break: match on
        # the block with whitespace collapsed rather than line by line.
        flat = " ".join(block.split())
        unplayed = [
            (civilization.strip(), leader.strip())
            for civilization, leader in re.findall(r"([^,:.]+?) \(([^)]+)\)", flat)
        ]

        pairs = (
            [(civ, leader) for _, civ, leader, _ in ranked_rows]
            + covered_rows
            + unplayed
        )
        self.assertEqual(len(pairs), len(set(pairs)), "a pair is listed twice")
        self.assertEqual(set(pairs), set(roster))
        self.assertEqual(
            [rank for rank, *_ in ranked_rows], list(range(1, len(ranked_rows) + 1))
        )
        bounds = [bound for *_, bound in ranked_rows]
        self.assertEqual(bounds, sorted(bounds, reverse=True))

    def test_python_win_bound_matches_the_rust_golden_values(self):
        """The printed bound is the bound the league selects on, to the digit."""
        z = rankings.selection_z(ROOT)
        self.assertEqual(z, 1.96)
        for wins, games, lower, upper in GOLDEN_BOUNDS:
            self.assertAlmostEqual(
                rankings.win_confidence(wins, games, z, upper=False), lower, places=15
            )
            self.assertAlmostEqual(
                rankings.win_confidence(wins, games, z, upper=True), upper, places=15
            )

    def test_selection_z_is_read_from_rust_rather_than_copied(self):
        """Drift-proofing: the constant has exactly one definition, and it is Rust's."""
        source = (ROOT / "src/league.rs").read_text(encoding="utf-8")
        self.assertEqual(
            len(re.findall(r"^const\s+SELECTION_Z\s*:", source, flags=re.MULTILINE)), 1
        )


class RankingTests(unittest.TestCase):
    def test_equal_evidence_resolves_nothing_rather_than_falling_back_to_elo(self):
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

        round_number, rows, coverage = rankings.rank_leaders(
            [("Civ One", "Leader One"), ("Civ Two", "Leader Two")],
            league,
            minimum_games=5,
            z=1.96,
        )

        # Every entrant here wins 1 of 6, so no bound separates from any other:
        # equal evidence must resolve nothing rather than fall back to Elo order.
        self.assertEqual(round_number, 12)
        self.assertEqual(rows, [])
        self.assertEqual(
            [(row.civilization, row.candidates) for row in coverage],
            [("Civ One", 2), ("Civ Two", 2)],
        )

    def test_a_clear_winner_is_ranked_and_a_close_one_is_only_covered(self):
        """The separation bar, which is the whole point of the change.

        `runaway` wins 90 of 100; `plodder` wins 5 of 100 — the bound separates.
        In the second pair both win about half of 40, which does not separate even
        though one has the higher rate, so it is reported rather than crowned.
        """
        league = {
            "round": 7,
            "strategies": [
                strategy(
                    "runaway",
                    {
                        "Leader One": {"Civ One": civ_rating(1500.0, 100, wins=90)},
                        "Leader Two": {"Civ Two": civ_rating(1500.0, 40, wins=22)},
                    },
                ),
                strategy(
                    "plodder",
                    {
                        "Leader One": {"Civ One": civ_rating(2900.0, 100, wins=5)},
                        "Leader Two": {"Civ Two": civ_rating(1500.0, 40, wins=18)},
                    },
                ),
            ],
        }

        _, rows, coverage = rankings.rank_leaders(
            [("Civ One", "Leader One"), ("Civ Two", "Leader Two")],
            league,
            minimum_games=5,
            z=1.96,
        )

        # `plodder` carries by far the higher placement Elo on Civ One (2900 v 1500)
        # and still loses, because it wins 5 games in 100. That is the inversion.
        self.assertEqual([(r.civilization, r.strategy) for r in rows], [("Civ One", "runaway")])
        self.assertEqual([(r.civilization, r.leader_name) for r in coverage], [("Civ Two", "runaway")])

    def test_an_incomplete_league_reports_coverage_instead_of_failing(self):
        """A fresh clone must still produce an honest table.

        This previously raised, so the documented refresh command exited 2 on any
        checkout that had not been running the league — which is why the shipped
        table was not reproducible from the repository at all.
        """
        league = {
            "round": 3,
            "strategies": [
                strategy(
                    "alpha",
                    {"Leader One": {"Civ One": civ_rating(1600.0)}},
                )
            ],
        }

        _, rows, coverage = rankings.rank_leaders(
            [("Civ One", "Leader One"), ("Civ Two", "Leader Two")],
            league,
            minimum_games=5,
            z=1.96,
        )

        self.assertEqual(rows, [])
        missing = {row.civilization: row for row in coverage}
        self.assertEqual(missing["Civ Two"].candidates, 0)
        self.assertEqual(missing["Civ Two"].leader_record, "no qualifying record")

    def test_wins_exceeding_games_is_rejected(self):
        league = {
            "round": 3,
            "strategies": [
                strategy("alpha", {"Leader One": {"Civ One": civ_rating(1600.0, 6, wins=7)}})
            ],
        }

        with self.assertRaisesRegex(rankings.RankingError, "records 7 wins in 6 games"):
            rankings.rank_leaders(
                [("Civ One", "Leader One")], league, minimum_games=5, z=1.96
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
