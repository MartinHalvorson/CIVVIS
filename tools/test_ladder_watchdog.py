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

#: Held for the whole module run; removed when the interpreter exits.
_SANDBOX = TemporaryDirectory(prefix="civvis-test-gamelock-")


def setUpModule() -> None:
    """⚠ NEVER ASK THIS MACHINE WHETHER A GAME MAY START.

    `civ6_control/gamelock.py` resolves two paths in the operator's home
    directory at import time: the durable operator-halt marker
    (`~/.civvis-operator-halt.json`) and the live game lock
    (`~/.civvis-civ6-game.lock`). Both are correct in production, and both are
    real files on an operator's Mac — so a test that leaves them alone asks
    *this machine* whether a game may start.

    That is not hypothetical. A halt was recorded here on 2026-08-19T15:06Z and
    left in force; from that day seventeen tests in this file and eleven in
    `test_spectator_supervisor.py` failed on the operator's Mac and passed on
    the runner, because `gamelock.standing_hold()` was correctly reporting a
    halt no test had asked for. Nothing in the output named the cause, and a red
    suite nobody can explain is a suite people stop reading.

    So point both at an empty sandbox, through the two environment overrides
    `gamelock` already publishes for exactly this — which a subprocess a test
    spawns inherits — and at the module constants, which an already-imported
    `gamelock` reads. A test that wants a halt writes one into the sandbox; a
    test that wants none gets none, on every machine.
    """
    sandbox = Path(_SANDBOX.name)
    halt, lock = sandbox / "operator-halt.json", sandbox / "civ6-game.lock"
    intent = sandbox / "operator-intent"
    intent.write_text("running\n")
    os.environ["CIVVIS_OPERATOR_HALT_FILE"] = str(halt)
    os.environ["CIVVIS_GAME_LOCK_DIR"] = str(lock)
    os.environ["CIVVIS_OPERATOR_INTENT_FILE"] = str(intent)
    ladder_watchdog.gamelock.OPERATOR_HALT = halt
    ladder_watchdog.gamelock.LOCK = lock
    ladder_watchdog.gamelock.OPERATOR_INTENT = intent


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
    def test_a_stopped_operator_intent_prevents_a_terminal_start(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            intent = tmp / "intent"
            intent.write_text("stopped\n")
            log = tmp / "log"
            code = ladder_watchdog.main([
                "--runs", str(ledger_with(tmp, [14.3])), "--stale-hours", "3",
                "--lock", str(tmp / "absent"), "--intent-file", str(intent),
                "--state", str(tmp / "state.json"), "--log", str(log)])
            self.assertEqual(code, 0)
            self.assertEqual(self.starts, [])
            self.assertIn("operator intent is stopped", log.read_text())

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

    def test_an_explicit_halt_prevents_initial_supervisor_start(self):
        """A hidden Terminal host must not undo an operator's halt.

        The old host started a fresh supervisor before its first poll, so a
        live lock-based hold could race with the host into a new game.
        """
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            log = tmp / "host.log"
            started = tmp / "supervisor.started"
            lock = tmp / "host.lock"
            supervisor_lock = tmp / "supervisor.lock"
            supervisor = tmp / "civvis-game-supervisor.sh"
            supervisor.write_text(
                "#!/bin/zsh\n"
                f"print -r -- started > {started}\n")
            supervisor.chmod(0o755)
            gamelock = tmp / "gamelock.py"
            gamelock.write_text(
                "import sys\n"
                "if '--halt-status' in sys.argv:\n"
                "    print('the game is explicitly halted for this test')\n"
                "    raise SystemExit(0)\n"
                "raise SystemExit(1)\n")
            result = subprocess.run(
                ["/bin/zsh", str(self.HOST)],
                env={
                    **os.environ,
                    "CIVVIS_SUPERVISOR": str(supervisor),
                    "CIVVIS_GAMELOCK": str(gamelock),
                    "CIVVIS_INTERACTIVE_HOST_LOG": str(log),
                    "CIVVIS_INTERACTIVE_HOST_LOCK": str(lock),
                    "CIVVIS_SUPERVISOR_LOCK": str(supervisor_lock),
                },
                capture_output=True, text=True, timeout=5,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(started.exists(), result.stdout)
            self.assertIn("operator halt active; exiting before startup",
                          log.read_text())

    def test_every_restart_poll_checks_for_an_operator_halt_first(self):
        source = self.HOST.read_text()
        loop = source[source.index("while true; do"):]
        self.assertIn('if held=$(hold_status); then', loop)
        self.assertLess(loop.index('if held=$(hold_status); then'),
                        loop.index('if ! pid_is_live "$supervisor_pid"; then'))
        self.assertIn('operator halt active; stopping owned children and exiting',
                      loop)

    def test_the_host_stops_only_for_the_explicit_halt_never_a_standing_hold(self):
        """A live lock holder that drives no run is a report, not a halt.

        Between one attempt's exit and the next attempt's launch the batch loop
        holds the game lock under a tag no process yet carries, so
        `gamelock.py --hold-status` answers "held … no harness is driving that
        tag" for a few seconds. The host polls every five. When it acted on
        that answer it stopped its own supervisor and the game under it: four
        games on 2026-08-18/19 ended as `game exited` at t18/t44/t72/t83 within
        seconds of such a line. The host must ask `--halt-status` — the durable
        operator marker and nothing else — and a fake lock helper that answers
        yes to `--hold-status` and no to `--halt-status` must not stop the
        supervisor it started.
        """
        source = self.HOST.read_text()
        helper = source[source.index("hold_status() {"):source.index("release_lock() {")]
        self.assertIn('"$GAMELOCK" --halt-status', helper)
        self.assertNotIn('"$GAMELOCK" --hold-status', helper,
                         "the host must not act on the standing (transient) hold")
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            log = tmp / "host.log"
            started = tmp / "supervisor.started"
            lock = tmp / "host.lock"
            supervisor_lock = tmp / "supervisor.lock"
            supervisor = tmp / "civvis-game-supervisor.sh"
            supervisor.write_text(
                "#!/bin/zsh\n"
                f"print -r -- started > {started}\n"
                "sleep 30\n")
            supervisor.chmod(0o755)
            gamelock = tmp / "gamelock.py"
            gamelock.write_text(
                "import sys\n"
                "if '--hold-status' in sys.argv:\n"
                "    print('the game is held by pid 1 under tag x since now, "
                "and no harness is driving that tag')\n"
                "    raise SystemExit(0)\n"
                "raise SystemExit(1)\n")
            with subprocess.Popen(
                ["/bin/zsh", str(self.HOST)],
                env={
                    **os.environ,
                    "CIVVIS_SUPERVISOR": str(supervisor),
                    "CIVVIS_GAMELOCK": str(gamelock),
                    "CIVVIS_INTERACTIVE_HOST_LOG": str(log),
                    "CIVVIS_INTERACTIVE_HOST_LOCK": str(lock),
                    "CIVVIS_SUPERVISOR_LOCK": str(supervisor_lock),
                    "CIVVIS_INTERACTIVE_HOST_POLL_S": "0.2",
                    "CIVVIS_POPUP_KEEPER": str(tmp / "absent-popup-keeper.sh"),
                    "CIVVIS_MIRROR_KEEPER": str(tmp / "absent-mirror-keeper.sh"),
                },
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            ) as host:
                try:
                    deadline = time.monotonic() + 5.0
                    while time.monotonic() < deadline and not started.exists():
                        time.sleep(0.05)
                    self.assertTrue(started.exists(), "the host must start its supervisor")
                    time.sleep(1.5)
                    self.assertIsNone(host.poll(),
                                      "a standing hold must not make the host exit")
                    text = log.read_text()
                    self.assertNotIn("operator halt active", text)
                    self.assertNotIn("stopping owned children", text)
                finally:
                    host.terminate()
                    host.wait(timeout=5)

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

    def test_an_existing_popup_keeper_is_adopted_not_respawned(self):
        """A host restart must reuse the lock-owned clearer keeper.

        The supervisor can legitimately restart while the interactive host and
        its first popup keeper remain alive.  Starting another keeper then
        makes that second copy immediately exit on the keeper lock, which used
        to send the host into a five-second respawn loop.  The new host must
        adopt the live, correctly identified lock holder and leave it alone on
        host exit.
        """
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            log = tmp / "host.log"
            starts = tmp / "popup.starts"
            host_lock = tmp / "host.lock"
            supervisor_lock = tmp / "supervisor.lock"
            popup_lock = tmp / "popup.lock"
            popup_lock.mkdir()
            supervisor = tmp / "civvis-game-supervisor.sh"
            supervisor.write_text("#!/bin/zsh\nwhile true; do sleep 1; done\n")
            supervisor.chmod(0o755)
            popup_keeper = tmp / "popup-keeper.sh"
            popup_keeper.write_text(
                "#!/bin/zsh\n"
                "print -r -- started >> \"$POPUP_STARTS\"\n"
                "while true; do sleep 1; done\n")
            popup_keeper.chmod(0o755)
            gamelock = tmp / "gamelock.py"
            gamelock.write_text("raise SystemExit(1)\n")
            external = subprocess.Popen(
                ["/bin/zsh", str(popup_keeper)],
                env={**os.environ, "POPUP_STARTS": str(starts)},
            )
            host = None
            try:
                (popup_lock / "pid").write_text(f"{external.pid}\n")
                host = subprocess.Popen(
                    ["/bin/zsh", str(self.HOST)],
                    env={
                        **os.environ,
                        "CIVVIS_SUPERVISOR": str(supervisor),
                        "CIVVIS_POPUP_KEEPER": str(popup_keeper),
                        "CIVVIS_POPUP_KEEPER_LOCK": str(popup_lock),
                        "CIVVIS_MIRROR_KEEPER": str(tmp / "absent-mirror-keeper.sh"),
                        "CIVVIS_WEDGE_WATCHDOG": str(tmp / "absent-watchdog.sh"),
                        "CIVVIS_GAMELOCK": str(gamelock),
                        "CIVVIS_INTERACTIVE_HOST_LOG": str(log),
                        "CIVVIS_INTERACTIVE_HOST_LOCK": str(host_lock),
                        "CIVVIS_SUPERVISOR_LOCK": str(supervisor_lock),
                        "CIVVIS_WEDGE_LOCK": str(tmp / "wedge.lock"),
                        "CIVVIS_INTERACTIVE_HOST_POLL_S": "0.2",
                        "POPUP_STARTS": str(starts),
                    },
                )
                deadline = time.monotonic() + 5
                expected = f"adopted popup keeper pid {external.pid}"
                while time.monotonic() < deadline:
                    if log.exists() and expected in log.read_text():
                        break
                    time.sleep(0.05)
                self.assertTrue(log.exists(), "host did not write a health record")
                self.assertIn(expected, log.read_text())
                self.assertEqual(starts.read_text().splitlines(), ["started"],
                                 "the host must not launch a duplicate keeper")
                host.terminate()
                host.wait(timeout=5)
                self.assertIsNone(external.poll(),
                                  "the host must not terminate an adopted popup keeper")
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

    def _owned_detector(self, source: str) -> str:
        start = source.index("supervisor_owns_process() {")
        return source[start:source.index("\n# A `pgrep` candidate", start)]

    def test_an_inherited_lock_holder_is_not_our_orphan(self):
        """A fresh supervisor leaves a live inherited harness alone.

        The game lock is global and only proves exclusive access to Civ VI. It
        cannot prove that the harness belongs to the supervisor process that
        has just started, so model a live Python harness whose parent sits
        outside the shell under test.
        """
        detector = self._owned_detector(self.SUPERVISOR.read_text())
        harness = subprocess.Popen(["/bin/zsh", "-c", "exec sleep 30"])
        try:
            with TemporaryDirectory() as raw:
                tmp = Path(raw)
                home = tmp / "home"
                lock = home / ".civvis-civ6-game.lock"
                lock.mkdir(parents=True)
                (lock / "holder.json").write_text(
                    '{\n  "pid": %d\n}\n' % harness.pid)
                fake_ps = (
                    "ps() {\n"
                    "  if [[ \"$4\" == \"command=\" ]]; then\n"
                    "    print -r -- '/usr/local/bin/python3 -u /tmp/civ6_play.py'\n"
                    "  elif [[ \"$4\" == \"ppid=\" ]]; then\n"
                    "    print -r -- 1\n"
                    "  fi\n"
                    "}\n")
                result = subprocess.run(
                    ["/bin/zsh", "-c",
                     fake_ps + detector + "\nowned_harness_pid\n"],
                    env={**os.environ, "HOME": str(home)},
                    capture_output=True, text=True, timeout=5)

            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertEqual(result.stdout, "")
        finally:
            if harness.poll() is None:
                harness.terminate()
                harness.wait(timeout=5)

    def test_a_child_harness_remains_owned_by_its_supervisor(self):
        """The guard preserves the normal supervisor-to-climb ownership path."""
        detector = self._owned_detector(self.SUPERVISOR.read_text())
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            fake_ps = (
                "ps() {\n"
                "if [[ \"$4\" == \"command=\" ]]; then\n"
                "  print -r -- '/usr/local/bin/python3 -u /tmp/civ6_play.py'\n"
                "elif [[ \"$4\" == \"ppid=\" ]]; then\n"
                "  if [[ \"$2\" == \"$SUPERVISOR_TEST_HARNESS\" ]]; then\n"
                "    print -r -- \"$SUPERVISOR_TEST_PARENT\"\n"
                "  else\n"
                "    print -r -- 1\n"
                "  fi\n"
                "fi\n"
                "}\n")
            home = tmp / "home"
            script = (
                fake_ps
                + "sleep 30 &\n"
                "harness=$!\n"
                "mkdir -p \"$HOME/.civvis-civ6-game.lock\"\n"
                "{\n"
                "  print -r -- '{'\n"
                "  print -r -- \"  \\\"pid\\\": $harness\"\n"
                "  print -r -- '}'\n"
                "} > \"$HOME/.civvis-civ6-game.lock/holder.json\"\n"
                "export SUPERVISOR_TEST_HARNESS=\"$harness\"\n"
                "export SUPERVISOR_TEST_PARENT=\"$$\"\n"
                + detector + "\n"
                + "owned_harness_pid\n"
                "rc=$?\n"
                "kill \"$harness\" 2>/dev/null || true\n"
                "wait \"$harness\" 2>/dev/null || true\n"
                "exit \"$rc\"\n")
            result = subprocess.run(
                ["/bin/zsh", "-c", script],
                env={**os.environ, "HOME": str(home)},
                capture_output=True, text=True, timeout=5)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.strip().isdigit(), result.stdout)

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

    def test_launch_boundary_rechecks_after_preflight(self):
        """A new owner can appear while the supervisor builds or refreshes.

        The initial ownership scan cannot protect the whole preflight window:
        an independent live game may start after that scan but before the
        climb is launched.  Keep the second scan immediately after mirror
        refresh so the supervisor does not spend a no-turn cycle colliding
        with that owner.
        """
        source = self.SUPERVISOR.read_text()
        mirror_end = source.index("\n  LAUNCH_UNOWNED_PID=", source.index("display mirror"))
        launch_check_end = source.index("\n  TAG=$(date -u", mirror_end)
        boundary = source[mirror_end:launch_check_end]

        self.assertIn(
            "LAUNCH_UNOWNED_PID=$(unowned_harness_pid || true)", boundary)
        self.assertIn(
            "an unowned Civ VI harness appeared during preflight", boundary)
        self.assertIn("sleep 60", boundary)
        self.assertIn("continue", boundary)
        self.assertLess(
            boundary.index("LAUNCH_UNOWNED_PID="), boundary.index("sleep 60"))


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

    def test_a_held_game_is_not_started_against_on_a_fresh_ledger_either(self):
        """The arm a halt actually lands on.

        A halt arrives while the ledger is still FRESH, so for the first
        `--stale-hours` after one it is the no-supervisor arm that runs, not
        the stale arm that owned this guard. It asked nothing, so it opened a
        Terminal host every cooldown that read the halt and exited having
        played no turn — nine on 2026-08-19, two more on 2026-08-20.
        """
        with TemporaryDirectory() as raw, self.held():
            tmp = Path(raw)
            runs = ledger_with(tmp, [0.2])
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(tmp / "absent"),
                "--supervisor", "/x/supervisor.sh",
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 2)
            self.assertEqual(self.starts, [],
                             "opened a window that cannot take the game")
            self.assertIn("HELD no supervisor is running",
                          (tmp / "log").read_text())

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
            self.assertIn("not starting or stopping anything", written)

    def test_a_stopped_intent_is_not_answered_with_a_restart(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            intent = ladder_watchdog.gamelock.OPERATOR_INTENT
            intent.write_text("stopped\n")
            self.addCleanup(lambda: intent.write_text("running\n"))
            runs = ledger_with(tmp, [14.3])
            code = ladder_watchdog.main([
                "--runs", str(runs), "--stale-hours", "3",
                "--lock", str(tmp / "absent"),
                "--supervisor", "/x/supervisor.sh",
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 2)
            self.assertEqual(self.starts, [])
            self.assertIn("verification intent is 'stopped'", (tmp / "log").read_text())

    def test_a_missing_intent_is_fail_closed(self):
        with TemporaryDirectory() as raw:
            tmp = Path(raw)
            intent = ladder_watchdog.gamelock.OPERATOR_INTENT
            intent.unlink()
            self.addCleanup(lambda: intent.write_text("running\n"))
            runs = ledger_with(tmp, [0.2])
            code = ladder_watchdog.main([
                "--runs", str(runs), "--lock", str(tmp / "absent"),
                "--state", str(tmp / "state.json"), "--log", str(tmp / "log")])
            self.assertEqual(code, 2)
            self.assertEqual(self.starts, [])
            self.assertIn("verification intent is missing or unreadable", (tmp / "log").read_text())

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



class TheSuiteReadsASandboxNotThisMachine(unittest.TestCase):
    """⚠ THE GUARD ON `setUpModule`. Delete that sandbox and this file goes
    green on a runner with no halt marker and red only on the operator's Mac —
    the failure it exists to prevent, returning silently. This says so on every
    machine instead, including a clean one."""

    def test_the_halt_marker_and_the_game_lock_live_in_the_sandbox(self):
        sandbox = Path(_SANDBOX.name)
        for name, path in (("OPERATOR_HALT", ladder_watchdog.gamelock.OPERATOR_HALT),
                           ("LOCK", ladder_watchdog.gamelock.LOCK)):
            self.assertTrue(
                path.is_relative_to(sandbox),
                f"gamelock.{name} is {path}, outside the test sandbox: this suite "
                f"is reading the machine it runs on. See setUpModule.")
        self.assertEqual(os.environ.get("CIVVIS_OPERATOR_HALT_FILE"),
                         str(ladder_watchdog.gamelock.OPERATOR_HALT),
                         "a subprocess a test spawns must inherit the same sandbox")
        self.assertTrue(
            ladder_watchdog.gamelock.OPERATOR_INTENT.is_relative_to(sandbox),
            "gamelock.OPERATOR_INTENT is outside the test sandbox")
        self.assertEqual(os.environ.get("CIVVIS_OPERATOR_INTENT_FILE"),
                         str(ladder_watchdog.gamelock.OPERATOR_INTENT),
                         "a subprocess a test spawns must inherit the intent sandbox")
        self.assertEqual(os.environ.get("CIVVIS_GAME_LOCK_DIR"),
                         str(ladder_watchdog.gamelock.LOCK))


class ASupervisorThatIsPlayingTurnsIsNotWedged(unittest.TestCase):
    """⚠⚠ The staleness signal cannot see a game in progress.

    `civ6_ladder.staleness_problem` asks when a game last *finished*. A
    250-turn game at Online speed takes hours — routinely longer than
    `--stale-hours` — so a healthy long game is indistinguishable from a wedge
    by that question alone. On 2026-08-28 the first live game in nine days
    reached turn 41 while the keeper counted down a two-hour cooldown, and the
    tick after it expired would have SIGTERMed the supervisor playing it.
    """

    @staticmethod
    def _runs(root: Path, quiet_s: float) -> Path:
        runs = root / "control"
        (runs / "civvis-20260828T122324Z").mkdir(parents=True)
        events = runs / "civvis-20260828T122324Z" / "events.jsonl"
        events.write_text('{"kind":"turn","turn":41}\n')
        stamp = time.time() - quiet_s
        os.utime(events, (stamp, stamp))
        return runs

    def test_a_run_writing_events_reads_as_playing(self):
        with TemporaryDirectory() as tmp:
            runs = self._runs(Path(tmp), quiet_s=2)
            quiet = ladder_watchdog.newest_attempt_activity_s(runs)
            self.assertIsNotNone(quiet)
            self.assertLess(quiet, ladder_watchdog.LIVE_EVENT_QUIET_SECONDS)

    def test_a_run_that_has_gone_quiet_does_not_shield_a_wedge(self):
        with TemporaryDirectory() as tmp:
            runs = self._runs(Path(tmp), quiet_s=4000)
            quiet = ladder_watchdog.newest_attempt_activity_s(runs)
            self.assertGreater(quiet, ladder_watchdog.LIVE_EVENT_QUIET_SECONDS)

    def test_no_runs_at_all_is_not_a_claim_that_one_is_playing(self):
        with TemporaryDirectory() as tmp:
            self.assertIsNone(
                ladder_watchdog.newest_attempt_activity_s(Path(tmp)))

    def test_the_newest_run_is_the_one_that_counts(self):
        """An old finished run must not make a quiet new one look alive."""
        with TemporaryDirectory() as tmp:
            runs = self._runs(Path(tmp), quiet_s=4000)
            stale = runs / "civvis-20260801T000000Z"
            stale.mkdir()
            fresh_events = stale / "events.jsonl"
            fresh_events.write_text('{"kind":"turn"}\n')
            recent = time.time() - 5
            os.utime(fresh_events, (recent, recent))
            # The most recently written events file wins, whatever its name.
            self.assertLess(ladder_watchdog.newest_attempt_activity_s(runs), 60)

    def test_the_stale_arm_leaves_a_playing_supervisor_alone(self):
        """The whole point: a stale ledger plus a live run is not a stop."""
        source = (Path(__file__).resolve().parent / "ops"
                  / "ladder_watchdog.py").read_text(encoding="utf-8")
        self.assertIn("quiet = newest_attempt_activity_s(runs)", source)
        self.assertIn("is playing, ", source)
        # The guard must sit BEFORE the stop, not after it.
        self.assertLess(source.index("quiet = newest_attempt_activity_s(runs)"),
                        source.index("ok, detail = stop_supervisor(alive)"))


if __name__ == "__main__":
    unittest.main()
