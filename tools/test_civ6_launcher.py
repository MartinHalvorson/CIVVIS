#!/usr/bin/env python3
"""Regression tests for naming a launch macOS refused, rather than stalling on it."""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import launcher  # noqa: E402

DAMAGED = ('"Civilization VI" is damaged and can\'t be opened. You should move '
           'it to the Trash., This file was downloaded on an unknown date.')


def _osascript(stdout: str, returncode: int = 0):
    return mock.patch.object(
        launcher.subprocess, "run",
        return_value=mock.Mock(stdout=stdout, stderr="", returncode=returncode))


class GatekeeperRefusalTest(unittest.TestCase):
    def test_the_damaged_modal_is_reported_with_its_own_words(self) -> None:
        with _osascript(DAMAGED):
            self.assertIn("damaged", launcher.gatekeeper_refusal())

    def test_some_other_agent_window_is_not_a_refusal(self) -> None:
        with _osascript("Software Update is available"):
            self.assertIsNone(launcher.gatekeeper_refusal())

    def test_no_window_at_all_is_not_a_refusal(self) -> None:
        """osascript exits non-zero when window 1 does not exist."""
        with _osascript("", returncode=1):
            self.assertIsNone(launcher.gatekeeper_refusal())

    def test_an_unavailable_osascript_is_not_a_refusal(self) -> None:
        with mock.patch.object(launcher.subprocess, "run", side_effect=OSError("no")):
            self.assertIsNone(launcher.gatekeeper_refusal())


class BundleSignatureTest(unittest.TestCase):
    """⚠ THESE MOCK THE PATH AS WELL AS `codesign`, AND THAT IS THE POINT.

    `bundle_signature_error` reads `game_binary()` before it runs anything, and
    `env.install_dir()` raises `SystemExit` on a machine with no Civilization VI
    — so on every host but a play host these three errored out before reaching
    the subprocess mock they were written around. Nothing noticed, because this
    file is not one of the suites the gate runs.

    Skipping them on such a host would have been the wrong repair: what they
    test is how `codesign`'s words are reported, not whether the game is
    installed. Naming the bundle makes them run everywhere, which is where a
    regression in that reporting would actually be caught.
    """

    def setUp(self) -> None:
        patched = mock.patch.object(
            launcher, "game_binary",
            return_value=Path("/Games/Civ6.app/Contents/MacOS/Civ6_Exe_Child"))
        patched.start()
        self.addCleanup(patched.stop)

    def test_a_valid_bundle_reports_nothing(self) -> None:
        with mock.patch.object(
                launcher.subprocess, "run",
                return_value=mock.Mock(returncode=0, stdout="", stderr="")):
            self.assertIsNone(launcher.bundle_signature_error())

    def test_the_mod_s_own_breakage_is_reported_in_codesign_s_words(self) -> None:
        """What installing into the DLC tree actually produces, verbatim."""
        stderr = ("/…/Civ6.app: a sealed resource is missing or invalid\n"
                  "file added: /…/Civ6.app/Contents/Assets/DLC/CivvisControl/config.json\n")
        with mock.patch.object(
                launcher.subprocess, "run",
                return_value=mock.Mock(returncode=1, stdout="", stderr=stderr)):
            self.assertIn("sealed resource", launcher.bundle_signature_error())

    def test_a_missing_codesign_is_reported_rather_than_raised(self) -> None:
        with mock.patch.object(launcher.subprocess, "run",
                               side_effect=subprocess.TimeoutExpired("codesign", 120)):
            self.assertIn("could not run codesign", launcher.bundle_signature_error())


