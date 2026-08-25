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
    def test_the_deal_lane_is_summed_onto_the_ledger(self):
        events = self.runs / "civvis-20260824T230000Z" / "events.jsonl"
        events.parent.mkdir(parents=True, exist_ok=True)
        lines = [
            {"kind": "turn", "ctx": "agent", "turn": 5},
            {"kind": "deal_session", "phase": "opening", "target": 3},
            {"kind": "deal_session", "phase": "asked", "target": 3},
            {"kind": "deal_session", "phase": "answered", "target": 3},
            {"kind": "deal_closed", "target": 3, "gold": 90},
            {"kind": "deal_session", "phase": "closed", "target": 3},
            {"kind": "deal_session", "phase": "opening", "target": 4},
            {"kind": "deal_session", "phase": "unanswered", "target": 4},
            {"kind": "deal_expired", "target": 4},
            {"kind": "deal_session", "phase": "opening", "target": 5},
            {"kind": "deal_session", "phase": "answered", "target": 5},
            {"kind": "deal_declined", "target": 5, "worth": 20},
            {"kind": "peace_response", "target": 6, "accepted": True},
            {"kind": "peace_response", "target": 7, "accepted": False},
            {"kind": "deal_sessions_stood_down", "unanswered": 3},
        ]
        with events.open("w") as handle:
            for line in lines:
                handle.write(json.dumps(line) + "\n")
            handle.write('{"kind": "deal_closed", "tru')  # a torn tail line
        self.assertEqual(civ6_ladder.deal_totals(events), {
            "sessions_opened": 3, "sessions_answered": 2,
            "sessions_unanswered": 1, "stood_down": True,
            "closed": 1, "declined": 1, "expired": 1,
            "peace_accepted": 1, "peace_refused": 1,
        })
        # A run that wrote no deal event reads as silence, not zeros.
        quiet = self.runs / "civvis-20260824T230100Z" / "events.jsonl"
        quiet.parent.mkdir(parents=True, exist_ok=True)
        quiet.write_text('{"kind": "turn", "ctx": "agent", "turn": 5}\n')
        self.assertIsNone(civ6_ladder.deal_totals(quiet))
        self.assertIsNone(civ6_ladder.deal_totals(self.runs / "missing" / "events.jsonl"))

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



class LiveArmAttributionTests(unittest.TestCase):
    """A row must say WHICH ARM played it, or two pinned batches cannot be
    compared after the fact — the precondition for pricing anything live."""

    def test_both_halves_of_the_arm_ride_the_entry(self) -> None:
        entry = civ6_ladder.entry_from({
            "tag": "civvis-x", "last_turn": 250,
            "withheld": ["peacetime-deterrence"],
            "mod_arms": {"PeaceDeterrence": False},
        })
        self.assertEqual(entry["withheld"], ["peacetime-deterrence"])
        self.assertEqual(entry["mod_arms"], {"PeaceDeterrence": False})

    def test_an_older_row_is_unknown_not_a_full_bundle(self) -> None:
        # None means "nobody could record it", which is a different claim from
        # [] ("the full shipped bundle played"). Reading the first as the
        # second would silently pool control rows with treatment rows.
        entry = civ6_ladder.entry_from({"tag": "old", "last_turn": 250})
        self.assertIsNone(entry["withheld"])
        self.assertIsNone(entry["mod_arms"])
        full = civ6_ladder.entry_from({"tag": "new", "last_turn": 250, "withheld": []})
        self.assertEqual(full["withheld"], [])
class AnAbandonedRowIsALossTheLadderChoseNotToPlayOut(unittest.TestCase):
    """An early-stop policy is a loss the ladder chose not to play out.

    The verdict must retain whether it was the measured expected-win rule or
    the operator's all-three-standings restart rule, or the ledger reads a
    deliberately stopped game as a wedge.
    """

    def test_the_verdict_rides_the_entry_and_the_outcome_column_names_it(self) -> None:
        verdict = {"turn": 124, "score": 300, "rival_best": 500,
                   "expected_win_rate": 0.0278, "floor": 0.05,
                   "consecutive_turns": 5}
        entry = civ6_ladder.entry_from({
            "tag": "civvis-ab", "difficulty": "DIFFICULTY_CHIEFTAIN",
            "configured": True, "last_turn": 124, "last_score": 300,
            "reason": "abandoned", "abandoned": verdict,
        })
        self.assertEqual(entry["abandoned"], verdict)
        self.assertEqual(entry["reason"], "abandoned")
        self.assertFalse(entry["won"])
        self.assertFalse(entry["defeat"])
        state = {"attempts": [entry], "wins": {}}
        text = civ6_ladder.markdown_for(state)
        self.assertIn("| abandoned |", text)
        self.assertIn("`abandoned` means the harness stopped under a recorded early-stop", text)
        self.assertIn("science and culture leaders", text)

    def test_a_three_signal_restart_verdict_is_kept_verbatim(self) -> None:
        verdict = {"rule": "score_science_culture_deficit", "turn": 104,
                   "score": 69, "rival_best": 100, "score_ratio": 0.69,
                   "score_ratio_ceiling": 0.70, "science": 9,
                   "rival_best_science": 10, "culture": 8,
                   "rival_best_culture": 10, "consecutive_turns": 5}
        entry = civ6_ladder.entry_from({
            "tag": "civvis-restart", "configured": True, "last_turn": 104,
            "last_score": 69, "reason": "abandoned", "abandoned": verdict,
        })
        self.assertEqual(entry["abandoned"], verdict)

    def test_an_ordinary_row_carries_no_verdict(self) -> None:
        entry = civ6_ladder.entry_from({"tag": "old", "last_turn": 250})
        self.assertIsNone(entry["abandoned"])


