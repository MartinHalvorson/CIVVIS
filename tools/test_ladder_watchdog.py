#!/usr/bin/env python3
"""The watchdog restarts a stopped ladder loop, and only for the right reason."""

from __future__ import annotations

import contextlib
import io
import json
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


class FakeLaunchctl:
    """Records the launchctl argv the watchdog would run."""

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
        done.stderr = "" if self.returncode == 0 else "Could not find service"
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


class WatchdogTestCase(unittest.TestCase):
    """Every subprocess this module would run, captured and then restored.

    ⚠ `ladder_watchdog.subprocess` IS the stdlib module, so assigning
    `ladder_watchdog.subprocess.run = fake` does not patch this module — it
    patches `subprocess` for the whole interpreter, permanently. Written that
    way first, and the damage showed up nowhere near here: `test_memguard` and
    `test_spectator_supervisor` both failed in the full discovery run and both
    passed in isolation, because by then every `subprocess.run` in the process
    was returning this file's fake launchctl. Patch through `mock`, which
    restores.
    """

    def setUp(self):
        self.launchctl = FakeLaunchctl()
        patch = mock.patch.object(ladder_watchdog.subprocess, "run",
                                  self.launchctl)
        patch.start()
        self.addCleanup(patch.stop)

    @property
    def calls(self):
        return self.launchctl.calls


class WatchdogDecides(WatchdogTestCase):
    def test_a_loop_that_is_playing_is_left_alone(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [0.5, 4.0])
            fake = self.launchctl
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 0)
            self.assertEqual(fake.calls, [],
                             "a ladder recording attempts must not be kicked")

    def test_a_stopped_loop_is_restarted(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            fake = self.launchctl
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--label", "com.example.ladder",
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 1)
            kicks = [c for c in fake.calls if c[0] == "launchctl"]
            self.assertEqual(len(kicks), 1, f"expected one kick, got {fake.calls}")
            self.assertIn("com.example.ladder", kicks[0][-1])

    def test_the_kick_is_a_restart_not_a_no_op(self):
        """`launchctl kickstart` without -k does nothing to a running job.

        The wedged case this exists to clear is a job that IS running. Without
        `-k` the watchdog would report success every interval and change
        nothing, which is the failure it was built to end.
        """
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            fake = self.launchctl
            ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertIn("-k", fake.calls[0])

    def test_a_second_kick_waits_for_the_cooldown(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            state = tmp / "state.json"
            fake = self.launchctl
            argv = ["--runs", str(runs), "--stale-hours", "3",
                    "--cooldown-hours", "2",
                    "--state", str(state), "--log", str(tmp / "log")]
            ladder_watchdog.main(argv)
            ladder_watchdog.main(argv)
            ladder_watchdog.main(argv)
            kicks = [c for c in fake.calls if "kickstart" in c]
            self.assertEqual(len(kicks), 1,
                             "a wedge this cannot clear must not be kicked "
                             "every interval until morning")

    def test_the_cooldown_expires(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            state = tmp / "state.json"
            long_ago = (datetime.now(timezone.utc)
                        - timedelta(hours=9)).isoformat(timespec="seconds")
            state.write_text(json.dumps({"last_kick_utc": long_ago, "kicks": 1}))
            fake = self.launchctl
            ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--cooldown-hours", "2",
                "--state", str(state), "--log", str(tmp / "log")])
            self.assertTrue([c for c in fake.calls if "kickstart" in c])

    def test_an_unrecorded_summary_is_not_a_reason_to_restart(self):
        """`check` exits 1 for three problems; only one is the supervisor's.

        A summary on disk that the ledger has not imported wants `sync`, and a
        trailing snapshot wants `publish`. Restarting the game loop fixes
        neither, so keying this watchdog on that exit code would kill a healthy
        game every ten minutes for a paperwork problem.
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

            fake = self.launchctl
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 0)
            self.assertEqual(fake.calls, [])

    def test_an_empty_ledger_counts_as_stopped(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [])
            fake = self.launchctl
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 1)
            self.assertTrue([c for c in fake.calls if "kickstart" in c])

    def test_a_failed_restart_is_reported_not_swallowed(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            log = tmp / "log"
            fake = self.launchctl
            fake.returncode = 113
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--state", str(tmp / "state.json"), "--log", str(log)])
            self.assertEqual(code, 1)
            self.assertIn("could not restart", log.read_text())


class AFailedKickIsNotACooldown(WatchdogTestCase):
    """The cooldown protects against restarting a wedge, not against retrying.

    Measured on this host at 2026-08-17T20:41:48Z: the watchdog's first kick
    failed because the supervisor job was not loaded yet, and recording it as a
    kick parked the retry for two hours — extending the exact outage the
    watchdog exists to end.
    """

    def test_a_failed_kick_is_retried_next_interval(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            self.launchctl.returncode = 113
            argv = ["--runs", str(runs), "--stale-hours", "3",
                    "--cooldown-hours", "2",
                    "--state", str(tmp / "state.json"),
                    "--log", str(tmp / "log")]
            ladder_watchdog.main(argv)
            ladder_watchdog.main(argv)
            kicks = [c for c in self.calls if "kickstart" in c]
            self.assertEqual(len(kicks), 2,
                             "a kick that never reached launchd changed "
                             "nothing and must be tried again")

    def test_a_failed_kick_is_counted_separately(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            state = tmp / "state.json"
            self.launchctl.returncode = 113
            ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--state", str(state), "--log", str(tmp / "log")])
            recorded = json.loads(state.read_text())
            self.assertEqual(recorded.get("failed_kicks"), 1)
            self.assertNotIn("last_kick_utc", recorded,
                             "a failure must not start the cooldown clock")

    def test_a_kick_that_lands_still_holds_the_cooldown(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            argv = ["--runs", str(runs), "--stale-hours", "3",
                    "--cooldown-hours", "2",
                    "--state", str(tmp / "state.json"),
                    "--log", str(tmp / "log")]
            ladder_watchdog.main(argv)
            ladder_watchdog.main(argv)
            kicks = [c for c in self.calls if "kickstart" in c]
            self.assertEqual(len(kicks), 1)


class TheLogIsWrittenOnce(WatchdogTestCase):
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
                    "--state", str(tmp / "state.json"), "--log", str(log)])
            stale = [line for line in log.read_text().splitlines()
                     if "STALE" in line]
            self.assertEqual(len(stale), 1, log.read_text())
            self.assertEqual(captured.getvalue(), "",
                             "under launchd stdout IS the log file")


class StalenessHasOneDefinition(unittest.TestCase):
    """`check` reports it, the watchdog acts on it, both call the same function."""

    def test_check_and_watchdog_agree_at_the_boundary(self):
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
