#!/usr/bin/env python3
"""The keeper starts a stopped ladder loop, stops a wedged one, and does neither
for the wrong reason."""

from __future__ import annotations

import contextlib
import io
import json
import os
import subprocess
import sys
import time
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
    """Records the argv the keeper would run, e.g. `open -g -j -a Terminal`."""

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
            mock.patch.object(ladder_watchdog, "interactive_host_pid",
                              lambda lock=None: None),
        ):
            patch.start()
            self.addCleanup(patch.stop)

    @property
    def starts(self):
        return [c for c in self.runner.calls
                if c and c[0] == "open" and "-a" in c
                and c[c.index("-a") + 1:c.index("-a") + 2] == ["Terminal"]]

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
            self.assertIn("-g", self.starts[0])
            self.assertIn("-j", self.starts[0])
            self.assertEqual(self.starts[0][-1], "/x/supervisor.sh")

    def test_an_existing_interactive_host_is_not_given_a_second_terminal(self):
        with TemporaryDirectory() as raw, \
             mock.patch.object(ladder_watchdog, "interactive_host_pid",
                               lambda lock=None: os.getpid()):
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            log = tmp / "log"
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(tmp / "absent"),
                "--host-lock", str(tmp / "host.lock"),
                "--state", str(tmp / "state.json"), "--log", str(log)])
            self.assertEqual(code, 1)
            self.assertEqual(self.starts, [])
            self.assertIn("not opening a second Terminal", log.read_text())

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


@unittest.skipUnless(Path("/bin/zsh").exists(),
                     "the interactive host is a zsh script")
