#!/usr/bin/env python3
"""Generate CIVVIS's current, evidence-backed AI strategy ranking.

``data/league/league.json`` is the committed, reproducible league snapshot.
The public ranking includes only strategies currently eligible for live games
and orders them by the league's conservative outright-win objective at the
requested table size.  Historical exact leader/civilization ratings remain in
the source ledger; they are not promoted into a misleading current leaderboard.

Examples:
    python3 tools/update_ai_player_elo_rankings.py
    python3 tools/update_ai_player_elo_rankings.py --check
    python3 tools/update_ai_player_elo_rankings.py --league /path/to/league.json
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
import textwrap
from dataclasses import dataclass
from datetime import date
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LEAGUE = Path("data/league/league.json")
DEFAULT_OUTPUT = Path("AI_PLAYER_ELO_RANKINGS.md")
DEFAULT_TABLE_SIZE = 8
SELECTION_Z = 1.96


class RankingError(ValueError):
    """Raised when the league cannot produce an honest complete ranking."""


@dataclass(frozen=True)
class CurrentStrategyRank:
    """One live-eligible strategy with evidence at the requested table size."""

    player: str
    strategy: str
    role: str
    elo: float
    rd: float
    games: int
    wins: int
    win_bound: float
    last_played: str | None


@dataclass(frozen=True)
class StrategyWithoutTableEvidence:
    """A live-eligible strategy that has not played the requested table size."""

    player: str
    strategy: str
    role: str
    elo: float
    rd: float
    last_played: str | None


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


def optional_iso_date(value: Any, description: str) -> str | None:
    """Validate an optional UTC calendar date kept with exact pair evidence."""
    if value is None:
        return None
    result = nonempty_text(value, description)
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", result):
        raise RankingError(f"{description} is not YYYY-MM-DD")
    try:
        date.fromisoformat(result)
    except ValueError as error:
        raise RankingError(f"{description} is not a real calendar date") from error
    return result


def flag(value: Any, description: str) -> bool:
    if value is None:
        return False
    if not isinstance(value, bool):
        raise RankingError(f"{description} is not boolean")
    return value


def display_player(raw_player: Any, strategy: str, description: str) -> str:
    """Use a strategy ID when a legacy league record lacks a display player."""
    if raw_player is None or raw_player == "":
        return strategy
    return nonempty_text(raw_player, description)


def strategy_role(value: Any, description: str) -> str:
    if not isinstance(value, dict) or len(value) != 1:
        raise RankingError(f"{description} is not a recognized strategy kind")
    if "Builtin" in value:
        builtin = value["Builtin"]
        if not isinstance(builtin, dict):
            raise RankingError(f"{description} Builtin value is not an object")
        ai = nonempty_text(builtin.get("ai"), f"{description} builtin AI")
        return f"builtin {ai}"
    if "Advanced" in value:
        advanced = value["Advanced"]
        if not isinstance(advanced, dict):
            raise RankingError(f"{description} Advanced value is not an object")
        target = advanced.get("target")
        if target is None:
            return "generalist"
        return f"{nonempty_text(target, f'{description} target')} specialist"
    raise RankingError(f"{description} is not a recognized strategy kind")


def win_confidence(wins: int, games: int, *, upper: bool = False) -> float:
    """Mirror the league's Wilson confidence bound on outright table wins."""
    if games == 0:
        return 1.0 if upper else 0.0
    n = float(games)
    p = wins / n
    z2 = SELECTION_Z * SELECTION_Z
    centre = p + z2 / (2.0 * n)
    margin = SELECTION_Z * math.sqrt(
        p * (1.0 - p) / n + z2 / (4.0 * n * n)
    )
    return min(
        1.0,
        max(0.0, (centre + (margin if upper else -margin)) / (1.0 + z2 / n)),
    )


