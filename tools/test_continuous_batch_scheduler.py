#!/usr/bin/env python3
"""Regression tests for the durable 5,000-completed-game rotation state."""
from __future__ import annotations

import datetime as dt
import json
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

TOOLS = Path(__file__).resolve().parent
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

import continuous_batch_scheduler as scheduler


def complete_rows(path: Path, *, seed_first: int, target_games: int, complete_games: int) -> None:
    """Write a tiny all-seats ledger; six physical rows represent one game."""
    lines = [
        {
            "kind": "header",
            "all_seats": True,
            "design": "independent",
            "players": 6,
            "batch": {
                "seed_first": seed_first,
                "seed_last": seed_first + target_games - 1,
                "target_games": target_games,
                "target_seats": target_games * 6,
            },
        }
    ]
    for game in range(complete_games):
        seed = seed_first + game
        for seat in range(6):
            lines.append({
                "kind": "game",
                "seed": seed,
                "arm": 0,
                "game": game,
                "seat": seat,
                "win": seat == 0,
                "winner": 0,
            })
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row) + "\n" for row in lines), encoding="utf-8")


class Reservations(unittest.TestCase):
    def test_worker_cap_is_eighty_five_percent_floor(self):
        self.assertEqual(scheduler.workers_for_cores(18), 15)
        self.assertEqual(scheduler.workers_for_cores(1), 1)
        self.assertEqual(scheduler.workers_for_cores(10), 8)

    def test_interrupted_segment_never_reuses_its_unplayed_tail(self):
        state = scheduler.new_state(171_011_669, 5_000)
        first = scheduler.reserve_segment(state, scheduler.empty_status())
        self.assertEqual((first["seed_first"], first["seed_last"]), (171_011_669, 171_016_668))
        # Imagine the process stopped after 746 games. The next target is the
        # remaining *completed-game* difference, but must start after all 5k
        # seeds already reserved for the interrupted process.
        partial = {"complete_games": 746}
        second = scheduler.reserve_segment(state, partial)
        self.assertEqual(second["seed_first"], 171_016_669)
        self.assertEqual(second["target_games"], 4_254)
        self.assertEqual(state["next_seed"], second["seed_last"] + 1)

    def test_state_rejects_a_next_seed_inside_a_reserved_window(self):
        state = scheduler.new_state(10, 5)
        scheduler.reserve_segment(state, scheduler.empty_status())
        state["next_seed"] = 11
        with self.assertRaisesRegex(scheduler.SchedulerError, "reuses a reserved seed"):
            scheduler.validate_state(state)


