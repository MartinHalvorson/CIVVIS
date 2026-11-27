#!/usr/bin/env python3
"""How far the simulator's win rates are from the live seat's, level by level.

    python3 tools/sim_live_calibration.py                       # live ledger vs the Elo table
    python3 tools/sim_live_calibration.py --engine rows.jsonl   # ... and vs a native batch
    python3 tools/sim_live_calibration.py --results results.jsonl --json

The live seat plays ONE CIVVIS agent against five Civilization VI AIs at a
level (`results.jsonl` on the `ledger` branch: one row per launch). The
simulator has two ways to say what that seat should win:

- **The Elo table** (`data/strategy_elo.json`, `src/bin/strategy_elo.rs`).
  Each `civ6:<level>` entrant is our `stock` agent playing the level's AI
  handicap from `data/difficulties.json`, rated at four-seat tables against
  roster agents who play on the human's terms. The fit's own model
  (`strategies::fit`: a seat wins with probability proportional to
  `10^(elo/400)`) prices the live table directly: our lane's agent against
  `players - 1` copies of the level's entrant,

      p = 10^(a/400) / (10^(a/400) + (players - 1) * 10^(r/400))

  averaged over the lanes the live games actually played, each lane played
  by its twin (`beta/elo_page.TWINS`: a `religious` seat is `holy`, a
  `science` seat `moonshot`, a seat with no lane `jack`).
- **A native batch** (`--engine`): `gene_screen` rows of one measured seat
  against handicapped rival chairs (`--rivals firaxis-faithful --handicap
  rivals --rival-chairs 5`, `docs/GENE_SCREEN.md`). Its measured seats' wins,
  per level.

For each level the table prints the live record with its 95% range, each
prediction and its miss in percentage points (prediction minus live), and
the **implied rival Elo**: the rating the level's rivals would need on our
scale for the Elo model to predict the live record, fitted exactly as the
ELO Rankings page fits the Firaxis levels (`beta/elo_page.fit_level`, with
its imaginary pair, so a level we always beat reads `≤`). Set beside the
table's own `civ6:<level>` rating it is the size of the miss in the unit the
simulator speaks.

Live games are counted the way the Tournaments page counts them
(`beta/home_rankings.tournament_series`): a `<tag>-contN` resume is the same
game as `<tag>`, a game counts once by its best run, a game whose every run is
`unfinished` is not a game, only six-player Continents tables are read, and a
table two of our agents sat at (`civ` reads `A+B`) is left out, as the ELO
Rankings page leaves it out.
`--since-tag` (default `civvis-t7-`, the Official Benchmark's first game)
bounds the tags counted, so the default reads the benchmark and every
tournament after it.

`--max-miss PP` exits 1 when any level with games on both sides misses by
more than PP points (a calibration check for a field meant to match live).
Reads the files and the fetched ledger branch; writes nothing.
"""

from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "beta"))

import elo_page  # noqa: E402  the site's Firaxis-level fit: twins, rivals, fit_level
import home_rankings as ranks  # noqa: E402  how the Tournaments page counts a game

ELO_DEFAULT = ROOT / "data" / "strategy_elo.json"
RESULTS_REF = "refs/remotes/origin/ledger:results.jsonl"
SINCE_DEFAULT = "civvis-t7-"
#: Civilization VI's ladder, lowest first (`strategies::CIV6_RUNGS`).
LEVELS = tuple(ranks.CIV6_RUNGS)
#: Best first: a game counts by the best of its runs.
OUTCOMES = tuple(ranks.OUTCOMES)
LIVE_PLAYERS = 6
LIVE_MAP = "Continents.lua"
Z95 = 1.959963984540054


def level_of(difficulty: str | None) -> str | None:
    """`DIFFICULTY_DEITY` (or `deity`) is `deity`; anything else is None."""
    rung = str(difficulty or "").removeprefix("DIFFICULTY_").lower()
    return rung if rung in LEVELS else None


game_key = ranks.game_key


def read_jsonl(text: str) -> list[dict]:
    rows = []
    for line in text.splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if isinstance(row, dict):
            rows.append(row)
    return rows


