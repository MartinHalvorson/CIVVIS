#!/usr/bin/env python3
"""Fit a replica-aware, action-conditioned ranker with a fixed abstainer.

The counterfactual emitter gives every candidate four matched doctrine returns.
This trainer keeps that structure instead of collapsing it to one scalar:
each unordered candidate pair becomes a logistic example whose target is a
Jeffreys posterior over the four observed wins, losses, and ties.  Splits are
by independent game, never by row or pair.

The output is an optional ``civvis-action-policy-v1`` artifact.  It is not
installed into the shipped tree.  A weak held-out fit, zero-coverage
abstainer, or negative gated lift refuses to write the artifact unless the
operator explicitly asks for a diagnostic with ``--allow-nonimproving``.
The Rust loader and ``action_conditioned_eval.py`` both enforce the artifact's
declared ``min_probability``; a later command cannot tune that threshold on
the profile it reports.

Example (fresh data only)::

    python tools/action_policy_train.py \
        --data /tmp/q-standard-fresh.csv \
        --selection-data /tmp/q-selection-fresh.csv \
        --out /tmp/action_policy.json

The selection corpus is opened for scoring only.  Use the reusable evaluator
with a separately reserved external corpus before considering any gameplay
experiment.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import math
from pathlib import Path
import random
import sys
from typing import Iterable, Sequence

try:  # Running as ``python tools/action_policy_train.py``.
    from action_conditioned_eval import (
        ACTION_WIDTH,
        RAW_WIDTH,
        Dataset,
        Decision,
        EvaluationError,
        load_dataset,
        validate_game_range,
    )
except ImportError:  # Running as ``python -m tools.action_policy_train``.
    from tools.action_conditioned_eval import (  # type: ignore[no-redef]
        ACTION_WIDTH,
        RAW_WIDTH,
        Dataset,
        Decision,
        EvaluationError,
        load_dataset,
        validate_game_range,
    )


SCHEMA = "civvis-action-policy-v1"
REQUIRED_REPLICAS = 4
DEFAULT_EPOCHS = 80
DEFAULT_BATCH_SIZE = 32
DEFAULT_RATE = 0.05
DEFAULT_L2 = 0.0001
DEFAULT_MIN_PROBABILITY = 0.70
MIN_SELECTION_COVERAGE = 0.05
EPSILON = 1e-12
U64_MASK = (1 << 64) - 1


@dataclass(frozen=True)
class Pair:
    game: int
    difference: tuple[float, ...]
    target: float


@dataclass(frozen=True)
class Policy:
    weights: tuple[float, ...]
    min_probability: float

    def score(self, features: Sequence[float]) -> float:
        if len(features) != len(self.weights):
            raise EvaluationError(
                f"policy expects {len(self.weights)} features, found {len(features)}"
            )
        value = sum(weight * feature for weight, feature in zip(self.weights, features))
        if not math.isfinite(value):
            raise EvaluationError("non-finite action-policy score")
        return value


@dataclass
class _GameMetric:
    decisions: int = 0
    overrides: int = 0
    expert_regret: float = 0.0
    ranked_regret: float = 0.0
    gated_lift: float = 0.0
    positive: int = 0
    tied: int = 0
    negative: int = 0


def sigmoid(value: float) -> float:
    if value >= 0.0:
        exponential = math.exp(-value) if value < 60.0 else 0.0
        return 1.0 / (1.0 + exponential)
    exponential = math.exp(value) if value > -60.0 else 0.0
    return exponential / (1.0 + exponential)


def stable_game_bucket(game: int) -> float:
    """Match the closed Rust trainers' deterministic game hash split."""
    value = (game * 0x9E3779B97F4A7C15) & U64_MASK
    value ^= value >> 29
    value = (value * 0xBF58476D1CE4E5B9) & U64_MASK
    value ^= value >> 32
    return (value % 1000) / 1000.0


