#!/usr/bin/env python3
"""Safely screen an action-conditioned ranker before any gameplay A/B.

The closed ``q_counterfactual`` emitter records one chosen move and its
same-actor alternatives from the identical pre-action state.  The historical
Q tools each parsed that CSV independently, and the last one stopped at a
selection failure before opening the untouched deployment profile.  This
module makes that boundary reusable:

* the CSV schema, candidate grouping, four matched replicas, finite values,
  and declared return means are checked before a score is evaluated;
* a frozen ``civvis-q-advantage-v1`` linear artifact is scored without fitting
  or threshold search;
* all headline metrics are macro-averaged by independent game;
* an explicit fixed margin is the only override rule; and
* an optional external file is opened only after the selection coverage gate
  passes.

It deliberately does not train, tune, or integrate a policy.  The command is
an evaluator for a fresh preregistration, not a way to reopen the rejected
closed experiments.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
import json
import math
from pathlib import Path
import statistics
import sys
from typing import Iterable, Sequence, TextIO


# These are the append-only action-space widths in src/action_space.rs.  The
# parser fails closed if a corpus does not expose this exact current schema;
# silently scoring a stale feature vector would make the gate meaningless.
STATE_WIDTH = 34
KIND_WIDTH = 85
LEGACY_WIDTH = 13
DESTINATION_WIDTH = 35
ACTION_WIDTH = KIND_WIDTH + LEGACY_WIDTH + DESTINATION_WIDTH
RAW_WIDTH = STATE_WIDTH + ACTION_WIDTH
DESTINATION_ROLE_WIDTH = 8
PLAN_OFFSET = DESTINATION_ROLE_WIDTH
PLAN_WIDTH = 3
REQUIRED_REPLICAS = 4
RETURN_TOLERANCE = 2e-6
Z95 = 1.959963984540054


class EvaluationError(ValueError):
    """A malformed corpus, artifact, or preregistration argument."""


@dataclass(frozen=True)
class DecisionKey:
    game: int
    turn: int
    seat: int
    unit: int

    def label(self) -> str:
        return f"game {self.game} turn {self.turn} seat {self.seat} unit {self.unit}"


@dataclass(frozen=True)
class Candidate:
    chosen: bool
    features: tuple[float, ...]
    mean_return: float
    replicas: tuple[float, ...]


@dataclass(frozen=True)
class Decision:
    key: DecisionKey
    candidates: tuple[Candidate, ...]


@dataclass(frozen=True)
class Dataset:
    path: str
    decisions: tuple[Decision, ...]
    width: int = RAW_WIDTH
    replicas: int = REQUIRED_REPLICAS

    @property
    def games(self) -> tuple[int, ...]:
        return tuple(sorted({decision.key.game for decision in self.decisions}))

    @property
    def rows(self) -> int:
        return sum(len(decision.candidates) for decision in self.decisions)


@dataclass(frozen=True)
class LinearModel:
    schema: str
    feature_width: int
    keep: str
    interactions: str
    weights: tuple[float, ...]

    @classmethod
    def load(cls, path: str | Path) -> "LinearModel":
        source = Path(path)
        try:
            payload = json.loads(source.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise EvaluationError(f"cannot read model {source}: {error}") from error
        if not isinstance(payload, dict):
            raise EvaluationError(f"model {source}: expected a JSON object")
        schema = payload.get("schema")
        if schema != "civvis-q-advantage-v1":
            raise EvaluationError(
                f"model {source}: expected civvis-q-advantage-v1, found {schema!r}"
            )
        feature_width = payload.get("feature_width")
        keep = payload.get("keep")
        interactions = payload.get("interactions", "none")
        raw_weights = payload.get("weights")
        if (
            isinstance(feature_width, bool)
            or not isinstance(feature_width, int)
            or feature_width <= 0
        ):
            raise EvaluationError(f"model {source}: feature_width must be positive")
        if keep not in {
            "state",
            "action",
            "kind",
            "geometry",
            "legacy-geometry",
            "destination",
            "destination-no-plan",
            "plan",
        }:
            raise EvaluationError(f"model {source}: unsupported feature block {keep!r}")
        if interactions not in {"none", "role"}:
            raise EvaluationError(
                f"model {source}: interactions must be 'none' or 'role', found {interactions!r}"
            )
        if not isinstance(raw_weights, list) or len(raw_weights) != feature_width:
            raise EvaluationError(
                f"model {source}: {feature_width} weights required, found "
                f"{len(raw_weights) if isinstance(raw_weights, list) else 'non-list'}"
            )
        weights: list[float] = []
        for index, raw in enumerate(raw_weights):
            if not isinstance(raw, (int, float)) or not math.isfinite(float(raw)):
                raise EvaluationError(f"model {source}: weight {index} is not finite")
            weights.append(float(raw))
        expected_width = transformed_width(interactions)
        if feature_width != expected_width:
            raise EvaluationError(
                f"model {source}: width {feature_width} does not match current "
                f"{interactions} feature contract {expected_width}"
            )
        return cls(
            schema=schema,
            feature_width=feature_width,
            keep=keep,
            interactions=interactions,
            weights=tuple(weights),
        )

    def score(self, features: Sequence[float]) -> float:
        if len(features) != self.feature_width:
            raise EvaluationError(
                f"model expects {self.feature_width} features, found {len(features)}"
            )
        return sum(weight * value for weight, value in zip(self.weights, features))


@dataclass(frozen=True)
class Report:
    label: str
    games: int
    decisions: int
    rows: int
    overrides: int
    coverage: float
    coverage_se: float
    expert_regret: float
    ranked_regret: float
    ungated_lift: float
    gated_lift: float
    gated_lift_se: float
    gated_lower_95: float
    positive_overrides: int
    tied_overrides: int
    negative_overrides: int

    def as_dict(self) -> dict[str, object]:
        return {
            "label": self.label,
            "games": self.games,
            "decisions": self.decisions,
            "rows": self.rows,
            "overrides": self.overrides,
            "coverage": self.coverage,
            "coverage_se": self.coverage_se,
            "expert_regret": self.expert_regret,
            "ranked_regret": self.ranked_regret,
            "ungated_lift": self.ungated_lift,
            "gated_lift": self.gated_lift,
            "gated_lift_se": self.gated_lift_se,
            "gated_lower_95": self.gated_lower_95,
            "override_outcomes": {
                "positive": self.positive_overrides,
                "tied": self.tied_overrides,
                "negative": self.negative_overrides,
            },
        }


def transformed_width(interactions: str) -> int:
    """Return the q_advantage output width for the current action schema."""
    if interactions == "none":
        return RAW_WIDTH
    if interactions == "role":
        quantities = DESTINATION_WIDTH - DESTINATION_ROLE_WIDTH
        return RAW_WIDTH + DESTINATION_ROLE_WIDTH * quantities
    raise EvaluationError(f"unsupported interactions {interactions!r}")


def _parse_int(raw: str, path: str, line: int, name: str) -> int:
    try:
        value = int(raw)
    except ValueError as error:
        raise EvaluationError(f"{path}:{line}: invalid {name} {raw!r}") from error
    if value < 0:
        raise EvaluationError(f"{path}:{line}: {name} must be non-negative")
    return value


def _parse_float(raw: str, path: str, line: int, name: str) -> float:
    try:
        value = float(raw)
    except ValueError as error:
        raise EvaluationError(f"{path}:{line}: invalid {name} {raw!r}") from error
    if not math.isfinite(value):
        raise EvaluationError(f"{path}:{line}: {name} is not finite")
    return value


def _header(path: str, names: Sequence[str]) -> int:
    prefix = ["game", "turn", "seat", "unit", "chosen"]
    if list(names[: len(prefix)]) != prefix:
        raise EvaluationError(f"{path}: expected q_counterfactual identity columns {prefix}")
    state_names = [f"s{index}" for index in range(STATE_WIDTH)]
    action_names = [f"a{index}" for index in range(ACTION_WIDTH)]
    expected_prefix = prefix + state_names + action_names + ["return"]
    if list(names[: len(expected_prefix)]) != expected_prefix:
        raise EvaluationError(
            f"{path}: stale or reordered q_counterfactual schema; expected "
            f"s0..s{STATE_WIDTH - 1}, a0..a{ACTION_WIDTH - 1}, return"
        )
    return len(expected_prefix) - 1


def parse_dataset(stream: TextIO, path: str = "<stream>") -> Dataset:
    """Parse and validate one q_counterfactual CSV without dropping rows."""
    reader = csv.reader(stream)
    try:
        names = next(reader)
    except StopIteration as error:
        raise EvaluationError(f"{path}: empty counterfactual file") from error
    return_column = _header(path, names)
    replica_names = list(names[return_column + 1 :])
    expected_replicas = [f"r{index}" for index in range(REQUIRED_REPLICAS)]
    if replica_names != expected_replicas:
        raise EvaluationError(
            f"{path}: expected replica columns {expected_replicas}, found {replica_names}"
        )

    decisions: list[Decision] = []
    current_key: DecisionKey | None = None
    current: list[Candidate] = []
    seen: set[DecisionKey] = set()

    def finish() -> None:
        nonlocal current_key, current
        if current_key is None:
            return
        if len(current) < 2:
            raise EvaluationError(f"{path}: {current_key.label()} has fewer than two candidates")
        chosen = sum(candidate.chosen for candidate in current)
        if chosen != 1 or not current[0].chosen:
            raise EvaluationError(
                f"{path}: {current_key.label()} must begin with exactly one chosen candidate"
            )
        decisions.append(Decision(current_key, tuple(current)))
        current_key = None
        current = []

    for line_number, fields in enumerate(reader, start=2):
        if not fields or all(not field for field in fields):
            raise EvaluationError(f"{path}:{line_number}: blank rows are not allowed")
        if len(fields) != len(names):
            raise EvaluationError(
                f"{path}:{line_number}: {len(fields)} fields, expected {len(names)}"
            )
        key = DecisionKey(
            game=_parse_int(fields[0], path, line_number, "game"),
            turn=_parse_int(fields[1], path, line_number, "turn"),
            seat=_parse_int(fields[2], path, line_number, "seat"),
            unit=_parse_int(fields[3], path, line_number, "unit"),
        )
        if fields[4] not in {"0", "1"}:
            raise EvaluationError(f"{path}:{line_number}: chosen must be 0 or 1")
        if current_key != key:
            finish()
            if key in seen:
                raise EvaluationError(f"{path}:{line_number}: decision group {key.label()} repeats")
            seen.add(key)
            current_key = key
        features = tuple(
            _parse_float(value, path, line_number, f"feature {index}")
            for index, value in enumerate(fields[5:return_column])
        )
        if len(features) != RAW_WIDTH:
            raise EvaluationError(f"{path}:{line_number}: expected {RAW_WIDTH} features")
        mean_return = _parse_float(fields[return_column], path, line_number, "return")
        replicas = tuple(
            _parse_float(value, path, line_number, f"replica {index}")
            for index, value in enumerate(fields[return_column + 1 :])
        )
        replica_mean = statistics.fmean(replicas)
        if abs(mean_return - replica_mean) > RETURN_TOLERANCE:
            raise EvaluationError(
                f"{path}:{line_number}: return {mean_return} does not match "
                f"replica mean {replica_mean}"
            )
        current.append(Candidate(fields[4] == "1", features, mean_return, replicas))
    finish()
    if not decisions:
        raise EvaluationError(f"{path}: no complete decisions")
    return Dataset(path=path, decisions=tuple(decisions))


def load_dataset(path: str | Path) -> Dataset:
    source = Path(path)
    try:
        with source.open(newline="", encoding="utf-8") as stream:
            return parse_dataset(stream, str(source))
    except OSError as error:
        raise EvaluationError(f"cannot read {source}: {error}") from error


def validate_game_range(dataset: Dataset, seed: int, games: int, label: str) -> None:
    if seed < 0 or games <= 0:
        raise EvaluationError(f"{label}: seed must be non-negative and games positive")
    expected = set(range(seed, seed + games))
    actual = set(dataset.games)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        parts = []
        if missing:
            parts.append(f"missing {missing[:5]}{'...' if len(missing) > 5 else ''}")
        if extra:
            parts.append(f"unexpected {extra[:5]}{'...' if len(extra) > 5 else ''}")
        raise EvaluationError(f"{label}: game range mismatch ({'; '.join(parts)})")


def _mask(features: Sequence[float], keep: str) -> list[float]:
    row = list(features)
    state = STATE_WIDTH
    kinds = KIND_WIDTH
    legacy = state + kinds
    destination = legacy + LEGACY_WIDTH
    plan = destination + PLAN_OFFSET
    plan_end = plan + PLAN_WIDTH

    def blank(start: int, end: int) -> None:
        for index in range(max(0, start), min(len(row), end)):
            row[index] = 0.0

    if keep == "state":
        blank(state, len(row))
    elif keep == "action":
        blank(0, state)
    elif keep == "kind":
        blank(0, state)
        blank(state + kinds, len(row))
    elif keep == "geometry":
        blank(0, state + kinds)
    elif keep == "legacy-geometry":
        blank(0, legacy)
        blank(destination, len(row))
    elif keep == "destination":
        blank(0, destination)
    elif keep == "destination-no-plan":
        blank(0, destination)
        blank(plan, plan_end)
    elif keep == "plan":
        blank(0, plan)
        blank(plan_end, len(row))
    else:
        raise EvaluationError(f"unsupported feature block {keep!r}")
    return row


def transform(features: Sequence[float], keep: str, interactions: str) -> tuple[float, ...]:
    row = _mask(features, keep)
    if interactions == "role":
        destination = STATE_WIDTH + KIND_WIDTH + LEGACY_WIDTH
        roles = row[destination : destination + DESTINATION_ROLE_WIDTH]
        quantities = row[
            destination + DESTINATION_ROLE_WIDTH : destination + DESTINATION_WIDTH
        ]
        row.extend(role * quantity for role in roles for quantity in quantities)
    elif interactions != "none":
        raise EvaluationError(f"unsupported interactions {interactions!r}")
    return tuple(row)


@dataclass
class _GameAccumulator:
    decisions: int = 0
    rows: int = 0
    overrides: int = 0
    expert_regret: float = 0.0
    ranked_regret: float = 0.0
    ungated_lift: float = 0.0
    gated_lift: float = 0.0
    positive: int = 0
    tied: int = 0
    negative: int = 0


def _mean_and_se(values: Iterable[float]) -> tuple[float, float]:
    values = list(values)
    if not values:
        return 0.0, 0.0
    mean = statistics.fmean(values)
    if len(values) < 2:
        return mean, 0.0
    return mean, statistics.stdev(values) / math.sqrt(len(values))


def evaluate(
    dataset: Dataset,
    model: LinearModel,
    *,
    min_margin: float,
    label: str = "evaluation",
) -> Report:
    if not math.isfinite(min_margin):
        raise EvaluationError("min_margin must be finite")
    games: dict[int, _GameAccumulator] = {}
    for decision in dataset.decisions:
        scores = [
            model.score(transform(candidate.features, model.keep, model.interactions))
            for candidate in decision.candidates
        ]
        if any(not math.isfinite(score) for score in scores):
            raise EvaluationError(f"{label}: non-finite model score for {decision.key.label()}")
        chosen_index = next(index for index, candidate in enumerate(decision.candidates) if candidate.chosen)
        chosen_return = decision.candidates[chosen_index].mean_return
        oracle_return = max(candidate.mean_return for candidate in decision.candidates)
        ranked_index = max(range(len(scores)), key=lambda index: (scores[index], -index))
        ranked_return = decision.candidates[ranked_index].mean_return
        sibling_indices = [index for index in range(len(scores)) if index != chosen_index]
        sibling_index = max(sibling_indices, key=lambda index: (scores[index], -index))
        margin = scores[sibling_index] - scores[chosen_index]
        override = margin > min_margin
        selected_return = decision.candidates[sibling_index].mean_return if override else chosen_return
        accumulator = games.setdefault(decision.key.game, _GameAccumulator())
        accumulator.decisions += 1
        accumulator.rows += len(decision.candidates)
        accumulator.overrides += int(override)
        accumulator.expert_regret += oracle_return - chosen_return
        accumulator.ranked_regret += oracle_return - ranked_return
        accumulator.ungated_lift += ranked_return - chosen_return
        accumulator.gated_lift += selected_return - chosen_return
        if override:
            difference = selected_return - chosen_return
            if difference > RETURN_TOLERANCE:
                accumulator.positive += 1
            elif difference < -RETURN_TOLERANCE:
                accumulator.negative += 1
            else:
                accumulator.tied += 1

    if not games:
        raise EvaluationError(f"{label}: no decisions")
    coverage, coverage_se = _mean_and_se(
        accumulator.overrides / accumulator.decisions for accumulator in games.values()
    )
    expert_regret, _ = _mean_and_se(
        accumulator.expert_regret / accumulator.decisions for accumulator in games.values()
    )
    ranked_regret, _ = _mean_and_se(
        accumulator.ranked_regret / accumulator.decisions for accumulator in games.values()
    )
    ungated_lift, _ = _mean_and_se(
        accumulator.ungated_lift / accumulator.decisions for accumulator in games.values()
    )
    gated_lift, gated_lift_se = _mean_and_se(
        accumulator.gated_lift / accumulator.decisions for accumulator in games.values()
    )
    return Report(
        label=label,
        games=len(games),
        decisions=sum(accumulator.decisions for accumulator in games.values()),
        rows=sum(accumulator.rows for accumulator in games.values()),
        overrides=sum(accumulator.overrides for accumulator in games.values()),
        coverage=coverage,
        coverage_se=coverage_se,
        expert_regret=expert_regret,
        ranked_regret=ranked_regret,
        ungated_lift=ungated_lift,
        gated_lift=gated_lift,
        gated_lift_se=gated_lift_se,
        gated_lower_95=gated_lift - Z95 * gated_lift_se,
        positive_overrides=sum(accumulator.positive for accumulator in games.values()),
        tied_overrides=sum(accumulator.tied for accumulator in games.values()),
        negative_overrides=sum(accumulator.negative for accumulator in games.values()),
    )


def selection_pass(report: Report, min_coverage: float = 0.05) -> bool:
    """The development gate: positive point lift and nonzero declared coverage."""
    return report.gated_lift > 0.0 and report.coverage >= min_coverage


def external_pass(report: Report, min_coverage: float = 0.05) -> bool:
    """The untouched-profile gate: positive 95% lower lift and coverage."""
    return report.gated_lower_95 > 0.0 and report.coverage >= min_coverage


def _print_report(report: Report) -> None:
    print(
        f"{report.label}: {report.games} games, {report.decisions} decisions, "
        f"{report.rows} candidate rows"
    )
    print(
        f"  regret expert {report.expert_regret:.5f} -> ranked {report.ranked_regret:.5f}; "
        f"ungated lift {report.ungated_lift:+.5f}"
    )
    print(
        f"  gated lift {report.gated_lift:+.5f} +/- {report.gated_lift_se:.5f} "
        f"(95% lower {report.gated_lower_95:+.5f}); "
        f"coverage {report.coverage:.1%} +/- {report.coverage_se:.1%} "
        f"({report.overrides}/{report.decisions})"
    )
    print(
        f"  override outcomes +/=/− {report.positive_overrides}/"
        f"{report.tied_overrides}/{report.negative_overrides}"
    )


def _optional_profile(
    parser: argparse.ArgumentParser,
    args: argparse.Namespace,
    dataset: Dataset,
    prefix: str,
) -> None:
    seed = getattr(args, f"{prefix}_seed")
    games = getattr(args, f"{prefix}_games")
    if (seed is None) != (games is None):
        parser.error(f"--{prefix}-seed and --{prefix}-games must be supplied together")
    if seed is not None:
        try:
            validate_game_range(dataset, seed, games, prefix)
        except EvaluationError as error:
            parser.error(str(error))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="screen a frozen action-conditioned ranker with a fixed abstention margin"
    )
    parser.add_argument("--model", required=True, help="civvis-q-advantage-v1 JSON artifact")
    parser.add_argument("--data", required=True, help="primary counterfactual CSV")
    parser.add_argument("--data-seed", type=int, help="optional exact primary seed prefix")
    parser.add_argument("--data-games", type=int, help="optional exact primary game count")
    parser.add_argument("--selection-data", help="fresh Standard selection CSV")
    parser.add_argument("--selection-seed", type=int)
    parser.add_argument("--selection-games", type=int)
    parser.add_argument("--external-data", help="untouched external CSV; opened only after selection passes")
    parser.add_argument("--external-seed", type=int)
    parser.add_argument("--external-games", type=int)
    parser.add_argument(
        "--min-margin",
        type=float,
        required=True,
        help="fixed score margin; no threshold sweep is implemented",
    )
    parser.add_argument(
        "--min-coverage",
        type=float,
        default=0.05,
        help="minimum game-macro override coverage for selection/external gates (default: 0.05)",
    )
    parser.add_argument("--json", action="store_true", help="emit reports as JSON")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if not math.isfinite(args.min_margin):
        parser.error("--min-margin must be finite")
    if not 0.0 <= args.min_coverage <= 1.0:
        parser.error("--min-coverage must be between 0 and 1")
    if args.external_data and not args.selection_data:
        parser.error("--external-data requires --selection-data")
    if args.data_seed is not None or args.data_games is not None:
        if args.data_seed is None or args.data_games is None:
            parser.error("--data-seed and --data-games must be supplied together")
    try:
        model = LinearModel.load(args.model)
        primary = load_dataset(args.data)
        if args.data_seed is not None:
            validate_game_range(primary, args.data_seed, args.data_games, "data")
        reports: list[Report] = [
            evaluate(primary, model, min_margin=args.min_margin, label="data")
        ]
        selection: Report | None = None
        selection_status: bool | None = None
        external_status: bool | None = None
        if args.selection_data:
            selection_dataset = load_dataset(args.selection_data)
            _optional_profile(parser, args, selection_dataset, "selection")
            selection = evaluate(
                selection_dataset, model, min_margin=args.min_margin, label="selection"
            )
            reports.append(selection)
            selection_status = selection_pass(selection, args.min_coverage)
            if not args.json:
                print(f"selection gate: {'PASS' if selection_status else 'FAIL'}")
            if args.external_data and not selection_status:
                if not args.json:
                    print("external profile remained unopened because selection failed")
                return 3
        if args.external_data:
            external_dataset = load_dataset(args.external_data)
            _optional_profile(parser, args, external_dataset, "external")
            external = evaluate(
                external_dataset, model, min_margin=args.min_margin, label="external"
            )
            reports.append(external)
            external_status = external_pass(external, args.min_coverage)
            if not args.json:
                print(f"external gate: {'PASS' if external_status else 'FAIL'}")
        if args.json:
            print(
                json.dumps(
                    {
                        "reports": {report.label: report.as_dict() for report in reports},
                        "selection_gate": selection_status,
                        "external_gate": external_status,
                    },
                    sort_keys=True,
                )
            )
        else:
            for report in reports:
                _print_report(report)
        return 0
    except EvaluationError as error:
        parser.error(str(error))
    return 2


if __name__ == "__main__":
    sys.exit(main())
