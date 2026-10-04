#!/usr/bin/env python3
"""Measure production ramps from recorded Firaxis games, without controlling them.

Use the first state of each turn, so extra decision frames do not inflate output.
Rival totals come from public_stats, never from the partial visible city roster.
The cumulative column sums observed start-of-turn rates, not production spent;
missing turns are reported and are never interpolated. Tile coverage is explicit.

    python3 tools/civ6_production_report.py <run> [<run> ...] --turns 50 75 100 150
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Iterable


def report(events: Iterable[str], checkpoints: set[int]) -> dict:
    plots: dict[tuple[int, int], dict] = {}
    seen: set[int] = set()
    rows = []
    cumulative = 0.0
    for line in events:
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            # A live writer may still be appending its final line.
            continue
        if event.get("kind") == "tiles":
            for plot in event.get("plots", []):
                plots[(plot["x"], plot["y"])] = plot
            continue
        if event.get("kind") != "state":
            continue
        turn = event.get("turn")
        if not isinstance(turn, int) or turn in seen:
            continue
        seen.add(turn)
        cities = event.get("cities", [])
        production = sum(city.get("yields", {}).get("production", 0) for city in cities)
        cumulative += production
        if turn not in checkpoints:
            continue
        rivals = []
        for rival in event.get("rivals", []):
            stats = rival.get("public_stats", {})
            rate = stats.get("production")
            count = stats.get("city_count")
            rivals.append({
                "player": rival.get("player"), "leader": rival.get("leader"),
                "production": rate, "cities": count,
                "production_per_city": rate / count if rate is not None and count else None,
            })
        best = max((r["production"] for r in rivals if r["production"] is not None), default=None)
        worked = known = bare = productive_bare = 0
        for city in cities:
            for tile in city.get("worked", []):
                position = (tile["x"], tile["y"])
                if position == (city["x"], city["y"]):
                    continue
                worked += 1
                plot = plots.get(position)
                if plot is None:
                    continue
                known += 1
                if not plot.get("im") and not plot.get("d"):
                    bare += 1
                    if tile.get("yields", {}).get("production", 0) > 0:
                        productive_bare += 1
        first = min(seen)
        rows.append({
            "turn": turn, "cities": len(cities), "production": production,
            "production_per_city": production / len(cities) if cities else None,
            "rivals": rivals, "ratio_to_best_observed_rival": production / best if best else None,
            "science": event.get("science"), "culture": event.get("culture"),
            "gold_per_turn": event.get("gold_per_turn"),
            "builders": sum(u.get("kind") == "UNIT_BUILDER" for u in event.get("units", [])),
            "builder_charges": sum(u.get("build_charges", 0) for u in event.get("units", []) if u.get("kind") == "UNIT_BUILDER"),
            "worked_noncenter_tiles": worked, "worked_tiles_with_plot_records": known,
            "worked_unimproved_tiles": bare, "worked_unimproved_productive_tiles": productive_bare,
            "observed_production_rate_sum": cumulative,
            "first_observed_turn": first, "observed_turn_count": len(seen),
            "missing_turn_count": turn - first + 1 - len(seen),
        })
    return {"checkpoints": rows, "missing_checkpoints": sorted(checkpoints - seen)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("runs", type=Path, nargs="+")
    parser.add_argument("--turns", type=int, nargs="+", default=[25, 50, 75, 100, 125, 150])
    args = parser.parse_args()
    reports = []
    for run in args.runs:
        path = run / "events.jsonl" if run.is_dir() else run
        with path.open() as events:
            reports.append({"run": str(run), **report(events, set(args.turns))})
    print(json.dumps(reports, indent=2))


if __name__ == "__main__":
    main()