def split_by_game(
    dataset: Dataset, holdout: float = 0.25
) -> tuple[tuple[Decision, ...], tuple[Decision, ...]]:
    if not 0.0 < holdout < 1.0:
        raise EvaluationError("holdout must be strictly between 0 and 1")
    train = tuple(
        decision for decision in dataset.decisions if stable_game_bucket(decision.key.game) >= holdout
    )
    validation = tuple(
        decision for decision in dataset.decisions if stable_game_bucket(decision.key.game) < holdout
    )
    if not train or not validation:
        raise EvaluationError(
            "game split produced an empty side; provide more independent games"
        )
    return train, validation


def pair_target(left: Sequence[float], right: Sequence[float]) -> float:
    """Jeffreys posterior P(left beats right) over matched replicas."""
    if len(left) != len(right) or len(left) != REQUIRED_REPLICAS:
        raise EvaluationError("every candidate pair must have four replica returns")
    wins = sum(a > b for a, b in zip(left, right))
    ties = sum(a == b for a, b in zip(left, right))
    # Beta(1/2, 1/2): 4-0 -> .90, 3-1 -> .70, 2-2 -> .50.
    return (wins + 0.5 * ties + 0.5) / (len(left) + 1.0)


def make_pairs(decisions: Iterable[Decision]) -> tuple[Pair, ...]:
    pairs: list[Pair] = []
    for decision in decisions:
        candidates = decision.candidates
        for left_index in range(len(candidates)):
            for right_index in range(left_index + 1, len(candidates)):
                left = candidates[left_index]
                right = candidates[right_index]
                difference = tuple(a - b for a, b in zip(left.features, right.features))
                if len(difference) != RAW_WIDTH or not all(
                    math.isfinite(value) for value in difference
                ):
                    raise EvaluationError(
                        f"{decision.key.label()}: non-finite or stale feature width"
                    )
                pairs.append(
                    Pair(
                        game=decision.key.game,
                        difference=difference,
                        target=pair_target(left.replicas, right.replicas),
                    )
                )
    if not pairs:
        raise EvaluationError("counterfactual corpus contains no candidate pairs")
    return tuple(pairs)


def fit(
    pairs: Sequence[Pair],
    *,
    epochs: int,
    batch_size: int,
    rate: float,
    l2: float,
    seed: int,
) -> Policy:
    if epochs < 1 or batch_size < 1:
        raise EvaluationError("epochs and batch_size must be positive")
    if not math.isfinite(rate) or rate <= 0.0:
        raise EvaluationError("rate must be finite and positive")
    if not math.isfinite(l2) or l2 < 0.0:
        raise EvaluationError("l2 must be finite and non-negative")
    width = len(pairs[0].difference)
    if width != RAW_WIDTH or any(len(pair.difference) != width for pair in pairs):
        raise EvaluationError(f"pair features must use the current {RAW_WIDTH}-wide schema")
    weights = [0.0] * width
    order = list(range(len(pairs)))
    rng = random.Random(seed)
    for _epoch in range(epochs):
        rng.shuffle(order)
        for start in range(0, len(order), batch_size):
            batch = order[start : start + batch_size]
            gradients = [0.0] * width
            for index in batch:
                pair = pairs[index]
                logit = sum(weight * value for weight, value in zip(weights, pair.difference))
                probability = sigmoid(logit)
                error = probability - pair.target
                for feature_index, value in enumerate(pair.difference):
                    gradients[feature_index] += error * value
            scale = 1.0 / len(batch)
            for feature_index, gradient in enumerate(gradients):
                update = scale * gradient + l2 * weights[feature_index]
                weights[feature_index] -= rate * update
                if not math.isfinite(weights[feature_index]):
                    raise EvaluationError("non-finite action-policy weight")
    return Policy(tuple(weights), DEFAULT_MIN_PROBABILITY)


def pairwise_bce(policy: Policy, pairs: Sequence[Pair]) -> float:
    total = 0.0
    for pair in pairs:
        probability = min(max(sigmoid(policy.score(pair.difference)), EPSILON), 1.0 - EPSILON)
        total += -pair.target * math.log(probability) - (1.0 - pair.target) * math.log(
            1.0 - probability
        )
    return total / len(pairs)