class WaitForMainMenuTest(unittest.TestCase):
    """⚠ A REFUSED LAUNCH USED TO CONSUME THE WHOLE 420-SECOND ALLOWANCE.

    The process stays up, so the live-pid check reads as progress and the loop
    waits out its deadline. Measured 2026-08-07: six minutes per attempt with no
    Logs directory, no events, and nothing naming the modal on screen.
    """

    def test_final_content_marker_proves_the_menu_is_ready(self) -> None:
        with TemporaryDirectory() as tmp:
            log_dir = Path(tmp)
            (log_dir / "Modding.log").write_text(
                "Discovered 123 mods\n"
                + launcher.MAIN_MENU_READY_MARKER + "\n")
            with mock.patch.object(launcher.env, "logs_dir", return_value=log_dir), \
                 mock.patch.object(launcher.env, "game_pids") as pids:
                self.assertTrue(launcher.wait_for_main_menu(timeout_s=1.0))
        pids.assert_not_called()

    def test_successful_reconfiguration_keeps_the_ready_child_alive(self) -> None:
        # Native run 20260921T115736Z visibly reached the main menu, but
        # emitted the changed-content completion path rather than the
        # unchanged-content marker. The old wait killed it after 420 seconds.
        with TemporaryDirectory() as tmp:
            log_dir = Path(tmp)
            (log_dir / "Modding.log").write_text(
                "[3285481.864] Modding Framework - Finished Apply Settings\n"
                "[3285481.864] ConfigureContent - Importing implicit files, post settings\n"
                "[3285481.864] Successfully reconfigured game.\n")
            child = mock.Mock()
            child.poll.return_value = None
            with mock.patch.object(launcher.env, "logs_dir", return_value=log_dir), \
                 mock.patch.object(launcher.env, "game_pids", return_value=[]):
                self.assertTrue(launcher.wait_for_launched_main_menu(child, timeout_s=1.0))
            child.terminate.assert_not_called()
            child.kill.assert_not_called()

    def test_early_discovery_is_not_mistaken_for_an_interactive_menu(self) -> None:
        with TemporaryDirectory() as tmp:
            log_dir = Path(tmp)
            for incomplete in (
                "Discovered 123 mods\n",
                "Modding Framework - Finished Apply Settings\n",
                "ConfigureContent - Importing implicit files, post settings\n",
            ):
                with self.subTest(incomplete=incomplete):
                    (log_dir / "Modding.log").write_text(incomplete)
                    with mock.patch.object(launcher.env, "logs_dir", return_value=log_dir), \
                         mock.patch.object(launcher.env, "game_pids", return_value=[]), \
                         mock.patch.object(launcher, "gatekeeper_refusal") as asked:
                        self.assertFalse(launcher.wait_for_main_menu(timeout_s=1.0))
                    asked.assert_not_called()

    def test_a_refusal_returns_immediately_instead_of_waiting_out_the_timeout(self) -> None:
        slept = []
        with mock.patch.object(launcher.env, "logs_dir", return_value=Path("/nope")), \
             mock.patch.object(launcher.env, "game_pids", return_value=[123]), \
             mock.patch.object(launcher, "gatekeeper_refusal", return_value=DAMAGED), \
             mock.patch.object(launcher, "bundle_signature_error", return_value=None), \
             mock.patch.object(launcher.time, "sleep", side_effect=slept.append):
            self.assertFalse(launcher.wait_for_main_menu(timeout_s=420.0, poll_s=3.0))
        self.assertEqual(slept, [], "a refusal must not sleep through the allowance")

    def test_a_live_game_with_no_refusal_still_waits(self) -> None:
        """The refusal check must not turn an ordinary slow start into a failure."""
        with mock.patch.object(launcher.env, "logs_dir", return_value=Path("/nope")), \
             mock.patch.object(launcher.env, "game_pids", return_value=[123]), \
             mock.patch.object(launcher, "gatekeeper_refusal", return_value=None) as asked, \
             mock.patch.object(launcher.time, "sleep") as sleep:
            self.assertFalse(launcher.wait_for_main_menu(timeout_s=0.05, poll_s=0.01))
        self.assertTrue(sleep.called)
        asked.assert_called()

    def test_a_dead_process_still_fails_without_asking_macos(self) -> None:
        with mock.patch.object(launcher.env, "logs_dir", return_value=Path("/nope")), \
             mock.patch.object(launcher.env, "game_pids", return_value=[]), \
             mock.patch.object(launcher, "gatekeeper_refusal") as asked:
            self.assertFalse(launcher.wait_for_main_menu(timeout_s=420.0))
        asked.assert_not_called()