class MachineSharing(unittest.TestCase):
    """--cpu-share, --machine-cpu-ceiling and the progress line."""

    def test_worker_share_is_a_percent_floor_that_never_exceeds_the_cap(self):
        self.assertEqual(scheduler.workers_for_cores(18, 50), 9)
        self.assertEqual(scheduler.workers_for_cores(18, 85), 15)
        self.assertEqual(scheduler.workers_for_cores(1, 10), 1)
        with self.assertRaisesRegex(scheduler.SchedulerError, "85% cap"):
            scheduler.workers_for_cores(18, 90)

    def test_jobs_and_cpu_share_are_alternatives(self):
        import contextlib
        import io
        errors = io.StringIO()
        with tempfile.TemporaryDirectory() as temporary, contextlib.redirect_stderr(errors):
            root = Path(temporary)
            (root / ".git").mkdir()
            code = scheduler.main(["run", "--state-dir", str(root), "--repo", str(root),
                                   "--jobs", "4", "--cpu-share", "50"])
        self.assertEqual(code, 2)
        self.assertIn("alternatives", errors.getvalue())

    def test_host_busy_fraction_survives_a_wrapped_mach_counter(self):
        before = (2**32 - 100, 50)
        after = (200, 350)  # busy advanced 300 across the wrap, idle 300
        self.assertAlmostEqual(scheduler.busy_fraction(before, after), 0.5)
        self.assertIsNone(scheduler.busy_fraction((5, 5), (5, 5)))

    def test_cpu_time_reads_the_macos_and_linux_ps_forms(self):
        self.assertAlmostEqual(scheduler.parse_cpu_time("146:12.28"), 8772.28)
        self.assertAlmostEqual(scheduler.parse_cpu_time("0:00.04"), 0.04)
        self.assertEqual(scheduler.parse_cpu_time("01:02:03"), 3723)
        self.assertEqual(scheduler.parse_cpu_time("2-00:00:01"), 172_801)

    def test_overshoot_is_cut_at_once_and_headroom_retaken_by_halves(self):
        # 18 cores, ceiling 90% = 16.2 cores. The games burn 9 running; the
        # rest of the host burns 9, so only 7.2 remain for the games.
        duty, demand = scheduler.governed_duty(
            ceiling=0.9, cores=18, busy=1.0, group_cores=9.0, applied_duty=1.0,
            demand_cores=None, previous_duty=1.0)
        self.assertEqual(demand, 9.0)
        self.assertAlmostEqual(duty, 0.8)
        # The rest of the host drops to 5 cores: the full 9 fit again, but the
        # duty climbs half the way per window rather than jumping back.
        duty, demand = scheduler.governed_duty(
            ceiling=0.9, cores=18, busy=(5 + 7.2) / 18, group_cores=7.2, applied_duty=0.8,
            demand_cores=demand, previous_duty=duty)
        self.assertAlmostEqual(duty, 0.9)
        self.assertAlmostEqual(demand, 9.0)

    def test_a_host_saturated_by_others_pauses_the_games_and_keeps_their_demand(self):
        duty, demand = scheduler.governed_duty(
            ceiling=0.9, cores=18, busy=1.0, group_cores=0.0, applied_duty=0.0,
            demand_cores=9.0, previous_duty=0.3)
        self.assertEqual(duty, 0.0)
        self.assertEqual(demand, 9.0)
        # Headroom returns: a paused group resumes even though it measured nothing.
        duty, _ = scheduler.governed_duty(
            ceiling=0.9, cores=18, busy=0.5, group_cores=0.0, applied_duty=0.0,
            demand_cores=demand, previous_duty=duty)
        self.assertAlmostEqual(duty, 0.4)  # half of the (16.2 - 9) / 9 target

    def test_a_starved_measurement_never_shrinks_demand_below_the_workers(self):
        # Nine workers at the lowest priority on a host others saturate: when
        # resumed they were scheduled for 0.35 cores in a 0.2-duty window.
        # Sizing the next duty from that 1.75-core "demand" would let them take
        # all nine cores as soon as the other work paused.
        duty, demand = scheduler.governed_duty(
            ceiling=0.9, cores=18, busy=0.8, group_cores=0.35, applied_duty=0.2,
            demand_cores=None, previous_duty=0.2, demand_floor=9.0)
        self.assertEqual(demand, 9.0)
        # 16.2 - (14.4 - 0.35) = 2.15 allowed cores for nine workers.
        self.assertAlmostEqual(duty, 0.2 + 0.5 * (2.15 / 9.0 - 0.2))
        # A group that measures above its worker count keeps its measurement.
        _, demand = scheduler.governed_duty(
            ceiling=0.9, cores=18, busy=0.5, group_cores=9.4, applied_duty=1.0,
            demand_cores=demand, previous_duty=1.0, demand_floor=9.0)
        self.assertAlmostEqual(demand, 9.4)

    def test_a_mistimed_window_cannot_latch_the_games_near_zero(self):
        # 2026-10-09..15 on mbp-m5-max-128: seven workers, demand latched at
        # 144.6 cores, so the duty sat near 0.02 -- under 0.1, where demand is
        # never re-measured -- for six days while most of the host stood idle.
        duty, demand = scheduler.governed_duty(
            ceiling=1.0, cores=18, busy=0.75, group_cores=0.06, applied_duty=0.016,
            demand_cores=144.57, previous_duty=0.016, demand_floor=7.0)
        self.assertEqual(demand, 8.0, "seven one-thread workers plus the process's own")
        # 18 - (13.5 - 0.06) = 4.56 allowed cores; half the way from 0.016.
        self.assertAlmostEqual(duty, 0.016 + 0.5 * (4.56 / 8.0 - 0.016))
        # A fresh mis-timed reading (group ran far past its counted slices) is
        # capped the same way rather than re-latching.
        _, demand = scheduler.governed_duty(
            ceiling=1.0, cores=18, busy=0.5, group_cores=7.0, applied_duty=0.1,
            demand_cores=demand, previous_duty=0.5, demand_floor=7.0)
        self.assertEqual(demand, 8.0)

    def test_governor_counts_the_run_it_really_gave(self):
        # A governor thread that wakes late lets the games run past run_for;
        # applied_duty must report the run they got, not the run requested.
        import threading
        import time

        class Host:
            busy = 0

            def ticks(self):
                Host.busy += 1
                return (Host.busy, 99 * Host.busy)  # a 1%-busy host

        governor = scheduler.CpuGovernor(
            100, cores=18, workers=7, host=Host(), group_cpu=lambda _group: 0.0,
            send=lambda _group, _which: None)
        governor.duty = 0.1
        real_wait = threading.Event.wait

        def slow_wait(event, timeout=None):
            # Every wait overruns by 20 ms, like a starved thread's wake-up.
            return real_wait(event, None if timeout is None else timeout + 0.02)

        stop = threading.Event()
        with mock.patch.object(scheduler, "GOVERNOR_WINDOW_SECONDS", 0.2), \
                mock.patch.object(scheduler, "GOVERNOR_PERIOD_SECONDS", 0.05), \
                mock.patch.object(threading.Event, "wait", slow_wait):
            serving = threading.Thread(target=governor._serve, args=(5, stop), daemon=True)
            serving.start()
            deadline = time.monotonic() + 3.0
            while not governor.snapshot() and time.monotonic() < deadline:
                time.sleep(0.01)
            snapshot = governor.snapshot()
            stop.set()
            serving.join(timeout=3.0)
        self.assertTrue(snapshot)
        # Requested 0.1 of each slice (5 ms of 50) but each run lasted ~25 ms:
        # ~0.28 really applied, where counting run_for reported ~0.06.
        self.assertGreater(snapshot["applied_duty"], 0.2)

    def test_governor_pauses_an_overloaded_host_and_always_resumes_on_release(self):
        import signal as signals
        import threading
        import time

        class Host:
            busy = 0

            def ticks(self):
                # Every sample reports a fully busy host.
                Host.busy += 1000
                return (Host.busy, 0)

        sent: list[int] = []
        paused = threading.Event()

        def send(group, which):
            self.assertEqual(group, 4242)
            sent.append(which)
            if which == signals.SIGSTOP:
                paused.set()

        cpu = iter(float(value) for value in range(0, 10_000, 9))
        governor = scheduler.CpuGovernor(
            90, cores=18, host=Host(), group_cpu=lambda _group: next(cpu), send=send)
        with mock.patch.object(scheduler, "GOVERNOR_WINDOW_SECONDS", 0.05), \
                mock.patch.object(scheduler, "GOVERNOR_PERIOD_SECONDS", 0.01):
            governor.govern(4242)
            self.assertTrue(paused.wait(2.0), "an over-ceiling host must pause the games")
            deadline = time.monotonic() + 2.0
            while not governor.snapshot() and time.monotonic() < deadline:
                time.sleep(0.01)
            governor.release()
        self.assertEqual(sent[0], signals.SIGCONT, "govern must first clear an inherited pause")
        self.assertEqual(sent[-1], signals.SIGCONT, "release must leave the games running")
        self.assertLess(governor.snapshot()["next_duty"], 1.0)

    def test_pmset_power_readings(self):
        on_battery = ("Now drawing from 'Battery Power'\n -InternalBattery-0 (id=1)\t82%; "
                      "discharging; 1:02 remaining present: true\n")
        charging = ("Now drawing from 'AC Power'\n -InternalBattery-0 (id=1)\t4%; "
                    "charging; 2:14 remaining present: true\n")
        losing = ("Now drawing from 'AC Power'\n -InternalBattery-0 (id=1)\t40%; "
                  "discharging; (no estimate) present: true\n")
        desktop = "Now drawing from 'AC Power'\n"
        self.assertTrue(scheduler.battery_is_draining(on_battery))
        self.assertFalse(scheduler.battery_is_draining(charging))
        self.assertTrue(scheduler.battery_is_draining(losing), "an adapter that cannot keep up")
        self.assertFalse(scheduler.battery_is_draining(desktop))
        self.assertIsNone(scheduler.battery_is_draining("garbage"))

    def test_governor_pauses_the_games_on_battery_whatever_the_cpu(self):
        import signal as signals
        import threading
        import time

        class IdleHost:
            busy = 0
            idle = 0

            def ticks(self):
                # A nearly idle host: the ceiling alone would run the games.
                IdleHost.busy += 1
                IdleHost.idle += 99
                return (IdleHost.busy, IdleHost.idle)

        sent: list[int] = []
        snapshot_ready = threading.Event()
        governor = scheduler.CpuGovernor(
            90, cores=18, workers=9, host=IdleHost(), group_cpu=lambda _group: 0.0,
            send=lambda _group, which: sent.append(which), pause_on_battery=True,
            battery=lambda: True)
        with mock.patch.object(scheduler, "GOVERNOR_WINDOW_SECONDS", 0.05), \
                mock.patch.object(scheduler, "GOVERNOR_PERIOD_SECONDS", 0.01), \
                mock.patch("builtins.print"):
            governor.govern(11)
            deadline = time.monotonic() + 2.0
            while not governor.snapshot() and time.monotonic() < deadline:
                time.sleep(0.01)
            snapshot = governor.snapshot()
            governor.release()
        self.assertTrue(snapshot["on_battery"])
        self.assertEqual(snapshot["next_duty"], 0.0)
        self.assertEqual(snapshot["applied_duty"], 0.0)
        # After the opening SIGCONT that clears an inherited pause, nothing but
        # stops until release resumes the group (the thread and release() both
        # send that final SIGCONT).
        self.assertEqual(sent[0], signals.SIGCONT)
        middle = sent[1:]
        while middle and middle[-1] == signals.SIGCONT:
            middle.pop()
        self.assertTrue(middle)
        self.assertEqual(set(middle), {signals.SIGSTOP})
        self.assertEqual(sent[-1], signals.SIGCONT)

    def test_governor_without_a_measurement_never_pauses(self):
        import signal as signals

        class NoHost:
            def ticks(self):
                return None

        sent: list[int] = []
        governor = scheduler.CpuGovernor(
            90, cores=18, host=NoHost(), group_cpu=lambda _group: 1.0,
            send=lambda _group, which: sent.append(which))
        with mock.patch.object(scheduler, "GOVERNOR_WINDOW_SECONDS", 0.02):
            governor.govern(7)
            import time
            time.sleep(0.1)
            governor.release()
        self.assertNotIn(signals.SIGSTOP, sent)

    # --- whole workers ---------------------------------------------------

    @staticmethod
    def worker(pid, cpu, *, priority=20, group=77, name="gene_screen", stopped=False):
        return scheduler.ProcessRow(pid=pid, group=group, priority=priority, stopped=stopped,
                                    cpu_seconds=cpu, name=name)

    @staticmethod
    def seen(governor, workers):
        """The workers as the next reading of the process table has them: one
        the governor holds is stopped, and every other runs."""
        import dataclasses

        return [dataclasses.replace(row, stopped=row.pid in governor.held) for row in workers]

    def test_whole_worker_count_follows_the_room_under_the_ceiling(self):
        count = scheduler.governed_workers
        # 80% of 18 cores is 14.4. The rest of the host burns 9 - 5 = 4 cores,
        # so 10.4 are allowed: one more worker a window, not all at once.
        self.assertEqual(count(ceiling=0.8, cores=18, busy=0.5, group_cores=5.0,
                               running=5, total=14), 6)
        # Half a core over the allowance is tolerated ...
        self.assertEqual(count(ceiling=0.8, cores=18, busy=14.8 / 18, group_cores=5.0,
                               running=5, total=14), 5)
        # ... and so is less than a whole free core.
        self.assertEqual(count(ceiling=0.8, cores=18, busy=13.6 / 18, group_cores=5.0,
                               running=5, total=14), 5)
        # A compile takes eight cores: the whole overshoot goes in one window.
        # Rest 4 + 8 = 12, allowed 2.4, so two workers stay.
        self.assertEqual(count(ceiling=0.8, cores=18, busy=17.0 / 18, group_cores=5.0,
                               running=5, total=14), 2)
        # The rest of the host alone is over the ceiling: nothing runs.
        self.assertEqual(count(ceiling=0.6, cores=18, busy=1.0, group_cores=2.0,
                               running=3, total=9), 0)
        # Never more than there are.
        self.assertEqual(count(ceiling=1.0, cores=18, busy=0.1, group_cores=1.8,
                               running=3, total=3), 3)

    def test_process_table_names_a_row_by_the_last_part_of_its_path(self):
        rows = scheduler.parse_process_table(
            "  14687 14683  20 RN     64:33.53 /Users/a/sources/abc/target/release/gene_screen\n"
            "  84281 84281  31 S     101:02.10 /Users/a/Sid Meier's Civilization VI/Civ6.app/"
            "Contents/MacOS/Civ6_Exe_Child\n"
            "  14690 14683  20 TN      0:01.00 /usr/bin/caffeinate\n"
            "garbage\n")
        self.assertEqual([row.name for row in rows],
                         ["gene_screen", "Civ6_Exe_Child", "caffeinate"])
        self.assertEqual(rows[0].group, 14683)
        self.assertAlmostEqual(rows[0].cpu_seconds, 64 * 60 + 33.53)
        self.assertEqual([row.stopped for row in rows], [False, False, True])

    def governor_for_workers(self, **keywords):
        import signal as signals

        sent: list[tuple[int, int]] = []
        lifted: list[int] = []
        governor = scheduler.CpuGovernor(
            80, cores=18, processes=lambda: [], send=lambda _group, _which: None,
            send_one=lambda pid, which: sent.append((pid, which)), lift=lifted.append,
            **keywords)
        governor.background_priority = scheduler.BACKGROUND_PRIORITY
        return governor, sent, lifted, signals

    def test_the_worker_furthest_ahead_stops_first_and_the_one_furthest_behind_resumes(self):
        governor, sent, _lifted, signals = self.governor_for_workers()
        workers = [self.worker(1, 500.0), self.worker(2, 900.0), self.worker(3, 100.0),
                   self.worker(4, 700.0)]
        self.assertEqual(governor._apply_workers(workers, 2, now=10.0), 2)
        self.assertEqual(sent, [(2, signals.SIGSTOP), (4, signals.SIGSTOP)])
        self.assertEqual(governor.held, {2, 4})
        del sent[:]
        # Room for one more: the stopped worker with the least CPU time.
        self.assertEqual(governor._apply_workers(self.seen(governor, workers), 3, now=12.0), 3)
        self.assertEqual(sent, [(4, signals.SIGCONT)])
        self.assertEqual(governor.held, {2})

    def test_stopped_workers_take_turns_with_running_ones(self):
        governor, sent, _lifted, signals = self.governor_for_workers()
        workers = [self.worker(1, 900.0), self.worker(2, 400.0), self.worker(3, 100.0)]
        governor._apply_workers(workers, 2, now=0.0)
        self.assertEqual(governor.held, {1})
        del sent[:]
        later = [self.worker(1, 900.0), self.worker(2, 1000.0), self.worker(3, 700.0)]
        # Nothing moves before the rotation is due ...
        governor._apply_workers(self.seen(governor, later), 2, now=scheduler.WORKER_ROTATE_SECONDS - 1)
        self.assertEqual(sent, [])
        # ... then the running worker furthest ahead swaps with the held one.
        governor._apply_workers(self.seen(governor, later), 2, now=scheduler.WORKER_ROTATE_SECONDS + 1)
        self.assertEqual(sent, [(2, signals.SIGSTOP), (1, signals.SIGCONT)])
        self.assertEqual(governor.held, {2})
        del sent[:]
        # A swap that would trade places for under a minute of CPU is skipped.
        close = [self.worker(1, 1010.0), self.worker(2, 1000.0), self.worker(3, 1030.0)]
        governor._apply_workers(self.seen(governor, close), 2, now=3 * scheduler.WORKER_ROTATE_SECONDS)
        self.assertEqual(sent, [])

    def test_a_held_worker_that_something_else_resumed_is_stopped_again(self):
        # 2026-11-24: a benchmark stopped three running workers for its own
        # four minutes and resumed them as it left. In between the governor
        # had cut its target to nought and held all three, so they ran
        # outside its count for fifty-two minutes.
        governor, sent, _lifted, signals = self.governor_for_workers()
        workers = [self.worker(1, 500.0), self.worker(2, 900.0), self.worker(3, 100.0)]
        self.assertEqual(governor._apply_workers(workers, 0, now=1.0), 0)
        self.assertEqual(governor.held, {1, 2, 3})
        del sent[:]
        # The next reading shows them stopped: nothing to do.
        self.assertEqual(governor._apply_workers(self.seen(governor, workers), 0, now=2.0), 0)
        self.assertEqual(sent, [])
        # Something else resumes 1 and 3. The table shows them running, the
        # governor still holds them, and both are stopped again at once.
        resumed = [self.worker(1, 510.0), self.worker(2, 900.0, stopped=True), self.worker(3, 110.0)]
        self.assertEqual(governor._apply_workers(resumed, 0, now=3.0), 0)
        self.assertEqual(sorted(sent), [(1, signals.SIGSTOP), (3, signals.SIGSTOP)])
        self.assertEqual(governor.held, {1, 2, 3})
        del sent[:]
        # With room for one, the one it then lets run is not stopped again.
        self.assertEqual(governor._apply_workers(self.seen(governor, workers), 1, now=4.0), 1)
        self.assertEqual(sent, [(3, signals.SIGCONT)])
        del sent[:]
        self.assertEqual(governor._apply_workers(self.seen(governor, workers), 1, now=5.0), 1)
        self.assertEqual(sent, [])

    def test_a_worker_parked_in_the_background_band_is_stopped_and_lifted_when_it_resumes(self):
        governor, sent, lifted, signals = self.governor_for_workers()
        # Another controller parked worker 3 (priority 4): it is not running
        # in any useful sense, so it stops first and is never the first back.
        workers = [self.worker(1, 500.0), self.worker(2, 600.0),
                   self.worker(3, 100.0, priority=4)]
        self.assertEqual(governor._apply_workers(workers, 2, now=1.0), 2)
        self.assertEqual(sent, [(3, signals.SIGSTOP)])
        self.assertEqual(lifted, [])
        del sent[:]
        self.assertEqual(governor._apply_workers(self.seen(governor, workers), 1, now=2.0), 1)
        self.assertEqual(sent, [(2, signals.SIGSTOP)])
        del sent[:]
        # Back to two: the worker outside the band first, though it is ahead.
        self.assertEqual(governor._apply_workers(self.seen(governor, workers), 2, now=3.0), 2)
        self.assertEqual(sent, [(2, signals.SIGCONT)])
        self.assertEqual(lifted, [])
        del sent[:]
        # Room for the third: it is lifted out of the band as it resumes.
        self.assertEqual(governor._apply_workers(self.seen(governor, workers), 3, now=4.0), 3)
        self.assertEqual(lifted, [3])
        self.assertEqual(sent, [(3, signals.SIGCONT)])

    def test_governor_stops_whole_workers_never_the_group_and_resumes_on_release(self):
        import signal as signals
        import threading
        import time

        class Host:
            busy = 0
            idle = 0

            def ticks(self):
                # Every window the host reads 17 of 18 cores busy.
                Host.busy += 17
                Host.idle += 1
                return (Host.busy, Host.idle)

        clock = {"cpu": 0.0}

        def table():
            # Four workers that burn nothing measurable, a launcher, and a
            # stranger in another group: the rest of the host holds the load.
            clock["cpu"] += 0.001
            return [self.worker(pid, 100.0 * pid + clock["cpu"]) for pid in (11, 12, 13, 14)] + [
                self.worker(10, 1.0, name="Python"), self.worker(99, 5.0, group=5)]

        group_signals: list[int] = []
        one: list[tuple[int, int]] = []
        stopped = threading.Event()

        def send_one(pid, which):
            one.append((pid, which))
            if len({p for p, w in one if w == signals.SIGSTOP}) == 4:
                stopped.set()

        governor = scheduler.CpuGovernor(
            80, cores=18, host=Host(), processes=table,
            send=lambda _group, which: group_signals.append(which), send_one=send_one)
        with mock.patch.object(scheduler, "GOVERNOR_WINDOW_SECONDS", 0.02):
            governor.govern(77)
            self.assertTrue(stopped.wait(3.0), "an over-ceiling host must stop every worker")
            deadline = time.monotonic() + 2.0
            while governor.snapshot().get("workers_running") != 0 and time.monotonic() < deadline:
                time.sleep(0.01)
            snapshot = governor.snapshot()
            governor.release()
        self.assertEqual(snapshot["mode"], "workers")
        self.assertEqual((snapshot["workers"], snapshot["workers_running"]), (4, 0))
        self.assertEqual({pid for pid, _which in one}, {11, 12, 13, 14},
                         "only gene_screen processes of the governed group are signalled")
        self.assertNotIn(signals.SIGSTOP, group_signals, "whole workers: the group is never sliced")
        self.assertEqual(group_signals[-1], signals.SIGCONT, "release must leave the games running")
        self.assertEqual(governor.held, set())

    def test_work_named_beside_lowers_the_ceiling_while_it_runs(self):
        import threading
        import time

        class Host:
            busy = 0
            idle = 0

            def ticks(self):
                # 70% busy: under an 80% ceiling, over a 60% one.
                Host.busy += 7
                Host.idle += 3
                return (Host.busy, Host.idle)

        game_up = threading.Event()

        def table():
            rows = [self.worker(pid, 100.0 * pid) for pid in (11, 12, 13)]
            if game_up.is_set():
                rows.append(self.worker(500, 9.0, group=500, name="Civ6_Exe_Child"))
            return rows

        governor = scheduler.CpuGovernor(
            80, cores=10, host=Host(), processes=table, send=lambda _group, _which: None,
            send_one=lambda _pid, _which: None, beside={"Civ6_Exe_Child": 60, "rustc": 70})

        def reading(want):
            deadline = time.monotonic() + 3.0
            while time.monotonic() < deadline:
                snapshot = governor.snapshot()
                if snapshot and want(snapshot):
                    return snapshot
                time.sleep(0.01)
            self.fail(f"no such reading; last was {governor.snapshot()}")

        with mock.patch.object(scheduler, "GOVERNOR_WINDOW_SECONDS", 0.02):
            governor.govern(77)
            # The rest of the host holds seven of ten cores: one worker fits
            # under eight, none under six.
            alone = reading(lambda snapshot: "foreground" not in snapshot
                            and snapshot["workers_running"] == 1)
            self.assertEqual(alone["ceiling_percent"], 80.0)
            self.assertEqual(alone["workers_next"], 1)
            game_up.set()
            beside = reading(lambda snapshot: snapshot.get("foreground") == "Civ6_Exe_Child"
                             and snapshot["workers_running"] == 0)
            self.assertEqual(beside["ceiling_percent"], 60.0)
            governor.release()

    def test_a_sliced_group_that_becomes_several_workers_is_resumed_whole(self):
        # One worker is sliced with the group, launcher included; when the
        # second worker arrives the group must be running again, or the
        # launcher never starts the third.
        import signal as signals
        import threading
        import time

        class Host:
            busy = 0

            def ticks(self):
                Host.busy += 1000
                return (Host.busy, 0)  # a saturated host: the slice plan pauses the group

        second = threading.Event()
        group_signals: list[int] = []
        paused = threading.Event()

        def send(_group, which):
            group_signals.append(which)
            if which == signals.SIGSTOP:
                paused.set()

        def table():
            rows = [self.worker(10, 1.0, name="Python"), self.worker(11, 100.0)]
            if second.is_set():
                rows.append(self.worker(12, 0.0))
            return rows

        governor = scheduler.CpuGovernor(
            80, cores=18, workers=14, host=Host(), processes=table, send=send,
            send_one=lambda _pid, _which: None)
        with mock.patch.object(scheduler, "GOVERNOR_WINDOW_SECONDS", 0.04), \
                mock.patch.object(scheduler, "GOVERNOR_PERIOD_SECONDS", 0.01):
            governor.govern(77)
            self.assertTrue(paused.wait(3.0), "one worker on a saturated host is sliced")
            second.set()
            deadline = time.monotonic() + 3.0
            while governor.snapshot().get("mode") != "workers" and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertEqual(governor.snapshot().get("mode"), "workers")
            sent_so_far = list(group_signals)
            governor.release()
        self.assertEqual(sent_so_far[-1], signals.SIGCONT,
                         "whole workers must start from a running group")

    def test_the_load_ceiling_stops_a_worker_at_it_and_adds_none_near_it(self):
        limit = scheduler.load_limited
        # Heading for 17.2 under a ceiling of 15: one worker stops, whatever CPU allows.
        self.assertEqual(limit(7, running=6, projected=17.2, ceiling=15.0), 5)
        # Within one of the ceiling: nobody is added, nobody stops.
        self.assertEqual(limit(7, running=6, projected=14.3, ceiling=15.0), 6)
        # Clear of it: the CPU plan stands.
        self.assertEqual(limit(7, running=6, projected=12.0, ceiling=15.0), 7)
        # It never raises a count the CPU plan lowered, and never goes below none.
        self.assertEqual(limit(2, running=6, projected=12.0, ceiling=15.0), 2)
        self.assertEqual(limit(0, running=0, projected=30.0, ceiling=15.0), 0)

    def test_a_beside_option_may_carry_a_load_ceiling(self):
        options = ["Civ6_Exe_Child=60", "/x/heavy.sh=55/13"]
        self.assertEqual(scheduler.parse_beside(options),
                         {"Civ6_Exe_Child": 60.0, "/x/heavy.sh": 55.0})
        self.assertEqual(scheduler.parse_beside_loads(options), {"/x/heavy.sh": 13.0})
        for bad in ("rustc=55/fast", "rustc=/13"):
            with self.assertRaises(scheduler.SchedulerError):
                scheduler.parse_beside([bad])
        with self.assertRaises(scheduler.SchedulerError):
            scheduler.parse_beside_loads(["rustc=55/0"])
        with self.assertRaises(scheduler.SchedulerError):
            scheduler.CpuGovernor(80, cores=18, load_ceiling=0)

    def test_governor_holds_the_load_under_its_ceiling_though_cpu_has_room(self):
        # An idle host by CPU, a one-minute load of 20: the workers queue
        # behind something CPU does not show. A waiting script tightens the
        # ceiling further, and a load well under it lets a worker back in.
        import threading
        import time

        class Host:
            busy = 0
            idle = 0

            def ticks(self):
                Host.busy += 1
                Host.idle += 99
                return (Host.busy, Host.idle)

        load = {"now": 20.0}
        queued = threading.Event()
        governor = scheduler.CpuGovernor(
            80, cores=18, host=Host(),
            processes=lambda: [self.worker(pid, 100.0 * pid) for pid in (11, 12, 13, 14)],
            send=lambda _group, _which: None, send_one=lambda _pid, _which: None,
            load_ceiling=15.0, loadavg=lambda: (load["now"], 0.0, 0.0),
            beside_scripts={"/x/heavy.sh": 55}, beside_script_loads={"/x/heavy.sh": 13.0},
            commands=lambda: (["/bin/zsh /x/heavy.sh 3 cargo build"] if queued.is_set() else []))

        def reading(want):
            deadline = time.monotonic() + 3.0
            while time.monotonic() < deadline:
                snapshot = governor.snapshot()
                if snapshot and want(snapshot):
                    return snapshot
                time.sleep(0.01)
            self.fail(f"no such reading; last was {governor.snapshot()}")

        with mock.patch.object(scheduler, "GOVERNOR_WINDOW_SECONDS", 0.02):
            governor.govern(77)
            over = reading(lambda snapshot: snapshot.get("load_ceiling") == 15.0
                           and snapshot["workers_running"] == 0 and snapshot["workers_next"] == 0)
            self.assertEqual(over["load"], 20.0)
            load["now"] = 14.0  # under the host's ceiling of 15, but within one of it
            near = reading(lambda snapshot: snapshot.get("load") == 14.0)
            self.assertEqual(near["workers_next"], 0, "nobody is added within one of the ceiling")
            load["now"] = 5.0
            reading(lambda snapshot: snapshot["workers_running"] == 1)
            queued.set()
            load["now"] = 13.5  # fine for the host, over what the queued script asks
            tight = reading(lambda snapshot: snapshot.get("load_ceiling") == 13.0
                            and snapshot["workers_running"] == 0)
            self.assertEqual(tight["foreground"], "heavy.sh")
            governor.release()

    def test_a_worker_is_added_only_while_the_running_ones_are_fed(self):
        count = scheduler.governed_workers
        # mbp-m5-max-128, 2026-11-23: the host reads 55% busy and the ceiling
        # is 80%, so by CPU alone there is room for many more. But six running
        # workers are receiving 4.7 cores: the class they run in is full.
        self.assertEqual(count(ceiling=0.8, cores=18, busy=0.55, group_cores=4.7,
                               running=6, total=14), 6, "no seventh worker into a queue")
        # Fourteen running on the same 4.7 cores: nine of them only wait.
        self.assertEqual(count(ceiling=0.8, cores=18, busy=0.55, group_cores=4.7,
                               running=14, total=14), 6,
                         "one worker stays queued for the next free core, the rest stop")
        # Fed workers grow as before.
        self.assertEqual(count(ceiling=0.8, cores=18, busy=0.55, group_cores=5.8,
                               running=6, total=14), 7)
        # Starved outright on a saturated host: the ceiling's own cut still rules.
        self.assertEqual(count(ceiling=0.8, cores=18, busy=1.0, group_cores=0.3,
                               running=5, total=14), 0)
        # Starved under the ceiling: one worker keeps waiting.
        self.assertEqual(count(ceiling=0.8, cores=18, busy=0.5, group_cores=0.2,
                               running=5, total=14), 1)

    def test_a_governor_taking_over_a_running_segment_starts_from_no_workers(self):
        import signal as signals
        import threading
        import time

        class Host:
            busy = 0
            idle = 0

            def ticks(self):
                Host.busy += 1
                Host.idle += 99  # an idle host: every worker could run
                return (Host.busy, Host.idle)

        stops: list[int] = []
        first_plan = threading.Event()

        def send_one(pid, which):
            if which == signals.SIGSTOP:
                stops.append(pid)
                if len(set(stops)) == 4:
                    first_plan.set()

        governor = scheduler.CpuGovernor(
            80, cores=18, host=Host(),
            processes=lambda: [self.worker(pid, 100.0 * pid) for pid in (11, 12, 13, 14)],
            send=lambda _group, _which: None, send_one=send_one)
        with mock.patch.object(scheduler, "GOVERNOR_WINDOW_SECONDS", 0.05):
            governor.govern(77)
            self.assertTrue(first_plan.wait(3.0),
                            "the first window stops every worker of an adopted segment")
            time.sleep(0.01)
            governor.release()
        self.assertEqual(set(stops[:4]), {11, 12, 13, 14})

    def test_room_is_retaken_against_the_recent_peak_of_other_work(self):
        count = scheduler.governed_workers
        # This window the rest of the host used 4 cores (9 - 5): 10.4 allowed,
        # a sixth worker fits. But a moment ago it used 9: against that peak
        # only 5.4 are free, and the games stay where they are.
        self.assertEqual(count(ceiling=0.8, cores=18, busy=0.5, group_cores=5.0,
                               running=5, total=14), 6)
        self.assertEqual(count(ceiling=0.8, cores=18, busy=0.5, group_cores=5.0,
                               running=5, total=14, rest_peak=9.0), 5)
        # The peak never cuts: only the window just ended does.
        self.assertEqual(count(ceiling=0.8, cores=18, busy=0.5, group_cores=5.0,
                               running=5, total=14, rest_peak=13.0), 5)
        # And a burst in this window cuts at once whatever the peak was.
        self.assertEqual(count(ceiling=0.8, cores=18, busy=17.0 / 18, group_cores=5.0,
                               running=5, total=14, rest_peak=1.0), 2)

    def test_named_work_that_stops_keeps_its_ceiling_for_a_while(self):
        # A game between two matches is gone for seconds: its ceiling must
        # outlast the gap, and end once the work has really stopped.
        import threading
        import time

        class Host:
            busy = 0
            idle = 0

            def ticks(self):
                Host.busy += 7
                Host.idle += 3
                return (Host.busy, Host.idle)

        game_up = threading.Event()
        game_up.set()

        def table():
            rows = [self.worker(pid, 100.0 * pid) for pid in (11, 12, 13)]
            if game_up.is_set():
                rows.append(self.worker(500, 9.0, group=500, name="Civ6_Exe_Child"))
            return rows

        governor = scheduler.CpuGovernor(
            80, cores=10, host=Host(), processes=table, send=lambda _group, _which: None,
            send_one=lambda _pid, _which: None, beside={"Civ6_Exe_Child": 60},
            linger_seconds=0.5)
        with mock.patch.object(scheduler, "GOVERNOR_WINDOW_SECONDS", 0.02):
            governor.govern(77)
            deadline = time.monotonic() + 3.0
            while (governor.snapshot().get("ceiling_percent") != 60.0
                   and time.monotonic() < deadline):
                time.sleep(0.01)
            self.assertEqual(governor.snapshot().get("ceiling_percent"), 60.0)
            game_up.clear()
            gone = time.monotonic()
            time.sleep(0.2)
            held = governor.snapshot()
            self.assertEqual(held["ceiling_percent"], 60.0,
                             "0.2 s after the game quit its ceiling still holds")
            self.assertEqual(held["foreground"], "Civ6_Exe_Child")
            deadline = time.monotonic() + 3.0
            while (governor.snapshot().get("ceiling_percent") != 80.0
                   and time.monotonic() < deadline):
                time.sleep(0.01)
            released = time.monotonic() - gone
            self.assertEqual(governor.snapshot().get("ceiling_percent"), 80.0)
            self.assertGreaterEqual(released, 0.5)
            governor.release()

    def test_a_script_is_seen_while_it_runs_or_waits_but_not_in_our_own_options(self):
        lines = [
            "/usr/bin/python3 curated_scheduler.py run --beside-script /x/heavy.sh=55",
            "/bin/zsh /x/heavy.sh 3 cargo test --profile ci",
            "vim /y/heavy.sh.bak",
        ]
        self.assertEqual(scheduler.scripts_running(lines, ["/x/heavy.sh", "/x/other.sh"]),
                         {"/x/heavy.sh"})
        self.assertEqual(scheduler.scripts_running(lines[:1] + lines[2:], ["/x/heavy.sh"]), set())

    def test_a_queued_script_lowers_the_ceiling_though_it_burns_nothing(self):
        import threading
        import time

        class Host:
            busy = 0
            idle = 0

            def ticks(self):
                # 70% busy: under an 80% ceiling, over a 55% one.
                Host.busy += 7
                Host.idle += 3
                return (Host.busy, Host.idle)

        queued = threading.Event()
        governor = scheduler.CpuGovernor(
            80, cores=10, host=Host(),
            processes=lambda: [self.worker(pid, 100.0 * pid) for pid in (11, 12, 13)],
            send=lambda _group, _which: None, send_one=lambda _pid, _which: None,
            beside_scripts={"/x/heavy.sh": 55},
            commands=lambda: (["/bin/zsh /x/heavy.sh 3 cargo build"] if queued.is_set() else []))

        def reading(want):
            deadline = time.monotonic() + 3.0
            while time.monotonic() < deadline:
                snapshot = governor.snapshot()
                if snapshot and want(snapshot):
                    return snapshot
                time.sleep(0.01)
            self.fail(f"no such reading; last was {governor.snapshot()}")

        with mock.patch.object(scheduler, "GOVERNOR_WINDOW_SECONDS", 0.02):
            governor.govern(77)
            alone = reading(lambda snapshot: "foreground" not in snapshot
                            and snapshot["workers_running"] == 1)
            self.assertEqual(alone["ceiling_percent"], 80.0)
            queued.set()
            beside = reading(lambda snapshot: snapshot.get("foreground") == "heavy.sh"
                             and snapshot["workers_running"] == 0)
            self.assertEqual(beside["ceiling_percent"], 55.0)
            governor.release()

    def test_beside_options_name_a_process_and_a_ceiling(self):
        self.assertEqual(scheduler.parse_beside(["Civ6_Exe_Child=70", "rustc=60"]),
                         {"Civ6_Exe_Child": 70.0, "rustc": 60.0})
        for bad in ("rustc", "=60", "rustc=fast"):
            with self.assertRaises(scheduler.SchedulerError):
                scheduler.parse_beside([bad])
        with self.assertRaises(scheduler.SchedulerError):
            scheduler.CpuGovernor(80, cores=18, beside={"rustc": 0})
        # Work to stand back for can only lower the ceiling.
        governor = scheduler.CpuGovernor(60, cores=18, beside={"rustc": 90})
        self.assertEqual(governor.beside, {"rustc": 0.6})

    def test_progress_line_counts_this_batch_and_every_batch_before_it(self):
        state = scheduler.new_state(1000, 5000)
        state["history"].append({"complete_games": 5000, "complete_seats": 30000})
        report = {"batch": state["current"]["id"], "phase": "running", "complete_games": 120,
                  "complete_seats": 720, "goal_completed_games": 5000,
                  "games_per_hour": 150.0, "eta_at": "2026-10-02T00:00:00Z"}
        log = scheduler.ProgressLog(5)
        line = log.line(state, report, {"host_busy_percent": 88.5, "games_cores": 8.1,
                                        "applied_duty": 0.93}, now=0.0)
        self.assertIn("games=120/5,000 seats=720/30,000", line)
        self.assertIn("all_batches games=5,120 seats=30,720", line)
        self.assertIn("host_cpu=88.5%", line)
        self.assertIsNone(log.line(state, report, now=299.0))
        self.assertIsNotNone(log.line(state, report, now=300.0))


