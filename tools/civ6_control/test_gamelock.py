"""Tests for the game lock's foreign-run check.

The case that matters is the WEDGE: a harness that dies without teardown leaves
its run tag written into the installed mod, and every later launch is "foreign"
to a process that no longer exists. Civilization VI stays up, so the guard never
expires on its own.

Run: python3 -m unittest tools/civ6_control/test_gamelock.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from civ6_control import gamelock  # noqa: E402


class TagHasLiveOwner(unittest.TestCase):
    def setUp(self) -> None:
        self._real = gamelock._processes
        self.addCleanup(lambda: setattr(gamelock, "_processes", self._real))

    def fake(self, rows: list[tuple[int, str]]) -> None:
        gamelock._processes = lambda: rows

    def test_a_driving_harness_counts_as_an_owner(self) -> None:
        self.fake([(4242, "python3 -u tools/civ6_play.py --tag run-A --export-state")])
        self.assertTrue(gamelock._tag_has_live_owner("run-A"))

    def test_the_brain_also_counts(self) -> None:
        self.fake([(4243, "python3 tools/civ6_brain.py --run-dir /runs/run-A")])
        self.assertTrue(gamelock._tag_has_live_owner("run-A"))

    def test_a_tag_with_nothing_behind_it_is_not_owned(self) -> None:
        """The wedge. This is the whole point of the change."""
        self.fake([(1, "/sbin/launchd"), (99, "python3 tools/civ6_play.py --tag run-B")])
        self.assertFalse(gamelock._tag_has_live_owner("run-A"))

    def test_the_tag_alone_is_not_an_owner(self) -> None:
        """A tail, an editor, or a grep carrying the tag holds no game."""
        self.fake([
            (500, "tail -f /Users/x/civvis-civ6-runs/control/run-A-play.log"),
            (501, "grep -r run-A /Users/x/civvis-civ6-runs"),
        ])
        self.assertFalse(gamelock._tag_has_live_owner("run-A"))

    def test_this_process_never_counts_as_the_owner(self) -> None:
        """The self-match trap: a caller carrying the tag must not find itself."""
        self.fake([(os.getpid(), "python3 tools/civ6_play.py --tag run-A")])
        self.assertFalse(gamelock._tag_has_live_owner("run-A"))

    def test_the_parent_never_counts_either(self) -> None:
        self.fake([(os.getppid(), "python3 tools/civ6_civvis_climb.py --tag run-A")])
        self.assertFalse(gamelock._tag_has_live_owner("run-A"))

    def test_an_unreadable_process_table_fails_closed(self) -> None:
        """Cannot prove it is dead -> must not hand out the game."""
        self.fake([])
        self.assertTrue(gamelock._tag_has_live_owner("run-A"))


class ForeignRun(unittest.TestCase):
    """`foreign_run` end to end, with the installation stubbed out."""

    def setUp(self) -> None:
        import civ6_env as env

        self._pids, self._assets = env.game_pids, env.assets_dir
        self._procs = gamelock._processes
        self.addCleanup(lambda: setattr(env, "game_pids", self._pids))
        self.addCleanup(lambda: setattr(env, "assets_dir", self._assets))
        self.addCleanup(lambda: setattr(gamelock, "_processes", self._procs))
        self.env = env

    def install(self, tmp: Path, tag: str | None) -> None:
        cfg = tmp / "DLC" / "CivvisControl"
        cfg.mkdir(parents=True, exist_ok=True)
        import json

        (cfg / "config.json").write_text(json.dumps({"RunTag": tag}))
        self.env.assets_dir = lambda *a, **k: tmp

    def test_a_dead_tag_does_not_block_a_new_run(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as td:
            self.install(Path(td), "civvis-DEAD")
            self.env.game_pids = lambda: [1234]          # the game is up
            gamelock._processes = lambda: [(1, "/sbin/launchd")]
            self.assertIsNone(gamelock.foreign_run("civvis-NEW"))

    def test_a_live_foreign_run_still_blocks(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as td:
            self.install(Path(td), "civvis-LIVE")
            self.env.game_pids = lambda: [1234]
            gamelock._processes = lambda: [
                (777, "python3 tools/civ6_play.py --tag civvis-LIVE"),
            ]
            answer = gamelock.foreign_run("civvis-NEW")
            self.assertIsNotNone(answer)
            self.assertIn("civvis-LIVE", answer)

    def test_our_own_tag_is_never_foreign(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as td:
            self.install(Path(td), "civvis-MINE")
            self.env.game_pids = lambda: [1234]
            gamelock._processes = lambda: [
                (777, "python3 tools/civ6_play.py --tag civvis-MINE"),
            ]
            self.assertIsNone(gamelock.foreign_run("civvis-MINE"))

    def test_no_game_running_is_never_foreign(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as td:
            self.install(Path(td), "civvis-DEAD")
            self.env.game_pids = lambda: []
            self.assertIsNone(gamelock.foreign_run("civvis-NEW"))


class ExplicitOperatorHalt(unittest.TestCase):
    """A halt must survive the process that asked for it.

    The old live-lock-only design lost a halt as soon as its helper process was
    reaped, allowing an interactive host to race straight into another game.
    """

    def setUp(self) -> None:
        self.tmp = TemporaryDirectory()
        root = Path(self.tmp.name)
        self._lock = gamelock.LOCK
        self._halt = gamelock.OPERATOR_HALT
        self._foreign = gamelock.foreign_run
        gamelock.LOCK = root / "game.lock"
        gamelock.OPERATOR_HALT = root / "operator-halt.json"
        gamelock.foreign_run = lambda tag: None
        self.addCleanup(self._restore)

    def _restore(self) -> None:
        gamelock.release(force=True)
        gamelock.LOCK = self._lock
        gamelock.OPERATOR_HALT = self._halt
        gamelock.foreign_run = self._foreign
        self.tmp.cleanup()

    def test_halt_blocks_acquisition_and_names_the_operator_reason(self) -> None:
        gamelock.request_operator_halt("close the game windows")

        self.assertFalse(gamelock.acquire("civvis-new"))
        description = gamelock.standing_hold()
        self.assertIsNotNone(description)
        self.assertIn("explicitly halted", description)
        self.assertIn("close the game windows", description)

    def test_resume_is_explicit_and_reopens_the_lock(self) -> None:
        gamelock.request_operator_halt()
        self.assertFalse(gamelock.acquire("civvis-new"))

        self.assertTrue(gamelock.clear_operator_halt())
        self.assertTrue(gamelock.acquire("civvis-new"))

    def test_a_malformed_marker_fails_closed(self) -> None:
        gamelock.OPERATOR_HALT.write_text("not json\n")

        self.assertFalse(gamelock.acquire("civvis-new"))
        description = gamelock.standing_hold()
        self.assertIsNotNone(description)
        self.assertIn("unreadable explicit operator halt marker", description)

    def test_halt_status_answers_only_the_explicit_marker(self) -> None:
        """A live holder that drives no run is a standing hold, not a halt.

        `--hold-status` deliberately reports both; `--halt-status` is what a
        process that will STOP something must ask, because the standing half is
        the few-second window between attempts (see the interactive host).
        """
        env = {**os.environ,
               "CIVVIS_OPERATOR_HALT_FILE": str(gamelock.OPERATOR_HALT),
               "CIVVIS_GAME_LOCK_DIR": str(gamelock.LOCK)}
        cli = [sys.executable, str(Path(gamelock.__file__))]
        # Nothing halted, nothing held: both answer no.
        self.assertEqual(
            subprocess.run(cli + ["--halt-status"], env=env,
                           capture_output=True, text=True, timeout=5).returncode, 1)
        # A live holder under a tag nobody drives: standing hold yes, halt no.
        self.assertTrue(gamelock.acquire("civvis-test-orphan-tag"))
        try:
            standing = subprocess.run(cli + ["--hold-status"], env=env,
                                      capture_output=True, text=True, timeout=5)
            self.assertEqual(standing.returncode, 0, standing.stderr)
            self.assertIn("no harness is driving", standing.stdout)
            halt = subprocess.run(cli + ["--halt-status"], env=env,
                                  capture_output=True, text=True, timeout=5)
            self.assertEqual(halt.returncode, 1,
                             f"a standing hold is not an operator halt: {halt.stdout}")
        finally:
            gamelock.release()
        # The explicit marker: both answer yes.
        gamelock.request_operator_halt("maintenance")
        halt = subprocess.run(cli + ["--halt-status"], env=env,
                              capture_output=True, text=True, timeout=5)
        self.assertEqual(halt.returncode, 0, halt.stderr)
        self.assertIn("explicitly halted", halt.stdout)

    def test_hold_status_cli_has_a_machine_readable_exit_code(self) -> None:
        gamelock.request_operator_halt("maintenance")
        result = subprocess.run(
            [sys.executable, str(Path(gamelock.__file__)), "--hold-status"],
            env={**os.environ,
                 "CIVVIS_OPERATOR_HALT_FILE": str(gamelock.OPERATOR_HALT)},
            capture_output=True, text=True, timeout=5,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("explicitly halted", result.stdout)


if __name__ == "__main__":
    unittest.main()