class OwnedFailedStartupTest(unittest.TestCase):
    """A child that hosted no game is stopped even past a refused quit (2026-11-04)."""

    def child(self, alive: bool = True) -> mock.Mock:
        process = mock.Mock(pid=34853)
        process.poll.return_value = None if alive else 0
        return process

    def test_a_graceful_stop_is_all_it_takes_on_a_healthy_host(self) -> None:
        process = self.child()
        with mock.patch.object(launcher, "stop", return_value=True):
            self.assertTrue(launcher.stop_owned_failed_startup(process))
        process.kill.assert_not_called()

    def test_our_child_is_killed_when_it_refuses_the_quit(self) -> None:
        process = self.child()
        with mock.patch.object(launcher, "stop", return_value=False), \
                mock.patch.object(launcher.env, "game_pids", return_value=[]), \
                mock.patch("builtins.print"):
            self.assertTrue(launcher.stop_owned_failed_startup(process))
        process.kill.assert_called_once_with()
        process.wait.assert_called_once()

    def test_a_game_that_is_not_our_child_is_never_killed(self) -> None:
        process = self.child(alive=False)
        with mock.patch.object(launcher, "stop", return_value=False):
            self.assertFalse(launcher.stop_owned_failed_startup(process))
        process.kill.assert_not_called()
        with mock.patch.object(launcher, "stop", return_value=False):
            self.assertFalse(launcher.stop_owned_failed_startup(None))

    def test_a_child_that_survives_the_kill_is_reported(self) -> None:
        process = self.child()
        process.wait.side_effect = subprocess.TimeoutExpired("Civ6_Exe_Child", 10)
        with mock.patch.object(launcher, "stop", return_value=False), \
                mock.patch("builtins.print"):
            self.assertFalse(launcher.stop_owned_failed_startup(process))


class HallOfFameRotationTest(unittest.TestCase):
    """An oversized Hall of Fame is moved aside before a launch, never deleted."""

    def setUp(self) -> None:
        self._tmp = TemporaryDirectory()
        self.dir = Path(self._tmp.name)
        self.hof = self.dir / "HallofFame.sqlite"
        self.hof.write_bytes(b"x" * 2048)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def rotate(self, **kwargs):
        options = dict(limit=1024, running=False, stamp="20261104T234200Z")
        options.update(kwargs)
        return launcher.rotate_hall_of_fame(self.hof, **options)

    def test_a_big_file_with_no_game_running_is_archived_beside_itself(self) -> None:
        archive = self.rotate()
        self.assertEqual(archive, self.dir / "HallofFame-archive-20261104T234200Z.sqlite")
        self.assertFalse(self.hof.exists())
        self.assertEqual(archive.read_bytes(), b"x" * 2048)

    def test_a_file_under_the_cap_is_left_alone(self) -> None:
        self.assertIsNone(self.rotate(limit=4096))
        self.assertTrue(self.hof.exists())

    def test_a_running_game_holds_the_file(self) -> None:
        self.assertIsNone(self.rotate(running=True))
        self.assertTrue(self.hof.exists())

    def test_the_running_check_asks_for_game_pids_by_default(self) -> None:
        with mock.patch.object(launcher.env, "game_pids", return_value=[4242]) as pids:
            self.assertIsNone(launcher.rotate_hall_of_fame(self.hof, limit=1024))
        pids.assert_called_once()
        self.assertTrue(self.hof.exists())

    def test_a_leftover_journal_keeps_its_database_in_place(self) -> None:
        for suffix in ("-journal", "-wal", "-shm"):
            with self.subTest(suffix=suffix):
                journal = self.dir / f"HallofFame.sqlite{suffix}"
                journal.write_bytes(b"j")
                self.assertIsNone(self.rotate())
                self.assertTrue(self.hof.exists())
                journal.unlink()

    def test_an_existing_archive_is_never_overwritten(self) -> None:
        taken = self.dir / "HallofFame-archive-20261104T234200Z.sqlite"
        taken.write_bytes(b"older")
        self.assertIsNone(self.rotate())
        self.assertEqual(taken.read_bytes(), b"older")
        self.assertTrue(self.hof.exists())

    def test_a_missing_file_or_a_zero_cap_does_nothing(self) -> None:
        self.assertIsNone(self.rotate(limit=0))
        self.hof.unlink()
        self.assertIsNone(self.rotate())

    def test_the_default_path_is_the_legacy_user_directory(self) -> None:
        self.assertEqual(launcher.hall_of_fame_path(),
                         launcher.env.LEGACY_USER_DIR / "Sid Meier's Civilization VI"
                         / "HallofFame.sqlite")

    def test_the_play_harness_rotates_before_it_launches(self) -> None:
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text()
        body = source[source.index("    def launch_to_menu() -> bool:"):]
        self.assertLess(body.index("launcher.rotate_hall_of_fame()"),
                        body.index("launcher.launch("))


if __name__ == "__main__":
    unittest.main()
