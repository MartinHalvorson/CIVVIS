"""Audit consumed-seed controller setup and summarize every recorded outcome."""

import argparse
import csv
import hashlib
import json
import statistics
from pathlib import Path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--raw", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    directory = Path(__file__).parent
    stem = "2026-10-04-production-growth-ledger"
    manifest = json.loads((directory / f"{stem}-v2-manifest.json").read_text())
    assert sha(directory / f"{stem}-probe-v2.rs") == manifest["probe_sha256"]
    old_manifest = json.loads((directory / f"{stem}-manifest.json").read_text())
    assert sha(directory / f"{stem}-probe.rs") == old_manifest["probe_sha256"]
    setups = []
    summaries = []
    city_records = []
    expected_files = set()
    for difficulty in ("emperor", "deity"):
        seeds = manifest[f"{difficulty}_seeds"]
        for arm in ("stock", "live", "native"):
            csv_name = f"{stem}-{difficulty}.csv" if arm == "stock" else f"{stem}-v2-{arm}-{difficulty}.csv"
            with (directory / csv_name).open() as stream:
                rows = list(csv.DictReader(stream))
            outcomes = []
            for seed in seeds:
                if arm == "stock":
                    root = args.reference
                    prefix = f"civvis-production-growth-ledger-{difficulty}-{seed}"
                    world_path = root / f"{prefix}-diagnostic-final.json"
                    detail_path = root / f"{prefix}-cities.jsonl"
                else:
                    root = args.raw
                    prefix = f"civvis-production-growth-ledger-v2-{difficulty}-{seed}-{arm}"
                    world_path = root / f"{prefix}-final.json"
                    detail_path = root / f"{prefix}-cities.jsonl"
                    setup_path = root / f"{prefix}-setup.json"
                    setup = json.loads(setup_path.read_text())
                    expected = [True] * 4 if arm == "native" else [True, False, False, False]
                    assert [p["pid"] for p in setup["flags"]] == list(range(4))
                    assert [p["wide_map_capacity"] for p in setup["flags"]] == expected
                    assert "wide-map-capacity" in setup["deployment_tags"]
                    setups.append({"difficulty": difficulty, "seed": seed, "arm": arm, "wide_map_capacity": expected})
                    expected_files.update(f"{prefix}-{suffix}" for suffix in ("final.json", "actions.json", "cities.jsonl", "setup.json"))
                world = json.loads(world_path.read_text())
                outcomes.append({"seed": seed, "last_turn": world["turn"], "winner": world["winner"], "alive": world["players"][0]["alive"]})
                for line in detail_path.read_text().splitlines():
                    record = json.loads(line)
                    player = record["players"][0]
                    cities = player["cities"]
                    city_records.append({
                        "difficulty": difficulty, "arm": arm, "seed": seed, "turn": record["turn"],
                        "cities": len(cities), "population": sum(c["pop"] for c in cities),
                        "production": sum(c["yields"]["production"] for c in cities),
                        "gold": player["gold"], "income": player["income"],
                        "government": player.get("government"), "policies": player.get("policies"),
                        "contracted_gpt": player.get("contracted_gpt"),
                        "native_unit_maintenance": player.get("native_unit_maintenance"),
                        "worked_bare_production_gain": sum(max([i["gain"]["production"] for i in t["legal_improvements"]] + [0]) for c in cities for t in c["worked_jobs"]),
                        "plus_three_production_gain": sum(c["production_weight_counterfactual"]["production"] - c["yields"]["production"] for c in cities),
                        "plus_three_food_gain": sum(c["food_weight_counterfactual"]["food"] - c["yields"]["food"] for c in cities),
                        "plus_three_food_production_change": sum(c["food_weight_counterfactual"]["production"] - c["yields"]["production"] for c in cities),
                        "housing_bound": sum(c["housing"] - c["pop"] <= 1 for c in cities),
                        "housing_bound_without_granary": sum(c["housing"] - c["pop"] <= 1 and "granary" not in c["buildings"] for c in cities),
                        "amenity_short": sum(c["amenity_surplus"] < 0 for c in cities),
                    })
            checkpoints = []
            for turn in (25, 50, 75, 100, 125, 150):
                checkpoint = [r for r in rows if int(r["turn"]) == turn]
                present = [int(r["seed"]) for r in checkpoint]
                assert len(present) == len(set(present)) and set(present) <= set(seeds)
                checkpoints.append({
                    "turn": turn, "games": len(checkpoint),
                    "missing_seeds": sorted(set(seeds) - set(present)),
                    "mean": {key: statistics.mean(float(r[key]) for r in checkpoint) if checkpoint else None for key in ("production", "rival_production", "cumulative_production", "population", "cities", "science", "culture")},
                    "mean_production_ratio": statistics.mean(float(r["production"]) / float(r["rival_production"]) for r in checkpoint) if checkpoint else None,
                })
            summaries.append({"difficulty": difficulty, "arm": arm, "games": len(seeds), "wins": sum(r["winner"] == 0 for r in outcomes), "eliminations": sum(not r["alive"] for r in outcomes), "outcomes": outcomes, "checkpoints": checkpoints})
    prefix = "civvis-production-growth-ledger-v2-deity-61007100-stock"
    expected_files.update(f"{prefix}-{suffix}" for suffix in ("final.json", "actions.json", "cities.jsonl", "setup.json"))
    setup = json.loads((args.raw / f"{prefix}-setup.json").read_text())
    assert not any(p["wide_map_capacity"] for p in setup["flags"])
    assert "wide-map-capacity" in setup["deployment_tags"]
    for kind in ("actions", "final"):
        assert (args.raw / f"{prefix}-{kind}.json").read_bytes() == (args.reference / f"civvis-production-growth-ledger-deity-61007100-diagnostic-{kind}.json").read_bytes()
    assert len(expected_files) == 68
    assert set(p.name for p in args.raw.iterdir() if p.is_file()) == expected_files
    result = {
        "scope": "Consumed-seed setup audit; no treatment-strength inference and no actual Firaxis verification.",
        "new_diagnostic_executions": 17, "fresh_strength_samples": 0,
        "stock_observer_identical_actions": len(json.loads((args.raw / f"{prefix}-actions.json").read_text())),
        "setup_readbacks": setups,
        "summaries": summaries, "city_records": city_records,
        "durable_evidence": str(args.raw), "reference_evidence": str(args.reference),
        "sha256": {name: sha(args.raw / name) for name in sorted(expected_files)},
        "limitations": "Stock is the prior ledger-only subject and stock rivals. Live changes the focal bundle and uses fog-honest planning with stock rivals. Native enables deployment and authoritative planning for all majors; it changes opponents too. Arm differences are not isolated policy effects. Missing checkpoints are retained, not filled. Government and deal diagnostics are absent from original stock snapshots; the exact extra stock replay provides them for one seed.",
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(f"PASS: 17 setup-audit executions, 16 live/native flag readbacks, {result['stock_observer_identical_actions']} identical stock actions; zero fresh strength samples")
    for summary in summaries:
        point = next(c for c in summary["checkpoints"] if c["turn"] == 75)
        print(summary["difficulty"], summary["arm"], "T75", point["games"], "P", round(point["mean"]["production"], 3), "ratio", round(point["mean_production_ratio"], 3), "wins", summary["wins"], "eliminations", summary["eliminations"])


if __name__ == "__main__":
    main()
