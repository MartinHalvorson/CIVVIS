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


class AGameThatWillNotExitReportsTheSafeStopFailure(unittest.TestCase):
    """The supervisor must get a diagnostic instead of a second exception."""

    def test_quit_game_writes_the_remaining_pids_to_stderr(self):
        with mock.patch.object(civ6_env, "game_pids",
                               side_effect=[[4242], [4242], [4242]]), \
             mock.patch.object(civ6_env, "request_macos_quit",
                               return_value=False), \
             mock.patch.object(civ6_env.os, "kill") as kill, \
             mock.patch("time.time", side_effect=[0.0, 21.0]), \
             mock.patch("time.sleep"), \
             mock.patch("builtins.print") as printed:
            self.assertFalse(civ6_env.quit_game(timeout_s=20.0))

        kill.assert_called_once_with(4242, civ6_env.signal.SIGTERM)
        printed.assert_called_once_with(
            "[env] Civilization VI did not exit after SIGTERM: [4242]",
            file=sys.stderr,
            flush=True,
        )


class MacosQuitMenuTest(unittest.TestCase):
    """The native menu is a graceful fallback, not a disguised hard kill."""

    def test_request_macos_quit_uses_the_application_quit_menu(self):
        completed = mock.Mock(returncode=0, stdout="true\n")
        with mock.patch.object(civ6_env.sys, "platform", "darwin"), \
             mock.patch.object(civ6_env.subprocess, "run",
                               return_value=completed) as ran:
            self.assertTrue(civ6_env.request_macos_quit())

        command = ran.call_args.args[0]
        self.assertEqual(command[:2], ["osascript", "-e"])
        self.assertIn("Quit Civilization VI", command[2])
        self.assertEqual(ran.call_args.kwargs["timeout"], 5)

    def test_request_macos_quit_refuses_non_macos_hosts(self):
        with mock.patch.object(civ6_env.sys, "platform", "linux"), \
             mock.patch.object(civ6_env.subprocess, "run") as ran:
            self.assertFalse(civ6_env.request_macos_quit())
        ran.assert_not_called()


if __name__ == "__main__":
    unittest.main()


class ExitConfirmationWiringTests(unittest.TestCase):
    """The graceful quit has to answer the question it just asked."""

    def test_quit_game_answers_the_confirmation_while_it_waits(self):
        """★ Without this the menu quit opens a modal, the SIGTERM that follows
        cannot get through it, and `quit_game` — rightly refusing to SIGKILL a
        process that owns the save files — leaves the lane stalled for an
        operator. Measured twice on 2026-09-10."""
        now, clock, sleep = self._clock()
        answered = []

        def confirm():
            answered.append(now[0])
            return True

        def pids():
            return [] if answered else [4242]   # the modal holds it until answered

        with mock.patch.object(civ6_env, "game_pids", side_effect=pids), \
             mock.patch.object(civ6_env, "request_macos_quit", return_value=True), \
             mock.patch.object(civ6_env, "confirm_exit_dialog", side_effect=confirm), \
             mock.patch.object(civ6_env.os, "kill") as kill, \
             mock.patch("time.time", side_effect=clock), \
             mock.patch("time.sleep", side_effect=sleep):
            self.assertTrue(civ6_env.quit_game(timeout_s=20.0))
        self.assertEqual(len(answered), 1)
        kill.assert_not_called()

    def _clock(self):
        """A fake time.time that advances only when the code under test sleeps."""
        now = [1000.0]

        def sleep(seconds):
            now[0] += seconds

        return now, (lambda: now[0]), sleep

    def test_a_game_that_leaves_by_itself_is_never_photographed(self):
        """The end-of-game quit closes the game unasked; the confirmation look
        is a screenshot that can stall ~15 s, so it waits EXIT_CONFIRM_AFTER_S."""
        now, clock, sleep = self._clock()

        def pids():
            # gone 9 s after the quit: Civ's own exit takes 5-8 s, longer after a
            # long game, and must not be met by a screenshot that can stall 15 s
            return [4242] if now[0] < 1009.0 else []

        with mock.patch.object(civ6_env, "game_pids", side_effect=pids), \
             mock.patch.object(civ6_env, "request_macos_quit", return_value=True), \
             mock.patch.object(civ6_env, "confirm_exit_dialog") as confirm, \
             mock.patch.object(civ6_env.os, "kill") as kill, \
             mock.patch("time.time", side_effect=clock), \
             mock.patch("time.sleep", side_effect=sleep):
            self.assertTrue(civ6_env.quit_game(timeout_s=20.0))
        confirm.assert_not_called()
        kill.assert_not_called()

    def test_a_game_still_up_after_the_settle_is_looked_at_once_a_second(self):
        """A modal that is not found yet is looked for again a second later,
        never on every 0.2 s poll, and is answered inside the menu window."""
        now, clock, sleep = self._clock()
        answered = []

        def confirm():
            answered.append(now[0])
            return len(answered) == 3   # found on the third look

        def pids():
            return [4242] if len(answered) < 3 else []

        with mock.patch.object(civ6_env, "game_pids", side_effect=pids), \
             mock.patch.object(civ6_env, "request_macos_quit", return_value=True), \
             mock.patch.object(civ6_env, "confirm_exit_dialog", side_effect=confirm), \
             mock.patch.object(civ6_env.os, "kill") as kill, \
             mock.patch("time.time", side_effect=clock), \
             mock.patch("time.sleep", side_effect=sleep):
            self.assertTrue(civ6_env.quit_game(timeout_s=20.0))
        self.assertEqual(len(answered), 3)
        self.assertGreaterEqual(answered[0] - 1000.0, civ6_env.EXIT_CONFIRM_AFTER_S)
        self.assertGreaterEqual(answered[1] - answered[0], 1.0)
        kill.assert_not_called()

    def test_the_confirmation_is_never_asked_when_the_menu_quit_was_refused(self):
        """No menu quit, no modal to answer — go straight to the TERM path."""
        with mock.patch.object(civ6_env, "game_pids",
                               side_effect=[[4242], [4242], []]), \
             mock.patch.object(civ6_env, "request_macos_quit", return_value=False), \
             mock.patch.object(civ6_env, "confirm_exit_dialog") as confirm, \
             mock.patch.object(civ6_env.os, "kill"), \
             mock.patch("time.sleep"):
            civ6_env.quit_game(timeout_s=20.0)
        confirm.assert_not_called()

    def test_a_reader_that_raises_never_escapes_the_quit_path(self):
        """⚠ A quit must not be turned into a traceback by a screen reader."""
        import civ6_play

        with mock.patch.object(civ6_play, "confirm_exit_dialog",
                               side_effect=RuntimeError("no capture")):
            self.assertFalse(civ6_env.confirm_exit_dialog())


