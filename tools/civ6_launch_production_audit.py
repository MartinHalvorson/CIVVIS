#!/usr/bin/env python3
"""Audit native launch timing and support at the actual first Spaceport city.

Read-only. Missing observations are unknown, never zero. UI yield-source text
is retained verbatim; its English Factory attribution is not a stacking model.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

TECHS = ("industrialization", "chemistry", "rocketry", "satellites")
FUELS = ("RESOURCE_COAL", "RESOURCE_OIL", "RESOURCE_URANIUM")


def city_observation(state, city):
    raw = city.get("yield_sources", {}).get("production")
    match = re.search(r"^\s*\+([0-9.]+) from Factory\s*$", raw, re.M) if isinstance(raw, str) else None
    return {
        "turn": state["turn"], "id": city.get("id"), "name": city.get("name"),
        "production": city.get("production"), "producing": city.get("producing"),
        "producing_observed": "producing" in city,
        "population": city.get("pop"), "food_surplus": city.get("food_surplus"),
        "buildings": city.get("buildings"), "production_sources": raw,
        "factory_production_in_english_ui": float(match[1]) if match else None,
        "support_offers": [item for item in city.get("buildable", [])
            if any(word in str(item.get("t", "")) for word in ("FACTORY", "POWER_PLANT"))],
        "buildable_observed": "buildable" in city,
        "fuel_stocks": {fuel: state.get("strategic_resources", {}).get(fuel) for fuel in FUELS},
        "fuel_income": {fuel: state.get("strategic_resource_income", {}).get(fuel) for fuel in FUELS},
    }


def summarize(states):
    first, projects, pad = {}, {}, None
    # Only the first state of a turn defines the before-action census. A later
    # frame can reflect orders issued during that same turn.
    turns = {}
    for state in states:
        turns.setdefault(state["turn"], state)
        for tech in TECHS:
            if "TECH_" + tech.upper() in state.get("techs", []):
                first[tech] = min(first.get(tech, state["turn"]), state["turn"])
        for project in state.get("science_projects", []):
            projects[project] = min(projects.get(project, state["turn"]), state["turn"])
        for city in state.get("cities", []):
            if any(d.get("type") == "DISTRICT_SPACEPORT" and d.get("complete") is True
                   for d in city.get("districts", [])):
                candidate = (state["turn"], city.get("id"))
                if pad is None or candidate < pad:
                    pad = candidate
    observations = []
    if pad is not None:
        rocket = first.get("rocketry")
        for turn, state in sorted(turns.items()):
            # Retain the last 20 turns of preparation and the construction/
            # launch interval. This is a census, not a claim of causality.
            if rocket is not None and turn < rocket - 20:
                continue
            city = next((c for c in state.get("cities", []) if c.get("id") == pad[1]), None)
            if city is not None:
                observations.append(city_observation(state, city))
    return {"first_owned_techs": first, "first_observed_projects": projects,
            "first_spaceport": {"turn": pad[0], "city_id": pad[1]} if pad else None,
            "launch_city_observations": observations}


def audit(path):
    digest = hashlib.sha256()
    def states():
        with path.open("rb") as stream:
            for line in stream:
                digest.update(line)
                row = json.loads(line)
                if row.get("kind") == "state":
                    yield row
    result = summarize(states())
    result["events_sha256"] = digest.hexdigest()
    result["source"] = str(path)
    result["interpretation"] = "First observed native milestones. No causal savings estimate. Missing fields are unknown. UI Factory attribution does not prove an own Factory or stacking. Fuel income and bank are observed separately; neither measures powered status."
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("events", type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    result = audit(args.events)
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({key: value for key, value in result.items() if key != "launch_city_observations"}, indent=2))


if __name__ == "__main__":
    main()
