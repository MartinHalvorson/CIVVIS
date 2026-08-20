"""The climb loop's attempt accounting.

⚠ THIS IS THE FIRST TEST OF ANY civ6_* TOOL, and it exists because the harness spent
eleven of twenty-four attempts in two minutes on a machine where Steam had exited,
then printed "no win in the attempts given". Nothing was wrong with the game, the
mod, or CIVVIS. The loop counted ITERATIONS and reported them as GAMES.

The property under test is one sentence: an attempt that produced no turn does not
spend a rung of the budget. Everything below is that sentence from a different angle.
"""

from pathlib import Path
import collections
import json
import re
import sys
import tempfile
import time
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_civvis_climb as climb


class BusyOnlyCountsARealGame(unittest.TestCase):
    """`pgrep -f` matches command lines, so anything that NAMES the harness hits.

    Twice on 2026-08-19 the lane stalled on this: a leftover agent shell whose
    argv carried `civ6_play|civ6_brain` (from a grep) made every attempt die
    with "something already holds the game; refusing to stop an unowned run",
    while the lock file was empty, no harness ran and Civ 6 was down. The second
    time the shell belonged to a DIFFERENT agent session, which this process
    must not kill. Verify the executable instead, exactly as
    `civvis-game-supervisor.sh` already does for its own scan.
    """

    def _lsof(self, path):
        return f"p1\nftxt\nn{path}\n"

    def test_a_shell_that_merely_names_the_harness_is_not_a_game(self):
        with mock.patch.object(climb, "run", side_effect=[
            "4242\n",                       # pgrep hit
            self._lsof("/bin/zsh"),          # ... but it is a shell
        ]):
            self.assertIsNone(climb.busy(),
                              "a shell carrying the name in its argv is not a game")

    def test_the_python_harness_still_counts(self):
        with mock.patch.object(climb, "run", side_effect=[
            "4242\n",
            self._lsof("/Library/Frameworks/Python.framework/Versions/3.9/.../Python"),
        ]):
            self.assertEqual(climb.busy(), "4242")

    def test_the_game_itself_still_counts(self):
        with mock.patch.object(climb, "run", side_effect=[
            "77\n",
            self._lsof("/Users/x/Steam/common/Sid Meier's Civilization VI/Civ6.app/Content"),
        ]):
            self.assertEqual(climb.busy(), "77")

    def test_an_unresolvable_pid_is_treated_as_real(self):
        """Refusing to touch an unproven game is the safe direction."""
        with mock.patch.object(climb, "run", side_effect=["9\n", ""]):
            self.assertEqual(climb.busy(), "9")

    def test_a_real_game_survives_a_shell_in_the_same_sweep(self):
        with mock.patch.object(climb, "run", side_effect=[
            "4242 77\n",
            self._lsof("/bin/zsh"),
            self._lsof("/Users/x/Civ6.app/Contents/MacOS/Civ6_Exe"),
        ]):
            self.assertEqual(climb.busy(), "77")


class MirrorFreshnessTests(unittest.TestCase):
    def test_follower_output_path_reads_lsof_file_field(self):
        mirror_log = climb.MIRROR_FOLLOW_LOG
        with mock.patch.object(
            climb, "run", return_value=f"p101\nf1\nn{mirror_log}\n"
        ) as run:
            self.assertEqual(
                climb.follower_output_path(101),
                mirror_log,
            )

        run.assert_called_once_with(["lsof", "-a", "-p", "101", "-d", "1", "-Fn"])

    def test_follower_owns_mirror_requires_the_dedicated_runtime_output(self):
        with mock.patch.object(
            climb, "follower_output_path", return_value=climb.MIRROR_FOLLOW_LOG
        ):
            self.assertTrue(climb.follower_owns_mirror(101))
        with mock.patch.object(
            climb, "follower_output_path", return_value=Path("/tmp/other-follow.log")
        ):
            self.assertFalse(climb.follower_owns_mirror(102))

    def test_owned_mirror_pids_uses_runtime_output_and_visible_port(self):
        """A different worktree's follower must survive this batch's refresh."""
        with mock.patch.object(
            climb, "matching_pids", side_effect=[[101, 102], [201, 202]]
        ), mock.patch.object(
            climb, "follower_owns_mirror", side_effect=[True, False]
        ), mock.patch.object(
            climb, "mirror_listener_pids", return_value={202}
        ):
            self.assertEqual(climb.owned_mirror_pids(), [101, 202])

    def test_retire_mirror_stops_the_follower_and_its_detached_server(self):
        """A new batch cannot inherit a mirror process from another build."""
        with mock.patch.object(climb, "owned_mirror_pids", return_value=[101, 202]), \
             mock.patch.object(climb, "process_running", return_value=False), \
             mock.patch.object(climb.os, "kill") as kill:
            self.assertEqual(climb.retire_mirror(), [101, 202])

        self.assertEqual(
            kill.call_args_list,
            [mock.call(101, climb.signal.SIGTERM), mock.call(202, climb.signal.SIGTERM)],
        )

    def test_ensure_mirror_always_starts_a_follower_from_this_checkout(self):
        """A live PID alone proves nothing about which revision it loaded."""
        with mock.patch.object(climb, "retire_mirror", return_value=[101, 202]) as retire, \
             mock.patch.object(climb, "_detach") as detach:
            climb.ensure_mirror()

        retire.assert_called_once_with()
        detach.assert_called_once_with(
            [climb.sys.executable, "-u", str(climb.HERE / "follow.py")],
            climb.MIRROR_FOLLOW_LOG,
            "mirror",
        )


class TeardownOwnershipTests(unittest.TestCase):
    """The ladder must never clean up a run whose ownership it cannot prove."""

    TAG = "civvis-MINE"

    def test_main_refuses_a_busy_game_before_preflight_or_teardown(self):
        """The startup guard is the first defence against a foreign run."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            orders_bin = root / "civvis_orders"
            orders_bin.write_text("#!/bin/sh\n")
            argv = ["civ6_civvis_climb.py", "--orders-bin", str(orders_bin),
                    "--logs", str(root / "logs")]
            with mock.patch.object(climb.sys, "argv", argv), \
                 mock.patch.object(climb, "busy", return_value="456\n"), \
                 mock.patch.object(climb, "teardown") as teardown, \
                 mock.patch.object(climb, "run") as run:
                self.assertEqual(3, climb.main())

        teardown.assert_not_called()
        run.assert_not_called()

    def test_an_untagged_cleanup_does_not_touch_a_running_game(self):
        with mock.patch.object(climb, "busy", return_value="123\n"), \
             mock.patch.object(climb, "run") as run:
            self.assertFalse(climb.teardown())

        run.assert_not_called()

    def test_a_foreign_installed_tag_is_preserved_without_acquiring_or_stopping(self):
        with mock.patch.object(climb, "installed_run_tag", return_value="civvis-OTHER"), \
             mock.patch.object(climb.gamelock, "acquire") as acquire, \
             mock.patch.object(climb.launcher, "stop") as stop, \
             mock.patch.object(climb.install, "clear_run_tag") as clear:
            self.assertFalse(climb.teardown(self.TAG))

        acquire.assert_not_called()
        stop.assert_not_called()
        clear.assert_not_called()

    def test_a_matching_tag_waits_for_its_live_controller_instead_of_killing_it(self):
        with mock.patch.object(climb, "installed_run_tag", return_value=self.TAG), \
             mock.patch.object(climb.gamelock, "acquire", return_value=False) as acquire, \
             mock.patch.object(climb, "run") as run, \
             mock.patch.object(climb.launcher, "stop") as stop:
            self.assertFalse(climb.teardown(self.TAG))

        acquire.assert_called_once_with(self.TAG)
        run.assert_not_called()
        stop.assert_not_called()

    def test_matching_orphan_is_locked_then_stopped_and_cleared(self):
        with mock.patch.object(climb, "installed_run_tag", return_value=self.TAG), \
             mock.patch.object(climb.gamelock, "acquire", return_value=True) as acquire, \
             mock.patch.object(climb.gamelock, "release") as release, \
             mock.patch.object(climb, "busy", side_effect=["456\n", None]), \
             mock.patch.object(climb, "run") as run, \
             mock.patch.object(climb.time, "sleep"), \
             mock.patch.object(climb.launcher, "stop", return_value=True) as stop, \
             mock.patch.object(climb.install, "clear_run_tag", return_value=True) as clear, \
             mock.patch.object(climb, "dismiss_crash_dialogs") as dismiss:
            self.assertTrue(climb.teardown(self.TAG))

        acquire.assert_called_once_with(self.TAG)
        self.assertEqual(2, run.call_count, "only tag-specific controller sweeps")
        for call in run.call_args_list:
            self.assertEqual(["pkill", "-f"], call.args[0][:2])
            self.assertIn(re.escape(self.TAG), call.args[0][2])
        stop.assert_called_once_with(timeout_s=45.0)
        clear.assert_called_once_with()
        dismiss.assert_called_once_with()
        release.assert_called_once_with()

    def test_tag_change_after_lock_refuses_before_any_process_is_stopped(self):
        with mock.patch.object(climb, "installed_run_tag",
                               side_effect=[self.TAG, "civvis-OTHER"]), \
             mock.patch.object(climb.gamelock, "acquire", return_value=True), \
             mock.patch.object(climb.gamelock, "release") as release, \
             mock.patch.object(climb, "run") as run, \
             mock.patch.object(climb.launcher, "stop") as stop:
            self.assertFalse(climb.teardown(self.TAG))

        run.assert_not_called()
        stop.assert_not_called()
        release.assert_called_once_with()


class _NoWait:
    """The `time` module with `sleep` removed, and nothing else changed."""

    def __init__(self, real):
        self._real = real

    def sleep(self, seconds):
        pass

    def __getattr__(self, name):
        return getattr(self._real, name)


class FakeProc:
    """A play/brain process that starts, is waited on, and is already finished."""

    def __init__(self, *args, **kwargs):
        pass

    def wait(self, timeout=None):
        return 0

    def poll(self):
        return 0

    def send_signal(self, sig):
        pass


class _Harness:
    """Shared rig. NOT a TestCase — subclassing one re-runs all of its tests."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.logs = root / "logs"
        self.logs.mkdir()
        self.runs = root / "runs"
        self.runs.mkdir()
        self.ledger = root / "ladder.jsonl"
        self.orders_bin = root / "civvis_orders"
        self.orders_bin.write_text("#!/bin/sh\n")

        self.saved = {name: getattr(climb, name) for name in
                      ("LEDGER", "BLOCKED_BACKOFF_S", "teardown", "busy",
                       "wake_steam", "outcome_of", "code_state", "RUN_ROOT")}
        climb.LEDGER = self.ledger
        climb.RUN_ROOT = self.runs
        climb.BLOCKED_BACKOFF_S = (0.0, 0.0, 0.0)   # the table, without the waiting
        climb.teardown = lambda *args, **kwargs: None
        climb.busy = lambda: None
        self.woke = []
        climb.wake_steam = lambda: self.woke.append(1)

        self.saved_popen = climb.subprocess.Popen
        climb.subprocess.Popen = FakeProc
        self.saved_run = climb.run
        climb.run = lambda cmd, timeout=60.0: "deadbeef"
        # One program unless a test says otherwise; `code_state` gets its own suite.
        climb.code_state = lambda: "deadbeef"

        # The harness waits three seconds between starting the game and starting the
        # brain, which is right for a real launch and is pure dead time here. Without
        # this the suite runs for a minute, and a slow test is a test that gets
        # skipped — which is how the accounting defect survived in the first place.
        self.saved_time = climb.time
        climb.time = _NoWait(climb.time)

        self.steam = True
        self.saved_steam = climb.launcher.steam_running
        self.saved_binary = climb.launcher.game_binary
        climb.launcher.steam_running = lambda: self.steam
        binary = root / "Civ6"
        binary.write_text("")
        climb.launcher.game_binary = lambda: binary

    def tearDown(self):
        for name, value in self.saved.items():
            setattr(climb, name, value)
        climb.subprocess.Popen = self.saved_popen
        climb.run = self.saved_run
        climb.time = self.saved_time
        climb.launcher.steam_running = self.saved_steam
        climb.launcher.game_binary = self.saved_binary
        self.tmp.cleanup()

    def climb_with(self, outcomes, attempts=3, argv_extra=(), revs=None):
        """Run main() with a scripted sequence of per-attempt outcomes.

        `revs` scripts what `code_state()` reports on successive calls, which is how
        a commit landing mid-batch is reproduced without one.
        """
        seq = list(outcomes)
        if revs is not None:
            pending = list(revs)
            climb.code_state = lambda: pending.pop(0) if pending else "deadbeef"

        def outcome_of(tag):
            # The play log is what a blocked row quotes, so write one like the real
            # harness would before reading it back.
            record = seq.pop(0) if seq else {"last_turn": None}
            return dict(record)

        climb.outcome_of = outcome_of
        argv = sys.argv
        sys.argv = ["climb", "--attempts", str(attempts),
                    "--orders-bin", str(self.orders_bin),
                    "--logs", str(self.logs), "--timeout", "0.1", *argv_extra]
        try:
            code = climb.main()
        finally:
            sys.argv = argv
        rows = [json.loads(line) for line in
                self.ledger.read_text().splitlines()] if self.ledger.exists() else []
        return code, rows

