#!/usr/bin/env python3
"""Read live science pace and group retained runs without counting resumes as games.

Usage: python3 tools/civ6_race_audit.py RUN_DIR [RUN_DIR ...] --out report.json
Reads events.jsonl[.gz] and summary.json. Never starts or changes a game.
Milestones are first OBSERVATIONS, not inferred completion dates. Checkpoints
require a frame at the requested turn. Missing evidence stays unknown.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import re
from pathlib import Path

TECHS = ("WRITING", "EDUCATION", "INDUSTRIALIZATION", "CHEMISTRY", "ROCKETRY",
         "SATELLITES", "NANOTECHNOLOGY", "SMART_MATERIALS", "OFFWORLD_MISSION")
PROJECTS = ("LAUNCH_EARTH_SATELLITE", "LAUNCH_MOON_LANDING", "LAUNCH_MARS_BASE",
            "LAUNCH_EXOPLANET_EXPEDITION")
CHECKPOINTS = (30, 60, 100, 150, 180, 200)


def events(path: Path):
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rt") as handle:
        for line in handle:
            try:
                event = json.loads(line)
            except ValueError:
                continue  # a live writer may leave a partial final line
            if isinstance(event, dict):
                yield event


def event_path(run: Path) -> Path | None:
    return next((run / name for name in ("events.jsonl", "events.jsonl.gz")
                 if (run / name).is_file()), None)


def race_totals(path: Path) -> dict | None:
    first = last = None
    milestones, checkpoints, city_history = {}, {}, {}
    boosted = {"techs": set(), "civics": set()}
    completed = {"techs": set(), "civics": set()}
    boost_audit = {"techs": [], "civics": []}
    observed_turns = set()
    idle = set()
    for state in events(path):
        turn = state.get("turn")
        if state.get("kind") != "state" or type(turn) is not int:
            continue
        if last is not None and turn < last:
            continue  # never merge a stale frame into the forward chronology
        if first is None:
            first = turn
        last = turn
        observed_turns.add(turn)

        def mark(name):
            milestones.setdefault(name, {"observed_turn": turn,
                                          "present_at_first_frame": turn == first})

        for tree, field in (("techs", "boosted_techs"), ("civics", "boosted_civics")):
            if not isinstance(state.get(tree), list):
                continue
            boosted[tree].update(state.get(field) or [])
            known = set(state[tree])
            for node in sorted(known - completed[tree]):
                boost_audit[tree].append({"node": node, "observed_turn": turn,
                                         "boost_observed": node in boosted[tree],
                                         "present_at_first_frame": turn == first})
            completed[tree].update(known)
        for tech in TECHS:
            if "TECH_" + tech in (state.get("techs") or []):
                mark(tech.lower())
        for project in PROJECTS:
            if "PROJECT_" + project in (state.get("science_projects") or []):
                mark(project.lower())
        if (state.get("science_victory_points_per_turn") or 0) > 1:
            mark("accelerated_flight")

        cities = state.get("cities")
        if not isinstance(cities, list):
            continue
        campuses = pads = libraries = 0
        for city in cities:
            cid = str(city.get("id"))
            history = city_history.setdefault(cid, {"first_observed_turn": turn})
            for district in city.get("districts") or []:
                if not district.get("complete") or district.get("pillaged"):
                    continue
                kind = district.get("type")
                if kind in ("DISTRICT_CAMPUS", "DISTRICT_SEOWON", "DISTRICT_OBSERVATORY"):
                    campuses += 1
                    mark("campus")
                    history.setdefault("campus_observed_turn", turn)
                if kind == "DISTRICT_SPACEPORT":
                    pads += 1
                    mark("spaceport")
                    history.setdefault("spaceport_observed_turn", turn)
            if "BUILDING_LIBRARY" in (city.get("buildings") or []):
                libraries += 1
                mark("library")
                history.setdefault("library_observed_turn", turn)
            if "producing" in city and not city["producing"]:
                idle.add((turn, cid))
        if len(cities) >= 2:
            mark("second_city")
        if turn in CHECKPOINTS and str(turn) not in checkpoints:
            checkpoints[str(turn)] = {
                "cities": len(cities), "campuses": campuses, "libraries": libraries,
                "operational_spaceports": pads, "science": state.get("science"),
                "culture": state.get("culture"),
                "production": (state.get("public_stats") or {}).get("production"),
                "techs": len(state["techs"]) if "techs" in state else None,
                "research": state.get("research"),
                "science_projects": state.get("science_projects"),
                "science_victory_points": state.get("science_victory_points"),
                "science_victory_points_per_turn": state.get("science_victory_points_per_turn"),
            }
    if first is None:
        return None
    return {"first_turn": first, "last_turn": last,
            "observed_turns": len(observed_turns),
            "missing_turns": last - first + 1 - len(observed_turns),
            "milestones": milestones, "checkpoints": checkpoints,
            "cities": city_history, "idle_city_turns": len(idle),
            "boost_audit": boost_audit}


def game_key(summary: dict) -> str:
    # Only the launcher's established continuation suffix is normalized.
    # Capture-free attempt numbers remain distinct games.
    return summary.get("game_id") or re.sub(r"-cont\d+(?:-[\w-]+)?$", "",
                                             summary.get("tag", ""))


def cohort(summary: dict) -> dict:
    seat = summary.get("seat") or {}
    return {**{key: summary.get(key) for key in (
        "difficulty", "ruleset", "speed", "map_size", "max_turns",
        "victory_target", "modes", "genome_treatments", "mod_arms")},
        "leader": seat.get("leader"), "map": seat.get("map"),
        "players": seat.get("players"), "victories": seat.get("victories"),
        "revisions": sorted(summary.get("decider_revisions") or []),
        "binaries": sorted({b["binary_sha256"] for b in
                            summary.get("decider_binaries") or []
                            if b.get("binary_sha256")})}


def report(runs: list[Path]) -> dict:
    groups = {}
    for run in runs:
        summary = json.loads((run / "summary.json").read_text())
        path = event_path(run)
        reading = race_totals(path) if path else summary.get("race")
        groups.setdefault(game_key(summary), []).append((summary, reading))
    games = []
    for key, segments in groups.items():
        segments.sort(key=lambda pair: (pair[1] or {}).get("first_turn", 10**9))
        identities = [cohort(summary) for summary, _ in segments]
        encoded = {json.dumps(identity, sort_keys=True) for identity in identities}
        reasons = []
        if len(encoded) != 1:
            reasons.append("settings_or_controller_changed")
        identity = identities[0]
        if any(identity.get(k) is None for k in identity) or not identity["binaries"]:
            reasons.append("missing_provenance")
        if len(identity["revisions"]) != 1 or len(identity["binaries"]) != 1:
            reasons.append("controller_not_frozen")
        if not all(s.get("configured") for s, _ in segments):
            reasons.append("unverified_settings")
        if not segments[0][1] or segments[0][1]["first_turn"] > 1:
            reasons.append("opening_not_retained")
        terminals = [s for s, _ in segments if
                     (s.get("outcome") or {}).get("kind") == "victory" or
                     ((s.get("outcome") or {}).get("kind") == "defeat" and
                      (s.get("outcome") or {}).get("ours"))]
        if not terminals:
            reasons.append("no_game_outcome")
        elif len(terminals) > 1:
            reasons.append("multiple_terminal_segments")
        games.append({"game": key, "segments": [s.get("tag") for s, _ in segments],
                      "eligible": not reasons, "exclusions": reasons,
                      "cohort": hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest(),
                      "identity": identity,
                      "outcome": terminals[-1].get("outcome") if terminals else None,
                      "readings": [r for _, r in segments]})
    return {"games": games, "game_count": len(games),
            "eligible_games": sum(g["eligible"] for g in games)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("runs", nargs="+", type=Path)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    text = json.dumps(report(args.runs), indent=2, sort_keys=True) + "\n"
    if args.out:
        args.out.write_text(text)
    else:
        print(text, end="")


if __name__ == "__main__":
    main()
