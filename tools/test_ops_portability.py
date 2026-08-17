#!/usr/bin/env python3
"""Operational scripts must run on the host that installs them.

`tools/ops/` was written on one machine and grew a habit of naming that
machine's home directory outright. That is invisible on the machine it was
written on and total everywhere else, and on 2026-08-17 it cost the project
14.3 hours of Civilization VI attempts: `civvis-game-supervisor.sh` carried
`REPO=/Users/martin/CIVVIS`, so on this host it reached `cd "$REPO"`, logged
"no tree at ...", slept, and did that forever. It could never be installed as a
service, so the ladder loop was hand-started from a terminal session instead —
and when that session ended, so did the loop, with nothing watching.

Two rules, because the debt is real and paying all of it here would be a
different change than the one that fixes the supervisor:

* a script installed as a launchd service must have zero hardcoded homes;
* every other script is held to a recorded count that may fall and never rise.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civvis_collab  # noqa: E402

OPS = Path(__file__).resolve().parent / "ops"

# `/Users/$USER/` and `/Users/${HOME}` are derivations, not hardcoded homes.
HARDCODED_HOME = re.compile(r"/Users/(?!\$)[A-Za-z][A-Za-z0-9._-]*/")

# Installed by `civvis_collab.py bootstrap` as launchd services. A service that
# only works on its author's machine is a service that silently does nothing.
MANAGED = {
    "civvis-game-supervisor.sh",
    "civvis-ladder-terminal-launcher.sh",
    "ladder_watchdog.py",
}

# Everything else, at the count measured on 2026-08-17. These are hand-run
# operator scripts, not installed services, so they are ratcheted rather than
# blocked: lower a number when you fix one, and never raise one.
LEGACY_DEBT = {
    "civvis-keeper.sh": 9,
    "civvis-chain-status.sh": 8,
    "civvis-batch-loop.sh": 7,
    "civvis-interactive-host.sh": 7,
    "civvis-refresh.sh": 5,
    "civvis-popup-keeper.sh": 4,
    "civvis-overnight-audit.sh": 3,
    "civvis-overnight-watchdog.sh": 3,
    "civvis-challenger-guard.sh": 2,
    "civvis-item6-rerun.sh": 2,
    "civvis-goal-report.sh": 1,
    "civvis-goal-watch.sh": 1,
    "civvis-mirror-keeper.sh": 1,
    "civvis-tabs.sh": 1,
    "civvis-tcc-probe.sh": 1,
}


def hardcoded_homes(path: Path) -> list[str]:
    """Hardcoded home paths in executable lines. Comments may quote history."""
    found = []
    for number, line in enumerate(path.read_text(errors="replace").splitlines(), 1):
        if line.lstrip().startswith("#"):
            continue
        for hit in HARDCODED_HOME.findall(line):
            found.append(f"{path.name}:{number}: {hit}")
    return found


class ServicesRunWhereTheyAreInstalled(unittest.TestCase):
    def test_managed_scripts_name_no_home_directory(self):
        for name in sorted(MANAGED):
            path = OPS / name
            self.assertTrue(path.is_file(), f"{name} is installed but missing")
            found = hardcoded_homes(path)
            self.assertEqual(found, [], f"{name} is installed as a launchd "
                                        f"service and must derive its paths:\n  "
                                        + "\n  ".join(found))

    def test_the_supervisor_derives_its_tree(self):
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        self.assertIn("HEAD_REPO=", source)
        self.assertIn("REPO=$HEAD_REPO", source,
                      "the `head` pin must resolve to the derived tree")

    def test_the_supervisor_refuses_a_treeless_repo(self):
        # `cd /` succeeds, so a wrong derivation (the script run from a copy
        # whose three-dirnames-up is not a repo) is otherwise a silent 120s
        # build-fail loop with an empty sha. The cycle must check the tree is
        # buildable and say how to fix the derivation it used.
        source = (OPS / "civvis-game-supervisor.sh").read_text()
        self.assertIn('Cargo.toml" ]]', source,
                      "the cycle must verify $REPO is a buildable tree")
        self.assertIn("set CIVVIS_HEAD_REPO", source,
                      "the refusal must name the override that fixes it")


class LegacyDebtOnlyFalls(unittest.TestCase):
    def test_no_script_gains_a_hardcoded_home(self):
        for name, allowed in sorted(LEGACY_DEBT.items()):
            path = OPS / name
            if not path.is_file():
                continue
            actual = len(hardcoded_homes(path))
            self.assertLessEqual(
                actual, allowed,
                f"{name} went from {allowed} hardcoded home paths to {actual}. "
                f"Derive the path instead — see the supervisor's HEAD_REPO.")

    def test_a_fixed_script_lowers_its_recorded_number(self):
        for name, allowed in sorted(LEGACY_DEBT.items()):
            path = OPS / name
            if not path.is_file():
                self.fail(f"{name} is gone; drop it from LEGACY_DEBT")
            actual = len(hardcoded_homes(path))
            self.assertEqual(
                actual, allowed,
                f"{name} now has {actual} hardcoded home paths, not {allowed}. "
                f"Someone fixed some: set LEGACY_DEBT['{name}'] = {actual} so "
                f"the ratchet holds the new floor.")

    def test_a_new_script_is_classified(self):
        known = MANAGED | set(LEGACY_DEBT)
        for path in sorted(OPS.iterdir()):
            if not path.is_file() or path.name in known:
                continue
            self.assertEqual(
                hardcoded_homes(path), [],
                f"{path.name} is new and names a home directory. Derive it, or "
                f"add it to LEGACY_DEBT with a reason.")


class ManagedServicesCanBeUpdated(unittest.TestCase):
    """`write_managed_service` refuses to replace a plist without its marker.

    It makes that check before the "identical, nothing to do" check, so a
    managed plist written without the marker installs once and can then never
    be changed. The memory guard shipped exactly that way on 2026-08-17: every
    later `bootstrap` raised the moment the content had to move.
    """

    def _plists(self):
        return {
            "memguard": civvis_collab.macos_memguard_plist(Path("/x/memguard.py")),
            "ladder-watchdog": civvis_collab.macos_ladder_watchdog_plist(
                Path("/x/ladder_watchdog.py")),
        }

    def test_every_managed_plist_carries_the_marker(self):
        marker = civvis_collab.FRESHNESS_MARKER.encode("utf-8")
        for name, data in self._plists().items():
            self.assertIn(marker, data,
                          f"the {name} plist cannot be updated after install")

    def test_a_managed_plist_can_be_rewritten(self):
        from tempfile import TemporaryDirectory
        with TemporaryDirectory() as raw:
            for name, data in self._plists().items():
                path = Path(raw) / f"{name}.plist"
                self.assertTrue(civvis_collab.write_managed_service(path, data))
                changed = data.replace(b"<integer>10</integer>",
                                       b"<integer>11</integer>") + b"\n"
                civvis_collab.write_managed_service(path, changed)

    def test_no_launch_agent_runs_the_supervisor_directly(self):
        """A LaunchAgent cannot install the control mod, so it cannot play.

        Installing the mod writes inside `Civ6.app`; macOS attributes that
        permission to the responsible process, and a LaunchAgent's children
        inherit launchd's empty grant set. #1888 shipped a KeepAlive job that
        ran `civvis-game-supervisor.sh` directly and every attempt under it died
        at "cannot install .../DLC/CivvisControl" having played no turns, while
        launchd faithfully restarted a loop that could never work.
        """
        self.assertFalse(hasattr(civvis_collab, "macos_ladder_plist"),
                         "the KeepAlive supervisor job cannot work on macOS")
        for name, data in self._plists().items():
            plist = data.decode()
            self.assertNotIn("civvis-game-supervisor.sh", plist,
                             f"the {name} job runs the loop from launchd, which "
                             f"cannot install the mod; start it through Terminal")

    def test_the_keeper_starts_the_loop_through_terminal(self):
        sys.path.insert(0, str(OPS))
        import ladder_watchdog  # noqa: PLC0415

        source = (OPS / "ladder_watchdog.py").read_text()
        self.assertIn('"open", "-a", "Terminal"', source,
                      "Terminal is the only context here that holds the grant")
        self.assertTrue(hasattr(ladder_watchdog, "start_supervisor"))

    def test_the_keeper_runs_on_an_interval_not_keepalive(self):
        watchdog = civvis_collab.macos_ladder_watchdog_plist(
            Path("/x/w.py")).decode()
        self.assertIn("<key>StartInterval</key>", watchdog)
        self.assertNotIn("<key>KeepAlive</key>", watchdog,
                         "`open` returns immediately; KeepAlive would spin")

    def test_the_broken_keepalive_job_is_retired_on_install(self):
        """Hosts that took #1888 must not keep restarting a loop that cannot play."""
        source = civvis_collab.__file__
        text = Path(source).read_text()
        self.assertIn("def retire_ladder_keepalive_job", text)
        self.assertIn("retire_ladder_keepalive_job()", text.split(
            "def retire_ladder_keepalive_job")[1],
            "install must actually call it")