def latest_played(value: Any, description: str) -> str | None:
    if not isinstance(value, dict):
        raise RankingError(f"{description} leader_elo is not an object")
    dates: list[str] = []
    for raw_leader, civilizations in value.items():
        leader = nonempty_text(raw_leader, f"{description} leader key")
        if not isinstance(civilizations, dict):
            raise RankingError(f"{description} ratings for {leader!r} are not an object")
        for raw_civilization, raw_rating in civilizations.items():
            civilization = nonempty_text(
                raw_civilization, f"{description} civilization key for {leader!r}"
            )
            if not isinstance(raw_rating, dict):
                raise RankingError(
                    f"{description} {leader}/{civilization} rating is not an object"
                )
            played = optional_iso_date(
                raw_rating.get("last_played"),
                f"{description} {leader}/{civilization} last_played",
            )
            if played is not None:
                dates.append(played)
    return max(dates, default=None)


def extract_rankings(
    league: dict[str, Any], table_size: int = DEFAULT_TABLE_SIZE
) -> tuple[
    int,
    list[CurrentStrategyRank],
    list[StrategyWithoutTableEvidence],
    int,
]:
    """Return only current live strategies, ranked on exact table-size evidence."""
    if table_size < 2:
        raise RankingError("table size must be at least 2")
    round_number = nonnegative_integer(league.get("round"), "league round")
    strategies = league.get("strategies")
    if not isinstance(strategies, list):
        raise RankingError("league strategies is not a list")

    rows: list[CurrentStrategyRank] = []
    without_table_evidence: list[StrategyWithoutTableEvidence] = []
    strategy_names: set[str] = set()
    excluded = 0

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
        league_only = flag(
            raw_strategy.get("league_only"), f"league strategy {strategy!r} league_only"
        )
        role = strategy_role(raw_strategy.get("kind"), f"league strategy {strategy!r} kind")
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
        last_played = latest_played(
            raw_strategy.get("leader_elo", {}), f"league strategy {strategy!r}"
        )
        if retired or human or league_only:
            excluded += 1
            continue

        by_table_size = raw_strategy.get("wins_by_table_size", {})
        if not isinstance(by_table_size, dict):
            raise RankingError(f"league strategy {strategy!r} wins_by_table_size is not an object")
        raw_evidence = by_table_size.get(str(table_size), by_table_size.get(table_size))
        if raw_evidence is None:
            without_table_evidence.append(
                StrategyWithoutTableEvidence(
                    player=player,
                    strategy=strategy,
                    role=role,
                    elo=global_elo,
                    rd=global_rd,
                    last_played=last_played,
                )
            )
            continue
        if not isinstance(raw_evidence, dict):
            raise RankingError(
                f"league strategy {strategy!r} {table_size}-player evidence is not an object"
            )
        games = nonnegative_integer(
            raw_evidence.get("games"),
            f"league strategy {strategy!r} {table_size}-player games",
        )
        wins = nonnegative_integer(
            raw_evidence.get("wins"),
            f"league strategy {strategy!r} {table_size}-player wins",
        )
        if games < 1:
            raise RankingError(
                f"league strategy {strategy!r} has {table_size}-player evidence without a game"
            )
        if wins > games:
            raise RankingError(
                f"league strategy {strategy!r} has more {table_size}-player wins than games"
            )
        rows.append(
            CurrentStrategyRank(
                player=player,
                strategy=strategy,
                role=role,
                elo=global_elo,
                rd=global_rd,
                games=games,
                wins=wins,
                win_bound=win_confidence(wins, games),
                last_played=last_played,
            )
        )

    if not rows and not without_table_evidence:
        raise RankingError(f"league round {round_number} has no live-eligible strategies")

    rows.sort(
        key=lambda row: (
            -row.win_bound,
            -(row.elo - SELECTION_Z * row.rd),
            -row.elo,
            row.player.casefold(),
            row.strategy.casefold(),
        )
    )
    without_table_evidence.sort(
        key=lambda row: (-row.elo, row.player.casefold(), row.strategy.casefold())
    )
    return round_number, rows, without_table_evidence, excluded


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
    rows: list[CurrentStrategyRank],
    without_table_evidence: list[StrategyWithoutTableEvidence],
    excluded: int,
    source: Path,
    table_size: int = DEFAULT_TABLE_SIZE,
) -> str:
    current = len(rows) + len(without_table_evidence)
    lines = [
        "# Current AI Strategy Rankings",
        "",
        textwrap.fill(
            f"League round **{round_number}**, generated from `{display_source(source)}`. "
            f"This table contains the **{current}** strategies currently eligible for live "
            f"games and ranks their exact **{table_size}-player** evidence. Retired, human, "
            f"and offline-only entries are omitted ({excluded} roster entr"
            f"{'y' if excluded == 1 else 'ies'} at this round), and historical "
            "leader/civilization rows are not carried into the public leaderboard.",
            width=88,
        ),
        "",
        textwrap.fill(
            f"Rank is the lower {SELECTION_Z:g}σ Wilson bound on outright wins, the same "
            "conservative objective the league uses for table-size-aware selection. "
            "Placement Elo is retained only as matchmaking context; it does not decide this "
            "order. Confidence intervals can overlap, so rank 1 is the current selection "
            "leader rather than a claim that every alternative is statistically separated.",
            width=88,
        ),
        "",
        textwrap.fill(
            "Last played is the latest retained UTC date for the strategy. Exact "
            "leader/civilization evidence remains reproducible in the league snapshot; CIVVIS "
            "publishes a pair recommendation only where conservative win evidence actually "
            "separates it from every rival, as documented in the README.",
            width=88,
        ),
        "",
        "Refresh this document after updating the committed league snapshot:",
        "",
        "`python3 tools/update_ai_player_elo_rankings.py`",
        "",
        "Use `--check` to verify that this generated file is current.",
        "",
        f"| Rank | Player (strategy) | Role | {table_size}p wins/games | Conservative win bound | Placement Elo | RD | Last played |",
        "|---:|---|---|---:|---:|---:|---:|---|",
    ]
    for rank, row in enumerate(rows, start=1):
        lines.append(
            f"| {rank} | {player_strategy_label(row.player, row.strategy)} | "
            f"{markdown_text(row.role)} | {row.wins}/{row.games} | "
            f"{100.0 * row.win_bound:.1f}% | {row.elo:.1f} | ±{row.rd:.1f} | "
            f"{row.last_played or '—'} |"
        )

    lines.extend(
        [
            "",
            f"## Current strategies without {table_size}-player evidence",
            "",
        ]
    )
    if without_table_evidence:
        lines.extend(
            [
                textwrap.fill(
                    f"These strategies remain eligible, but have no retained {table_size}-player "
                    "win record. Their placement rating is shown for identification only; they "
                    "are deliberately not mixed into the evidence-backed ranking above.",
                    width=88,
                ),
                "",
                "| Player (strategy) | Role | Placement Elo | RD | Last played |",
                "|---|---|---:|---:|---|",
            ]
        )
        for row in without_table_evidence:
            lines.append(
                f"| {player_strategy_label(row.player, row.strategy)} | "
                f"{markdown_text(row.role)} | {row.elo:.1f} | ±{row.rd:.1f} | "
                f"{row.last_played or '—'} |"
            )
    else:
        lines.append(f"Every current strategy has retained {table_size}-player evidence.")
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
    parser.add_argument(
        "--players",
        type=int,
        default=DEFAULT_TABLE_SIZE,
        help=f"table size whose current win evidence is ranked (default: {DEFAULT_TABLE_SIZE})",
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
        round_number, rows, without_table_evidence, excluded = extract_rankings(
            load_object(league_path), args.players
        )
        expected = render_document(
            round_number,
            rows,
            without_table_evidence,
            excluded,
            league_path,
            args.players,
        )
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
                f"{output_path} is current for {len(rows)} evidence-backed "
                f"{args.players}-player strategies at league round {round_number}"
            )
            return 0
        output_path.write_text(expected, encoding="utf-8")
        print(
            f"updated {output_path} with {len(rows)} evidence-backed "
            f"{args.players}-player strategies from league round {round_number}"
        )
        return 0
    except RankingError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
