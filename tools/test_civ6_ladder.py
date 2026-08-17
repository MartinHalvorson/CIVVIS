#!/usr/bin/env python3
"""The ladder records itself: live ledger, idempotence, backfill, staleness."""

from __future__ import annotations

import io
import json
import os
import sys
import threading
import time
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


class ScoreLead(LedgerCase):
    """The gap to the best rival rides the ledger; outcome.score cannot."""

    def test_final_standing_reads_the_last_turn_that_saw_both(self):
        events = self.runs / "events.jsonl"
        events.write_text("\n".join([
            json.dumps({"kind": "turn", "ctx": "agent", "turn": 1,
                        "score": 10, "rival_best": 12}),
            json.dumps({"kind": "turn", "ctx": "agent", "turn": 250,
                        "score": 731, "rival_best": 1200}),
            # The victory event's score is the LOCAL seat's, never a rival's.
            json.dumps({"kind": "victory", "score": 715, "team": 5}),
        ]))
        self.assertEqual(civ6_ladder.final_standing(events), (731, 1200))

    def test_the_lead_is_recorded_on_the_attempt(self):
        civ6_ladder.record_summary(write_run(
            self.runs, summary("gap", rival_best=1200, last_score=731)))
        entry = self.state()["attempts"][0]
        self.assertEqual(entry["rival_best"], 1200)
        self.assertEqual(entry["lead"], -469)

    def test_an_unmeasured_run_records_no_lead(self):
        civ6_ladder.record_summary(write_run(self.runs, summary("old")))
        entry = self.state()["attempts"][0]
        self.assertIsNone(entry["rival_best"])
        self.assertIsNone(entry["lead"])


class ConcurrentWriters(LedgerCase):
    """Two games finishing at once must not erase one another's attempt.

    Recording is a whole-file read-modify-write and this host plays games in
    parallel, so the window is real: on 2026-08-17 the runs directory held 41
    summaries the live ledger had never seen, including the first Settler win.
    """

    def record_together(self, tags: list[str], *, delay: float = 0.05) -> None:
        """Record every tag from its own thread, forcing them to interleave.

        The sleep goes between the load and the save so the race is certain
        rather than lucky: without the lock every writer reads the same empty
        ledger and the last one to finish wins.
        """
        paths = [write_run(self.runs, summary(tag)) for tag in tags]
        real_apply = civ6_ladder.apply

        def slow_apply(state, body):
            changed = real_apply(state, body)
            time.sleep(delay)
            return changed

        civ6_ladder.apply = slow_apply
        self.addCleanup(setattr, civ6_ladder, "apply", real_apply)
        threads = [threading.Thread(target=civ6_ladder.record_summary,
                                    args=(path,)) for path in paths]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join(timeout=60)

    def test_no_attempt_is_lost_when_four_runs_finish_at_once(self):
        tags = [f"civvis-{index}" for index in range(4)]
        self.record_together(tags)
        self.assertEqual(sorted(a["tag"] for a in self.state()["attempts"]),
                         sorted(tags))

    def test_the_ledger_is_replaced_atomically_and_leaves_no_scratch(self):
        civ6_ladder.record_summary(write_run(self.runs, summary("civvis-1")))
        leftovers = [p.name for p in self.runs.iterdir()
                     if p.name.endswith(".tmp") or p.name.endswith(".lock")]
        self.assertEqual(leftovers, [])
        # Readable as JSON means the replace was whole-file, not a truncate.
        self.assertEqual(len(self.state()["attempts"]), 1)

    def test_a_lock_left_by_a_dead_writer_is_broken_not_waited_on(self):
        lock = self.ledger.parent / (self.ledger.name + ".lock")
        lock.write_text("99999\n")
        old = time.time() - 600
        os.utime(lock, (old, old))
        # stale_after is 120s by default, so a ten-minute-old lock is a corpse.
        civ6_ladder.record_summary(write_run(self.runs, summary("civvis-1")))
        self.assertEqual(len(self.state()["attempts"]), 1)
        self.assertFalse(lock.exists())

    def test_a_live_lock_is_reported_rather_than_silently_ignored(self):
        lock = self.ledger.parent / (self.ledger.name + ".lock")
        lock.write_text("1\n")
        self.addCleanup(lambda: lock.unlink(missing_ok=True))
        with self.assertRaises(civ6_ladder.LedgerBusy) as caught:
            with civ6_ladder.ledger_lock(self.ledger, timeout=0.05):
                pass
        # The message must name the remedy: the summary is still on disk.
        self.assertIn("sync", str(caught.exception))


