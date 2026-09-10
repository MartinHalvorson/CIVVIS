#!/usr/bin/env python3
"""Measure opponent pace from public live observations and all tournament seats.

Usage: python3 tools/civ6_transfer_calibration.py --live RUN [RUN ...]
       --native ROWS.jsonl [ROWS.jsonl ...]

Reports observed rivals, not just winners. Missing/unmet rivals are coverage
gaps, not zero-strength opponents. Repeated combat frames do not increase a
turn's weight. Different speeds/difficulties remain separate cohorts; this
tool never silently fits away a handicap or changes production policy.
"""
from __future__ import annotations

import argparse
import collections
import hashlib
import json
import math
import statistics
import re
from pathlib import Path

METRICS = ("cities", "techs", "science", "culture", "military")


def finite(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and value >= 0


def normalized(value, prefix):
    return str(value or "unknown").lower().removeprefix(prefix.lower())


def quantiles(values):
    values = sorted(values)
    def at(q):
        point = (len(values) - 1) * q
        lo, hi = math.floor(point), math.ceil(point)
        return values[lo] + (values[hi] - values[lo]) * (point - lo)
    return {"n": len(values), "p10": at(.1), "median": at(.5), "p90": at(.9)} if values else {"n": 0}


def records(file):
    with Path(file).open(encoding="utf-8") as stream:
        for number, line in enumerate(stream, 1):
            if not line.strip():
                continue
            try:
                yield json.loads(line)
            except ValueError as exc:
                raise ValueError(f"{file}:{number}: invalid JSON") from exc


def live_samples(directory):
    directory = Path(directory)
    summary = json.loads((directory / "summary.json").read_text())
    if summary.get("configured") is not True:
        raise ValueError(f"{directory}: setup was not verified")
    if summary.get("isolated_action_probes") is True:
        raise ValueError(f"{directory}: deliberately perturbed diagnostic game is not calibration evidence")
    speed = normalized(summary.get("speed"), "GAMESPEED_")
    difficulty = normalized(summary.get("difficulty"), "DIFFICULTY_")
    # Last opening-frame export wins on a reload. Mid-turn frames never count
    # as additional independent observations.
    frames = {}
    dimensions = (None, None)
    for event in records(directory / "events.jsonl"):
        if event.get("kind") == "tiles":
            dimensions = (event.get("width", dimensions[0]), event.get("height", dimensions[1]))
        turn = event.get("turn")
        if event.get("kind") == "state" and isinstance(turn, int) and turn % 25 == 0 and event.get("frame", 0) == 0:
            frames[turn] = event
    for turn, event in sorted(frames.items()):
        for rival in event.get("rivals", []):
            stats = rival.get("public_stats") or {}
            if not isinstance(stats, dict): stats = {}
            values = {"cities": stats.get("city_count"), "techs": rival.get("techs_researched", rival.get("techs")),
                      "science": rival.get("science"), "culture": rival.get("culture"), "military": rival.get("military")}
            seat = summary.get("seat") or {}
            profile = {"map": str(seat.get("map", "unknown")).lower().removesuffix(".lua"),
                       "width": dimensions[0], "height": dimensions[1], "players": seat.get("players"),
                       "city_states": seat.get("city_states"), "ruleset": seat.get("ruleset"),
                       "modes": seat.get("modes"), "native_competitions": True}
            yield {"cohort": (speed, difficulty, turn), "source": "live", "profile": profile,
                   "run": re.sub(r"-cont\d+$", "", directory.name), "seat": rival.get("player"), "target": "observed_firaxis",
                   "values": {k: v for k, v in values.items() if finite(v)}}


def native_samples(files):
    seen = set()
    contracts = set()
    for file in files:
        header = None
        for row in records(file):
            if row.get("kind") == "header":
                header = row
                contracts.add(header.get("player_contract") or "legacy")
                if len(contracts) != 1:
                    raise ValueError("native inputs mix player contracts; calibrate each epoch separately")
                # A binary/seed pair is not a game identity: difficulty,
                # profile, contract and target mixture can all differ.
                header_id = hashlib.sha256(json.dumps(header, sort_keys=True, allow_nan=False).encode()).hexdigest()
                continue
            if row.get("kind") != "game": continue
            if header is None: raise ValueError(f"{file}: seat before header")
            for sample in row.get("trajectory", []):
                key = (header_id, row.get("difficulty"), row["seed"], row["seat"],
                       row.get("player_target"), sample["turn"])
                if key in seen: continue
                seen.add(key)
                yield {"cohort": (header.get("speed", "unknown"), row.get("difficulty") or header.get("difficulty") or "prince", sample["turn"]),
                       "profile": {"map": header.get("map"), "width": header.get("width"),
                                   "height": header.get("height"), "players": header.get("players"),
                                   "city_states": header.get("city_states"), "ruleset": "RULESET_EXPANSION_2",
                                   "modes": [], "native_competitions": header.get("native_competitions", False)},
                       "source": "native", "model_build": header.get("build", {}).get("binary_sha256"),
                       # Split repeated worlds together even across binaries;
                       # otherwise a rebuild could move a training map into validation.
                       "run": json.dumps([header.get("map"), header.get("width"), header.get("height"),
                                          header.get("speed"), row.get("difficulty") or header.get("difficulty"), row["seed"]]),
                       "seat": row["seat"],
                       "target": row.get("player_target") or "unrecorded",
                       "values": {k: sample[k] for k in METRICS if finite(sample.get(k))}}


def summarize(samples):
    groups = collections.defaultdict(list)
    for sample in samples:
        groups[tuple(sample["cohort"])].append(sample)
    cohorts = []
    for cohort, rows in sorted(groups.items()):
        live = [r for r in rows if r["source"] == "live"]
        native = [r for r in rows if r["source"] == "native"]
        metrics = {}
        for metric in METRICS:
            observed = [r["values"][metric] for r in live if metric in r["values"]]
            modeled = [r["values"][metric] for r in native if metric in r["values"]]
            metrics[metric] = {"live": quantiles(observed), "native": quantiles(modeled),
                               "median_gap": statistics.median(modeled) - statistics.median(observed) if observed and modeled else None}
        targets = {}
        for target in sorted({r["target"] for r in native}):
            targets[target] = {metric: quantiles([r["values"][metric] for r in native if r["target"] == target and metric in r["values"]]) for metric in METRICS}
        cohorts.append({"speed": cohort[0], "difficulty": cohort[1], "turn": cohort[2],
                        "live_runs": len({r["run"] for r in live}), "native_games": len({r["run"] for r in native}),
                        "status": "comparable_pace" if live and native else "unmatched_cohort",
                        "metrics": metrics, "native_targets": targets})
    return {"schema": 1, "scope": "public rival pace; not causal policy identification",
            "selection": "met rivals only; surviving seats at each checkpoint; no winner-only filtering",
            "production_policy_changed": False, "cohorts": cohorts}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live", nargs="+", required=True, type=Path)
    parser.add_argument("--native", nargs="*", default=[], type=Path)
    parser.add_argument("--fit-target-mix", action="store_true", help="fit a coverage-constrained prior on training games and evaluate on untouched games; never changes production")
    args = parser.parse_args()
    samples = [sample for directory in args.live for sample in live_samples(directory)]
    samples.extend(native_samples(args.native))
    report = summarize(samples)
    if args.fit_target_mix:
        from civ6_target_mix import fit
        report["target_mix_calibration"] = fit(samples)
    print(json.dumps(report, indent=2, allow_nan=False))
    return 0 if samples else 2


if __name__ == "__main__":
    raise SystemExit(main())