class MacosResolutionIsUnchanged(unittest.TestCase):
    """⚠ The ladder hosts play ~90 unattended games a day through these paths.
    The Windows half is reached only behind `on_windows()`; off Windows every
    answer is the one this module gave before it existed."""

    def test_the_macos_layout_constants(self):
        support = Path.home() / "Library/Application Support"
        self.assertEqual(civ6_env.SUPPORT, support)
        self.assertEqual(civ6_env.LEGACY_USER_DIR, support / "Sid Meier's Civilization VI")
        self.assertEqual(civ6_env.NESTED_USER_DIR, support / "Sid Meier's Civilization VI"
                         / "Firaxis Games/Sid Meier's Civilization VI")
        self.assertEqual(civ6_env.INSTALL_CANDIDATES[:2], (
            support / "Steam/steamapps/common/Sid Meier's Civilization VI",
            Path("/Applications/Sid Meier's Civilization VI")))

    def test_off_windows_the_windows_resolvers_are_never_asked(self):
        refuse = mock.Mock(side_effect=AssertionError("asked a Windows resolver"))
        with tempfile.TemporaryDirectory() as temporary, \
                mock.patch.object(civ6_env, "on_windows", return_value=False), \
                mock.patch.object(civ6_env, "windows_install_candidates", refuse), \
                mock.patch.object(civ6_env, "windows_user_dir", refuse), \
                mock.patch.object(civ6_env, "windows_processes", refuse), \
                mock.patch.dict(os.environ, {"CIV6_INSTALL": temporary, "CIV6_DIR": "",
                                             "CIV6_USER_DIR": ""}), \
                mock.patch.object(civ6_env.subprocess, "run",
                                  return_value=mock.Mock(stdout="")):
            self.assertEqual(civ6_env.install_dir(), Path(temporary))
            self.assertIn(civ6_env.user_dir(), (civ6_env.NESTED_USER_DIR,
                                                civ6_env.LEGACY_USER_DIR))
            self.assertEqual(civ6_env.game_pids(), [])

    def test_the_user_dir_is_still_the_freshest_of_the_two(self):
        with tempfile.TemporaryDirectory() as temporary:
            legacy = Path(temporary) / "legacy"
            nested = legacy / "Firaxis Games" / "nested"
            nested.mkdir(parents=True)
            with mock.patch.object(civ6_env, "on_windows", return_value=False), \
                    mock.patch.object(civ6_env, "LEGACY_USER_DIR", legacy), \
                    mock.patch.object(civ6_env, "NESTED_USER_DIR", nested), \
                    mock.patch.dict(os.environ, {"CIV6_USER_DIR": ""}):
                self.assertEqual(civ6_env.user_dir(), nested, "untouched: nested wins")
                (legacy / "AppOptions.txt").write_text("x")
                (nested / "AppOptions.txt").write_text("x")
                os.utime(legacy / "AppOptions.txt", (1, 1))
                self.assertEqual(civ6_env.user_dir(), nested)
                os.utime(nested / "AppOptions.txt", (0, 0))
                self.assertEqual(civ6_env.user_dir(), legacy)

    def test_the_posix_command_line_is_read_with_the_same_ps(self):
        with mock.patch.object(civ6_env, "on_windows", return_value=False), \
                mock.patch.object(civ6_env.subprocess, "run",
                                  return_value=mock.Mock(stdout="python civ6_brain.py\n")) as ran:
            self.assertEqual(civ6_env.process_command(77), "python civ6_brain.py\n")
        self.assertEqual(ran.call_args.args[0], ["ps", "-o", "command=", "-p", "77"])
        self.assertEqual(ran.call_args.kwargs.get("timeout"), 5)

    def test_a_pid_is_alive_only_when_it_is_a_real_process(self):
        self.assertTrue(civ6_env.pid_alive(os.getpid()))
        for nonsense in (0, -1, "x", None):
            with self.subTest(pid=nonsense):
                self.assertFalse(civ6_env.pid_alive(nonsense))