def load_results(path: str | None) -> list[dict]:
    """The ledger's result lines, from a file or from the fetched branch."""
    if path:
        return read_jsonl(Path(path).read_text(encoding="utf-8"))
    shown = subprocess.run(["git", "-C", str(ROOT), "show", RESULTS_REF],
                           capture_output=True, check=False)
    if shown.returncode != 0:
        raise SystemExit("no ledger here: run `git fetch origin ledger:refs/remotes/origin/ledger` "
                         "or pass --results results.jsonl")
    return read_jsonl(shown.stdout.decode("utf-8", "replace"))


def live_games(rows: list[dict], since: str = SINCE_DEFAULT, players: int = LIVE_PLAYERS,
               map_name: str = LIVE_MAP) -> list[dict]:
    """One row per finished live game at the benchmark's table: its best run."""
    games: dict[str, list[dict]] = {}
    for row in rows:
        key = game_key(row)
        # A table with two of our agents (its civ reads `A+B`) is left out, as
        # the ELO Rankings page leaves it out: either one winning was a win.
        if (key and key >= since and row.get("outcome") in OUTCOMES
                and row.get("players") == players and row.get("map") == map_name
                and level_of(row.get("difficulty")) and "+" not in str(row.get("civ") or "")):
            games.setdefault(key, []).append(row)
    finished = []
    for key in sorted(games):
        best = min(games[key], key=lambda row: OUTCOMES.index(row["outcome"]))
        if best["outcome"] != "unfinished":
            finished.append({**best, "game": key})
    return finished


def wilson(wins: int, games: int, z: float = Z95) -> tuple[float, float] | None:
    """Wilson's 95% interval for a rate, honest at 0 and 100%."""
    if games <= 0:
        return None
    p = wins / games
    d = 1 + z * z / games
    centre = (p + z * z / (2 * games)) / d
    half = z * math.sqrt(p * (1 - p) / games + z * z / (4 * games * games)) / d
    return max(0.0, centre - half), min(1.0, centre + half)


def lone_seat_win(agent_elo: float, rival_elo: float, rivals: int) -> float:
    """The Elo model's chance that one seat beats `rivals` equal rivals."""
    return 1.0 / (1.0 + rivals * 10 ** ((rival_elo - agent_elo) / 400.0))


agent_for = elo_page.twin


def elo_prediction(games: list[dict], ratings: dict, level: str,
                   rival_elo: float | None = None) -> float | None:
    """The mean Elo-model win chance over `games` (one level's), each game's
    lane played by its roster agent against the level's rivals."""
    if rival_elo is None:
        rival = ratings.get(f"civ6:{level}")
        if not rival:
            return None
        rival_elo = float(rival["elo"])
    chances = []
    for game in games:
        agent = ratings.get(agent_for(game.get("target")))
        if not agent:
            return None
        players = int(game.get("players") or LIVE_PLAYERS)
        chances.append(lone_seat_win(float(agent["elo"]), rival_elo, players - 1))
    return sum(chances) / len(chances) if chances else None


def implied_rival_elo(games: list[dict], ratings: dict) -> dict | None:
    """The rival rating at which the Elo model predicts the live record, as
    `beta/elo_page.fit_level` fits it: each game is its twin's published
    rating and margin against the result. `None` without games or ratings."""
    rated = []
    for game in games:
        twin = ratings.get(agent_for(game.get("target")))
        if not twin:
            return None
        margin = twin.get("margin95", elo_page.MARGIN_Z * float(twin.get("sd") or 0))
        rated.append((float(twin["elo"]), float(margin or 0), game["outcome"] == "win"))
    return elo_page.fit_level(rated)


