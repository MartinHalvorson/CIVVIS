#!/usr/bin/env python3
"""Audit final saves from the league that actually ran in production.

The league CSV is the sampling frame.  A save is admitted only when both its
seed and terminal turn equal the recorded match, which excludes follow-on
archives produced by ``until_next_victory``.  The output is explicitly a
final-state census: it can establish that a policy, civic, district, or
building was present at the end of a game, but it cannot estimate per-turn
usage.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import csv
from dataclasses import dataclass
import json
import math
from pathlib import Path
import statistics
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class Placement:
    strategy: str
    civ: str


@dataclass(frozen=True)
class Match:
    round: int
    seed: int
    turn: int
    victory: str
    placements: tuple[Placement, ...]


@dataclass(frozen=True)
class Archive:
    result_path: Path
    save_path: Path
    seed: int
    turn: int
    revision: str
    game_speed: str
    map_script: str
    max_turns: int
    victory: str


class CensusError(RuntimeError):
    """An archive violates an invariant required for a faithful census."""


def parse_placement(value: str) -> Placement:
    """Read both historical ``strategy@civ`` and current 3-field rows."""
    fields = value.split("@")
    if len(fields) < 2 or not fields[0] or not fields[-1]:
        raise CensusError(f"invalid placement {value!r}")
    return Placement(strategy=fields[0], civ=fields[-1])


def load_matches(path: Path) -> list[Match]:
    matches: list[Match] = []
    seen_seeds: set[int] = set()
    seen_rounds: set[int] = set()
    with path.open(newline="", encoding="utf-8-sig") as handle:
        for row in csv.DictReader(handle):
            match = Match(
                round=int(row["round"]),
                seed=int(row["seed"]),
                turn=int(row["turns"]),
                victory=row["victory"],
                placements=tuple(
                    parse_placement(item) for item in row["placements"].split("|")
                ),
            )
            if match.seed in seen_seeds:
                raise CensusError(f"duplicate match seed {match.seed}")
            if match.round in seen_rounds:
                raise CensusError(f"duplicate match round {match.round}")
            seen_seeds.add(match.seed)
            seen_rounds.add(match.round)
            matches.append(match)
    return sorted(matches, key=lambda match: match.round)


def _read_result(path: Path) -> Archive:
    with path.open(encoding="utf-8") as handle:
        result = json.load(handle)
    suffix = ".result.json"
    if not path.name.endswith(suffix):
        raise CensusError(f"unexpected result filename {path}")
    save_path = path.with_name(path.name[: -len(suffix)] + ".save.json")
    runtime = result.get("runtime") or {}
    return Archive(
        result_path=path,
        save_path=save_path,
        seed=int(result["seed"]),
        turn=int(result["turn"]),
        revision=str(runtime.get("revision") or "unknown"),
        game_speed=str(result.get("game_speed") or "unknown"),
        map_script=str(result.get("map_script") or "unknown"),
        max_turns=int(result.get("max_turns") or 0),
        victory=str(result.get("victory_type") or "unknown"),
    )


def index_archives(results_dir: Path) -> dict[tuple[int, int], list[Archive]]:
    archives: dict[tuple[int, int], list[Archive]] = defaultdict(list)
    for path in sorted(results_dir.glob("*.result.json")):
        archive = _read_result(path)
        archives[(archive.seed, archive.turn)].append(archive)
    return dict(archives)


def join_matches(
    matches: Iterable[Match],
    archives: dict[tuple[int, int], list[Archive]],
) -> tuple[list[tuple[Match, Archive]], list[Match]]:
    joined: list[tuple[Match, Archive]] = []
    missing: list[Match] = []
    for match in matches:
        candidates = archives.get((match.seed, match.turn), [])
        if not candidates:
            missing.append(match)
            continue
        if len(candidates) != 1:
            paths = ", ".join(str(item.result_path) for item in candidates)
            raise CensusError(
                f"match round {match.round} has multiple exact archives: {paths}"
            )
        archive = candidates[0]
        if not archive.save_path.is_file():
            raise CensusError(f"result has no matching save: {archive.result_path}")
        joined.append((match, archive))
    return joined, missing


def _objects(value: Any) -> list[dict[str, Any]]:
    if isinstance(value, list):
        return value
    if isinstance(value, dict):
        return list(value.values())
    raise CensusError(f"expected list or object, got {type(value).__name__}")


def _envoys(player: dict[str, Any], minor: int) -> int:
    return sum(
        int(count)
        for target, count in player.get("envoys", [])
        if int(target) == minor
    )


def _suzerain(
    minor: int, alive_majors: list[dict[str, Any]]
) -> int | None:
    counts = [(_envoys(player, minor), int(player["id"])) for player in alive_majors]
    if not counts:
        return None
    best = max(count for count, _ in counts)
    leaders = [pid for count, pid in counts if count == best]
    return leaders[0] if best >= 3 and len(leaders) == 1 else None


def _has_district(city: dict[str, Any], district: str) -> bool:
    districts = city.get("districts") or {}
    if isinstance(districts, dict):
        return district in districts
    return district in districts


def _has_building(city: dict[str, Any], building: str) -> bool:
    return building in (city.get("buildings") or [])


def analyze_save(
    match: Match,
    archive: Archive,
    government_rules: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    with archive.save_path.open(encoding="utf-8") as handle:
        save = json.load(handle)
    if int(save["seed"]) != match.seed or int(save["turn"]) != match.turn:
        raise CensusError(f"save metadata disagrees with match: {archive.save_path}")
    if str(save.get("victory_type") or "unknown") != match.victory:
        raise CensusError(f"save victory disagrees with match: {archive.save_path}")

    players = _objects(save["players"])
    cities = _objects(save["cities"])
    majors = [
        player
        for player in players
        if not player.get("is_minor", False)
        and not player.get("is_barbarian", False)
        and not player.get("is_free_city", False)
    ]
    alive_majors = [player for player in majors if player.get("alive", True)]
    alive_minors = [
        player
        for player in players
        if player.get("is_minor", False)
        and player.get("alive", True)
        and not player.get("is_barbarian", False)
        and not player.get("is_free_city", False)
    ]
    by_civ: dict[str, dict[str, Any]] = {}
    for player in majors:
        civ = str(player["civ"])
        if civ in by_civ:
            raise CensusError(f"duplicate major civilization {civ}: {archive.save_path}")
        by_civ[civ] = player
    if len(match.placements) != len(majors):
        raise CensusError(
            f"round {match.round} records {len(match.placements)} seats but save has "
            f"{len(majors)} majors"
        )

    observations: list[dict[str, Any]] = []
    for placement in match.placements:
        try:
            player = by_civ[placement.civ]
        except KeyError as error:
            raise CensusError(
                f"round {match.round} has no {placement.civ} major in its save"
            ) from error
        pid = int(player["id"])
        met = {int(other) for other in player.get("met", [])}
        met_minors = [minor for minor in alive_minors if int(minor["id"]) in met]
        held = [
            minor
            for minor in met_minors
            if _suzerain(int(minor["id"]), alive_majors) == pid
        ]
        deficits: list[int] = []
        for minor in met_minors:
            minor_id = int(minor["id"])
            if _suzerain(minor_id, alive_majors) == pid:
                continue
            best_rival = max(
                (
                    _envoys(rival, minor_id)
                    for rival in alive_majors
                    if int(rival["id"]) != pid
                ),
                default=0,
            )
            deficits.append(max(0, max(3, best_rival + 1) - _envoys(player, minor_id)))

        owned_cities = [city for city in cities if int(city["owner"]) == pid]
        government = str(player.get("government") or "none")
        government_rule = government_rules.get(government, {})
        policies = {str(policy) for policy in player.get("policies", [])}
        civics = {str(civic) for civic in player.get("civics", [])}
        observations.append(
            {
                "round": match.round,
                "seed": match.seed,
                "turn": match.turn,
                "revision": archive.revision,
                "strategy": placement.strategy,
                "family": strategy_family(placement.strategy),
                "civ": placement.civ,
                "player": pid,
                "alive": bool(player.get("alive", True)),
                "cities": len(owned_cities),
                "envoys_free": int(player.get("envoys_free") or 0),
                "envoys_placed": sum(
                    int(count) for _, count in player.get("envoys", [])
                ),
                "city_states_met": len(met_minors),
                "suzerain": len(held),
                "envoy_deficits": deficits,
                "envoy_shortfall": sum(deficits),
                "government": government,
                "envoys_per_threshold": int(
                    government_rule.get("envoys_per_threshold") or 0
                ),
                "influence_per_turn": float(
                    government_rule.get("influence_per_turn") or 0.0
                ),
                "political_philosophy": "political_philosophy" in civics,
                "ideology": "ideology" in civics,
                "charismatic_leader": "charismatic_leader" in policies,
                "gunboat_diplomacy": "gunboat_diplomacy" in policies,
                "diplomatic_quarter": any(
                    _has_district(city, "diplomatic_quarter")
                    for city in owned_cities
                ),
                "consulate": any(
                    _has_building(city, "consulate") for city in owned_cities
                ),
                "chancery": any(
                    _has_building(city, "chancery") for city in owned_cities
                ),
            }
        )
    return observations


def strategy_family(strategy: str) -> str:
    if strategy.startswith("g") and "-" in strategy:
        return "genome"
    if strategy in {"advanced", "advanced_v1", "basic"}:
        return "builtin"
    return strategy


def _mean(rows: list[dict[str, Any]], field: str) -> float:
    return statistics.fmean(float(row[field]) for row in rows) if rows else 0.0


def _pct(rows: list[dict[str, Any]], field: str) -> float:
    return (
        sum(bool(row[field]) for row in rows) / len(rows) * 100.0 if rows else 0.0
    )


def summarize(rows: list[dict[str, Any]]) -> dict[str, Any]:
    deficits = sorted(
        deficit for row in rows for deficit in row.get("envoy_deficits", [])
    )
    met = sum(int(row["city_states_met"]) for row in rows)
    held = sum(int(row["suzerain"]) for row in rows)
    governments = Counter(str(row["government"]) for row in rows)
    return {
        "games": len({int(row["round"]) for row in rows}),
        "seats": len(rows),
        "envoys_free_mean": _mean(rows, "envoys_free"),
        "envoys_free_zero_pct": (
            sum(int(row["envoys_free"]) == 0 for row in rows) / len(rows) * 100.0
            if rows
            else 0.0
        ),
        "envoys_placed_mean": _mean(rows, "envoys_placed"),
        "city_states_met_mean": _mean(rows, "city_states_met"),
        "suzerain_mean": _mean(rows, "suzerain"),
        "city_states_held_pct": held / met * 100.0 if met else 0.0,
        "envoy_shortfall_mean": _mean(rows, "envoy_shortfall"),
        "envoy_deficit_median": statistics.median(deficits) if deficits else 0.0,
        "envoy_deficit_p90": (
            deficits[min(len(deficits) - 1, math.floor(len(deficits) * 0.9))]
            if deficits
            else 0
        ),
        "envoy_deficit_max": deficits[-1] if deficits else 0,
        "envoys_per_threshold_mean": _mean(rows, "envoys_per_threshold"),
        "influence_per_turn_mean": _mean(rows, "influence_per_turn"),
        "political_philosophy_pct": _pct(rows, "political_philosophy"),
        "ideology_pct": _pct(rows, "ideology"),
        "charismatic_leader_pct": _pct(rows, "charismatic_leader"),
        "gunboat_diplomacy_pct": _pct(rows, "gunboat_diplomacy"),
        "diplomatic_quarter_pct": _pct(rows, "diplomatic_quarter"),
        "consulate_pct": _pct(rows, "consulate"),
        "chancery_pct": _pct(rows, "chancery"),
        "governments": dict(
            sorted(governments.items(), key=lambda item: (-item[1], item[0]))
        ),
    }


def load_current_roster(path: Path | None) -> set[str]:
    if path is None:
        return set()
    with path.open(encoding="utf-8") as handle:
        league = json.load(handle)
    return {
        str(entry["name"])
        for entry in league["strategies"]
        if not entry.get("retired", False) and not entry.get("human", False)
    }


def build_report(
    matches: list[Match],
    joined: list[tuple[Match, Archive]],
    missing: list[Match],
    observations: list[dict[str, Any]],
    archive_count: int,
    current_roster: set[str],
) -> dict[str, Any]:
    cohorts: dict[str, list[dict[str, Any]]] = {
        "all": observations,
        "genome": [row for row in observations if row["family"] == "genome"],
    }
    for strategy in sorted({str(row["strategy"]) for row in observations}):
        cohorts[strategy] = [
            row for row in observations if row["strategy"] == strategy
        ]
    if current_roster:
        cohorts["current_roster"] = [
            row for row in observations if row["strategy"] in current_roster
        ]

    profile_counts = Counter(
        (archive.game_speed, archive.map_script, archive.max_turns, archive.victory)
        for _, archive in joined
    )
    revisions = Counter(archive.revision for _, archive in joined)
    return {
        "method": {
            "unit": "final save per league-recorded game",
            "join": "exact seed and terminal turn",
            "interpretation": "final-state prevalence, not per-turn usage",
        },
        "join": {
            "match_rows": len(matches),
            "joined_games": len(joined),
            "missing_rounds": [match.round for match in missing],
            "archive_results": archive_count,
            "archive_results_not_exactly_joined": archive_count - len(joined),
        },
        "profiles": [
            {
                "game_speed": profile[0],
                "map_script": profile[1],
                "max_turns": profile[2],
                "victory_type": profile[3],
                "games": count,
            }
            for profile, count in sorted(
                profile_counts.items(), key=lambda item: (-item[1], item[0])
            )
        ],
        "revisions": dict(
            sorted(revisions.items(), key=lambda item: (-item[1], item[0]))
        ),
        "cohorts": {
            label: {
                "all_seats": summarize(rows),
                "alive_seats": summarize([row for row in rows if row["alive"]]),
            }
            for label, rows in cohorts.items()
            if rows
        },
    }


def render(report: dict[str, Any]) -> str:
    join = report["join"]
    lines = [
        "LIVE LEAGUE FINAL-STATE CENSUS",
        (
            f"joined {join['joined_games']}/{join['match_rows']} recorded games by "
            f"exact seed+turn; {len(join['missing_rounds'])} recorded rounds missing"
        ),
        "Final snapshots establish end-state presence; they do not estimate per-turn usage.",
        "",
        (
            "cohort             scope   games seats free mean/zero  placed  met/held "
            "held% shortfall  e/t"
        ),
    ]
    preferred = ["all", "current_roster", "genome", "advanced", "advanced_v1"]
    labels = preferred + sorted(set(report["cohorts"]) - set(preferred))
    for label in labels:
        if label not in report["cohorts"]:
            continue
        for scope, key in (("alive", "alive_seats"), ("all", "all_seats")):
            row = report["cohorts"][label][key]
            lines.append(
                f"{label[:18]:<18} {scope:<5} {row['games']:>5} {row['seats']:>5} "
                f"{row['envoys_free_mean']:>4.2f}/{row['envoys_free_zero_pct']:>4.1f}% "
                f"{row['envoys_placed_mean']:>6.1f} "
                f"{row['city_states_met_mean']:>4.1f}/{row['suzerain_mean']:<4.1f} "
                f"{row['city_states_held_pct']:>5.1f}% "
                f"{row['envoy_shortfall_mean']:>8.1f} "
                f"{row['envoys_per_threshold_mean']:>4.2f}"
            )
    lines.extend(
        [
            "",
            "cohort             scope  politics ideology charismatic gunboat  DQ consulate chancery",
        ]
    )
    for label in labels:
        if label not in report["cohorts"]:
            continue
        for scope, key in (("alive", "alive_seats"), ("all", "all_seats")):
            row = report["cohorts"][label][key]
            lines.append(
                f"{label[:18]:<18} {scope:<5} "
                f"{row['political_philosophy_pct']:>8.1f}% "
                f"{row['ideology_pct']:>7.1f}% "
                f"{row['charismatic_leader_pct']:>10.1f}% "
                f"{row['gunboat_diplomacy_pct']:>6.1f}% "
                f"{row['diplomatic_quarter_pct']:>4.1f}% "
                f"{row['consulate_pct']:>8.1f}% "
                f"{row['chancery_pct']:>7.1f}%"
            )
    all_alive = report["cohorts"]["all"]["alive_seats"]
    governments = ", ".join(
        f"{name}={count}" for name, count in list(all_alive["governments"].items())[:8]
    )
    lines.extend(["", f"alive-seat final governments: {governments}"])
    if join["missing_rounds"]:
        lines.append(
            "unmatched recorded rounds: "
            + ",".join(str(value) for value in join["missing_rounds"])
        )
    return "\n".join(lines)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--matches", type=Path, required=True)
    parser.add_argument("--league", type=Path)
    parser.add_argument(
        "--governments", type=Path, default=ROOT / "data" / "governments.json"
    )
    parser.add_argument("--round-min", type=int)
    parser.add_argument("--round-max", type=int)
    parser.add_argument("--revision")
    parser.add_argument("--format", choices=("text", "json"), default="text")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    matches = load_matches(args.matches)
    if args.round_min is not None:
        matches = [match for match in matches if match.round >= args.round_min]
    if args.round_max is not None:
        matches = [match for match in matches if match.round <= args.round_max]
    archives = index_archives(args.results)
    joined, missing = join_matches(matches, archives)
    if args.revision:
        joined = [pair for pair in joined if pair[1].revision == args.revision]
        matches = [match for match, _ in joined]
        missing = []
    with args.governments.open(encoding="utf-8") as handle:
        government_rules = json.load(handle)
    observations: list[dict[str, Any]] = []
    for match, archive in joined:
        observations.extend(analyze_save(match, archive, government_rules))
    if not observations:
        raise CensusError("no recorded games matched the requested cohort")
    report = build_report(
        matches,
        joined,
        missing,
        observations,
        sum(len(items) for items in archives.values()),
        load_current_roster(args.league),
    )
    if args.format == "json":
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
