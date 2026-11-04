#!/usr/bin/env python3
"""Read-only census for the preregistered native Science fuel pilot.

Preserve native evidence without deciding whether a sent deal delivered fuel.
Completed summaries define configuration; ongoing/missing summaries stay pending.
"""
import argparse
from datetime import datetime
import hashlib
import json
from pathlib import Path
import re

GENE = "science-power-buys-fuel"
FUELS = ("RESOURCE_COAL", "RESOURCE_OIL", "RESOURCE_URANIUM")
PLANTS = ("BUILDING_COAL_POWER_PLANT", "BUILDING_FOSSIL_FUEL_POWER_PLANT",
          "BUILDING_POWER_PLANT")


def stamp(value):
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def digest(data):
    return hashlib.sha256(data).hexdigest()


def records(path, sources, allow_text=False):
    if not path.exists():
        sources[path.name] = None
        return
    sha = hashlib.sha256()
    with path.open("rb") as stream:
        for number, raw in enumerate(stream, 1):
            sha.update(raw)
            # A writer can still be appending the last line in a live attempt.
            if not raw.endswith(b"\n"):
                sources[path.name] = {"sha256": sha.hexdigest(), "partial_tail": True}
                return
            try:
                row = json.loads(raw)
            except json.JSONDecodeError:
                if allow_text:
                    continue
                raise
            if isinstance(row, dict):
                yield number, raw, row
    sources[path.name] = {"sha256": sha.hexdigest(), "partial_tail": False}


def audit_run(directory):
    sources = {}
    summary_path = directory / "summary.json"
    if summary_path.exists():
        raw = summary_path.read_bytes()
        summary = json.loads(raw)
        sources[summary_path.name] = {"sha256": digest(raw)}
    else:
        summary = {}
        sources[summary_path.name] = None
    starts = []
    binaries = []
    for _, _, row in records(directory / "runtime_updates.jsonl", sources):
        binaries.append(row)
        if row.get("status") == "start" and row.get("utc"):
            starts.append(row["utc"])
    genomes = []
    for number, raw, row in records(directory / "why.log", sources, allow_text=True):
        # why.log contains JSON genome lines alongside ordinary text.
        if row.get("kind") == "genome":
            tags = row.get("treatments")
            normalized = ({**row, "treatments": sorted(t for t in tags if t != GENE),
                           "forced": sorted(t for t in row.get("forced", []) if t != GENE),
                           "ledger_withheld": sorted(t for t in row.get("ledger_withheld", [])
                                                     if t != GENE)}
                          if isinstance(tags, list) else None)
            item = {"line": number, "raw_line_sha256": digest(raw), "raw": row,
                    "enabled": GENE in tags if isinstance(tags, list) else None,
                    "normalized_sha256": digest(json.dumps(normalized, sort_keys=True).encode())
                    if normalized is not None else None}
            if not genomes or genomes[-1]["raw"] != row:
                genomes.append(item)
    states, deals, milestones = {}, [], {}
    note_fires = []
    for number, raw, row in records(directory / "civvis_notes.jsonl", sources):
        match = re.search(r"science_power_fuel_buy=(\d+)", row.get("note", ""))
        if match and int(match[1]) > 0:
            note_fires.append({"line": number, "raw_line_sha256": digest(raw), "raw": row})
    for number, raw, row in records(directory / "events.jsonl", sources):
        kind = row.get("kind")
        if kind in ("deal_offer", "deal_response", "deal_closed"):
            # Resource IDs in replies are numeric: retain every buy reply,
            # rather than guessing that an unmatched ID is (or is not) Coal.
            if row.get("direction") == "buy" or "RESOURCE_COAL" in json.dumps(row):
                deals.append({"line": number, "raw_line_sha256": digest(raw), "raw": row})
        if kind != "state":
            continue
        for tech in ("INDUSTRIALIZATION", "CHEMISTRY", "ROCKETRY", "SATELLITES"):
            if "TECH_" + tech in row.get("techs", []):
                milestones.setdefault(tech, row["turn"])
        for project in row.get("science_projects", []):
            milestones.setdefault(project, row["turn"])
        # Retain the earliest observed frame of each turn, including a raw-line
        # hash so a selected value can be checked against its original state.
        turn = row["turn"]
        if turn not in states or row.get("frame", 0) < states[turn]["raw"].get("frame", 0):
            states[turn] = {"line": number, "raw_line_sha256": digest(raw), "raw": row}
    readings = []
    coal_turns = [d["raw"].get("turn") for d in deals
                  if d["raw"].get("kind") == "deal_offer"
                  and "RESOURCE_COAL" in json.dumps(d["raw"])]
    for turn, item in sorted(states.items()):
        row = item["raw"]
        cities = []
        for city in row.get("cities", []):
            buildings = city.get("buildings")
            interested = (isinstance(buildings, list) and any(
                b in buildings for b in (*PLANTS, "BUILDING_RESEARCH_LAB")))
            if interested:
                cities.append({key: city.get(key) for key in (
                    "id", "name", "x", "y", "buildings", "pillaged_buildings", "power",
                    "yields", "yield_sources", "production", "producing", "production_turns")})
        near_coal = any(t is not None and abs(t - turn) <= 2 for t in coal_turns)
        if cities or near_coal or turn in (100, 150, max(states)):
            readings.append({"line": item["line"], "raw_line_sha256": item["raw_line_sha256"],
                "turn": turn, "frame": row.get("frame"), "gold": row.get("gold"),
                "gold_per_turn": row.get("gold_per_turn"), "science": row.get("science"),
                "fuel_stocks": {f: row.get("strategic_resources", {}).get(f) for f in FUELS},
                "fuel_income": {f: row.get("strategic_resource_income", {}).get(f) for f in FUELS},
                "cities": cities})
    return {"run": directory.name, "family": re.sub(r"-cont\d+$", "", directory.name),
            "started_utc": min(starts, key=stamp) if starts else None,
            "sources": sources, "runtime_updates": binaries, "genomes": genomes,
            "summary": summary, "milestones": milestones, "candidate_notes": note_fires,
            "buy_deal_events": deals, "readings": readings}


