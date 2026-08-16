#!/usr/bin/env python3
"""The ladder records itself: live ledger, idempotence, backfill, staleness."""

from __future__ import annotations

import io
import json
import sys
import unittest
from contextlib import redirect_stdout
from datetime import datetime, timezone
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402


def summary(tag: str, *, won: bool = False, configured: bool = True,
            difficulty: str = "DIFFICULTY_SETTLER",
            finished: str = "2026-08-16T12:00:00Z", **extra) -> dict:
    body = {
        "tag": tag,
        "finished_utc": finished,
        "difficulty": difficulty,
        "configured": configured,
        "last_turn": 100,
        "last_score": 200,
        "map_size": "MAPSIZE_TINY",
        "speed": "GAMESPEED_ONLINE",
        "reason": "stopped",
        "outcome": {"kind": "victory", "won": won, "victory": 3}
                   if won else None,
    }
    body.update(extra)
    return body


def write_run(runs: Path, body: dict) -> Path:
    run_dir = runs / body["tag"]
    run_dir.mkdir(parents=True)
    path = run_dir / "summary.json"
    path.write_text(json.dumps(body))
    return path


class LedgerCase(unittest.TestCase):
    """Shared temp-dir harness: runs dir, live ledger, patched snapshot."""

    def setUp(self):
        self._tmp = TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        root = Path(self._tmp.name)
        self.runs = root / "control"
        self.runs.mkdir()
        self.ledger = civ6_ladder.live_ledger_for(self.runs)
        self.snapshot = root / "civ6_ladder.json"
        self.markdown = root / "CIV6_LADDER.md"
        # Point the module's committed-snapshot default at a temp path so a
        # test can never seed from — or write over — the real docs record.
        self._data, civ6_ladder.DATA = civ6_ladder.DATA, self.snapshot
        self._ledger_md, civ6_ladder.LEDGER = civ6_ladder.LEDGER, self.markdown
        self.addCleanup(lambda: setattr(civ6_ladder, "DATA", self._data))
        self.addCleanup(lambda: setattr(civ6_ladder, "LEDGER", self._ledger_md))

    def state(self) -> dict:
        return json.loads(self.ledger.read_text())


class RecordsItself(LedgerCase):
    def test_the_ledger_lives_beside_the_runs_it_records(self):
        path = write_run(self.runs, summary("civvis-1"))
        self.assertTrue(civ6_ladder.record_summary(path))
        self.assertEqual(self.ledger, self.runs / "ladder.json")
        self.assertEqual([a["tag"] for a in self.state()["attempts"]],
                         ["civvis-1"])

    def test_recording_the_same_tag_twice_counts_once(self):
        path = write_run(self.runs, summary("civvis-1"))
        self.assertTrue(civ6_ladder.record_summary(path))
        self.assertFalse(civ6_ladder.record_summary(path))
        self.assertEqual(len(self.state()["attempts"]), 1)

    def test_a_configured_victory_claims_the_rung_and_the_first_win_stands(self):
        civ6_ladder.record_summary(write_run(
            self.runs, summary("first-win", won=True,
                               finished="2026-08-16T12:00:00Z")))
        civ6_ladder.record_summary(write_run(
            self.runs, summary("second-win", won=True,
                               finished="2026-08-17T12:00:00Z")))
        win = self.state()["wins"]["DIFFICULTY_SETTLER"]
        self.assertEqual(win["tag"], "first-win")

    def test_an_unconfigured_victory_claims_nothing(self):
        civ6_ladder.record_summary(write_run(
            self.runs, summary("menu-defaults", won=True, configured=False)))
        self.assertEqual(self.state()["wins"], {})
        self.assertTrue(self.state()["attempts"][0]["won"])

    def test_a_fresh_ledger_seeds_from_the_committed_snapshot(self):
        self.snapshot.write_text(json.dumps({
            "attempts": [{"tag": "historic", "utc": "2026-07-30T00:00:00Z",
                          "difficulty": "DIFFICULTY_SETTLER",
                          "configured": True, "won": False}],
            "wins": {}}))
        civ6_ladder.record_summary(write_run(self.runs, summary("civvis-1")))
        self.assertEqual([a["tag"] for a in self.state()["attempts"]],
                         ["historic", "civvis-1"])

    def test_sync_backfills_every_unrecorded_summary_oldest_first(self):
        write_run(self.runs, summary("late", finished="2026-08-16T12:00:00Z"))
        write_run(self.runs, summary("early", finished="2026-08-14T12:00:00Z"))
        recorded_path = write_run(
            self.runs, summary("already", finished="2026-08-15T12:00:00Z"))
        civ6_ladder.record_summary(recorded_path)
        with redirect_stdout(io.StringIO()):
            civ6_ladder.sync(self.runs, self.ledger)
        self.assertEqual([a["tag"] for a in self.state()["attempts"]],
                         ["already", "early", "late"])

    def test_publish_writes_the_snapshot_and_the_markdown(self):
        civ6_ladder.record_summary(write_run(self.runs, summary("civvis-1")))
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        self.assertEqual(json.loads(self.snapshot.read_text()), self.state())
        self.assertIn("civvis-1", self.markdown.read_text())

    def check(self, stale_hours=None, now=None) -> int:
        with redirect_stdout(io.StringIO()) as out:
            code = civ6_ladder.check(self.runs, self.ledger, stale_hours,
                                     self.snapshot, now=now)
        self.last_report = out.getvalue()
        return code

    def test_check_fails_on_an_unrecorded_summary(self):
        write_run(self.runs, summary("unrecorded"))
        self.assertEqual(self.check(), 1)
        self.assertIn("sync", self.last_report)

    def test_check_fails_when_the_snapshot_trails_the_ledger(self):
        civ6_ladder.record_summary(write_run(self.runs, summary("civvis-1")))
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        civ6_ladder.record_summary(write_run(self.runs, summary("civvis-2")))
        self.assertEqual(self.check(), 1)
        self.assertIn("publish", self.last_report)

    def test_check_fails_when_no_run_has_finished_recently(self):
        civ6_ladder.record_summary(write_run(
            self.runs, summary("old", finished="2026-08-15T00:00:00Z")))
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        late = datetime(2026, 8, 16, tzinfo=timezone.utc)
        self.assertEqual(self.check(stale_hours=12, now=late), 1)
        self.assertIn("supervisor", self.last_report)
        self.assertEqual(self.check(stale_hours=36, now=late), 0)

    def test_check_passes_when_everything_is_current(self):
        civ6_ladder.record_summary(write_run(self.runs, summary("civvis-1")))
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        self.assertEqual(self.check(), 0)


