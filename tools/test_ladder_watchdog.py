#!/usr/bin/env python3
"""The keeper starts a stopped ladder loop, stops a wedged one, and does neither
for the wrong reason."""

from __future__ import annotations

import contextlib
import io
import json
import os
import sys
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent / "ops"))

import civ6_ladder  # noqa: E402
import ladder_watchdog  # noqa: E402


class FakeRunner:
    """Records the argv the keeper would run, e.g. `open -a Terminal ...`."""

    def __init__(self, returncode: int = 0):
        self.calls: list[list[str]] = []
        self.returncode = returncode

    def __call__(self, argv, **kwargs):
        self.calls.append(list(argv))

        class Done:
            pass

        done = Done()
        done.returncode = self.returncode
        done.stdout = ""
        done.stderr = "" if self.returncode == 0 else "could not open"
        return done


def ledger_with(tmp: Path, ages_hours: list[float], *,
                now: datetime | None = None) -> Path:
    """A runs directory whose ladder holds attempts of the given ages."""
    now = now or datetime.now(timezone.utc)
    runs = tmp / "control"
    runs.mkdir(parents=True, exist_ok=True)
    attempts = []
    for index, age in enumerate(ages_hours):
        stamp = (now - timedelta(hours=age)).strftime("%Y-%m-%dT%H:%M:%SZ")
        attempts.append({"tag": f"run-{index}", "utc": stamp,
                         "difficulty": "DIFFICULTY_SETTLER"})
    (runs / "ladder.json").write_text(
        json.dumps({"attempts": attempts, "wins": {}}, indent=2))
    return runs


class KeeperTestCase(unittest.TestCase):
    """Every subprocess and every stop the keeper would issue, captured.

    ⚠ `ladder_watchdog.subprocess` IS the stdlib module, so assigning
    `ladder_watchdog.subprocess.run = fake` does not patch this module — it
    patches `subprocess` for the whole interpreter, permanently. Written that
    way first, and the damage showed up nowhere near here: `test_memguard` and
    `test_spectator_supervisor` both failed in the full discovery run and both
    passed in isolation, because by then every `subprocess.run` in the process
    was returning this file's fake. Patch through `mock`, which restores.
    """

    def setUp(self):
        self.runner = FakeRunner()
        self.stopped: list[int] = []
        for patch in (
            mock.patch.object(ladder_watchdog.subprocess, "run", self.runner),
            mock.patch.object(ladder_watchdog, "stop_supervisor",
                              lambda pid, **kw: (self.stopped.append(pid),
                                                 (True, f"SIGTERM to {pid}"))[1]),
        ):
            patch.start()
            self.addCleanup(patch.stop)

    @property
    def starts(self):
        return [c for c in self.runner.calls if c[:3] == ["open", "-a", "Terminal"]]

    def live_lock(self, tmp: Path) -> Path:
        """A lock directory naming a process that really is alive: this one."""
        lock = tmp / "supervisor.lock"
        lock.mkdir()
        (lock / "pid").write_text(f"{os.getpid()}\n")
        return lock

    def dead_lock(self, tmp: Path) -> Path:
        """A lock left behind by a supervisor that was killed, pid long gone."""
        lock = tmp / "supervisor.lock"
        lock.mkdir()
        (lock / "pid").write_text("999999\n")
        return lock


class WhenNoLoopIsRunning(KeeperTestCase):
    def test_the_loop_is_started_through_terminal(self):
        """Not `zsh script`. A LaunchAgent's child cannot install the mod.

        Measured 2026-08-17: from a bare LaunchAgent the mod install fails with
        "Operation not permitted", so every attempt plays no turns. `open -a
        Terminal` hands it to the one app on the host holding the grant.
        """
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(tmp / "absent"),
                "--supervisor", "/x/supervisor.sh",
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 1)
            self.assertEqual(len(self.starts), 1, self.runner.calls)
            self.assertEqual(self.starts[0][-1], "/x/supervisor.sh")

    def test_a_lock_from_a_dead_supervisor_is_not_a_supervisor(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(self.dead_lock(tmp)),
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(len(self.starts), 1,
                             "a pid file is a claim, not an answer")

    def test_a_fresh_ledger_with_no_loop_still_starts_one(self):
        """The last game finished, then the supervisor died.

        Waiting for the ledger to go stale would throw away every hour between
        now and the staleness limit, for a condition already visible.
        """
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [0.2])
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(tmp / "absent"),
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 1)
            self.assertEqual(len(self.starts), 1)

    def test_a_failed_start_is_reported_not_swallowed(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            log = tmp / "log"
            self.runner.returncode = 1
            ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(tmp / "absent"),
                "--state", str(tmp / "state.json"), "--log", str(log)])
            self.assertIn("could not start the loop", log.read_text())