class SourcePreparation(unittest.TestCase):
    def test_pinned_gene_screen_build_enables_developer_tools(self):
        """The scheduler must build the feature-gated tournament binary."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            commit = "a" * 40
            source_dir = root / "sources" / commit
            (source_dir / ".git").mkdir(parents=True)
            binary = source_dir / "target" / "release" / "gene_screen"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"test binary")
            commands: list[list[str]] = []

            def git_output(repo: Path, *args: str) -> str:
                if args == ("rev-parse", "origin/main"):
                    return commit
                if args == ("rev-parse", "HEAD"):
                    self.assertEqual(repo, source_dir)
                    return commit
                if args == ("status", "--porcelain"):
                    self.assertEqual(repo, source_dir)
                    return ""
                self.fail(f"unexpected git command: {args}")

            def run_checked(command, **_kwargs):
                commands.append(list(command))
                return SimpleNamespace(stdout="")

            with mock.patch.object(scheduler, "git_output", side_effect=git_output), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run_checked), \
                    mock.patch.object(scheduler, "sha256", return_value="b" * 64):
                source = scheduler.ensure_source(root, scheduler.new_state(100, 1), root / "repo")

            self.assertEqual(source["commit"], commit)
            self.assertIn(
                [
                    "cargo", "build", "--release", "--locked", "--features",
                    "developer-tools", "--bin", "gene_screen",
                ],
                commands,
            )

            # A shared machine builds on the games' worker budget.
            commands.clear()
            with mock.patch.object(scheduler, "git_output", side_effect=git_output), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run_checked), \
                    mock.patch.object(scheduler, "sha256", return_value="b" * 64):
                scheduler.ensure_source(root, scheduler.new_state(100, 1), root / "repo",
                                        build_jobs=9)
            self.assertIn(
                [
                    "cargo", "build", "--release", "--locked", "--features",
                    "developer-tools", "--bin", "gene_screen", "--jobs", "9",
                ],
                commands,
            )


class ValidatedRows(unittest.TestCase):
    def test_scheduler_counts_one_game_from_six_seat_rows(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(500, 1)
            batch = state["current"]
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            complete_rows(root / batch["rows"], seed_first=reservation["seed_first"],
                          target_games=1, complete_games=1)
            status = scheduler.refresh_status(root, state)
            self.assertEqual(status["complete_games"], 1)
            self.assertEqual(status["complete_seats"], 6)
            self.assertEqual(status["records"], 7)
            self.assertEqual(batch["wins"], 1)

    def test_an_unchanged_ledger_is_parsed_once_across_progress_polls(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(600, 3)
            batch = state["current"]
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            row_path = root / batch["rows"]
            complete_rows(row_path, seed_first=reservation["seed_first"],
                          target_games=3, complete_games=1)
            with mock.patch.object(scheduler, "summarize", wraps=scheduler.summarize) as reader:
                first = scheduler.refresh_status(root, state)
                first["complete_games"] = 99  # a caller's edit must not reach the cache
                self.assertEqual(scheduler.refresh_status(root, state)["complete_games"], 1)
                self.assertEqual(reader.call_count, 1)
                complete_rows(row_path, seed_first=reservation["seed_first"],
                              target_games=3, complete_games=2)
                self.assertEqual(scheduler.refresh_status(root, state)["complete_games"], 2)
                self.assertEqual(reader.call_count, 2)

    def test_a_ledger_caught_mid_write_is_read_again_next_poll(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(700, 2)
            batch = state["current"]
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            row_path = root / batch["rows"]
            complete_rows(row_path, seed_first=reservation["seed_first"],
                          target_games=2, complete_games=1)
            whole = row_path.read_text(encoding="utf-8")
            # Half of the second game's six seats: the reader refuses to count it.
            complete_rows(row_path, seed_first=reservation["seed_first"],
                          target_games=2, complete_games=2)
            seats = row_path.read_text(encoding="utf-8").splitlines(keepends=True)[-6:]
            row_path.write_text(whole + "".join(seats[:3]), encoding="utf-8")
            with self.assertRaisesRegex(scheduler.SchedulerError, "not safe to count"):
                scheduler.refresh_status(root, state)
            row_path.write_text(whole + "".join(seats), encoding="utf-8")
            self.assertEqual(scheduler.refresh_status(root, state)["complete_games"], 2)

    def test_rows_from_an_unreserved_header_fail_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(900, 1)
            batch = state["current"]
            complete_rows(root / batch["rows"], seed_first=901, target_games=1, complete_games=1)
            with self.assertRaisesRegex(scheduler.SchedulerError, "outside this scheduler state"):
                scheduler.refresh_status(root, state)

    def test_non_six_player_header_cannot_enter_a_standard_rotation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(1_000, 1)
            batch = state["current"]
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            row_path = root / batch["rows"]
            complete_rows(row_path, seed_first=reservation["seed_first"], target_games=1, complete_games=1)
            rows = [json.loads(line) for line in row_path.read_text().splitlines()]
            rows[0]["players"] = 5
            row_path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
            with self.assertRaisesRegex(scheduler.SchedulerError, "not safe to count"):
                scheduler.refresh_status(root, state)

    def test_publication_rechecks_the_frozen_analysis_before_any_pr_side_effect(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(2_000, 1)
            batch = state["current"]
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            complete_rows(root / batch["rows"], seed_first=reservation["seed_first"],
                          target_games=1, complete_games=1)
            scheduler.refresh_status(root, state)
            batch["phase"] = "frozen"
            (root / batch["analysis"]).write_text(json.dumps({
                "kind": "gene_screen_analysis",
                "games": 2,
                "seats": 6,
                "batch": {
                    "complete_games": 1,
                    "complete_seats": 6,
                    "target_games": 1,
                    "target_seats": 6,
                },
            }), encoding="utf-8")
            with self.assertRaisesRegex(scheduler.SchedulerError, "no longer matches"):
                scheduler.publish_batch(
                    root, root / "scheduler-state.json", state, repo=root,
                    machine="test-machine", agent="continuous-batch")


class PublicationRecovery(unittest.TestCase):
    def pushed_publication(self, root: Path) -> tuple[dict, dict, Path]:
        state = scheduler.new_state(2_000, 1)
        batch = state["current"]
        worktree = root / "publication-worktree"
        worktree.mkdir()
        batch.update({
            "phase": "publishing",
            "complete_games": 1,
            "complete_seats": 6,
            "wins": 1,
            "publication": {
                "stage": "pushed",
                "worktree": str(worktree),
                "pr_number": 123,
                "report": "docs/gene_screens/example.json",
            },
        })
        return state, batch, worktree

    def test_already_merged_publication_is_persisted_without_shipping_again(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, batch, _worktree = self.pushed_publication(root)

            def run(command, **_kwargs):
                self.assertEqual(command[:3], ["gh", "pr", "view"])
                return SimpleNamespace(stdout=json.dumps({
                    "state": "MERGED",
                    "mergedAt": "2026-08-25T13:35:00Z",
                    "mergeCommit": {"oid": "a" * 40},
                }))

            with mock.patch.object(scheduler, "refresh_status", return_value={"complete_games": 1}), \
                    mock.patch.object(scheduler, "validate_analysis"), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run):
                scheduler.publish_batch(
                    root, root / "scheduler-state.json", state, repo=root,
                    machine="test-machine", agent="continuous-batch")

            self.assertEqual(batch["phase"], "published")
            self.assertEqual(batch["publication"]["stage"], "merged")
            self.assertEqual(batch["publication"]["merged_at"], "2026-08-25T13:35:00Z")
            self.assertEqual(batch["publication"]["merge_commit"], "a" * 40)
            persisted = json.loads((root / "scheduler-state.json").read_text(encoding="utf-8"))
            self.assertEqual(persisted["current"]["publication"]["stage"], "merged")

    def test_claimed_publication_deletes_the_waivers_its_report_proves(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, batch, _worktree = self.pushed_publication(root)
            batch["publication"]["stage"] = "claimed"
            commands = []

            def run(command, **_kwargs):
                commands.append(command)
                if command[:3] == ["gh", "pr", "view"]:
                    return SimpleNamespace(stdout=json.dumps({"state": "OPEN"}))
                if command[:2] == ["git", "status"]:
                    return SimpleNamespace(stdout="\n".join((
                        " M docs/gene_ledger.json",
                        " M tools/gene_fire_waivers.json",
                        "?? docs/gene_screens/example.json",
                    )))
                return SimpleNamespace(stdout="")

            reached = scheduler.SchedulerError("the guard let the waiver file through")
            with mock.patch.object(scheduler, "refresh_status", return_value={"complete_games": 1}), \
                    mock.patch.object(scheduler, "validate_analysis"), \
                    mock.patch.object(scheduler, "write_reporting_artifact"), \
                    mock.patch.object(scheduler.subprocess, "run",
                                      return_value=SimpleNamespace(returncode=0)), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run), \
                    mock.patch.object(scheduler, "validate_publication", side_effect=reached):
                with self.assertRaisesRegex(scheduler.SchedulerError, "let the waiver file through"):
                    scheduler.publish_batch(
                        root, root / "scheduler-state.json", state, repo=root,
                        machine="test-machine", agent="continuous-batch")
            self.assertIn([sys.executable, "tools/gene_fires.py", "--prune-proven"], commands)
            prune = commands.index([sys.executable, "tools/gene_fires.py", "--prune-proven"])
            status = next(i for i, command in enumerate(commands) if command[:2] == ["git", "status"])
            self.assertLess(prune, status, "the guard must see the pruned file")

    def test_claimed_publication_merged_by_an_operator_skips_regeneration(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, batch, _worktree = self.pushed_publication(root)
            batch["publication"]["stage"] = "claimed"

            def run(command, **_kwargs):
                self.assertEqual(command[:3], ["gh", "pr", "view"])
                return SimpleNamespace(stdout=json.dumps({
                    "state": "MERGED",
                    "mergedAt": "2026-08-25T13:37:00Z",
                    "mergeCommit": {"oid": "c" * 40},
                }))

            with mock.patch.object(scheduler, "refresh_status", return_value={"complete_games": 1}), \
                    mock.patch.object(scheduler, "validate_analysis"), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run):
                scheduler.publish_batch(
                    root, root / "scheduler-state.json", state, repo=root,
                    machine="test-machine", agent="continuous-batch")

            self.assertEqual(batch["phase"], "published")
            self.assertEqual(batch["publication"]["stage"], "merged")
            self.assertEqual(batch["publication"]["merge_commit"], "c" * 40)

    def test_merged_publication_recovers_after_ship_removed_its_worktree(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, batch, worktree = self.pushed_publication(root)
            batch["publication"]["stage"] = "prepared"
            worktree.rmdir()
            queried_from = []

            def run(command, **kwargs):
                self.assertEqual(command[:3], ["gh", "pr", "view"])
                queried_from.append(kwargs["cwd"])
                return SimpleNamespace(stdout=json.dumps({
                    "state": "MERGED",
                    "mergedAt": "2026-08-25T13:38:00Z",
                    "mergeCommit": {"oid": "d" * 40},
                }))

            with mock.patch.object(scheduler, "refresh_status", return_value={"complete_games": 1}), \
                    mock.patch.object(scheduler, "validate_analysis"), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run):
                scheduler.publish_batch(
                    root, root / "scheduler-state.json", state, repo=root,
                    machine="test-machine", agent="continuous-batch")

            self.assertEqual(queried_from, [scheduler.ROOT])
            self.assertEqual(batch["phase"], "published")
            self.assertEqual(batch["publication"]["stage"], "merged")
            self.assertEqual(batch["publication"]["merge_commit"], "d" * 40)

    def test_merge_race_after_lookup_is_recovered_before_propagating_ship_error(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, batch, _worktree = self.pushed_publication(root)
            views = iter((
                {"state": "OPEN", "mergedAt": None, "mergeCommit": None},
                {
                    "state": "MERGED",
                    "mergedAt": "2026-08-25T13:36:00Z",
                    "mergeCommit": {"oid": "b" * 40},
                },
            ))

            def run(command, **_kwargs):
                if command[:3] == ["gh", "pr", "view"]:
                    return SimpleNamespace(stdout=json.dumps(next(views)))
                self.assertEqual(command, [sys.executable, "tools/civvis_collab.py", "ship"])
                raise scheduler.SchedulerError("ship publication PR failed: PR is already merged")

            with mock.patch.object(scheduler, "refresh_status", return_value={"complete_games": 1}), \
                    mock.patch.object(scheduler, "validate_analysis"), \
                    mock.patch.object(scheduler, "run_checked", side_effect=run):
                scheduler.publish_batch(
                    root, root / "scheduler-state.json", state, repo=root,
                    machine="test-machine", agent="continuous-batch")

            self.assertEqual(batch["phase"], "published")
            self.assertEqual(batch["publication"]["stage"], "merged")
            self.assertEqual(batch["publication"]["merge_commit"], "b" * 40)


class PublicationMetadata(unittest.TestCase):
    def test_artifact_name_is_unique_to_the_batch_and_uses_seats(self):
        batch = scheduler.new_batch(1, 5_000, ident="rotation-a")
        batch["complete_seats"] = 30_000
        name = scheduler.reporting_filename(batch)
        self.assertIn("30000-total-seats", name)
        self.assertIn("rotation-a", name)

    def test_body_names_games_seats_and_the_pinned_binary(self):
        batch = scheduler.new_batch(1, 5_000, ident="rotation-b")
        batch.update({
            "complete_games": 5_000,
            "complete_seats": 30_000,
            "wins": 5_000,
            "source": {
                "commit": "a" * 40,
                "binary_sha256": "b" * 64,
            },
        })
        body = scheduler.publication_body(
            batch, "docs/gene_screens/example.json", machine="test-machine",
            agent="continuous-batch", coordinated="#1234", computer="Test Mac")
        self.assertIn("5,000 validated completed games / 30,000 seats / 5,000 wins", body)
        self.assertIn("changes no game mechanics", body)
        self.assertIn("reselects the default genome", body)
        self.assertIn("Computer: `Test Mac`", body)
        self.assertIn("Coordinated with: #1234", body)
        self.assertIn("publish validated 5,000-completed-game", body)
        self.assertIn("overwrite-guard: allow this report deliberately regenerates", body)

    def test_every_publication_reselects_defaults_from_its_latest_three_reports(self):
        self.assertEqual(
            scheduler.reporting_write_command("docs/gene_screens/example.json"),
            [
                sys.executable, "tools/genes.py", "write",
                "--reselect-deployment-defaults",
                "--reporting-batch",
                "docs/gene_screens/example.json",
            ],
        )

    def test_the_guard_knows_every_path_genes_py_write_records(self):
        """⭐ The allowed set is DERIVED from the writers, not restated here.

        `PUBLICATION_GENERATED_FILES` is deliberately explicit — a new
        generated artifact should be reviewed into it rather than swept into a
        publication. But the list lives in this file and the writing lives in
        `tools/genes.py`, so when #2584 taught `genes.py write` to record
        `tools/genome_cost_floor.json` the two drifted, and the fail-closed
        guard refused every continuous publication on every machine until
        someone noticed. Restating the tuple in a test cannot catch that: it is
        the same hand-maintenance a second time.

        This asks the writers what they write. `genes.py write` ends by writing
        these four paths and calling `genome_cost.record`, which writes its own;
        a sixth output added as a module path constant fails here instead of in
        a stalled tournament.
        """
        import genes
        import genome_cost

        recorded = {
            genes.LEDGER_JSON,
            genes.RANKING_MD,
            genes.EVIDENCE_MD,
            genes.REGISTRY_PATH,
            genome_cost.RECORD_JSON,
        }
        relative = {
            str(path.resolve().relative_to(genes.ROOT.resolve()))
            for path in recorded
        }
        self.assertEqual(
            relative,
            set(scheduler.PUBLICATION_GENERATED_FILES),
            "every path `genes.py write` records must be allowed through the "
            "publication guard, and nothing else",
        )

    def test_publication_claim_and_guard_include_every_generated_ranking_artifact(self):
        self.assertEqual(
            scheduler.PUBLICATION_GENERATED_FILES,
            (
                "docs/gene_ledger.json",
                "GENE_HEURISTIC_RANKING.md",
                "docs/GENE_RANKING_EVIDENCE.md",
                "src/ai/advanced/genes.rs",
                "tools/genome_cost_floor.json",
            ),
        )
        batch = scheduler.new_batch(1, 1, ident="publication-artifacts")
        batch.update({
            "complete_games": 1,
            "complete_seats": 6,
            "wins": 1,
            "source": {"commit": "a" * 40, "binary_sha256": "b" * 64},
        })
        body = scheduler.publication_body(
            batch, "docs/gene_screens/example.json", machine="test-machine",
            agent="continuous-batch", coordinated="none", computer="Test Mac")
        self.assertIn("`docs/GENE_RANKING_EVIDENCE.md`", body)
        self.assertIn("`src/ai/advanced/genes.rs`", body)
        self.assertIn("`tools/gene_fire_waivers.json`", body)


class PublicationTiming(unittest.TestCase):
    def test_freeze_persists_the_completion_timestamp_before_analyzer_work(self):
        """A failed analyzer retry must not inflate the batch's reported rate."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(3_000, 1)
            batch = state["current"]
            batch["created_at"] = "2026-08-25T10:00:00Z"
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            complete_rows(root / batch["rows"], seed_first=reservation["seed_first"],
                          target_games=1, complete_games=1)
            batch["source"] = {"binary": "/not-run", "worktree": str(root)}
            state_path = root / "scheduler-state.json"

            with mock.patch.object(scheduler, "utc_now", return_value="2026-08-25T10:01:00Z"), \
                    mock.patch.object(scheduler, "run_checked",
                                      side_effect=scheduler.SchedulerError("analyzer failed")):
                with self.assertRaisesRegex(scheduler.SchedulerError, "analyzer failed"):
                    scheduler.freeze_analysis(root, state, state_pathname=state_path)

            self.assertEqual(batch["completed_at"], "2026-08-25T10:01:00Z")
            persisted = json.loads(state_path.read_text(encoding="utf-8"))
            self.assertEqual(persisted["current"]["completed_at"], "2026-08-25T10:01:00Z")

    def test_reporting_artifact_carries_exact_whole_batch_rate_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "analysis.json"
            target = root / "report.json"
            source.write_text(json.dumps({
                "kind": "gene_screen_analysis", "games": 3_000, "seats": 18_000,
            }), encoding="utf-8")
            batch = scheduler.new_batch(1, 3_000, ident="timed")
            batch.update({
                "complete_games": 3_000,
                "created_at": "2026-08-25T10:00:00Z",
                "completed_at": "2026-08-25T10:25:00Z",
            })

            scheduler.write_reporting_artifact(source, target, batch)

            self.assertNotIn("continuous_batch_timing",
                             json.loads(source.read_text(encoding="utf-8")))
            self.assertEqual(json.loads(target.read_text(encoding="utf-8"))[
                "continuous_batch_timing"], {
                    "schema": "continuous_batch_timing/v1",
                    "started_at": "2026-08-25T10:00:00Z",
                    "completed_at": "2026-08-25T10:25:00Z",
                    "elapsed_seconds": 1_500,
                    "completed_games": 3_000,
                })


