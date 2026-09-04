#!/usr/bin/env python3
"""Verification startup cuts and front-end acknowledgements reach the game files.

`civ6_setup.VERIFICATION_OPTIONS` turns off the intro video, the historic-moment
animation and two shadow passes, and saves the game's own acknowledgement of
its known native startup warnings. None changes the game that is played. These
options are only useful if they land in the game's own files, in the keys this
version defines, while the game is closed -- so the tests exercise the real
rewrite on copies of the real file shapes, the guards around it, and the place
`civ6_play.py` calls it: immediately before the launch.
"""

from __future__ import annotations

import inspect
import os
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_env as env  # noqa: E402
import civ6_play  # noqa: E402
import civ6_setup  # noqa: E402

APP_OPTIONS = """;Application options
[Video]
RenderWidth 1650
;Set to 1 play the intro video on startup.
PlayIntroVideo 1
[Misc]
AcceptedUnknownDevice 0
AcceptedOutdatedDriver 0
EnableTuner 1
"""

USER_OPTIONS = """[Game]
AutoEndTurn 0
QuickMovement 1
QuickCombat 1
PlayHistoricMomentAnimation 1
GameEffectsLogLevel 2
"""

GRAPHICS_OPTIONS = """Version 10
[Video]
PerformanceImpact 2
MSAA 2
[Shadows]
EnableShadows 1
[CloudShadows]
EnableCloudShadows 1
[Leaders]
Quality 0
"""


def write_user_dir(root: Path, graphics: str | None = GRAPHICS_OPTIONS) -> Path:
    (root / "AppOptions.txt").write_text(APP_OPTIONS)
    (root / "UserOptions.txt").write_text(USER_OPTIONS)
    if graphics is not None:
        (root / "GraphicsOptions.txt").write_text(graphics)
    return root


class TheVerificationOptionsAreAppliedInPlace(unittest.TestCase):
    def test_every_key_is_rewritten_and_nothing_else_moves(self) -> None:
        with TemporaryDirectory() as tmp:
            user = write_user_dir(Path(tmp))
            applied = civ6_setup.apply_verification(user)
            self.assertEqual(applied, {
                "AppOptions.txt": {
                    "PlayIntroVideo": ("1", 0),
                    "AcceptedUnknownDevice": ("0", 1),
                    "AcceptedOutdatedDriver": ("0", 1),
                },
                "UserOptions.txt": {"PlayHistoricMomentAnimation": ("1", 0)},
                "GraphicsOptions.txt": {"EnableShadows": ("1", 0),
                                        "EnableCloudShadows": ("1", 0)},
            })
            self.assertEqual(env.read_option(user / "AppOptions.txt", "PlayIntroVideo"), "0")
            self.assertEqual(env.read_option(user / "AppOptions.txt", "AcceptedUnknownDevice"), "1")
            self.assertEqual(env.read_option(user / "AppOptions.txt", "AcceptedOutdatedDriver"), "1")
            self.assertEqual(env.read_option(user / "GraphicsOptions.txt", "EnableShadows"), "0")
            self.assertEqual(env.read_option(user / "GraphicsOptions.txt", "EnableCloudShadows"), "0")
            # The cuts are cosmetic and stay cosmetic: what the game plays is untouched.
            self.assertEqual(env.read_option(user / "UserOptions.txt", "QuickCombat"), "1")
            self.assertEqual(env.read_option(user / "UserOptions.txt", "GameEffectsLogLevel"), "2")
            self.assertEqual(env.read_option(user / "GraphicsOptions.txt", "MSAA"), "2")
            self.assertEqual(env.read_option(user / "AppOptions.txt", "EnableTuner"), "1")
            # The game's own comment above the key survives the rewrite.
            self.assertIn(";Set to 1 play the intro video on startup.",
                          (user / "AppOptions.txt").read_text())

    def test_a_configured_install_reports_no_change(self) -> None:
        with TemporaryDirectory() as tmp:
            user = write_user_dir(Path(tmp))
            civ6_setup.apply_verification(user)
            self.assertEqual(civ6_setup.apply_verification(user), {})

    def test_a_key_this_version_does_not_define_is_reported_not_added(self) -> None:
        with TemporaryDirectory() as tmp:
            user = write_user_dir(Path(tmp), graphics="Version 10\n[Shadows]\nEnableShadows 1\n")
            applied = civ6_setup.apply_verification(user)
            self.assertEqual(applied["GraphicsOptions.txt"],
                             {"EnableShadows": ("1", 0), "EnableCloudShadows": (None, 0)})
            self.assertNotIn("EnableCloudShadows", (user / "GraphicsOptions.txt").read_text())

    def test_a_file_the_game_has_not_written_is_skipped(self) -> None:
        with TemporaryDirectory() as tmp:
            user = write_user_dir(Path(tmp), graphics=None)
            applied = civ6_setup.apply_verification(user)
            self.assertNotIn("GraphicsOptions.txt", applied)
            self.assertFalse((user / "GraphicsOptions.txt").exists())

    def test_revert_restores_the_games_own_values(self) -> None:
        with TemporaryDirectory() as tmp:
            user = write_user_dir(Path(tmp))
            civ6_setup.apply_verification(user)
            civ6_setup.apply_verification(user, civ6_setup.VERIFICATION_DEFAULTS)
            for name, keys in civ6_setup.VERIFICATION_OPTIONS.items():
                for key in keys:
                    self.assertEqual(env.read_option(user / name, key),
                                     str(civ6_setup.VERIFICATION_DEFAULTS[name][key]),
                                     f"{name}: {key}")

    def test_the_defaults_cover_exactly_the_keys_that_are_cut(self) -> None:
        cut = {(name, key) for name, keys in civ6_setup.VERIFICATION_OPTIONS.items()
               for key in keys}
        restored = {(name, key) for name, keys in civ6_setup.VERIFICATION_DEFAULTS.items()
                    for key in keys}
        self.assertEqual(cut, restored)

    def test_the_command_line_applies_and_reports_them(self) -> None:
        with TemporaryDirectory() as tmp:
            user = write_user_dir(Path(tmp))
            with mock.patch.dict(os.environ, {"CIV6_USER_DIR": str(user)}), \
                    mock.patch.object(civ6_setup.env, "game_pids", return_value=[]), \
                    mock.patch("sys.stdout") as out:
                self.assertEqual(civ6_setup.main(["--verification"]), 0)
            printed = "".join(call.args[0] for call in out.write.call_args_list)
            self.assertIn("PlayIntroVideo 1 -> 0", printed)
            self.assertEqual(env.read_option(user / "AppOptions.txt", "PlayIntroVideo"), "0")
            self.assertEqual(env.read_option(user / "AppOptions.txt", "AcceptedUnknownDevice"), "1")
            self.assertEqual(env.read_option(user / "AppOptions.txt", "AcceptedOutdatedDriver"), "1")
            # The logging channels are a separate decision and were not touched.
            self.assertEqual(env.read_option(user / "AppOptions.txt", "EnableTuner"), "1")