def engine_levels(paths: list[str]) -> tuple[dict[str, dict], list[str]]:
    """Measured-seat wins per level from `gene_screen` rows, and the fields
    (rivals / handicap / chairs and the calibration knobs) they played.

    Only a source that seats one measured seat against handicapped rival
    chairs is the live arrangement; any other is refused with its reason, so
    a self-play batch (every seat ours, a win rate of 1/players by symmetry)
    cannot pass for a prediction of the live seat. Every file given is pooled
    per level, so give one field's files per run (`fields` names each)."""
    levels: dict[str, dict] = {}
    fields: list[str] = []
    for path in paths:
        rows = read_jsonl(Path(path).read_text(encoding="utf-8"))
        headers = [row for row in rows if row.get("kind") == "header"]
        header = headers[0] if headers else {}
        players = int(header.get("players") or 0)
        live_shape = (header.get("handicap") == "rivals" and bool(header.get("rivals"))
                      and players > 1 and int(header.get("rival_chairs") or 0) == players - 1)
        if not live_shape:
            raise SystemExit(
                f"{path}: not one measured seat against {max(players - 1, 1)} handicapped rivals "
                f"(rivals={header.get('rivals')!r}, handicap={header.get('handicap')!r}, "
                f"rival_chairs={header.get('rival_chairs')!r}); a self-play batch predicts "
                f"1/{players or '?'} for every seat by symmetry and says nothing about the live seat")
        knobs = [f"{key}={header[key]}" for key in (
            "rivals", "rival_rung", "rival_pace", "rival_late_science", "rival_space_floor",
            "rival_victory_pace", "rival_war_pace", "target_mix") if header.get(key) not in (None, "", False)]
        fields.append(f"{Path(path).name}: " + " ".join(knobs))
        for row in rows:
            if row.get("kind") != "game":
                continue
            level = level_of(row.get("difficulty") or header.get("difficulty") or "prince")
            cell = levels.setdefault(level, {"wins": 0, "games": 0, "turns": []})
            cell["games"] += 1
            cell["wins"] += bool(row.get("win"))
            if isinstance(row.get("turn"), int):
                cell["turns"].append(row["turn"])
    return levels, fields