class InteractiveHostOwnership(unittest.TestCase):
    """A recovery host must adopt a live loop instead of competing with it."""

    HOST = Path(__file__).resolve().parent / "ops" / "civvis-interactive-host.sh"

    def test_the_terminal_launcher_routes_normal_recovery_through_the_host(self):
        source = (Path(__file__).resolve().parent / "ops" /
                  "civvis-ladder-terminal-launcher.sh").read_text()
        self.assertIn('civvis-interactive-host.sh', source)
        self.assertIn('CIVVIS_LADDER_HOST', source)

    def test_an_adopted_supervisor_survives_the_host(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            log = tmp / "host.log"
            host_lock = tmp / "host.lock"
            supervisor_lock = tmp / "supervisor.lock"
            supervisor_lock.mkdir()
            supervisor = tmp / "civvis-game-supervisor.sh"
            supervisor.write_text("#!/bin/zsh\nwhile true; do sleep 1; done\n")
            supervisor.chmod(0o755)
            external = subprocess.Popen(["/bin/zsh", str(supervisor)])
            host = None
            try:
                (supervisor_lock / "pid").write_text(f"{external.pid}\n")
                helper_marks = []
                helpers = []
                for name in ("popup-keeper.sh", "mirror-keeper.sh"):
                    marker = tmp / f"{name}.started"
                    helper = tmp / name
                    helper.write_text(
                        "#!/bin/zsh\n"
                        f"print -r -- started > {marker}\n"
                        "while true; do sleep 1; done\n")
                    helper.chmod(0o755)
                    helper_marks.append(marker)
                    helpers.append(helper)
                env = {
                    **os.environ,
                    "CIVVIS_SUPERVISOR": str(supervisor),
                    "CIVVIS_POPUP_KEEPER": str(helpers[0]),
                    "CIVVIS_MIRROR_KEEPER": str(helpers[1]),
                    "CIVVIS_INTERACTIVE_HOST_LOG": str(log),
                    "CIVVIS_INTERACTIVE_HOST_LOCK": str(host_lock),
                    "CIVVIS_SUPERVISOR_LOCK": str(supervisor_lock),
                    "CIVVIS_INTERACTIVE_HOST_POLL_S": "1",
                }
                host = subprocess.Popen(["/bin/zsh", str(self.HOST)], env=env)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    if log.exists() and "adopted already-live game supervisor" in log.read_text():
                        break
                    time.sleep(0.05)
                self.assertTrue(log.exists(), "host did not write a health record")
                self.assertIn("adopted already-live game supervisor", log.read_text())
                self.assertFalse(any(mark.exists() for mark in helper_marks),
                                 "an adopted batch must not get duplicate helpers")
                host.terminate()
                host.wait(timeout=5)
                self.assertIsNone(external.poll(),
                                  "the host must not terminate an adopted supervisor")
            finally:
                if host is not None and host.poll() is None:
                    host.terminate()
                    host.wait(timeout=5)
                if external.poll() is None:
                    external.terminate()
                    external.wait(timeout=5)


@unittest.skipUnless(Path("/bin/zsh").exists(),
                     "the supervisor ownership check is a zsh function")
class SupervisorUnownedHarnessDetection(unittest.TestCase):
    """A filename in an operator shell must not impersonate a live harness."""

    SUPERVISOR = (Path(__file__).resolve().parent / "ops" /
                  "civvis-game-supervisor.sh")

    def test_a_global_candidate_is_typed_before_it_delays_a_batch(self):
        """macOS `ps -o comm` truncates Python, so use the executable mapping.

        This guards the exact restart failure where a harmless diagnostic
        command containing the harness filename looked like an unowned game
        and made the supervisor sleep for a retry interval.
        """
        source = self.SUPERVISOR.read_text()
        start = source.index("unowned_harness_pid() {")
        end = source.index("\n# The live display", start)
        detector = source[start:end]

        self.assertIn("pgrep -f '[c]iv6_play.py'", detector)
        self.assertIn('lsof -a -p "$pid" -d txt -Fn', detector)
        self.assertIn('*/Python|*/python|*/python[0-9]*', detector)
        self.assertLess(detector.index('lsof -a -p "$pid" -d txt -Fn'),
                        detector.index('print -r -- "$pid"'))
        self.assertIn('UNOWNED_PID=$(unowned_harness_pid || true)', source)
        self.assertNotIn("if pgrep -f '[c]iv6_play.py'", source)

    def test_a_shell_argument_cannot_delay_the_next_batch(self):
        """Only the candidate whose executable is Python is reported.

        Use real sleeping PIDs because the function deliberately performs
        `kill -0` before trusting any process-table answer.  The command and
        executable readers are tiny stand-ins for the macOS tools, letting the
        test model the original shell diagnostic and a real Python harness
        without starting a game.
        """
        source = self.SUPERVISOR.read_text()
        start = source.index("unowned_harness_pid() {")
        end = source.index("\n# The live display", start)
        detector = source[start:end]
        shell_candidate = subprocess.Popen(["/bin/zsh", "-c", "exec sleep 30"])
        harness_candidate = subprocess.Popen(["/bin/zsh", "-c", "exec sleep 30"])
        try:
            with TemporaryDirectory() as raw:
                tmp = Path(raw)
                bin_dir = tmp / "bin"
                bin_dir.mkdir()

                def fake(name: str, body: str) -> None:
                    path = bin_dir / name
                    path.write_text("#!/bin/zsh\n" + body)
                    path.chmod(0o755)

                fake("pgrep", f"print -r -- {shell_candidate.pid}\n"
                              f"print -r -- {harness_candidate.pid}\n")
                fake("ps", "if [[ \"$2\" == \"%s\" ]]; then\n"
                           "  print -r -- '/bin/zsh -c inspect civ6_play.py'\n"
                           "else\n"
                           "  print -r -- '/usr/local/bin/python3 -u /tmp/civ6_play.py'\n"
                           "fi\n" % shell_candidate.pid)
                fake("lsof", "if [[ \"$3\" == \"%s\" ]]; then\n"
                             "  print -r -- 'n/bin/zsh'\n"
                             "else\n"
                             "  print -r -- 'n/usr/local/bin/python3'\n"
                             "fi\n" % shell_candidate.pid)

                result = subprocess.run(
                    ["/bin/zsh", "-c", detector + "\nunowned_harness_pid\n"],
                    env={**os.environ, "PATH": f"{bin_dir}:{os.environ['PATH']}"},
                    capture_output=True, text=True, timeout=5)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), str(harness_candidate.pid))
        finally:
            for process in (shell_candidate, harness_candidate):
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=5)


class OvernightWatchdogHasNoImplicitDeadline(unittest.TestCase):
    def test_the_default_watchdog_is_unbounded_but_accepts_an_operator_deadline(self):
        source = (Path(__file__).resolve().parent / "ops" /
                  "civvis-overnight-watchdog.sh").read_text()
        self.assertIn(
            'AUDIT=${CIVVIS_OVERNIGHT_AUDIT:-${SELF_DIR}/civvis-overnight-audit.sh}',
            source)
        self.assertIn('STOP_AT=${CIVVIS_OVERNIGHT_STOP_AT:-}', source)
        self.assertIn('if [[ -n "$STOP_AT" ]]', source)
        self.assertIn('if [[ -n "$stop_epoch" ]] && (( now >= stop_epoch ))', source)