class WhenTheLoopIsAliveButNotPlaying(KeeperTestCase):
    def test_a_wedged_supervisor_is_stopped(self):
        """KeepAlive can never see this: to the process table it is healthy."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(self.live_lock(tmp)),
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 1)
            self.assertEqual(self.stopped, [os.getpid()])
            self.assertEqual(self.starts, [],
                             "the next tick starts the replacement, not this one")

    def test_a_loop_that_is_playing_is_left_entirely_alone(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [0.5, 4.0])
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(self.live_lock(tmp)),
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 0)
            self.assertEqual(self.stopped, [])
            self.assertEqual(self.runner.calls, [])

    def test_an_empty_ledger_counts_as_stopped(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [])
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(self.live_lock(tmp)),
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 1)
            self.assertEqual(self.stopped, [os.getpid()])

    def test_an_unrecorded_summary_is_not_a_reason_to_act(self):
        """`check` exits 1 for three problems; only one is the loop's.

        A summary on disk the ledger has not imported wants `sync`, and a
        trailing snapshot wants `publish`. Restarting the game loop fixes
        neither, so keying this on that exit code would kill a healthy game
        every ten minutes for a paperwork problem.
        """
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [0.5])
            unrecorded = runs / "civvis-unrecorded"
            unrecorded.mkdir()
            (unrecorded / "summary.json").write_text(json.dumps(
                {"tag": "civvis-unrecorded", "difficulty": "DIFFICULTY_SETTLER"}))

            reported = civ6_ladder.check(runs, runs / "ladder.json",
                                         stale_hours=3.0,
                                         snapshot=tmp / "absent.json")
            self.assertEqual(reported, 1, "check must still see the paperwork")

            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(self.live_lock(tmp)),
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 0)
            self.assertEqual(self.stopped, [])


class TheCooldownBoundsTheActing(KeeperTestCase):
    def test_a_wedge_is_not_restarted_every_interval(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            argv = ["--runs", str(runs), "--stale-hours", "3",
                    "--cooldown-hours", "2",
                    "--lock", str(self.live_lock(tmp)),
                    "--state", str(tmp / "state.json"), "--log", str(tmp / "log")]
            for _ in range(3):
                ladder_watchdog.main(argv)
            self.assertEqual(len(self.stopped), 1,
                             "a wedge this cannot clear must escalate to one "
                             "report, not restart until morning")

    def test_the_cooldown_expires(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            state = tmp / "state.json"
            long_ago = (datetime.now(timezone.utc)
                        - timedelta(hours=9)).isoformat(timespec="seconds")
            state.write_text(json.dumps({"last_kick_utc": long_ago, "kicks": 1}))
            ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--cooldown-hours", "2",
                "--lock", str(self.live_lock(tmp)),
                "--state", str(state), "--log", str(tmp / "log")])
            self.assertEqual(len(self.stopped), 1)

    def test_a_failed_action_does_not_start_the_cooldown(self):
        """Measured 2026-08-17T20:41:48Z: the first restart failed because the
        job it addressed was not loaded, and the failure took the full cooldown
        with it — extending the outage the keeper exists to end."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            state = tmp / "state.json"
            self.runner.returncode = 1
            argv = ["--runs", str(runs), "--stale-hours", "3",
                    "--cooldown-hours", "2",
                    "--lock", str(tmp / "absent"),
                    "--state", str(state), "--log", str(tmp / "log")]
            ladder_watchdog.main(argv)
            ladder_watchdog.main(argv)
            self.assertEqual(len(self.starts), 2,
                             "an action that never took effect changed nothing")
            recorded = json.loads(state.read_text())
            self.assertEqual(recorded.get("failed_starts"), 2)
            self.assertNotIn("last_start_utc", recorded)

    def test_an_absent_loop_is_restarted_in_minutes_not_hours(self):
        """A loop that is GONE is not a loop that is WEDGED.

        Stopping a wedged supervisor is disruptive and may not help, so it waits
        hours. Starting an absent one is cheap and is the whole point, so making
        it wait the same two hours reproduces in miniature the outage the keeper
        exists to end.
        """
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            state = tmp / "state.json"
            gone = (datetime.now(timezone.utc)
                    - timedelta(minutes=20)).isoformat(timespec="seconds")
            state.write_text(json.dumps({"last_start_utc": gone, "starts": 1}))
            ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--cooldown-hours", "2", "--start-cooldown-minutes", "15",
                "--lock", str(tmp / "absent"),
                "--state", str(state), "--log", str(tmp / "log")])
            self.assertEqual(len(self.starts), 1,
                             "20 minutes is past the start cooldown; a two-hour "
                             "wait to restart a dead loop is the outage itself")

    def test_a_crash_loop_is_still_bounded(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            argv = ["--runs", str(runs), "--stale-hours", "3",
                    "--start-cooldown-minutes", "15",
                    "--lock", str(tmp / "absent"),
                    "--state", str(tmp / "state.json"), "--log", str(tmp / "log")]
            for _ in range(4):
                ladder_watchdog.main(argv)
            self.assertEqual(len(self.starts), 1)

    def test_stopping_a_wedge_and_starting_a_loop_keep_separate_clocks(self):
        """A stop must not silence a start, nor the other way round."""
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            state = tmp / "state.json"
            common = ["--runs", str(runs), "--stale-hours", "3",
                      "--cooldown-hours", "2", "--start-cooldown-minutes", "15",
                      "--state", str(state), "--log", str(tmp / "log")]
            ladder_watchdog.main(common + ["--lock", str(self.live_lock(tmp))])
            self.assertEqual(len(self.stopped), 1)
            ladder_watchdog.main(common + ["--lock", str(tmp / "absent")])
            self.assertEqual(len(self.starts), 1,
                             "stopping a wedge must not block the restart")

    def test_an_action_that_lands_does_start_the_cooldown(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            argv = ["--runs", str(runs), "--stale-hours", "3",
                    "--cooldown-hours", "2",
                    "--lock", str(tmp / "absent"),
                    "--state", str(tmp / "state.json"), "--log", str(tmp / "log")]
            ladder_watchdog.main(argv)
            ladder_watchdog.main(argv)
            self.assertEqual(len(self.starts), 1)


class StoppingIsGentle(unittest.TestCase):
    def test_a_wedged_supervisor_gets_TERM_not_KILL(self):
        """The supervisor's own header: hard kills wedge the Civ 6 core.

        A remedy that leaves the game unable to start a gameplay context has
        become the fault. TERM is trapped; the supervisor releases its lock.
        """
        import signal
        sent = []
        ok, detail = ladder_watchdog.stop_supervisor(
            4242, killer=lambda pid, sig: sent.append((pid, sig)))
        self.assertTrue(ok)
        self.assertEqual(sent, [(4242, signal.SIGTERM)])

    def test_a_process_that_already_exited_is_not_an_error(self):
        def gone(pid, sig):
            raise ProcessLookupError

        ok, detail = ladder_watchdog.stop_supervisor(4242, killer=gone)
        self.assertTrue(ok)
        self.assertIn("already gone", detail)


class TheLogIsWrittenOnce(KeeperTestCase):
    """launchd points StandardOutPath at the same file `log()` appends to."""

    def test_a_non_tty_run_does_not_double_every_line(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            log = tmp / "log"
            captured = io.StringIO()
            with contextlib.redirect_stdout(captured):
                ladder_watchdog.main([
                    "--runs", str(runs), "--stale-hours", "3",
                    "--lock", str(tmp / "absent"),
                    "--state", str(tmp / "state.json"), "--log", str(log)])
            opened = [line for line in log.read_text().splitlines()
                      if "starting the loop" in line]
            self.assertEqual(len(opened), 1, log.read_text())
            self.assertEqual(captured.getvalue(), "",
                             "under launchd stdout IS the log file")


class StalenessHasOneDefinition(unittest.TestCase):
    """`check` reports it, the keeper acts on it, both call the same function."""

    def test_check_and_keeper_agree_at_the_boundary(self):
        now = datetime.now(timezone.utc)
        for age, stale in ((2.9, False), (3.1, True)):
            state = {"attempts": [{
                "tag": "run",
                "utc": (now - timedelta(hours=age)).strftime("%Y-%m-%dT%H:%M:%SZ"),
            }], "wins": {}}
            problem = civ6_ladder.staleness_problem(state, 3.0, now=now)
            self.assertEqual(problem is not None, stale, f"at {age}h")

    def test_the_check_still_reports_staleness_through_the_helper(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            code = civ6_ladder.check(runs, runs / "ladder.json",
                                     stale_hours=3.0,
                                     snapshot=tmp / "absent.json")
            self.assertEqual(code, 1)


if __name__ == "__main__":
    unittest.main()