class TheHarnessAppliesThemRightBeforeLaunching(unittest.TestCase):
    def test_the_cuts_land_when_the_game_is_closed(self) -> None:
        with TemporaryDirectory() as tmp:
            user = write_user_dir(Path(tmp))
            with mock.patch.dict(os.environ, {"CIV6_USER_DIR": str(user)}), \
                    mock.patch.object(civ6_play.env, "game_pids", return_value=[]):
                applied = civ6_play.apply_verification_options()
            self.assertEqual(set(applied), {"AppOptions.txt", "UserOptions.txt",
                                            "GraphicsOptions.txt"})
            self.assertEqual(env.read_option(user / "AppOptions.txt", "PlayIntroVideo"), "0")

    def test_a_running_game_keeps_its_files(self) -> None:
        """The game rewrites its options on exit, so an edit now would be lost --
        and it is not the harness's file to touch while another game holds it."""
        with TemporaryDirectory() as tmp:
            user = write_user_dir(Path(tmp))
            with mock.patch.dict(os.environ, {"CIV6_USER_DIR": str(user)}), \
                    mock.patch.object(civ6_play.env, "game_pids", return_value=[4242]):
                self.assertEqual(civ6_play.apply_verification_options(), {})
            self.assertEqual(env.read_option(user / "AppOptions.txt", "PlayIntroVideo"), "1")

    def test_a_failure_to_apply_never_costs_the_launch(self) -> None:
        with mock.patch.object(civ6_play.env, "game_pids", return_value=[]), \
                mock.patch.object(civ6_setup, "apply_verification",
                                  side_effect=OSError("disk says no")):
            self.assertEqual(civ6_play.apply_verification_options(), {})

    def test_play_applies_them_before_the_launch_and_can_be_told_not_to(self) -> None:
        # `play` is a thin wrapper; the launch sequence lives in `_play`.
        source = inspect.getsource(civ6_play._play)
        hook = source.index("apply_verification_options()")
        launch = source.index("launcher.launch(")
        self.assertLess(hook, launch, "the options must be written before the game "
                                      "opens its files, not after")
        self.assertIn("args.keep_game_options", source[:launch])


if __name__ == "__main__":
    unittest.main()
