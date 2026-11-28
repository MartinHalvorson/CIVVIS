#!/usr/bin/env python3
"""The hotseat companion's one program, and the harness pieces it freezes.

The frozen bundle itself is built and run by `.github/workflows/hotseat-bundle.yml`
on macOS and Windows; these run from the checkout on every pull request, so the
routing, the self-check and the Windows branches of the lock cannot rot between
bundle builds.
"""

from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE / "civ6_control"))

import civ6_hotseat as hotseat  # noqa: E402
import civvis_hotseat_app as app  # noqa: E402
import package_hotseat  # noqa: E402
from civ6_control import gamelock  # noqa: E402
from civ6_control import install as modinstall  # noqa: E402

APP = HERE / "civvis_hotseat_app.py"

#: Stands in for `civvis_orders --serve`: one JSON answer per turn on stdin.
FAKE_DECIDER = """#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    turn = int(line.strip() or 0)
    print(json.dumps({"turn": turn, "note": "fake decider", "orders": [
        {"kind": "research", "subject": None, "verb": "TECH_MINING", "x": None, "y": None},
        {"kind": "unit", "subject": 65536, "verb": "FOUND_CITY", "x": 31, "y": 12}]}),
        flush=True)
"""


def run_app(*args: str, env: dict | None = None, timeout: float = 120) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(APP), *args], capture_output=True, text=True,
                          timeout=timeout, env={**os.environ, **(env or {})},
                          cwd=tempfile.gettempdir())


class RoutingTest(unittest.TestCase):
    def test_the_worker_name_is_the_harness_name(self):
        """The harness asks for it and this program answers to it: one name."""
        self.assertEqual(app.WORKER_COMMAND, hotseat.WORKER_COMMAND)
        self.assertEqual(hotseat.WORKER_COMMAND, "civ6_brain.py",
                         "the stale-worker check and gamelock match on the script name")

    def test_the_worker_route_is_the_brain(self):
        done = run_app("civ6_brain.py", "--help")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("--run-dir", done.stdout)
        self.assertIn("--github-refresh-seconds", done.stdout)

    def test_help_lists_the_programs_commands_and_the_harness_options(self):
        done = run_app("--help")
        self.assertEqual(done.returncode, 0, done.stderr)
        for word in ("doctor", "selfcheck", "--players", "--stop", "--status"):
            self.assertIn(word, done.stdout)

    def test_a_frozen_harness_asks_for_the_worker_by_name(self):
        seat = Path("/r/hs-p1")
        with mock.patch.object(hotseat, "FROZEN", True):
            frozen = hotseat.brain_command(seat, Path("/r/o.sqlite"), Path("/r/b"), "civvis")
        source = hotseat.brain_command(seat, Path("/r/o.sqlite"), Path("/r/b"), "civvis")
        self.assertEqual(frozen[:2], [sys.executable, "civ6_brain.py"])
        self.assertEqual(source[:2], [sys.executable, str(HERE / "civ6_brain.py")],
                         "a run from a checkout is unchanged")
        self.assertEqual(frozen[2:], source[2:])

    def test_from_a_checkout_nothing_about_the_decider_changes(self):
        self.assertFalse(hotseat.FROZEN)
        self.assertIsNone(hotseat.BUNDLED_BIN)
        if os.name != "nt":
            self.assertEqual(hotseat.ORDERS_NAME, "civvis_orders")
            self.assertEqual(hotseat.worker_session(), {"start_new_session": True})

    def test_a_frozen_harness_tries_its_own_decider_first(self):
        with tempfile.TemporaryDirectory() as temporary:
            bundled = Path(temporary) / hotseat.ORDERS_NAME
            bundled.write_text("#!/bin/sh\n")
            bundled.chmod(0o755)
            with mock.patch.object(hotseat, "BUNDLED_BIN", bundled):
                self.assertEqual(hotseat.resolve_orders_bin(None), bundled)