class DeciderRebuildTests(unittest.TestCase):
    """The climb rebuilds this checkout's own decider before it plays; a supplied
    binary is left alone. See `refresh_orders_binary`."""

    def test_a_supplied_binary_is_not_rebuilt(self):
        with tempfile.TemporaryDirectory() as tmp:
            supplied = Path(tmp) / "civvis_orders"
            supplied.write_bytes(b"x")
            with mock.patch.object(climb.subprocess, "run") as run:
                note = climb.refresh_orders_binary(supplied)
            run.assert_not_called()
            self.assertIn("supplied binary", note)

    def test_the_default_binary_is_rebuilt_with_cargo_release(self):
        default = climb.HERE.parent / "target" / "release" / "civvis_orders"
        with mock.patch.object(climb.subprocess, "run") as run, \
                mock.patch.object(climb, "code_state", lambda: "cafef00d"):
            run.return_value = mock.Mock(returncode=0, stdout="", stderr="")
            note = climb.refresh_orders_binary(default)
        run.assert_called_once()
        cmd = run.call_args.args[0]
        self.assertEqual(cmd[:3], ["cargo", "build", "--release"])
        self.assertIn("--locked", cmd)
        self.assertEqual(cmd[-2:], ["--bin", "civvis_orders"])
        self.assertEqual(run.call_args.kwargs["cwd"], str(climb.HERE.parent))
        self.assertIn("cafef00d", note)

    def test_a_failed_rebuild_is_named_and_the_batch_goes_on(self):
        default = climb.HERE.parent / "target" / "release" / "civvis_orders"
        with mock.patch.object(climb.subprocess, "run") as run:
            run.return_value = mock.Mock(returncode=101, stdout="", stderr="error: boom")
            note = climb.refresh_orders_binary(default)
        self.assertIn("FAILED", note)
        self.assertIn("boom", note)

    def test_no_build_skips_the_rebuild(self):
        default = climb.HERE.parent / "target" / "release" / "civvis_orders"
        with mock.patch.object(climb.subprocess, "run") as run:
            note = climb.refresh_orders_binary(default, enabled=False)
        run.assert_not_called()
        self.assertIn("--no-build", note)


class ClimbBudgetTests(_Harness, unittest.TestCase):
    # ---- the regression itself -------------------------------------------------

    def test_dead_steam_spends_no_attempts(self):
        """The 2026-07-31 batch: Steam gone, and the budget must survive untouched."""
        self.steam = False
        code, rows = self.climb_with([], attempts=3)

        self.assertEqual(code, 3, "a batch with no games played is not a loss")
        self.assertEqual(rows, [], "a game that never started is not a ledger row")
        self.assertTrue(self.woke, "it should try to bring Steam back")

    def test_blocked_start_never_reaches_the_game(self):
        """No run directory, no logs, no mod sync — the gate is BEFORE all of it."""
        self.steam = False
        self.climb_with([], attempts=3)
        self.assertEqual(list(self.logs.iterdir()), [],
                         "a blocked start must not leave empty logs behind")

    # ---- the same judgement, made after the fact --------------------------------

    def test_a_run_with_no_turn_is_a_hole_not_a_loss(self):
        """Steam can die AFTER the gate passes; the record has to be judged too."""
        code, rows = self.climb_with(
            [{"last_turn": None}, {"last_turn": None}, {"last_turn": None}],
            attempts=3)

        self.assertEqual(code, 3)
        self.assertEqual(len(rows), 3, "the holes are still written down")
        for row in rows:
            self.assertIsNone(row["attempt"], "a hole holds no attempt number")
            self.assertIn("blocked", row)

    def test_a_hole_carries_the_reason_it_failed(self):
        (self.logs / "x").write_text("")   # keep the dir; tag names are timestamps
        code, rows = self.climb_with([{"last_turn": None}], attempts=1)
        self.assertTrue(rows[0]["blocked"], "an unexplained hole helps nobody")

    # ---- and a real game still counts exactly once -------------------------------

    def test_measured_runs_spend_exactly_one_rung_each(self):
        code, rows = self.climb_with(
            [{"last_turn": 190, "last_score": 267},
             {"last_turn": 240, "last_score": 119},
             {"last_turn": 104, "last_score": 140}],
            attempts=3)

        self.assertEqual(code, 1, "played out, no win")
        self.assertEqual([r["attempt"] for r in rows], [1, 2, 3])

    def test_holes_do_not_renumber_the_attempts_around_them(self):
        """A batch is compared row to row later; the numbering has to stay honest."""
        code, rows = self.climb_with(
            [{"last_turn": 190}, {"last_turn": None}, {"last_turn": 240}],
            attempts=2)

        played = [r["attempt"] for r in rows if r["attempt"] is not None]
        self.assertEqual(played, [1, 2], "the hole did not consume attempt 2")
        self.assertEqual(code, 1)

    def test_a_blocked_streak_is_broken_by_a_real_game(self):
        """Three holes then a game must not trip the give-up bound (table is 3)."""
        code, rows = self.climb_with(
            [{"last_turn": None}, {"last_turn": None}, {"last_turn": 190},
             {"last_turn": None}, {"last_turn": 240}],
            attempts=2)

        self.assertEqual(code, 1, "the streak reset, so it kept playing")
        self.assertEqual([r["attempt"] for r in rows if r["attempt"]], [1, 2])


