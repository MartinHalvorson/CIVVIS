#!/usr/bin/env python3
"""Read native Artifact activation evidence without issuing host commands.

Proposals, owned units, occupied work slots and activation events remain
separate observations. Missing exports do not establish zero inventory.
"""
import argparse
import json
from pathlib import Path

from civ6_science_fuel_pilot import digest, records

GENE = "artifact-before-more-museums"
MUSEUM = "BUILDING_MUSEUM_ARTIFACT"
DIGGER = "UNIT_ARCHAEOLOGIST"
ARTIFACT = "GREATWORKOBJECT_ARTIFACT"
WORK_OBJECTS = {
    ARTIFACT, "GREATWORKOBJECT_WRITING", "GREATWORKOBJECT_SCULPTURE",
    "GREATWORKOBJECT_PORTRAIT", "GREATWORKOBJECT_LANDSCAPE",
    "GREATWORKOBJECT_RELIGIOUS", "GREATWORKOBJECT_MUSIC", "GREATWORKOBJECT_RELIC",
}


def artifact_count(cities):
    """Count reported occupied Artifact slots only after complete work queries."""
    if not isinstance(cities, list) or not cities:
        return None
    slots = {}
    for city in cities:
        works = city.get("great_works")
        if not isinstance(works, list) or type(city.get("id")) is not int:
            return None
        for work in works:
            if not isinstance(work, dict) or work.get("object") not in WORK_OBJECTS:
                return None
            if not isinstance(work.get("building"), str) or type(work.get("slot")) is not int:
                return None
            key = (city["id"], work["building"], work["slot"])
            if key in slots and slots[key] != work:
                return None
            slots[key] = work
    return sum(work["object"] == ARTIFACT for work in slots.values())


def snapshot(row):
    cities, units = row.get("cities"), row.get("units")
    people = None
    diggers = None
    if isinstance(units, list):
        people = []
        diggers = []
        for unit in units:
            person = unit.get("great_person")
            if isinstance(person, dict) and person.get("required_great_work") == ARTIFACT:
                people.append({"id": unit.get("id"), "x": unit.get("x"), "y": unit.get("y"),
                               "great_person": person})
            if unit.get("kind") == DIGGER:
                diggers.append({k: unit.get(k) for k in ("id", "x", "y", "moves", "build_charges")})
    museums = None
    if isinstance(cities, list):
        museums = []
        for city in cities:
            buildings = city.get("buildings")
            if not isinstance(buildings, list) or MUSEUM not in buildings:
                continue
            menu = city.get("buildable")
            quotes = ([item for item in menu if isinstance(item, dict) and item.get("t") == DIGGER]
                      if isinstance(menu, list) else None)
            current = city.get("producing_hash")
            museums.append({"id": city.get("id"), "name": city.get("name"),
                            "producing_hash": current,
                            "idle": current == 0 if type(current) is int else None,
                            "production_turns": city.get("production_turns"),
                            "legal_digger_quotes": quotes,
                            "pillaged_buildings": city.get("pillaged_buildings"),
                            "great_works": city.get("great_works")})
    return {"turn": row.get("turn"), "frame": row.get("frame"),
            "artifact_count": artifact_count(cities), "artifact_people": people,
            "owned_diggers": diggers, "museum_cities": museums,
            "museum_inventory_complete": isinstance(cities, list) and all(
                isinstance(city.get("buildings"), list) for city in cities),
            "science": row.get("science"), "tourism_per_turn": row.get("tourism_per_turn"),
            "foreign_tourists": row.get("foreign_tourists")}


def audit_run(directory):
    directory = Path(directory)
    sources = {}
    summary = {}
    path = directory / "summary.json"
    if path.exists():
        raw = path.read_bytes()
        try:
            summary = json.loads(raw)
            sources[path.name] = {"sha256": digest(raw), "complete": True}
        except json.JSONDecodeError:
            sources[path.name] = {"sha256": digest(raw), "complete": False}
    else:
        sources[path.name] = None
    runtime_updates = [{"line": number, "raw_line_sha256": digest(raw), "raw": row}
                       for number, raw, row in records(directory / "runtime_updates.jsonl", sources)]
    genomes = []
    for number, raw, row in records(directory / "why.log", sources, allow_text=True):
        if row.get("kind") == "genome":
            tags = row.get("treatments")
            enabled = GENE in tags if isinstance(tags, list) and all(isinstance(t, str) for t in tags) else None
            genomes.append({"line": number, "raw_line_sha256": digest(raw),
                            "enabled": enabled, "raw": row})
    proposals = []
    for number, raw, row in records(directory / "decisions.jsonl", sources):
        orders = [o for o in row.get("orders", []) if o.get("kind") == "produce" and o.get("verb") == DIGGER]
        if orders:
            proposals.append({"line": number, "raw_line_sha256": digest(raw),
                              "orders": orders, "decision_identity": {
                                  k: row.get("decision", {}).get(k) for k in ("turn", "frame")},
                              "execution_status": row.get("execution_status"),
                              "native_receipt_sha256": row.get("sha256"),
                              "binary_sha256": row.get("binary_on_disk_sha256")})
    observations, events, watched_units = [], [], set()
    unknown = {"units": 0, "artifact_inventory": 0, "museum_inventory": 0}
    state_rows = 0
    for number, raw, row in records(directory / "events.jsonl", sources):
        if row.get("kind") == "state":
            state_rows += 1
            reading = snapshot(row)
            people, diggers = reading["artifact_people"], reading["owned_diggers"]
            unknown["units"] += diggers is None
            unknown["artifact_inventory"] += reading["artifact_count"] is None
            unknown["museum_inventory"] += not reading["museum_inventory_complete"]
            watched_units.update(u["id"] for u in (people or []) + (diggers or []) if type(u.get("id")) is int)
            if people or diggers or reading["museum_cities"] or (reading["artifact_count"] or 0) > 0:
                observations.append({"line": number, "raw_line_sha256": digest(raw), **reading})
        else:
            text = json.dumps(row).upper()
            subject = row.get("unit", row.get("subject"))
            watched = type(subject) is int and subject in watched_units
            if watched or any(word in text for word in (DIGGER, "EXCAVATE", "MARY_LEAKEY", ARTIFACT)):
                events.append({"line": number, "raw_line_sha256": digest(raw), "raw": row})
    return {"schema": 1, "run": directory.name, "gene": GENE,
            "summary": {k: summary.get(k) for k in ("configured", "difficulty", "victory_target", "reason", "outcome")},
            "sources": sources, "runtime_updates": runtime_updates,
            "genomes": genomes, "state_rows": state_rows, "unknown_state_fields": unknown,
            "production_proposals": proposals, "observations": observations,
            "event_evidence": events,
            "event_selection": "Named Archaeologist, EXCAVATE, Mary Leakey or Artifact rows, plus later top-level unit/subject IDs observed on the relevant units. This is a retained event selection, not proof of absent activation or excavation events.",
            "interpretation": "Read-only observations, not a treatment effect. A production proposal is not an owned unit; an excavation verdict or missing unit is not an acquired Artifact. Work counts require complete city work exports and valid occupied slots. Unknown units, inventory and queues remain unknown. Repeated states are retained as frames, not independent games. Whole-file hashes cover bytes read from possibly growing files. No settings or native commands are written."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.out.resolve().is_relative_to(args.run_dir.resolve()):
        parser.error("write the audit outside its input run directory")
    result = audit_run(args.run_dir)
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"run": result["run"], "state_rows": result["state_rows"],
                      "production_proposals": len(result["production_proposals"]),
                      "observation_rows": len(result["observations"])}))


if __name__ == "__main__":
    main()