def census(root, armed_utc, count):
    # Inspect starts cheaply before reading potentially large host event logs.
    starts = {}
    for directory in sorted(root.glob("civvis-*")):
        path = directory / "runtime_updates.jsonl"
        if not path.exists():
            continue
        for line in path.read_bytes().splitlines(keepends=True):
            if not line.endswith(b"\n"):
                break
            row = json.loads(line)
            if row.get("status") == "start" and row.get("utc"):
                starts[directory] = row["utc"]
                break
    family_starts = {}
    for directory, start in starts.items():
        family = re.sub(r"-cont\d+$", "", directory.name)
        if family not in family_starts or stamp(start) < stamp(family_starts[family]):
            family_starts[family] = start
    runs = [audit_run(d) for d in starts
            if stamp(family_starts[re.sub(r"-cont\d+$", "", d.name)]) > stamp(armed_utc)]
    families = {}
    for run in runs:
        families.setdefault(run["family"], []).append(run)
    primary, other, pending = [], [], []
    for name, members in sorted(families.items(), key=lambda pair: stamp(family_starts[pair[0]])):
        configured = [r for r in members if r["summary"].get("configured") is True]
        family = {"family": name, "started_utc": family_starts[name], "runs": members}
        if not configured:
            pending.append(family)
        elif any(r["summary"].get("victory_target") == "science" for r in configured):
            primary.append(family)
        else:
            other.append(family)
    return {"schema": 1, "gene": GENE, "armed_utc": armed_utc,
            "primary_target": count, "primary_observed": len(primary),
            "primary": primary[:count], "primary_after_stop_boundary": primary[count:],
            "other_lanes": other, "pending_or_setup_attrition": pending,
            "unclassified_no_runtime_start": [d.name for d in sorted(root.glob("civvis-*"))
                if d not in starts and d.name[7:23] >= stamp(armed_utc).strftime("%Y%m%dT%H%M%S")],
            "interpretation": "Read-only execution census, not a strength estimate. A proposal, closable reply, or sent acceptance is not delivery. Numeric resource IDs are not guessed; all buy replies remain visible. Missing values are unknown. Whole-file hashes describe bytes read, not an immutable live file. Completed configured summaries define cohort membership; ongoing runs remain pending. No settings are written."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--armed-utc", required=True)
    parser.add_argument("--count", type=int, default=8)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    result = census(args.root, args.armed_utc, args.count)
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: result[k] for k in ("primary_target", "primary_observed")}))


if __name__ == "__main__":
    main()
