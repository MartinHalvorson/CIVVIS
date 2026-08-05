"""Tests for the game lock's foreign-run check.

The case that matters is the WEDGE: a harness that dies without teardown leaves
its run tag written into the installed mod, and every later launch is "foreign"
to a process that no longer exists. Civilization VI stays up, so the guard never
expires on its own.

Run: python3 -m unittest tools/civ6_control/test_gamelock.py
"""

from __future__ import annotations

import os
import sys
import unittest
from pathlib import Path

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


if __name__ == "__main__":
    unittest.main()