class TheEarliestWinIsTheMilestone(LedgerCase):
    """A rung is claimed at the time it was first climbed, not first filed."""

    def test_a_win_recorded_late_moves_the_milestone_back(self):
        # The order this host actually produced: the 23:23Z win recorded
        # itself, the 06:49Z win was dropped, and `sync` rescued it later.
        civ6_ladder.record_summary(write_run(self.runs, summary(
            "second-win", won=True, finished="2026-08-16T23:23:58Z")))
        self.assertEqual(
            self.state()["wins"]["DIFFICULTY_SETTLER"]["tag"], "second-win")
        write_run(self.runs, summary(
            "first-win", won=True, finished="2026-08-16T06:49:58Z"))
        with redirect_stdout(io.StringIO()):
            civ6_ladder.sync(self.runs, self.ledger)
        win = self.state()["wins"]["DIFFICULTY_SETTLER"]
        self.assertEqual(win["tag"], "first-win")
        self.assertEqual(win["utc"], "2026-08-16T06:49:58Z")

    def test_a_later_win_never_moves_the_milestone_forward(self):
        civ6_ladder.record_summary(write_run(self.runs, summary(
            "first-win", won=True, finished="2026-08-16T06:49:58Z")))
        civ6_ladder.record_summary(write_run(self.runs, summary(
            "second-win", won=True, finished="2026-08-16T23:23:58Z")))
        self.assertEqual(
            self.state()["wins"]["DIFFICULTY_SETTLER"]["tag"], "first-win")

    def test_sync_recovers_a_win_the_live_record_dropped(self):
        write_run(self.runs, summary("dropped-win", won=True,
                                     finished="2026-08-16T06:49:58Z"))
        with redirect_stdout(io.StringIO()):
            civ6_ladder.sync(self.runs, self.ledger)
        self.assertIn("DIFFICULTY_SETTLER", self.state()["wins"])


class TheTableSaysWhatHappened(LedgerCase):
    """Absent and zero are different answers, and Score Victory is zero."""

    def test_a_score_victory_is_named_not_rendered_as_unknown(self):
        civ6_ladder.record_summary(write_run(self.runs, summary(
            "score-win", won=True,
            outcome={"kind": "victory", "won": True, "victory": 0})))
        # The rung table only; the attempt log below it repeats the tag.
        row = [line for line in
               civ6_ladder.markdown_for(self.state()).splitlines()
               if line.startswith("| 1 | Settler |")]
        self.assertEqual(len(row), 1, "the rung row should be rendered once")
        self.assertNotIn("?", row[0])
        self.assertIn("| 0 |", row[0])

    def test_cell_tells_absent_and_zero_apart(self):
        self.assertEqual(civ6_ladder.cell(0), "0")
        self.assertEqual(civ6_ladder.cell(None), "—")
        self.assertEqual(civ6_ladder.cell(""), "—")
        self.assertEqual(civ6_ladder.cell(250), "250")

    def test_an_attempt_with_no_score_renders_a_dash_not_none(self):
        civ6_ladder.record_summary(write_run(
            self.runs, summary("thin", last_turn=None, last_score=None)))
        table = civ6_ladder.markdown_for(self.state())
        self.assertNotIn("None", table)


