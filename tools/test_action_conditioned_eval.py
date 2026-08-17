import io
import json
from pathlib import Path
from contextlib import redirect_stdout
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

from action_conditioned_eval import (
    ACTION_WIDTH,
    RAW_WIDTH,
    REQUIRED_REPLICAS,
    Candidate,
    Dataset,
    Decision,
    DecisionKey,
    EvaluationError,
    LinearModel,
    evaluate,
    external_pass,
    main,
    parse_dataset,
    selection_pass,
    transform,
    transformed_width,
    validate_game_range,
)


def header() -> str:
    names = ["game", "turn", "seat", "unit", "chosen"]
    names.extend(f"s{index}" for index in range(34))
    names.extend(f"a{index}" for index in range(ACTION_WIDTH))
    names.append("return")
    names.extend(f"r{index}" for index in range(REQUIRED_REPLICAS))
    return ",".join(names)


def row(game: int, turn: int, chosen: int, feature: float, value: float) -> str:
    features = [0.0] * RAW_WIDTH
    # The first destination term is at the end of state + kind + legacy.  The
    # destination model below keeps that block and scores this one coordinate.
    features[34 + 85 + 13] = feature
    replicas = ",".join(str(value) for _ in range(REQUIRED_REPLICAS))
    fields = [str(game), str(turn), "0", "7", str(chosen)]
    fields.extend(str(value) for value in features)
    fields.extend([str(value), replicas])
    return ",".join(fields)


def csv_text(*, games=(100,), reversed_group=False) -> str:
    rows = [header()]
    for game in games:
        rows.append(row(game, 50, 1, 0.0, 0.20))
        sibling = row(game, 50, 0, 1.0, 0.40)
        if reversed_group:
            rows[-1], sibling = sibling, rows[-1]
        rows.append(sibling)
    return "\n".join(rows) + "\n"


def model(*, weight: float = 1.0, interactions: str = "none") -> LinearModel:
    weights = [0.0] * transformed_width(interactions)
    weights[34 + 85 + 13] = weight
    return LinearModel(
        schema="civvis-q-advantage-v1",
        feature_width=len(weights),
        keep="destination",
        interactions=interactions,
        weights=tuple(weights),
    )


class SchemaTests(unittest.TestCase):
    def test_current_schema_and_replica_mean_are_required(self):
        dataset = parse_dataset(io.StringIO(csv_text()), "fixture.csv")
        self.assertEqual(dataset.rows, 2)
        self.assertEqual(dataset.games, (100,))
        self.assertEqual(dataset.replicas, 4)

    def test_stale_header_fails_closed(self):
        source = csv_text().replace("a132", "a131", 1)
        with self.assertRaisesRegex(EvaluationError, "stale or reordered"):
            parse_dataset(io.StringIO(source), "stale.csv")

    def test_mean_must_match_all_four_replicas(self):
        source = csv_text().replace(",0.2,0.2,0.2,0.2,0.2", ",0.2,0.2,0.2,0.2,0.1", 1)
        with self.assertRaisesRegex(EvaluationError, "does not match"):
            parse_dataset(io.StringIO(source), "bad-return.csv")

    def test_groups_must_be_contiguous_and_chosen_first(self):
        with self.assertRaisesRegex(EvaluationError, "must begin"):
            parse_dataset(io.StringIO(csv_text(reversed_group=True)), "order.csv")

        source = csv_text(games=(100, 101)) + "\n".join(csv_text(games=(100,)).splitlines()[1:]) + "\n"
        with self.assertRaisesRegex(EvaluationError, "repeats"):
            parse_dataset(io.StringIO(source), "duplicate.csv")

    def test_game_range_is_exact(self):
        dataset = parse_dataset(io.StringIO(csv_text(games=(100, 101))), "range.csv")
        validate_game_range(dataset, 100, 2, "selection")
        with self.assertRaisesRegex(EvaluationError, "game range mismatch"):
            validate_game_range(dataset, 101, 2, "selection")