class FrozenBuildTests(_Harness, unittest.TestCase):
    """A batch is a comparison, so every row in it has to be the same program.

    ⚠ THE REAL CASE: the ladder was deliberately frozen at `1ee5dcb` with eleven
    attempts queued, a commit landed at 12:45, and attempts 14 onward silently ran
    `23878b2` under the same column headings. `code_rev` recorded the change and
    nothing enforced the freeze — it existed only as an intention.
    """

    def test_a_commit_mid_batch_ends_the_batch(self):
        code, rows = self.climb_with(
            [{"last_turn": 191, "last_score": 267},
             {"last_turn": 240, "last_score": 119}],
            attempts=6,
            # pin, attempt 1, attempt 2, then the 12:45 commit lands
            revs=["1ee5dcb", "1ee5dcb", "1ee5dcb", "23878b2"])

        self.assertEqual(code, 4, "a changed build is not a played-out batch")
        self.assertEqual([r["code_rev"] for r in rows], ["1ee5dcb", "1ee5dcb"])
        self.assertEqual(len(rows), 2, "no row was written for the new program")

    def test_no_pin_lets_revisions_mix_when_asked(self):
        code, rows = self.climb_with(
            [{"last_turn": 191}, {"last_turn": 240}],
            attempts=2, argv_extra=("--no-pin",),
            revs=["1ee5dcb", "23878b2"])

        self.assertEqual(code, 1, "played out")
        self.assertEqual([r["code_rev"] for r in rows], ["1ee5dcb", "23878b2"])

    def test_an_unchanged_build_runs_the_whole_batch(self):
        code, rows = self.climb_with(
            [{"last_turn": 191}, {"last_turn": 240}, {"last_turn": 104}],
            attempts=3, revs=["1ee5dcb"] * 8)

        self.assertEqual(code, 1)
        self.assertEqual(len(rows), 3)

    def test_a_dirty_tree_that_changes_also_ends_the_batch(self):
        """An edit to uncommitted work is a new program too, and `+dirty` hid it."""
        code, rows = self.climb_with(
            [{"last_turn": 191}], attempts=3,
            revs=["1ee5dcb+9f2c1e04", "1ee5dcb+9f2c1e04", "1ee5dcb+a7b0c331"])

        self.assertEqual(code, 4)
        self.assertEqual(len(rows), 1)


class CodeStateTests(unittest.TestCase):
    """`rev+dirty` cannot tell two uncommitted states apart. The fingerprint can."""

    def setUp(self):
        self.saved_run = climb.run

    def tearDown(self):
        climb.run = self.saved_run

    def _state(self, rev, diff="", status=""):
        def fake(cmd, timeout=60.0):
            if "rev-parse" in cmd:
                return rev
            return status if "status" in cmd else diff
        climb.run = fake
        return climb.code_state()

    def test_a_clean_tree_is_named_by_its_revision_alone(self):
        self.assertEqual(self._state("1ee5dcb\n"), "1ee5dcb")

    def test_two_different_dirty_trees_get_different_names(self):
        one = self._state("1ee5dcb\n", diff="diff --git a/x b/x\n+one\n")
        two = self._state("1ee5dcb\n", diff="diff --git a/x b/x\n+two\n")
        self.assertNotEqual(one, two, "this is the whole point of the fingerprint")
        self.assertTrue(one.startswith("1ee5dcb+"))

    def test_the_same_dirty_tree_keeps_its_name(self):
        diff = "diff --git a/x b/x\n+same\n"
        self.assertEqual(self._state("1ee5dcb\n", diff=diff),
                         self._state("1ee5dcb\n", diff=diff))

    def test_a_new_untracked_tool_changes_the_name(self):
        """A file that appears mid-session changes what runs; `git diff` cannot see it."""
        clean = self._state("1ee5dcb\n")
        added = self._state("1ee5dcb\n", status="?? tools/civ6_newthing.py\n")
        self.assertNotEqual(clean, added)

    def test_a_git_failure_is_never_accepted_as_a_revision(self):
        """`run()` returns stdout+stderr, so a non-repo handed back its own error.

        The ledger recorded rows whose identity was the words `fatal: not a git
        repository`, and the hash appended to it was a hash of that same message —
        so every non-repo tree pinned alike. Two different programs, one name, which
        is precisely what this function exists to prevent.
        """
        name = self._state("fatal: not a git repository (or any of the "
                           "parent directories): .git\n")
        self.assertNotIn("fatal", name)
        self.assertNotIn(" ", name)
        self.assertTrue(name.startswith("nogit+"), name)

    def test_two_non_git_trees_that_differ_get_different_names(self):
        """The old code gave both the same name, because it hashed the error text."""
        import pathlib
        import tempfile

        def name_for(body):
            with tempfile.TemporaryDirectory() as tmp:
                root = pathlib.Path(tmp)
                (root / "tools").mkdir()
                (root / "tools" / "civ6_play.py").write_text(body)
                saved, climb.HERE = climb.HERE, root / "tools"
                try:
                    return self._state("fatal: not a git repository\n")
                finally:
                    climb.HERE = saved

        self.assertNotEqual(name_for("one"), name_for("two"))
        self.assertEqual(name_for("same"), name_for("same"))



class BatchPowerTests(unittest.TestCase):
    """A batch that reports a result without its power is how an underpowered
    number gets quoted. The line is printed where the operator reads the
    batch, not left in a table nobody opens."""

    def test_it_reproduces_the_measured_sample_sizes(self):
        # Cross-check against the arithmetic computed independently from the
        # ladder in the 2026-08-17 study: ~152 games per arm separates the
        # measured 25% Settler win rate from 40%, ~58 from 50%.
        self.assertLessEqual(climb.resolvable_win_rate(152), 0.40 + 0.005)
        self.assertGreater(climb.resolvable_win_rate(152), 0.25)
        self.assertLessEqual(climb.resolvable_win_rate(60), 0.50 + 0.005)

    def test_a_bigger_batch_resolves_a_finer_effect(self):
        coarse = climb.resolvable_win_rate(8)
        fine = climb.resolvable_win_rate(400)
        self.assertIsNotNone(coarse)
        self.assertIsNotNone(fine)
        self.assertGreater(coarse, fine)

    def test_a_single_attempt_is_named_a_smoke_test(self):
        line = climb.batch_power_line(1)
        self.assertIn("smoke test", line)
        self.assertNotIn("80% power", line)

    def test_the_line_says_PER_ARM_so_a_batch_is_not_read_as_a_comparison(self):
        # The number is per arm; a single pinned batch is HALF of a paired
        # comparison, and the line must not let it read as the whole thing.
        self.assertIn("PER ARM", climb.batch_power_line(8))


class BatchCompositionTests(unittest.TestCase):
    """A game killed by our own clock is not a loss, and nothing said so."""

    def test_a_clean_batch_says_so_without_a_warning(self):
        line = climb.batch_composition_line(collections.Counter({"stopped": 8}))
        self.assertIn("8/8 played to a finish", line)
        self.assertNotIn("NOT a loss", line)

    def test_an_unfinished_attempt_is_named_and_disclaimed(self):
        # The row carries `outcome.won = None`, so any win rate counting
        # `won == True` over all rows scores it a non-win. The batch has to
        # say it happened or the number is quietly wrong.
        line = climb.batch_composition_line(
            collections.Counter({"stopped": 5, "timeout": 3}))
        self.assertIn("5/8 played to a finish", line)
        self.assertIn("timeout=3", line)
        self.assertIn("NOT a loss", line)

    def test_an_empty_batch_does_not_claim_a_denominator(self):
        line = climb.batch_composition_line(collections.Counter())
        self.assertIn("measured nothing", line)
        self.assertNotIn("0/0", line)

    def test_every_ending_is_named_not_just_the_known_ones(self):
        # Reasons are whatever the harness wrote; an unfamiliar one must still
        # appear rather than be folded into a catch-all.
        line = climb.batch_composition_line(
            collections.Counter({"stopped": 1, "attempt frozen; resume failed": 1}))
        self.assertIn("attempt frozen; resume failed=1", line)


class BusyPatternTest(unittest.TestCase):
    """`busy()` must see a running game, not a command line that names one.

    The pattern was `Civ6_Exe|civ6_play.py|civ6_brain.py` and `pgrep -f` matches
    whole command lines, so the harness's own window probe --
    `osascript -e 'tell ... process "Civ6_Exe_Child" ...'`, which the mirror
    keeper runs while the lane is idle -- reported a game that was not running.
    `start` then refused ("something already holds the game"), the batch recorded
    PLAYED NO TURNS, and four of those put the supervisor to sleep for ten
    minutes. Measured 2026-08-18 with no game running at all.
    """

    def _match(self, command: str) -> bool:
        return re.search(climb.RUNNING_GAME_PATTERN, command) is not None

    def test_a_running_game_and_its_harness_are_seen(self) -> None:
        for command in (
            "/Users/x/Civ6.app/Contents/MacOS/Civ6_Exe_Child",
            "/usr/bin/python3 /Users/x/CIVVIS/tools/civ6_play.py --tag civvis-1",
            "/usr/bin/python3 /Users/x/CIVVIS/tools/civ6_brain.py --run-dir /x",
        ):
            self.assertTrue(self._match(command), command)

    def test_a_mention_is_not_a_running_game(self) -> None:
        for command in (
            'osascript -e tell application "System Events" to tell process '
            '"Civ6_Exe_Child" to get position of window 1',
            "pgrep -fl Civ6_Exe|civ6_play.py|civ6_brain.py",
            "/bin/zsh -c echo checking civ6_play.py status",
        ):
            self.assertFalse(self._match(command), command)

if __name__ == "__main__":
    unittest.main()