class ThePublishedPairAgrees(unittest.TestCase):
    """The committed markdown must be derivable from the committed snapshot.

    ⚠ THE TWO COMMITTED FILES ARE ONE RECORD IN TWO FORMATS AND NOTHING JOINED
    THEM. `docs/CIV6_LADDER.md` says "do not edit it by hand" and no gate could
    tell whether somebody had. This is the join: regenerate the markdown from
    the snapshot beside it and require them to match, so a hand edit, a partial
    publish, or a snapshot landed without its markdown fails the suite that
    already runs on every pull request.
    """

    def test_the_markdown_matches_its_snapshot(self):
        if not civ6_ladder.DATA.is_file():
            self.skipTest("no published snapshot yet")
        state = json.loads(civ6_ladder.DATA.read_text())
        self.assertEqual(
            civ6_ladder.LEDGER.read_text(),
            civ6_ladder.markdown_for(state),
            "docs/CIV6_LADDER.md is out of step with docs/civ6_ladder.json; "
            "run `python3 tools/civ6_ladder.py publish` and land both files")


class LatestCodeGuarantee(LedgerCase):
    """The run's revision history reaches the ledger; the watcher's silence
    fails check."""

    def test_decider_revisions_read_start_and_handoffs_deduplicated(self):
        updates = self.runs / "runtime_updates.jsonl"
        rows = [
            {"kind": "runtime_update", "status": "start", "to_revision": "aaa"},
            {"kind": "runtime_update", "status": "handoff",
             "from_revision": "aaa", "to_revision": "bbb"},
            # The handoff re-execs the brain, whose fresh start repeats bbb.
            {"kind": "runtime_update", "status": "start", "to_revision": "bbb"},
            {"kind": "runtime_update", "status": "failed_reexec",
             "to_revision": "ccc"},  # not a code change; the bridge stayed old
            {"kind": "runtime_update", "status": "handoff",
             "from_revision": "bbb", "to_revision": "ddd"},
        ]
        updates.write_text("\n".join(json.dumps(r) for r in rows))
        self.assertEqual(civ6_ladder.decider_revisions(updates),
                         ["aaa", "bbb", "ddd"])
        self.assertIsNone(civ6_ladder.decider_revisions(self.runs / "absent"))

    def test_the_revision_history_is_recorded_on_the_attempt(self):
        civ6_ladder.record_summary(write_run(
            self.runs, summary("tracked", decider_revisions=["aaa", "bbb"])))
        self.assertEqual(self.state()["attempts"][0]["revisions"],
                         ["aaa", "bbb"])

    def heartbeat_problem(self, payload, minutes=10,
                          now=datetime(2026, 8, 17, 12, 0,
                                       tzinfo=timezone.utc)):
        beat = self.runs / "cache" / "heartbeat.json"
        beat.parent.mkdir(parents=True, exist_ok=True)
        if payload is not None:
            beat.write_text(json.dumps(payload))
        return civ6_ladder.runtime_heartbeat_problem(beat, minutes, now=now)

    def test_a_machine_without_the_runtime_cache_is_nobodys_problem(self):
        absent = self.runs / "never-existed" / "heartbeat.json"
        self.assertIsNone(
            civ6_ladder.runtime_heartbeat_problem(absent, 10))

    def test_a_cache_with_no_heartbeat_fails(self):
        problem = self.heartbeat_problem(None)
        self.assertIn("no heartbeat", problem)

    def test_a_fresh_clean_heartbeat_passes_and_a_stale_one_fails(self):
        fresh = {"utc": "2026-08-17T11:55:00Z", "last_error": ""}
        self.assertIsNone(self.heartbeat_problem(fresh))
        stale = {"utc": "2026-08-17T09:00:00Z", "last_error": ""}
        self.assertIn("old", self.heartbeat_problem(stale))

    def test_a_reported_refresh_error_fails_even_when_fresh(self):
        beat = {"utc": "2026-08-17T11:59:00Z",
                "last_error": "cargo build failed (101): expected `;`"}
        self.assertIn("cargo build failed", self.heartbeat_problem(beat))
