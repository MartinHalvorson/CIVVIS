#!/usr/bin/env python3

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock


MODULE_PATH = Path(__file__).with_name("civvis_match_machine.py")
SPEC = importlib.util.spec_from_file_location("civvis_match_machine", MODULE_PATH)
machine = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = machine
SPEC.loader.exec_module(machine)


class MatchMachineTests(unittest.TestCase):
    def test_game_contract_is_online_continents_free_for_all(self):
        command = machine.game_command(
            Path("/tmp/civvis"), Path("/tmp/league"), 42, 8870, visible=False
        )
        value = lambda flag: command[command.index(flag) + 1]
        self.assertEqual(value("--players"), "8")
        self.assertEqual((value("--width"), value("--height")), ("84", "54"))
        self.assertEqual(value("--city-states"), "12")
        self.assertEqual(value("--turns"), "250")
        self.assertEqual(value("--speed"), "online")
        self.assertEqual(value("--map"), "continents")
        self.assertNotIn("--teams", command)
        self.assertIn("--league-record", command)
        self.assertIn("--no-open", command)

    def test_game_contract_accepts_explicit_speed_and_turn_override(self):
        command = machine.game_command(
            Path("civvis"),
            Path("league"),
            1,
            2,
            visible=False,
            speed="standard",
            turns=600,
        )
        value = lambda flag: command[command.index(flag) + 1]
        self.assertEqual(value("--speed"), "standard")
        self.assertEqual(value("--turns"), "600")

    def test_headless_contract_carries_a_strategy_coverage_target(self):
        command = machine.game_command(
            Path("civvis"), Path("league"), 1, 2, visible=False, focus_strategy="g4-10"
        )
        self.assertEqual(command[command.index("--force-strategy") + 1], "g4-10")

    @staticmethod
    def schedule_subject(league, strategies):
        (league / "league.json").write_text(
            json.dumps({"strategies": strategies}), encoding="utf-8"
        )
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.league = league
        subject.strategy_cursor = 0
        subject.strategy_schedule = []
        subject.schedule_roster = frozenset()
        subject.event = mock.Mock()
        subject.refresh_strategy_schedule()
        return subject

    def test_strategy_schedule_covers_every_unretired_entry_and_repeats_top_eight(self):
        with tempfile.TemporaryDirectory() as directory:
            league = Path(directory) / "league"
            league.mkdir()
            # Equal measurement need (no eight-seat games, default RD), so the
            # full pass falls to name order; the exploitation tail is still
            # the rating order.
            subject = self.schedule_subject(
                league,
                [
                    {"name": "low", "rating": 1200},
                    {"name": "top", "rating": 1800},
                    {"name": "retired", "rating": 2000, "retired": True},
                ],
            )
            self.assertEqual(subject.strategy_schedule, ["low", "top", "top", "low"])
            self.assertEqual(
                [subject.next_focus_strategy() for _ in range(4)],
                ["low", "top", "top", "low"],
            )

    def test_strategy_schedule_serves_measurement_need_first(self):
        with tempfile.TemporaryDirectory() as directory:
            league = Path(directory) / "league"
            league.mkdir()
            subject = self.schedule_subject(
                league,
                [
                    {
                        "name": "veteran",
                        "rating": 1900,
                        "rd": 60,
                        "wins_by_table_size": {"8": {"games": 500, "wins": 80}},
                    },
                    {
                        "name": "settling",
                        "rating": 1600,
                        "rd": 200,
                        "wins_by_table_size": {"8": {"games": 12, "wins": 2}},
                    },
                    {"name": "newborn", "rating": 1500, "rd": 350},
                ],
            )
            # Fewest eight-seat games first; the strongest still headlines the
            # exploitation tail.
            self.assertEqual(
                subject.strategy_schedule[:3], ["newborn", "settling", "veteran"]
            )
            self.assertEqual(subject.strategy_schedule[3], "veteran")

    def test_focus_rebuilds_when_selection_changes_the_roster(self):
        with tempfile.TemporaryDirectory() as directory:
            league = Path(directory) / "league"
            league.mkdir()
            veteran = {
                "name": "veteran",
                "rating": 1900,
                "rd": 60,
                "wins_by_table_size": {"8": {"games": 500, "wins": 80}},
            }
            subject = self.schedule_subject(league, [veteran])
            self.assertEqual(subject.next_focus_strategy(), "veteran")
            # A rating-only refresh must not reset the cursor mid-cycle.
            (league / "league.json").write_text(
                json.dumps({"strategies": [dict(veteran, rating=1500)]}),
                encoding="utf-8",
            )
            self.assertEqual(subject.strategy_cursor, 1)
            subject.next_focus_strategy()
            self.assertEqual(subject.strategy_cursor, 2)
            # A birth changes the active-name set: rebuild, restart from the
            # most-needed entry — the newborn — and drop nobody.
            (league / "league.json").write_text(
                json.dumps(
                    {"strategies": [veteran, {"name": "g4032-56", "rd": 350}]}
                ),
                encoding="utf-8",
            )
            self.assertEqual(subject.next_focus_strategy(), "g4032-56")
            self.assertIn("veteran", subject.strategy_schedule)

    def test_focus_drops_a_retiree_at_the_next_launch(self):
        with tempfile.TemporaryDirectory() as directory:
            league = Path(directory) / "league"
            league.mkdir()
            subject = self.schedule_subject(
                league,
                [{"name": "keeper", "rd": 100}, {"name": "culled", "rd": 90}],
            )
            self.assertIn("culled", subject.strategy_schedule)
            (league / "league.json").write_text(
                json.dumps(
                    {
                        "strategies": [
                            {"name": "keeper", "rd": 100},
                            {"name": "culled", "rd": 90, "retired": True},
                        ]
                    }
                ),
                encoding="utf-8",
            )
            for _ in range(4):
                self.assertNotEqual(subject.next_focus_strategy(), "culled")

    def test_cli_derives_the_stock_turn_limit_for_the_selected_speed(self):
        online = machine.parse_args(["--watch-pid", "1"])
        standard = machine.parse_args(["--watch-pid", "1", "--speed", "standard"])
        self.assertEqual((online.speed, online.turns), ("online", 250))
        self.assertEqual((standard.speed, standard.turns), ("standard", 500))
        self.assertEqual(online.visible_pace, 0)

    def test_cli_accepts_a_timezone_aware_absolute_deadline(self):
        args = machine.parse_args(
            ["--watch-pid", "1", "--deadline-utc", "2026-08-02T15:50:48Z"]
        )

        self.assertEqual(args.deadline_utc.isoformat(), "2026-08-02T15:50:48+00:00")

    def test_first_visible_game_opens_browser_and_replacements_reuse_tab(self):
        visible = machine.game_command(Path("civvis"), Path("league"), 1, 2, visible=True)
        replacement = machine.game_command(
            Path("civvis"),
            Path("league"),
            2,
            2,
            visible=True,
            open_browser=False,
        )
        headless = machine.game_command(Path("civvis"), Path("league"), 1, 2, visible=False)
        self.assertNotIn("--no-open", visible)
        self.assertIn("--no-open", replacement)
        self.assertIn("--no-open", headless)

    def test_port_selection_reserves_the_base_port_for_visible_successors(self):
        with mock.patch.object(machine, "port_available", return_value=True):
            self.assertEqual(machine.game_port(8870, set(), visible=True), 8870)
        with mock.patch.object(machine, "port_available", return_value=False):
            self.assertIsNone(machine.game_port(8870, set(), visible=True))
        with mock.patch.object(machine, "free_port", return_value=8871) as choose:
            self.assertEqual(machine.game_port(8870, set(), visible=False), 8871)
            choose.assert_called_once_with(8871, set())

    def test_fill_slots_replaces_a_completed_visible_game(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.pending_revision = None
        subject.args = SimpleNamespace(limit=70, headless=8, max_processes=8)
        subject.games = []
        subject.visible_started = True
        subject.launch = mock.Mock()

        subject.fill_slots(machine.Resources(20, 20, 12, 0, False))

        subject.launch.assert_called_once_with(visible=True)

    def test_fill_slots_replaces_visible_game_while_head_build_is_pending(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.pending_revision = "new-head"
        subject.build_future = None
        subject.args = SimpleNamespace(limit=70, headless=8, max_processes=8)
        subject.games = []
        subject.launch = mock.Mock()

        subject.fill_slots(machine.Resources(20, 20, 12, 0, False))

        subject.launch.assert_called_once_with(visible=True)

    def test_fill_slots_sheds_headless_before_reserving_visible_at_hard_ceiling(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.pending_revision = "new-head"
        subject.build_future = None
        subject.args = SimpleNamespace(limit=70, headless=8, max_processes=8)
        game = SimpleNamespace(visible=False, paused=False, process=object(), seed=7)
        subject.games = [game]
        subject.event = mock.Mock()
        subject.launch = mock.Mock()

        with mock.patch.object(machine, "set_paused", return_value=True) as pause:
            subject.fill_slots(machine.Resources(70, 20, 12, 0, False))

        pause.assert_called_once_with(game.process, True)
        self.assertTrue(game.paused)
        subject.launch.assert_not_called()
        subject.event.assert_called_once()

    def test_fill_slots_reserves_visible_slot_before_recovery_margin(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.pending_revision = None
        subject.build_future = None
        subject.args = SimpleNamespace(limit=70, headless=8, max_processes=8)
        headless = SimpleNamespace(visible=False, paused=False, process=object(), seed=7)
        subject.games = [headless]
        subject.launch = mock.Mock()
        subject.event = mock.Mock()

        with mock.patch.object(machine, "set_paused", return_value=True):
            subject.fill_slots(machine.Resources(60, 20, 12, 0, False))

        self.assertTrue(headless.paused)
        subject.launch.assert_called_once_with(visible=True)

    def test_fill_slots_drains_headless_games_while_head_build_is_pending(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.pending_revision = "new-head"
        subject.build_future = None
        subject.args = SimpleNamespace(limit=70, headless=8, max_processes=8)
        subject.games = [SimpleNamespace(visible=True, paused=False)]
        subject.launch = mock.Mock()

        subject.fill_slots(machine.Resources(20, 20, 12, 0, False))

        subject.launch.assert_not_called()

    def test_capacity_reserves_cpu_for_background_build(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.args = SimpleNamespace(limit=70)

        self.assertTrue(
            subject.capacity_available(
                machine.Resources(20, 20, 12, 0, False), cpu_reservation=25
            )
        )
        self.assertFalse(
            subject.capacity_available(
                machine.Resources(41, 20, 12, 0, False), cpu_reservation=25
            )
        )

    def test_a_killed_run_is_not_recorded_as_a_finished_window(self):
        """The 2026-08-15 league outage, as a test.

        A SIGTERM from the dying agent session that had launched this in the
        background wrote `reason: "stopped"` beside a `deadline_utc` 17h51m in
        the future — byte-for-byte the record a window that ran to term
        writes. The league then sat idle for eight days because every reader
        of that file, human and machine, saw a completed job.
        """
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.deadline = machine.time.monotonic() + 3600
        subject.stop_signal = None
        unspent = max(0.0, subject.deadline - machine.time.monotonic())

        # Killed with most of its window unspent.
        subject.stop_signal = "SIGTERM"
        killed = subject.stop_cause(unspent)

        # The same clock, no signal: the loop simply came back.
        subject.stop_signal = None
        exited = subject.stop_cause(unspent)

        # The window genuinely ran out.
        ended = subject.stop_cause(0.0)

        self.assertEqual(killed, "stopped:sigterm")
        self.assertEqual(ended, "stopped:window_ended")
        self.assertNotEqual(killed, ended,
                            "a kill and a completed window must not write the "
                            "same reason; that is the whole defect")
        self.assertNotEqual(exited, killed)

    def test_a_signal_wins_over_the_clock(self):
        """A run killed in its last second is a kill, not a finished window."""
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.stop_signal = "SIGINT"
        self.assertEqual(subject.stop_cause(0.0), "stopped:sigint")

    def test_the_signal_handler_records_which_signal_arrived(self):
        """`stop()` discarded its argument, so nothing downstream could tell."""
        import signal as signal_module

        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.stopping = False
        subject.stop_signal = None
        with mock.patch.object(machine, "MatchMachine", return_value=subject), \
             mock.patch.object(machine, "parse_args", return_value=SimpleNamespace()), \
             mock.patch.object(machine.signal, "signal") as installed:
            with mock.patch.object(subject, "run", create=True, return_value=0):
                machine.main([])
        handlers = {call.args[0]: call.args[1] for call in installed.call_args_list}
        self.assertIn(signal_module.SIGTERM, handlers)
        handlers[signal_module.SIGTERM](signal_module.SIGTERM, None)
        self.assertTrue(subject.stopping)
        self.assertEqual(subject.stop_signal, "SIGTERM")

    def test_terminal_watcher_compares_the_original_process_identity(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.args = SimpleNamespace(watch_pid=42)
        subject.watch_identity = "started-at"

        with mock.patch.object(machine, "process_is_same", return_value=True):
            self.assertFalse(subject.watched_terminal_closed())
        with mock.patch.object(machine, "process_is_same", return_value=False):
            self.assertTrue(subject.watched_terminal_closed())

    def test_capacity_wait_returns_cleanly_when_the_watched_terminal_closes(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.args = SimpleNamespace(watch_pid=42, resource_log_interval=60, poll=1)
        subject.watch_identity = "started-at"
        subject.deadline = machine.time.monotonic() + 60
        subject.stopping = False
        subject.event = mock.Mock()
        subject.watched_terminal_closed = mock.Mock(return_value=True)

        self.assertIsNone(subject.wait_for_capacity("initial build"))

        self.assertTrue(subject.stopping)
        subject.event.assert_called_once_with("terminal_closed", watch_pid=42)

    def test_closed_terminal_before_initial_build_cleans_up_without_starting_work(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.args = SimpleNamespace(duration=60, watch_pid=42, headless=6, limit=70)
        subject.stopping = False
        subject.stop_signal = None
        subject.deadline = machine.time.monotonic() + 60
        subject.games = []
        subject.caffeinate = None
        subject.build_future = None
        subject.build_executor = mock.Mock()
        subject.completed = 0
        subject.failed = 0
        subject.event = mock.Mock()
        subject.keep_awake = mock.Mock()
        subject.ensure_source = mock.Mock()
        subject.refresh_ranking = mock.Mock()
        subject.persist = mock.Mock()
        subject.watched_terminal_closed = mock.Mock(return_value=True)

        self.assertEqual(subject.run(), 0)

        subject.keep_awake.assert_not_called()
        subject.ensure_source.assert_not_called()
        subject.build_executor.shutdown.assert_called_once_with(wait=True)
        subject.event.assert_any_call("terminal_closed", watch_pid=42)

    def test_expired_window_before_initial_build_cleans_up_without_starting_work(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.args = SimpleNamespace(duration=60, watch_pid=42, headless=6, limit=70)
        subject.deadline = 10.0
        subject.stopping = False
        subject.stop_signal = None
        subject.games = []
        subject.caffeinate = None
        subject.build_future = None
        subject.build_executor = mock.Mock()
        subject.completed = 0
        subject.failed = 0
        subject.event = mock.Mock()
        subject.keep_awake = mock.Mock()
        subject.ensure_source = mock.Mock()
        subject.refresh_ranking = mock.Mock()
        subject.persist = mock.Mock()
        subject.watched_terminal_closed = mock.Mock(return_value=False)

        with mock.patch.object(machine.time, "monotonic", return_value=10.0):
            self.assertEqual(subject.run(), 0)

        subject.keep_awake.assert_not_called()
        subject.ensure_source.assert_not_called()
        subject.build_executor.shutdown.assert_called_once_with(wait=True)
        subject.event.assert_any_call("operator_window_ended", purpose="startup")

    def test_completed_background_build_is_activated_on_operator_thread(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        future = machine.Future()
        promoted = Path("/tmp/civvis-new")
        future.set_result(promoted)
        subject.build_future = future
        subject.build_revision = "new-head"
        subject.activate_build = mock.Mock()

        subject.poll_head_build(100.0)

        subject.activate_build.assert_called_once_with("new-head", promoted)
        self.assertIsNone(subject.build_future)
        self.assertIsNone(subject.build_revision)

    def test_long_work_pauses_and_resumes_as_host_pressure_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.runtime = root
            subject.args = SimpleNamespace(limit=70, poll=1)
            subject.deadline = machine.time.monotonic() + 60
            subject.stopping = False
            # A HEAD build must get a recovery slot even when the game
            # governor has shed part of the fleet. Requiring every game to
            # resume first can starve promotion under steady moderate load.
            subject.games = [SimpleNamespace(paused=True)]
            subject.watched_terminal_closed = mock.Mock(return_value=False)
            subject.event = mock.Mock()
            process = mock.Mock(pid=1234, returncode=0)
            process.poll.side_effect = [None, None, None, 0]
            samples = [
                machine.Resources(60, 20, 12, 0, False),
                machine.Resources(20, 20, 12, 0, False),
            ]

            with mock.patch.object(
                machine.subprocess, "Popen", return_value=process
            ) as launch, mock.patch.object(
                machine, "resources", side_effect=samples
            ), mock.patch.object(
                machine, "set_paused", return_value=True
            ) as pause, mock.patch.object(machine.time, "sleep"):
                result = subject.resource_capped_command(
                    "civvis",
                    "validate",
                    cwd=root,
                    env=None,
                    timeout=30,
                    purpose="validation",
                    log_path=root / "build.log",
                )

            self.assertEqual(result.returncode, 0)
            self.assertEqual(
                pause.call_args_list,
                [mock.call(process, True), mock.call(process, False)],
            )
            self.assertEqual(
                [call.args[0] for call in subject.event.call_args_list],
                ["work_paused_for_resources", "work_resumed"],
            )
            self.assertTrue(launch.call_args.kwargs["start_new_session"])

    def test_validation_is_stopped_before_its_first_resource_sample(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.runtime = root
            subject.args = SimpleNamespace(limit=70, poll=1)
            subject.deadline = machine.time.monotonic() + 60
            subject.stopping = False
            subject.watched_terminal_closed = mock.Mock(return_value=False)
            subject.event = mock.Mock()
            process = mock.Mock(pid=1234, returncode=0)
            process.poll.side_effect = [None, None, 0]
            order = []

            def pause_process(candidate, paused):
                order.append(("pause", paused))
                return True

            def sample_resources(runtime):
                order.append(("sample", runtime))
                return machine.Resources(20, 20, 12, 0, False)

            with mock.patch.object(
                machine.subprocess, "Popen", return_value=process
            ), mock.patch.object(
                machine, "resources", side_effect=sample_resources
            ), mock.patch.object(
                machine, "set_paused", side_effect=pause_process
            ), mock.patch.object(machine.time, "sleep"):
                result = subject.resource_capped_command(
                    "civvis",
                    "validate",
                    cwd=root,
                    env=None,
                    timeout=30,
                    purpose="validation",
                    log_path=root / "build.log",
                )

            self.assertEqual(result.returncode, 0)
            self.assertEqual(
                order,
                [("pause", True), ("sample", root), ("pause", False)],
            )

    def test_build_cpu_duty_cycle_bounds_a_saturating_worker(self):
        self.assertEqual(machine.build_cpu_duty_cycle(None, 60), 0.0)
        self.assertEqual(machine.build_cpu_duty_cycle(43, 60), 0.0)
        self.assertEqual(machine.build_cpu_duty_cycle(10, 60), 0.35)

        duty = machine.build_cpu_duty_cycle(30, 60)
        worst_case_average = 30 + (100 - 30) * duty
        self.assertAlmostEqual(worst_case_average, 43.0)

    def test_build_is_stopped_before_resource_sample_and_runs_in_bounded_pulses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.runtime = root
            subject.args = SimpleNamespace(limit=60, poll=1)
            subject.deadline = machine.time.monotonic() + 60
            subject.stopping = False
            subject.watched_terminal_closed = mock.Mock(return_value=False)
            subject.event = mock.Mock()
            process = mock.Mock(pid=1234, returncode=0)
            process.poll.side_effect = [None, None, None, 0]

            with mock.patch.object(
                machine.subprocess, "Popen", return_value=process
            ), mock.patch.object(
                machine, "resources", return_value=machine.Resources(20, 20, 12, 0, False)
            ), mock.patch.object(
                machine, "set_paused", return_value=True
            ) as pause, mock.patch.object(machine.time, "sleep") as sleep:
                result = subject.resource_capped_command(
                    "cargo",
                    "build",
                    cwd=root,
                    env=None,
                    timeout=30,
                    purpose="build",
                    log_path=root / "build.log",
                )

            self.assertEqual(result.returncode, 0)
            self.assertEqual(
                pause.call_args_list,
                [
                    mock.call(process, True),
                    mock.call(process, False),
                    mock.call(process, True),
                ],
            )
            duty = machine.build_cpu_duty_cycle(20, 60)
            self.assertEqual(
                sleep.call_args_list,
                [
                    mock.call(machine.BUILD_CPU_DUTY_CYCLE_SECONDS * duty),
                    mock.call(machine.BUILD_CPU_DUTY_CYCLE_SECONDS * (1.0 - duty)),
                ],
            )
            subject.event.assert_called_once_with(
                "work_cpu_throttle_started",
                purpose="build",
                pid=1234,
                cycle_seconds=machine.BUILD_CPU_DUTY_CYCLE_SECONDS,
                max_duty=machine.BUILD_CPU_MAX_DUTY,
            )

    def test_build_throttle_fails_closed_if_a_pulse_cannot_be_stopped(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.runtime = root
            subject.args = SimpleNamespace(limit=60, poll=1)
            subject.deadline = machine.time.monotonic() + 60
            subject.stopping = False
            subject.watched_terminal_closed = mock.Mock(return_value=False)
            subject.event = mock.Mock()
            process = mock.Mock(pid=1234)
            process.poll.side_effect = [None, None, None]

            with mock.patch.object(
                machine.subprocess, "Popen", return_value=process
            ), mock.patch.object(
                machine, "resources", return_value=machine.Resources(20, 20, 12, 0, False)
            ), mock.patch.object(
                machine, "set_paused", side_effect=[True, True, False]
            ), mock.patch.object(
                machine, "stop_process"
            ) as stop, mock.patch.object(machine.time, "sleep"):
                with self.assertRaisesRegex(RuntimeError, "cannot re-pause"):
                    subject.resource_capped_command(
                        "cargo",
                        "build",
                        cwd=root,
                        env=None,
                        timeout=30,
                        purpose="build",
                        log_path=root / "build.log",
                    )

            stop.assert_called_once_with(process, timeout=2)

    def test_build_pressure_requires_recovery_headroom_before_resuming(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.runtime = root
            subject.args = SimpleNamespace(limit=60, poll=1)
            subject.deadline = machine.time.monotonic() + 60
            subject.stopping = False
            subject.watched_terminal_closed = mock.Mock(return_value=False)
            subject.event = mock.Mock()
            process = mock.Mock(pid=1234, returncode=0)
            process.poll.side_effect = [None, None, None, None, 0, 0]
            samples = [
                machine.Resources(48, 20, 12, 0, False),
                machine.Resources(44, 20, 12, 0, False),
                machine.Resources(30, 20, 12, 0, False),
            ]

            with mock.patch.object(
                machine.subprocess, "Popen", return_value=process
            ), mock.patch.object(
                machine, "resources", side_effect=samples
            ), mock.patch.object(
                machine, "set_paused", return_value=True
            ) as pause, mock.patch.object(machine.time, "sleep") as sleep:
                result = subject.resource_capped_command(
                    "cargo",
                    "build",
                    cwd=root,
                    env=None,
                    timeout=30,
                    purpose="build",
                    log_path=root / "build.log",
                )

            self.assertEqual(result.returncode, 0)
            self.assertEqual(
                pause.call_args_list,
                [mock.call(process, True), mock.call(process, False)],
            )
            self.assertEqual(
                [call.args[0] for call in subject.event.call_args_list],
                [
                    "work_cpu_throttle_started",
                    "work_paused_for_resources",
                    "work_resumed",
                ],
            )
            self.assertEqual(sleep.call_args_list[:2], [mock.call(1), mock.call(1)])
            resumed = subject.event.call_args_list[-1]
            self.assertEqual(resumed.kwargs["resources"]["cpu"], 30)

    def test_background_build_yields_until_visible_game_recovers(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.runtime = root
            subject.args = SimpleNamespace(limit=60, poll=1)
            subject.deadline = 100
            subject.stopping = False
            subject.watched_terminal_closed = mock.Mock(return_value=False)
            subject.event = mock.Mock()
            visible = SimpleNamespace(visible=True, paused=True)
            subject.games = [visible]
            process = mock.Mock(pid=1234, returncode=0)
            process.poll.side_effect = [None, None, None, None, 0, 0]

            def sleep(_seconds):
                visible.paused = False

            with mock.patch.object(
                machine.subprocess, "Popen", return_value=process
            ), mock.patch.object(
                machine, "resources", return_value=machine.Resources(20, 20, 12, 0, False)
            ), mock.patch.object(
                machine, "set_paused", return_value=True
            ) as pause, mock.patch.object(
                machine.time, "monotonic", side_effect=[0, 1, 2, 8]
            ), mock.patch.object(machine.time, "sleep", side_effect=sleep):
                result = subject.resource_capped_command(
                    "cargo",
                    "build",
                    cwd=root,
                    env=None,
                    timeout=30,
                    purpose="build",
                    log_path=root / "build.log",
                    prefer_visible=True,
                )

            self.assertEqual(result.returncode, 0)
            self.assertEqual(
                pause.call_args_list,
                [mock.call(process, True), mock.call(process, False)],
            )
            self.assertEqual(
                [call.args[0] for call in subject.event.call_args_list],
                [
                    "work_cpu_throttle_started",
                    "work_yielded_for_visible",
                    "work_resumed_after_visible",
                ],
            )

    def test_paused_build_does_not_age_against_the_timeout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.runtime = root
            subject.args = SimpleNamespace(limit=60, poll=1)
            subject.deadline = 10_000.0
            subject.stopping = False
            subject.watched_terminal_closed = mock.Mock(return_value=False)
            subject.event = mock.Mock()
            process = mock.Mock(pid=1234, returncode=0)
            process.poll.side_effect = [None, None, None, None, 0]

            with mock.patch.object(
                machine.subprocess, "Popen", return_value=process
            ), mock.patch.object(
                machine, "resources", return_value=machine.Resources(90, 20, 12, 0, False)
            ), mock.patch.object(
                machine, "set_paused", return_value=True
            ), mock.patch.object(
                machine.time, "monotonic", side_effect=[0.0, 10.0, 20.0, 30.0]
            ), mock.patch.object(machine.time, "sleep"), mock.patch.object(
                machine, "stop_process"
            ) as stop:
                result = subject.resource_capped_command(
                    "cargo",
                    "build",
                    cwd=root,
                    env=None,
                    timeout=5,
                    purpose="build",
                    log_path=root / "build.log",
                )

            self.assertEqual(result.returncode, 0)
            stop.assert_not_called()

    def test_duty_cycle_run_pulses_age_the_build_toward_its_timeout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.runtime = root
            subject.args = SimpleNamespace(limit=60, poll=1)
            subject.deadline = 10_000.0
            subject.stopping = False
            subject.watched_terminal_closed = mock.Mock(return_value=False)
            subject.event = mock.Mock()
            process = mock.Mock(pid=1234, returncode=None)
            process.poll.return_value = None
            duty = machine.build_cpu_duty_cycle(10, 60)
            self.assertGreater(duty, 0.0)
            timeout = 2.5 * machine.BUILD_CPU_DUTY_CYCLE_SECONDS * duty

            with mock.patch.object(
                machine.subprocess, "Popen", return_value=process
            ), mock.patch.object(
                machine, "resources", return_value=machine.Resources(10, 10, 12, 0, False)
            ), mock.patch.object(
                machine, "set_paused", return_value=True
            ), mock.patch.object(
                machine.time, "monotonic", side_effect=[float(i) for i in range(12)]
            ), mock.patch.object(machine.time, "sleep"), mock.patch.object(
                machine, "stop_process"
            ) as stop:
                with self.assertRaises(machine.subprocess.TimeoutExpired):
                    subject.resource_capped_command(
                        "cargo",
                        "build",
                        cwd=root,
                        env=None,
                        timeout=timeout,
                        purpose="build",
                        log_path=root / "build.log",
                    )

            stop.assert_called_once_with(process, timeout=2)

    def test_background_head_build_enables_visible_priority(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.build_executor = mock.Mock()

        subject.start_head_build("new-head")

        subject.build_executor.submit.assert_called_once_with(
            subject.compile_build, "new-head", True
        )

    def test_terminal_close_cancels_resource_capped_work(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.runtime = root
            subject.args = SimpleNamespace(limit=70, poll=1)
            subject.deadline = machine.time.monotonic() + 60
            subject.stopping = False
            subject.watched_terminal_closed = mock.Mock(return_value=True)
            subject.stop_for_terminal_close = mock.Mock()
            process = mock.Mock(pid=1234)
            process.poll.return_value = None

            with mock.patch.object(
                machine.subprocess, "Popen", return_value=process
            ), mock.patch.object(
                machine, "set_paused", return_value=True
            ), mock.patch.object(machine, "stop_process") as stop:
                result = subject.resource_capped_command(
                    "civvis",
                    "validate",
                    cwd=root,
                    env=None,
                    timeout=30,
                    purpose="validation",
                    log_path=root / "build.log",
                )

            self.assertIsNone(result)
            subject.stop_for_terminal_close.assert_called_once_with()
            stop.assert_called_once_with(process, timeout=2)

    def test_refresh_ranking_uses_a_runtime_snapshot_while_source_can_build(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            runtime = root / "runtime"
            league = runtime / "league"
            source_tools = source / "tools"
            source_tools.mkdir(parents=True)
            league.mkdir(parents=True)
            (league / "league.json").write_text("{}", encoding="utf-8")
            source_script = source_tools / "update_ai_player_elo_rankings.py"
            source_script.write_text("source", encoding="utf-8")
            runtime.mkdir(exist_ok=True)
            cached_script = runtime / "update_ai_player_elo_rankings.py"
            cached_script.write_text("cached", encoding="utf-8")

            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.source = source
            subject.runtime = runtime
            subject.league = league
            subject.ranking = runtime / "AI_PLAYER_ELO_RANKINGS.md"
            subject.ranking_updater = cached_script
            subject.event = mock.Mock()

            completed = SimpleNamespace(returncode=0, stdout="")
            with mock.patch.object(machine, "command", return_value=completed) as run:
                subject.refresh_ranking()

            self.assertEqual(run.call_args.args[1], str(cached_script))
            self.assertEqual(run.call_args.kwargs["cwd"], runtime)
            subject.event.assert_not_called()

    def test_cache_ranking_updater_snapshots_the_post_build_source_script(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            runtime = root / "runtime"
            source_script = source / "tools" / "update_ai_player_elo_rankings.py"
            source_script.parent.mkdir(parents=True)
            source_script.write_text("post-build updater", encoding="utf-8")
            runtime.mkdir()

            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.source = source
            subject.runtime = runtime
            subject.ranking_updater = runtime / "update_ai_player_elo_rankings.py"
            subject.event = mock.Mock()

            subject.cache_ranking_updater()

            self.assertEqual(
                subject.ranking_updater.read_text(encoding="utf-8"), "post-build updater"
            )
            subject.event.assert_not_called()

    def test_activate_build_snapshots_the_ranking_updater_before_refreshing(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.args = SimpleNamespace(sync_interval=300)
        subject.pending_revision = "new-head"
        subject.next_sync = 1.0
        subject.cache_ranking_updater = mock.Mock()
        subject.initialize_league = mock.Mock()
        subject.refresh_ranking = mock.Mock()
        subject.event = mock.Mock()

        promoted = Path("/tmp/civvis-new")
        with mock.patch.object(machine.time, "monotonic", return_value=100.0):
            subject.activate_build("new-head", promoted)

        subject.cache_ranking_updater.assert_called_once_with()
        subject.initialize_league.assert_called_once_with()
        subject.refresh_ranking.assert_called_once_with()
        self.assertIsNone(subject.pending_revision)
        self.assertEqual(subject.next_sync, 400.0)

    def test_sync_replaces_a_stale_pending_revision_before_a_build(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.args = SimpleNamespace(sync_interval=300)
        subject.current_revision = "current-head"
        subject.pending_revision = "stale-head"
        subject.fetch = mock.Mock(return_value="fresh-head")
        subject.event = mock.Mock()

        with mock.patch.object(machine.time, "monotonic", return_value=100.0):
            subject.sync()

        self.assertEqual(subject.pending_revision, "fresh-head")
        self.assertEqual(subject.next_sync, 400.0)
        subject.event.assert_called_once_with(
            "head_changed", current="current-head", target="fresh-head"
        )

    def test_sync_clears_a_queued_revision_when_head_returns_to_current(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.args = SimpleNamespace(sync_interval=300)
        subject.current_revision = "current-head"
        subject.pending_revision = "stale-head"
        subject.fetch = mock.Mock(return_value="current-head")
        subject.event = mock.Mock()

        with mock.patch.object(machine.time, "monotonic", return_value=100.0):
            subject.sync()

        self.assertIsNone(subject.pending_revision)
        self.assertEqual(subject.next_sync, 400.0)
        subject.event.assert_not_called()

    def test_fill_slots_resumes_paused_work_before_admitting_a_new_game(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.pending_revision = None
        subject.args = SimpleNamespace(limit=70, headless=8, max_processes=8)
        subject.games = [SimpleNamespace(visible=True, paused=True)]
        subject.launch = mock.Mock()

        subject.fill_slots(machine.Resources(20, 20, 12, 0, False))

        subject.launch.assert_not_called()

    def test_cpu_parser_uses_the_last_top_sample(self):
        report = "CPU usage: 10.0% user, 5.0% sys, 85.0% idle\nCPU usage: 20.0% user, 9.5% sys, 70.5% idle"
        self.assertEqual(machine.parse_top_cpu(report), 29.5)
        self.assertIsNone(machine.parse_top_cpu("not top"))

    def test_cpu_sampling_timeout_fails_closed_without_crashing_the_machine(self):
        with mock.patch.object(machine.sys, "platform", "darwin"), mock.patch.object(
            machine, "darwin_cpu_percent", return_value=None
        ), mock.patch.object(
            machine,
            "command",
            side_effect=subprocess.TimeoutExpired(["top"], 4),
        ):
            self.assertIsNone(machine.cpu_percent())

        missing_cpu = machine.Resources(None, 20, 12, 0, False)
        self.assertTrue(missing_cpu.overloaded(70))
        self.assertFalse(missing_cpu.comfortably_below(70))
        self.assertEqual(machine.resource_action(missing_cpu, 70), "shed_all")

    def test_darwin_cpu_tick_sampler_bootstraps_and_measures_idle_delta(self):
        first = (100, 50, 850, 0)
        second = (106, 55, 869, 0)
        with mock.patch.object(machine, "_darwin_cpu_ticks", None), mock.patch.object(
            machine, "darwin_cpu_ticks", side_effect=[first, second]
        ), mock.patch.object(machine.time, "sleep") as pause:
            self.assertAlmostEqual(machine.darwin_cpu_percent(), 100.0 * 11.0 / 30.0)

        pause.assert_called_once_with(0.2)

    def test_macos_cpu_sampling_uses_native_ticks_before_top(self):
        with mock.patch.object(machine.sys, "platform", "darwin"), mock.patch.object(
            machine, "darwin_cpu_percent", return_value=20.0
        ), mock.patch.object(machine, "command") as run:
            self.assertEqual(machine.cpu_percent(), 20.0)

        run.assert_not_called()

    def test_macos_cpu_sampling_falls_back_to_one_immediate_top_snapshot(self):
        report = "CPU usage: 12.0% user, 8.0% sys, 80.0% idle\n"
        completed = subprocess.CompletedProcess(["top"], 0, report)
        with mock.patch.object(machine.sys, "platform", "darwin"), mock.patch.object(
            machine, "darwin_cpu_percent", return_value=None
        ), mock.patch.object(machine, "command", return_value=completed
        ) as run:
            self.assertEqual(machine.cpu_percent(), 20.0)

        run.assert_called_once_with("top", "-l", "1", "-n", "0", timeout=4)

    def test_resource_ceiling_is_hard_and_resume_has_headroom(self):
        safe = machine.Resources(59.0, 20.0, 12.0, 0.0, False)
        edge = machine.Resources(70.0, 20.0, 12.0, 0.0, False)
        thermal = machine.Resources(1.0, 1.0, 1.0, 1.0, True)
        self.assertTrue(safe.comfortably_below(70))
        self.assertFalse(safe.overloaded(70))
        self.assertTrue(edge.overloaded(70))
        self.assertTrue(thermal.overloaded(70))
        self.assertEqual(machine.resource_action(machine.Resources(49, 20, 12, 0, False), 70), "resume")
        self.assertEqual(machine.resource_action(machine.Resources(55, 20, 12, 0, False), 70), "shed_one")
        self.assertEqual(machine.resource_action(machine.Resources(60, 20, 12, 0, False), 70), "shed_headless")
        self.assertEqual(machine.resource_action(edge, 70), "shed_all")

    def test_gpu_load_below_the_limit_does_not_margin_gate_cpu_work(self):
        crowded = machine.Resources(20, 20, 12, 60, False)
        self.assertEqual(machine.resource_action(crowded, 70), "resume")
        self.assertTrue(crowded.comfortably_below(70, margin=machine.RESUME_MARGIN))

    def test_gpu_at_the_limit_still_stops_everything(self):
        saturated = machine.Resources(20, 20, 12, 70, False)
        self.assertEqual(machine.resource_action(saturated, 70), "shed_all")
        self.assertTrue(saturated.overloaded(70))
        self.assertFalse(saturated.comfortably_below(70, margin=machine.RESUME_MARGIN))

    def test_head_transition_recovery_alternates_headless_and_visible(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.args = SimpleNamespace(limit=70)
        subject.maxima = {name: 0.0 for name in ("cpu", "memory", "disk", "gpu")}
        subject.pending_revision = "new-head"
        subject.build_future = None
        subject.resume_not_before = 0.0
        subject.resume_visible_next = False
        subject.event = mock.Mock()
        visible = SimpleNamespace(
            visible=True, paused=True, process=object(), seed=1
        )
        headless = SimpleNamespace(
            visible=False, paused=True, process=object(), seed=2
        )
        subject.games = [visible, headless]
        safe = machine.Resources(20, 20, 12, 0, False)

        with mock.patch.object(machine, "set_paused", return_value=True) as resume:
            subject.govern(safe)
            headless.paused = True
            subject.resume_not_before = 0.0
            subject.govern(safe)

        self.assertEqual(
            resume.call_args_list,
            [mock.call(headless.process, False), mock.call(visible.process, False)],
        )
        self.assertFalse(visible.paused)
        self.assertTrue(headless.paused)

    def test_steady_state_recovery_keeps_visible_first(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        subject.args = SimpleNamespace(limit=70)
        subject.maxima = {name: 0.0 for name in ("cpu", "memory", "disk", "gpu")}
        subject.pending_revision = None
        subject.build_future = None
        subject.resume_not_before = 0.0
        subject.resume_visible_next = False
        subject.event = mock.Mock()
        visible = SimpleNamespace(
            visible=True, paused=True, process=object(), seed=1
        )
        headless = SimpleNamespace(
            visible=False, paused=True, process=object(), seed=2
        )
        subject.games = [visible, headless]

        with mock.patch.object(machine, "set_paused", return_value=True) as resume:
            subject.govern(machine.Resources(20, 20, 12, 0, False))

        resume.assert_called_once_with(visible.process, False)
        self.assertFalse(visible.paused)
        self.assertTrue(headless.paused)

    def test_match_lookup_finds_a_concurrent_out_of_order_result(self):
        with tempfile.TemporaryDirectory() as directory:
            league = Path(directory)
            (league / "matches.csv").write_text(
                "round,seed,turns,victory,placements\n"
                "60,12,300,science,a@Trajan@Rome@0|b@Cleopatra@Egypt@1\n"
                "61,10,250,culture,b@Trajan@Rome@0|a@Cleopatra@Egypt@1\n",
                encoding="utf-8",
            )
            self.assertEqual(machine.match_row(league, 12)["victory"], "science")
            self.assertIsNone(machine.match_row(league, 99))

    def test_recorded_result_is_authoritative_over_later_server_status(self):
        with tempfile.TemporaryDirectory() as directory:
            league = Path(directory)
            (league / "matches.csv").write_text(
                "round,seed,turns,victory,placements\n"
                "60,12,423,science,a@Trajan@Rome@0|b@Cleopatra@Egypt@1\n",
                encoding="utf-8",
            )
            row = machine.match_row(league, 12)
            self.assertEqual(machine.winner_placement(row), "a@Trajan@Rome@0")

    def test_state_write_is_atomic_json(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "state.json"
            machine.atomic_json(target, {"active": 8})
            self.assertEqual(json.loads(target.read_text()), {"active": 8})
            self.assertFalse(target.with_suffix(".json.tmp").exists())

    def test_visible_browser_open_state_survives_operator_restart(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory) / "state.json"
            self.assertFalse(machine.visible_browser_already_opened(state))
            state.write_text('{"visible_started": true}\n', encoding="utf-8")
            self.assertTrue(machine.visible_browser_already_opened(state))

    def test_game_lifecycle_events_do_not_collide_with_event_kind(self):
        with tempfile.TemporaryDirectory() as directory:
            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.args = SimpleNamespace(port=8870)
            subject.binary = Path("/tmp/civvis")
            subject.league = Path(directory) / "league"
            subject.logs = Path(directory) / "logs"
            subject.logs.mkdir()
            subject.current_revision = "abc123"
            subject.next_seed = 100
            subject.games = []
            subject.visible_started = False
            subject.visible_completed = False
            subject.visible_completed_count = 0
            subject.visible_browser_opened = False
            subject.completed = 0
            subject.failed = 0
            events = []
            subject.event = lambda kind, **values: events.append((kind, values))
            process = mock.Mock(pid=1234)

            with mock.patch.object(machine, "game_port", return_value=8870), mock.patch.object(
                machine.subprocess, "Popen", return_value=process
            ) as launch:
                subject.launch(visible=True)
            self.assertEqual(launch.call_args.kwargs["env"]["CIVVIS_COMMIT"], "abc123")
            subject.games[0].last_status = {
                "turn": 435,
                "winner": 1,
                "victory_type": "religious",
            }
            with mock.patch.object(machine, "stop_process"), mock.patch.object(
                machine,
                "match_row",
                return_value={
                    "seed": "100",
                    "turns": "423",
                    "victory": "science",
                    "placements": "a@Trajan@Rome@0|b@Cleopatra@Egypt@1",
                },
            ):
                subject.finish(
                    subject.games[0],
                    failed=True,
                    reason="match machine stopped before result",
                )

            self.assertEqual(events[0][0], "game_started")
            self.assertEqual(events[0][1]["game_kind"], "visible")
            self.assertEqual(events[1][0], "game_completed")
            self.assertEqual(events[1][1]["game_kind"], "visible")
            self.assertEqual(events[1][1]["reason"], "rated result recorded before process stopped")
            self.assertEqual(events[1][1]["turn"], "423")
            self.assertEqual(events[1][1]["victory"], "science")
            self.assertIsNone(events[1][1]["winner"])
            self.assertEqual(events[1][1]["winner_placement"], "a@Trajan@Rome@0")
            self.assertEqual(subject.completed, 1)
            self.assertEqual(subject.failed, 0)

    def test_compile_build_excludes_inherited_commit_from_cargo(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            built = source / "target" / "release" / "civvis"
            built.parent.mkdir(parents=True)
            built.write_text("binary", encoding="utf-8")

            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            subject.source = source
            subject.runtime = root / "runtime"
            subject.args = SimpleNamespace(build_jobs=1, build_timeout=30)
            subject.event = mock.Mock()
            complete = subprocess.CompletedProcess([], 0, "")

            with mock.patch.dict(
                machine.os.environ, {"CIVVIS_COMMIT": "inherited"}, clear=False
            ), mock.patch.object(
                machine, "command", return_value=complete
            ), mock.patch.object(
                subject, "resource_capped_command", side_effect=[complete, complete]
            ) as run:
                promoted = subject.compile_build("current-head")

            cargo_call = run.call_args_list[0]
            self.assertEqual(cargo_call.args[0], "cargo")
            self.assertNotIn("CIVVIS_COMMIT", cargo_call.kwargs["env"])
            self.assertTrue(promoted.exists())

    def test_visible_successor_reuses_process_and_records_both_lifecycle_events(self):
        subject = machine.MatchMachine.__new__(machine.MatchMachine)
        process = mock.Mock(pid=1234)
        game = machine.GameProcess(
            process=process,
            seed=100,
            port=8870,
            revision="abc123",
            visible=True,
            started_monotonic=10.0,
            started_utc="then",
            log="visible.log",
            winner_seen=20.0,
            last_status={"turn": 250, "winner": 1},
        )
        subject.games = [game]
        subject.completed = 0
        subject.failed = 0
        subject.visible_completed = False
        subject.visible_completed_count = 0
        subject.next_seed = 101
        events = []
        subject.event = lambda kind, **values: events.append((kind, values))
        row = {
            "seed": "100",
            "turns": "250",
            "victory": "score",
            "placements": "a@Trajan@Rome@0|b@Cleopatra@Egypt@1",
        }

        with mock.patch.object(machine.time, "monotonic", return_value=30.0):
            subject.adopt_visible_successor(
                game, seed=200, row=row, status={"turn": 1, "winner": None}
            )

        self.assertIs(subject.games[0].process, process)
        self.assertEqual(subject.games[0].seed, 200)
        self.assertEqual(subject.games[0].last_status["turn"], 1)
        self.assertIsNone(subject.games[0].winner_seen)
        self.assertEqual(subject.completed, 1)
        self.assertEqual(subject.visible_completed_count, 1)
        self.assertEqual([event[0] for event in events], ["game_completed", "game_started"])
        self.assertTrue(events[1][1]["reused_process"])

    def test_poll_adopts_automatic_visible_successor_without_stopping_server(self):
        with tempfile.TemporaryDirectory() as directory:
            league = Path(directory)
            (league / "matches.csv").write_text(
                "round,seed,turns,victory,placements\n"
                "1,100,250,score,a@Trajan@Rome@0|b@Cleopatra@Egypt@1\n",
                encoding="utf-8",
            )
            subject = machine.MatchMachine.__new__(machine.MatchMachine)
            process = mock.Mock(pid=1234)
            process.poll.return_value = None
            subject.league = league
            subject.current_revision = "abc123"
            subject.games = [
                machine.GameProcess(
                    process=process,
                    seed=100,
                    port=8870,
                    revision="abc123",
                    visible=True,
                    started_monotonic=10.0,
                    started_utc="then",
                    log="visible.log",
                    winner_seen=20.0,
                    last_status={"turn": 250, "winner": 1},
                )
            ]
            subject.completed = 0
            subject.failed = 0
            subject.visible_completed = False
            subject.visible_completed_count = 0
            subject.next_seed = 101
            subject.event = mock.Mock()

            def response(_port, path, **_kwargs):
                return (
                    {"seed": 200, "server_instance": 1234}
                    if path == "/runtime"
                    else {"turn": 1, "winner": None}
                )

            with mock.patch.object(machine.time, "monotonic", return_value=30.0), mock.patch.object(
                machine, "http_json", side_effect=response
            ), mock.patch.object(machine, "stop_process") as stop:
                changed = subject.poll_games()

            self.assertTrue(changed)
            stop.assert_not_called()
            self.assertEqual(subject.games[0].seed, 200)
            self.assertEqual(subject.completed, 1)

if __name__ == "__main__":
    unittest.main()