def evaluate(
    policy: Policy, decisions: Iterable[Decision]
) -> dict[str, float | int | dict[str, int]]:
    games: dict[int, _GameMetric] = {}
    for decision in decisions:
        scores = [policy.score(candidate.features) for candidate in decision.candidates]
        expert = 0
        siblings = range(1, len(scores))
        if not siblings:
            raise EvaluationError(f"{decision.key.label()}: fewer than two candidates")
        sibling = max(siblings, key=lambda index: (scores[index], -index))
        best_return = max(candidate.mean_return for candidate in decision.candidates)
        expert_return = decision.candidates[expert].mean_return
        ranked_index = max(range(len(scores)), key=lambda index: (scores[index], -index))
        margin = scores[sibling] - scores[expert]
        override = sigmoid(margin) >= policy.min_probability
        selected_index = sibling if override else expert
        selected_return = decision.candidates[selected_index].mean_return
        metric = games.setdefault(decision.key.game, _GameMetric())
        metric.decisions += 1
        metric.overrides += int(override)
        metric.expert_regret += best_return - expert_return
        metric.ranked_regret += best_return - decision.candidates[ranked_index].mean_return
        metric.gated_lift += selected_return - expert_return
        if override:
            difference = selected_return - expert_return
            if difference > 2e-6:
                metric.positive += 1
            elif difference < -2e-6:
                metric.negative += 1
            else:
                metric.tied += 1
    if not games:
        raise EvaluationError("no decisions to evaluate")

    per_game = list(games.values())
    decisions_total = sum(metric.decisions for metric in per_game)
    return {
        "games": len(per_game),
        "decisions": decisions_total,
        "overrides": sum(metric.overrides for metric in per_game),
        "coverage": sum(metric.overrides / metric.decisions for metric in per_game)
        / len(per_game),
        "expert_regret": sum(metric.expert_regret / metric.decisions for metric in per_game)
        / len(per_game),
        "ranked_regret": sum(metric.ranked_regret / metric.decisions for metric in per_game)
        / len(per_game),
        "gated_lift": sum(metric.gated_lift / metric.decisions for metric in per_game)
        / len(per_game),
        "override_outcomes": {
            "positive": sum(metric.positive for metric in per_game),
            "tied": sum(metric.tied for metric in per_game),
            "negative": sum(metric.negative for metric in per_game),
        },
    }


