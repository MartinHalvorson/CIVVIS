#!/usr/bin/env python3
"""Focused checks for the direct-session Automation.log relay."""

from __future__ import annotations

import json
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import watch  # noqa: E402
from civ6_control.watch import EventLogBridge, PREFIX  # noqa: E402


def line(event: dict) -> str:
    return PREFIX + json.dumps(event) + "\n"


class EventLogBridgeTest(unittest.TestCase):
    def test_backfills_only_the_requested_run_and_preserves_duplicates(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log = root / "Automation.log"
            run = root / "run"
            repeated = {"kind": "await", "run": "live", "polls": 1}
            existing = {"kind": "seat", "run": "live", "leader": "LEADER_TRAJAN"}
            log.write_text(
                line({"kind": "seat", "run": "other"})
                + line(existing)
                + line(repeated)
                + line(repeated)
            )
            run.mkdir()
            (run / "events.jsonl").write_text(json.dumps(existing) + "\n")

            bridge = EventLogBridge(run, tag="live", log_path=log)
            self.assertEqual(bridge.pump(), 2)
            self.assertEqual(bridge.pump(), 0)

            events = [json.loads(row) for row in
                      (run / "events.jsonl").read_text().splitlines()]
            self.assertEqual(events, [existing, repeated, repeated])

    def test_keeps_a_partial_log_line_until_it_is_complete(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log = root / "Automation.log"
            event = {"kind": "state", "run": "live", "turn": 1}
            payload = PREFIX + json.dumps(event)
            log.write_text(payload[:20])

            bridge = EventLogBridge(root / "run", tag="live", log_path=log)
            self.assertEqual(bridge.pump(), 0)
            with log.open("a") as output:
                output.write(payload[20:] + "\n")
            self.assertEqual(bridge.pump(), 1)
            self.assertEqual(
                [json.loads(row) for row in
                 (root / "run" / "events.jsonl").read_text().splitlines()],
                [event],
            )


class HeldLastRecordTest(unittest.TestCase):
    """`Automation.Log` writes a record's newline only when the next record comes.

    The last record logged -- a replan frame's `state` -- used to wait as an
    unterminated tail until something else was logged (~0.2 s). A prefixed tail
    that parses whole is the complete record: relay it now, once.
    """

    def _tail(self, root: Path) -> "tuple[watch.LogTail, Path]":
        log = root / "Automation.log"
        log.write_text("")
        return watch.LogTail(log), log

    def _append(self, log: Path, text: str) -> None:
        with log.open("a") as output:
            output.write(text)

    def test_a_complete_record_without_its_newline_is_relayed_at_once(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tail, log = self._tail(Path(temporary))
            state = {"kind": "state", "turn": 7, "frame": 1}
            self._append(log, line({"kind": "replan_frame", "turn": 7}) + PREFIX + json.dumps(state))
            self.assertEqual([e["kind"] for e in tail.poll()], ["replan_frame", "state"])
            self.assertEqual(tail.poll(), [], "nothing new, nothing relayed")
            self._append(log, "\n")
            self.assertEqual(tail.poll(), [], "its newline does not relay it twice")
            self._append(log, line({"kind": "await", "polls": 1}))
            self.assertEqual([e["kind"] for e in tail.poll()], ["await"])

    def test_the_newline_and_the_next_record_together_relay_only_the_next(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tail, log = self._tail(Path(temporary))
            self._append(log, PREFIX + json.dumps({"kind": "state", "turn": 1}))
            self.assertEqual(len(tail.poll()), 1)
            self._append(log, "\n" + PREFIX + json.dumps({"kind": "export_timing", "turn": 1}))
            self.assertEqual([e["kind"] for e in tail.poll()], ["export_timing"],
                             "the next held record is relayed whole too")

    def test_a_half_written_record_still_waits(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tail, log = self._tail(Path(temporary))
            payload = PREFIX + json.dumps({"kind": "state", "turn": 2, "units": [{"id": 1}]})
            self._append(log, payload[:payload.index("}") + 1])   # a nested object closed
            self.assertEqual(tail.poll(), [], "a nested brace is not the record's end")
            self._append(log, payload[payload.index("}") + 1:])
            self.assertEqual(len(tail.poll()), 1)
            self._append(log, "\n")
            self.assertEqual(tail.poll(), [])

    def test_a_tail_that_grows_after_relay_is_never_relayed_twice(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tail, log = self._tail(Path(temporary))
            self._append(log, PREFIX + json.dumps({"kind": "await", "polls": 1}))
            self.assertEqual(len(tail.poll()), 1)
            self._append(log, " trailing")
            self.assertEqual(tail.poll(), [])
            self._append(log, "\n")
            self.assertEqual(tail.poll(), [])

    def test_only_objects_are_relayed_from_the_tail(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tail, log = self._tail(Path(temporary))
            self._append(log, PREFIX + "12")
            self.assertEqual(tail.poll(), [])
            self._append(log, "3\n")
            self.assertEqual(tail.poll(), [], "a non-object record is not an event")

    def test_a_truncated_log_forgets_what_it_relayed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tail, log = self._tail(Path(temporary))
            record = PREFIX + json.dumps({"kind": "loaded", "version": 2})
            self._append(log, line({"kind": "seat"}) + record)
            self.assertEqual(len(tail.poll()), 2)
            log.write_text(record)   # a new game rewrote the log from the start
            self.assertEqual([e["kind"] for e in tail.poll()], ["loaded"])

    def test_the_bridge_relays_a_held_record_once(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log = root / "Automation.log"
            event = {"kind": "state", "run": "live", "turn": 3}
            log.write_text(PREFIX + json.dumps(event))
            bridge = EventLogBridge(root / "run", tag="live", log_path=log)
            self.assertEqual(bridge.pump(), 1)
            with log.open("a") as output:
                output.write("\n")
            self.assertEqual(bridge.pump(), 0)


class RestartInPlaceTailTest(unittest.TestCase):
    """A game restarted inside Civ VI keeps writing the same Automation.log."""

    def test_the_next_run_starts_after_the_parked_runs_records(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "Automation.log"
            log.write_text(line({"kind": "turn", "run": "civvis-old", "turn": 40})
                           + PREFIX + json.dumps({"kind": "restart_parked",
                                                  "run": "civvis-old"}))
            tail = watch.LogTail(log)
            tail.skip_to_end()
            self.assertEqual(tail.poll(), [])
            with log.open("a") as output:
                output.write("\n" + line({"kind": "loaded", "run": "civvis-new"}))
            self.assertEqual([e["run"] for e in tail.poll()], ["civvis-new"])

    def test_a_tail_bound_to_a_run_drops_every_other_runs_records(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "Automation.log"
            log.write_text("")
            tail = watch.LogTail(log)
            tail.run = "civvis-new"
            with log.open("a") as output:
                output.write(line({"kind": "restart_called", "run": "civvis-old"})
                             + line({"kind": "autoclose_armed", "run": "civvis-new"})
                             + line({"kind": "untagged"})
                             + PREFIX + json.dumps({"kind": "loaded", "run": "civvis-old"}))
            self.assertEqual([e["kind"] for e in tail.poll()],
                             ["autoclose_armed", "untagged"])
            with log.open("a") as output:
                output.write("\n" + line({"kind": "seat", "run": "civvis-new"}))
            self.assertEqual([e["kind"] for e in tail.poll()], ["seat"],
                             "a dropped held record is not relayed when its newline comes")
            tail.run = None
            with log.open("a") as output:
                output.write(line({"kind": "loaded", "run": "civvis-old"}))
            self.assertEqual([e["run"] for e in tail.poll()], ["civvis-old"])


class GameLivenessTest(unittest.TestCase):
    """The relay asks `ps` only to (re)list; known pids are checked by syscall."""

    def _liveness(self, listings, running):
        clock = {"t": 0.0}
        calls = []

        def game_pids():
            calls.append(clock["t"])
            return listings.pop(0) if listings else []

        def kill(pid, signal):
            outcome = running.get(pid, ProcessLookupError)
            if outcome is not True:
                raise outcome()

        live = watch.GameLiveness(relist_s=10.0, clock=lambda: clock["t"])
        return live, clock, calls, game_pids, kill

    def test_a_known_running_pid_needs_no_ps(self) -> None:
        live, clock, calls, game_pids, kill = self._liveness([[4242], [4242]], {4242: True})
        with patch.object(watch.env, "game_pids", game_pids), \
             patch.object(watch.os, "kill", kill):
            for step in range(44):
                clock["t"] = step * 0.25
                self.assertTrue(live.alive())
        self.assertEqual(calls, [0.0, 0.0 + 10.0], "one listing, one 10 s refresh")

    def test_an_exit_is_confirmed_by_a_real_listing(self) -> None:
        running = {4242: True}
        live, clock, calls, game_pids, kill = self._liveness([[4242], []], running)
        with patch.object(watch.env, "game_pids", game_pids), \
             patch.object(watch.os, "kill", kill):
            self.assertTrue(live.alive())
            del running[4242]
            clock["t"] = 0.25
            self.assertFalse(live.alive(), "a gone pid is re-listed, and empty is an exit")
        self.assertEqual(len(calls), 2)

    def test_a_restarted_game_is_found_by_the_relisting(self) -> None:
        running = {4242: True}
        live, clock, calls, game_pids, kill = self._liveness([[4242], [5151]], running)
        with patch.object(watch.env, "game_pids", game_pids), \
             patch.object(watch.os, "kill", kill):
            self.assertTrue(live.alive())
            del running[4242]; running[5151] = True
            clock["t"] = 0.25
            self.assertTrue(live.alive())
            clock["t"] = 0.5
            self.assertTrue(live.alive())
        self.assertEqual(len(calls), 2, "the new pid is then checked by syscall")

    def test_a_pid_owned_by_another_user_is_running(self) -> None:
        live, clock, calls, game_pids, kill = self._liveness([[7]], {7: PermissionError})
        with patch.object(watch.env, "game_pids", game_pids), \
             patch.object(watch.os, "kill", kill):
            self.assertTrue(live.alive())
            clock["t"] = 1.0
            self.assertTrue(live.alive())
        self.assertEqual(len(calls), 1)

    def test_a_timed_out_listing_keeps_the_last_answer(self) -> None:
        # env.game_pids returns the last known pids when `ps` times out; the
        # liveness check passes that through rather than calling it an exit.
        live, clock, calls, game_pids, kill = self._liveness([[4242], [4242]], {})
        with patch.object(watch.env, "game_pids", game_pids), \
             patch.object(watch.os, "kill", kill):
            self.assertTrue(live.alive())
            clock["t"] = 0.25
            self.assertTrue(live.alive())


class FollowTest(unittest.TestCase):
    def test_locked_interval_does_not_consume_timeout(self) -> None:
        class Tail:
            def __init__(self) -> None:
                self.polls = 0

            def poll(self) -> list[dict]:
                self.polls += 1
                return [{"kind": "done"}] if self.polls == 3 else []

        seen: list[dict] = []
        with patch.object(watch.time, "monotonic",
                          side_effect=[0.0, 0.0, 2.0, 4.0, 4.0]), \
             patch.object(watch.time, "sleep") as sleep, \
             patch.object(watch.env, "game_pids", return_value=[123]):
            reason = watch.follow(
                Tail(), 1.0, seen.append, poll_s=0.25,
                stop_when=lambda event: event["kind"] == "done",
                pause_when=iter([True, True, False]).__next__,
            )

        self.assertEqual(reason, "stopped")
        self.assertEqual(seen, [{"kind": "done"}])
        self.assertEqual(sleep.call_count, 2)
class RelayCadenceTest(unittest.TestCase):
    """`read_s` relays the board between upkeep passes without adding upkeep."""

    def _run(self, tail, **kw):
        now = {"t": 0.0}
        passes = {"calls": 0}
        self.ps_calls = 0

        def game_pids():
            self.ps_calls += 1
            return [1]

        def each_poll():
            passes["calls"] += 1

        with patch.object(watch.time, "monotonic", lambda: now["t"]), \
             patch.object(watch.time, "sleep",
                          lambda s: now.__setitem__("t", now["t"] + s)), \
             patch.object(watch.env, "game_pids", game_pids):
            reason = watch.follow(tail, on_event=lambda _e: None,
                                  each_poll=each_poll, **kw)
        return reason, now["t"], passes["calls"]

    def test_a_board_written_mid_sleep_is_relayed_before_the_next_pass(self) -> None:
        class Tail:
            polls = 0

            def poll(self):
                Tail.polls += 1
                return [{"kind": "state", "turn": 9}] if Tail.polls == 4 else []

        reason, elapsed, upkeep = self._run(
            Tail(), timeout_s=60.0, poll_s=1.0, read_s=0.05, stall_s=None,
            stop_when=lambda event: event["kind"] == "state")
        self.assertEqual(reason, "stopped")
        # One upkeep pass, then the board three reads (0.15 s) into its sleep
        # rather than at the second pass a whole second later.
        self.assertEqual(upkeep, 1)
        self.assertAlmostEqual(elapsed, 0.15, places=6)

    def test_upkeep_keeps_its_cadence_while_the_tail_is_read_fast(self) -> None:
        class Tail:
            polls = 0

            def poll(self):
                Tail.polls += 1
                return []

        reason, elapsed, upkeep = self._run(
            Tail(), timeout_s=2.0, poll_s=0.5, read_s=0.05, stall_s=None)
        self.assertEqual(reason, "timeout")
        self.assertEqual(upkeep, 4)
        # Ten reads per half-second sleep plus one per pass.
        self.assertGreaterEqual(Tail.polls, 40)
        # The game's pid is re-checked by syscall; `ps` lists once per 10 s.
        self.assertEqual(self.ps_calls, 1)

    def test_without_read_s_the_loop_sleeps_once_per_pass(self) -> None:
        class Tail:
            polls = 0

            def poll(self):
                Tail.polls += 1
                return []

        reason, elapsed, upkeep = self._run(
            Tail(), timeout_s=2.0, poll_s=0.5, stall_s=None)
        self.assertEqual(reason, "timeout")
        self.assertEqual((upkeep, Tail.polls), (4, 4))


class FakeTail:
    """Emits whatever the test scripts, one batch per poll."""

    def __init__(self, batches):
        self.batches = list(batches)

    def poll(self):
        return self.batches.pop(0) if self.batches else []


def _follow(batches, **kw):
    # `env.game_pids()` is consulted every pass and must look alive, or the loop
    # returns "game exited" before either watchdog is reached.
    #
    # ⚠ THE CLOCK IS FAKE, AND THAT IS NOT A SHORTCUT — it is the only way these
    # assertions mean anything. A test that waits out a real five-second budget
    # is asserting on whatever the machine happened to manage in five seconds,
    # so under load it can reach a different pass count than it did when it was
    # written. Pinned to `poll_s`, "the budget expired on pass 500" is a fact.
    #
    # It is also 10 of the Python suite's 25 seconds, on every PR's CI gate and
    # every local validation, spent asleep.
    now = {"t": 0.0}
    original = watch.env.game_pids
    watch.env.game_pids = lambda: [1]
    try:
        with patch.object(watch.time, "monotonic", lambda: now["t"]), \
             patch.object(watch.time, "sleep",
                          lambda s: now.__setitem__("t", now["t"] + s)):
            return watch.follow(FakeTail(batches), timeout_s=5.0,
                                on_event=lambda e: None, poll_s=0.01, **kw)
    finally:
        watch.env.game_pids = original


def test_a_frozen_turn_is_caught_even_though_events_keep_arriving():
    # The exact shape of the congress wedge: a turn record, then the same turn
    # over and over, forever.
    batches = [[{"kind": "turn", "turn": 209}]] + [[{"kind": "state", "turn": 209}]] * 400
    reason = _follow(batches, stall_s=None, frozen_s=0.05)
    assert reason.startswith("stalled"), reason
    assert "209" in reason and "not advanced" in reason, reason


class RisingTail:
    """A game that never stops advancing, so the loop can only end on timeout.

    ⚠ A finite script cannot test this: once it runs out the turn really has
    stopped, and the watchdog fires — correctly. My first version of this test
    asserted the bug back.
    """

    def __init__(self):
        self.turn = 0

    def poll(self):
        self.turn += 1
        return [{"kind": "turn", "turn": self.turn}]


def test_a_run_whose_turn_is_advancing_is_never_called_frozen():
    now = {"t": 0.0}
    original = watch.env.game_pids
    watch.env.game_pids = lambda: [1]
    try:
        with patch.object(watch.time, "monotonic", lambda: now["t"]), \
             patch.object(watch.time, "sleep",
                          lambda s: now.__setitem__("t", now["t"] + s)):
            reason = watch.follow(RisingTail(), timeout_s=0.5,
                                  on_event=lambda e: None, poll_s=0.01,
                                  stall_s=None, frozen_s=0.05)
    finally:
        watch.env.game_pids = original
    assert reason == "timeout", reason


def test_setup_emits_no_turn_and_must_not_be_killed():
    """⚠ Before a turn is ever seen there is nothing to freeze.

    Setup emits no turn at all. Treating "no turn yet" as a frozen turn would kill
    every run before it started — the same mistake `popup_clear.py` guards with its
    own "no turn recorded yet; this is setup" refusal.
    """
    batches = [[{"kind": "seat", "civ": "CIVILIZATION_CANADA"}]] * 400
    reason = _follow(batches, stall_s=None, frozen_s=0.05)
    assert "not advanced" not in reason, reason


def test_silence_is_still_caught_by_its_own_watchdog():
    """The original death must keep working; the two are independent."""
    reason = _follow([[]], stall_s=0.05, frozen_s=None)
    assert reason.startswith("stalled: no event"), reason


class AiPhaseStallTest(RelayCadenceTest):
    """`ai_stall_s`: a rival turn the mod reported hung is not given the full
    silence window (civvis-20261112T112009Z hung at t90, t89 and t88)."""

    def test_a_reported_hang_ends_the_watch_after_its_own_window(self) -> None:
        class Tail:
            polls = 0

            def poll(self):
                Tail.polls += 1
                if Tail.polls == 2:
                    return [{"kind": "state", "turn": 90},
                            {"kind": "ai_phase_stall", "turn": 90, "waited": 30.5}]
                return []

        reason, elapsed, _ = self._run(
            Tail(), timeout_s=3600.0, poll_s=1.0, stall_s=420.0, ai_stall_s=240.0)
        self.assertTrue(reason.startswith(watch.AI_PHASE_STALL_REASON), reason)
        self.assertTrue(reason.startswith("stalled"), reason)
        self.assertGreater(elapsed, 240.0)
        self.assertLess(elapsed, 250.0)

    def test_a_slow_rival_turn_that_resumes_is_not_a_stall(self) -> None:
        class Tail:
            polls = 0

            def poll(self):
                Tail.polls += 1
                if Tail.polls == 2:
                    return [{"kind": "ai_phase_stall", "turn": 90, "waited": 30.5}]
                if Tail.polls == 100:
                    return [{"kind": "state", "turn": 91}]
                return []

        reason, elapsed, _ = self._run(
            Tail(), timeout_s=3600.0, poll_s=1.0, stall_s=420.0, ai_stall_s=240.0)
        # The later event cleared the report; only the ordinary silence
        # window ends the watch, 420 s after that event.
        self.assertEqual(reason, "stalled: no event for 420s")
        self.assertGreater(elapsed, 500.0)

    def test_without_the_window_a_reported_hang_waits_as_before(self) -> None:
        class Tail:
            polls = 0

            def poll(self):
                Tail.polls += 1
                return ([{"kind": "ai_phase_stall", "turn": 90}]
                        if Tail.polls == 2 else [])

        reason, elapsed, _ = self._run(
            Tail(), timeout_s=3600.0, poll_s=1.0, stall_s=420.0)
        self.assertEqual(reason, "stalled: no event for 420s")
        self.assertGreater(elapsed, 420.0)


#: The record the live seat wrote at 2026-11-15 07:27:34Z (civvis-20261115T071444Z
#: turn 159) and the two session rows before it, fields as relayed.
T159_UNANSWERED = {"kind": "deal_session", "phase": "unanswered", "queued": False,
                   "sent": True, "target": 4, "turn": 159, "unanswered": 1,
                   "why": "session_closed"}
T159_STALL = {"kind": "ai_phase_stall", "turn": 159, "waited": 30, "core_busy": False,
              "processing": False, "queued": False, "active_players": [4],
              "open_sessions": [], "visible": [], "held_locks": [], "lock_overflow": 0}


class HungRivalSessionsTest(RelayCadenceTest):
    """A stall report naming the rival of a dead deal session is not waited
    out; every other report keeps the caller's window."""

    def _tracker(self, *events):
        tracker = watch.HungRivalSessions()
        for event in events:
            tracker.note(event)
        return tracker

    def test_the_live_hang_of_turn_159_is_recognised(self) -> None:
        tracker = self._tracker(
            {"kind": "deal_session", "action": "EQUALIZE", "phase": "opening",
             "target": 4, "turn": 159},
            {"kind": "deal_session", "action": "EQUALIZE", "hold": 30, "phase": "asked",
             "sent": True, "session": 4, "target": 4, "turn": 159},
            T159_UNANSWERED)
        self.assertEqual(tracker.hung_after(T159_STALL), "unanswered/session_closed")
        self.assertEqual(tracker.window(T159_STALL), watch.AI_STALL_DEAD_SESSION_S)

    def test_a_refusal_whose_counter_never_came_is_recognised(self) -> None:
        tracker = self._tracker(
            {"kind": "deal_session", "phase": "counter_wait", "session": 4,
             "target": 4, "turn": 51, "hold": 4},
            {"kind": "deal_session", "phase": "closed", "target": 4, "turn": 51,
             "why": "counter_not_sent"})
        stall = dict(T159_STALL, turn=51)
        self.assertEqual(tracker.hung_after(stall), "closed/counter_not_sent")

    def test_every_other_report_keeps_the_callers_window(self) -> None:
        dead = self._tracker(T159_UNANSWERED)
        for name, tracker, stall in (
            ("no session this turn", self._tracker(), T159_STALL),
            ("a session that closed answered",
             self._tracker(dict(T159_UNANSWERED, phase="closed", why="answered")),
             T159_STALL),
            ("a counter that was answered",
             self._tracker(dict(T159_UNANSWERED, phase="closed", why="counter_answered")),
             T159_STALL),
            ("another rival's turn", dead, dict(T159_STALL, active_players=[2])),
            ("two active players", dead, dict(T159_STALL, active_players=[4, 2])),
            ("our own turn", dead, dict(T159_STALL, active_players=[0])),
            ("a busy core", dead, dict(T159_STALL, core_busy=True)),
            ("no core reading", dead, {k: v for k, v in T159_STALL.items()
                                       if k != "core_busy"}),
            ("the dead session was a turn earlier", dead, dict(T159_STALL, turn=160)),
            ("no active list", dead, dict(T159_STALL, active_players=None)),
            ("a boolean player", dead, dict(T159_STALL, active_players=[True])),
        ):
            with self.subTest(name):
                self.assertIsNone(tracker.hung_after(stall))
                self.assertIsNone(tracker.window(stall))

    def test_a_new_turn_forgets_the_last_turns_sessions(self) -> None:
        tracker = self._tracker(
            T159_UNANSWERED,
            {"kind": "deal_session", "phase": "closed", "target": 2, "turn": 160,
             "why": "answered"})
        self.assertIsNone(tracker.hung_after(dict(T159_STALL, turn=160)))
        self.assertIsNone(tracker.hung_after(T159_STALL))

    def test_rows_that_are_not_sessions_or_carry_no_rival_are_ignored(self) -> None:
        tracker = self._tracker(
            {"kind": "deal_active_hold", "phase": "holding", "target": 4, "turn": 159},
            {"kind": "deal_session", "phase": "unanswered", "turn": 159},
            {"kind": "deal_session", "phase": "unanswered", "target": "4", "turn": 159},
            "not an event")
        self.assertIsNone(tracker.hung_after(T159_STALL))

    def _tail(self, events, later=None):
        class Tail:
            polls = 0

            def poll(self):
                Tail.polls += 1
                if Tail.polls == 2:
                    return list(events)
                if later is not None and Tail.polls == later[0]:
                    return list(later[1])
                return []

        return Tail()

    def test_the_hang_behind_a_dead_session_ends_the_watch_in_seconds(self) -> None:
        tracker = watch.HungRivalSessions()
        tail = self._tail([T159_UNANSWERED, {"kind": "end_turn_wait", "turn": 159},
                           T159_STALL])
        now = {"t": 0.0}
        with patch.object(watch.time, "monotonic", lambda: now["t"]), \
             patch.object(watch.time, "sleep",
                          lambda s: now.__setitem__("t", now["t"] + s)), \
             patch.object(watch.env, "game_pids", lambda: [1]):
            reason = watch.follow(tail, on_event=tracker.note, timeout_s=3600.0,
                                  poll_s=0.25, stall_s=420.0, ai_stall_s=60.0,
                                  ai_stall_window=tracker.window)
        self.assertTrue(reason.startswith(watch.AI_PHASE_STALL_REASON), reason)
        self.assertIn("turn 159 had not advanced 5s after", reason)
        self.assertGreater(now["t"], watch.AI_STALL_DEAD_SESSION_S)
        self.assertLess(now["t"], watch.AI_STALL_DEAD_SESSION_S + 1.0)

    def test_a_report_without_a_dead_session_waits_the_callers_window(self) -> None:
        tracker = watch.HungRivalSessions()
        reason, elapsed, _ = self._run(
            self._tail([dict(T159_UNANSWERED, phase="closed", why="answered"),
                        T159_STALL]),
            timeout_s=3600.0, poll_s=0.25, stall_s=420.0, ai_stall_s=60.0,
            ai_stall_window=tracker.window)
        self.assertTrue(reason.startswith(watch.AI_PHASE_STALL_REASON), reason)
        self.assertIn("had not advanced 60s after", reason)
        self.assertGreater(elapsed, 60.0)
        self.assertLess(elapsed, 61.0)

    def test_a_record_inside_the_short_window_clears_the_report(self) -> None:
        tracker = watch.HungRivalSessions()
        now = {"t": 0.0}
        # Poll 2 is the stall; poll 10 (2 s later at 0.25 s a pass) is the
        # rival's turn going on after all.
        tail = self._tail([T159_UNANSWERED, T159_STALL],
                          later=(10, [{"kind": "state", "turn": 160}]))
        with patch.object(watch.time, "monotonic", lambda: now["t"]), \
             patch.object(watch.time, "sleep",
                          lambda s: now.__setitem__("t", now["t"] + s)), \
             patch.object(watch.env, "game_pids", lambda: [1]):
            reason = watch.follow(tail, on_event=tracker.note, timeout_s=3600.0,
                                  poll_s=0.25, stall_s=420.0, ai_stall_s=60.0,
                                  ai_stall_window=tracker.window)
        self.assertEqual(reason, "stalled: no event for 420s")
        self.assertGreater(now["t"], 420.0)

    def test_the_records_of_a_hung_game_do_not_clear_the_report(self) -> None:
        """Live civvis-20261115T071444Z-cont1, turn 175: the flush record and
        two HUD wakes arrived 150 s after the report and the old rule (any
        later event clears it) then waited on a hang for good."""
        stall = dict(T159_STALL, turn=175, active_players=[1])
        after = [{"kind": "ai_phase_stall_flush", "turn": 175},
                 {"kind": "controller_wake", "active": False, "frame": 1, "pending": 0,
                  "source": "TopPanel", "turn": 175},
                 {"kind": "ui_heartbeat", "frames": 600, "screen": "TopPanel"},
                 {"kind": "combat", "turn": 175, "our_turn": False},
                 {"kind": "order_verified", "turn": 174, "checked_on": 175}]
        # The report is poll 2; the later records land 10 s into a 60 s window.
        reason, elapsed, _ = self._run(
            self._tail([{"kind": "state", "turn": 175}, stall], later=(12, after)),
            timeout_s=3600.0, poll_s=1.0, stall_s=420.0, ai_stall_s=60.0)
        self.assertTrue(reason.startswith(watch.AI_PHASE_STALL_REASON), reason)
        self.assertIn("turn 175 had not advanced 60s after", reason)
        self.assertGreater(elapsed, 60.0)
        self.assertLess(elapsed, 63.0)

    def test_only_a_later_turn_clears_the_report(self) -> None:
        stall = dict(T159_STALL, turn=175, active_players=[1])
        for name, later, cleared in (
            ("the next turn's board", {"kind": "state", "turn": 176}, True),
            ("the next turn's first small record", {"kind": "moves_restored", "turn": 176}, True),
            ("the same turn again", {"kind": "state", "turn": 175}, False),
            ("a record with no turn", {"kind": "autoclose", "screen": "InGamePopup"}, False),
            ("a boolean turn", {"kind": "turn", "turn": True}, False),
        ):
            with self.subTest(name):
                reason, elapsed, _ = self._run(
                    self._tail([{"kind": "state", "turn": 175}, stall], later=(12, [later])),
                    timeout_s=3600.0, poll_s=1.0, stall_s=420.0, ai_stall_s=60.0)
                if cleared:
                    self.assertEqual(reason, "stalled: no event for 420s")
                else:
                    self.assertTrue(reason.startswith(watch.AI_PHASE_STALL_REASON), reason)
                    self.assertLess(elapsed, 63.0)

    def test_a_locked_console_or_an_operator_pause_holds_the_window(self) -> None:
        """`pause_when` credits its time to the report as to every other clock."""
        stall = dict(T159_STALL, turn=175, active_players=[1])
        now = {"t": 0.0}
        tail = self._tail([{"kind": "state", "turn": 175}, stall])
        with patch.object(watch.time, "monotonic", lambda: now["t"]), \
             patch.object(watch.time, "sleep",
                          lambda s: now.__setitem__("t", now["t"] + s)), \
             patch.object(watch.env, "game_pids", lambda: [1]):
            reason = watch.follow(tail, on_event=lambda _e: None, timeout_s=3600.0,
                                  poll_s=1.0, stall_s=420.0, ai_stall_s=60.0,
                                  pause_when=lambda: 10.0 <= now["t"] < 310.0)
        self.assertTrue(reason.startswith(watch.AI_PHASE_STALL_REASON), reason)
        self.assertGreater(now["t"], 360.0)
        self.assertLess(now["t"], 364.0)

    def test_the_window_is_only_ever_shortened_and_never_armed_from_off(self) -> None:
        stall = [{"kind": "ai_phase_stall", "turn": 90}]
        reason, elapsed, _ = self._run(
            self._tail(stall), timeout_s=3600.0, poll_s=1.0, stall_s=420.0,
            ai_stall_s=60.0, ai_stall_window=lambda _event: 900.0)
        self.assertIn("had not advanced 60s after", reason)
        reason, elapsed, _ = self._run(
            self._tail(stall), timeout_s=3600.0, poll_s=1.0, stall_s=420.0,
            ai_stall_s=60.0, ai_stall_window=lambda _event: -1.0)
        self.assertIn("had not advanced 60s after", reason)
        reason, elapsed, _ = self._run(
            self._tail(stall), timeout_s=3600.0, poll_s=1.0, stall_s=420.0,
            ai_stall_window=lambda _event: 5.0)
        self.assertEqual(reason, "stalled: no event for 420s")

    def test_a_second_report_gets_its_own_window(self) -> None:
        """A short window chosen for one report does not leak into the next."""
        tracker = watch.HungRivalSessions()
        # Stall with a dead session, cleared 2 s later; then a plain stall.
        tail = self._tail([T159_UNANSWERED, T159_STALL],
                          later=(10, [{"kind": "state", "turn": 160},
                                      dict(T159_STALL, turn=160)]))
        now = {"t": 0.0}
        with patch.object(watch.time, "monotonic", lambda: now["t"]), \
             patch.object(watch.time, "sleep",
                          lambda s: now.__setitem__("t", now["t"] + s)), \
             patch.object(watch.env, "game_pids", lambda: [1]):
            reason = watch.follow(tail, on_event=tracker.note, timeout_s=3600.0,
                                  poll_s=0.25, stall_s=420.0, ai_stall_s=60.0,
                                  ai_stall_window=tracker.window)
        self.assertIn("had not advanced 60s after", reason)
        self.assertGreater(now["t"], 62.0)


class StallHandoffTest(unittest.TestCase):
    """The marker that tells the climb a quickly-ended attempt was frozen."""

    def test_the_marker_is_the_one_the_climb_and_the_watchdog_use(self) -> None:
        import civ6_civvis_climb as climb
        self.assertEqual(watch.STALL_HANDOFF_MARKER, climb.WEDGE_HANDOFF_MARKER)
        script = (Path(__file__).resolve().parent / "ops"
                  / "civvis-agent-wedge-watchdog.sh").read_text()
        self.assertIn(f":-{watch.STALL_HANDOFF_MARKER}}}", script)

    def test_the_climb_reads_the_harness_marker_as_frozen_at_any_staleness(self) -> None:
        import civ6_civvis_climb as climb

        class Exited:
            def wait(self, timeout=None):
                return 0

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "run").mkdir()
            (root / "run" / "events.jsonl").write_text(
                json.dumps({"kind": "turn", "turn": 159}) + "\n")
            original = climb.RUN_ROOT
            climb.RUN_ROOT = root
            try:
                # No marker and a fresh turn: the exit is a finished game.
                self.assertEqual(climb.wait_watching_the_turn(
                    Exited(), "run", 1000.0, 900.0,
                    locked_probe=lambda: False, paused_probe=lambda: False), "exited")
                reason = (f"{watch.AI_PHASE_STALL_REASON}: turn 159 had not advanced "
                          "5s after the game reported a hung rival turn")
                marker = watch.write_stall_handoff(root / "run", "run", 159, reason)
                self.assertEqual(marker.name, climb.WEDGE_HANDOFF_MARKER)
                self.assertEqual(climb.wedge_handoff("run"), {
                    "tag": "run", "turn": 159, "reason": reason, "by": "civ6_play",
                    "utc": json.loads(marker.read_text())["utc"]})
                self.assertEqual(climb.wait_watching_the_turn(
                    Exited(), "run", 1000.0, 900.0,
                    locked_probe=lambda: False, paused_probe=lambda: False), "frozen")
            finally:
                climb.RUN_ROOT = original

    def test_an_existing_marker_is_left_alone(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            run = Path(temporary)
            first = '{"tag":"run","turn":88,"reason":"NO GAME PROGRESS","utc":"x"}\n'
            (run / watch.STALL_HANDOFF_MARKER).write_text(first)
            watch.write_stall_handoff(run, "run", 90, "later")
            self.assertEqual((run / watch.STALL_HANDOFF_MARKER).read_text(), first)
            self.assertEqual([p.name for p in run.iterdir()],
                             [watch.STALL_HANDOFF_MARKER])

    def test_an_unreadable_turn_is_recorded_as_none(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            marker = watch.write_stall_handoff(Path(temporary), "run", True, "r")
            self.assertIsNone(json.loads(marker.read_text())["turn"])


class ProjectionTest(unittest.TestCase):
    """`turns_left_seconds` is the whole decision, so test it on its own."""

    def test_it_reports_the_time_the_remaining_turns_should_take(self) -> None:
        # 100 turns in 200 s is 2 s a turn; 50 turns are left.
        self.assertEqual(
            watch.turns_left_seconds(1, 0.0, 101, 200.0, 151), 100.0)

    def test_a_run_past_its_finish_line_has_nothing_left_to_project(self) -> None:
        self.assertEqual(watch.turns_left_seconds(1, 0.0, 260, 100.0, 250), 0.0)

    def test_nothing_is_projected_without_two_turns_to_measure(self) -> None:
        """One turn is a point, not a rate, and setup has not even that."""
        self.assertIsNone(watch.turns_left_seconds(None, 0.0, None, 0.0, 250))
        self.assertIsNone(watch.turns_left_seconds(7, 5.0, 7, 5.0, 250))

    def test_no_finish_line_means_no_projection(self) -> None:
        self.assertIsNone(watch.turns_left_seconds(1, 0.0, 101, 200.0, None))


class ProgressBudgetTest(unittest.TestCase):
    """The three runs cut short on 2026-08-10/11, and the ones that deserve it."""

    class Rising:
        """Advances one turn per poll, forever."""

        def __init__(self, start=0):
            self.turn = start

        def poll(self):
            self.turn += 1
            return [{"kind": "turn", "turn": self.turn}]

    @staticmethod
    def _fake_clock():
        """A clock that moves only when the loop sleeps.

        ⚠ Real time makes these tests both slow and a coin toss: a run scripted
        to reach its finish line in 40 polls of 1 ms may or may not beat a 50 ms
        budget depending on the machine. With the clock pinned to `poll_s`, "the
        budget expired at poll 5" is a fact rather than a hope.
        """
        now = {"t": 0.0}
        return (lambda: now["t"]), (lambda s: now.__setitem__("t", now["t"] + s))

    def _follow(self, tail, **kw):
        monotonic, sleep = self._fake_clock()
        with patch.object(watch.time, "monotonic", monotonic), \
             patch.object(watch.time, "sleep", sleep), \
             patch.object(watch.env, "game_pids", return_value=[1]):
            return watch.follow(tail, on_event=lambda _e: None, poll_s=1.0,
                                stall_s=None, frozen_s=None, **kw)

    def test_a_run_that_can_still_reach_its_limit_is_given_the_time(self) -> None:
        """Turn 209 of 250 with the clock up: fifteen more minutes, not a kill.

        One turn a second, a 5 s budget and a finish line at turn 40. The old
        loop returned "timeout" at t=5 however healthy the run was. Projected
        from the rate actually managed, turn 40 arrives at t=39, inside a 60 s
        ceiling -- so the run keeps going and ends on its OWN terms.
        """
        reason = self._follow(
            self.Rising(), timeout_s=5.0, ceiling_s=60.0, finish_turn=40,
            stop_when=lambda event: event.get("turn", 0) >= 40)
        self.assertEqual(reason, "stopped")

    def test_a_run_that_cannot_get_there_is_still_stopped_on_time(self) -> None:
        """A game at turn 60 after two hours is not going to finish.

        This is the case the wall clock is FOR and it has to keep working: at
        one turn a second a finish line a million turns away does not fit under
        any ceiling, so the budget expires exactly as it used to.
        """
        reason = self._follow(self.Rising(), timeout_s=5.0, ceiling_s=600.0,
                              finish_turn=10 ** 6)
        self.assertEqual(reason, "timeout")

    def test_without_a_ceiling_the_budget_is_exactly_what_it_was(self) -> None:
        """The default must not be able to change any existing caller.

        `ceiling_s` defaults to `timeout_s`, so there is no room to extend into
        and every projection is refused -- whatever `finish_turn` says.
        """
        reason = self._follow(self.Rising(), timeout_s=5.0, finish_turn=40)
        self.assertEqual(reason, "timeout")

    def test_a_stalled_run_is_never_bought_more_time(self) -> None:
        """Extensions are for slow runs, not dead ones.

        A wedged game emits its turn forever without advancing. The rate stops
        with it, the projected finish runs off past any ceiling, and the frozen
        watchdog still gets it -- the ceiling must not become a way to sit in
        front of a wedge for longer than the old budget allowed.
        """
        batches = [[{"kind": "turn", "turn": 209}]] + \
                  [[{"kind": "state", "turn": 209}]] * 400
        monotonic, sleep = self._fake_clock()
        with patch.object(watch.time, "monotonic", monotonic), \
             patch.object(watch.time, "sleep", sleep), \
             patch.object(watch.env, "game_pids", return_value=[1]):
            reason = watch.follow(FakeTail(batches), timeout_s=5.0,
                                  on_event=lambda _e: None, poll_s=1.0,
                                  stall_s=None, frozen_s=3.0,
                                  finish_turn=250, ceiling_s=600.0)
        self.assertTrue(reason.startswith("stalled"), reason)
        self.assertIn("209", reason)


class FrozenFollowTest(unittest.TestCase):
    def test_frozen_turn_is_caught_while_events_keep_arriving(self) -> None:
        test_a_frozen_turn_is_caught_even_though_events_keep_arriving()

    def test_advancing_turn_is_never_called_frozen(self) -> None:
        test_a_run_whose_turn_is_advancing_is_never_called_frozen()

    def test_setup_without_a_turn_is_not_called_frozen(self) -> None:
        test_setup_emits_no_turn_and_must_not_be_killed()

    def test_silence_keeps_its_independent_watchdog(self) -> None:
        test_silence_is_still_caught_by_its_own_watchdog()


if __name__ == "__main__":
    failures = 0
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            started = time.time()
            try:
                fn()
                print(f"PASS {name}  ({time.time() - started:.2f}s)")
            except AssertionError as exc:
                failures += 1
                print(f"FAIL {name}: {exc}")
    raise SystemExit(1 if failures else 0)
