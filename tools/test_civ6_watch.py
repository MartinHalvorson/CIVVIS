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
