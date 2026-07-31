#!/usr/bin/env python3
"""Generate CIVVIS's complete root-level AI player Elo ranking.

``data/league/league.json`` is the committed, reproducible Glicko-2 league
snapshot.  Every observed ``(player strategy, civilization, leader)`` rating
in that ledger belongs in the ranking, including ratings held by a retired
strategy.  Strategies without a leader/civilization record are listed
separately with their global rating instead of being assigned a made-up pair
rating.

Examples:
    python3 tools/update_ai_player_elo_rankings.py
    python3 tools/update_ai_player_elo_rankings.py --check
    python3 tools/update_ai_player_elo_rankings.py --league /path/to/league.json
"""

from __future__ import annotations

import argparse
import json
import math
import sys
import textwrap
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LEAGUE = Path("data/league/league.json")
DEFAULT_OUTPUT = Path("AI_PLAYER_ELO_RANKINGS.md")


class RankingError(ValueError):
    """Raised when the league cannot produce an honest complete ranking."""


@dataclass(frozen=True)
class PairRating:
    """One exact strategy/civilization/leader rating from the league ledger."""

    player: str
    strategy: str
    civilization: str
    leader: str
    elo: float
    rd: float
    games: int
    wins: int
    status: str


@dataclass(frozen=True)
class StrategyWithoutPairRating:
    """A roster strategy that has not yet played an exact leader/civ pairing."""

    player: str
    strategy: str
    elo: float
    rd: float
    games: int
    wins: int
    status: str