class OpeningTempoTests(unittest.TestCase):
    """The ladder's strongest measured correlate, watched instead of
    reconstructed: cities at t60 (r=+0.69 with final lead) and the second
    city's founding turn (r=-0.49)."""

    def test_the_tempo_columns_ride_the_entry(self) -> None:
        entry = civ6_ladder.entry_from({
            "tag": "civvis-x", "difficulty": "DIFFICULTY_SETTLER",
            "configured": True, "last_turn": 250, "last_score": 900,
            "city_two_turn": 21, "cities_at_60": 6,
        })
        self.assertEqual(entry["city_two_turn"], 21)
        self.assertEqual(entry["cities_at_60"], 6)

    def test_a_summary_without_the_columns_records_none(self) -> None:
        entry = civ6_ladder.entry_from({"tag": "old", "last_turn": 250})
        self.assertIsNone(entry["city_two_turn"])
        self.assertIsNone(entry["cities_at_60"])

    def test_a_slow_opening_is_alarmed_on_the_median_not_one_run(self) -> None:
        # Nine fast runs and one very late founding: ordinary map variance,
        # and the alarm must stay silent.
        attempts = [{"city_two_turn": t} for t in [20, 22, 19, 25, 21, 20, 23, 24, 22, 66]]
        self.assertIsNone(civ6_ladder.opening_tempo_problem(attempts, 30))
        # A window whose MIDDLE has slipped is the empire, not the map.
        slow = [{"city_two_turn": t} for t in [41, 44, 39, 54, 57, 45, 48, 40, 52, 43]]
        problem = civ6_ladder.opening_tempo_problem(slow, 30)
        self.assertIsNotNone(problem)
        self.assertIn("opening tempo regressed", problem)

    def test_too_few_rows_says_nothing_rather_than_passing(self) -> None:
        # A ledger recorded before the column existed must not read as healthy
        # OR as broken; there is simply nothing to say.
        thin = [{"city_two_turn": 50}, {"city_two_turn": 60}]
        self.assertIsNone(civ6_ladder.opening_tempo_problem(thin, 30))
        self.assertIsNone(civ6_ladder.opening_tempo_problem([{"turns": 250}] * 40, 30))

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

    def test_applied_counts_verified_orders_and_reported_keeps_the_return_codes(self):
        # Three turns: t1 verified 3 of 4 reported ok; t2 verified 1 of 3; t3
        # is the last turn and never gets a verdict, so it keeps its reported
        # count. A legacy `turn` row (no `orders_reported`) reads its
        # `orders_applied` as the reported count.
        events = self.runs / "events.jsonl"
        lines = [
            json.dumps({"kind": "turn", "ctx": "agent", "turn": 1,
                        "orders_seen": 5, "orders_applied": 4, "orders_reported": 4}),
            json.dumps({"kind": "turn_verified", "ctx": "agent", "turn": 1,
                        "checked_on": 2, "orders_issued": 5, "orders_applied": 3,
                        "orders_failed": 2, "orders_unverifiable": 0,
                        "orders_seen": 5, "orders_reported": 4}),
            json.dumps({"kind": "turn", "ctx": "agent", "turn": 2,
                        "orders_seen": 3, "orders_applied": 3}),
            json.dumps({"kind": "turn_verified", "ctx": "agent", "turn": 2,
                        "checked_on": 3, "orders_applied": 1}),
            json.dumps({"kind": "turn", "ctx": "agent", "turn": 3,
                        "orders_seen": 2, "orders_applied": 2, "orders_reported": 2}),
            # Verdict events are the ledger's; a UI-context copy must not count.
            json.dumps({"kind": "turn_verified", "ctx": "ui", "turn": 3,
                        "orders_applied": 9}),
        ]
        events.write_text("\n".join(lines) + "\n")
        self.assertEqual(civ6_ladder.orders_ledger(events), {
            "orders_seen": 10, "orders_reported": 9, "orders_applied": 6,
            "orders_verified_turns": 2, "orders_unverified_turns": 1})
        self.assertEqual(civ6_ladder.orders_totals(events), (10, 6))

    def test_the_summary_carries_both_counts_and_both_rates(self):
        events_lines = [
            json.dumps({"kind": "turn", "ctx": "agent", "turn": 1,
                        "orders_seen": 10, "orders_applied": 10, "orders_reported": 10}),
            json.dumps({"kind": "turn_verified", "ctx": "agent", "turn": 1,
                        "orders_applied": 6}),
        ]
        # A summary the harness wrote with totals but before `orders_reported`
        # existed gets the reported count filled in; the totals stay its own.
        path = write_run(self.runs, summary("verified", orders_seen=10, orders_applied=6))
        (path.parent / "events.jsonl").write_text("\n".join(events_lines) + "\n")
        civ6_ladder.record_summary(path)
        attempt = self.state()["attempts"][0]
        self.assertEqual(attempt["applied_pct"], 60.0)
        self.assertEqual(attempt["reported_pct"], 100.0)
        enriched = civ6_ladder.with_bridge_health(json.loads(path.read_text()), path)
        self.assertEqual(enriched["orders_applied"], 6)
        self.assertEqual(enriched["orders_reported"], 10)
        self.assertEqual(enriched["orders_unverified_turns"], 0)
        # A summary with no totals at all is filled from the events, verified
        # count and reported count both.
        bare = write_run(self.runs, summary("bare"))
        (bare.parent / "events.jsonl").write_text("\n".join(events_lines) + "\n")
        filled = civ6_ladder.with_bridge_health(json.loads(bare.read_text()), bare)
        self.assertEqual((filled["orders_seen"], filled["orders_applied"],
                          filled["orders_reported"]), (10, 6, 10))
        # A summary that already carries everything is returned as it is.
        full = {"orders_seen": 1, "orders_applied": 1, "orders_reported": 1}
        self.assertIs(civ6_ladder.with_bridge_health(full, bare), full)

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


VICTORY_TABLE = [
    {"index": 0, "type": "VICTORY_SCORE"},
    {"index": 3, "type": "VICTORY_CULTURE"},
    {"index": 5, "type": "VICTORY_TECHNOLOGY"},
]


class TheVictoryHasAName(LedgerCase):
    """The index the event reported, glossed by the host's own victory table.

    The milestones are stated per victory type, so a row reading `5` cannot
    substantiate a Science claim or refute a Culture one.
    """

    def test_the_name_comes_from_the_host_table_in_the_same_record(self):
        entry = civ6_ladder.entry_from(summary(
            "named", won=True,
            outcome={"kind": "victory", "won": True, "victory": 5},
            seat={"victory_types": VICTORY_TABLE}))
        self.assertEqual(entry["victory"], 5)
        self.assertEqual(entry["victory_type"], "VICTORY_TECHNOLOGY")

    def test_a_run_without_the_export_keeps_the_index_and_names_nothing(self):
        """⚠ The whole point of the raw index is that a missing table produces a
        blank, never an invented literal."""
        entry = civ6_ladder.entry_from(summary(
            "unnamed", won=True,
            outcome={"kind": "victory", "won": True, "victory": 5}))
        self.assertEqual(entry["victory"], 5)
        self.assertIsNone(entry["victory_type"])

    def test_an_index_absent_from_the_table_is_not_guessed(self):
        entry = civ6_ladder.entry_from(summary(
            "gap", won=True,
            outcome={"kind": "victory", "won": True, "victory": 4},
            seat={"victory_types": VICTORY_TABLE}))
        self.assertIsNone(entry["victory_type"])

    def test_a_run_that_never_ended_has_no_victory_at_all(self):
        entry = civ6_ladder.entry_from(summary("stopped"))
        self.assertIsNone(entry["victory"])
        self.assertIsNone(entry["victory_type"])

    def test_the_rung_row_carries_the_name_beside_the_index(self):
        civ6_ladder.record_summary(write_run(self.runs, summary(
            "science-win", won=True,
            outcome={"kind": "victory", "won": True, "victory": 5},
            seat={"victory_types": VICTORY_TABLE})))
        row = next(line for line in
                   civ6_ladder.markdown_for(self.state()).splitlines()
                   if line.startswith("| 1 | Settler |"))
        self.assertIn("| 5 | VICTORY_TECHNOLOGY |", row)