class WedgedAttempt(unittest.TestCase):
    """★★★★★ An attempt whose harness is blocked must be killed FROM OUTSIDE.

    `civ6_play --frozen-seconds` watches the turn from inside its own poll loop.
    On 2026-08-02 that did not save run civvis-20260802T064240Z: it wedged at
    turn 206 on `WorldCongressBetweenTurns` for over ten minutes with the flag
    armed, and never fired — no rescue line, no summary.json, so `follow` had
    not returned. Replaying that run's events shows the in-loop logic WOULD have
    fired, so it was right and simply never ran.

    ⚠ The mod appends to events.jsonl from inside the GAME. A growing file proves
    the game is alive, not the harness. Only another process can see that.
    """

    # How much the fake clock advances per `play.wait()`. The real loop is paced
    # by that call blocking for up to 20 s; here it stands in for "the harness
    # waited a bit and the attempt is still running", and 0.1 against the 3 s
    # budget below gives 30 passes — enough for the lock-credit logic to run
    # more than once, and instant.
    WAIT_STEP_S = 0.1

    class _Play:
        """A subprocess that never exits until it is signalled.

        ⚠ `wait` ADVANCES THE CLOCK, and that is what makes these tests fast.
        The real one blocks; this one used to return instantly, so the waiter
        span a hot loop against real `time.time()` for the whole 3 s budget —
        6 seconds of burned CPU across this class, on every PR's CI gate and
        every local validation. Moving the clock here keeps the loop's pacing
        faithful (one pass per wait) without spending any of it.
        """

        def __init__(self, clock=None, step=0.0):
            self.signalled = None
            self._dead = False
            self._clock = clock
            self._step = step

        def wait(self, timeout=None):
            if self._clock is not None:
                self._clock["t"] += self._step
            if self._dead:
                return 0
            raise climb.subprocess.TimeoutExpired("play", timeout or 0)

        def send_signal(self, sig):
            self.signalled = sig
            self._dead = True

        def kill(self):
            self._dead = True

    def _run(self, turns, frozen_s, locked_probe=None):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "run").mkdir()
            events = root / "run" / "events.jsonl"
            events.write_text("".join(
                json.dumps({"kind": "turn", "turn": t}) + "\n" for t in turns))
            original = climb.RUN_ROOT
            climb.RUN_ROOT = root
            # ⚠ The waiter reads `time.time()`, not `monotonic`. Pinning it is
            # also what makes the assertions mean anything: a budget waited out
            # in real time asserts on whatever the machine managed in three
            # seconds, which is not the same number twice under load.
            clock = {"t": 0.0}
            try:
                play = self._Play(clock, self.WAIT_STEP_S)
                with mock.patch.object(climb.time, "time", lambda: clock["t"]):
                    why = climb.wait_watching_the_turn(
                        play, "run", 3.0, frozen_s,
                        locked_probe=locked_probe or (lambda: False))
                return why, play.signalled
            finally:
                climb.RUN_ROOT = original

    def test_a_locked_screen_does_not_kill_a_healthy_attempt(self):
        """⚠⚠ A LOCKED SCREEN IS NOT A STALLED GAME.

        `civ6_play` waits at the macOS authentication boundary instead of
        scripting past it. Neither timer here knew that: `deadline` is wall
        clock and `last_turn_at` cannot advance while the game is not being
        driven, so a long enough lock killed a healthy attempt.

        Live: run `civvis-20260804T102440Z` reached turn 82 with 4 cities and
        score 185, then was stopped by "attempt exceeded its own timeout" after
        the session locked mid-game.

        With the screen reported LOCKED and `frozen_s=0`, the pre-fix code kills
        the attempt immediately. It must not.
        """
        original_cap = climb.LOCK_CREDIT_CAP_S
        # A ZERO cap keeps the test bounded: the first locked slice exceeds it.
        # Without ANY cap the loop spins forever, because every locked slice
        # extends the deadline it is racing — precisely what the first version
        # of this fix did, and the fake `play.wait` returns instantly so almost
        # no wall clock accrues to reach a larger cap.
        climb.LOCK_CREDIT_CAP_S = 0.0
        started = time.time()
        try:
            why, signalled = self._run([1, 2, 206], frozen_s=0.0,
                                        locked_probe=lambda: True)
        finally:
            climb.LOCK_CREDIT_CAP_S = original_cap
        self.assertNotEqual(
            why, "frozen",
            "a locked screen must not be read as a frozen turn")
        self.assertEqual(
            why, "locked",
            "past the credit cap the attempt must end, and must SAY it was the lock")

    def test_a_permanent_lock_cannot_hang_the_attempt_forever(self):
        """⚠ An unbounded pause holds the game hostage on a machine left locked.

        Every locked slice extends the deadline it is racing, so without a cap
        this loop never terminates. Bounded here by construction: the call must
        return rather than run past the test's own patience.
        """
        original_cap = climb.LOCK_CREDIT_CAP_S
        climb.LOCK_CREDIT_CAP_S = 0.0
        started = time.time()
        try:
            why, signalled = self._run([1, 2, 206], frozen_s=600.0,
                                        locked_probe=lambda: True)
        finally:
            climb.LOCK_CREDIT_CAP_S = original_cap
        self.assertEqual(why, "locked")
        self.assertLess(time.time() - started, 30.0,
                        "the capped wait must resolve promptly, not spin")

    def test_an_unlocked_frozen_turn_is_still_killed(self):
        """The pause must be conditional — with the screen UNLOCKED the guard stands."""
        why, signalled = self._run([1, 2, 206], frozen_s=0.0,
                                   locked_probe=lambda: False)
        self.assertEqual(why, "frozen")
        self.assertEqual(signalled, climb.signal.SIGTERM)

    def test_a_frozen_turn_is_killed_from_outside(self):
        why, signalled = self._run([1, 2, 206], frozen_s=0.0)
        self.assertEqual(why, "frozen", "a stuck turn must end the attempt")
        self.assertEqual(signalled, climb.signal.SIGTERM)

    def test_setup_has_no_turn_and_must_not_be_killed(self):
        """⚠ Killing every attempt before it starts is the failure this must not have."""
        why, signalled = self._run([], frozen_s=0.0)
        self.assertNotEqual(why, "frozen", "no turn seen yet is not a frozen turn")
        self.assertIsNone(signalled)

    def test_a_patient_setting_lets_a_slow_attempt_live(self):
        why, signalled = self._run([1, 2, 3], frozen_s=600.0)
        self.assertNotEqual(why, "frozen")
        self.assertIsNone(signalled)


class KilledAttemptSaysSo(unittest.TestCase):
    """⚠⚠⚠ A row with a turn count and a score and NO reason reads like a
    finished game.

    `outcome_of` reads what `civ6_play` wrote on its way out, and a run this loop
    SIGTERMs never gets there. `civvis-20260811T115348Z` is in the ladder that
    way: turn 194 of 250, score 490, stopped by the outer watchdog while still
    advancing, `reason` absent. I included it in medians myself before noticing.

    The loop knows why it killed the attempt. These pin that it says so, and that
    it never overwrites a reason the harness did supply.
    """

    @staticmethod
    def _stamp(record, why):
        """The rule under test, applied exactly as the loop applies it."""
        if record.get("reason") is None and why is not None:
            record["reason"] = f"attempt {why}"
        return record

    def test_a_killed_attempt_is_not_mistaken_for_a_finished_one(self):
        row = self._stamp({"last_turn": 194, "last_score": 490}, "timeout")
        self.assertEqual(row["reason"], "attempt timeout")

    def test_a_lock_is_named_as_a_lock(self):
        """The log already refuses to blame a timeout for an unattended machine;
        the ledger must not either."""
        row = self._stamp({"last_turn": 82}, "locked")
        self.assertEqual(row["reason"], "attempt locked")

    def test_the_harnesss_own_reason_always_wins(self):
        """`civ6_play` saw the ending from inside. That answer is better."""
        row = self._stamp({"last_turn": 250, "reason": "stopped"}, "timeout")
        self.assertEqual(row["reason"], "stopped")

    def test_nothing_is_invented_when_the_loop_has_no_verdict(self):
        row = self._stamp({"last_turn": 250}, None)
        self.assertIsNone(row.get("reason"))


class MissingSummaryExitProvenanceTests(unittest.TestCase):
    """A child that vanishes after turns must leave more than ``attempt exited``.

    `civvis-20260820T210941Z` reached turn 95 with four cities, then its child
    exited without `summary.json`. The old row retained the turn but discarded
    the only remaining OS-level signal: the child's return code.
    """

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.saved_root = climb.RUN_ROOT
        climb.RUN_ROOT = self.root

    def tearDown(self):
        climb.RUN_ROOT = self.saved_root
        self.tmp.cleanup()

    def test_an_exit_without_a_summary_preserves_status_and_last_state(self):
        run = self.root / "civvis-exited"
        run.mkdir()
        outcome = {"last_turn": 95, "last_score": 203, "rival_best": 396,
                   "cities": 4, "army": 7}

        detail = climb.record_unexplained_child_exit(
            "civvis-exited", "exited", -9, outcome)

        self.assertEqual(detail["returncode"], -9)
        self.assertEqual(detail["watcher_reason"], "exited")
        self.assertEqual(detail["last_observed"], outcome)
        self.assertEqual(
            json.loads((run / "exit.json").read_text()), detail,
            "the run-local sidecar survives for a later ledger backfill")

    def test_outcome_of_reloads_the_exit_sidecar(self):
        run = self.root / "civvis-exited"
        run.mkdir()
        (run / "events.jsonl").write_text(
            json.dumps({"kind": "turn", "turn": 95, "score": 203,
                        "rival_best": 396, "cities": 4, "army": 7}) + "\n")
        detail = climb.record_unexplained_child_exit(
            "civvis-exited", "exited", 137, climb.outcome_of("civvis-exited"))

        self.assertEqual(climb.outcome_of("civvis-exited")["child_exit"], detail)

    def test_a_real_summary_or_a_non_exit_never_gets_the_artifact(self):
        run = self.root / "civvis-finished"
        run.mkdir()
        (run / "summary.json").write_text(json.dumps({"reason": "stopped"}))
        self.assertIsNone(climb.record_unexplained_child_exit(
            "civvis-finished", "exited", 0, {}))
        self.assertFalse((run / "exit.json").exists())

        self.assertIsNone(climb.record_unexplained_child_exit(
            "civvis-finished", "frozen", -15, {}))
        self.assertFalse((run / "exit.json").exists())


