#!/usr/bin/env python3
"""Native replay controls, discovered by the tools unittest workflow.

The fixtures reproduce the failure where identical replay arms agree with
each other but differ from native. CLI tests use small persistent deciders,
so the output files and exit status are checked without a game installation.
"""

from __future__ import annotations

import copy
import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_city_route_replay as replay


def reply(turn=1, frame=0, verb="FORTIFY"):
    return {"turn": turn,
            "orders": [{"kind": "unit", "subject": 23, "verb": verb,
                        "x": None, "y": None}],
            "decision": {"turn": turn, "frame": frame,
                         "native_actions": [{"type": "fortify", "unit": 23}],
                         "victory_portfolio": {"focus": 0.0}}}


class NativeControlTests(unittest.TestCase):
    def test_duplicate_arms_are_not_a_native_control(self):
        original = reply()
        recorded = reply(verb="ATTACK")
        control = replay.NativeControl({(1, 0): recorded}, 0)
        control.observe(1, 0, original, copy.deepcopy(original))
        result = control.result()
        self.assertEqual(result["changed_complete_reply_frames"], 0)
        self.assertFalse(result["gate_passed"])
        self.assertFalse(result["first_mismatch"]["orders_match"])
        self.assertEqual(result["first_mismatch"]["recorded"]["orders"], recorded["orders"])

    def test_identical_arms_need_every_native_frame_to_match(self):
        first, second = reply(), reply(2)
        control = replay.NativeControl({(1, 0): first, (2, 0): second}, 0)
        control.observe(1, 0, first, copy.deepcopy(first))
        control.observe(2, 0, second, copy.deepcopy(second))
        self.assertTrue(control.result()["gate_passed"])
        self.assertEqual(control.result()["matched_frames"], 2)

    def test_same_actions_do_not_establish_complete_decision_agreement(self):
        original = reply()
        recorded = copy.deepcopy(original)
        recorded["decision"]["victory_portfolio"]["focus"] = 0.8
        candidate = reply(verb="MOVE_TO")
        control = replay.NativeControl({(1, 0): recorded}, 0)
        control.observe(1, 0, original, candidate)
        result = control.result()
        self.assertTrue(result["first_changed_complete_reply"]["orders_match"])
        self.assertFalse(result["first_changed_complete_reply"]["complete_decision_match"])
        self.assertFalse(result["gate_passed"])

    def test_first_complete_change_includes_telemetry_before_action_changes(self):
        original = reply()
        original["orders"].append({"kind": "order_failed", "verb": "unit:MOVE_TO moved_away"})
        recorded = copy.deepcopy(original)
        recorded["orders"][-1] = {"kind": "order_verified", "verb": "unit:MOVE_TO"}
        candidate = copy.deepcopy(original)
        candidate["orders"][-1] = {"kind": "turn_verified", "verb": "different ledger"}
        later = reply(2)
        control = replay.NativeControl({(1, 0): recorded, (2, 0): later}, 0)
        control.observe(1, 0, original, candidate)
        control.observe(2, 0, later, reply(2, verb="ATTACK"))
        result = control.result()
        self.assertEqual(replay.actionable_orders(original), replay.actionable_orders(candidate))
        self.assertEqual(result["first_changed_complete_reply"]["turn"], 1)
        self.assertFalse(result["gate_passed"], "the later native match cannot replace turn 1")

    def test_decision_only_change_is_the_first_complete_change(self):
        original = reply()
        candidate = copy.deepcopy(original)
        candidate["decision"]["victory_portfolio"]["focus"] = 0.9
        control = replay.NativeControl({(1, 0): original}, 0)
        control.observe(1, 0, original, candidate)
        result = control.result()
        self.assertEqual(result["changed_complete_reply_frames"], 1)
        self.assertTrue(result["gate_passed"])
        self.assertEqual(result["first_changed_complete_reply"]["candidate"], candidate)

    def test_a_pass_at_first_change_retains_earlier_native_mismatches(self):
        first, second = reply(), reply(2)
        control = replay.NativeControl({(1, 0): reply(verb="ATTACK"), (2, 0): second}, 0)
        control.observe(1, 0, first, copy.deepcopy(first))
        control.observe(2, 0, second, reply(2, verb="MOVE_TO"))
        result = control.result()
        self.assertTrue(result["gate_passed"], "only the first change is licensed")
        self.assertEqual(result["matched_frames"], 1)
        self.assertEqual(result["all_native_mismatches"][0]["turn"], 1)
        self.assertEqual(result["first_mismatch"]["original"], replay.control_payload(first))

    def test_a_missing_record_and_empty_board_are_coverage_gaps(self):
        empty = {"turn": 1, "orders": [], "note": "no terrain"}
        control = replay.NativeControl({}, 1)
        control.observe(1, 0, empty, copy.deepcopy(empty))
        result = control.result()
        self.assertFalse(result["gate_passed"])
        self.assertEqual(result["missing_record_frames"], 1)
        self.assertEqual(result["unkeyed_native_records"], 1)

    def test_missing_decision_and_explicit_null_are_not_identical(self):
        original = {"turn": 1, "orders": []}
        recorded = {"turn": 1, "frame": 0, "orders": [], "decision": None}
        control = replay.NativeControl({(1, 0): recorded}, 0)
        control.observe(1, 0, original, copy.deepcopy(original))
        self.assertFalse(control.result()["gate_passed"])

    def test_boolean_and_numeric_values_do_not_establish_native_agreement(self):
        original = reply()
        for value in (False, 0):
            with self.subTest(value=value):
                recorded = copy.deepcopy(original)
                recorded["decision"]["victory_portfolio"]["focus"] = value
                control = replay.NativeControl({(1, 0): recorded}, 0)
                control.observe(1, 0, original, copy.deepcopy(original))
                self.assertFalse(control.result()["gate_passed"])

    def test_type_only_change_cannot_be_replaced_by_a_later_native_match(self):
        original, later = reply(), reply(2)
        candidate = copy.deepcopy(original)
        candidate["decision"]["victory_portfolio"]["focus"] = False
        control = replay.NativeControl({(1, 0): reply(verb="ATTACK"), (2, 0): later}, 0)
        control.observe(1, 0, original, candidate)
        control.observe(2, 0, later, reply(2, verb="MOVE_TO"))
        result = control.result()
        self.assertEqual(result["first_changed_complete_reply"]["turn"], 1)
        self.assertFalse(result["gate_passed"])

    def test_nonfinite_reply_values_cannot_establish_control(self):
        for value in (float("nan"), float("inf"), float("-inf")):
            with self.subTest(value=value):
                original = reply()
                original["decision"]["victory_portfolio"]["focus"] = value
                control = replay.NativeControl({(1, 0): original}, 0)
                with self.assertRaises(ValueError):
                    control.observe(1, 0, original, copy.deepcopy(original))

    def test_ambiguous_native_frames_are_rejected(self):
        with TemporaryDirectory() as raw:
            path = Path(raw) / "native.jsonl"
            path.write_text(json.dumps(reply()) + "\n" + json.dumps(reply(verb="ATTACK")) + "\n")
            with self.assertRaisesRegex(ValueError, "ambiguous duplicate"):
                replay.load_native_records(path)

    def test_unkeyed_native_records_are_counted_without_guessing_frame_zero(self):
        with TemporaryDirectory() as raw:
            path = Path(raw) / "native.jsonl"
            path.write_text(json.dumps({"turn": 1, "orders": []}) + "\n")
            records, unkeyed = replay.load_native_records(path)
            self.assertEqual(records, {})
            self.assertEqual(unkeyed, 1)
        for value in (True, 1.0, "1", -1):
            self.assertIsNone(replay.frame_key({"turn": value, "frame": 0}))

    def test_malformed_native_replies_are_rejected(self):
        with TemporaryDirectory() as raw:
            path = Path(raw) / "native.jsonl"
            for text in ("broken json\n", "[]\n", '{"orders":null}\n'):
                path.write_text(text)
                with self.assertRaises(ValueError):
                    replay.load_native_records(path)


