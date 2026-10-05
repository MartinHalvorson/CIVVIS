import json
import hashlib
import tempfile
import unittest
from pathlib import Path

from civ6_decision_trace import captured_input, compare_transition, record_decision, validate_input_capture


class TraceTests(unittest.TestCase):
    def capture(self, directory):
        return {"schema": 1, "complete": True, "error": None, "directory": str(directory),
                "source": str(directory / "events.jsonl"), "archive": "bytes.bin",
                "index": "snapshots.jsonl", "reads": [{"snapshot_id": 0, "count": 2},
                {"error": "read failed", "count": 1}, {"snapshot_id": 1, "count": 1}]}

    def test_input_capture_descriptor_is_preserved_and_part_of_receipt_hash(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            binary = directory / "binary"
            binary.write_bytes(b"binary")
            payload = {"turn": 1, "orders": [], "decision": {"schema": 1, "turn": 1, "frame": 0, "native_actions": []}}
            plain = record_decision(directory, payload, str(binary))
            payload["input_capture"] = self.capture(directory)
            record = record_decision(directory, payload, str(binary))
            self.assertNotIn("input_capture", plain)
            self.assertEqual(record["input_capture"], payload["input_capture"])
            self.assertNotEqual(record["sha256"], plain["sha256"])
            encoded = json.dumps({k: v for k, v in record.items() if k != "sha256"}, sort_keys=True, separators=(",", ":"), allow_nan=False)
            self.assertEqual(record["sha256"], hashlib.sha256(encoded.encode()).hexdigest())
            self.assertEqual(record["execution_status"], "not_observed")

    def test_reconstructs_delta_bytes_and_replacement_without_reading_future_index(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            capture = self.capture(directory)
            rows = [{"id": 0, "parent": None, "offset": 0, "append_bytes": 3, "total_bytes": 3},
                    {"id": 1, "parent": 0, "offset": 3, "append_bytes": 7, "total_bytes": 10},
                    {"id": 2, "parent": None, "offset": 10, "append_bytes": 4, "total_bytes": 4}]
            index = directory / "snapshots.jsonl"
            index.write_text("".join(json.dumps(row) + "\n" for row in rows) + '{"still appending')
            (directory / "bytes.bin").write_bytes("é\npartialnew\n".encode())
            self.assertEqual(captured_input(capture, 0), "é\n".encode())
            self.assertEqual(captured_input(capture, 1), "é\npartial".encode())
            capture["reads"].append({"snapshot_id": 2, "count": 1})
            self.assertEqual(captured_input(capture, 2), b"new\n")
            with self.assertRaisesRegex(ValueError, "not a read"):
                captured_input(capture, 3)
            (directory / "bytes.bin").write_bytes(b"short")
            with self.assertRaisesRegex(ValueError, "truncated"):
                captured_input(capture, 2)
            rows[1]["parent"] = 1
            index.write_text("".join(json.dumps(row) + "\n" for row in rows))
            with self.assertRaisesRegex(ValueError, "sequence"):
                captured_input(capture, 1)

    def test_capture_failure_remains_evidence_not_success(self):
        with tempfile.TemporaryDirectory() as temp:
            capture = dict(self.capture(Path(temp)), complete=False, error="disk full")
            validate_input_capture(capture)
            with self.assertRaisesRegex(ValueError, "incomplete"):
                captured_input(capture, 0)
            for updates in ({"schema": True}, {"complete": True}, {"directory": "relative"},
                            {"archive": "../escape"}, {"reads": [{"snapshot_id": True, "count": 1}]},
                            {"reads": [{"snapshot_id": 0, "count": False}]},
                            {"reads": [{"snapshot_id": 0, "error": "both", "count": 1}]}):
                with self.assertRaises(ValueError):
                    validate_input_capture(dict(capture, **updates))

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
        case = {"same_turn": True, "intervening_actions": 0, "phase": "settled",
                "predictions": {"damage": {"low": 24, "high": 36}, "accepted": True},
                "observed": {"damage": 30, "accepted": True}}
        self.assertEqual(compare_transition(case)["status"], "match")
        case["observed"]["damage"] = 37
        self.assertEqual(compare_transition(case)["status"], "mismatch")
        del case["observed"]["damage"]
        self.assertEqual(compare_transition(case)["status"], "unverifiable")

    def test_matching_request_boundary_does_not_prove_execution(self):
        case = {"same_turn": True, "intervening_actions": 0, "phase": "request_boundary",
                "predictions": {"position": [2, 3]}, "observed": {"position": [2, 3]}}
        self.assertEqual(compare_transition(case)["status"], "unverifiable")
        case["phase"] = "settled"
        self.assertEqual(compare_transition(case)["status"], "match")

    def test_nonfinite_and_inverted_bounds_are_refused(self):
        for interval in ({"low": 4, "high": 3}, {"low": 0, "high": float("inf")}):
            with self.assertRaises(ValueError):
                compare_transition({"same_turn": True, "intervening_actions": 0, "phase": "settled",
                                    "predictions": {"damage": interval}, "observed": {"damage": 3}})

    def test_missing_unknown_or_gap_marked_phase_cannot_pass(self):
        case = {"same_turn": True, "intervening_actions": 0,
                "predictions": {"position": [2, 3]}, "observed": {"position": [2, 3]}}
        for phase in (None, "request_boundary", "timeout", "unknown"):
            self.assertEqual(compare_transition(dict(case, phase=phase))["status"], "unverifiable")
        self.assertEqual(compare_transition(dict(case, phase="settled", coverage_gap="missing map"))["status"], "unverifiable")
        self.assertEqual(compare_transition(dict(case, phase="settled", intervening_actions=False))["status"], "unverifiable")

    def test_exact_nested_facts_preserve_types_and_reject_nonfinite_tails(self):
        case = {"same_turn": True, "intervening_actions": 0, "phase": "settled",
                "predictions": {"position": [1, 2]}, "observed": {"position": [True, 2]}}
        self.assertEqual(compare_transition(case)["status"], "mismatch")
        case["observed"]["position"] = [1, 2, float("inf")]
        with self.assertRaises(ValueError): compare_transition(case)

    def test_boolean_frame_is_not_a_decision_frame(self):
        payload = {"turn": 12, "orders": [], "decision": {"schema": 1, "turn": 12, "frame": True, "native_actions": []}}
        with self.assertRaisesRegex(ValueError, "frame"):
            record_decision(Path("/unused"), payload, "/unused")


if __name__ == "__main__":
    unittest.main()