class RedrawingIsNotPublishing(LedgerCase):
    """⚠ `publish` imports this machine's runs; `render` cannot.

    A contributor changing how a row is drawn reaches for the command that
    rewrites the file, and `publish` was the only one — so redrawing the table
    also landed whatever attempts the local ledger happened to hold. On this
    laptop that was eighteen rows of fortnight-old experiments, dated before the
    newest row already in the snapshot.
    """

    def _snapshot(self, attempts):
        path = Path(self._tmp.name) / "snapshot.json"
        path.write_text(json.dumps({"attempts": attempts, "wins": {}}))
        return path

    def test_render_reads_the_snapshot_and_writes_only_the_markdown(self):
        snapshot = self._snapshot([{"tag": "a", "won": False, "configured": True,
                                    "difficulty": "DIFFICULTY_SETTLER",
                                    "turns": 250, "score": 100, "utc": "x"}])
        before = snapshot.read_text()
        markdown = Path(self._tmp.name) / "out.md"
        civ6_ladder.render(snapshot, markdown)
        self.assertEqual(snapshot.read_text(), before)
        self.assertIn("`a`", markdown.read_text())

    def test_render_ignores_the_live_ledger_entirely(self):
        """The signature has no ledger in it — the guarantee is structural."""
        civ6_ladder.record_summary(write_run(self.runs, summary("local-only")))
        snapshot = self._snapshot([])
        markdown = Path(self._tmp.name) / "out.md"
        civ6_ladder.render(snapshot, markdown)
        self.assertNotIn("local-only", markdown.read_text())
        self.assertEqual(json.loads(snapshot.read_text())["attempts"], [])

    def test_publish_still_lands_the_live_ledger(self):
        """The import path is unchanged; only the accident is removed."""
        civ6_ladder.record_summary(write_run(self.runs, summary("landed")))
        snapshot = Path(self._tmp.name) / "snapshot.json"
        markdown = Path(self._tmp.name) / "out.md"
        civ6_ladder.publish(self.ledger, snapshot, markdown)
        tags = [a["tag"] for a in json.loads(snapshot.read_text())["attempts"]]
        self.assertIn("landed", tags)


class TheBoardRecordsEveryVictoryNotJustTheRung(LedgerCase):
    """★★★★★ `wins` has ONE SLOT PER DIFFICULTY and the Settler slot is full.

    The objective list is five victory types at one difficulty. The rung claim
    keeps the earliest win at a difficulty — right for a rung, and unable to
    represent "Settler, but by Science this time": a later win at a claimed rung
    loses the comparison and survives only as an ordinary attempt row.
    """

    def _state(self, *attempts, victory_types=None):
        state = {"attempts": list(attempts), "wins": {}}
        if victory_types is not None:
            state["victory_types"] = victory_types
        return state

    @staticmethod
    def _win(index, utc, difficulty="DIFFICULTY_SETTLER", name=None,
             configured=True):
        return {"victory": index, "victory_type": name, "utc": utc, "won": True,
                "configured": configured, "difficulty": difficulty}

    def test_a_second_victory_type_at_a_claimed_rung_is_recorded(self):
        """The exact case the rung table drops."""
        board = civ6_ladder.victory_board(self._state(
            self._win(0, "2026-08-16T06:49:58Z", name="VICTORY_SCORE"),
            self._win(5, "2026-08-20T00:00:00Z", name="VICTORY_TECHNOLOGY"),
        ))
        rows = {index: beaten for index, _, beaten in board}
        self.assertEqual(rows[0]["DIFFICULTY_SETTLER"], "2026-08-16T06:49:58Z")
        self.assertEqual(rows[5]["DIFFICULTY_SETTLER"], "2026-08-20T00:00:00Z")

    def test_the_rung_table_really_does_drop_it(self):
        """Not an assumption about the other code — a check on it, so this
        test fails loudly if `wins` ever gains its own per-type key."""
        state = {"attempts": [], "wins": {}}
        for tag, utc, victory in (("score-first", "2026-08-16T06:49:58Z", 0),
                                  ("science-later", "2026-08-20T00:00:00Z", 5)):
            civ6_ladder.apply(state, summary(
                tag, won=True, finished=utc,
                outcome={"kind": "victory", "won": True, "victory": victory}))
        self.assertEqual(len(state["wins"]), 1)
        self.assertEqual(state["wins"]["DIFFICULTY_SETTLER"]["victory"], 0)
        # ...while the board carries both.
        self.assertEqual(
            sorted(index for index, _, _ in civ6_ladder.victory_board(state)),
            [0, 5])

    def test_earliest_wins_per_type_the_same_way_the_rung_does(self):
        board = civ6_ladder.victory_board(self._state(
            self._win(5, "2026-08-20T00:00:00Z"),
            self._win(5, "2026-08-18T00:00:00Z"),
        ))
        self.assertEqual(board[0][2]["DIFFICULTY_SETTLER"], "2026-08-18T00:00:00Z")

    def test_each_difficulty_is_its_own_column(self):
        board = civ6_ladder.victory_board(self._state(
            self._win(5, "2026-08-20T00:00:00Z"),
            self._win(5, "2026-08-25T00:00:00Z", difficulty="DIFFICULTY_CHIEFTAIN"),
        ))
        self.assertEqual(board[0][2], {
            "DIFFICULTY_SETTLER": "2026-08-20T00:00:00Z",
            "DIFFICULTY_CHIEFTAIN": "2026-08-25T00:00:00Z"})

    def test_a_win_in_a_game_that_drifted_off_its_settings_claims_nothing(self):
        self.assertEqual(civ6_ladder.victory_board(self._state(
            self._win(5, "2026-08-20T00:00:00Z", configured=False))), [])

    def test_a_difficulty_that_is_not_a_rung_claims_nothing(self):
        self.assertEqual(civ6_ladder.victory_board(self._state(
            self._win(5, "2026-08-20T00:00:00Z", difficulty="DIFFICULTY_SANDBOX"))),
            [])

    def test_every_victory_the_host_offers_gets_a_row_even_unbeaten(self):
        """A checklist, not a list of things that happened."""
        board = civ6_ladder.victory_board(self._state(
            self._win(0, "2026-08-16T06:49:58Z"),
            victory_types=[{"index": 0, "type": "VICTORY_SCORE"},
                           {"index": 5, "type": "VICTORY_TECHNOLOGY"},
                           {"index": 3, "type": "VICTORY_CULTURE"}]))
        self.assertEqual([(index, name) for index, name, _ in board],
                         [(0, "VICTORY_SCORE"), (5, "VICTORY_TECHNOLOGY"),
                          (3, "VICTORY_CULTURE")])
        self.assertEqual(board[1][2], {})

    def test_a_win_the_host_table_does_not_carry_is_still_on_the_board(self):
        """The win is the evidence; the table is only the gloss."""
        board = civ6_ladder.victory_board(self._state(
            self._win(6, "2026-08-20T00:00:00Z"),
            victory_types=[{"index": 0, "type": "VICTORY_SCORE"}]))
        self.assertEqual([index for index, _, _ in board], [0, 6])

    def test_nothing_won_and_no_host_table_renders_no_board_at_all(self):
        self.assertEqual(civ6_ladder.victory_board(self._state()), [])

    def test_the_host_table_is_kept_from_the_first_run_that_exports_it(self):
        state = {"attempts": [], "wins": {}}
        table = [{"index": 0, "type": "VICTORY_SCORE"}]
        civ6_ladder.apply(state, summary("first", seat={"victory_types": table}))
        self.assertEqual(state["victory_types"], table)
        # It is a property of the game, not the run: a later run does not
        # overwrite it, so one bad export cannot rewrite the board's rows.
        civ6_ladder.apply(state, summary(
            "second", seat={"victory_types": [{"index": 9, "type": "VICTORY_ODD"}]}))
        self.assertEqual(state["victory_types"], table)