if __name__ == "__main__":
    unittest.main()


# The launcher is zsh, and zsh is what macOS ships. CI runs Linux images that
# have no /bin/zsh, so these skip there rather than assert about a shell the
# target host is guaranteed to have and the runner is guaranteed not to.
HAS_ZSH = Path("/bin/zsh").exists()


@unittest.skipUnless(HAS_ZSH, "the launcher is zsh; this runner has no /bin/zsh")
class TheLoopsOutputSurvivesItsWindow(unittest.TestCase):
    """A Terminal window is not a log.

    Opened directly, the supervisor's shell output lives only in a GUI window
    that closes when the process ends. On 2026-08-17T21:13Z the loop exited
    cleanly after one failed attempt and left no evidence on disk about why,
    because the only copy of its stderr was painted into a window that was gone
    before anyone looked.
    """

    LAUNCHER = OPS / "civvis-ladder-terminal-launcher.sh"

    def test_the_keeper_opens_the_launcher_not_the_loop(self):
        sys.path.insert(0, str(OPS))
        import ladder_watchdog  # noqa: PLC0415

        self.assertEqual(ladder_watchdog.SUPERVISOR_SCRIPT.name,
                         "civvis-ladder-terminal-launcher.sh")

    def _run(self, body: str, log: Path) -> subprocess.CompletedProcess:
        """Run the real launcher against a stub supervisor."""
        with TemporaryDirectory() as raw:
            stub = Path(raw) / "stub.sh"
            stub.write_text("#!/bin/zsh\n" + body)
            stub.chmod(0o755)
            return subprocess.run(
                ["/bin/zsh", str(self.LAUNCHER)],
                env={**os.environ, "CIVVIS_LADDER_LOG": str(log),
                     "CIVVIS_LADDER_SUPERVISOR": str(stub)},
                capture_output=True, text=True, timeout=60)

    def test_the_loops_stdout_and_stderr_land_in_the_file(self):
        with TemporaryDirectory() as raw:
            log = Path(raw) / "ladder.log"
            self._run("print -r -- 'on stdout'\nprint -r -- 'on stderr' >&2\n", log)
            text = log.read_text()
            self.assertIn("on stdout", text)
            self.assertIn("on stderr", text,
                          "stderr is the half that explains an exit")

    def test_the_exit_status_is_a_line_not_the_absence_of_one(self):
        with TemporaryDirectory() as raw:
            log = Path(raw) / "ladder.log"
            done = self._run("exit 42\n", log)
            self.assertIn("supervisor exited with status 42", log.read_text())
            self.assertEqual(done.returncode, 42,
                             "the launcher must not swallow the status either")

    def test_a_supervisor_killed_mid_run_still_records_its_exit(self):
        """The 2026-08-17T21:20Z failure: `{ } | tee` loses the last line.

        `tee` dies with the rest of the pipeline, and the last thing written is
        exactly the thing worth keeping.
        """
        with TemporaryDirectory() as raw:
            log = Path(raw) / "ladder.log"
            self._run("kill -TERM $$\n", log)
            self.assertIn("supervisor exited with status", log.read_text())

    def test_the_launcher_derives_its_paths(self):
        self.assertEqual(hardcoded_homes(self.LAUNCHER), [])