class DeadlineRotation(unittest.TestCase):
    DEADLINE = scheduler.parse_utc_timestamp("2026-08-26T12:00:00Z", name="test deadline")

    def test_new_deadline_state_persists_the_deadline_and_successor_goal(self):
        state = scheduler.new_state(10, 1_000_000, deadline_at=self.DEADLINE, next_goal_games=3_000)
        deadline = state["current"]["deadline"]
        self.assertEqual(deadline, {
            "schema": scheduler.CONTINUOUS_BATCH_DEADLINE_SCHEMA,
            "deadline_at": "2026-08-26T12:00:00Z",
            "next_goal_completed_games": 3_000,
        })
        scheduler.validate_state(state)

    def test_deadline_snapshot_discards_only_a_terminal_partial_game_group(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            raw = root / "rows-continuous.jsonl"
            target = root / "rows-deadline-cutoff.jsonl"
            complete_rows(raw, seed_first=700, target_games=100, complete_games=2)
            raw_bytes = raw.read_bytes()
            # The process can be interrupted between two rows of its final
            # six-seat write.  The raw ledger stays intact for audit.
            raw.write_bytes(raw_bytes + b'{"kind":"game","seed":702,"arm":0,"seat":0')

            status, dropped = scheduler.snapshot_complete_prefix(raw, target)

            self.assertEqual(status["complete_games"], 2)
            self.assertEqual(status["complete_seats"], 12)
            self.assertEqual(dropped, 1)
            self.assertEqual(raw.read_bytes(), raw_bytes + b'{"kind":"game","seed":702,"arm":0,"seat":0')
            self.assertEqual(scheduler.summarize(target)["complete_games"], 2)

    def test_sealed_deadline_uses_actual_games_but_keeps_raw_target_auditable(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(900, 100, deadline_at=self.DEADLINE, next_goal_games=3_000)
            batch = state["current"]
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            raw = root / batch["rows"]
            complete_rows(raw, seed_first=reservation["seed_first"], target_games=100, complete_games=2)
            raw_bytes = raw.read_bytes()
            raw.write_bytes(raw_bytes + b'{"kind":"game","seed":902,"arm":0,"seat":0')
            state_path = root / "scheduler-state.json"

            with mock.patch.object(scheduler, "freeze_analysis") as freeze:
                scheduler.seal_deadline_cutoff(
                    root, state_path, state, stopped_at="2026-08-26T12:00:00Z")

            self.assertEqual(state["goal_completed_games"], 2)
            self.assertEqual(batch["goal_completed_games"], 2)
            self.assertEqual(batch["raw_rows"], batch["directory"] + "/rows-continuous.jsonl")
            self.assertEqual(batch["deadline"]["original_goal_completed_games"], 100)
            self.assertEqual(batch["deadline"]["actual_completed_games"], 2)
            self.assertEqual(batch["deadline"]["dropped_trailing_records"], 1)
            self.assertEqual(scheduler.refresh_status(root, state)["complete_games"], 2)
            self.assertTrue(freeze.called)

    def test_rotation_switches_to_the_deadline_successor_goal_once_published(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(1_100, 100, deadline_at=self.DEADLINE, next_goal_games=3_000)
            batch = state["current"]
            state["goal_completed_games"] = 2
            batch.update({"goal_completed_games": 2, "phase": "published", "complete_games": 2,
                          "complete_seats": 12, "wins": 2, "source": {"commit": "a"},
                          "publication": {"stage": "merged"}})
            batch["deadline"].update({
                "cutoff_at": "2026-08-26T12:00:00Z",
                "original_goal_completed_games": 100,
                "actual_completed_games": 2,
                "raw_rows": batch["rows"],
                "raw_rows_sha256": "a" * 64,
                "frozen_rows": batch["rows"],
                "dropped_trailing_records": 0,
            })

            scheduler.rotate(root / "scheduler-state.json", state)

            self.assertEqual(state["goal_completed_games"], 3_000)
            self.assertEqual(state["current"]["goal_completed_games"], 3_000)
            self.assertNotIn("deadline", state["current"])
            self.assertEqual(state["history"][0]["deadline"]["actual_completed_games"], 2)

    def test_tick_seals_an_already_due_deadline_before_reading_live_rows(self):
        state = scheduler.new_state(1_200, 100, deadline_at=self.DEADLINE, next_goal_games=3_000)
        with mock.patch.object(scheduler, "seal_deadline_cutoff") as seal, \
                mock.patch.object(scheduler.dt, "datetime", wraps=scheduler.dt.datetime) as clock:
            clock.now.return_value = scheduler.parse_utc_timestamp(
                "2026-08-26T12:00:01Z", name="after deadline")
            self.assertEqual(scheduler.tick(
                Path("/state"), Path("/state/scheduler-state.json"), state,
                repo=Path("/repo"), jobs=1, machine="machine", agent="agent", publish=True),
                "frozen_deadline")
        self.assertTrue(seal.called)

    def test_restarted_scheduler_waits_for_its_persisted_live_child(self):
        state = scheduler.new_state(1_300, 10)
        reservation = scheduler.reserve_segment(state, scheduler.empty_status())
        reservation.update({"launch_state": "running", "pid": 4242, "process_group": 4242})
        with mock.patch.object(scheduler, "process_is_alive", return_value=True), \
                mock.patch.object(scheduler, "refresh_status") as status:
            outcome = scheduler.tick(
                Path("/state"), Path("/state/scheduler-state.json"), state,
                repo=Path("/repo"), jobs=1, machine="machine", agent="agent", publish=True)
        self.assertEqual(outcome, "active_segment")
        status.assert_not_called()


class OperatorCommands(unittest.TestCase):
    """cut / status / publish never need the daemon's lock, a restart, or a hand-edited state."""

    DEADLINE = scheduler.parse_utc_timestamp("2026-08-26T12:00:00Z", name="test deadline")

    @staticmethod
    def running_state(root: Path, *, seed: int = 2_000, goal: int = 100):
        state = scheduler.new_state(seed, goal)
        scheduler.reserve_segment(state, scheduler.empty_status())
        path = root / "scheduler-state.json"
        scheduler.atomic_json(path, state)
        return state, path

    @staticmethod
    def consumed_cut_deadline(batch: dict) -> None:
        batch["deadline"] = {
            "schema": scheduler.CONTINUOUS_BATCH_DEADLINE_SCHEMA,
            "deadline_at": "2026-08-26T12:00:00Z",
            "next_goal_completed_games": 100,
            "requested_via": scheduler.DEADLINE_REQUESTED_VIA_CUT,
            "cutoff_at": "2026-08-26T12:00:03Z",
            "original_goal_completed_games": 100,
            "actual_completed_games": 2,
            "raw_rows": batch["rows"],
            "raw_rows_sha256": "a" * 64,
            "frozen_rows": batch["rows"],
            "dropped_trailing_records": 0,
        }
        batch["raw_rows"] = batch["rows"]

    def test_cut_request_becomes_the_running_batch_deadline(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, path = self.running_state(root)
            request = scheduler.write_cut_request(root, deadline_at=self.DEADLINE, note="table refresh")
            with self.assertRaises(scheduler.SchedulerError):
                scheduler.write_cut_request(root, deadline_at=self.DEADLINE)

            self.assertEqual(scheduler.adopt_cut_request(root, path, state), self.DEADLINE)

            deadline = state["current"]["deadline"]
            self.assertEqual(deadline["schema"], scheduler.CONTINUOUS_BATCH_DEADLINE_SCHEMA)
            self.assertEqual(deadline["deadline_at"], "2026-08-26T12:00:00Z")
            self.assertEqual(deadline["next_goal_completed_games"], 100)
            self.assertEqual(deadline["requested_via"], scheduler.DEADLINE_REQUESTED_VIA_CUT)
            self.assertEqual(deadline["note"], "table refresh")
            scheduler.validate_state(state)
            self.assertEqual(scheduler.batch_deadline(state["current"]), self.DEADLINE)
            # Durable before the request file moves into the batch's audit directory.
            persisted = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(persisted["current"]["deadline"]["deadline_at"], "2026-08-26T12:00:00Z")
            self.assertFalse(request.exists())
            self.assertTrue((root / state["current"]["directory"] / scheduler.CUT_REQUEST_NAME).is_file())
            self.assertIsNone(scheduler.adopt_cut_request(root, path, state))

    def test_cut_request_that_cannot_apply_is_set_aside_with_its_reason(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, path = self.running_state(root)
            state["current"]["phase"] = "frozen"
            scheduler.write_cut_request(root, deadline_at=self.DEADLINE)

            self.assertIsNone(scheduler.adopt_cut_request(root, path, state))

            self.assertNotIn("deadline", state["current"])
            self.assertFalse(scheduler.cut_request_path(root).exists())
            rejected = sorted(root.glob("cut-request.rejected-*.json"))
            self.assertEqual(len(rejected), 1)
            record = json.loads(rejected[0].read_text(encoding="utf-8"))
            self.assertIn("not running", record["rejected"])
            self.assertIn(scheduler.CUT_REQUEST_SCHEMA, record["request"])

            state["current"]["phase"] = "running"
            state["current"]["deadline"] = {
                "schema": scheduler.CONTINUOUS_BATCH_DEADLINE_SCHEMA,
                "deadline_at": "2026-08-26T13:00:00Z",
                "next_goal_completed_games": 100,
            }
            scheduler.write_cut_request(root, deadline_at=self.DEADLINE)
            self.assertIsNone(scheduler.adopt_cut_request(root, path, state))
            self.assertEqual(state["current"]["deadline"]["deadline_at"], "2026-08-26T13:00:00Z")
            self.assertEqual(len(list(root.glob("cut-request.rejected-*.json"))), 2)

    def test_restart_without_deadline_flags_keeps_a_cut_deadline(self):
        state = scheduler.new_state(2_100, 100)
        state["current"]["deadline"] = {
            "schema": scheduler.CONTINUOUS_BATCH_DEADLINE_SCHEMA,
            "deadline_at": "2026-08-26T12:00:00Z",
            "next_goal_completed_games": 100,
            "requested_via": scheduler.DEADLINE_REQUESTED_VIA_CUT,
        }
        scheduler.verify_deadline_invocation(state, deadline_at=None, next_goal_games=None)
        del state["current"]["deadline"]["requested_via"]
        with self.assertRaises(scheduler.SchedulerError):
            scheduler.verify_deadline_invocation(state, deadline_at=None, next_goal_games=None)

    def test_unchanged_service_arguments_load_a_state_frozen_by_a_cut(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, path = self.running_state(root, seed=2_200)
            batch = state["current"]
            self.consumed_cut_deadline(batch)
            batch.update({"phase": "frozen", "goal_completed_games": 2, "complete_games": 2,
                          "complete_seats": 12, "wins": 2, "frozen_at": "2026-08-26T12:00:03Z"})
            state["goal_completed_games"] = 2
            scheduler.validate_state(state)
            scheduler.atomic_json(path, state)

            loaded = scheduler.load_state(path, seed_floor=None, goal_games=100)

            self.assertEqual(loaded["goal_completed_games"], 2)
            with self.assertRaises(scheduler.SchedulerError):
                scheduler.load_state(path, seed_floor=None, goal_games=3_000)

    def test_status_reads_while_another_process_holds_the_lock(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.running_state(root)
            with (root / "scheduler.lock").open("a+") as lock:
                scheduler.fcntl.flock(lock.fileno(), scheduler.fcntl.LOCK_EX | scheduler.fcntl.LOCK_NB)
                import io
                from contextlib import redirect_stdout
                buffer = io.StringIO()
                with redirect_stdout(buffer):
                    code = scheduler.main(["status", "--state-dir", str(root), "--json"])
            self.assertEqual(code, 0)
            report = json.loads(buffer.getvalue())
            self.assertEqual(report["schema"], "continuous_batch_status/v1")
            self.assertEqual(report["phase"], "running")
            self.assertTrue(report["scheduler_running"])
            self.assertEqual(report["remaining_games"], 100)
            self.assertIsNone(report["games_per_hour"])
            self.assertFalse(report["cut_request_pending"])

    def test_cut_command_only_targets_a_running_batch(self):
        import io
        from contextlib import redirect_stderr, redirect_stdout
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, path = self.running_state(root)
            with redirect_stdout(io.StringIO()):
                self.assertEqual(scheduler.main(["cut", "--state-dir", str(root), "--in", "0"]), 0)
            request = json.loads(scheduler.cut_request_path(root).read_text(encoding="utf-8"))
            self.assertEqual(request["schema"], scheduler.CUT_REQUEST_SCHEMA)
            scheduler.parse_utc_timestamp(request["deadline_at"], name="request deadline")
            scheduler.cut_request_path(root).unlink()

            state["current"]["phase"] = "frozen"
            scheduler.atomic_json(path, state)
            errors = io.StringIO()
            with redirect_stderr(errors):
                self.assertEqual(scheduler.main(["cut", "--state-dir", str(root)]), 2)
            self.assertIn("only a running batch can be cut", errors.getvalue())
            self.assertFalse(scheduler.cut_request_path(root).exists())

    def test_publish_command_refuses_a_running_batch(self):
        import io
        from contextlib import redirect_stderr
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.running_state(root)
            errors = io.StringIO()
            with redirect_stderr(errors):
                code = scheduler.main([
                    "publish", "--state-dir", str(root), "--repo", str(scheduler.ROOT),
                    "--goal-games", "100"])
            self.assertEqual(code, 2)
            self.assertIn("publish applies to a frozen batch", errors.getvalue())

    def test_run_segment_adopts_a_cut_request_and_stops_its_child(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state, path = self.running_state(root, seed=2_300)
            batch = state["current"]
            binary = root / "fake-gene-screen"
            binary.write_text("#!/bin/sh\nexec sleep 30\n", encoding="utf-8")
            binary.chmod(0o755)
            batch["source"] = {"binary": str(binary), "worktree": str(root), "commit": "a"}
            reservation = batch["reservations"][0]
            scheduler.write_cut_request(root, deadline_at=dt.datetime.now(dt.timezone.utc))

            result = scheduler.run_segment(root, path, state, batch, reservation, jobs=1)

            self.assertTrue(result.stopped_at_deadline)
            self.assertIsNotNone(reservation["deadline_stopped_at"])
            self.assertEqual(reservation["launch_state"], "finished")
            self.assertEqual(reservation["complete_games_at_start"], 0)
            self.assertEqual(batch["deadline"]["requested_via"], scheduler.DEADLINE_REQUESTED_VIA_CUT)
            self.assertIsNotNone(scheduler.batch_deadline(batch))
            self.assertTrue((root / batch["directory"] / scheduler.CUT_REQUEST_NAME).is_file())


    def test_repeated_tick_outcomes_are_logged_once_with_their_count(self):
        log = scheduler.OutcomeLog()
        self.assertTrue(log.note("active_segment").endswith(" active_segment"))
        self.assertIsNone(log.note("active_segment"))
        self.assertIsNone(log.note("active_segment"))
        self.assertTrue(log.note("frozen_deadline").endswith(
            " frozen_deadline (previous outcome repeated 2 more times)"))
        self.assertTrue(log.note("awaiting_publication").endswith(" awaiting_publication"))


class AutomationRecovery(unittest.TestCase):
    def test_stop_after_publish_never_rotates_even_after_restart(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / ".git").mkdir()
            state = scheduler.new_state(1000, 1)
            state["current"]["phase"] = "frozen"
            path = root / "scheduler-state.json"
            scheduler.atomic_json(path, state)

            def publish(_root, state_path, current, **kwargs):
                self.assertTrue(kwargs["publish"])
                current["current"]["phase"] = "published"
                scheduler.atomic_json(state_path, current)
                return "published"

            args = ["run", "--state-dir", str(root), "--repo", str(root),
                    "--goal-games", "1", "--machine", "test", "--stop-after-publish"]
            with mock.patch.object(scheduler, "tick", side_effect=publish) as tick, \
                    mock.patch.object(scheduler, "checkpoint_progress"):
                self.assertEqual(scheduler.main(args), 0)
                self.assertEqual(tick.call_count, 1)
                tick.reset_mock()
                self.assertEqual(scheduler.main(args), 0)
                tick.assert_not_called()

    def test_running_segment_persists_progress_before_child_exits(self):
        import subprocess
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(1000, 2)
            batch = state["current"]
            reservation = scheduler.reserve_segment(state, scheduler.empty_status())
            binary = root / "fake-binary"
            binary.touch()
            batch["source"] = {"binary": str(binary), "worktree": str(root), "commit": "a"}
            complete_rows(root / batch["rows"], seed_first=1000, target_games=2, complete_games=1)
            child = mock.Mock(pid=12345)
            calls = 0

            def wait(**kwargs):
                nonlocal calls
                calls += 1
                if calls == 1:
                    raise subprocess.TimeoutExpired("fake", 1)
                self.assertEqual(json.loads((root / "status.json").read_text())["complete_games"], 1)
                return 0

            child.wait.side_effect = wait
            with mock.patch.object(scheduler.subprocess, "Popen", return_value=child):
                scheduler.run_segment(root, root / "state.json", state, batch, reservation, jobs=1)
            self.assertEqual(calls, 2)

    def test_fingerprint_includes_untracked_report_and_source_diff(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            report = root / "report.json"
            report.write_text('{"games":1}')
            with mock.patch.object(scheduler, "git_output", side_effect=["head", "diff"]):
                first = scheduler.publication_validation_fingerprint(root, "report.json")
            report.write_text('{"games":2}')
            with mock.patch.object(scheduler, "git_output", side_effect=["head", "diff"]):
                second = scheduler.publication_validation_fingerprint(root, "report.json")
            with mock.patch.object(scheduler, "git_output", side_effect=["head", "changed"]):
                third = scheduler.publication_validation_fingerprint(root, "report.json")
            self.assertEqual(len({first, second, third}), 3)

    def test_fingerprint_survives_a_fetch_that_moves_origin_main(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "report.json").write_text('{"games":1}')
            asked: list[tuple[str, ...]] = []

            def git(_repo, *args):
                asked.append(args)
                return "value"

            with mock.patch.object(scheduler, "git_output", side_effect=git):
                scheduler.publication_validation_fingerprint(root, "report.json")
            self.assertFalse(any("origin/main" in args for args in asked), asked)

    def test_a_failure_that_passes_alone_is_reported_not_fatal(self):
        import subprocess
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log = root / "logs" / "rust-tests.log"
            full = subprocess.CompletedProcess(
                [], 101, "test server::tests::slow_page ... FAILED\ntest result: FAILED. 9 passed; 1 failed\n",
                "error: 1 target failed\n")
            alone = subprocess.CompletedProcess([], 0, "test result: ok. 1 passed; 0 failed\n", "")
            with mock.patch.object(scheduler.subprocess, "run", side_effect=[full, alone]) as run:
                retried = scheduler.run_rust_tests(root, description="tests",
                                                   env={"RUST_TEST_THREADS": "1"}, log_path=log)
            self.assertEqual(retried, ["server::tests::slow_page"])
            first, second = (call.args[0] for call in run.call_args_list)
            self.assertIn("--no-fail-fast", first)
            self.assertEqual(second[-4:], ["server::tests::slow_page", "--", "--exact", "--test-threads=1"])
            self.assertIn("isolated retry: server::tests::slow_page", log.read_text())

    def test_a_failure_that_fails_alone_still_fails_the_gate(self):
        import subprocess
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            full = subprocess.CompletedProcess([], 101, "test tests::real_bug ... FAILED\n", "")
            alone = subprocess.CompletedProcess([], 101, "test tests::real_bug ... FAILED\n", "")
            filtered_out = subprocess.CompletedProcess([], 0, "test result: ok. 0 passed; 0 failed\n", "")
            for retry in (alone, filtered_out):
                with mock.patch.object(scheduler.subprocess, "run", side_effect=[full, retry]):
                    with self.assertRaisesRegex(scheduler.SchedulerError, "again on its own"):
                        scheduler.run_rust_tests(root, description="tests", env={},
                                                 log_path=root / "rust.log")

    def test_a_broken_build_or_many_failures_is_never_retried(self):
        import subprocess
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            compile_error = subprocess.CompletedProcess([], 101, "", "error[E0425]: cannot find value\n")
            many = subprocess.CompletedProcess(
                [], 101, "".join(f"test tests::t{n} ... FAILED\n" for n in range(4)), "")
            for result in (compile_error, many):
                with mock.patch.object(scheduler.subprocess, "run", return_value=result) as run:
                    with self.assertRaisesRegex(scheduler.SchedulerError, "full log"):
                        scheduler.run_rust_tests(root, description="tests", env={},
                                                 log_path=root / "rust.log")
                self.assertEqual(run.call_count, 1)

    def test_rewriting_identical_generated_bytes_keeps_their_mtime(self):
        import os
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "same.rs").write_text("pub const A: u8 = 1;\n")
            (root / "moved.rs").write_text("pub const B: u8 = 1;\n")
            for name in ("same.rs", "moved.rs"):
                os.utime(root / name, ns=(1_000_000_000, 1_000_000_000))
            snapshots = scheduler.file_snapshots(root, ["same.rs", "moved.rs", "absent.rs"])
            self.assertEqual(set(snapshots), {"same.rs", "moved.rs"})
            (root / "same.rs").write_text("pub const A: u8 = 1;\n")
            (root / "moved.rs").write_text("pub const B: u8 = 2;\n")
            scheduler.restore_unchanged_mtimes(root, snapshots)
            self.assertEqual((root / "same.rs").stat().st_mtime_ns, 1_000_000_000)
            self.assertNotEqual((root / "moved.rs").stat().st_mtime_ns, 1_000_000_000)

    def test_publication_body_names_a_test_that_needed_the_retry(self):
        batch = {
            "id": "b", "complete_games": 1, "complete_seats": 6, "wins": 1,
            "source": {"commit": "c", "binary_sha256": "s"},
            "publication": {"validation_retried": ["server::tests::slow_page"]},
        }
        body = scheduler.publication_body(batch, "r.json", machine="m", agent="a",
                                          coordinated="none", computer="MacBook Pro")
        self.assertIn("`cargo test --profile ci --locked` — passed only on an isolated retry "
                      "under host load: `server::tests::slow_page`", body)
        batch["publication"] = {}
        body = scheduler.publication_body(batch, "r.json", machine="m", agent="a",
                                          coordinated="none", computer="MacBook Pro")
        self.assertIn("- [x] `cargo test --profile ci --locked`\n", body)

    def test_failure_keeps_assertion_stdout_and_stderr_and_full_log(self):
        import subprocess
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log = root / "checks" / "rust.log"
            result = subprocess.CompletedProcess([], 101, "assertion failed: expected winner\n",
                                                 "error: test failed\n")
            with mock.patch.object(scheduler.subprocess, "run", return_value=result):
                with self.assertRaises(scheduler.SchedulerError) as caught:
                    scheduler.run_checked(["cargo", "test"], cwd=root,
                                          description="tests", log_path=log)
            self.assertIn("expected winner", str(caught.exception))
            self.assertIn("error: test failed", str(caught.exception))
            self.assertIn(str(log), str(caught.exception))
            self.assertIn("assertion failed", log.read_text())

    def test_validation_retry_skips_only_unchanged_successes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(1000, 1)
            path = root / "scheduler-state.json"
            calls = []

            def check(command, **kwargs):
                calls.append(kwargs["description"])

            with mock.patch.object(scheduler, "publication_validation_fingerprint", return_value="first"), \
                    mock.patch.object(scheduler, "run_checked", side_effect=check), \
                    mock.patch.object(scheduler, "run_rust_tests",
                                      side_effect=scheduler.SchedulerError("test failure")):
                with self.assertRaises(scheduler.SchedulerError):
                    scheduler.validate_publication(root, path, state, root, "report.json")
            persisted = json.loads(path.read_text())
            self.assertEqual(set(persisted["current"]["publication"]["validation"]),
                             {"ranking", "ranking-tests"})
            with mock.patch.object(scheduler, "publication_validation_fingerprint", return_value="first"), \
                    mock.patch.object(scheduler, "run_checked") as run, \
                    mock.patch.object(scheduler, "run_rust_tests", return_value=[]) as rust:
                scheduler.validate_publication(root, path, persisted, root, "report.json")
                self.assertEqual([call.kwargs["description"] for call in run.call_args_list],
                                 ["check publication diff"])
                self.assertEqual(rust.call_count, 1)
            with mock.patch.object(scheduler, "publication_validation_fingerprint", return_value="changed"), \
                    mock.patch.object(scheduler, "run_checked") as run, \
                    mock.patch.object(scheduler, "run_rust_tests", return_value=[]) as rust:
                scheduler.validate_publication(root, path, persisted, root, "report.json")
                self.assertEqual(run.call_count + rust.call_count, 4)

    def test_mutation_during_validation_cannot_be_reused(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(1000, 1)
            with mock.patch.object(scheduler, "publication_validation_fingerprint", side_effect=["before", "after"]), \
                    mock.patch.object(scheduler, "run_checked"), \
                    mock.patch.object(scheduler, "run_rust_tests", return_value=[]):
                with self.assertRaisesRegex(scheduler.SchedulerError, "changed during validation"):
                    scheduler.validate_publication(root, root / "state.json", state, root, "report.json")
            self.assertEqual(state["current"]["publication"]["validation"], {})

    def test_live_progress_preserves_last_good_count_during_partial_write(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = scheduler.new_state(1000, 2)
            scheduler.reserve_segment(state, scheduler.empty_status())
            rows = root / state["current"]["rows"]
            complete_rows(rows, seed_first=1000, target_games=2, complete_games=1)
            path = root / "state.json"
            scheduler.checkpoint_progress(root, path, state)
            self.assertEqual(json.loads((root / "status.json").read_text())["complete_games"], 1)
            good = rows.read_text()
            rows.write_text(good + '{"kind":')
            scheduler.checkpoint_progress(root, path, state)
            self.assertEqual(json.loads((root / "status.json").read_text())["complete_games"], 1)
            self.assertIsNotNone(json.loads((root / "progress-error.json").read_text())["error"])
            complete_rows(rows, seed_first=1000, target_games=2, complete_games=2)
            scheduler.checkpoint_progress(root, path, state)
            self.assertEqual(json.loads((root / "status.json").read_text())["complete_games"], 2)
            self.assertIsNone(json.loads((root / "progress-error.json").read_text())["error"])


if __name__ == "__main__":
    unittest.main()