class EveryRowSaysWhichLaneItPlayed(LedgerCase):
    """★★★★★ 307 rows and no column for the objective.

    The summary recorded every setting of the GAME — difficulty, size, speed,
    modes, turn cap — and not the one setting that says what the AGENT was
    trying to do. That was survivable while the launchers offered one workable
    lane. #1871 made all six of `VictoryTarget`'s variants selectable, so rows
    from here on can differ in objective, and a ledger that cannot separate them
    cannot answer the only question anyone asks of it.
    """

    def test_the_asked_for_lane_reaches_the_ledger_row(self):
        entry = civ6_ladder.entry_from(summary("aimed", victory_target="religious"))
        self.assertEqual(entry["victory_target"], "religious")

    def test_a_row_from_before_the_column_existed_reads_absent_not_wrong(self):
        self.assertIsNone(civ6_ladder.entry_from(summary("older"))["victory_target"])

    def test_what_was_asked_for_and_what_was_won_are_different_columns(self):
        """`civvis` is the absence of a pinned target, not a seventh victory
        condition, so it must never be confused with the outcome."""
        entry = civ6_ladder.entry_from(summary(
            "chose", won=True, victory_target="civvis",
            outcome={"kind": "victory", "won": True, "victory": 4},
            seat={"victory_types": [{"index": 4, "type": "VICTORY_RELIGIOUS"}]}))
        self.assertEqual(entry["victory_target"], "civvis")
        self.assertEqual(entry["victory"], 4)
        self.assertEqual(entry["victory_type"], "VICTORY_RELIGIOUS")

    def test_the_attempt_table_shows_the_lane(self):
        civ6_ladder.record_summary(write_run(
            self.runs, summary("shown", victory_target="diplomatic")))
        table = civ6_ladder.markdown_for(self.state())
        self.assertIn("| playing for |", table)
        row = next(line for line in table.splitlines() if "`shown`" in line)
        self.assertIn("| diplomatic |", row)

    def test_a_row_without_a_lane_renders_a_dash_not_none(self):
        civ6_ladder.record_summary(write_run(self.runs, summary("older")))
        self.assertNotIn("None", civ6_ladder.markdown_for(self.state()))


class ARowSaysWhichGameItWasPlayedIn(LedgerCase):
    """★★★★★ A ladder is a comparison, and a comparison needs the rows to be
    the same game.

    `civ6_play.py` checks the ruleset and the optional modes from inside the
    running game and refuses a run that does not match. None of it reached the
    record: 307 rows carried neither field, so "these are all Settler games" was
    an assertion with nothing under it. A mode PERSISTS — GAMEMODE_HEROES ran on
    a live game with twelve hero units while every log said plain Gathering
    Storm — and the ruleset is the same axis one level up.
    """

    def test_the_row_records_what_the_game_reported(self):
        entry = civ6_ladder.entry_from(summary(
            "checked", ruleset="RULESET_EXPANSION_2", modes=[]))
        self.assertEqual(entry["ruleset"], "RULESET_EXPANSION_2")
        self.assertEqual(entry["modes"], [])

    def test_a_row_from_before_the_readback_is_absent_not_agreed(self):
        entry = civ6_ladder.entry_from(summary("older"))
        self.assertIsNone(entry["ruleset"])
        self.assertIsNone(entry["modes"])

    def test_the_note_counts_unverified_rows_separately(self):
        note = " ".join(civ6_ladder.same_game_note([
            {"ruleset": "RULESET_EXPANSION_2"}, {"ruleset": None}, {"ruleset": None},
        ]))
        self.assertIn("RULESET_EXPANSION_2", note)
        self.assertIn("2 row(s) carry no ruleset readback", note)

    def test_an_unreadable_readback_is_unverified_not_a_ruleset_recorded(self):
        """★★★★★ `?` IS THE MOD'S ANSWER FOR A LOOKUP THAT FAILED, and because
        it is a truthy string it was printed as a ruleset this record had
        observed. The published prose read "Rulesets recorded: ?,
        RULESET_EXPANSION_2" over three rows whose ruleset nobody knows —
        `civvis-20260818T032030Z`, `040903Z` and `045332Z`, all three played to
        223/250/250 turns.
        """
        note = " ".join(civ6_ladder.same_game_note([
            {"ruleset": "RULESET_EXPANSION_2"},
            {"ruleset": civ6_ladder.UNREADABLE},
            {"ruleset": None},
        ]))
        self.assertIn("Rulesets recorded: RULESET_EXPANSION_2.", note)
        self.assertNotIn("?,", note)
        self.assertIn("2 row(s) carry no ruleset readback", note)

    def test_the_note_says_unverified_is_not_a_mismatch(self):
        """The prose is the only thing a reader has. It must not let an
        unverified row be read as a game played under the wrong rules."""
        note = " ".join(civ6_ladder.same_game_note([{"ruleset": None}]))
        self.assertIn("Unverified is not a mismatch", note)

    def test_rows_misfiled_before_the_distinction_existed_are_owned(self):
        """⚠ ROWS ARE NEVER REWRITTEN, so the three games filed as
        `wrong_ruleset` on an unreadable readback still read that way in the
        table. Without this line the note above contradicts them."""
        note = " ".join(civ6_ladder.same_game_note([
            {"ruleset": civ6_ladder.UNREADABLE, "reason": "wrong_ruleset"},
            {"ruleset": civ6_ladder.UNREADABLE, "reason": "wrong_ruleset"},
            {"ruleset": "RULESET_EXPANSION_2", "reason": "stopped"},
        ]))
        self.assertIn("2 of those row(s) were nevertheless recorded as", note)

    def test_a_genuinely_wrong_ruleset_row_is_not_called_misfiled(self):
        """The game reported a ruleset and it differed. That refusal was
        correct and must not be excused by this line."""
        note = " ".join(civ6_ladder.same_game_note([
            {"ruleset": "RULESET_STANDARD", "reason": "wrong_ruleset"},
        ]))
        self.assertNotIn("nevertheless recorded", note)

    def test_a_mode_that_was_on_is_called_out(self):
        note = " ".join(civ6_ladder.same_game_note([
            {"ruleset": "RULESET_EXPANSION_2", "modes": ["GAMEMODE_HEROES"]},
        ]))
        self.assertIn("GAMEMODE_HEROES", note)
        self.assertIn("not measuring the same game", note)

    def test_a_clean_record_says_so_without_a_warning(self):
        note = " ".join(civ6_ladder.same_game_note([
            {"ruleset": "RULESET_EXPANSION_2", "modes": []},
        ]))
        self.assertNotIn("⚠", note)
        self.assertNotIn("no ruleset readback", note)


