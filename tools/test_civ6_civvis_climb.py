"""The climb loop's attempt accounting.

⚠ THIS IS THE FIRST TEST OF ANY civ6_* TOOL, and it exists because the harness spent
eleven of twenty-four attempts in two minutes on a machine where Steam had exited,
then printed "no win in the attempts given". Nothing was wrong with the game, the
mod, or CIVVIS. The loop counted ITERATIONS and reported them as GAMES.

The property under test is one sentence: an attempt that produced no turn does not
spend a rung of the budget. Everything below is that sentence from a different angle.
"""

from pathlib import Path
import json
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_civvis_climb as climb


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
        self.ledger = root / "ladder.jsonl"
        self.orders_bin = root / "civvis_orders"
        self.orders_bin.write_text("#!/bin/sh\n")

        self.saved = {name: getattr(climb, name) for name in
                      ("LEDGER", "BLOCKED_BACKOFF_S", "teardown", "busy",
                       "wake_steam", "outcome_of", "code_state")}
        climb.LEDGER = self.ledger
        climb.BLOCKED_BACKOFF_S = (0.0, 0.0, 0.0)   # the table, without the waiting
        climb.teardown = lambda: None
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


if __name__ == "__main__":
    unittest.main()
