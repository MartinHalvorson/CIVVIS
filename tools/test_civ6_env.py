#!/usr/bin/env python3
"""Regression checks for safe Civilization VI process detection."""

from __future__ import annotations

import os
import re
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_env  # noqa: E402


class Civ6EnvTest(unittest.TestCase):
    def test_game_process_detection_ignores_osascript_argument_text(self) -> None:
        listing = """
 101 /usr/bin/osascript -e tell application \"System Events\" to tell process \"Civ6_Exe_Child\" to get name
 102 /bin/zsh -lc pgrep -f Civ6_Exe
 103 /Users/martbot/Library/Application Support/Steam/steamapps/common/Sid Meier's Civilization VI/Civ6.app/Contents/MacOS/Civ6_Exe_Child
 104 /Users/martbot/Library/Application Support/Steam/steamapps/common/Sid Meier's Civilization VI/Civ6.app/Contents/MacOS/Civ6_Exe --child
 105 /Applications/Steam.app/Contents/MacOS/steam_osx
"""

        self.assertEqual(civ6_env._game_pids_from_ps(listing), [103, 104])


class InstallResolutionTest(unittest.TestCase):
    """One resolver, and it covers the platform the fleet actually runs on."""

    def test_the_candidate_list_covers_macos_and_windows(self):
        r"""⚠ `civ6_fidelity.py` shipped four candidates and every one began
        `C:\` or `D:\`, on a fleet that runs entirely on macOS. The audit
        that checks we are modelling Gathering Storm rather than Vanilla
        therefore never found an install and never ran."""
        rendered = [str(path) for path in civ6_env.INSTALL_CANDIDATES]
        self.assertTrue(
            any("Library/Application Support/Steam" in path for path in rendered),
            "a macOS Steam install must be findable",
        )
        self.assertTrue(
            any(path.startswith(("C:", "D:", "E:")) for path in rendered),
            "the Windows candidates must survive the move into this module",
        )

    def test_both_historical_environment_variables_are_honoured(self):
        """`$CIV6_DIR` was `civ6_fidelity.py`'s and `civ6_type_names.py`'s;
        `$CIV6_INSTALL` was everything else's. Dropping either would break
        whatever already exports it."""
        self.assertEqual(civ6_env.INSTALL_ENV_VARS, ("CIV6_INSTALL", "CIV6_DIR"))
        for name in civ6_env.INSTALL_ENV_VARS:
            with self.subTest(variable=name):
                with tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary)
                    env = {other: "" for other in civ6_env.INSTALL_ENV_VARS}
                    env[name] = str(root)
                    with mock.patch.dict(os.environ, env, clear=False):
                        self.assertEqual(civ6_env.install_dir(), root)

    def test_an_assets_directory_is_accepted_where_an_install_is_expected(self):
        """`--civ6` is passed by hand as both, and on macOS the useful path is
        the one inside the bundle."""
        with tempfile.TemporaryDirectory() as temporary:
            assets = Path(temporary) / "Assets"
            (assets / civ6_env.GAMEPLAY_DATA).mkdir(parents=True)
            self.assertEqual(civ6_env.assets_dir(str(assets)), assets)

    def test_an_install_root_resolves_into_the_macos_bundle(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "Sid Meier's Civilization VI"
            nested = root / civ6_env.ASSETS_SUBPATH
            (nested / civ6_env.GAMEPLAY_DATA).mkdir(parents=True)
            self.assertEqual(civ6_env.assets_dir(str(root)), nested)

    def test_no_other_tool_searches_for_an_install_of_its_own(self):
        """Discovered, not listed: any tool growing a private install search
        fails here rather than silently finding nothing on somebody's machine."""
        search = re.compile(
            r"Steam/steamapps/common/Sid Meier|SteamLibrary.{1,2}steamapps"
            r"|Program Files.{0,10}Steam"
        )
        allowed = {"civ6_env.py", "test_civ6_env.py"}
        offenders = sorted(
            path.name
            for path in Path(__file__).resolve().parent.glob("*.py")
            if path.name not in allowed
            and search.search(path.read_text(encoding="utf-8"))
        )
        self.assertEqual(offenders, [], "install search belongs in civ6_env.py alone")
        # Non-vacuity: the pattern really does match the module that owns it.
        owner = Path(__file__).resolve().parent / "civ6_env.py"
        self.assertTrue(search.search(owner.read_text(encoding="utf-8")))


class AStuckProcessListingMustNotLookLikeAnExitedGame(unittest.TestCase):
    """⚠⚠⚠ `game_pids` ran with no timeout, and it stopped a game being played.

    `watch.follow` calls it on EVERY poll, so an unbounded `ps` is an unbounded
    hang in the loop that drives Civilization VI. Measured 2026-08-28, run
    `civvis-20260828T160200Z`: the harness blocked here at turn 8 and never
    polled again. The wedge sample settled it — every Civ 6 thread was idle,
    the main thread parked in `_pthread_cond_wait` for 1638 of 1644 samples.
    The engine was not stuck; nothing was asking it to do anything.
    """

    LISTING = "  4242 /Steam/common/Sid Meier's Civilization VI/Civ6.app/Contents/MacOS/Civ6_Exe_Child\n"

    def test_a_listing_that_returns_is_read_normally(self):
        with mock.patch.object(civ6_env.subprocess, "run",
                               return_value=mock.Mock(stdout=self.LISTING)):
            self.assertEqual(civ6_env.game_pids(), [4242])

    def test_the_call_is_bounded(self):
        with mock.patch.object(civ6_env.subprocess, "run",
                               return_value=mock.Mock(stdout="")) as ran:
            civ6_env.game_pids()
        self.assertEqual(ran.call_args.kwargs.get("timeout"),
                         civ6_env.GAME_PIDS_TIMEOUT_S)

    def test_a_timeout_keeps_the_last_known_pids(self):
        """The whole point: unknown is not the same as none.

        `watch.follow` reads an empty list as "game exited" and ends the
        attempt, so answering a question we could not ask with `[]` would trade
        a hang for a silently discarded game.
        """
        with mock.patch.object(civ6_env.subprocess, "run",
                               return_value=mock.Mock(stdout=self.LISTING)):
            self.assertEqual(civ6_env.game_pids(), [4242])
        with mock.patch.object(
                civ6_env.subprocess, "run",
                side_effect=civ6_env.subprocess.TimeoutExpired("ps", 10)):
            self.assertEqual(civ6_env.game_pids(), [4242])

    def test_a_caller_cannot_mutate_the_remembered_listing(self):
        with mock.patch.object(civ6_env.subprocess, "run",
                               return_value=mock.Mock(stdout=self.LISTING)):
            got = civ6_env.game_pids()
        got.append(999)
        with mock.patch.object(
                civ6_env.subprocess, "run",
                side_effect=civ6_env.subprocess.TimeoutExpired("ps", 10)):
            self.assertEqual(civ6_env.game_pids(), [4242])

    def test_a_missing_ps_is_a_real_answer_of_none(self):
        """OSError is "there is no ps here", not "we could not ask"."""
        with mock.patch.object(civ6_env.subprocess, "run",
                               side_effect=OSError("no ps")):
            self.assertEqual(civ6_env.game_pids(), [])


if __name__ == "__main__":
    unittest.main()