class ADefeatIsNotAStall(LedgerCase):
    """★★★★★ OUR OWN DEFEAT WAS INDISTINGUISHABLE FROM THE HARNESS HANGING.

    `civ6_play.py` keeps the terminal event for a defeat exactly as it does for
    a victory, and the mod synthesises one when no cities remain. `entry_from`
    projected only `won`, `victory` and `victory_type`, and a defeat carries
    none of them, so an elimination landed as `{"reason": "stopped",
    "victory": null, "won": false}` — byte-identical to a run that wedged.
    Measured on `civvis-20260815T160346Z` (t233), `195951Z` (t102) and
    `210845Z` (t226), each carrying `{"kind": "defeat", "ours": true}` in
    `events.jsonl` and each filed as `stopped`.
    """

    @staticmethod
    def _defeat(tag, *, ours=True, turn=233, **extra):
        return summary(tag, last_turn=turn, reason="stopped",
                       outcome={"kind": "defeat", "ours": ours, "player": 0,
                                "turn": turn},
                       **extra)

    def test_our_elimination_reaches_the_row(self):
        entry = civ6_ladder.entry_from(self._defeat("civvis-20260815T160346Z"))
        self.assertTrue(entry["defeat"])
        self.assertFalse(entry["won"], "losing is not winning")

    def test_a_stall_and_a_defeat_are_not_the_same_row(self):
        """The whole defect: two different endings, one indistinguishable row."""
        stalled = civ6_ladder.entry_from(
            summary("hung", reason="stalled: no event for 240s"))
        lost = civ6_ladder.entry_from(self._defeat("lost"))
        self.assertNotEqual(
            (stalled["won"], stalled["defeat"]), (lost["won"], lost["defeat"]))
        self.assertFalse(stalled["defeat"])

    def test_a_rivals_elimination_is_not_ours(self):
        """⚠ Civilization VI emits a `defeat` when ANY player is eliminated,
        including a city-state; 39 of the 111 run event streams still on
        this machine carry at least one.
        `civ6_play.finished()` already refuses to stop on those — a run was cut
        sixteen turns short of a score victory because player 7 died — and the
        recording side needs the same distinction."""
        entry = civ6_ladder.entry_from(self._defeat("rival-died", ours=False))
        self.assertFalse(entry["defeat"])

    def test_a_victory_is_not_recorded_as_a_defeat(self):
        entry = civ6_ladder.entry_from(summary("won-it", won=True))
        self.assertTrue(entry["won"])
        self.assertFalse(entry["defeat"])

    def test_a_row_from_before_the_column_existed_reads_false_not_none(self):
        self.assertFalse(civ6_ladder.entry_from(summary("older"))["defeat"])

    def test_the_attempt_table_says_defeat_where_it_said_stopped(self):
        civ6_ladder.record_summary(write_run(
            self.runs, self._defeat("civvis-20260815T195951Z", turn=102)))
        row = next(line for line in civ6_ladder.markdown_for(
            self.state()).splitlines() if "civvis-20260815T195951Z" in line)
        self.assertIn("| defeat |", row)
        self.assertNotIn("| stopped |", row)

    def test_a_stalled_row_still_reports_how_it_stalled(self):
        civ6_ladder.record_summary(write_run(self.runs, summary(
            "wedged", reason="stalled: no event for 240s")))
        row = next(line for line in civ6_ladder.markdown_for(
            self.state()).splitlines() if "`wedged`" in line)
        self.assertIn("stalled: no event for 240s", row)
        self.assertNotIn("defeat", row)

    def test_the_ending_census_counts_eliminations_apart_from_the_stalls(self):
        for tag in ("civvis-a", "civvis-b"):
            civ6_ladder.record_summary(write_run(self.runs, self._defeat(tag)))
        civ6_ladder.record_summary(write_run(self.runs, summary("won-it", won=True)))
        table = civ6_ladder.markdown_for(self.state())
        self.assertIn("2 more ended in our own elimination", table)

    def test_an_empty_record_renders_nothing(self):
        self.assertEqual(civ6_ladder.same_game_note([]), [])


class TheCensusSaysWhichLanesComplete(unittest.TestCase):
    """Which victory conditions have ended a game here — the record's only
    empirical evidence about lane reachability inside the turn budget."""

    @staticmethod
    def _attempts(*pairs):
        return [{"victory": index, "victory_type": name}
                for index, name in pairs]

    def test_it_counts_every_terminal_event_ours_and_theirs(self):
        census = civ6_ladder.victory_census(self._attempts(
            (0, None), (0, None), (6, None), (0, None)))
        self.assertEqual(census, [(0, None, 3), (6, None, 1)])

    def test_runs_that_never_ended_are_not_counted_as_a_lane(self):
        census = civ6_ladder.victory_census(
            [{"victory": None}, {"reason": "stopped"}, {"victory": 3}])
        self.assertEqual(census, [(3, None, 1)])

    def test_a_name_seen_once_labels_every_row_with_that_index(self):
        """One run with the export names the index for the whole history."""
        census = civ6_ladder.victory_census(self._attempts(
            (3, None), (3, "VICTORY_CULTURE"), (3, None)))
        self.assertEqual(census, [(3, "VICTORY_CULTURE", 3)])

    def test_ties_break_on_the_index_so_the_table_is_deterministic(self):
        census = civ6_ladder.victory_census(self._attempts((6, None), (2, None)))
        self.assertEqual([row[0] for row in census], [2, 6])

    def test_score_victory_is_index_zero_and_survives_the_falsy_trap(self):
        census = civ6_ladder.victory_census(self._attempts((0, "VICTORY_SCORE")))
        self.assertEqual(census, [(0, "VICTORY_SCORE", 1)])


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