class OuterWatchdogOrdering(unittest.TestCase):
    """⚠⚠⚠ The backstop must sit ABOVE the budget it backs, not underneath it.

    `civ6_play --timeout` stopped being a hard stop in #1532: the budget now
    extends while a run can still reach `--max-turns`, up to `--timeout-ceiling`
    (1.5x by default). This watchdog stayed at `--timeout + 600`, which put a
    6000 s kill underneath 8100 s of legitimate inner budget.

    It fired on healthy games. Run `civvis-20260811T115348Z` was stopped at turn
    194 of 250, score 490, still advancing — 100 min 41 s in, `timeout + 600` to
    the second. The SIGTERM also costs the summary, so the ladder keeps a row
    with `last_turn` and `last_score` and NO `reason`, which reads like a
    finished game.
    """

    @staticmethod
    def _args(timeout=5400.0, ceiling=None):
        from types import SimpleNamespace
        return SimpleNamespace(timeout=timeout, timeout_ceiling=ceiling)

    def test_the_watchdog_outlives_the_childs_ceiling(self):
        args = self._args()
        ceiling = climb.attempt_ceiling(args)
        self.assertGreater(
            ceiling + 600, ceiling,
            "the backstop must be strictly later than the budget it backs")
        self.assertGreater(
            ceiling + 600, args.timeout + 600,
            "the old formula is exactly the bug: it sat below the ceiling")

    def test_the_default_matches_civ6_plays_own(self):
        """Both sides default to 1.5x, so an unset flag cannot invert them."""
        self.assertEqual(climb.attempt_ceiling(self._args(5400.0)), 8100.0)

    def test_an_explicit_ceiling_is_honoured_on_both_sides(self):
        """Passing --timeout-ceiling equal to --timeout restores a hard stop."""
        self.assertEqual(climb.attempt_ceiling(self._args(5400.0, 5400.0)), 5400.0)


class SettingsDealt(unittest.TestCase):
    """★★★★★ A row dealt something other than what was asked must say so.

    `is_win` refuses to count a VICTORY at the wrong rung. That check only ever
    fires on a win, so every losing row — all of them, so far — was written with
    `difficulty_asked: DIFFICULTY_SETTLER` beside a seat that said otherwise, and
    nothing said a word.

    Measured 2026-08-02: 25 consecutive runs dealt DIFFICULTY_SETTLER and the
    26th dealt DIFFICULTY_PRINCE on identical setup code. Rare, not chronic —
    which is why it has to be RECORDED rather than watched for.
    """

    # ⚠ This used to be a local re-declaration "in the same shape" as the climb's
    # inline comparison. It was a second copy of a rule, and a second copy of a rule
    # is a test that goes green while the shipped one changes underneath it. Call
    # the shipped function.
    _mismatch = staticmethod(climb.settings_mismatch)

    ASKED = {"difficulty": "DIFFICULTY_SETTLER", "map_size": "MAPSIZE_SMALL",
             "speed": "GAMESPEED_ONLINE", "leader": "LEADER_TRAJAN"}

    DEALT = {"difficulty": "DIFFICULTY_SETTLER", "size": "MAPSIZE_SMALL",
             "speed": "GAMESPEED_ONLINE", "leader": "LEADER_TRAJAN"}

    def test_the_rung_the_game_actually_dealt_is_recorded(self):
        dealt = {**self.DEALT, "difficulty": "DIFFICULTY_PRINCE"}
        found = self._mismatch(self.ASKED, dealt)
        self.assertIn("difficulty", found)
        self.assertEqual(found["difficulty"]["dealt"], "DIFFICULTY_PRINCE")
        self.assertNotIn("map_size", found, "settings that matched must stay quiet")

    def test_a_matching_game_records_nothing(self):
        """⚠ A field that always fires says nothing. Silence is the healthy case."""
        self.assertEqual(self._mismatch(self.ASKED, self.DEALT), {})

    def test_a_seat_dealt_the_wrong_leader_is_recorded(self):
        """The 2026-08-03 census: 190 runs, Trajan 4 of them, and no row said so.

        The picker verifies its own click, so this should never fire — which is
        exactly the argument for recording it rather than trusting it.
        """
        found = self._mismatch(self.ASKED, {**self.DEALT, "leader": "LEADER_TOKUGAWA"})
        self.assertEqual(found["leader"],
                         {"asked": "LEADER_TRAJAN", "dealt": "LEADER_TOKUGAWA"})

    def test_asking_for_no_leader_accepts_whatever_is_dealt(self):
        """`--leader ""` is a deliberate random deal, not a field to police."""
        asked = {**self.ASKED, "leader": ""}
        self.assertEqual(self._mismatch(asked, {**self.DEALT, "leader": "LEADER_GORGO"}),
                         {})

    def test_a_seat_that_could_not_be_read_is_not_a_mismatch(self):
        """⚠ Absent is not wrong. An old export naming nothing must not be
        reported as the game having dealt the wrong rung."""
        self.assertEqual(self._mismatch(self.ASKED, {}), {})


class BatchPinStalenessTests(unittest.TestCase):
    """The pin line must say when the tree is behind, or a stale tree is invisible.

    On 2026-08-02 the batch tree ran hours behind `origin/main` with four merged
    fixes unbuilt, and `batch pinned to 1000a13` read exactly as it would have on
    main. Reporting is enough — pinning to an old commit is a legitimate way to keep
    a batch comparable — but it must not be silent.
    """

    def test_the_helper_reports_rather_than_refuses(self) -> None:
        source = (Path(__file__).resolve().parent
                  / "civ6_civvis_climb.py").read_text(encoding="utf-8")
        start = source.index("def commits_behind_main()")
        whole = source[start:source.index("\ndef ", start + 10)]
        # ⚠ Assert against CODE, not prose. The docstring says "does not fetch",
        # and a substring check that reads the docstring is checking the comment.
        opening = whole.index('"""')
        code = whole[whole.index('"""', opening + 3) + 3:]
        self.assertIn("rev-list", code)
        self.assertIn("HEAD..origin/main", code)
        # ⚠ Must never fetch: a batch cannot depend on the network.
        self.assertNotIn("fetch", code)
        # ⚠ Must never abort: an old pin is legitimate.
        self.assertNotIn("sys.exit", code)

    def test_the_pin_line_carries_the_staleness(self) -> None:
        source = (Path(__file__).resolve().parent
                  / "civ6_civvis_climb.py").read_text(encoding="utf-8")
        # ⚠ Assert on the PRINT EXPRESSION, not the neighbourhood. The first version
        # of this test read 700 characters either side, so it passed unchanged when
        # the staleness was computed and then never concatenated — which is exactly
        # the bug it exists to catch.
        start = source.index('print(f"batch pinned to {pinned}"')
        statement = source[start:source.index("flush=True)", start)]
        self.assertIn("staleness", statement,
                      f"the computed staleness must reach the printed line: {statement}")
        before = source[start - 700:start]
        self.assertIn("commits_behind_main()", before)
        self.assertIn("behind origin/main", before)


class OneDeciderTests(_Harness, unittest.TestCase):
    """The climb must not start a decider; it must configure the one `civ6_play` runs.

    ⚠⚠⚠ Measured live 2026-08-03 on `civvis-20260803T185256Z`: TWO `civ6_brain.py`
    processes on one run dir, one `orders.sqlite` and one `why.log` —

        pid 81760   civ6_brain.py ...                 (spawned by civ6_play)
        pid 81772   civ6_brain.py --war-from-plan     (spawned by the climb)

    It hid because both deciders are deterministic over the same `events.jsonl`, so
    their logs agreed turn for turn, and nothing compared the two CONFIGURATIONS —
    which differed on exactly the flag the operator had just turned on.
    """

    def _play_argv(self, argv_extra=()):
        """The argv the climb hands to `civ6_play.py`, and every other spawn."""
        seen = []

        class Recording(FakeProc):
            def __init__(self, argv, *args, **kwargs):
                seen.append(list(argv))
                super().__init__(argv, *args, **kwargs)

        climb.subprocess.Popen = Recording
        try:
            self.climb_with([{"last_turn": 40}], attempts=1, argv_extra=argv_extra)
        finally:
            climb.subprocess.Popen = FakeProc
        return seen

    def test_the_climb_starts_exactly_one_process_and_it_is_not_a_brain(self):
        spawned = self._play_argv()
        brains = [argv for argv in spawned
                  if any("civ6_brain.py" in str(word) for word in argv)]
        self.assertEqual(brains, [], "the climb must not spawn its own decider")
        plays = [argv for argv in spawned
                 if any("civ6_play.py" in str(word) for word in argv)]
        self.assertEqual(len(plays), 1)

    def test_every_decider_setting_reaches_the_process_that_runs_it(self):
        """A setting the climb keeps to itself is a setting that grew a second brain."""
        play = next(argv for argv in self._play_argv()
                    if any("civ6_play.py" in str(word) for word in argv))
        words = [str(word) for word in play]
        self.assertIn("--civvis-decides", words)
        # ⚠ NOT `--civvis-war-from-plan`. That is the one setting the climb must
        # NOT pass: `civ6_play` refuses it as replay-only, the climb's own second
        # brain was the only thing that ever applied it live, and asking for it is
        # now refused before any of this runs. See `WarFromPlanGuardTests`.
        self.assertNotIn("--civvis-war-from-plan", words)
        self.assertIn("--civvis-victory", words)
        self.assertIn("--civvis-strategy", words)
        # The chain's one default, not a copy of it — the climb declared its own
        # `science` until 2026-08-18 and `civ6_play` declared another beside it.
        self.assertEqual(words[words.index("--civvis-victory") + 1],
                         climb.DEFAULT_VICTORY)
        self.assertEqual(words[words.index("--civvis-strategy") + 1], "")
        # `--orders-bin` used to reach only the climb's own brain, so `civ6_play`
        # fell back to its repo-relative default and a worktree without a build died
        # with "CIVVIS decision binary does not exist" while the flag naming a real
        # binary sat on the command line.
        self.assertIn("--civvis-bin", words)
        self.assertEqual(words[words.index("--civvis-bin") + 1], str(self.orders_bin))

    def test_domination_and_auto_remain_explicit_opt_ins(self):
        play = next(
            argv for argv in self._play_argv(("--victory", "domination", "--strategy", "auto"))
            if any("civ6_play.py" in str(word) for word in argv)
        )
        words = [str(word) for word in play]
        self.assertEqual(words[words.index("--civvis-victory") + 1], "domination")
        self.assertEqual(words[words.index("--civvis-strategy") + 1], "auto")

    def test_the_three_lanes_that_were_unreachable_now_reach_the_game(self):
        """Culture, Religion and Diplomacy are implemented in `advanced.rs` and were
        absent from this launcher's `choices`, so argparse rejected them before
        anything could be measured. A lane the engine plays and the ladder cannot
        select is a lane that does not exist."""
        for lane in ("culture", "religious", "diplomatic"):
            with self.subTest(lane=lane):
                play = next(argv for argv in self._play_argv(("--victory", lane))
                            if any("civ6_play.py" in str(word) for word in argv))
                words = [str(word) for word in play]
                self.assertEqual(words[words.index("--civvis-victory") + 1], lane)

    def test_civ6_play_forwards_the_war_flag_to_the_brain(self):
        """The far end of the same wire. A flag `civ6_play` accepts and drops is worse
        than one it does not accept: the climb would look configured and not be."""
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        self.assertIn('"--civvis-war-from-plan"', source)
        self.assertIn("if args.civvis_war_from_plan:", source)
        self.assertIn('command.append("--war-from-plan")', source)