class ManualLobbyFrozenTest(unittest.TestCase):
    """`civvis-hotseat --manual-lobby`, the mode the bundle is for."""

    def test_from_the_command_line_the_harness_reads_argv_itself(self):
        """So its module-level switch sees `--manual-lobby` before anything of
        `civ6_control` loads, and an abbreviated flag can re-execute."""
        with mock.patch.object(hotseat, "main", return_value=0) as harness, \
                mock.patch.object(sys, "argv", ["civvis-hotseat", "--status"]):
            app.main()
            app.main(["--status"])
        self.assertEqual([call.args for call in harness.call_args_list],
                         [(None,), (["--status"],)])

    def _reexec(self, frozen: bool) -> list[str]:
        with mock.patch.object(hotseat, "FROZEN", frozen), \
                mock.patch.object(hotseat.screen_free, "engaged", return_value=False), \
                mock.patch.object(hotseat, "read_current", return_value=None), \
                mock.patch.object(sys, "argv", ["civvis-hotseat", "--manual"]), \
                mock.patch.object(hotseat.os, "execve",
                                  side_effect=SystemExit("re-executed")) as execve:
            with self.assertRaises(SystemExit):
                hotseat.main()
        program, argv, environ = execve.call_args.args
        self.assertEqual(program, sys.executable)
        self.assertEqual(environ["CIVVIS_SCREEN_FREE"], "1")
        return argv

    def test_an_abbreviated_flag_restarts_the_frozen_program_screen_free(self):
        self.assertEqual(self._reexec(frozen=True), [sys.executable, "--manual"])

    def test_from_a_checkout_it_restarts_the_script_as_before(self):
        self.assertEqual(self._reexec(frozen=False),
                         [sys.executable, str(HERE / "civ6_hotseat.py"), "--manual"])


class HarnessRecognitionTest(unittest.TestCase):
    """`--status` and `--stop` recognise a harness pid by its command line."""

    def test_the_script_and_the_bundles_program_are_harnesses(self):
        for command in ("/usr/bin/python3 tools/civ6_hotseat.py --tag t",
                        "/Users/p/Downloads/civvis-hotseat/civvis-hotseat --manual-lobby",
                        "./civvis-hotseat",
                        "/usr/bin/python3 /r/tools/civvis_hotseat_app.py --manual-lobby",
                        '"C:\\Users\\p\\civvis-hotseat\\civvis-hotseat.exe" --tag t'):
            with self.subTest(command=command):
                self.assertTrue(hotseat.is_harness_command(command))

    def test_a_worker_or_a_folder_of_the_same_name_is_not(self):
        for command in ("/b/civvis-hotseat/civvis-hotseat civ6_brain.py --run-dir /r/t-p1",
                        "python3 /r/tools/civvis_hotseat_app.py civ6_brain.py --run-dir /r/t-p1",
                        "/bin/bash /Users/p/civvis-hotseat/civ6-hotseat.sh supervise",
                        "/Users/p/civvis-hotseat-main/target/release/civvis_orders --serve",
                        ""):
            with self.subTest(command=command):
                self.assertFalse(hotseat.is_harness_command(command))


class WindowsBranchesOfTheLockTest(unittest.TestCase):
    """⚠ `os.kill(pid, 0)` is a console Ctrl-C on Windows: a stale holder read
    as alive forever, and the next game was refused until somebody deleted the
    lock by hand. Windows asks `civ6_env` instead; POSIX is untouched."""

    def test_windows_asks_civ6_env_whether_the_holder_lives(self):
        with mock.patch.object(gamelock.sys, "platform", "win32"), \
                mock.patch("civ6_env.pid_alive", return_value=False) as alive, \
                mock.patch.object(gamelock.os, "kill",
                                  side_effect=AssertionError("os.kill on Windows")):
            self.assertFalse(gamelock._alive(4242))
        alive.assert_called_once_with(4242)

    def test_windows_reads_command_lines_without_ps(self):
        rows = [(7, '"C:\\b\\civvis-hotseat.exe" civ6_brain.py --run-dir C:\\r\\t-p1')]
        with mock.patch.object(gamelock.sys, "platform", "win32"), \
                mock.patch("civ6_env.windows_command_lines", return_value=rows), \
                mock.patch("subprocess.run", side_effect=AssertionError("ran ps")):
            self.assertEqual(gamelock._processes(), rows)

    def test_posix_keeps_its_own_probe(self):
        if os.name == "nt":
            self.skipTest("POSIX only")
        with mock.patch("civ6_env.pid_alive", side_effect=AssertionError("rerouted")):
            self.assertTrue(gamelock._alive(os.getpid()))