def load_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise RankingError(f"cannot read {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise RankingError(f"invalid JSON in {path}: {error}") from error
    if not isinstance(value, dict):
        raise RankingError(f"expected a JSON object in {path}")
    return value


def nonempty_text(value: Any, description: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise RankingError(f"{description} is not a non-empty string")
    return value.strip()


def finite_number(value: Any, description: str, *, minimum: float | None = None) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise RankingError(f"{description} is not numeric")
    result = float(value)
    if not math.isfinite(result):
        raise RankingError(f"{description} is not finite")
    if minimum is not None and result < minimum:
        raise RankingError(f"{description} is below {minimum}")
    return result


def nonnegative_integer(value: Any, description: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise RankingError(f"{description} is not a non-negative integer")
    return value


def flag(value: Any, description: str) -> bool:
    if value is None:
        return False
    if not isinstance(value, bool):
        raise RankingError(f"{description} is not boolean")
    return value


def status_label(*, retired: bool, human: bool) -> str:
    if human and retired:
        return "human / retired"
    if human:
        return "human"
    if retired:
        return "retired"
    return "active"


def display_player(raw_player: Any, strategy: str, description: str) -> str:
    """Use a strategy ID when a legacy league record lacks a display player."""
    if raw_player is None or raw_player == "":
        return strategy
    return nonempty_text(raw_player, description)


def extract_rankings(
    league: dict[str, Any],
) -> tuple[int, list[PairRating], list[StrategyWithoutPairRating]]:
    """Validate and return every exact pair rating plus uncovered strategies."""
    round_number = nonnegative_integer(league.get("round"), "league round")
    strategies = league.get("strategies")
    if not isinstance(strategies, list):
        raise RankingError("league strategies is not a list")

    rows: list[PairRating] = []
    without_pair_ratings: list[StrategyWithoutPairRating] = []
    strategy_names: set[str] = set()
    identities: set[tuple[str, str, str]] = set()

    for index, raw_strategy in enumerate(strategies):
        if not isinstance(raw_strategy, dict):
            raise RankingError(f"league strategy {index} is not an object")
        strategy = nonempty_text(raw_strategy.get("name"), f"league strategy {index} name")
        if strategy in strategy_names:
            raise RankingError(f"league has duplicate strategy name {strategy!r}")
        strategy_names.add(strategy)
        player = display_player(
            raw_strategy.get("username"), strategy, f"league strategy {strategy!r} username"
        )
        retired = flag(raw_strategy.get("retired"), f"league strategy {strategy!r} retired")
        human = flag(raw_strategy.get("human"), f"league strategy {strategy!r} human")
        status = status_label(retired=retired, human=human)
        global_elo = finite_number(
            raw_strategy.get("rating"), f"league strategy {strategy!r} global rating"
        )
        global_rd = finite_number(
            raw_strategy.get("rd"), f"league strategy {strategy!r} global RD", minimum=0.0
        )
        global_games = nonnegative_integer(
            raw_strategy.get("games"), f"league strategy {strategy!r} global games"
        )
        global_wins = nonnegative_integer(
            raw_strategy.get("wins"), f"league strategy {strategy!r} global wins"
        )
        if global_wins > global_games:
            raise RankingError(f"league strategy {strategy!r} has more global wins than games")

        leader_elos = raw_strategy.get("leader_elo", {})
        if not isinstance(leader_elos, dict):
            raise RankingError(f"league strategy {strategy!r} has invalid leader_elo")
        row_count = 0
        for raw_leader, civilizations in leader_elos.items():
            leader = nonempty_text(raw_leader, f"league strategy {strategy!r} leader key")
            if not isinstance(civilizations, dict):
                raise RankingError(
                    f"league strategy {strategy!r} has invalid ratings for leader {leader!r}"
                )
            for raw_civilization, raw_rating in civilizations.items():
                civilization = nonempty_text(
                    raw_civilization,
                    f"league strategy {strategy!r} civilization key for {leader!r}",
                )
                if not isinstance(raw_rating, dict):
                    raise RankingError(
                        f"league strategy {strategy!r} has invalid {leader}/{civilization} rating"
                    )
                games = nonnegative_integer(
                    raw_rating.get("games"),
                    f"{strategy!r} {leader}/{civilization} games",
                )
                wins = nonnegative_integer(
                    raw_rating.get("wins"),
                    f"{strategy!r} {leader}/{civilization} wins",
                )
                if games < 1:
                    raise RankingError(
                        f"{strategy!r} {leader}/{civilization} has a pair rating without a game"
                    )
                if wins > games:
                    raise RankingError(
                        f"{strategy!r} {leader}/{civilization} has more wins than games"
                    )
                identity = (strategy, leader, civilization)
                if identity in identities:
                    raise RankingError(
                        f"league has duplicate pair rating for {strategy!r} {leader}/{civilization}"
                    )
                identities.add(identity)
                rows.append(
                    PairRating(
                        player=player,
                        strategy=strategy,
                        civilization=civilization,
                        leader=leader,
                        elo=finite_number(
                            raw_rating.get("rating"),
                            f"{strategy!r} {leader}/{civilization} rating",
                        ),
                        rd=finite_number(
                            raw_rating.get("rd"),
                            f"{strategy!r} {leader}/{civilization} RD",
                            minimum=0.0,
                        ),
                        games=games,
                        wins=wins,
                        status=status,
                    )
                )
                row_count += 1
        if row_count == 0:
            without_pair_ratings.append(
                StrategyWithoutPairRating(
                    player=player,
                    strategy=strategy,
                    elo=global_elo,
                    rd=global_rd,
                    games=global_games,
                    wins=global_wins,
                    status=status,
                )
            )

    if not rows:
        raise RankingError(f"league round {round_number} has no leader/civilization ratings")

    rows.sort(
        key=lambda row: (
            -row.elo,
            row.player.casefold(),
            row.strategy.casefold(),
            row.civilization.casefold(),
            row.leader.casefold(),
        )
    )
    without_pair_ratings.sort(
        key=lambda row: (-row.elo, row.player.casefold(), row.strategy.casefold())
    )
    return round_number, rows, without_pair_ratings


def markdown_text(value: str) -> str:
    return value.replace("\\", "\\\\").replace("|", "\\|").replace("\n", " ")


def inline_code(value: str) -> str:
    fence = "`" if "`" not in value else "``"
    return f"{fence}{value}{fence}"


def player_strategy_label(player: str, strategy: str) -> str:
    return f"{markdown_text(player)} ({inline_code(strategy)})"


def display_source(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT.resolve()))
    except ValueError:
        return str(path.resolve())


def render_document(
    round_number: int,
    rows: list[PairRating],
    without_pair_ratings: list[StrategyWithoutPairRating],
    source: Path,
) -> str:
    rated_strategies = len({row.strategy for row in rows})
    total_strategies = rated_strategies + len(without_pair_ratings)
    lines = [
        "# AI Player Elo Rankings",
        "",
        textwrap.fill(
            f"League round **{round_number}**, generated from `{display_source(source)}`. "
            f"This complete table contains every one of the **{len(rows)}** recorded "
            "leader/civilization-specific Glicko-2 ratings. It includes active, retired, "
            "and human strategies whenever a rating record exists, and is sorted by exact "
            "pair Elo descending.",
            width=88,
        ),
        "",
        textwrap.fill(
            f"The league roster has **{total_strategies}** named strategies; "
            f"**{rated_strategies}** have at least one exact pair rating. The remaining "
            f"**{len(without_pair_ratings)}** are listed separately with their global rating "
            "because CIVVIS has not yet recorded an Elo for a specific civilization/leader pair.",
            width=88,
        ),
        "",
        "Refresh this document after updating the committed league snapshot:",
        "",
        "`python3 tools/update_ai_player_elo_rankings.py`",
        "",
        "Use `--check` to verify that this generated file is current.",
        "",
        "| Rank | Elo | Player (strategy) — civilization — leader | RD | Games | Wins | Status |",
        "|---:|---:|---|---:|---:|---:|---|",
    ]
    for rank, row in enumerate(rows, start=1):
        identity = (
            f"{player_strategy_label(row.player, row.strategy)} — "
            f"{markdown_text(row.civilization)} — {markdown_text(row.leader)}"
        )
        lines.append(
            f"| {rank} | {row.elo:.1f} | {identity} | ±{row.rd:.1f} | "
            f"{row.games} | {row.wins} | {row.status} |"
        )

    lines.extend(
        [
            "",
            "## Strategies without a civilization/leader Elo",
            "",
        ]
    )
    if without_pair_ratings:
        lines.extend(
            [
                textwrap.fill(
                    "These roster strategies have no `leader_elo` entries. Their global "
                    "Glicko-2 rating is shown in descending order, but it is deliberately not "
                    "mixed into the exact civilization/leader ranking above.",
                    width=88,
                ),
                "",
                "| Global Elo | Player (strategy) | RD | Games | Wins | Status |",
                "|---:|---|---:|---:|---:|---|",
            ]
        )
        for row in without_pair_ratings:
            lines.append(
                f"| {row.elo:.1f} | {player_strategy_label(row.player, row.strategy)} | "
                f"±{row.rd:.1f} | {row.games} | {row.wins} | {row.status} |"
            )
    else:
        lines.append("Every roster strategy has at least one civilization/leader rating.")
    return "\n".join(lines) + "\n"


def repo_path(value: str) -> Path:
    path = Path(value).expanduser()
    return path.resolve() if path.is_absolute() else (ROOT / path).resolve()


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        raise RankingError(f"cannot read {path}: {error}") from error


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--league",
        default=str(DEFAULT_LEAGUE),
        help="league.json to read (repo-relative; default: committed snapshot)",
    )
    parser.add_argument(
        "--output",
        default=str(DEFAULT_OUTPUT),
        help="Markdown artifact to write or check (repo-relative)",
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--check", action="store_true", help="fail if the artifact is stale"
    )
    mode.add_argument(
        "--stdout", action="store_true", help="print the artifact without writing"
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        league_path = repo_path(args.league)
        output_path = repo_path(args.output)
        round_number, rows, without_pair_ratings = extract_rankings(load_object(league_path))
        expected = render_document(round_number, rows, without_pair_ratings, league_path)
        if args.stdout:
            print(expected, end="")
            return 0
        if args.check:
            if read_text(output_path) != expected:
                print(
                    f"{output_path} is stale for league round {round_number}; run "
                    "tools/update_ai_player_elo_rankings.py without --check",
                    file=sys.stderr,
                )
                return 1
            print(
                f"{output_path} is current for all {len(rows)} player/civilization/leader "
                f"ratings at league round {round_number}"
            )
            return 0
        output_path.write_text(expected, encoding="utf-8")
        print(
            f"updated {output_path} with {len(rows)} player/civilization/leader ratings "
            f"from league round {round_number}"
        )
        return 0
    except RankingError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
