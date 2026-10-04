"""Summarize paired capacity games without discarding early endings."""

import argparse
import csv
import hashlib
import json
import math
import statistics
from pathlib import Path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def summarize(manifest, phase, csv_root, raw):
    results = []
    hashes = {}
    checks = []
    thresholds = manifest["thresholds"]
    for difficulty, seeds in manifest[phase].items():
        arms = {}
        outcomes = {}
        for arm in ("control", "candidate"):
            path = csv_root / f"2026-10-04-production-growth-capacity-{phase}-{arm}-{difficulty}.csv"
            hashes[path.name] = sha(path)
            with path.open() as stream:
                rows = list(csv.DictReader(stream))
            assert {int(r["seed"]) for r in rows} <= set(seeds)
            indexed = {}
            for row in rows:
                key = (int(row["seed"]), int(row["turn"]))
                # Every row is a checkpoint. Final outcomes come from worlds,
                # even when a game ends before its first checkpoint.
                assert not row["winner"]
                assert key not in indexed, key
                assert key[1] in (25, 50, 75, 100, 125, 150)
                for field, value in row.items():
                    if field not in ("alive", "winner"):
                        assert math.isfinite(float(value)), (field, value)
                indexed[key] = row
            arms[arm] = indexed
            outcomes[arm] = []
            for seed in seeds:
                prefix = f"civvis-production-growth-capacity-{difficulty}-{seed}-{arm}"
                paths = {suffix: raw / f"{prefix}-{suffix}" for suffix in (
                    "setup.json", "cities.jsonl", "actions.json", "final.json"
                )}
                hashes.update({p.name: sha(p) for p in paths.values()})
                setup = json.loads(paths["setup.json"].read_text())
                assert setup["seed"] == seed and setup["arm"] == arm
                assert setup["difficulty"] == difficulty
                assert setup["wide_map_capacity"] is True
                assert "wide-map-capacity" in setup["deployment_tags"]
                assert setup["actual_Firaxis_verification"] is False
                world = json.loads(paths["final.json"].read_text())
                actual_turns = {turn for s, turn in indexed if s == seed}
                assert all(turn <= world["turn"] for turn in actual_turns)
                # A win at the turn boundary can prevent that turn's observer.
                # Earlier checkpoints must exist; a truncated CSV is not an
                # early-ending game and must not silently improve the cohort.
                expected_earlier = {turn for turn in (25, 50, 75, 100, 125, 150)
                                    if turn < world["turn"]}
                assert expected_earlier <= actual_turns, (seed, expected_earlier, actual_turns)
                outcomes[arm].append({
                    "seed": seed, "last_turn": world["turn"],
                    "alive": world["players"][0]["alive"], "winner": world["winner"],
                })
        checkpoints = []
        for turn in (25, 50, 75, 100, 125, 150):
            present = {arm: {seed for seed in seeds if (seed, turn) in arms[arm]}
                       for arm in arms}
            matched = sorted(present["control"] & present["candidate"])
            fields = ("production", "cumulative_production", "science", "culture",
                      "population", "cities", "military_power", "granaries")
            means = {arm: {field: statistics.mean(float(arms[arm][seed, turn][field])
                     for seed in matched) if matched else None for field in fields}
                     for arm in arms}
            changes = {field: means["candidate"][field] / means["control"][field] - 1
                       if matched and means["control"][field] > 0 else None
                       for field in fields}
            ratios = {arm: statistics.mean(
                float(arms[arm][seed, turn]["production"]) /
                float(arms[arm][seed, turn]["rival_production"]) for seed in matched
            ) if matched else None for arm in arms}
            checkpoints.append({
                "turn": turn, "matched_seeds": matched,
                "missing_seeds": {arm: sorted(set(seeds) - present[arm]) for arm in arms},
                "matched_means": means, "relative_change": changes,
                "matched_mean_production_ratio": ratios,
            })
            if turn in (75, 100):
                checks.append({"difficulty": difficulty, "turn": turn,
                               "check": "matched_coverage",
                               "pass": len(matched) >= math.ceil(len(seeds) * thresholds["minimum_matched_fraction"])})
                floors = thresholds[f"t{turn}"][difficulty]
                for field, floor in floors.items():
                    value = changes[field]
                    checks.append({"difficulty": difficulty, "turn": turn,
                                   "check": field, "floor": floor, "actual": value,
                                   "pass": value is not None and value >= floor})
        eliminations = {arm: sum(not o["alive"] for o in outcomes[arm]) for arm in arms}
        checks.append({"difficulty": difficulty, "check": "no_additional_eliminations",
                       "pass": eliminations["candidate"] <= eliminations["control"]})
        changed = [seed for seed in seeds if (
            raw / f"civvis-production-growth-capacity-{difficulty}-{seed}-candidate-actions.json"
        ).read_bytes() != (
            raw / f"civvis-production-growth-capacity-{difficulty}-{seed}-control-actions.json"
        ).read_bytes()]
        checks.append({"difficulty": difficulty, "check": "changed_actions", "pass": bool(changed)})
        results.append({"difficulty": difficulty, "seeds": seeds,
                        "changed_action_seeds": changed, "outcomes": outcomes,
                        "eliminations": eliminations,
                        "wins": {arm: sum(o["winner"] == 0 for o in outcomes[arm]) for arm in arms},
                        "checkpoints": checkpoints})
    return {"phase": phase, "paired_games": sum(len(seeds) for seeds in manifest[phase].values()),
            "scope": "Native simulation, focal public live bridge and fixed stock rivals; no actual Firaxis verification or high-level parity claim.",
            "results": results, "checks": checks, "passes_numeric_screen": all(c["pass"] for c in checks),
            "sha256": hashes}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--phase", choices=("pilot", "confirmation"), required=True)
    parser.add_argument("--csv-root", type=Path, required=True)
    parser.add_argument("--raw", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    probe = args.csv_root / "2026-10-04-production-growth-capacity-probe.rs"
    assert sha(probe) == manifest["probe_sha256"]
    result = summarize(manifest, args.phase, args.csv_root, args.raw)
    result["manifest_sha256"] = sha(args.manifest)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"phase": args.phase, "pairs": result["paired_games"],
                      "passes_numeric_screen": result["passes_numeric_screen"]}))


if __name__ == "__main__":
    main()
