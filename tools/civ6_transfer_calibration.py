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
PROFILE_KEYS = ("map", "width", "height", "players", "city_states", "ruleset", "modes", "native_competitions")


def canonical_profile(profile):
    profile = {key: (profile or {}).get(key) for key in PROFILE_KEYS}
    if isinstance(profile["modes"], list) and all(isinstance(mode, str) for mode in profile["modes"]):
        profile["modes"] = sorted(set(profile["modes"]))
    return profile


def complete_profile(profile):
    return (all(isinstance(profile[key], str) and profile[key] not in ("", "unknown")
                for key in ("map", "ruleset"))
            and all(type(profile[key]) is int and profile[key] > 0 for key in ("width", "height", "players"))
            and type(profile["city_states"]) is int and profile["city_states"] >= 0
            and type(profile["native_competitions"]) is bool
            and isinstance(profile["modes"], list) and all(isinstance(mode, str) for mode in profile["modes"]))


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
            if rival.get("alive") is False:
                continue
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
                contracts.add((header.get("player_contract") or "legacy", header.get("target_mix")))
                if len(contracts) != 1:
                    raise ValueError("native inputs mix player contracts or target mixtures; calibrate each epoch separately")
                # A binary/seed pair is not a game identity: difficulty,
                # profile, contract and target mixture can all differ.
                header_id = hashlib.sha256(json.dumps(header, sort_keys=True, allow_nan=False).encode()).hexdigest()
                continue
            if row.get("kind") != "game": continue
            if header is None: raise ValueError(f"{file}: seat before header")
            for sample in row.get("trajectory", []):
                if sample.get("alive") is False:
                    continue
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
    unique = {}
    for sample in samples:
        profile = canonical_profile(sample.get("profile"))
        profile_id = json.dumps(profile, sort_keys=True, allow_nan=False)
        key = (sample["source"], sample["run"], sample["seat"], tuple(sample["cohort"]),
               profile_id, sample.get("model_build"))
        if key in unique:
            if sample["values"] != unique[key]["values"] or sample["target"] != unique[key]["target"]:
                raise ValueError("conflicting observations for one game/seat/turn; select one authoritative continuation")
            continue
        unique[key] = sample
        groups[(tuple(sample["cohort"]), profile_id)].append(sample)
    cohorts = []
    for (cohort, profile_id), rows in sorted(groups.items()):
        profile = json.loads(profile_id)
        live = [r for r in rows if r["source"] == "live"]
        native = [r for r in rows if r["source"] == "native"]
        builds = {r.get("model_build") for r in native}
        status = ("unmatched_cohort" if not live or not native else
                  "incomplete_profile" if not complete_profile(profile) else
                  "unidentified_or_mixed_native_builds" if len(builds) != 1 or None in builds else
                  "comparable_pace")
        metrics = {}
        for metric in METRICS:
            observed = [r["values"][metric] for r in live if metric in r["values"]]
            modeled = [r["values"][metric] for r in native if metric in r["values"]]
            metrics[metric] = {"live": quantiles(observed), "native": quantiles(modeled),
                               "median_gap": statistics.median(modeled) - statistics.median(observed)
                               if status == "comparable_pace" and observed and modeled else None}
        targets = {}
        for target in sorted({r["target"] for r in native}):
            targets[target] = {metric: quantiles([r["values"][metric] for r in native if r["target"] == target and metric in r["values"]]) for metric in METRICS}
        cohorts.append({"speed": cohort[0], "difficulty": cohort[1], "turn": cohort[2],
                        "profile": profile,
                        "live_runs": len({r["run"] for r in live}), "native_games": len({r["run"] for r in native}),
                        "status": status,
                        "metrics": metrics, "native_targets": targets})
    return {"schema": 2, "scope": "profile-matched public rival pace; not causal policy identification",
            "selection": "met live rivals; known eliminated seats excluded from pace only; older missing survival readings remain unknown; no winner-only filtering",
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
