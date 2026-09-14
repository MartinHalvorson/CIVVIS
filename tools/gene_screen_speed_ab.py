#!/usr/bin/env python3
"""Measure frozen gene_screen binaries with matched standard tournament games.

This uses the actual tournament controller and independently drawn seat genomes.
It measures performance, not AI strength. Keep the machine free of other builds,
simulations and profiling; report the same-binary controls alongside the result.
Each block supplies ONE process CPU/wall estimate even with multiple game workers.

Freeze clean developer-tools release builds as civvis-<full source commit>, then:
    python3 tools/gene_screen_speed_ab.py --baseline PATH --candidate PATH \
        --out-dir FRESH_DIRECTORY --seed 900001 --pairs 5
For concurrent throughput, set --games-per-arm and --jobs explicitly. Every
recorded outcome except secs must match; raw rows, logs and provenance are retained.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import resource
import re
import statistics
import time
import subprocess


def digest(value):
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)
    return hashlib.sha256(encoded.encode()).hexdigest()


def binary_record(path):
    path = path.resolve()
    commit = path.name.removeprefix("civvis-")
    if re.fullmatch(r"civvis-[0-9a-f]{40}", path.name) is None:
        raise ValueError("Freeze each clean build as civvis-<full source commit> first")
    return {"path": str(path), "commit": commit,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def run_arm(record, seed, label, directory, games, jobs):
    rows_path = directory / f"{label}-{seed}.jsonl"
    log_path = directory / f"{label}-{seed}.log"
    command = [record["path"], "--games", str(games), "--jobs", str(jobs),
               "--difficulty", "emperor", "--start-seed", str(seed),
               "--out", str(rows_path)]
    environment = os.environ.copy()
    environment.pop("CIVVIS_COMMIT", None)
    wall_start = time.monotonic()
    before = resource.getrusage(resource.RUSAGE_CHILDREN).ru_utime
    with log_path.open("x") as log:
        subprocess.run(command, env=environment, stdout=log,
                       stderr=subprocess.STDOUT, check=True)
    cpu = resource.getrusage(resource.RUSAGE_CHILDREN).ru_utime - before
    wall = time.monotonic() - wall_start
    return read_result(rows_path, record, seed, games, cpu, wall)


def read_result(rows_path, record, seed, games, cpu=0, wall=0):
    records = [json.loads(line) for line in rows_path.read_text().splitlines() if line]
    headers = [row for row in records if row["kind"] == "header"]
    rows = sorted((row for row in records if row["kind"] == "game"),
                  key=lambda row: (row["game"], row["seat"]))
    if not (len(headers) == 1 and len(rows) == 6 * games):
        raise ValueError("incomplete or unexpected screen")
    if not (len(records) == 1 + 6 * games):
        raise ValueError("unexpected record type")
    header = headers[0]
    expected_batch = {"target_games": games, "target_seats": 6 * games,
                      "seed_first": seed, "seed_last": seed + games - 1}
    if header.get("batch") != expected_batch:
        raise ValueError("declared batch does not match requested games and seeds")
    if header.get("players") != 6 or header.get("start_seed") != seed:
        raise ValueError("unexpected player count or start seed")
    build = header.pop("build")
    if not (build["commit"] == record["commit"] and not build["dirty"]):
        raise ValueError('Invalid tournament records')
    if not (build["binary_sha256"] == record["sha256"]):
        raise ValueError("binary changed since registration")
    turns = 0
    for game in range(games):
        seats = rows[game * 6:(game + 1) * 6]
        if not ([row["seat"] for row in seats] == list(range(6))):
            raise ValueError('Invalid tournament records')
        if not (all(row["seed"] == seed + game and row["game"] == game for row in seats)):
            raise ValueError('Invalid tournament records')
        game_turns = {row["turn"] for row in seats}
        if not (len(game_turns) == 1 and next(iter(game_turns)) > 0):
            raise ValueError('Invalid tournament records')
        turns += next(iter(game_turns))
    for row in rows:
        del row["secs"]  # The Row schema's sole runtime field; retain every outcome.
    return {"cpu": cpu, "wall": wall, "turns": turns, "header": header,
            "genes_sha256": build["genes_sha256"], "outcome_digest": digest(rows)}


def summarize(pairs):
    baseline = sum(pair["baseline_cpu"] for pair in pairs)
    candidate = sum(pair["candidate_cpu"] for pair in pairs)
    baseline_wall = sum(pair["baseline_wall"] for pair in pairs)
    candidate_wall = sum(pair["candidate_wall"] for pair in pairs)
    return {"pairs": len(pairs),
            "total_games_per_arm": sum(pair["games_per_arm"] for pair in pairs),
            "median_wall_pct": statistics.median(pair["wall_delta_pct"] for pair in pairs),
            "pooled_wall_pct": 100 * (candidate_wall / baseline_wall - 1),
            "baseline_wall": baseline_wall, "candidate_wall": candidate_wall, "turns_per_arm": sum(pair["turns"] for pair in pairs),
            "median_pct": statistics.median(pair["delta_pct"] for pair in pairs),
            "min_pct": min(pair["delta_pct"] for pair in pairs),
            "max_pct": max(pair["delta_pct"] for pair in pairs),
            "pooled_pct": 100 * (candidate / baseline - 1),
            "baseline_cpu": baseline, "candidate_cpu": candidate}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--seed", type=int, default=900001)
    parser.add_argument("--pairs", type=int, default=5, help="matched blocks, each measured as one process")
    parser.add_argument("--games-per-arm", type=int, default=1)
    parser.add_argument("--jobs", type=int, default=1)
    parser.add_argument("--controls", type=int, default=2)
    args = parser.parse_args()
    if min(args.pairs, args.controls, args.games_per_arm, args.jobs) <= 0:
        parser.error("pairs, controls, games-per-arm and jobs must be positive")
    if args.seed < args.controls * args.games_per_arm:
        parser.error("control seeds would be negative; increase --seed")
    baseline, candidate = binary_record(args.baseline), binary_record(args.candidate)
    args.out_dir.mkdir(parents=True, exist_ok=False)
    manifest = {"status": "running", "baseline": baseline, "candidate": candidate,
                "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                "pairs": args.pairs, "seeds": [args.seed, args.seed + args.pairs * args.games_per_arm - 1],
                "controls": args.controls,
                "control_seeds": [args.seed - args.controls * args.games_per_arm, args.seed - 1],
                "games_per_arm": args.games_per_arm, "jobs": args.jobs,
                "profile": "standard gene_screen, all genes drawn, Emperor, matched blocks",
                "measurement_unit": "One aggregate process CPU estimate per block, not per game",
                "load_start": os.getloadavg()[0]}
    manifest_path = args.out_dir / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    results = {"control": [], "candidate": []}
    try:
        for phase, count, first, treatment in [
            ("control", args.controls, args.seed - args.controls * args.games_per_arm, baseline),
            ("candidate", args.pairs, args.seed, candidate),
        ]:
            for index in range(count):
                seed = first + index * args.games_per_arm
                arms = [("baseline", baseline), ("candidate", treatment)]
                if index % 2:
                    arms.reverse()
                samples = {}
                for label, record in arms:
                    samples[label] = run_arm(record, seed, phase + "-" + label, args.out_dir, args.games_per_arm, args.jobs)
                before, after = samples["baseline"], samples["candidate"]
                if not (before["header"] == after["header"]):
                    raise ValueError(f"header mismatch: seed {seed}")
                if not (before["genes_sha256"] == after["genes_sha256"]):
                    raise ValueError('Invalid tournament records')
                if not (before["outcome_digest"] == after["outcome_digest"]):
                    raise ValueError(f"outcomes differ: seed {seed}")
                if not (before["turns"] == after["turns"]):
                    raise ValueError('Invalid tournament records')
                pair = {"phase": phase, "seed": seed, "turns": before["turns"],
                        "baseline_cpu": before["cpu"], "candidate_cpu": after["cpu"],
                        "delta_pct": 100 * (after["cpu"] / before["cpu"] - 1),
                        "baseline_wall": before["wall"], "candidate_wall": after["wall"],
                        "wall_delta_pct": 100 * (after["wall"] / before["wall"] - 1),
                        "games_per_arm": args.games_per_arm,
                        "outcome_digest": before["outcome_digest"],
                        "load": os.getloadavg()[0]}
                results[phase].append(pair)
                with (args.out_dir / "pairs.jsonl").open("a") as log:
                    log.write(json.dumps(pair) + "\n")
                print(f"{phase} {index + 1}/{count}: seed {seed}, {pair['turns']} turns, "
                      f"{pair['delta_pct']:+.2f}% CPU, {pair['wall_delta_pct']:+.2f}% wall, "
                      "identical recorded outcomes", flush=True)
        manifest.update(status="complete", summary={key: summarize(rows) for key, rows in results.items()})
        print(json.dumps(manifest["summary"], indent=2), flush=True)
    except BaseException as error:
        manifest.update(status="failed", error=repr(error))
        raise
    finally:
        manifest["load_end"] = os.getloadavg()[0]
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
