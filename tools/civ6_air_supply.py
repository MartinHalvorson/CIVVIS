#!/usr/bin/env python3
"""Measure native Bomber supply from observations, separately from proposals.

Usage: python3 tools/civ6_air_supply.py RUN_DIR [RUN_DIR ...]
Reads events.jsonl[.gz] and summary.json without changing a game or its records.
Each result describes one run segment; a resumed first observation is not a
completion date. Missing income, units or district metadata remains unknown.
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

from civ6_race_audit import event_path, events

ALUMINUM = "RESOURCE_ALUMINUM"
BOMBERS = frozenset(("UNIT_BOMBER", "UNIT_JET_BOMBER"))


def _number(value):
    return value if type(value) in (int, float) and math.isfinite(value) and value >= 0 else None


def _stock(state):
    table = state.get("strategic_resources")
    # The exporter omits zero entries from a present stock table. A missing
    # entire table can also mean an unavailable API, so it stays unknown.
    return _number(table.get(ALUMINUM, 0)) if isinstance(table, dict) else None


def _income(state):
    table = state.get("strategic_resource_income")
    # Gross accumulation, imports and bonuses; demand is not subtracted.
    return _number(table.get(ALUMINUM)) if isinstance(table, dict) else None


def _bomber(unit):
    return unit.get("class") == "PROMOTION_CLASS_AIR_BOMBER" or unit.get("kind") in BOMBERS


def _wing(state):
    units = state.get("units")
    if not isinstance(units, list) or any(not isinstance(u, dict) or
            not isinstance(u.get("kind"), str) for u in units):
        return None
    return sum(_bomber(unit) for unit in units)


def _base_count(state):
    cities = state.get("cities")
    if not isinstance(cities, list):
        return None
    count = 0
    for city in cities:
        if not isinstance(city, dict) or not isinstance(city.get("districts"), list):
            return None
        for district in city["districts"]:
            if not isinstance(district, dict):
                return None
            if district.get("type") == "DISTRICT_AERODROME":
                if type(district.get("complete")) is not bool or type(district.get("pillaged")) is not bool:
                    return None
                count += district["complete"] and not district["pillaged"]
    return count


def air_supply_totals(path: Path) -> dict | None:
    return _air_supply_totals(events(path))


def _air_supply_totals(rows) -> dict | None:
    first = last = None
    local_player = None
    turns, majors, minors = set(), set(), set()
    milestones, deposits = {}, {}
    coverage = {field: {"observed_frames": 0, "unknown_frames": 0}
                for field in ("income", "stock", "bombers", "usable_aerodromes")}
    frames = 0
    peak = None
    latest = None

    def mark(name, turn):
        if name not in milestones or turn < milestones[name]["observed_turn"]:
            milestones[name] = {"observed_turn": turn}

    for row in rows:
        kind, turn = row.get("kind"), row.get("turn")
        if kind == "seat" and type(row.get("local_player")) is int:
            local_player = row["local_player"]
        if type(turn) is not int or (last is not None and turn < last):
            continue
        if kind == "tiles" and isinstance(row.get("plots"), list):
            for tile in row["plots"]:
                if not isinstance(tile, dict) or any(type(tile.get(k)) is not int for k in ("x", "y")):
                    continue
                pos = (tile["x"], tile["y"])
                present = tile.get("r") == ALUMINUM
                if not present and pos not in deposits:
                    continue
                history = deposits.setdefault(pos, [])
                observation = {"observed_turn": turn, "resource_present": present,
                               "owner": tile.get("o"), "improvement": tile.get("im"),
                               "pillaged": tile.get("p")}
                # Full and delta plot records contain the same fields. Sight
                # changes are not acquisition or connection milestones.
                if not history or any(history[-1].get(k) != v for k, v in observation.items()
                                      if k != "observed_turn"):
                    history.append(observation)
            continue
        if kind != "state":
            continue
        if first is None:
            first = turn
        last = turn
        turns.add(turn)
        frames += 1
        for field, identities in (("rivals", majors), ("minors", minors)):
            players = row.get(field)
            for player in players if isinstance(players, list) else []:
                if isinstance(player, dict) and type(player.get("player")) is int:
                    identities.add(player["player"])
        techs = row.get("techs")
        if isinstance(techs, list):
            for tech in ("RADIO", "ADVANCED_FLIGHT"):
                if "TECH_" + tech in techs:
                    mark(tech.lower(), turn)
        income, stock, wing, bases = _income(row), _stock(row), _wing(row), _base_count(row)
        values = {"income": income, "stock": stock, "bombers": wing, "usable_aerodromes": bases}
        for field, value in values.items():
            coverage[field]["unknown_frames" if value is None else "observed_frames"] += 1
        if income is not None and income > 0:
            mark("aluminum_positive_income", turn)
        if stock is not None and stock > 0:
            mark("aluminum_positive_stock", turn)
        if wing is not None:
            peak = max(peak or 0, wing)
            for name, count in (("first_bomber", 1), ("two_bombers", 2), ("four_bombers", 4)):
                if wing >= count:
                    mark(name, turn)
        if bases is not None and bases:
            mark("usable_aerodrome", turn)
        cities = row.get("cities")
        for city in cities if isinstance(cities, list) else []:
            if not isinstance(city, dict):
                continue
            if city.get("producing") == "DISTRICT_AERODROME":
                mark("aerodrome_queued", turn)
            if city.get("producing") in BOMBERS:
                mark("bomber_queued", turn)
        latest = {"observed_turn": turn, "aluminum_gross_income": income,
                  "aluminum_stock": stock, "bombers": wing, "usable_aerodromes": bases}
    if first is None:
        return None

    def owner_kind(owner):
        if type(owner) is not int:
            return "unknown"
        if owner == -1:
            return "unowned"
        if local_player is not None and owner == local_player:
            return "ours"
        if owner in majors and owner not in minors:
            return "major"
        if owner in minors and owner not in majors:
            return "minor"
        return "unknown"

    for history in deposits.values():
        for observation in history:
            observation["owner_kind"] = owner_kind(observation["owner"])
            if not observation["resource_present"]:
                continue
            turn = observation["observed_turn"]
            # Histories can be collected before the first state. Keep these
            # observations separate from any assertion of a completion date.
            mark("aluminum_plot_revealed", turn)
            if observation["owner_kind"] == "ours":
                mark("aluminum_owned_plot", turn)
                if observation["improvement"] == "IMPROVEMENT_MINE" and observation["pillaged"] is False:
                    mark("aluminum_owned_unpillaged_mine", turn)
    for milestone in milestones.values():
        milestone["present_at_first_frame"] = milestone["observed_turn"] <= first
    return {"scope": "run_segment", "local_player": local_player,
            "first_observed_turn": first, "last_observed_turn": last,
            "observed_turns": len(turns), "missing_turns": last - first + 1 - len(turns),
            "observed_state_frames": frames, "milestones": milestones,
            "coverage": coverage, "peak_bombers_observed": peak, "last_snapshot": latest,
            "aluminum_deposits": [{"plot": list(pos), "observations": history}
                                  for pos, history in sorted(deposits.items())]}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("runs", type=Path, nargs="+")
    args = parser.parse_args(argv)
    for run in args.runs:
        path = event_path(run)
        summary = json.loads((run / "summary.json").read_text()) if (run / "summary.json").is_file() else {}
        print(json.dumps({"run": run.name, "seat": summary.get("seat"),
                          "decider_revisions": summary.get("decider_revisions"),
                          "outcome": summary.get("outcome"),
                          "air_supply": air_supply_totals(path) if path else summary.get("air_supply")}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