class TheRecordBelongsToTheFleetNotToOneMachine(LedgerCase):
    """★★★★★ 76 GAMES WERE PLAYED, RECORDED, AND COULD NEVER REACH THE RECORD.

    `load` seeds a machine with no live ledger from the committed snapshot and
    then never consults it again, and `publish` wrote that machine's ledger
    over the shared document. Once a second Civilization VI seat existed, the
    two records forked from the 255 attempts of #1767 and every publish after
    that was one seat's copy. On 2026-08-23 the snapshot held 349 attempts and
    `mbp-m5-max-128` held 331: 255 shared, 94 only published, **76 only on
    this machine — including nine Settler victories** — and `check` reported
    "snapshot in step", because it compared the two lengths and 331 is not
    more than 349.
    """

    def _rows(self, tags):
        return [{"tag": tag, "utc": f"2026-08-1{i}T00:00:00Z",
                 "difficulty": "DIFFICULTY_SETTLER", "configured": True,
                 "won": False, "turns": 250, "score": 100}
                for i, tag in enumerate(tags)]

    def _snapshot(self, tags):
        self.snapshot.write_text(
            json.dumps({"attempts": self._rows(tags), "wins": {}}))

    def _live(self, tags):
        """Write the live ledger directly: a seat that forked from the record
        has rows the snapshot never had, and lacks rows the snapshot gained."""
        self.ledger.write_text(
            json.dumps({"attempts": self._rows(tags), "wins": {}}))

    def check(self, **kwargs) -> int:
        with redirect_stdout(io.StringIO()) as out:
            code = civ6_ladder.check(self.runs, self.ledger, None,
                                     self.snapshot, **kwargs)
        self.last_report = out.getvalue()
        return code

    def test_publish_keeps_every_row_another_seat_landed(self):
        self._snapshot(["remote-1", "remote-2"])
        self._live(["local-1"])
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        self.assertEqual(
            [a["tag"] for a in json.loads(self.snapshot.read_text())["attempts"]],
            ["remote-1", "remote-2", "local-1"])

    def test_publishing_twice_lands_nothing_the_second_time(self):
        self._snapshot(["remote-1"])
        self._live(["local-1"])
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        self.assertEqual(
            [a["tag"] for a in json.loads(self.snapshot.read_text())["attempts"]],
            ["remote-1", "local-1"])

    def test_a_longer_snapshot_is_not_evidence_the_ledger_was_published(self):
        """The 2026-08-23 shape exactly: more rows published than recorded
        here, and two of the recorded ones in no published record at all."""
        self._snapshot(["shared", "remote-1", "remote-2", "remote-3"])
        self._live(["shared", "local-1", "local-2"])
        state = json.loads(self.ledger.read_text())
        published = json.loads(self.snapshot.read_text())
        self.assertLess(len(state["attempts"]), len(published["attempts"]),
                        "the fixture must reproduce the count that hid it")
        self.assertEqual(self.check(), 1)
        self.assertIn("2 attempt(s) recorded here are in no published "
                      "snapshot", self.last_report)
        self.assertIn("publish", self.last_report)

    def test_the_other_seats_rows_are_a_note_and_not_a_failure(self):
        self._snapshot(["remote-1"])
        self._live(["local-1"])
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        self.assertEqual(self.check(), 0)
        self.assertIn("recorded by another seat", self.last_report)
        self.assertNotIn("LADDER:", self.last_report)

    def test_a_win_that_arrives_only_in_the_merge_claims_its_rung(self):
        base = {"attempts": [], "wins": {}}
        incoming = {"attempts": [{
            "tag": "other-seat-win", "utc": "2026-08-18T18:46:46Z",
            "difficulty": "DIFFICULTY_CHIEFTAIN", "configured": True,
            "won": True}], "wins": {}}
        merged, added = civ6_ladder.merge_state(base, incoming)
        self.assertEqual([a["tag"] for a in added], ["other-seat-win"])
        self.assertEqual(merged["wins"]["DIFFICULTY_CHIEFTAIN"]["tag"],
                         "other-seat-win")

    def test_the_earliest_win_still_stands_across_a_merge(self):
        """The milestone is when the rung was FIRST climbed, and a merge is
        exactly the late arrival that rule exists for."""
        late = {"tag": "late", "utc": "2026-08-18T00:00:00Z",
                "difficulty": "DIFFICULTY_SETTLER", "configured": True,
                "won": True}
        early = {"tag": "early", "utc": "2026-08-16T00:00:00Z",
                 "difficulty": "DIFFICULTY_SETTLER", "configured": True,
                 "won": True}
        merged, _ = civ6_ladder.merge_state(
            {"attempts": [late], "wins": {"DIFFICULTY_SETTLER": late}},
            {"attempts": [early], "wins": {}})
        self.assertEqual(merged["wins"]["DIFFICULTY_SETTLER"]["tag"], "early")
        back, _ = civ6_ladder.merge_state(
            {"attempts": [early], "wins": {"DIFFICULTY_SETTLER": early}},
            {"attempts": [late], "wins": {}})
        self.assertEqual(back["wins"]["DIFFICULTY_SETTLER"]["tag"], "early")

    def test_an_unconfigured_win_claims_nothing_through_a_merge_either(self):
        merged, _ = civ6_ladder.merge_state({"attempts": [], "wins": {}}, {
            "attempts": [{"tag": "menu-defaults", "utc": "2026-08-18T00:00:00Z",
                          "difficulty": "DIFFICULTY_SETTLER",
                          "configured": False, "won": True}], "wins": {}})
        self.assertEqual(merged["wins"], {})

    def _sync(self, quiet=True) -> str:
        with redirect_stdout(io.StringIO()) as out:
            civ6_ladder.sync(self.runs, self.ledger, quiet=quiet)
        return out.getvalue()

    def test_the_backfill_names_a_backlog_no_snapshot_has(self):
        """The one hook guaranteed to run between games says it out loud.

        `heal_the_ladder` calls `sync` before every attempt. Nothing calls
        `check`, so until this line existed an unpublished backlog was visible
        only to somebody who typed a command — and for 76 attempts nobody did.
        """
        self._snapshot(["shared"])
        self._live(["shared"])
        write_run(self.runs, summary("brand-new"))
        report = self._sync()
        self.assertIn("LADDER: 1 recorded attempt(s) are in no published "
                      "snapshot", report)
        self.assertIn("brand-new", report)

    def test_it_speaks_even_when_the_backfill_recorded_nothing(self):
        """The backlog is not created by this run; being quiet about it while
        nothing new arrives is exactly how it grew to 76."""
        self._snapshot(["shared"])
        self._live(["shared", "local-1"])
        report = self._sync()
        self.assertIn("local-1", report)

    def test_a_published_ledger_says_nothing(self):
        self._snapshot(["shared", "local-1"])
        self._live(["shared", "local-1"])
        self.assertNotIn("LADDER:", self._sync())

    def test_a_clone_with_no_snapshot_at_all_claims_nothing(self):
        """Absence of a committed record is not evidence of a backlog."""
        self._live(["local-1"])
        self.assertFalse(self.snapshot.exists())
        self.assertNotIn("LADDER:", self._sync())
        self.assertIsNone(
            civ6_ladder.unpublished_tags({"attempts": [{"tag": "x"}]}))

    def test_the_attempt_table_is_the_newest_forty_by_the_clock(self):
        """Two seats interleave, and `sync` appends old games at the end, so
        the last forty rows of the file are not the last forty games."""
        rows = [{"tag": f"remote-{i}", "utc": f"2026-08-2{i}T00:00:00Z",
                 "difficulty": "DIFFICULTY_SETTLER", "configured": True,
                 "won": False, "turns": 250, "score": 1}
                for i in range(3)]
        backfilled = [{"tag": "old-backfill", "utc": "2026-07-01T00:00:00Z",
                       "difficulty": "DIFFICULTY_SETTLER", "configured": True,
                       "won": False, "turns": 250, "score": 1}]
        table = civ6_ladder.markdown_for(
            {"attempts": rows + backfilled, "wins": {}})
        body = table.split("| run | difficulty |", 1)[1]
        self.assertLess(body.index("`old-backfill`"), body.index("`remote-0`"),
                        "a row recorded late must not sort as the newest game")


class TheCommittedRecordIsInternallyConsistent(unittest.TestCase):
    """A guard on the real `docs/civ6_ladder.json`, run by CI on every PR.

    The two ways this record can be corrupted are a hand edit and a bad merge,
    and both show up here: a tag recorded twice (a merge that appended instead
    of unioning) or a `wins` slot that is not the earliest configured win in
    `attempts` (a merge that kept one seat's milestone over an earlier one, or
    a row rewritten by hand). `ThePublishedPairAgrees` proves the markdown is
    derived from this file; this proves the file itself hangs together.
    """

    def setUp(self):
        if not civ6_ladder.DATA.is_file():
            self.skipTest("no published snapshot yet")
        self.state = json.loads(civ6_ladder.DATA.read_text())

    def test_no_attempt_is_recorded_twice(self):
        tags = [a.get("tag") for a in self.state["attempts"] if a.get("tag")]
        duplicated = sorted({t for t in tags if tags.count(t) > 1})
        self.assertEqual(duplicated, [],
                         "the published record holds the same run twice")

    def test_every_rung_names_the_earliest_configured_win_it_holds(self):
        earliest: dict = {}
        for attempt in self.state["attempts"]:
            difficulty = attempt.get("difficulty")
            if not (attempt.get("won") and attempt.get("configured")
                    and difficulty in civ6_ladder.NAMES):
                continue
            held = earliest.get(difficulty)
            if held is None or (attempt.get("utc") or "\uffff") < (
                    held.get("utc") or "\uffff"):
                earliest[difficulty] = attempt
        self.assertEqual(
            {k: v.get("tag") for k, v in self.state["wins"].items()},
            {k: v.get("tag") for k, v in earliest.items()},
            "a rung in `wins` is not the earliest configured win the record "
            "holds at that difficulty; run `civ6_ladder.py publish`")


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

    def test_decider_genome_reads_the_deciders_own_first_record(self):
        why = self.runs / "why.log"
        why.write_text(
            "[genome] no strategy 'WildCard9' in /x/data/league\n"
            '{"civ":null,"kind":"genome","lane":null,"parallel_settlers":true,'
            '"per_civ":null,"revision":null,"source":"AdvancedAi::new",'
            '"strategy":"stock","strength_bound":null,"treatments":["a","b"]}\n'
            "[why] t1 Strategy/Strategy Grand strategy: expansion | ...\n")
        self.assertEqual(
            civ6_ladder.decider_genome(why),
            {"strategy": "stock", "source": "AdvancedAi::new", "lane": None,
             "civ": None, "strength_bound": None})
        resolved = self.runs / "why2.log"
        resolved.write_text(
            '{"civ":"Rome","kind":"genome","lane":null,"source":"data/league",'
            '"strategy":"g56-48","strength_bound":0.51}\n')
        self.assertEqual(civ6_ladder.decider_genome(resolved)["strategy"], "g56-48")
        self.assertIsNone(civ6_ladder.decider_genome(self.runs / "absent.log"))
        # A why.log without the record (an older decider) is not "stock".
        (self.runs / "why3.log").write_text("[why] t1 nothing\n")
        self.assertIsNone(civ6_ladder.decider_genome(self.runs / "why3.log"))

    def test_the_genome_played_and_the_name_asked_for_are_recorded_on_the_attempt(self):
        civ6_ladder.record_summary(write_run(
            self.runs, summary("genomed",
                               genome={"strategy": "stock", "source": "AdvancedAi::new"},
                               strategy_requested="WildCard9")))
        row = self.state()["attempts"][0]
        self.assertEqual(row["genome"]["strategy"], "stock")
        self.assertEqual(row["strategy_requested"], "WildCard9")

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


