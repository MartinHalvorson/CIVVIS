#!/usr/bin/env python3
"""Refresh README's per-leader best-strategy ranking from a CIVVIS league.

The official roster is deliberately read from ``CIV6_LEADER_POOL`` instead of
from every entry in ``data/civs.json``.  The latter also contains CIVVIS's
expanded historical roster, which does not belong in this leaderboard.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
import textwrap
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
START_MARKER = "<!-- BEGIN CIV6 LEADER STRATEGY RANKING -->"
END_MARKER = "<!-- END CIV6 LEADER STRATEGY RANKING -->"


class RankingError(ValueError):
    """The roster or league cannot produce an honest complete ranking."""


@dataclass(frozen=True)
class RankedLeader:
    civilization: str
    leader: str
    strategy: str
    username: str
    elo: float
    rd: float
    games: int
    wins: int
    bound: float
    upper: float


@dataclass(frozen=True)
class Coverage:
    """A pair the evidence does not resolve, and what it would take to resolve it."""

    civilization: str
    leader: str
    candidates: int
    games: int
    leader_name: str
    leader_record: str


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


def rust_string_list(body: str) -> list[str]:
    values: list[str] = []
    for number, raw_line in enumerate(body.splitlines(), start=1):
        line = raw_line.split("//", 1)[0].strip().removesuffix(",").strip()
        if not line:
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            raise RankingError(
                f"CIV6_LEADER_POOL entry on body line {number} is not a string: {line}"
            ) from error
        if not isinstance(value, str) or not value.strip():
            raise RankingError(
                f"CIV6_LEADER_POOL entry on body line {number} is not a non-empty string"
            )
        values.append(value)
    return values


def official_roster(root: Path) -> list[tuple[str, str]]:
    game_path = root / "src/game.rs"
    try:
        game_source = game_path.read_text(encoding="utf-8")
    except OSError as error:
        raise RankingError(f"cannot read {game_path}: {error}") from error

    matches = list(
        re.finditer(
            r"pub\s+const\s+CIV6_LEADER_POOL\s*:\s*"
            r"\[&str;\s*(\d+)\s*\]\s*=\s*\[(.*?)\]\s*;",
            game_source,
            flags=re.DOTALL,
        )
    )
    if len(matches) != 1:
        raise RankingError(
            f"expected exactly one CIV6_LEADER_POOL in {game_path}, found {len(matches)}"
        )
    declared_count = int(matches[0].group(1))
    civilizations = rust_string_list(matches[0].group(2))
    if len(civilizations) != declared_count:
        raise RankingError(
            "CIV6_LEADER_POOL declares "
            f"{declared_count} entries but contains {len(civilizations)}"
        )
    if len(set(civilizations)) != len(civilizations):
        raise RankingError("CIV6_LEADER_POOL contains a duplicate civilization")

    civ_specs = load_object(root / "data/civs.json")
    roster: list[tuple[str, str]] = []
    for civilization in civilizations:
        spec = civ_specs.get(civilization)
        if not isinstance(spec, dict):
            raise RankingError(f"{civilization!r} has no object in data/civs.json")
        leader = spec.get("leader")
        if not isinstance(leader, str) or not leader.strip():
            raise RankingError(f"{civilization!r} has no leader in data/civs.json")
        roster.append((civilization, leader))
    return roster


def settled_games_threshold(root: Path) -> int:
    league_path = root / "src/league.rs"
    try:
        source = league_path.read_text(encoding="utf-8")
    except OSError as error:
        raise RankingError(f"cannot read {league_path}: {error}") from error
    matches = re.findall(
        r"pub\s+const\s+CIV_ELO_MIN_GAMES\s*:\s*u32\s*=\s*(\d+)\s*;",
        source,
    )
    if len(matches) != 1:
        raise RankingError(
            f"expected exactly one CIV_ELO_MIN_GAMES in {league_path}, found {len(matches)}"
        )
    return int(matches[0])


def finite_number(value: Any, description: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise RankingError(f"{description} is not numeric")
    result = float(value)
    if not math.isfinite(result):
        raise RankingError(f"{description} is not finite")
    return result


def positive_games(value: Any, description: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        raise RankingError(f"{description} is not a positive integer")
    return value


def selection_z(root: Path) -> float:
    """Read the league's own selection z, so this tool cannot drift from it.

    The bound printed here must be the bound the league *selects* on. Copying the
    constant would let the two diverge silently, which is the whole failure this
    change exists to correct, so it is parsed out of the Rust exactly as the
    roster is parsed out of ``CIV6_LEADER_POOL``.
    """
    league_path = root / "src/league.rs"
    try:
        source = league_path.read_text(encoding="utf-8")
    except OSError as error:
        raise RankingError(f"cannot read {league_path}: {error}") from error
    matches = re.findall(
        r"^const\s+SELECTION_Z\s*:\s*f64\s*=\s*([0-9.]+)\s*;", source, flags=re.MULTILINE
    )
    if len(matches) != 1:
        raise RankingError(
            f"expected exactly one SELECTION_Z in {league_path}, found {len(matches)}"
        )
    value = float(matches[0])
    if not math.isfinite(value) or value <= 0.0:
        raise RankingError(f"SELECTION_Z is not a positive finite number: {matches[0]}")
    return value


def win_confidence(wins: int, games: int, z: float, upper: bool) -> float:
    """Wilson bound on outright wins — a line-for-line mirror of `win_confidence`
    in `src/league.rs`. Same inputs must give the same number to the digit."""
    if games == 0:
        return 1.0 if upper else 0.0
    n = float(games)
    p = wins / n
    z2 = z * z
    centre = p + z2 / (2.0 * n)
    margin = z * math.sqrt(p * (1.0 - p) / n + z2 / (4.0 * n * n))
    return min(1.0, max(0.0, (centre + (margin if upper else -margin)) / (1.0 + z2 / n)))


def rank_leaders(
    roster: list[tuple[str, str]],
    league: dict[str, Any],
    minimum_games: int,
    z: float,
) -> tuple[int, list[RankedLeader], list[Coverage]]:
    """Order every pair by the statistic the league actually selects on, and print
    only the pairs whose leader separates from the field.

    ⚠ THIS DELIBERATELY PRINTS ALMOST NOTHING. Ranking by placement Glicko named a
    different strategy in 23 of 50 pairs and separated in none of them; the honest
    bar leaves one row. A short true table and an explicit coverage count is the
    point of the change, not a bug in it — see the README caption and
    `docs/EVAL_INTEGRITY.md` §5.
    """
    if minimum_games < 1:
        raise RankingError("minimum games must be positive")
    round_number = league.get("round")
    if isinstance(round_number, bool) or not isinstance(round_number, int):
        raise RankingError("league round is not an integer")
    strategies = league.get("strategies")
    if not isinstance(strategies, list):
        raise RankingError("league strategies is not a list")

    rows: list[RankedLeader] = []
    coverage: list[Coverage] = []
    for civilization, leader in roster:
        candidates: list[RankedLeader] = []
        for index, raw_strategy in enumerate(strategies):
            if not isinstance(raw_strategy, dict):
                raise RankingError(f"league strategy {index} is not an object")
            if raw_strategy.get("retired") is True or raw_strategy.get("human") is True:
                continue
            strategy = raw_strategy.get("name")
            if not isinstance(strategy, str) or not strategy.strip():
                raise RankingError(f"active league strategy {index} has no name")
            username = raw_strategy.get("username", "")
            if not isinstance(username, str):
                raise RankingError(f"league strategy {strategy!r} has a non-string username")
            leader_elos = raw_strategy.get("leader_elo", {})
            if not isinstance(leader_elos, dict):
                raise RankingError(f"league strategy {strategy!r} has invalid leader_elo")
            civilization_elos = leader_elos.get(leader)
            if civilization_elos is None:
                continue
            if not isinstance(civilization_elos, dict):
                raise RankingError(
                    f"league strategy {strategy!r} has invalid ratings for {leader}"
                )
            rating = civilization_elos.get(civilization)
            if rating is None:
                continue
            if not isinstance(rating, dict):
                raise RankingError(
                    f"league strategy {strategy!r} has invalid {leader}/{civilization} rating"
                )
            raw_games = rating.get("games", 0)
            if raw_games == 0:
                continue
            games = positive_games(
                raw_games,
                f"{strategy!r} {leader}/{civilization} games",
            )
            if games < minimum_games:
                continue
            wins = rating.get("wins", 0)
            if isinstance(wins, bool) or not isinstance(wins, int) or wins < 0:
                raise RankingError(
                    f"{strategy!r} {leader}/{civilization} has invalid wins: {wins!r}"
                )
            if wins > games:
                raise RankingError(
                    f"{strategy!r} {leader}/{civilization} records {wins} wins in {games} games"
                )
            candidates.append(
                RankedLeader(
                    civilization=civilization,
                    leader=leader,
                    strategy=strategy,
                    username=username,
                    elo=finite_number(
                        rating.get("rating"),
                        f"{strategy!r} {leader}/{civilization} rating",
                    ),
                    rd=finite_number(
                        rating.get("rd"),
                        f"{strategy!r} {leader}/{civilization} RD",
                    ),
                    games=games,
                    wins=wins,
                    bound=win_confidence(wins, games, z, upper=False),
                    upper=win_confidence(wins, games, z, upper=True),
                )
            )
        if not candidates:
            coverage.append(
                Coverage(
                    civilization=civilization,
                    leader=leader,
                    candidates=0,
                    games=0,
                    leader_name="—",
                    leader_record="no qualifying record",
                )
            )
            continue
        candidates.sort(
            key=lambda row: (
                -row.bound,
                -row.games,
                row.strategy.casefold(),
                row.username.casefold(),
            )
        )
        best = candidates[0]
        rest = candidates[1:]
        # Separation, applied the way the table's own heading claims. A strategy is
        # "best for this civilization" only if its conservative win bound clears the
        # OPTIMISTIC bound of every rival. Comparing point estimates is exactly what
        # let a strategy winning 8/43 outrank one winning 230/622.
        if rest and best.bound > max(row.upper for row in rest):
            rows.append(best)
        else:
            coverage.append(
                Coverage(
                    civilization=civilization,
                    leader=leader,
                    candidates=len(candidates),
                    games=sum(row.games for row in candidates),
                    leader_name=best.strategy,
                    leader_record=f"{best.wins}/{best.games}",
                )
            )

    rows.sort(
        key=lambda row: (
            -row.bound,
            row.civilization.casefold(),
            row.leader.casefold(),
            row.strategy.casefold(),
        )
    )
    coverage.sort(key=lambda row: (-row.games, row.civilization.casefold()))
    return round_number, rows, coverage


def markdown_text(value: str) -> str:
    return value.replace("\\", "\\\\").replace("|", "\\|")


def inline_code(value: str) -> str:
    fence = "`" if "`" not in value else "``"
    return f"{fence}{value}{fence}"


def render_block(
    round_number: int,
    rows: list[RankedLeader],
    coverage: list[Coverage],
    minimum_games: int,
    z: float,
) -> str:
    total = len(rows) + len(coverage)
    lines = [
        START_MARKER,
        "## Which strategy suits which civilization, where the evidence says so",
        "",
        textwrap.fill(
            f"League round **{round_number}**, over the canonical {total}-civilization Civ VI "
            "roster. A pair is listed only when the leading strategy's conservative outright-win "
            f"bound clears the optimistic bound of every rival — the same lower-{z:g}σ Wilson "
            "bound the league itself selects parents, retirement and live seating on. "
            f"**{len(rows)} of {total} pairs meet that bar.** The rest are reported as coverage "
            "rather than ranked, because on this evidence they have no best strategy.",
            width=88,
        ),
        "",
        "Refresh after the live league changes:",
        "",
        "`python3 tools/update_readme_rankings.py --league league/league.json`",
        "",
        "Add `--check` to verify without writing.",
        "",
    ]
    if rows:
        lines.extend(
            [
                "| Rank | Civilization | Leader | Strategy | Wins/games | Win bound |",
                "|---:|---|---|---|---:|---:|",
            ]
        )
        for rank, row in enumerate(rows, start=1):
            strategy = inline_code(row.strategy)
            if row.username and row.username != row.strategy:
                strategy += f" ({markdown_text(row.username)})"
            lines.append(
                f"| {rank} | {markdown_text(row.civilization)} | "
                f"{markdown_text(row.leader)} | {strategy} | "
                f"{row.wins}/{row.games} | {row.bound:.3f} |"
            )
    else:
        lines.append(
            "No leader/civilization pair separates at this round. Nothing is ranked."
        )
    lines.extend(
        [
            "",
            f"### {'All' if not rows else 'The other'} {len(coverage)} pairs: "
            "what the league has, and what it would take",
            "",
            textwrap.fill(
                "These are unresolved, not tied. Where two or more strategies have been rated "
                "on a pair, the leading one is shown with its actual record so the gap between "
                "the evidence and the claim stays visible — none of them separates from its own "
                "runner-up, so naming one would report who has been seated, not what suits the "
                "civilization.",
                width=88,
            ),
            "",
            "| Civilization | Leader | Candidates | Games | Leading strategy | Its record |",
            "|---|---|---:|---:|---|---:|",
        ]
    )
    contested = [row for row in coverage if row.candidates]
    for row in contested:
        lines.append(
            f"| {markdown_text(row.civilization)} | {markdown_text(row.leader)} | "
            f"{row.candidates} | {row.games} | {inline_code(row.leader_name)} | "
            f"{markdown_text(row.leader_record)} |"
        )
    # Pairs nobody has played enough to rate are a coverage fact, not fifty rows of
    # the same sentence. They are named rather than counted so a gap in the
    # snapshot cannot hide behind a number.
    unplayed = [row for row in coverage if not row.candidates]
    if unplayed:
        named = ", ".join(
            f"{markdown_text(row.civilization)} ({markdown_text(row.leader)})"
            for row in sorted(unplayed, key=lambda row: row.civilization.casefold())
        )
        lines.extend(
            [
                "",
                textwrap.fill(
                    f"The remaining **{len(unplayed)} of {len(rows) + len(coverage)}** pairs have "
                    f"no strategy with {minimum_games} games in this snapshot, so there is "
                    f"nothing to rank and nothing to contest: {named}.",
                    width=88,
                ),
            ]
        )
    lines.extend(
        [
            "",
            textwrap.fill(
                f"A strategy needs at least {minimum_games} games with that exact pair to "
                "qualify. The league's Glicko rating is deliberately not shown: it orders "
                "matchmaking, not strength, and ranking on it named a different strategy in "
                "23 of 50 pairs while separating in none of them. See "
                "`docs/EVAL_INTEGRITY.md` §5.",
                width=88,
            ),
            END_MARKER,
        ]
    )
    return "\n".join(lines)


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        raise RankingError(f"cannot read {path}: {error}") from error


def with_ranking(readme: str, block: str) -> str:
    starts = readme.count(START_MARKER)
    ends = readme.count(END_MARKER)
    if (starts, ends) == (1, 1):
        start = readme.index(START_MARKER)
        end = readme.index(END_MARKER, start) + len(END_MARKER)
        prefix = readme[:start].rstrip("\n")
        suffix = readme[end:].lstrip("\n")
        return f"{prefix}\n\n{block}\n\n{suffix}"
    if starts or ends:
        raise RankingError(
            f"README has mismatched ranking markers ({starts} start, {ends} end)"
        )
    if not readme.startswith("# CIVVIS\n"):
        raise RankingError("README must begin with '# CIVVIS' before inserting the ranking")
    title, remainder = readme.split("\n", 1)
    return f"{title}\n\n{block}\n\n{remainder.lstrip(chr(10))}"


def repo_path(root: Path, value: str) -> Path:
    path = Path(value).expanduser()
    return path.resolve() if path.is_absolute() else (root / path).resolve()


def default_league(root: Path) -> Path:
    live = root / "league/league.json"
    return live if live.is_file() else root / "data/league/league.json"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--league",
        help="league.json to read (repo-relative; defaults to live, then committed snapshot)",
    )
    parser.add_argument(
        "--readme", default="README.md", help="README to update (repo-relative)"
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--check", action="store_true", help="fail if README is not freshly generated"
    )
    mode.add_argument(
        "--stdout", action="store_true", help="print the generated block without writing"
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        league_path = (
            repo_path(ROOT, args.league) if args.league else default_league(ROOT)
        )
        readme_path = repo_path(ROOT, args.readme)
        roster = official_roster(ROOT)
        minimum_games = settled_games_threshold(ROOT)
        z = selection_z(ROOT)
        round_number, rows, coverage = rank_leaders(
            roster, load_object(league_path), minimum_games, z
        )
        block = render_block(round_number, rows, coverage, minimum_games, z)
        if args.stdout:
            print(block)
            return 0
        current = read_text(readme_path)
        expected = with_ranking(current, block)
        if args.check:
            if current != expected:
                print(
                    f"{readme_path} is stale for league round {round_number}; "
                    "run tools/update_readme_rankings.py without --check",
                    file=sys.stderr,
                )
                return 1
            print(
                f"{readme_path} is current at league round {round_number}: "
                f"{len(rows)} pair(s) resolve, {len(coverage)} reported as coverage"
            )
            return 0
        readme_path.write_text(expected, encoding="utf-8")
        print(
            f"updated {readme_path} from league round {round_number}: "
            f"{len(rows)} pair(s) resolve, {len(coverage)} reported as coverage"
        )
        return 0
    except RankingError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