if __name__ == "__main__":
    unittest.main()


class BridgeHealth(LedgerCase):
    """applied_pct rides the ledger; check floors it."""

    def test_orders_totals_sums_agent_turns_and_survives_a_torn_tail(self):
        events = self.runs / "events.jsonl"
        lines = [
            json.dumps({"kind": "turn", "ctx": "agent", "turn": 1,
                        "orders_seen": 4, "orders_applied": 4}),
            json.dumps({"kind": "turn", "ctx": "agent", "turn": 2,
                        "orders_seen": 6, "orders_applied": 3}),
            # A different context and a non-turn row must not count.
            json.dumps({"kind": "turn", "ctx": "ui", "turn": 2,
                        "orders_seen": 9, "orders_applied": 9}),
            json.dumps({"kind": "victory", "orders_seen": 9}),
            '{"kind": "turn", "ctx": "agent", "orders_se',  # torn tail
        ]
        events.write_text("\n".join(lines))
        self.assertEqual(civ6_ladder.orders_totals(events), (10, 7))

    def test_the_rate_is_recorded_on_the_attempt(self):
        civ6_ladder.record_summary(write_run(
            self.runs, summary("measured", orders_seen=200, orders_applied=194)))
        self.assertEqual(self.state()["attempts"][0]["applied_pct"], 97.0)

    def test_a_run_without_totals_records_no_rate_and_fails_nothing(self):
        civ6_ladder.record_summary(write_run(self.runs, summary("unmeasured")))
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        self.assertIsNone(self.state()["attempts"][0]["applied_pct"])
        self.assertEqual(
            civ6_ladder.check(self.runs, self.ledger, None, self.snapshot,
                              min_applied=95.0), 0)

    def test_check_floors_the_newest_measured_run(self):
        civ6_ladder.record_summary(write_run(
            self.runs, summary("healthy", orders_seen=100, orders_applied=97,
                               finished="2026-08-15T00:00:00Z")))
        civ6_ladder.record_summary(write_run(
            self.runs, summary("regressed", orders_seen=100, orders_applied=80,
                               finished="2026-08-16T00:00:00Z")))
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        with redirect_stdout(io.StringIO()) as out:
            code = civ6_ladder.check(self.runs, self.ledger, None,
                                     self.snapshot, min_applied=95.0)
        self.assertEqual(code, 1)
        self.assertIn("refusal ledger", out.getvalue())
        # And an unmeasured run arriving later must not mask the reading:
        # the floor reads the newest run that measured itself.
        civ6_ladder.record_summary(write_run(
            self.runs, summary("died-early", finished="2026-08-17T00:00:00Z")))
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        with redirect_stdout(io.StringIO()):
            code = civ6_ladder.check(self.runs, self.ledger, None,
                                     self.snapshot, min_applied=95.0)
        self.assertEqual(code, 1)