class TheHealthFloorStaysFalsifiable(LedgerCase):
    """A gap in measurement is not a gap in attempts.

    `--min-applied` reads only attempts that carry a rate, so an instrument
    that goes dark makes it silently unfalsifiable — while `--stale-hours`
    stays quiet, because attempts are still arriving. Measured on the live
    ledger 2026-08-18: attempts 266 to 306 — forty-one consecutive games, all
    played to the 250-turn clock, **including both Settler wins** — recorded no
    rate at all, and `check` was green the whole way. The bridge health of the
    project's only two external results is unknown as a result.
    """

    def _attempts(self, rates: list) -> None:
        for index, rate in enumerate(rates):
            kwargs = {}
            if rate is not None:
                kwargs = {"orders_seen": 1000,
                          "orders_applied": int(round(rate * 10))}
            civ6_ladder.record_summary(write_run(
                self.runs, summary(f"run-{index:03d}", **kwargs)))

    def test_a_single_unmeasured_run_is_still_not_a_problem(self):
        """The original reasoning holds for one: a run that died before its
        first turn has no rate and is the staleness check's business."""
        self._attempts([97.0, None])
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        self.assertEqual(
            civ6_ladder.check(self.runs, self.ledger, None, self.snapshot,
                              min_applied=95.0), 0)

    def test_a_run_of_unmeasured_attempts_is_reported(self):
        self._attempts([97.0] + [None] * 6)
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        out = io.StringIO()
        with redirect_stdout(out):
            code = civ6_ladder.check(self.runs, self.ledger, None, self.snapshot,
                                     min_applied=95.0)
        self.assertEqual(code, 1)
        self.assertIn("unmeasured on the last 6 attempt(s)", out.getvalue())

    def test_the_limit_is_configurable(self):
        self._attempts([97.0] + [None] * 6)
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        with redirect_stdout(io.StringIO()):
            self.assertEqual(
                civ6_ladder.check(self.runs, self.ledger, None, self.snapshot,
                                  min_applied=95.0, unmeasured_limit=99), 0)

    def test_the_instrument_coming_back_clears_it(self):
        self._attempts([97.0] + [None] * 6 + [96.0])
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        with redirect_stdout(io.StringIO()):
            self.assertEqual(
                civ6_ladder.check(self.runs, self.ledger, None, self.snapshot,
                                  min_applied=95.0), 0)

    def test_it_is_silent_without_the_floor_it_protects(self):
        """This exists to keep `--min-applied` honest. Asking for no floor is
        asking for no bridge-health opinion at all."""
        self._attempts([None] * 20)
        with redirect_stdout(io.StringIO()):
            civ6_ladder.publish(self.ledger, self.snapshot, self.markdown)
        self.assertEqual(
            civ6_ladder.check(self.runs, self.ledger, None, self.snapshot), 0)

    def test_old_rows_from_before_the_rate_existed_are_not_counted_forever(self):
        """Counted from the END, not totalled: the ledger legitimately holds
        hundreds of rows from before the rate was recorded at all."""
        self.assertEqual(civ6_ladder.trailing_unmeasured(
            [{"applied_pct": None}] * 300 + [{"applied_pct": 97.0}]), 0)
        self.assertEqual(civ6_ladder.trailing_unmeasured(
            [{"applied_pct": 97.0}] + [{"applied_pct": None}] * 3), 3)


class TheBackfillRecoversBridgeHealth(LedgerCase):
    """The self-healing path could not heal the one number it was built for.

    `civ6_play.py` sums `events.jsonl` into the summary when a run ends, and
    `sync` exists precisely because that write is best-effort and may not
    happen. But `sync` recorded whatever the summary contained, so a run whose
    bookkeeping failed lost its bridge health permanently — while its own
    events file sat beside it on disk. Rows 266 to 306 of the live ledger, the
    same 41 summaries `heal_the_ladder` was written to rescue, carry no rate at
    all, and both Settler wins are among them.
    """

    def _run_missing_totals(self, tag: str, seen: int, applied: int):
        """A summary written without totals, beside events that hold them."""
        path = write_run(self.runs, summary(tag))
        (path.parent / "events.jsonl").write_text("\n".join(
            json.dumps({"kind": "turn", "ctx": "agent", "turn": turn,
                        "orders_seen": seen, "orders_applied": applied})
            for turn in (1,)))
        return path

    def test_sync_reads_the_events_the_summary_forgot(self):
        self._run_missing_totals("backfilled", 200, 194)
        with redirect_stdout(io.StringIO()):
            civ6_ladder.sync(self.runs, self.ledger)
        self.assertEqual(self.state()["attempts"][0]["applied_pct"], 97.0)

    def test_the_automatic_path_recovers_it_too(self):
        path = self._run_missing_totals("live-but-unwritten", 100, 88)
        civ6_ladder.record_summary(path)
        self.assertEqual(self.state()["attempts"][0]["applied_pct"], 88.0)

    def test_a_summary_that_already_measured_itself_is_not_second_guessed(self):
        """The summary is the run's own reading; this only fills an absence."""
        path = write_run(self.runs, summary(
            "measured", orders_seen=200, orders_applied=194))
        (path.parent / "events.jsonl").write_text(json.dumps(
            {"kind": "turn", "ctx": "agent", "turn": 1,
             "orders_seen": 10, "orders_applied": 1}))
        civ6_ladder.record_summary(path)
        self.assertEqual(self.state()["attempts"][0]["applied_pct"], 97.0)

    def test_no_events_file_still_records_the_attempt(self):
        """Recording is best-effort by design: a finished game must never be
        lost to a bookkeeping error, and that outranks the number."""
        civ6_ladder.record_summary(write_run(self.runs, summary("no-events")))
        self.assertIsNone(self.state()["attempts"][0]["applied_pct"])
        self.assertEqual(len(self.state()["attempts"]), 1)

    def test_an_unreadable_events_file_is_not_fatal(self):
        path = write_run(self.runs, summary("torn"))
        (path.parent / "events.jsonl").write_text('{"kind": "turn", "ctx": "ag')
        civ6_ladder.record_summary(path)
        self.assertIsNone(self.state()["attempts"][0]["applied_pct"])
        self.assertEqual(len(self.state()["attempts"]), 1)