def calibrate(results: list[dict], ratings: dict, engine: dict[str, dict] | None = None,
              since: str = SINCE_DEFAULT) -> list[dict]:
    """One row per level that has live games or engine games."""
    by_level: dict[str, list[dict]] = {}
    for game in live_games(results, since):
        by_level.setdefault(level_of(game["difficulty"]), []).append(game)
    out = []
    for level in LEVELS:
        played = by_level.get(level, [])
        native = (engine or {}).get(level)
        if not played and not native:
            continue
        wins = sum(game["outcome"] == "win" for game in played)
        rate = wins / len(played) if played else None
        interval = wilson(wins, len(played))
        lanes: dict[str, int] = {}
        for game in played:
            twin = agent_for(game.get("target"))
            lanes[twin] = lanes.get(twin, 0) + 1
        predicted = elo_prediction(played, ratings, level) if played else None
        rival = ratings.get(f"civ6:{level}")
        row = {
            "level": level,
            "number": LEVELS.index(level) + 1,
            "live_wins": wins,
            "live_games": len(played),
            "live_rate": rate,
            "live_range": list(interval) if interval else None,
            "twins": dict(sorted(lanes.items())),
            "elo_rival": float(rival["elo"]) if rival else None,
            "elo_predicted": predicted,
            "elo_miss_pp": (predicted - rate) * 100 if predicted is not None and rate is not None else None,
            "implied_rival": implied_rival_elo(played, ratings) if played else None,
        }
        if native:
            n_rate = native["wins"] / native["games"] if native["games"] else None
            turns = sorted(native["turns"])
            row.update({
                "engine_wins": native["wins"],
                "engine_games": native["games"],
                "engine_rate": n_rate,
                "engine_range": list(wilson(native["wins"], native["games"]) or ()) or None,
                "engine_median_turn": turns[len(turns) // 2] if turns else None,
                "engine_miss_pp": ((n_rate - rate) * 100
                                   if n_rate is not None and rate is not None else None),
            })
        out.append(row)
    return out


def pct(value: float | None, digits: int = 1) -> str:
    return "—" if value is None else f"{100 * value:.{digits}f}%"


def signed(value: float | None) -> str:
    return "—" if value is None else f"{value:+.1f}"


def rated_text(fit: dict | None) -> str:
    """`1293 ±62`, or a bound: `≤ 495` for a level we always beat."""
    if not fit:
        return "—"
    if fit.get("bound") == "upper":
        return f"≤ {fit['elo']:.0f}"
    if fit.get("bound") == "lower":
        return f"≥ {fit['elo']:.0f}"
    return f"{fit['elo']:.0f} ±{fit['margin']:.0f}"


def render(rows: list[dict], fields: list[str], elo_path: str) -> str:
    engine = any("engine_games" in row for row in rows)
    head = ["Level", "Live W/G", "Live (95%)", "Twins", "Elo predicts", "Elo miss (pp)",
            "Sim civ6 Elo", "Live-implied Elo"]
    if engine:
        head += ["Engine W/G", "Engine (95%)", "Engine miss (pp)", "Engine end turn"]
    lines = ["| " + " | ".join(head) + " |", "|" + "---|" * len(head)]
    for row in rows:
        live_range = row["live_range"]
        cells = [
            f"Lv {row['number']} {row['level'].title()}",
            f"{row['live_wins']}/{row['live_games']}",
            (f"{pct(row['live_rate'])} ({pct(live_range[0], 0)}–{pct(live_range[1], 0)})"
             if live_range else "—"),
            " ".join(f"{twin} {count}" for twin, count in row["twins"].items()) or "—",
            pct(row["elo_predicted"], 2),
            signed(row["elo_miss_pp"]),
            "—" if row["elo_rival"] is None else f"{row['elo_rival']:.0f}",
            rated_text(row["implied_rival"]),
        ]
        if engine:
            span = row.get("engine_range")
            cells += [
                f"{row['engine_wins']}/{row['engine_games']}" if "engine_games" in row else "—",
                (f"{pct(row['engine_rate'])} ({pct(span[0], 0)}–{pct(span[1], 0)})" if span else "—"),
                signed(row.get("engine_miss_pp")),
                str(row.get("engine_median_turn") or "—"),
            ]
        lines.append("| " + " | ".join(cells) + " |")
    lines.append("")
    lines.append(f"Elo: {elo_path}. Each live game's lane is played by its twin "
                 f"({', '.join(f'{lane} {twin}' for lane, twin in sorted(elo_page.TWINS.items()))}; "
                 f"otherwise {elo_page.ADAPTIVE}) against {elo_page.RIVALS} copies of `civ6:<level>`.")
    lines.append("Miss = prediction minus live, in points. Live-implied Elo = the rating the "
                 "level's rivals need for the Elo model to predict the live record (beta/elo_page.fit_level).")
    for field in fields:
        lines.append(f"Engine: {field}")
    return "\n".join(lines)


def worst_miss(rows: list[dict]) -> float:
    misses = [abs(row[key]) for row in rows for key in ("elo_miss_pp", "engine_miss_pp")
              if row.get(key) is not None]
    return max(misses, default=0.0)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--results", help="results.jsonl (default: the fetched origin/ledger branch)")
    parser.add_argument("--elo", default=str(ELO_DEFAULT), help="strategy_elo rating file")
    parser.add_argument("--engine", nargs="*", default=[],
                        help="gene_screen rows: one measured seat against handicapped rival chairs")
    parser.add_argument("--since-tag", default=SINCE_DEFAULT,
                        help="count live games whose tag sorts at or after this (default: the benchmark's start)")
    parser.add_argument("--max-miss", type=float, help="exit 1 when any level misses by more than this many points")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    ratings = json.loads(Path(args.elo).read_text(encoding="utf-8")).get("ratings", {})
    engine, fields = engine_levels(args.engine) if args.engine else ({}, [])
    rows = calibrate(load_results(args.results), ratings, engine, args.since_tag)
    if args.json:
        print(json.dumps({"since_tag": args.since_tag, "elo": args.elo, "engine_fields": fields,
                          "levels": rows}, indent=1))
    else:
        shown = Path(args.elo).resolve()
        print(render(rows, fields, str(shown.relative_to(ROOT)) if shown.is_relative_to(ROOT) else args.elo))
    if args.max_miss is not None and worst_miss(rows) > args.max_miss:
        print(f"calibration: a level misses live by {worst_miss(rows):.1f} points, over {args.max_miss}",
              file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
