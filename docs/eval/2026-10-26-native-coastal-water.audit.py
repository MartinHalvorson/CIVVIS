"""Inspect fixed captured Native reads; candidate rules remain hypotheses."""
import argparse
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import sys

DIRS = ((1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1))


def neighbors(pos, width, convention="floor"):
    x, y = pos
    shift = lambda r: (r - (r & 1)) // 2 if convention == "floor" else (r + (r & 1)) // 2
    q = x - shift(y)
    return [((q + dq + shift(y + dr)) % width, y + dr) for dq, dr in DIRS]


def distance(a, b, width, convention="floor"):
    shift = lambda r: (r - (r & 1)) // 2 if convention == "floor" else (r + (r & 1)) // 2
    aq, ar = a[0] - shift(a[1]), a[1]
    bq, br = b[0] - shift(b[1]), b[1]
    return min(max(abs(aq - bq + wrap), abs(ar - br), abs(aq - bq + wrap + ar - br))
               for wrap in (-width, 0, width))


def explicit_plots(rows):
    plots = {}
    for index, row in enumerate(rows):
        if row.get("kind") == "tiles":
            for plot in row.get("plots", []):
                plots[(plot["x"], plot["y"])] = {"plot": plot, "row": index,
                    "turn": row.get("turn"), "frame": row.get("frame")}
    return plots


def classify(pos, width, plots, convention, exclude_ice):
    adjacent = neighbors(pos, width, convention)
    missing = [list(n) for n in adjacent if n not in plots]
    waters = [list(n) for n in adjacent if n in plots
              and plots[n]["plot"].get("t") in ("TERRAIN_COAST", "TERRAIN_OCEAN")
              and (not exclude_ice or plots[n]["plot"].get("f") != "FEATURE_ICE")]
    return {"neighbors": [list(n) for n in adjacent], "missing": missing,
            "qualifying_water": waters,
            "coastal": True if waters else None if missing else False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--census", type=Path, required=True)
    parser.add_argument("--dumps", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    sys.path.insert(0, str(args.repo / "tools"))
    from civ6_decision_trace import captured_input
    prefix = args.repo / "docs/eval/2026-10-26-native-opening-model-drift"
    selection_path = prefix.with_suffix(".selection.json")
    selection = json.loads(selection_path.read_bytes())
    digest = lambda data: hashlib.sha256(data).hexdigest()
    census_bytes = args.census.read_bytes()
    if digest(census_bytes) != selection["census_artifact"]["member_sha256"]:
        raise ValueError("census identity differs")
    census = json.loads(gzip.decompress(census_bytes))
    result = {"scope": __doc__, "source_sha256": digest(Path(__file__).read_bytes()),
              "selection_sha256": digest(selection_path.read_bytes()),
              "census_sha256": digest(census_bytes), "reads": [], "geometry": []}
    geometry = {}
    for index, spec in enumerate(selection["reads"]):
        case = census["cases"][spec["case_index"]]
        snapshot_id = spec["snapshot"]["snapshot_id"]
        data = captured_input(case["selected"]["decision"]["input_capture"], snapshot_id)
        if len(data) != spec["input_bytes"] or digest(data) != spec["input_sha256"]:
            raise ValueError("captured input identity differs")
        rows = [json.loads(line) for line in data.splitlines()]
        plots = explicit_plots(rows)
        dump_bytes = (args.dumps / f"{index:04d}" / "stdout").read_bytes()
        dump = json.loads(dump_bytes)
        variant = next(v for v in case["variants"] if v["descriptor"]["snapshot_id"] == snapshot_id)
        reading = {"index": index, "stem": spec["stem"], "turn": spec["turn"],
                   "snapshot_id": snapshot_id, "input_sha256": digest(data),
                   "dump_sha256": digest(dump_bytes), "cities": []}
        for city in variant["state"].get("cities", []):
            pos = (city["x"], city["y"])
            center = plots.get(pos)
            models = [m for m in dump["cities"] if (m["x"], m["y"]) == pos]
            if len(models) != 1:
                raise ValueError("model city match not unique")
            entry = {"id": city.get("id"), "name": city.get("name"), "position": list(pos),
                     "native_water_housing": city.get("housing_from_water"),
                     "model_water_housing": models[0].get("model_housing_sources", {}).get("water"),
                     "center": center, "rules": {}}
            for convention in ("floor", "ceil"):
                for exclude_ice in (False, True):
                    key = convention + ("_no_ice" if exclude_ice else "_all_water")
                    entry["rules"][key] = classify(pos, dump["width"], plots, convention, exclude_ice)
            relevant = {tuple(n) for r in entry["rules"].values() for n in r["neighbors"]}
            entry["adjacent_plot_records"] = [plots[n] for n in sorted(relevant) if n in plots]
            reading["cities"].append(entry)
        for row_index, row in enumerate(rows):
            kind = row.get("kind")
            if kind == "settler_founding_shelter_allowed":
                a, b, expected = row.get("from"), row.get("site"), 1
            elif kind == "move_fallback":
                a, b, expected = row.get("sent"), row.get("want"), row.get("distance")
            else:
                continue
            if not (isinstance(a, list) and len(a) == 2 and isinstance(b, list) and len(b) == 2
                    and type(expected) is int):
                raise ValueError("malformed quoted geometry event")
            key = (spec["stem"], row_index)
            geometry[key] = {"stem": spec["stem"], "row": row_index, "event": row,
                             "expected_distance": expected,
                             "floor_distance": distance(a, b, dump["width"], "floor"),
                             "ceil_distance": distance(a, b, dump["width"], "ceil")}
        result["reads"].append(reading)
    result["geometry"] = list(geometry.values())
    cities = [c for r in result["reads"] for c in r["cities"]]
    eligible = [c for c in cities if c["center"] is not None
                and c["center"]["plot"].get("fw") is False and c["native_water_housing"] in (2, 3)]
    def summarize(key):
        complete = [c for c in eligible if c["rules"][key]["coastal"] is not None]
        return {"eligible_reads": len(eligible), "known_classifications": len(complete),
                "matches": sum((3 if c["rules"][key]["coastal"] else 2) == c["native_water_housing"] for c in complete),
                "mismatches": [{"name": c["name"], "position": c["position"],
                    "native": c["native_water_housing"], "coastal": c["rules"][key]["coastal"]}
                    for c in complete if (3 if c["rules"][key]["coastal"] else 2) != c["native_water_housing"]]}
    result["summary"] = {"reads": len(result["reads"]), "city_readings": len(cities),
        "unique_city_times": len({(r["stem"], r["turn"], c["id"]) for r in result["reads"] for c in r["cities"]}),
        "rules": {key: summarize(key) for key in cities[0]["rules"]},
        "geometry_events": len(geometry), "geometry_match_counts": {
            convention: sum(g[convention + "_distance"] == g["expected_distance"] for g in geometry.values())
            for convention in ("floor", "ceil")},
        "native_water_values": dict(Counter(str(c["native_water_housing"]) for c in cities))}
    args.out.write_text(json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + "\n")
    print(json.dumps(result["summary"], indent=2))


if __name__ == "__main__":
    main()