# ---------------------------------------------------------------------------
# The ledger branch: append-only, idempotent, and never touching the
# management worktree's index or working tree.

import subprocess  # noqa: E402

#: A CI runner has no git identity; supplied through the environment so the
#: suite never writes to any repository's config.
PROBE = {
    "GIT_AUTHOR_NAME": "ledger probe",
    "GIT_AUTHOR_EMAIL": "probe@civvis.invalid",
    "GIT_COMMITTER_NAME": "ledger probe",
    "GIT_COMMITTER_EMAIL": "probe@civvis.invalid",
}


def _git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True,
                          text=True, check=True,
                          env={**os.environ, **PROBE}).stdout.strip()


class PublishRunTests(unittest.TestCase):
    def setUp(self):
        self.tmp = TemporaryDirectory()
        root = Path(self.tmp.name)
        self.origin = root / "origin.git"
        subprocess.run(["git", "init", "-q", "--bare", str(self.origin)], check=True)
        self.work = root / "work"
        subprocess.run(["git", "init", "-q", str(self.work)], check=True)
        _git(self.work, "remote", "add", "origin", str(self.origin))
        # A management worktree has a checked-out tree and a clean index; both
        # must be exactly as they were after a publish.
        (self.work / "tracked.txt").write_text("tracked\n")
        _git(self.work, "add", "--", "tracked.txt")
        _git(self.work, "commit", "-q", "-m", "seed")
        self.runs = root / "runs"
        for tag, finished in (("civvis-1", "2026-08-20T10:00:00Z"),
                              ("civvis-2", "2026-08-21T10:00:00Z")):
            run = self.runs / tag
            run.mkdir(parents=True)
            (run / "summary.json").write_text(json.dumps(
                summary(tag, finished=finished)))
            (run / "events.jsonl").write_text(
                json.dumps({"kind": "turn", "turn": 1}) + "\n")

    def tearDown(self):
        self.tmp.cleanup()

    def publish(self, tag: str) -> str:
        return civ6_ladder.publish_run(tag, self.runs, repo=self.work, env=PROBE)

    def ledger_files(self) -> list[str]:
        return _git(self.origin, "ls-tree", "-r", "--name-only",
                    "refs/heads/ledger").splitlines()

    def test_append_only_and_idempotent(self):
        self.assertEqual(self.publish("civvis-1"), "published")
        first = _git(self.origin, "rev-parse", "refs/heads/ledger")
        self.assertEqual(self.ledger_files(),
                         ["runs/civvis-1/events.jsonl.gz", "runs/civvis-1/summary.json"])
        # Orphan: the ledger's root shares nothing with the code history.
        self.assertEqual(_git(self.origin, "rev-list", "--count", first), "1")

        self.assertEqual(self.publish("civvis-1"), "already")
        self.assertEqual(_git(self.origin, "rev-parse", "refs/heads/ledger"), first)

        self.assertEqual(self.publish("civvis-2"), "published")
        second = _git(self.origin, "rev-parse", "refs/heads/ledger")
        self.assertEqual(_git(self.origin, "rev-parse", f"{second}^"), first)
        self.assertEqual(_git(self.origin, "rev-list", "--count", second), "2")
        self.assertEqual(len(self.ledger_files()), 4)
        # The payload is what the run wrote, gzipped.
        import gzip
        raw = subprocess.run(
            ["git", "-C", str(self.origin), "show",
             f"{second}:runs/civvis-2/events.jsonl.gz"],
            capture_output=True, check=True).stdout
        self.assertEqual(json.loads(gzip.decompress(raw))["turn"], 1)

    def test_management_worktree_is_untouched(self):
        before = _git(self.work, "status", "--porcelain")
        head = _git(self.work, "rev-parse", "HEAD")
        self.publish("civvis-1")
        self.assertEqual(_git(self.work, "status", "--porcelain"), before)
        self.assertEqual(_git(self.work, "rev-parse", "HEAD"), head)
        self.assertFalse((self.work / "runs").exists())
        self.assertEqual(_git(self.work, "ls-files"), "tracked.txt")
        self.assertFalse(list(self.runs.glob("*/.ledger-index-*")))

    def test_a_tip_that_moved_is_appended_to_not_forced(self):
        # Another seat publishes between this seat's fetch and push; the push
        # is refused as a non-fast-forward and the commit is rebuilt on the
        # new tip, so nothing that was on the branch is ever lost.
        self.publish("civvis-1")
        other = Path(self.tmp.name) / "other"
        subprocess.run(["git", "init", "-q", str(other)], check=True)
        _git(other, "remote", "add", "origin", str(self.origin))
        stale_tip = civ6_ladder.ledger_tip(self.work, env=PROBE)
        self.assertEqual(civ6_ladder.publish_run(
            "civvis-2", self.runs, repo=other, env=PROBE), "published")
        # Pin the local tracking ref to the stale tip and publish a third run
        # through a `ledger_tip` that answers stale first, fresh second.
        calls = []
        real = civ6_ladder.ledger_tip

        def stale_once(repo, remote="origin", branch="ledger", env=None):
            calls.append(1)
            return stale_tip if len(calls) == 1 else real(repo, remote, branch, env=env)
        run = self.runs / "civvis-3"
        run.mkdir()
        (run / "summary.json").write_text(json.dumps(summary("civvis-3")))
        (run / "events.jsonl").write_text(json.dumps({"kind": "turn"}) + "\n")
        civ6_ladder.ledger_tip = stale_once
        try:
            self.assertEqual(self.publish("civvis-3"), "published")
        finally:
            civ6_ladder.ledger_tip = real
        self.assertEqual(len(calls), 2)
        self.assertEqual(_git(self.origin, "rev-list", "--count", "refs/heads/ledger"), "3")
        self.assertEqual(len(self.ledger_files()), 6)

    def test_a_run_without_events_publishes_its_summary(self):
        (self.runs / "civvis-1" / "events.jsonl").unlink()
        self.assertEqual(self.publish("civvis-1"), "published")
        self.assertEqual(self.ledger_files(), ["runs/civvis-1/summary.json"])

    def test_cli(self):
        real_repo, real_env = civ6_ladder.REPO, dict(os.environ)
        civ6_ladder.REPO = self.work
        os.environ.update(PROBE)
        buffer = io.StringIO()
        try:
            with redirect_stdout(buffer):
                code = civ6_ladder.main(["--runs", str(self.runs),
                                         "publish-run", "civvis-1"])
                code2 = civ6_ladder.main(["--runs", str(self.runs),
                                          "publish-run", "civvis-1"])
        finally:
            civ6_ladder.REPO = real_repo
            os.environ.clear()
            os.environ.update(real_env)
        self.assertEqual((code, code2), (0, 0))
        self.assertEqual(buffer.getvalue().split(), ["published", "already"])