class WarFromPlanGuardTests(_Harness, unittest.TestCase):
    """`--war-from-plan` must not reach a live game behind `civ6_play`'s back.

    ⚠⚠⚠ `civ6_play.main` refuses `--civvis-war-from-plan` and says why: the override
    declares on a plan's preferred rival even when the planner DECLINED war, and
    live run `live-loop-rome-20260802-0800` forced one under a Religion plan on turn
    37, spent 213 turns in Recovery asking for peace, and finished 400-1081.

    The climb bypassed that guard for a day — not deliberately, but because it ran
    its OWN `civ6_brain.py`, which takes the flag directly. Removing the second
    decider is what made the conflict visible; this keeps it visible.
    """

    def test_asking_for_war_from_plan_refuses_the_batch(self):
        code, rows = self.climb_with([{"last_turn": 40}], attempts=1,
                                     argv_extra=("--war-from-plan",))
        self.assertEqual(code, 4, "a refused configuration is not a played batch")
        self.assertEqual(rows, [], "and it must not spend a rung or write a row")

    def test_the_default_batch_still_runs(self):
        """⚠ A guard that refuses everything is not a guard. Silence is the healthy case."""
        code, rows = self.climb_with([{"last_turn": 40}], attempts=1)
        self.assertEqual(code, 1)
        self.assertEqual(len(rows), 1)

    def test_civ6_play_still_holds_the_guard_this_one_defers_to(self):
        """If that guard is ever lifted, this refusal is stale and must go with it."""
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        self.assertIn('if "--civvis-war-from-plan" in raw_argv:', source)
        self.assertIn("replay-only", source)


class EndScreenEvidenceTests(unittest.TestCase):
    """`reached_end_screen` was keyed on an event that has NEVER been emitted.

    ⚠⚠⚠ Measured across every run on this machine: 250 runs, 145
    `autoclose_armed` for `EndGameMenu`, and ZERO `autoclose`. `armed` fires when
    the context registers its handler — at game START, line 19 of a real run — and
    the `autoclose` this looked for cannot happen at all, because Civilization VI
    halts the Game Core when it shows the end-of-game screen and the shim ticks off
    the frame loop that just stopped.

    So the column read `None` on all 250 rows while its name promised the opposite.
    """

    # ⚠ NOT `_outcome`: `unittest.TestCase` uses `self._outcome` for its own
    # `_Outcome` result recorder, so that name silently shadows the framework
    # and every call fails with "'_Outcome' object is not callable".
    def _row(self, events, tmp):
        run = Path(tmp) / "civvis-x"
        run.mkdir()
        (run / "events.jsonl").write_text(
            "\n".join(json.dumps(e) for e in events) + "\n")
        saved = climb.RUN_ROOT
        climb.RUN_ROOT = Path(tmp)
        try:
            return climb.outcome_of("civvis-x")
        finally:
            climb.RUN_ROOT = saved

    def test_a_terminal_event_means_the_game_reached_its_end(self):
        with tempfile.TemporaryDirectory() as tmp:
            record = self._row([
                {"kind": "turn", "turn": 250, "score": 479},
                {"kind": "victory", "turn": 251, "won": False, "team": 4},
            ], tmp)
        self.assertTrue(record["reached_end_screen"])
        self.assertEqual(record["end_screen_turn"], 251)

    def test_a_rivals_victory_still_counts_as_the_game_ending(self):
        """⚠ The question is 'did the game reach its end', not 'did we win'.
        `won()` remains the only thing that decides whose victory it was."""
        with tempfile.TemporaryDirectory() as tmp:
            record = self._row([
                {"kind": "victory", "turn": 251, "won": False, "team": 4},
            ], tmp)
        self.assertTrue(record["reached_end_screen"])
        self.assertFalse(climb.won(record))

    def test_the_armed_event_alone_is_not_an_ending(self):
        """⚠ `autoclose_armed` fires at game START. Counting it would mark every
        run as having reached its end screen, including the ones that hung."""
        with tempfile.TemporaryDirectory() as tmp:
            record = self._row([
                {"kind": "autoclose_armed", "screen": "EndGameMenu", "seconds": 10.0},
                {"kind": "turn", "turn": 42},
            ], tmp)
        self.assertIsNone(record["reached_end_screen"])

    def test_a_hung_run_is_not_reported_as_ended(self):
        with tempfile.TemporaryDirectory() as tmp:
            record = self._row([{"kind": "turn", "turn": 87}], tmp)
        self.assertIsNone(record["reached_end_screen"])


class FinalScreenHoldTests(unittest.TestCase):
    """The hold has to live in the harness, because the mod's clock cannot run."""

    def test_the_harness_holds_before_it_stops_the_game(self):
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        hold = source.index("holding the final screen")
        stop = source.index("game_stopped = launcher.stop()")
        self.assertLess(hold, stop,
                        "holding after the game is killed holds nothing")

    def test_the_hold_only_happens_when_the_game_actually_ended(self):
        """⚠ A stall or a wrong-modes refusal has nothing worth looking at, and
        holding there would add ten seconds to every failure in a batch."""
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        self.assertIn('if state["outcome"] and args.end_game_seconds > 0:', source)


class IdleSettlerTests(unittest.TestCase):
    """A settler nobody ordered is invisible on every other column in the row.

    ⚠⚠⚠ Measured across the archive 2026-08-03: 54 of 142 runs of >=50 turns (38%)
    park a settler for >=15 consecutive turns at FULL movement — median streak 37,
    worst 143. The applied-order rate never dips, because no order is issued to
    fail, and `why.log` actively reports it as "marching".
    """

    def _run(self, states, tmp):
        run = Path(tmp) / "r"
        run.mkdir()
        (run / "events.jsonl").write_text(
            "\n".join(json.dumps(s) for s in states) + "\n")
        return climb.longest_idle_settler(run / "events.jsonl")

    @staticmethod
    def _state(turn, x, y, moves, uid=7):
        return {"kind": "state", "turn": turn,
                "units": [{"kind": "UNIT_SETTLER", "id": uid,
                           "x": x, "y": y, "moves": moves}]}

    def test_a_settler_that_sits_at_full_movement_is_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            states = [self._state(t, 5, 5, 2) for t in range(1, 12)]
            self.assertEqual(self._run(states, tmp), 10)

    def test_a_settler_that_moves_is_not_idle(self):
        """⚠ The healthy case must read zero, or the column says nothing."""
        with tempfile.TemporaryDirectory() as tmp:
            states = [self._state(t, 5, 5 + t, 1) for t in range(1, 12)]
            self.assertEqual(self._run(states, tmp), 0)

    def test_a_settler_out_of_movement_is_NOT_idle(self):
        """⚠⚠ THE WHOLE POINT OF THE TEST. A unit that SPENT its movement was
        asked to act and failed — that is blocked, which is a different defect and
        a legitimate outcome. Only a unit still holding movement was never asked."""
        with tempfile.TemporaryDirectory() as tmp:
            states = [self._state(t, 5, 5, 0) for t in range(1, 12)]
            self.assertEqual(self._run(states, tmp), 0)

    def test_the_streak_resets_when_the_settler_finally_moves(self):
        with tempfile.TemporaryDirectory() as tmp:
            states = ([self._state(t, 5, 5, 2) for t in range(1, 6)]
                      + [self._state(6, 6, 5, 2)]
                      + [self._state(t, 6, 5, 2) for t in range(7, 10)])
            self.assertEqual(self._run(states, tmp), 4, "longest streak, not the last")

    def test_two_settlers_are_tracked_apart(self):
        """⚠ A shared counter would let a moving settler clear a parked one's streak."""
        with tempfile.TemporaryDirectory() as tmp:
            states = []
            for t in range(1, 10):
                states.append({"kind": "state", "turn": t, "units": [
                    {"kind": "UNIT_SETTLER", "id": 1, "x": 5, "y": 5, "moves": 2},
                    {"kind": "UNIT_SETTLER", "id": 2, "x": 9, "y": t, "moves": 2},
                ]})
            self.assertEqual(self._run(states, tmp), 8)

    def test_no_settlers_reads_zero(self):
        with tempfile.TemporaryDirectory() as tmp:
            states = [{"kind": "state", "turn": t, "units": []} for t in range(1, 5)]
            self.assertEqual(self._run(states, tmp), 0)


