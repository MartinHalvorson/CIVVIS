#!/usr/bin/env python3
"""Contracts for the durable, verified one-game retirement request."""

from __future__ import annotations

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import operator_retire  # noqa: E402


def harness(tag: str, pid: int = 4271) -> str:
    return (f" {pid} /usr/local/bin/python3 -u "
            f"/work/CIVVIS/tools/civ6_play.py --tag {tag} --civvis-decides\n")


class RetireRequestTest(unittest.TestCase):
    def make_live_run(self, root: Path, tag: str = "civvis-live") -> Path:
        run = root / tag
        run.mkdir(parents=True)
        (run / "events.jsonl").write_text(
            '{"kind":"seat","run":"civvis-live"}\n'
            '{"kind":"turn","run":"civvis-live","turn":4}\n')
        return run

    def test_only_an_actual_python_harness_can_own_the_request(self) -> None:
        rows = (
            " 71 /bin/zsh -c echo checking civ6_play.py --tag misleading\n"
            " 72 /usr/bin/osascript -e inspect civ6_play.py --tag also-misleading\n"
            + harness("civvis-live", 73)
        )
        self.assertEqual(
            operator_retire.live_harnesses(rows),
            [{"pid": 73, "tag": "civvis-live",
              "command": "/usr/local/bin/python3 -u "
                         "/work/CIVVIS/tools/civ6_play.py --tag civvis-live "
                         "--civvis-decides"}],
        )

    def test_request_binds_one_live_tagged_run_with_a_real_turn(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "control"
            run = self.make_live_run(root)
            requested = operator_retire.request_active_run(
                root, "operator: retire this game", ps_output=harness("civvis-live"),
                now="2026-08-29T15:00:00Z")

            self.assertEqual(requested["tag"], "civvis-live")
            self.assertEqual(requested["harness_pid"], 4271)
            self.assertEqual(requested["state"], "requested")
            self.assertEqual(
                json.loads((run / operator_retire.REQUEST_FILE).read_text()), requested)
            self.assertEqual(operator_retire.read_pending_request(run, "civvis-live"),
                             requested)
            self.assertIsNone(operator_retire.read_pending_request(run, "other-run"))

    def test_request_refuses_setup_ambiguous_or_already_requested_runs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "control"
            run = self.make_live_run(root)
            (run / "events.jsonl").write_text('{"kind":"seat"}\n')
            with self.assertRaisesRegex(operator_retire.RetireRequestError, "no recorded turn"):
                operator_retire.request_active_run(root, "operator", ps_output=harness("civvis-live"))

            (run / "events.jsonl").write_text('{"kind":"turn","turn":1}\n')
            with self.assertRaisesRegex(operator_retire.RetireRequestError, "choose among"):
                operator_retire.request_active_run(
                    root, "operator",
                    ps_output=harness("civvis-live", 1) + harness("second-live", 2),
                )

            operator_retire.request_active_run(root, "operator", ps_output=harness("civvis-live"))
            with self.assertRaisesRegex(operator_retire.RetireRequestError, "pending"):
                operator_retire.request_active_run(root, "operator", ps_output=harness("civvis-live"))

    def test_request_distinguishes_current_from_stale_continuation_summaries(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "control"
            run = self.make_live_run(root)
            summary = run / "summary.json"
            summary.write_text(json.dumps({"reason": "stalled"}))
            base_ns = 1_700_000_000_000_000_000
            os.utime(summary, ns=(base_ns, base_ns))
            os.utime(run / "events.jsonl", ns=(base_ns - 1, base_ns - 1))
            with self.assertRaisesRegex(operator_retire.RetireRequestError, "already complete"):
                operator_retire.request_active_run(
                    root, "operator", ps_output=harness("civvis-live"))

            with (run / "events.jsonl").open("a") as events:
                events.write('{"kind":"turn","turn":5}\n')
            os.utime(run / "events.jsonl", ns=(base_ns + 1, base_ns + 1))
            requested = operator_retire.request_active_run(
                root, "operator", ps_output=harness("civvis-live"))
            self.assertEqual(requested["tag"], "civvis-live")

    def test_recorded_retirement_closes_the_pending_request(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "control"
            run = self.make_live_run(root)
            requested = operator_retire.request_active_run(
                root, "operator: retire this game", ps_output=harness("civvis-live"))
            operator_retire.record_attempt(
                run, requested, "native retire order is awaiting acknowledgement")
            self.assertEqual(
                json.loads((run / operator_retire.STATUS_FILE).read_text())["state"], "pending")

            with mock.patch.object(operator_retire, "utc_stamp",
                                   return_value="2026-08-29T15:02:00Z"):
                retired = operator_retire.record_retired(
                    run, requested,
                    "the control mod acknowledged Civilization VI ACTION_RETIRE")
            self.assertEqual(retired["state"], "retired")
            self.assertEqual(retired["reason"], "operator: retire this game")
            self.assertEqual(retired["retired_utc"], "2026-08-29T15:02:00Z")
            self.assertIsNone(operator_retire.read_pending_request(run, "civvis-live"))


if __name__ == "__main__":
    unittest.main()