class OvernightAuditRecovery(unittest.TestCase):
    def test_a_live_legacy_supervisor_is_adopted_without_another_terminal(self):
        source = (Path(__file__).resolve().parent / "ops" /
                  "civvis-overnight-audit.sh").read_text()
        self.assertIn('supervisor_pid=$(live_supervisor_pid || true)', source)
        self.assertIn('host_state=supervisor-only', source)
        self.assertIn('/usr/bin/open -g -j -a Terminal "$HOST_LAUNCHER"', source)
        self.assertIn('/usr/bin/open -g -j -a Terminal "$MIRROR_KEEPER"', source)
        self.assertNotIn('/usr/bin/nohup /bin/zsh "$MIRROR_KEEPER"', source)
        self.assertIn(
            'if (( live_events && tiles_exported && ! mirror_healthy ))', source)
        self.assertLess(source.index('collect_mirror\nmirror_keeper_pid'),
                        source.index('if (( live_events && tiles_exported && ! mirror_healthy ))'))
        self.assertNotIn('tell application "Terminal" to do script', source)

    def test_a_healthy_primary_mirror_does_not_wait_for_an_absent_backup_keeper(self):
        source = (Path(__file__).resolve().parent / "ops" /
                  "civvis-overnight-audit.sh").read_text()
        self.assertIn('&& mirror_needs_settle; then', source)
        start = source.index('mirror_needs_settle() {')
        end = source.index('\n}', start) + 2
        predicate = source[start:end]
        cases = (("primary", 1, 1), ("", 1, 0), ("primary", 0, 0))
        shell = "/bin/zsh" if Path("/bin/zsh").exists() else "/bin/bash"
        for follower, healthy, expected in cases:
            result = subprocess.run(
                [shell, "-c",
                 f"{predicate}\nfollower_pid={follower!r}\n"
                 f"mirror_healthy={healthy}\nmirror_needs_settle"],
                capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, expected, result.stderr)


if __name__ == "__main__":
    unittest.main()


class WhenTheGameIsDeliberatelyHeld(KeeperTestCase):
    """A restart is this keeper's only remedy, so a cause no restart can reach
    has to stop it acting.

    An operator halt takes the game lock and sleeps. The ledger then goes stale
    exactly as a wedge does, and every supervisor started against it is refused
    at the lock having played nothing. Observed on `mbp-m5-pro-64`: a halt taken
    2026-08-02 was still in force on 2026-08-18, fifteen days of `blocked` rows
    that nobody attributed.
    """

    def held(self):
        return mock.patch.object(
            ladder_watchdog.gamelock, "standing_hold",
            lambda: "the game is held by pid 4242 under tag 'operator-halt', "
                    "and no harness is driving that tag")

    def test_a_held_game_is_not_answered_with_a_restart(self):
        with TemporaryDirectory() as raw, self.held():
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(tmp / "absent"),
                "--supervisor", "/x/supervisor.sh",
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 2)
            self.assertEqual(self.starts, [],
                             "started a loop that cannot take the game")

    def test_a_held_game_does_not_get_the_live_supervisor_killed_either(self):
        """The other arm: alive, not playing. Stopping it is disruptive and
        cannot help while the hold stands."""
        with TemporaryDirectory() as raw, self.held():
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(self.live_lock(tmp)),
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 2)
            self.assertEqual(self.stopped, [])
            self.assertEqual(self.starts, [])

    def test_the_hold_is_named_in_the_log_not_swallowed(self):
        """Standing down silently would reproduce the outage this tool ends."""
        with TemporaryDirectory() as raw, self.held():
            tmp = Path(raw)
            runs = ledger_with(tmp, [14.3])
            log = tmp / "log"
            ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(tmp / "absent"),
                "--state", str(tmp / "state.json"), "--log", str(log)])
            written = log.read_text()
            self.assertIn("operator-halt", written)
            self.assertIn("Release the hold", written)

    def test_a_healthy_ledger_is_still_left_alone_while_held(self):
        """A hold is not a problem to report; it only explains a stale ledger."""
        with TemporaryDirectory() as raw, self.held():
            tmp = Path(raw)
            runs = ledger_with(tmp, [0.2])
            log = tmp / "log"
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(self.live_lock(tmp)),
                "--state", str(tmp / "state.json"), "--log", str(log)])
            self.assertEqual(code, 0)
            self.assertFalse(log.exists() and log.read_text().strip())

    def test_an_unreadable_process_table_does_not_invent_a_hold(self):
        """`standing_hold` leans on `_tag_has_live_owner`, which fails CLOSED:
        with no readable `ps` it says a tag IS owned, so no hold is reported and
        the keeper behaves exactly as it did before this arm existed."""
        with mock.patch.object(ladder_watchdog.gamelock, "_processes",
                               lambda: []), \
             mock.patch.object(ladder_watchdog.gamelock, "_holder",
                               lambda: {"pid": os.getpid(), "tag": "whatever",
                                        "since": "2026-08-02T20:03:01Z"}):
            self.assertIsNone(ladder_watchdog.gamelock.standing_hold())