class PassThroughFlagsReachTheGame(unittest.TestCase):
    """Every boolean flag this loop defines must actually be FORWARDED.

    ⚠⚠ THE CHAIN IS FOUR LINKS AND THE FOURTH IS THE ONE THAT BREAKS. The mod
    reads a `cfg` key, `civ6_play.py` sets it from a flag, and this loop builds a
    FIXED argument list — so a flag can exist at every other layer and still be
    unreachable from a live game. `#1098` shipped exactly that way, and the
    envoy lane sat behind the same gap: `civ6_play.py` has taken `--envoys` all
    along and nothing could pass it.

    This is a source-level assertion rather than a behavioural one because the
    argument list is built inside the attempt loop, and a test that had to reach
    it would be testing the harness rather than the chain. The failure it exists
    to catch is textual: a flag declared and never forwarded.
    """

    SOURCE = (Path(__file__).resolve().parent / "civ6_civvis_climb.py").read_text()
    PLAY = (Path(__file__).resolve().parent / "civ6_play.py").read_text()

    def test_every_flag_civ6_play_also_accepts_is_actually_forwarded(self) -> None:
        """A flag both layers know about, that this loop never passes on, is #1098."""
        import re

        declared = set(
            re.findall(r'ap\.add_argument\("--([a-z0-9-]+)", action="store_true"', self.SOURCE)
        )
        self.assertIn("envoys", declared)

        missing = []
        for flag in sorted(declared):
            # Only flags the FAR END understands can be forwarded at all; the
            # rest are this loop's own business.
            if f'"--{flag}"' not in self.PLAY:
                continue
            if f'(["--{flag}"] if args.{flag.replace("-", "_")} else [])' not in self.SOURCE:
                missing.append(flag)
        self.assertEqual(
            [], missing,
            f"declared here and understood by civ6_play, but never forwarded: {missing}",
        )

    def test_envoys_is_forwarded_and_civ6_play_accepts_it(self) -> None:
        self.assertIn('(["--envoys"] if args.envoys else [])', self.SOURCE)
        # The far end of the link must exist, or forwarding it is a crash.
        self.assertIn('"--envoys"', self.PLAY)

    def test_envoys_defaults_off_because_the_lane_can_segfault(self) -> None:
        self.assertIn(
            'ap.add_argument("--envoys", action="store_true", default=False',
            self.SOURCE,
            "the envoy lane must stay opt-in while chooseEnvoy has a SIGSEGV history",
        )


class ResumeFromAutosaveTests(_Harness, unittest.TestCase):
    """A frozen attempt is reloaded from its latest autosave, not scored as it fell.

    ★★★★★ Three leading games died on the 900 s watchdog on 2026-08-16 with a
    turn-fresh `AutoSave_NNNN.Civ6Save` on disk (t178 leading 804 vs 715, t207,
    t102 leading with the lane's first capture). See `resume_from_autosave`.
    """

    class _Args:
        max_resumes = 2
        resume_min_turn = climb.RESUME_MIN_TURN

    def test_the_policy_resumes_only_a_frozen_mid_game_within_budget(self):
        args = self._Args()
        seen = []

        def finder(newer_than=None):
            seen.append(newer_than)
            return Path("/saves/AutoSave_0102.Civ6Save")

        frozen = {"last_turn": 102, "last_score": 340}
        self.assertEqual(
            climb.resume_from_autosave(frozen, "frozen", 0, args, 1234.5, latest=finder),
            Path("/saves/AutoSave_0102.Civ6Save"))
        self.assertEqual(seen, [1234.5], "only autosaves written since the attempt began")
        # Not frozen: a timeout, a locked screen, a normal exit — no resume.
        for why in ("timeout", "locked", "exited", None):
            self.assertIsNone(climb.resume_from_autosave(frozen, why, 0, args, 0.0, latest=finder))
        # Too early to be worth the load flow.
        self.assertIsNone(climb.resume_from_autosave(
            {"last_turn": climb.RESUME_MIN_TURN - 1}, "frozen", 0, args, 0.0, latest=finder))
        # No turn ever seen.
        self.assertIsNone(climb.resume_from_autosave({"last_turn": None}, "frozen", 0, args, 0.0, latest=finder))
        # Already on an end screen: the game is over, whatever the watchdog saw.
        self.assertIsNone(climb.resume_from_autosave(
            {"last_turn": 231, "end_screen_turn": 231}, "frozen", 0, args, 0.0, latest=finder))
        # Budget spent.
        self.assertIsNone(climb.resume_from_autosave(frozen, "frozen", 2, args, 0.0, latest=finder))
        # No autosave since the attempt began: nothing to reload.
        self.assertIsNone(climb.resume_from_autosave(frozen, "frozen", 0, args, 0.0,
                                                     latest=lambda newer_than=None: None))

    def test_a_frozen_attempt_is_reloaded_under_a_cont_tag_and_scored_from_it(self):
        spawned = []

        class Recording(FakeProc):
            def __init__(self, argv, *args, **kwargs):
                spawned.append(list(argv))
                super().__init__(argv, *args, **kwargs)

        verdicts = ["frozen", "exited"]
        saved_wait = climb.wait_watching_the_turn
        saved_latest = climb._latest_autosave
        climb.wait_watching_the_turn = lambda *a, **k: verdicts.pop(0)
        climb._latest_autosave = lambda newer_than=None: Path("/saves/AutoSave_0102.Civ6Save")
        climb.subprocess.Popen = Recording
        try:
            code, rows = self.climb_with(
                [{"last_turn": 102, "last_score": 340, "rival_best": 324},
                 {"last_turn": 250, "last_score": 910, "rival_best": 880}],
                attempts=1)
        finally:
            climb.wait_watching_the_turn = saved_wait
            climb._latest_autosave = saved_latest
            climb.subprocess.Popen = FakeProc

        self.assertEqual(code, 1, "played out, no win")
        self.assertEqual(len(rows), 1, "one attempt, one row — the freeze is not a row")
        row = rows[0]
        self.assertEqual(row["attempt"], 1)
        self.assertEqual(row["last_turn"], 250, "the score is the continuation's")
        self.assertEqual(row["last_score"], 910)
        # (The rig's scripted outcome carries no `reason`, so the climb stamps
        # the watcher's verdict — "exited" here, as for every rig row.)
        self.assertNotEqual(row.get("reason"), "attempt frozen",
                            "a resumed-and-finished game is not 'attempt frozen'")
        cont = row["resumed_from"] + "-cont1"
        self.assertEqual(row["resumes"], [{"tag": cont, "from_turn": 102,
                                           "save": "AutoSave_0102.Civ6Save"}])

        plays = [argv for argv in spawned if any("civ6_play.py" in str(w) for w in argv)]
        self.assertEqual(len(plays), 2, "the original launch and one continuation")
        first, second = plays
        self.assertNotIn("--load-save", first)
        self.assertIn("--load-save", second)
        self.assertEqual(second[second.index("--load-save") + 1], "/saves/AutoSave_0102.Civ6Save")
        self.assertEqual(second[second.index("--tag") + 1], cont)
        self.assertEqual(first[first.index("--tag") + 1], row["resumed_from"])
        # Everything else about the continuation is the original command.
        def without(argv, *flags):
            out, skip = [], False
            for word in argv:
                if skip:
                    skip = False
                    continue
                if word in flags:
                    skip = True
                    continue
                out.append(word)
            return out
        self.assertEqual(without(first, "--tag", "--orders-db"),
                         without(second, "--tag", "--orders-db", "--load-save"))

    def test_the_resume_budget_is_bounded_and_the_last_freeze_is_the_row(self):
        verdicts = ["frozen", "frozen", "frozen"]
        saved_wait = climb.wait_watching_the_turn
        saved_latest = climb._latest_autosave
        climb.wait_watching_the_turn = lambda *a, **k: verdicts.pop(0)
        climb._latest_autosave = lambda newer_than=None: Path("/saves/AutoSave_0150.Civ6Save")
        try:
            code, rows = self.climb_with(
                [{"last_turn": 102}, {"last_turn": 140}, {"last_turn": 151}],
                attempts=1)
        finally:
            climb.wait_watching_the_turn = saved_wait
            climb._latest_autosave = saved_latest

        self.assertEqual(len(rows), 1)
        row = rows[0]
        self.assertEqual(row["last_turn"], 151)
        self.assertEqual(row["reason"], "attempt frozen", "still frozen after the budget: say so")
        self.assertEqual([r["from_turn"] for r in row["resumes"]], [102, 140])
        self.assertTrue(row["resumes"][-1]["tag"].endswith("-cont2"))

    def test_a_resume_that_never_reaches_a_turn_keeps_the_frozen_row(self):
        verdicts = ["frozen", "exited"]
        saved_wait = climb.wait_watching_the_turn
        saved_latest = climb._latest_autosave
        climb.wait_watching_the_turn = lambda *a, **k: verdicts.pop(0)
        climb._latest_autosave = lambda newer_than=None: Path("/saves/AutoSave_0102.Civ6Save")
        try:
            code, rows = self.climb_with(
                [{"last_turn": 102, "last_score": 340}, {"last_turn": None}],
                attempts=1)
        finally:
            climb.wait_watching_the_turn = saved_wait
            climb._latest_autosave = saved_latest
        self.assertEqual(len(rows), 1, "the frozen game is the row, not a hole")
        row = rows[0]
        self.assertEqual(row["attempt"], 1, "and it spends its rung like any played game")
        self.assertEqual(row["last_turn"], 102)
        self.assertEqual(row["last_score"], 340)
        self.assertEqual(row["reason"], "attempt frozen; resume failed")
        self.assertTrue(row["resume_failed"]["tag"].endswith("-cont1"))
        self.assertEqual([r["from_turn"] for r in row["resumes"]], [102])

    def test_resumes_can_be_switched_off(self):
        verdicts = ["frozen"]
        saved_wait = climb.wait_watching_the_turn
        saved_latest = climb._latest_autosave
        climb.wait_watching_the_turn = lambda *a, **k: verdicts.pop(0)
        climb._latest_autosave = lambda newer_than=None: Path("/saves/AutoSave_0102.Civ6Save")
        try:
            code, rows = self.climb_with([{"last_turn": 102}], attempts=1,
                                         argv_extra=("--max-resumes", "0"))
        finally:
            climb.wait_watching_the_turn = saved_wait
            climb._latest_autosave = saved_latest
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["reason"], "attempt frozen")
        self.assertNotIn("resumes", rows[0])


