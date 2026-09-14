"""Fit a target-mixture proposal, not a Firaxis policy or a deployment change.

Games, not seats/turns, define the deterministic train/validation split. A
5% floor retains every objective. Validation cannot influence optimization.
The caller must gather comparable profiles and independent run identities.
"""
import collections
import hashlib
import json
import math
import statistics

TARGETS = ("civvis", "science", "culture", "religious", "diplomatic", "domination", "score")
METRICS = ("cities", "techs", "science", "culture", "military")
FLOOR = .05
PROFILE_KEYS = {"map", "width", "height", "players", "city_states", "ruleset", "modes", "native_competitions"}


def partition(run):
    return "validation" if int(hashlib.sha256(run.encode()).hexdigest()[:8], 16) % 3 == 0 else "train"


def matrix(rows, split):
    # Each independent game has equal weight within each feature/target,
    # regardless of how many rivals were met or how many chairs drew a goal.
    games = collections.defaultdict(list)
    for row in rows:
        if partition(row["run"]) != split:
            continue
        target = "live" if row["source"] == "live" else row["target"]
        for metric in METRICS:
            value = row["values"].get(metric)
            if isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and value >= 0:
                games[(row["cohort"][2], metric, target, row["run"])].append(math.log1p(value))
    features = collections.defaultdict(lambda: collections.defaultdict(list))
    for (turn, metric, target, _), values in games.items():
        features[(turn, metric)][target].append(statistics.mean(values))
    result = {}
    for key, targets in features.items():
        if all(len(targets.get(target, [])) >= 2 for target in ("live",) + TARGETS):
            result[key] = (statistics.mean(targets["live"]),
                           [statistics.mean(targets[target]) for target in TARGETS])
    return result


def loss(data, weights):
    return statistics.mean((sum(w * v for w, v in zip(weights, values)) - observed) ** 2
                           for observed, values in data.values())


def fit(samples):
    result = {"status": "insufficient_evidence", "production_policy_changed": False,
              "scope": "target-mixture pace proposal; not recovered Firaxis behavior",
              "split": "SHA256(independent game ID) mod 3; zero reserved for validation",
              "minimum_target_weight": FLOOR}
    builds = {row.get("model_build") for row in samples if row["source"] == "native"}
    if len(builds) != 1 or None in builds:
        result["reason"] = "one identified native binary is required per calibration"
        return result
    # Reloads and duplicated input paths cannot turn one game into evidence.
    unique = {}
    for row in samples:
        key = (row["source"], row["run"], row["seat"], tuple(row["cohort"]))
        unique[key] = row
    rows = list(unique.values())
    profiles = {json.dumps(row.get("profile"), sort_keys=True) for row in rows}
    if len(profiles) != 1 or not rows or not PROFILE_KEYS.issubset(rows[0].get("profile") or {}) or any(
            value is None or value == "unknown" for value in rows[0]["profile"].values()):
        result["reason"] = "complete matching map/rules/modes/competition profiles are required"
        return result
    if len({tuple(row["cohort"][:2]) for row in rows}) != 1:
        result["reason"] = "fit one speed/difficulty cohort at a time"
        return result
    counts = {f"{source}_{split}": len({r["run"] for r in rows
               if r["source"] == source and partition(r["run"]) == split})
              for source in ("live", "native") for split in ("train", "validation")}
    result["independent_games"] = counts
    if min(counts.values()) < 3:
        result["reason"] = "at least three independent games per source in each split are required"
        return result
    train = matrix(rows, "train")
    if len(train) < 10:
        result["reason"] = "training needs ten pace features, each with two games per target and live reference"
        return result
    # Frank-Wolfe on the floor-constrained simplex; exact line search, no
    # external optimizer, no fitted thresholds and no access to validation.
    weights = [1 / len(TARGETS)] * len(TARGETS)
    for _ in range(400):
        gradients = [0.] * len(TARGETS)
        for observed, values in train.values():
            residual = sum(w * v for w, v in zip(weights, values)) - observed
            for i, value in enumerate(values):
                gradients[i] += residual * value
        vertex = [FLOOR] * len(TARGETS)
        vertex[min(range(len(TARGETS)), key=lambda i: gradients[i])] += 1 - FLOOR * len(TARGETS)
        direction = [v - w for v, w in zip(vertex, weights)]
        numerator = denominator = 0.
        for observed, values in train.values():
            step = sum(d * v for d, v in zip(direction, values))
            residual = sum(w * v for w, v in zip(weights, values)) - observed
            numerator -= residual * step
            denominator += step * step
        gamma = min(1., max(0., numerator / denominator)) if denominator else 0.
        if gamma < 1e-12:
            break
        weights = [w + gamma * d for w, d in zip(weights, direction)]
    # Freeze the candidate before opening the held-out aggregate.
    validation = matrix(rows, "validation")
    common = set(train) & set(validation)
    if len(common) < 10:
        result["reason"] = "held-out feature coverage is insufficient"
        return result
    validation = {key: validation[key] for key in common}
    uniform = [1 / len(TARGETS)] * len(TARGETS)
    baseline, candidate = loss(validation, uniform), loss(validation, weights)
    result.update(training_features=len(train), validation_features=len(validation),
                  training_loss=loss(train, weights), validation_loss=candidate,
                  validation_uniform_loss=baseline, weights=dict(zip(TARGETS, weights)))
    result["status"] = "proposal" if candidate < baseline * .9 else "rejected_on_validation"
    result["reason"] = ("held-out log-pace error improved by over 10%; prospective tournament validation still required"
                        if result["status"] == "proposal" else "candidate did not clear the fixed held-out improvement gate")
    return result