class ModelTests(unittest.TestCase):
    def test_destination_mask_keeps_only_destination_terms(self):
        features = [1.0] * RAW_WIDTH
        transformed = transform(features, "destination", "none")
        self.assertEqual(len(transformed), RAW_WIDTH)
        self.assertTrue(all(value == 0.0 for value in transformed[:34 + 85 + 13]))
        self.assertTrue(all(value == 1.0 for value in transformed[34 + 85 + 13 :]))

    def test_role_interactions_have_declared_width(self):
        self.assertEqual(transformed_width("role"), RAW_WIDTH + 8 * 27)
        transformed = transform([1.0] * RAW_WIDTH, "destination", "role")
        self.assertEqual(len(transformed), transformed_width("role"))

    def test_model_loader_rejects_unknown_schema_and_wrong_width(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "model.json"
            path.write_text(json.dumps({"schema": "old", "weights": []}), encoding="utf-8")
            with self.assertRaisesRegex(EvaluationError, "expected civvis-q-advantage-v1"):
                LinearModel.load(path)
            path.write_text(
                json.dumps(
                    {
                        "schema": "civvis-q-advantage-v1",
                        "feature_width": RAW_WIDTH - 1,
                        "keep": "destination",
                        "interactions": "none",
                        "weights": [0.0] * (RAW_WIDTH - 1),
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(EvaluationError, "does not match"):
                LinearModel.load(path)


class MetricTests(unittest.TestCase):
    def test_fixed_margin_abstains_and_reports_game_macro_metrics(self):
        dataset = parse_dataset(io.StringIO(csv_text(games=(100, 101))), "metric.csv")
        report = evaluate(dataset, model(), min_margin=0.5, label="selection")
        self.assertEqual(report.decisions, 2)
        self.assertEqual(report.overrides, 2)
        self.assertAlmostEqual(report.coverage, 1.0)
        self.assertAlmostEqual(report.gated_lift, 0.20)
        self.assertGreater(report.gated_lower_95, 0.0)
        self.assertEqual(report.positive_overrides, 2)
        self.assertTrue(selection_pass(report, 0.05))
        self.assertTrue(external_pass(report, 0.05))

    def test_margin_gate_abstains_when_sibling_is_not_confident(self):
        dataset = parse_dataset(io.StringIO(csv_text()), "abstain.csv")
        report = evaluate(dataset, model(), min_margin=2.0)
        self.assertEqual(report.overrides, 0)
        self.assertEqual(report.coverage, 0.0)
        self.assertEqual(report.gated_lift, 0.0)
        self.assertFalse(selection_pass(report))

    def test_non_finite_score_fails_closed(self):
        features = [0.0] * RAW_WIDTH
        features[34 + 85 + 13] = 1e308
        key = DecisionKey(100, 50, 0, 7)
        dataset = Dataset(
            "overflow.csv",
            (
                Decision(
                    key,
                    (
                        Candidate(True, tuple(features), 0.2, (0.2,) * 4),
                        Candidate(False, tuple(features), 0.4, (0.4,) * 4),
                    ),
                ),
            ),
        )
        overflowing = model(weight=1e308)
        with self.assertRaisesRegex(EvaluationError, "non-finite model score"):
            evaluate(dataset, overflowing, min_margin=0.0)

    def test_report_is_macro_by_game_not_row_count(self):
        # One game has one decision and another has three; the game means must
        # carry equal weight even though the row count differs.
        decisions = []
        for game, count in ((100, 1), (101, 3)):
            for turn in range(count):
                key = DecisionKey(game, turn, 0, 7)
                decisions.append(
                    Decision(
                        key,
                        (
                            Candidate(True, tuple([0.0] * RAW_WIDTH), 0.20, (0.20,) * 4),
                            Candidate(False, tuple([1.0] * RAW_WIDTH), 0.40, (0.40,) * 4),
                        ),
                    )
                )
        report = evaluate(
            Dataset("manual", tuple(decisions)), model(), min_margin=0.5, label="macro"
        )
        self.assertAlmostEqual(report.coverage, 1.0)
        self.assertAlmostEqual(report.gated_lift, 0.20)

    def test_failed_selection_never_opens_external_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            data = root / "selection.csv"
            data.write_text(csv_text(), encoding="utf-8")
            artifact = root / "model.json"
            artifact.write_text(
                json.dumps(
                    {
                        "schema": "civvis-q-advantage-v1",
                        "feature_width": RAW_WIDTH,
                        "keep": "destination",
                        "interactions": "none",
                        "weights": [0.0] * RAW_WIDTH,
                    }
                ),
                encoding="utf-8",
            )
            output = io.StringIO()
            with redirect_stdout(output):
                code = main(
                    [
                        "--model",
                        str(artifact),
                        "--data",
                        str(data),
                        "--selection-data",
                        str(data),
                        "--external-data",
                        str(root / "must-not-open.csv"),
                        "--min-margin",
                        "0.0",
                    ]
                )
            self.assertEqual(code, 3)
            self.assertIn("remained unopened", output.getvalue())


if __name__ == "__main__":
    unittest.main()