class LadderBackfillTests(unittest.TestCase):
    """The loop replays whatever the automatic recording missed.

    Recording a summary is best-effort by design — `civ6_play.py` swallows the
    failure so a bookkeeping error can never cost a finished game. That trade
    only holds if something comes back for the misses, and until #1835 nothing
    did: 41 summaries sat unrecorded for three days, one of them the project's
    first Settler victory.
    """

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.runs = Path(self.tmp.name) / "control"
        self.runs.mkdir()
        patch = mock.patch.object(climb, "RUN_ROOT", self.runs)
        patch.start()
        self.addCleanup(patch.stop)
        # An absent live ledger seeds from the committed snapshot by design;
        # point that at a temp path so the fixture is the whole history here.
        import civ6_ladder
        snapshot = mock.patch.object(
            civ6_ladder, "DATA", Path(self.tmp.name) / "civ6_ladder.json")
        snapshot.start()
        self.addCleanup(snapshot.stop)

    def write_summary(self, tag: str, **extra) -> None:
        body = {"tag": tag, "finished_utc": "2026-08-16T06:49:58Z",
                "difficulty": "DIFFICULTY_SETTLER", "configured": True,
                "last_turn": 250, "last_score": 1021, "reason": "stopped",
                "outcome": None}
        body.update(extra)
        run_dir = self.runs / tag
        run_dir.mkdir()
        (run_dir / "summary.json").write_text(json.dumps(body))

    def test_an_unrecorded_summary_is_recorded_before_the_next_attempt(self):
        self.write_summary("civvis-dropped")
        climb.heal_the_ladder()
        state = json.loads((self.runs / "ladder.json").read_text())
        self.assertEqual([a["tag"] for a in state["attempts"]],
                         ["civvis-dropped"])

    def test_a_dropped_win_still_claims_its_rung(self):
        self.write_summary("civvis-dropped-win", outcome={
            "kind": "victory", "won": True, "victory": 0})
        climb.heal_the_ladder()
        state = json.loads((self.runs / "ladder.json").read_text())
        self.assertEqual(state["wins"]["DIFFICULTY_SETTLER"]["tag"],
                         "civvis-dropped-win")

    def test_a_broken_ledger_never_costs_an_attempt(self):
        # The record is worth less than the game: a backfill that cannot run
        # must report and return, not raise into the loop.
        (self.runs / "ladder.json").write_text("{ this is not json")
        self.write_summary("civvis-1")
        climb.heal_the_ladder()  # must not raise


class BatchRefreshSecondsTests(unittest.TestCase):
    """A pinned batch measures one program; a single attempt keeps live upgrades.

    The brain re-execs itself onto every origin/main advance at a turn
    boundary, so without this route a "pinned" batch still measures a moving
    program — the ledger stamped one `code_rev` on a run whose decider walked
    through four revisions.
    """

    def test_a_pinned_batch_freezes_the_decider(self):
        self.assertEqual(climb.batch_refresh_seconds(None, "abc1234", 8), 0.0)

    def test_a_single_attempt_keeps_the_brains_live_upgrade(self):
        self.assertIsNone(climb.batch_refresh_seconds(None, "abc1234", 1))

    def test_an_unpinned_batch_is_left_alone(self):
        self.assertIsNone(climb.batch_refresh_seconds(None, None, 8))

    def test_the_operators_choice_always_stands(self):
        self.assertEqual(climb.batch_refresh_seconds(30.0, "abc1234", 8), 30.0)
        self.assertEqual(climb.batch_refresh_seconds(0.0, None, 1), 0.0)

    @staticmethod
    def _play_args(**changes):
        from types import SimpleNamespace
        values = dict(
            difficulty="DIFFICULTY_SETTLER", map_size="MAPSIZE_SMALL",
            speed="GAMESPEED_ONLINE", leader=None, max_turns=250,
            timeout=7200.0, timeout_ceiling=None, probe_citizens=False,
            campus_specialist=False, envoys=False, envoy_place=False,
            envoy_levy=False, envoy_consider=False, victory="science",
            strategy="auto", war_from_plan=False, tile_export_every=25,
            refresh_seconds=None, no_peace_deterrence=False, without=[],
            no_counter_resolutions=False, combat_frames=0, replan_frames=2,
        )
        values.update(changes)
        return SimpleNamespace(**values)

    def test_the_abandon_floor_reaches_the_play_command_only_when_set(self):
        """Operator request 2026-08-19 — "ok to abandon games early if
        expected win rate <5%": the floor is forwarded verbatim, and absent it
        the harness keeps its own default of playing every game out."""
        cmd = climb.play_command(
            self._play_args(abandon_below_win_rate=0.05), "t",
            Path("orders.sqlite"), Path("civvis_orders"))
        self.assertIn("--abandon-below-win-rate", cmd)
        self.assertEqual(cmd[cmd.index("--abandon-below-win-rate") + 1], "0.05")
        self.assertNotIn(
            "--abandon-below-win-rate",
            climb.play_command(self._play_args(), "t",
                               Path("orders.sqlite"), Path("civvis_orders")))

    def test_the_mid_turn_frames_reach_the_play_command(self):
        """The combat frame (#2132) was never forwarded by the climb, so no
        ladder run ever played it; both frame counts now cross verbatim, the
        replan frames on by default and withholdable with 0."""
        cmd = climb.play_command(self._play_args(), "t",
                                 Path("orders.sqlite"), Path("civvis_orders"))
        self.assertEqual(cmd[cmd.index("--replan-frames") + 1], "2")
        self.assertEqual(cmd[cmd.index("--combat-frames") + 1], "0")
        cmd = climb.play_command(self._play_args(replan_frames=0, combat_frames=1), "t",
                                 Path("orders.sqlite"), Path("civvis_orders"))
        self.assertEqual(cmd[cmd.index("--replan-frames") + 1], "0")
        self.assertEqual(cmd[cmd.index("--combat-frames") + 1], "1")

    def test_a_withheld_deterrence_reaches_the_play_command(self):
        cmd = climb.play_command(self._play_args(no_peace_deterrence=True), "t",
                                 Path("orders.sqlite"), Path("civvis_orders"))
        self.assertIn("--no-peace-deterrence", cmd)
        cmd = climb.play_command(self._play_args(), "t",
                                 Path("orders.sqlite"), Path("civvis_orders"))
        self.assertNotIn("--no-peace-deterrence", cmd)

    def test_a_congress_control_batch_reaches_the_play_command(self):
        cmd = climb.play_command(
            self._play_args(no_counter_resolutions=True), "t",
            Path("orders.sqlite"), Path("civvis_orders"))
        self.assertIn("--no-counter-resolutions", cmd)
        self.assertNotIn(
            "--no-counter-resolutions",
            climb.play_command(self._play_args(), "t",
                               Path("orders.sqlite"), Path("civvis_orders")),
            "the treatment half withholds nothing",
        )

    def test_a_batch_can_be_the_control_half_of_a_live_ab(self):
        cmd = climb.play_command(
            self._play_args(without=["peacetime-deterrence"]), "t",
            Path("orders.sqlite"), Path("civvis_orders"))
        at = cmd.index("--civvis-without")
        self.assertEqual(cmd[at + 1], "peacetime-deterrence")
        self.assertNotIn(
            "--civvis-without",
            climb.play_command(self._play_args(), "t",
                               Path("orders.sqlite"), Path("civvis_orders")),
            "the treatment half withholds nothing",
        )

    def test_the_freeze_reaches_the_play_command(self):
        cmd = climb.play_command(self._play_args(refresh_seconds=0.0), "t",
                                 Path("orders.sqlite"), Path("civvis_orders"))
        at = cmd.index("--civvis-refresh-seconds")
        self.assertEqual(cmd[at + 1], "0.0")

    def test_no_choice_sends_no_refresh_flag(self):
        cmd = climb.play_command(self._play_args(), "t",
                                 Path("orders.sqlite"), Path("civvis_orders"))
        self.assertNotIn("--civvis-refresh-seconds", cmd)
