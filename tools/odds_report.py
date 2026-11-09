#!/usr/bin/env python3
"""Score pre-outcome odds_export JSONL forecasts, keeping uncertainty by game.

Usage: python3 tools/odds_report.py run.jsonl [run2.jsonl ...]

Brier is the mean squared error over seats; log loss is on the actual winner.
Phases use the latest observation at or before the requested clock fraction.
Finished games never contribute a post-outcome forecast. A game that ended
before a phase is absent at that phase, not carried forward with perfect odds.
The same seed in two profiles is rejected: its map is not an independent draw.
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import random
import statistics


def load(paths):
    games = []
    seeds = set()
    for path in paths:
        with Path(path).open() as source:
            for line_number, line in enumerate(source, 1):
                game = json.loads(line)
                if game.get("schema") != "civvis-odds-v1":
                    raise ValueError(f"{path}:{line_number}: unknown odds schema")
                if game["seed"] in seeds:
                    raise ValueError(f"duplicate seed {game['seed']}; keep maps independent")
                seeds.add(game["seed"])
                if not game["winners"]:
                    raise ValueError(f"seed {game['seed']} has no terminal outcome")
                if len(game["winners"]) != 1:
                    raise ValueError("this report requires free-for-all games")
                turns = [sample["turn"] for sample in game["samples"]]
                if turns != sorted(set(turns)):
                    raise ValueError("sample turns must be unique and increasing")
                if turns and turns[-1] > game["end_turn"]:
                    raise ValueError("a forecast cannot follow its outcome turn")
                for sample in game["samples"]:
                    if sample.get("terminal") is not False:
                        raise ValueError("terminal or unmarked snapshots are not forecasts")
                    seats = sample["seats"]
                    if len(seats) != game["players"] or len({s["pid"] for s in seats}) != len(seats):
                        raise ValueError("one forecast is required per major seat")
                    if game["winners"][0] not in {seat["pid"] for seat in seats}:
                        raise ValueError("winner is absent from the forecast")
                    for field in ("start", "now"):
                        validate_probabilities([seat[field] for seat in seats])
                games.append(game)
    if not games:
        raise ValueError("no completed games")
    return games


def validate_probabilities(values):
    if not values or any(not math.isfinite(p) or not 0.0 <= p <= 1.0 for p in values):
        raise ValueError("probabilities must be finite and between zero and one")
    if not math.isclose(sum(values), 1.0, abs_tol=1e-8):
        raise ValueError("free-for-all probabilities must sum to one")


def score(probabilities, pids, winner):
    validate_probabilities(probabilities)
    if winner not in pids or len(probabilities) != len(pids):
        raise ValueError("probabilities and outcomes do not identify the same seats")
    brier = statistics.mean((p - (pid == winner)) ** 2 for pid, p in zip(pids, probabilities))
    loss = -math.log(max(probabilities[pids.index(winner)], 1e-12))
    # The lowest seat breaks equal-probability ties, as the game does.
    favorite = max(zip(pids, probabilities), key=lambda row: (row[1], -row[0]))[0]
    return brier, loss, float(favorite == winner)


def at_phase(game, phase):
    turn = game["max_turns"] * phase
    if game["end_turn"] < turn:
        return None
    return next((s for s in reversed(game["samples"]) if s["turn"] <= turn), None)


def interval(values, repeats=2000):
    """Paired bootstrap of game-level differences, not of correlated seats."""
    if not values:
        return None
    rng = random.Random(0)
    estimates = sorted(statistics.mean(rng.choices(values, k=len(values))) for _ in range(repeats))
    return [estimates[int(repeats * .025)], estimates[min(repeats - 1, int(repeats * .975))]]


def report(games, predict=None, phases=(.25, .5, .75, .9),
           baseline_field="now", candidate_field="start"):
    result = []
    for phase in phases:
        rows = []
        for game in games:
            sample = at_phase(game, phase)
            if sample is None:
                continue
            pids = [seat["pid"] for seat in sample["seats"]]
            winner = game["winners"][0]
            baseline = score([seat[baseline_field] for seat in sample["seats"]], pids, winner)
            candidate = score(predict(game, sample) if predict else
                              [seat[candidate_field] for seat in sample["seats"]], pids, winner)
            rows.append((baseline, candidate))
        if not rows:
            continue
        metrics = {}
        for index, name in enumerate(("brier", "log_loss", "favorite_accuracy")):
            delta = [candidate[index] - baseline[index] for baseline, candidate in rows]
            metrics[name] = {
                baseline_field: statistics.mean(row[0][index] for row in rows),
                "candidate" if predict else candidate_field: statistics.mean(row[1][index] for row in rows),
                "difference": statistics.mean(delta),
                "difference_95": interval(delta),
            }
        result.append({"phase": phase, "games": len(rows), "metrics": metrics})
    return {"games": len(games), "phases": result}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+")
    parser.add_argument("--baseline", default="now")
    parser.add_argument("--candidate", default="start")
    parser.add_argument("--phases", default=".25,.5,.75,.9")
    args = parser.parse_args()
    phases = [float(value) for value in args.phases.split(",")]
    if not phases or any(not 0 < value < 1 for value in phases):
        parser.error("phases must lie between zero and one")
    print(json.dumps(report(load(args.paths), phases=phases,
                            baseline_field=args.baseline, candidate_field=args.candidate), indent=2))


if __name__ == "__main__":
    main()