def _vdf_path(path: Path) -> str:
    """A path as Steam writes it into a .vdf: backslashes doubled."""
    return str(path).replace("\\", "\\\\")


class WindowsSteamResolution(unittest.TestCase):
    """The Windows install, resolved over fake Steam trees (on any host)."""

    def test_libraryfolders_in_the_current_nested_layout(self):
        text = r'''
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"apps"		{ "228980"	"1" }
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"   // a second drive
		"apps"		{ "289070"	"30000000000" }
	}
}'''
        data = civ6_env.parse_vdf(text)
        self.assertEqual(data["libraryfolders"]["1"]["path"], r"D:\SteamLibrary")
        self.assertEqual(data["libraryfolders"]["1"]["apps"], {"289070": "30000000000"})

    def test_libraryfolders_in_the_older_flat_layout(self):
        data = civ6_env.parse_vdf('"LibraryFolders"\n{\n\t"TimeNextStatsReport"\t"1"\n'
                                  '\t"1"\t\t"E:\\\\Games\\\\Steam"\n}\n')
        self.assertEqual(data["LibraryFolders"]["1"], r"E:\Games\Steam")

    def _library(self, root: Path, manifest: str | None = None) -> Path:
        apps = root / "steamapps"
        apps.mkdir(parents=True, exist_ok=True)
        if manifest is not None:
            (apps / f"appmanifest_{civ6_env.STEAM_APP_ID}.acf").write_text(
                '"AppState"\n{\n\t"appid"\t"289070"\n\t"installdir"\t"%s"\n}\n' % manifest)
        return root

    def test_the_game_is_found_in_a_second_library(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            steam = self._library(base / "Steam")
            second = self._library(base / "SteamLibrary", manifest="Civ VI Here")
            game = second / "steamapps" / "common" / "Civ VI Here"
            (game / "DLC").mkdir(parents=True)
            (steam / "steamapps" / "libraryfolders.vdf").write_text(
                '"libraryfolders"\n{\n\t"0"\n\t{\n\t\t"path"\t"%s"\n\t}\n'
                '\t"1"\n\t{\n\t\t"path"\t"%s"\n\t}\n}\n'
                % (_vdf_path(steam), _vdf_path(second)))
            self.assertEqual(civ6_env.steam_libraries(steam), [steam, second])
            candidates = civ6_env.windows_install_candidates([steam])
            self.assertEqual(candidates[0], game, "the library with the manifest comes first")
            with mock.patch.object(civ6_env, "on_windows", return_value=True), \
                    mock.patch.object(civ6_env, "windows_steam_roots", return_value=[steam]), \
                    mock.patch.dict(os.environ, {"CIV6_INSTALL": "", "CIV6_DIR": ""}):
                self.assertEqual(civ6_env.install_dir(), game)
                # No Civ6.app on Windows: the install root is the assets root,
                # so install.py's DLC folder is <install>\DLC.
                self.assertEqual(civ6_env.assets_dir(), game)

    def test_a_library_without_a_manifest_uses_the_depot_folder_name(self):
        with tempfile.TemporaryDirectory() as temporary:
            library = self._library(Path(temporary))
            self.assertEqual(civ6_env.steam_app_dir(library),
                             library / "steamapps" / "common" / civ6_env.STEAM_INSTALL_NAME)

    def test_the_program_files_defaults_follow_the_registry(self):
        roots = civ6_env.windows_steam_roots(
            {"ProgramFiles(x86)": "X86", "ProgramFiles": "X64"}, read_registry=False)
        self.assertEqual(roots, [Path("X86") / "Steam", Path("X64") / "Steam"])


class WindowsUserDirectory(unittest.TestCase):
    def test_documents_my_games_is_the_user_dir(self):
        with tempfile.TemporaryDirectory() as temporary:
            documents = Path(temporary) / "Documents"
            user = documents / "My Games" / "Sid Meier's Civilization VI"
            self.assertEqual(civ6_env.windows_user_dir_candidates(documents, environ={}), [user])
            with mock.patch.object(civ6_env, "on_windows", return_value=True), \
                    mock.patch.object(civ6_env, "windows_documents_dir", return_value=documents), \
                    mock.patch.dict(os.environ, {"CIV6_USER_DIR": "", "USERPROFILE": "",
                                                 "OneDrive": ""}):
                self.assertEqual(civ6_env.user_dir(), user)
                self.assertEqual(civ6_env.logs_dir(), user / "Logs")
                self.assertEqual(civ6_env.mods_dir(), user / "Mods")
                self.assertEqual(civ6_env.app_options_path(), user / "AppOptions.txt")

    def test_a_redirected_documents_folder_is_chosen_by_its_fresh_options(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            shell = base / "OneDrive" / "Documents"
            profile = base / "Profile"
            candidates = civ6_env.windows_user_dir_candidates(
                shell, environ={"USERPROFILE": str(profile)})
            self.assertEqual(candidates, [shell / civ6_env.WINDOWS_USER_SUBPATH,
                                          profile / "Documents" / civ6_env.WINDOWS_USER_SUBPATH])
            self.assertEqual(civ6_env.windows_user_dir(candidates), candidates[0],
                             "nothing written yet: the shell's Documents")
            for candidate, stamp in zip(candidates, (1, 2)):
                candidate.mkdir(parents=True)
                (candidate / "AppOptions.txt").write_text("x")
                os.utime(candidate / "AppOptions.txt", (stamp, stamp))
            self.assertEqual(civ6_env.windows_user_dir(candidates), candidates[1])

    def test_mod_locations_say_they_are_unmeasured(self):
        with tempfile.TemporaryDirectory() as temporary, \
                mock.patch.object(civ6_env, "on_windows", return_value=True), \
                mock.patch.dict(os.environ, {"CIV6_INSTALL": temporary,
                                             "CIV6_USER_DIR": temporary}):
            rows = civ6_env.mod_install_candidates()
        self.assertEqual([(name, path) for name, path, _ in rows],
                         [("install DLC", Path(temporary) / "DLC"),
                          ("user Mods", Path(temporary) / "Mods")])
        self.assertTrue(all("UNMEASURED" in evidence for _, _, evidence in rows))


class WindowsProcesses(unittest.TestCase):
    def test_the_game_is_found_by_its_executable_names(self):
        table = [(4, "System"), (88, "civilizationvi_dx12.exe"), (90, "CivilizationVI.exe"),
                 (91, "LaunchPad.exe"), (92, "python.exe")]
        with mock.patch.object(civ6_env, "on_windows", return_value=True), \
                mock.patch.object(civ6_env, "windows_processes", return_value=table):
            self.assertEqual(civ6_env.game_pids(), [88, 90])
        with mock.patch.object(civ6_env, "on_windows", return_value=True), \
                mock.patch.object(civ6_env, "windows_processes", side_effect=OSError("no")):
            self.assertEqual(civ6_env.game_pids(), [])

    def test_command_lines_are_parsed_from_powershell(self):
        # A byte-order mark, should PowerShell write one, must not hide the first row.
        out = "\ufeff12 \"C:\\b\\civvis-hotseat.exe\" civ6_brain.py --run-dir C:\\r\n13 \n\nnoise\n"
        with mock.patch.object(civ6_env.subprocess, "run",
                               return_value=mock.Mock(stdout=out)) as ran:
            rows = civ6_env.windows_command_lines(12)
        self.assertEqual(rows, [(12, '"C:\\b\\civvis-hotseat.exe" civ6_brain.py --run-dir C:\\r'),
                                (13, "")])
        self.assertIn("ProcessId=12", ran.call_args.args[0][-1])
        with mock.patch.object(civ6_env.subprocess, "run", side_effect=OSError("none")):
            self.assertEqual(civ6_env.windows_command_lines(), [])

    def test_windows_never_hard_kills_the_game(self):
        """`os.kill(pid, SIGTERM)` there is TerminateProcess."""
        with mock.patch.object(civ6_env, "on_windows", return_value=True), \
                mock.patch.object(civ6_env, "game_pids", return_value=[4242]), \
                mock.patch.object(civ6_env.os, "kill",
                                  side_effect=AssertionError("killed the game")), \
                mock.patch("sys.stderr"):
            self.assertFalse(civ6_env.quit_game(timeout_s=1.0))
