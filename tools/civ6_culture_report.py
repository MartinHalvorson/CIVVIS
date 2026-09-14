#!/usr/bin/env python3
"""Read retained Pericles Culture runs; emit host-observed tourism bottlenecks.

Usage: python3 tools/civ6_culture_report.py ~/civvis-civ6-runs/control \
    --run-glob 'civvis-20260910*' > /tmp/pericles-culture.json

Each row is a segment, not an independent game. Continuations share a family.
No win rate is inferred across changing revisions, seeds, or resumed segments.
Missing telemetry stays null. Turn marks use exact turns, never interpolation.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


def snapshot(state: dict, local_player: int) -> dict:
    row = {key: state.get(key) for key in (
        "turn", "frame", "culture", "tourism_per_turn", "foreign_tourists",
        "domestic_tourists", "gold_per_turn", "faith_per_turn", "trade_capacity",
    )}
    row["suzerains"] = state.get("public_stats", {}).get("suzerain_count")
    cities = state.get("cities")
    row["cities"] = len(cities) if cities is not None else None
    row["completed_theaters"] = (
        sum(d.get("complete") is True and d.get("type") in (
            "DISTRICT_ACROPOLIS", "DISTRICT_THEATER")
            for city in cities for d in city["districts"])
        if cities is not None and all("districts" in c for c in cities) else None
    )
    row["great_works"] = (
        sum(len(city["great_works"]) for city in cities)
        if cities is not None and all("great_works" in c for c in cities) else None
    )
    for key, types in (
        ("amphitheaters", {"BUILDING_AMPHITHEATER"}),
        ("museums", {"BUILDING_MUSEUM_ART", "BUILDING_MUSEUM_ARTIFACT"}),
    ):
        row[key] = (sum(b in types for c in cities for b in c["buildings"])
                    if cities is not None and all("buildings" in c for c in cities) else None)
    row["cultural_great_people_on_map"] = (
        sum(u.get("kind") in {"UNIT_GREAT_WRITER", "UNIT_GREAT_ARTIST", "UNIT_GREAT_MUSICIAN"}
            for u in state["units"]) if "units" in state else None
    )
    rivals = state.get("rivals")
    routes = state.get("trade_routes")
    row["known_major_route_markets"] = (
        sorted({r["destination_player"] for r in routes
                if r.get("destination_player") in {p.get("player") for p in rivals}
                and r.get("destination_player") != local_player})
        if rivals is not None and routes is not None else None
    )
    # Rivals are the host's contacted major list. Do not call its maximum the
    # global victory bar: an unmet civilization can hold a larger defense.
    domestic = [r["domestic_tourists"] for r in (rivals or [])
                if r.get("domestic_tourists") is not None]
    row["largest_known_rival_domestic"] = max(domestic) if domestic else None
    return row


def report_run(run: Path, marks=(60, 100, 150, 200)) -> dict | None:
    summary_path = run / "summary.json"
    if not summary_path.exists():
        return None
    summary = json.loads(summary_path.read_text())
    seat = summary.get("seat") or {}
    if seat.get("leader") != "LEADER_PERICLES" or summary.get("victory_target") != "culture":
        return None
    states = {}
    malformed = 0
    events = run / "events.jsonl"
    if events.exists():
        with events.open() as stream:
            for line in stream:
                try:
                    state = json.loads(line)
                except json.JSONDecodeError:
                    malformed += 1
                    continue
                if state.get("kind") != "state" or not isinstance(state.get("turn"), int):
                    continue
                turn = state["turn"]
                # Prefer the latest replan frame, then the last observation.
                if turn not in states or state.get("frame", 0) >= (states[turn].get("frame") or 0):
                    states[turn] = snapshot(state, seat.get("local_player", 0))
    outcome = summary.get("outcome") or {}
    types = {v["index"]: v["type"] for v in seat.get("victory_types", [])}
    victory = types.get(outcome.get("victory"))
    # An unrelated player's defeat event is not our terminal outcome.
    terminal = (outcome.get("kind") == "victory" and isinstance(outcome.get("won"), bool)
                or outcome.get("kind") == "defeat" and outcome.get("ours") is True)
    won_culture = (outcome.get("won") is True and victory == "VICTORY_CULTURE"
                   if terminal and (victory is not None or outcome.get("kind") == "defeat")
                   else None)
    return {
        "run": run.name,
        "game_family": re.sub(r"-cont\d+$", "", run.name),
        "source": str(run.resolve()),
        "seat": seat,
        "decider_binaries": summary.get("decider_binaries"),
        "genome_treatments": summary.get("genome_treatments"),
        "seed_request": summary.get("seed_request"),
        "seed_probe": summary.get("seed_probe"),
        "outcome": outcome or None,
        "terminal": terminal,
        "victory_type": victory,
        "won_culture": won_culture,
        "malformed_event_lines": malformed,
        "observed_turns": len(states),
        "first_observed_positive_tourism_turn": min(
            (t for t, s in states.items() if (s["tourism_per_turn"] or 0) > 0), default=None),
        "marks": {str(t): states.get(t) for t in marks},
        "final_observed": states[max(states)] if states else None,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--run-glob", default="civvis-*")
    args = parser.parse_args()
    reports = [row for run in sorted(args.root.glob(args.run_glob)) if run.is_dir()
               for row in [report_run(run)] if row is not None]
    if not reports:
        parser.error("no retained Pericles Culture summaries matched")
    print(json.dumps({"segments": reports}, indent=2))


if __name__ == "__main__":
    main()