def _load_selection(parser: argparse.ArgumentParser, args: argparse.Namespace) -> Dataset:
    if not args.selection_data:
        return load_dataset(args.data)
    selection = load_dataset(args.selection_data)
    if args.selection_seed is not None or args.selection_games is not None:
        if args.selection_seed is None or args.selection_games is None:
            parser.error("--selection-seed and --selection-games must be supplied together")
        validate_game_range(selection, args.selection_seed, args.selection_games, "selection")
    return selection


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data", required=True, help="development q_counterfactual CSV")
    parser.add_argument(
        "--selection-data",
        help="independent selection CSV; if omitted, a deterministic game holdout is used",
    )
    parser.add_argument("--selection-seed", type=int)
    parser.add_argument("--selection-games", type=int)
    parser.add_argument("--out", required=True, help="candidate action_policy.json path")
    parser.add_argument("--epochs", type=int, default=DEFAULT_EPOCHS)
    parser.add_argument("--batch-size", type=int, default=DEFAULT_BATCH_SIZE)
    parser.add_argument("--rate", type=float, default=DEFAULT_RATE)
    parser.add_argument("--l2", type=float, default=DEFAULT_L2)
    parser.add_argument("--min-probability", type=float, default=DEFAULT_MIN_PROBABILITY)
    parser.add_argument("--holdout", type=float, default=0.25)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument(
        "--allow-nonimproving",
        action="store_true",
        help="write a diagnostic artifact despite a failed selection gate",
    )
    parser.add_argument("--json", action="store_true", help="emit metrics as JSON")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if not 0.5 < args.min_probability < 1.0 or not math.isfinite(args.min_probability):
        parser.error("--min-probability must be finite and in (0.5, 1)")
    try:
        development = load_dataset(args.data)
        if len(development.games) < 4:
            raise EvaluationError("development data needs at least four independent games")
        if args.selection_data:
            selection = _load_selection(parser, args)
            overlap = set(development.games) & set(selection.games)
            if overlap:
                raise EvaluationError(
                    f"development and selection games overlap: {sorted(overlap)[:5]}"
                )
        else:
            development, selection = split_by_game(development, args.holdout)
            # Keep the Dataset-shaped contract needed by make_pairs without
            # reconstructing metadata that the trainer does not use.
            development = Dataset(args.data, development)
            selection = Dataset(args.data, selection)

        train_pairs = make_pairs(development.decisions)
        validation_pairs = make_pairs(selection.decisions)
        policy = fit(
            train_pairs,
            epochs=args.epochs,
            batch_size=args.batch_size,
            rate=args.rate,
            l2=args.l2,
            seed=args.seed,
        )
        policy = Policy(policy.weights, args.min_probability)
        train_bce = pairwise_bce(policy, train_pairs)
        validation_bce = pairwise_bce(policy, validation_pairs)
        baseline_bce = math.log(2.0)
        validation_metrics = evaluate(policy, selection.decisions)
        beats_pairwise_baseline = validation_bce < baseline_bce - 1e-4
        selection_pass = (
            beats_pairwise_baseline
            and float(validation_metrics["coverage"]) >= MIN_SELECTION_COVERAGE
            and float(validation_metrics["gated_lift"]) > 0.0
        )
        metrics = {
            "schema": SCHEMA,
            "feature_width": RAW_WIDTH,
            "state_width": 34,
            "action_width": ACTION_WIDTH,
            "replicas": REQUIRED_REPLICAS,
            "seed": args.seed,
            "hyperparameters": {
                "epochs": args.epochs,
                "batch_size": args.batch_size,
                "rate": args.rate,
                "l2": args.l2,
                "min_probability": args.min_probability,
            },
            "games": {
                "train": len(development.games),
                "selection": len(selection.games),
            },
            "pairs": {"train": len(train_pairs), "selection": len(validation_pairs)},
            "train_bce": train_bce,
            "selection_bce": validation_bce,
            "constant_bce": baseline_bce,
            "beats_pairwise_baseline": beats_pairwise_baseline,
            "selection": validation_metrics,
            "selection_pass": selection_pass,
        }
        if not selection_pass and not args.allow_nonimproving:
            raise EvaluationError(
                "refusing to write action policy: selection requires held-out BCE below "
                "constant, at least 5% game-macro coverage, and positive gated lift; "
                "use --allow-nonimproving for a diagnostic only"
            )

        artifact = {
            "schema": SCHEMA,
            "feature_width": RAW_WIDTH,
            "keep": "all",
            "interactions": "none",
            "weights": list(policy.weights),
            "min_probability": policy.min_probability,
            "training": metrics,
        }
        destination = Path(args.out)
        destination.parent.mkdir(parents=True, exist_ok=True)
        temporary = destination.with_name(destination.name + ".tmp")
        temporary.write_text(json.dumps(artifact, separators=(",", ":")), encoding="utf-8")
        temporary.replace(destination)
        if args.json:
            print(json.dumps(metrics, sort_keys=True))
        else:
            print(
                f"selection: {len(selection.games)} games, "
                f"coverage {float(validation_metrics['coverage']):.1%}, "
                f"gated lift {float(validation_metrics['gated_lift']):+.5f}, "
                f"BCE {validation_bce:.5f} vs constant {baseline_bce:.5f}; "
                f"gate {'PASS' if selection_pass else 'DIAGNOSTIC'}"
            )
            print(f"wrote {destination}")
        return 0
    except EvaluationError as error:
        parser.error(str(error))
    return 2


if __name__ == "__main__":
    sys.exit(main())
