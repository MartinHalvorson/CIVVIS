"""Opt-in controller capture, restart isolation, and honest reply receipts."""
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import civ6_brain as brain
import civ6_play as play


class Process:
    def __init__(self):
        self.stdin = io.StringIO()
        self.stdout = io.StringIO()
        self.dead = False

    def poll(self):
        return 0 if self.dead else None

    def wait(self, timeout):
        self.dead = True
        return 0


class CaptureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.run = Path(self.temp.name).resolve()
        self.binary = self.run / "binary"
        self.binary.write_bytes(b"test binary")

    def decider(self, enabled=True):
        return brain.Decider(self.binary, self.run, "domination", capture_inputs=enabled)

    def payload(self, requested_directory, **capture_changes):
        capture = {"schema": 1, "complete": True, "error": None,
                   "source": str(self.run / "events.jsonl"), "directory": str(requested_directory),
                   "archive": "bytes.bin", "index": "snapshots.jsonl",
                   "reads": [{"snapshot_id": 0, "count": 2}]}
        capture.update(capture_changes)
        return {"turn": 7, "orders": [{"kind": "unit", "subject": 4,
                "verb": "MOVE_TO", "x": 2, "y": 3}], "note": "ok",
                "decision": {"schema": 1, "turn": 7, "frame": 2, "native_actions": []},
                "input_capture": capture}

    def statuses(self):
        return [json.loads(line) for line in
                (self.run / "input_capture_status.jsonl").read_text().splitlines()]

    def test_default_command_is_exact_and_creates_nothing(self):
        decider = self.decider(False)
        self.assertEqual(decider.command(), [str(self.binary), "--mirror", str(self.run),
                         "--serve", "--fresh-board", "--explain", "--victory", "domination"])
        self.assertFalse((self.run / "input-captures").exists())
        self.assertFalse((self.run / "input_capture_status.jsonl").exists())

    def test_enabled_command_requires_a_process_reservation(self):
        with self.assertRaisesRegex(ValueError, "reserved"):
            self.decider().command()

    def test_restarts_seat_runtime_death_and_workers_never_reuse_archives(self):
        directories = []
        with mock.patch.object(brain.subprocess, "Popen", side_effect=lambda *a, **k: Process()) as popen:
            decider = self.decider()
            self.addCleanup(decider.stop)
            for change in (None, "seat", "runtime", "death"):
                if change == "seat":
                    decider.set_civ("Rome")
                elif change == "runtime":
                    decider.use_runtime(self.run / "next-binary")
                elif change == "death":
                    decider.proc.dead = True
                    decider.stop()
                decider.start()
                directory = decider.capture_directory
                self.assertFalse(directory.exists(), "the executable needs a NEW directory")
                directory.mkdir()
                (directory / "bytes.bin").write_bytes(b"retained")
                directories.append(directory)
                command = popen.call_args.args[0]
                self.assertEqual(command[-2:], ["--capture-inputs", str(directory)])
            other = self.decider()
            self.addCleanup(other.stop)
            other.start()
            directories.append(other.capture_directory)
        self.assertEqual(len(set(directories)), 5)
        for directory in directories[:-1]:
            self.assertEqual((directory / "bytes.bin").read_bytes(), b"retained")

    def test_repeated_reservation_preserves_failed_start_container(self):
        with mock.patch.object(brain.subprocess, "Popen", side_effect=OSError("cannot exec")):
            decider = self.decider()
            with self.assertRaises(OSError):
                decider.start()
            first = decider.capture_directory
            with self.assertRaises(OSError):
                decider.start()
            self.assertNotEqual(first, decider.capture_directory)
            self.assertTrue(first.parent.is_dir())
            self.assertIsNone(decider.why)

    def test_valid_persistent_reply_retains_descriptor_and_exact_orders(self):
        with mock.patch.object(brain.subprocess, "Popen", return_value=Process()):
            decider = self.decider()
            self.addCleanup(decider.stop)
            decider.start()
            payload = self.payload(decider.capture_directory)
            decider.proc.stdout = io.StringIO(json.dumps(payload) + "\n")
            self.assertEqual(decider.ask(7), ([("unit", 4, "MOVE_TO", 2, 3)], "ok"))
        receipt = self.statuses()[0]
        self.assertEqual(receipt["status"], "reported_complete")
        self.assertFalse(receipt["archive_bytes_verified"])
        trace = json.loads((self.run / "decisions.jsonl").read_text())
        self.assertEqual(trace["input_capture"], payload["input_capture"])
        self.assertEqual(trace["execution_status"], "not_observed")

    def test_missing_invalid_wrong_path_incomplete_and_read_errors_are_not_success(self):
        directory = brain.new_input_capture(self.run)
        cases = [({}, "missing", False),
                 ({"schema": True}, "invalid", False),
                 ({"directory": str(self.run / "other")}, "invalid", False),
                 ({"source": str(self.run / "other.jsonl")}, "invalid", False),
                 ({"complete": False, "error": "disk full"}, "reported_incomplete", True),
                 ({"reads": [{"error": "missing source", "count": 1}]}, "read_failed", True)]
        for changes, status, valid in cases:
            with self.subTest(status=status, changes=changes):
                payload = self.payload(directory, **changes)
                if not changes:
                    del payload["input_capture"]
                self.assertIs(brain.record_input_capture(self.run, self.binary, 7,
                              directory, payload), valid)
                self.assertEqual(self.statuses()[-1]["status"], status)

    def test_ignored_or_malformed_flag_reply_does_not_discard_gameplay_orders(self):
        for capture in (None, {"schema": True}):
            with mock.patch.object(brain.subprocess, "Popen", return_value=Process()):
                decider = self.decider()
                decider.start()
                payload = self.payload(decider.capture_directory)
                if capture is None:
                    del payload["input_capture"]
                else:
                    payload["input_capture"] = capture
                decider.proc.stdout = io.StringIO(json.dumps(payload) + "\n")
                self.assertEqual(decider.ask(7)[0], [("unit", 4, "MOVE_TO", 2, 3)])
                decider.stop()
        traces = [json.loads(line) for line in (self.run / "decisions.jsonl").read_text().splitlines()]
        self.assertTrue(all("input_capture" not in trace for trace in traces))
        self.assertEqual([row["status"] for row in self.statuses()], ["missing", "invalid"])

    def test_one_shot_capture_gets_unique_archives_and_records_decisions(self):
        def respond(command, **kwargs):
            directory = Path(command[command.index("--capture-inputs") + 1])
            self.assertFalse(directory.exists())
            return SimpleNamespace(returncode=0, stdout=json.dumps(self.payload(directory)), stderr="")
        with mock.patch.object(brain.subprocess, "run", side_effect=respond):
            for _ in range(2):
                self.assertEqual(brain.civvis_orders(self.binary, self.run, 7, "domination",
                                 capture_inputs=True), [("unit", 4, "MOVE_TO", 2, 3)])
        statuses = self.statuses()
        self.assertNotEqual(statuses[0]["requested_directory"], statuses[1]["requested_directory"])
        self.assertEqual(len((self.run / "decisions.jsonl").read_text().splitlines()), 2)

    def test_one_shot_default_has_no_capture_flag_writes_or_new_trace(self):
        response = SimpleNamespace(returncode=0, stdout='{"orders":[]}', stderr="")
        with mock.patch.object(brain.subprocess, "run", return_value=response) as run:
            brain.civvis_orders(self.binary, self.run, 7, "domination")
        self.assertNotIn("--capture-inputs", run.call_args.args[0])
        self.assertEqual(list(self.run.iterdir()), [self.binary])

    def test_process_failures_have_explicit_status_not_a_success_receipt(self):
        for response in (SimpleNamespace(returncode=2, stdout="", stderr="unsupported"),
                         SimpleNamespace(returncode=0, stdout="bad json", stderr="")):
            with mock.patch.object(brain.subprocess, "run", return_value=response):
                self.assertEqual(brain.civvis_orders(self.binary, self.run, 7,
                                 "domination", capture_inputs=True), [])
        self.assertTrue(all(row["status"] == "decider_failed" for row in self.statuses()))
        self.assertFalse((self.run / "decisions.jsonl").exists())

    def test_symlinked_run_path_matches_the_actual_requested_events(self):
        alias = self.run / "alias"
        alias.symlink_to(self.run, target_is_directory=True)
        directory = brain.new_input_capture(alias)
        payload = self.payload(directory, source=str(alias / "events.jsonl"))
        self.assertTrue(brain.record_input_capture(alias, self.binary, 7, directory, payload))
        self.assertEqual(self.statuses()[0]["status"], "reported_complete")

    def test_dead_process_ask_reserves_a_new_archive_before_the_next_query(self):
        def start_process(command, **kwargs):
            proc = Process()
            directory = Path(command[command.index("--capture-inputs") + 1])
            proc.stdout = io.StringIO(json.dumps(self.payload(directory)) + "\n")
            return proc
        with mock.patch.object(brain.subprocess, "Popen", side_effect=start_process):
            decider = self.decider()
            self.assertEqual(decider.ask(7)[0], [("unit", 4, "MOVE_TO", 2, 3)])
            first = decider.capture_directory
            decider.proc.dead = True
            self.assertEqual(decider.ask(7)[0], [("unit", 4, "MOVE_TO", 2, 3)])
            self.assertNotEqual(first, decider.capture_directory)
            decider.stop()

    def test_no_board_reply_still_has_capture_status_without_a_fake_decision(self):
        with mock.patch.object(brain.subprocess, "Popen", return_value=Process()):
            decider = self.decider()
            decider.start()
            payload = self.payload(decider.capture_directory)
            del payload["decision"]
            payload["orders"] = []
            decider.proc.stdout = io.StringIO(json.dumps(payload) + "\n")
            self.assertEqual(decider.ask(7), ([], "ok"))
            decider.stop()
        self.assertEqual(self.statuses()[0]["status"], "reported_complete")
        self.assertIsNone(self.statuses()[0]["frame"])
        self.assertFalse((self.run / "decisions.jsonl").exists())

    def test_both_public_launch_modes_share_opt_in_forwarding(self):
        args = SimpleNamespace(civvis_victory="domination", civvis_strategy="",
               civvis_war_from_plan=False, civvis_refresh_seconds=0,
               civvis_with=[], civvis_without=[], timeout=10)
        default = play.supervised_brain_command(args, self.run, self.run / "orders.sqlite", self.binary)
        args.civvis_capture_inputs = True
        enabled = play.supervised_brain_command(args, self.run, self.run / "orders.sqlite", self.binary)
        self.assertEqual(enabled, default + ["--capture-inputs"])
        self.assertEqual(enabled.count("--capture-inputs"), 1)

    def test_cli_help_accepts_and_describes_opt_in(self):
        for module, flag in ((brain, "--capture-inputs"), (play, "--civvis-capture-inputs")):
            result = subprocess.run([sys.executable, module.__file__, flag, "--help"],
                                    capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(flag, result.stdout)


if __name__ == "__main__":
    unittest.main()
