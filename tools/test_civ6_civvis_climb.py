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


class ClimbBudgetTests(unittest.TestCase):
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
                       "wake_steam", "outcome_of")}
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

    def climb_with(self, outcomes, attempts=3):
        """Run main() with a scripted sequence of per-attempt outcomes."""
        seq = list(outcomes)

        def outcome_of(tag):
            # The play log is what a blocked row quotes, so write one like the real
            # harness would before reading it back.
            record = seq.pop(0) if seq else {"last_turn": None}
            return dict(record)

        climb.outcome_of = outcome_of
        argv = sys.argv
        sys.argv = ["climb", "--attempts", str(attempts),
                    "--orders-bin", str(self.orders_bin),
                    "--logs", str(self.logs), "--timeout", "0.1"]
        try:
            code = climb.main()
        finally:
            sys.argv = argv
        rows = [json.loads(line) for line in
                self.ledger.read_text().splitlines()] if self.ledger.exists() else []
        return code, rows

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


if __name__ == "__main__":
    unittest.main()