class BundleInputsTest(unittest.TestCase):
    def test_the_fixture_is_a_real_opening_board_with_nothing_personal(self):
        text = app.FIXTURE.read_text(encoding="utf-8")
        kinds = [json.loads(line)["kind"] for line in text.splitlines()]
        for kind in ("seat", "tiles", "tiles_done", "state"):
            self.assertIn(kind, kinds)
        self.assertEqual(kinds[-1], "state", "the board the worker answers is the last record")
        for marker in ("/Users/", "/home/", "C:\\\\Users", "martin"):
            self.assertNotIn(marker, text)

    def test_the_bundle_ships_the_mod_without_its_test_files(self):
        shipped = [path.name for path in package_hotseat.mod_files()]
        self.assertTrue(set(modinstall.SCRIPTS) <= set(shipped))
        self.assertIn("CivvisControl.modinfo", shipped)
        self.assertFalse([name for name in shipped if name.endswith("_test.lua")])

    def test_the_platform_tag_names_system_and_architecture(self):
        system, machine = package_hotseat.platform_tag().split("-", 1)
        self.assertIn(system, ("macos", "windows", "linux"))
        self.assertTrue(machine)


class DoctorTest(unittest.TestCase):
    def test_the_doctor_reports_a_fabricated_machine(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            (base / "install").mkdir()
            done = run_app("doctor", "--json", env={
                "CIV6_INSTALL": str(base / "install"), "CIV6_DIR": "",
                "CIV6_USER_DIR": str(base / "user"),
                "CIVVIS_HOTSEAT_RUNS": str(base / "runs")})
        self.assertEqual(done.returncode, 0, done.stderr)
        report = json.loads(done.stdout)
        self.assertEqual(report["install_dir"], str(base / "install"))
        self.assertEqual(report["mod"]["install_dir"],
                         str(base / "install" / "DLC" / "CivvisControl"))
        self.assertEqual(report["log_file"]["path"],
                         str(base / "user" / "Logs" / "Automation.log"))
        self.assertTrue(report["bundle"]["mod_scripts_present"])
        self.assertFalse(report["harness"]["running"])


@unittest.skipIf(os.name == "nt", "the fake decider is a POSIX script")
class SelfcheckTest(unittest.TestCase):
    """The whole self-check from a checkout, deciding with a stand-in binary."""

    def test_every_check_passes_and_cleans_up_after_itself(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            decider = base / "civvis_orders"
            decider.write_text(FAKE_DECIDER)
            decider.chmod(decider.stat().st_mode | stat.S_IXUSR)
            scratch = base / "scratch"
            done = run_app("selfcheck", "--orders-bin", str(decider), "--dir", str(scratch),
                           "--timeout", "60")
            output = done.stdout + done.stderr
            self.assertEqual(done.returncode, 0, output)
            self.assertIn("9 of 9 checks passed", output)
            self.assertIn("--status saw it running; --stop ended it", output)
            self.assertIn("RequireHarness in every prelude", output)
            self.assertIn("screen-free, 6 screen modules stood in, 0 loaded for real", output)
            self.assertIn("turn 1: 2 orders", output)
            self.assertFalse((scratch / "install" / "DLC" / modinstall.MOD_NAME).exists(),
                             "the mod is uninstalled again")
            self.assertFalse((scratch / "game.lock").exists(), "the lock is released")
            self.assertTrue((scratch / "runs" / "selfcheck" / "orders.sqlite").is_file())


if __name__ == "__main__":
    unittest.main()
