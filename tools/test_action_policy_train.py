import math
import unittest

try:
    from action_conditioned_eval import Candidate, Dataset, Decision, DecisionKey, RAW_WIDTH
    from action_policy_train import (
        DEFAULT_MIN_PROBABILITY,
        EvaluationError,
        Policy,
        evaluate,
        fit,
        make_pairs,
        pair_target,
        split_by_game,
    )
except ImportError:  # Package-style test invocation.
    from .action_conditioned_eval import Candidate, Dataset, Decision, DecisionKey, RAW_WIDTH
    from .action_policy_train import (
        DEFAULT_MIN_PROBABILITY,
        EvaluationError,
        Policy,
        evaluate,
        fit,
        make_pairs,
        pair_target,
        split_by_game,
    )


def decision(game: int, sibling_value: float = 1.0) -> Decision:
    chosen = Candidate(True, (0.0,) * RAW_WIDTH, 0.2, (0.2,) * 4)
    features = [0.0] * RAW_WIDTH
    features[-1] = sibling_value
    sibling = Candidate(False, tuple(features), 0.8, (0.8,) * 4)
    return Decision(DecisionKey(game, 50, 0, 7), (chosen, sibling))


class TargetTests(unittest.TestCase):
    def test_jeffreys_target_keeps_replica_disagreement(self):
        self.assertAlmostEqual(pair_target((1, 1, 1, 1), (0, 0, 0, 0)), 0.9)
        self.assertAlmostEqual(pair_target((1, 1, 1, 0), (0, 0, 0, 1)), 0.7)
        self.assertAlmostEqual(pair_target((1, 0, 1, 0), (0, 1, 0, 1)), 0.5)

    def test_wrong_replica_count_fails_closed(self):
        with self.assertRaisesRegex(EvaluationError, "four replica"):
            pair_target((1, 0), (0, 1))


class TrainingTests(unittest.TestCase):
    def test_pairs_are_action_conditioned_differences(self):
        pairs = make_pairs((decision(100),))
        self.assertEqual(len(pairs), 1)
        self.assertEqual(pairs[0].game, 100)
        self.assertEqual(pairs[0].difference[-1], -1.0)
        # The pair is oriented as chosen minus sibling, so the better sibling
        # correctly produces a 0.10 posterior for the left row.
        self.assertAlmostEqual(pairs[0].target, 0.1)

    def test_fit_is_deterministic_and_learns_the_direction(self):
        decisions = tuple(decision(game) for game in range(100, 112))
        dataset = Dataset("synthetic", decisions)
        pairs = make_pairs(dataset.decisions)
        first = fit(pairs, epochs=20, batch_size=4, rate=0.1, l2=0.0, seed=3)
        second = fit(pairs, epochs=20, batch_size=4, rate=0.1, l2=0.0, seed=3)
        self.assertEqual(first.weights, second.weights)
        self.assertLess(first.score(pairs[0].difference), 0.0)
        self.assertTrue(all(math.isfinite(weight) for weight in first.weights))

    def test_game_split_has_no_overlap(self):
        dataset = Dataset("synthetic", tuple(decision(game) for game in range(100, 140)))
        train, validation = split_by_game(dataset)
        self.assertTrue(train)
        self.assertTrue(validation)
        self.assertEqual(
            {item.key.game for item in train} & {item.key.game for item in validation}, set()
        )

    def test_abstention_metrics_are_game_macro(self):
        policy = Policy((0.0,) * (RAW_WIDTH - 1) + (-10.0,), DEFAULT_MIN_PROBABILITY)
        report = evaluate(policy, (decision(100), decision(101)))
        self.assertEqual(report["overrides"], 0)
        self.assertEqual(report["coverage"], 0.0)
        self.assertEqual(report["gated_lift"], 0.0)


if __name__ == "__main__":
    unittest.main()
