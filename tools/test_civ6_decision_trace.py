import json
import hashlib
import tempfile
import unittest
from pathlib import Path

from civ6_decision_trace import compare_transition, record_decision


class TraceTests(unittest.TestCase):
    def test_frame_and_both_action_representations_are_preserved(self):
        payload = {"turn": 12, "orders": [{"kind": "unit", "subject": 9, "verb": "MOVE_TO", "x": 3, "y": 4}],
                   "decision": {"schema": 1, "turn": 12, "frame": 2, "native_actions": [{"type": "move", "unit": 5, "to": [1, 4]}]}}
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "test-binary"
            binary.write_bytes(b"first revision")
            first = record_decision(Path(directory), payload, str(binary))
            binary.write_bytes(b"second revision")
            second = record_decision(Path(directory), payload, str(binary))
            rows = [json.loads(line) for line in (Path(directory) / "decisions.jsonl").read_text().splitlines()]
        self.assertEqual(rows, [first, second])
        self.assertEqual(first["execution_status"], "not_observed")
        self.assertEqual(first["orders"], payload["orders"])
        self.assertEqual(first["binary_on_disk_sha256"], hashlib.sha256(b"first revision").hexdigest())
        self.assertNotEqual(first["binary_on_disk_sha256"], second["binary_on_disk_sha256"])

    def test_unobserved_or_confounded_transitions_never_pass(self):
        for case in ({}, {"same_turn": False, "intervening_actions": 0},
                     {"same_turn": True, "intervening_actions": 1},
                     {"same_turn": True, "intervening_actions": 0}):
            self.assertEqual(compare_transition(case)["status"], "unverifiable")

    def test_roll_bounds_and_exact_results(self):
        case = {"same_turn": True, "intervening_actions": 0,
                "predictions": {"damage": {"low": 24, "high": 36}, "accepted": True},
                "observed": {"damage": 30, "accepted": True}}
        self.assertEqual(compare_transition(case)["status"], "match")
        case["observed"]["damage"] = 37
        self.assertEqual(compare_transition(case)["status"], "mismatch")
        del case["observed"]["damage"]
        self.assertEqual(compare_transition(case)["status"], "unverifiable")

    def test_nonfinite_and_inverted_bounds_are_refused(self):
        for interval in ({"low": 4, "high": 3}, {"low": 0, "high": float("inf")}):
            with self.assertRaises(ValueError):
                compare_transition({"same_turn": True, "intervening_actions": 0,
                                    "predictions": {"damage": interval}, "observed": {"damage": 3}})


if __name__ == "__main__":
    unittest.main()