class ReplayCliTests(unittest.TestCase):
    def setUp(self):
        self.temporary = TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.run = self.root / "run"
        self.run.mkdir()
        self.source = self.run / "events.jsonl"
        self.native = self.run / "decisions.jsonl"
        self.force = self.root / "forces.txt"
        self.force.write_text("anvil\n")
        self.out = self.root / "out"

    def binary(self, name, replies, mutate=None):
        path = self.root / name
        path.write_text(
            f"#!{sys.executable}\n"
            "import json, pathlib, sys\n"
            f"replies = {replies!r}\n"
            f"mutate = {str(mutate) if mutate else None!r}\n"
            "if mutate:\n"
            "    pathlib.Path(mutate).write_text('changed input\\n')\n"
            "for index, line in enumerate(sys.stdin):\n"
            "    print(json.dumps(replies[index]), flush=True)\n")
        path.chmod(0o755)
        return path

    def execute(self, original, candidate, recorded, mutate=None, extra=()):
        self.source.write_text("".join(json.dumps({"kind": "state", "turn": i + 1, "frame": 0})
                                      + "\n" for i in range(len(original))))
        self.native.write_text("".join(json.dumps({"binary": "native", **r}) + "\n"
                                      for r in recorded))
        before = {p: p.read_bytes() for p in (self.source, self.native, self.force)}
        baseline = self.binary("baseline", original)
        candidate_bin = self.binary("candidate", candidate, mutate)
        result = subprocess.run(
            [sys.executable, str(Path(replay.__file__)), "--run", str(self.run),
             "--baseline", str(baseline), "--candidate", str(candidate_bin),
             "--force-file", str(self.force), "--out", str(self.out), *extra],
            text=True, capture_output=True, timeout=15)
        if mutate is None:
            self.assertEqual(before, {p: p.read_bytes() for p in before})
        self.assertTrue((self.out / "provenance.json").is_file(), result.stderr)
        return result, json.loads((self.out / "native-control.json").read_text()), \
            json.loads((self.out / "provenance.json").read_text())

    def test_failed_native_control_exits_nonzero_and_keeps_both_arms(self):
        first, later = reply(), reply(2)
        candidate = copy.deepcopy(first)
        candidate["decision"]["victory_portfolio"]["focus"] = 0.7
        result, control, metadata = self.execute(
            [first, later], [candidate, reply(2, verb="ATTACK")],
            [reply(verb="MOVE_TO"), later])
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse(control["gate_passed"])
        self.assertEqual(control["first_changed_complete_reply"]["turn"], 1)
        self.assertEqual(metadata["native_control"]["changed_complete_reply_frames"], 2)
        self.assertEqual(metadata["returncodes"], {"baseline": 0, "candidate": 0})
        for arm in ("baseline", "candidate"):
            self.assertEqual(len((self.out / arm / "decisions.jsonl").read_text().splitlines()), 2)

    def test_successful_change_keeps_earlier_mismatches_in_the_cli_report(self):
        first, later = reply(), reply(2)
        result, control, metadata = self.execute(
            [first, later], [first, reply(2, verb="ATTACK")],
            [reply(verb="MOVE_TO"), later])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(control["gate_passed"])
        self.assertEqual(control["first_mismatch"]["turn"], 1)
        self.assertEqual(control["first_changed_complete_reply"]["turn"], 2)
        self.assertTrue(all(metadata["inputs_unchanged"].values()))

    def test_changed_input_cannot_report_a_passing_run(self):
        first = reply()
        result, control, metadata = self.execute([first], [first], [first], mutate=self.force)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertTrue(control["gate_passed"], "reply comparison and input integrity are distinct")
        self.assertFalse(metadata["inputs_unchanged"]["force_file"])
        self.assertFalse(metadata["validation_passed"])

    def test_partial_replay_cannot_report_success_when_a_decider_exits(self):
        first, later = reply(), reply(2)
        result, control, metadata = self.execute([first, later], [first], [first, later])
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(control["gate_passed"], "the one observed reply matches")
        self.assertIn("candidate closed output", metadata["error"])
        self.assertFalse(metadata["validation_passed"])

    def test_through_turn_checks_only_the_requested_prefix(self):
        first, later = reply(), reply(2)
        result, control, metadata = self.execute([first, later], [first, later], [first, later],
                                                extra=("--through-turn", "1"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(control["frames"], 1)
        self.assertEqual(metadata["frames"], 1)


if __name__ == "__main__":
    unittest.main()
