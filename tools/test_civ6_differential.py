"""Hermetic contract tests for the replay differential harness."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_differential as differential  # noqa: E402


def write_trace(root: Path, name: str, records: list[str]) -> Path:
    path = root / name
    path.write_text("\n".join(records) + "\n", encoding="utf-8")
    return path


def event(kind: str, turn: int, **fields: object) -> str:
    return json.dumps({"kind": kind, "turn": turn, **fields}, separators=(",", ":"))


class TraceLoadTest(unittest.TestCase):
    def test_selected_transition_records_keep_source_order(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = write_trace(Path(temporary), "trace.jsonl", [
                event("seat", 0, civ="ignored-by-default"),
                event("state", 1, value=1),
                event("orders", 1, applied=2),
                event("turn", 1, score=3),
            ])
            trace = differential.load_trace(path)

        self.assertEqual([(frame.turn, frame.phase, frame.occurrence)
                          for frame in trace.frames], [
                              (1, "state", 0), (1, "orders", 0), (1, "turn", 0),
                          ])

    def test_duplicate_keys_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "bad.jsonl"
            path.write_text('{"kind":"state","turn":1,"gold":4,"gold":5}\n')
            with self.assertRaisesRegex(differential.TraceError, "duplicate"):
                differential.load_trace(path)

    def test_a_selected_record_without_a_turn_is_not_silently_compared(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "bad.jsonl"
            path.write_text('{"kind":"state","gold":4}\n')
            with self.assertRaisesRegex(differential.TraceError, "integer turn"):
                differential.load_trace(path)

    def test_backwards_turns_and_gaps_are_distinct_contract_failures(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            backwards = write_trace(root, "backwards.jsonl", [
                event("state", 2, value=2), event("state", 1, value=1),
            ])
            with self.assertRaisesRegex(differential.TraceError, "backwards"):
                differential.load_trace(backwards)

            gap = write_trace(root, "gap.jsonl", [
                event("state", 1, value=1), event("state", 3, value=3),
            ])
            with self.assertRaisesRegex(differential.TraceError, "not contiguous"):
                differential.load_trace(gap, require_contiguous=True)

    def test_unterminated_final_record_is_explicitly_opt_in(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "partial.jsonl"
            path.write_text(event("state", 1, value=1) + "\n{" )
            with self.assertRaises(differential.TraceError):
                differential.load_trace(path)
            trace = differential.load_trace(path, allow_trailing_partial=True)
            self.assertEqual(len(trace.frames), 1)

    def test_an_empty_selected_window_is_not_a_false_green(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "empty.jsonl"
            path.write_text(event("seat", 0, civ="Rome") + "\n")
            with self.assertRaisesRegex(differential.TraceError, "no selected"):
                differential.load_trace(path)


class DifferentialComparisonTest(unittest.TestCase):
    def traces(self, left: list[str], right: list[str]) -> tuple[differential.Trace, differential.Trace]:
        root = Path(self.temporary.name)
        return (
            differential.load_trace(write_trace(root, "oracle.jsonl", left)),
            differential.load_trace(write_trace(root, "candidate.jsonl", right)),
        )

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_transport_metadata_and_set_order_do_not_create_drift(self) -> None:
        oracle, candidate = self.traces(
            [event("state", 1, run="oracle", ctx="agent", techs=["B", "A"],
                   cities=[{"id": 1, "pop": 2}])],
            [event("state", 1, run="candidate", ctx="other", techs=["A", "B"],
                   cities=[{"pop": 2, "id": 1}])],
        )
        report = differential.compare_traces(oracle, candidate)
        self.assertTrue(report["equal"])
        self.assertIsNone(report["first_divergence"])

    def test_first_semantic_difference_names_frame_path_and_hashes(self) -> None:
        oracle, candidate = self.traces(
            [event("state", 4, cities=[{"id": 1, "pop": 3}]),
             event("turn", 4, score=12)],
            [event("state", 4, cities=[{"id": 1, "pop": 4}]),
             event("turn", 4, score=12)],
        )
        report = differential.compare_traces(oracle, candidate)
        self.assertFalse(report["equal"])
        drift = report["first_divergence"]
        self.assertEqual(drift["type"], "state")
        self.assertEqual(drift["path"], "/cities/0/pop")
        self.assertNotEqual(drift["oracle"]["hash"], drift["candidate"]["hash"])
        self.assertEqual(report["matched_records"], 0)

    def test_order_source_is_decision_state_not_ignored_transport_metadata(self) -> None:
        oracle, candidate = self.traces(
            [event("orders", 3, source="civvis", applied=4)],
            [event("orders", 3, source="fallback", applied=4)],
        )
        drift = differential.compare_traces(oracle, candidate)["first_divergence"]
        self.assertEqual(drift["path"], "/source")

    def test_phase_reordering_is_structural_drift_even_when_values_match(self) -> None:
        oracle, candidate = self.traces(
            [event("state", 2, value=1), event("orders", 2, applied=1)],
            [event("orders", 2, applied=1), event("state", 2, value=1)],
        )
        drift = differential.compare_traces(oracle, candidate)["first_divergence"]
        self.assertEqual(drift["type"], "frame")
        self.assertEqual(drift["path"], "/frame")

    def test_missing_tail_frame_is_reported_without_dumping_whole_trace(self) -> None:
        oracle, candidate = self.traces(
            [event("state", 1, value=1), event("turn", 1, score=2)],
            [event("state", 1, value=1)],
        )
        report = differential.compare_traces(oracle, candidate)
        self.assertEqual(report["first_divergence"]["type"], "frame_count")
        self.assertEqual(report["first_divergence"]["side"], "oracle")

    def test_explicit_unordered_path_handles_a_real_set_field(self) -> None:
        oracle, candidate = self.traces(
            [event("state", 1, choices=[{"id": 1}, {"id": 2}])],
            [event("state", 1, choices=[{"id": 2}, {"id": 1}])],
        )
        strict = differential.compare_traces(oracle, candidate, unordered=())
        self.assertFalse(strict["equal"])
        relaxed = differential.compare_traces(oracle, candidate, unordered=("/choices",))
        self.assertTrue(relaxed["equal"])


class CliTest(unittest.TestCase):
    def test_exit_codes_are_equal_different_and_contract_error(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            same = write_trace(root, "same.jsonl", [event("state", 1, value=1)])
            other = write_trace(root, "other.jsonl", [event("state", 1, value=2)])
            bad = write_trace(root, "bad.jsonl", [event("state", 1, value=1),
                                                   event("state", 3, value=3)])
            self.assertEqual(differential.main(["--oracle", str(same),
                                                "--candidate", str(same)]), 0)
            self.assertEqual(differential.main(["--oracle", str(same),
                                                "--candidate", str(other)]), 1)
            self.assertEqual(differential.main(["--oracle", str(bad),
                                                "--candidate", str(same),
                                                "--require-contiguous"]), 2)

    def test_json_report_can_be_consumed_without_human_text(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            same = write_trace(root, "same.jsonl", [event("state", 1, value=1)])
            proc = subprocess.run(
                [sys.executable, str(Path(__file__).with_name("civ6_differential.py")),
                 "--oracle", str(same), "--candidate", str(same), "--json"],
                check=False, capture_output=True, text=True,
            )
            self.assertEqual(proc.returncode, 0)
            self.assertTrue(json.loads(proc.stdout)["equal"])
            self.assertEqual(proc.stderr, "")


if __name__ == "__main__":
    unittest.main()
