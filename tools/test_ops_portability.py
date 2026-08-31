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
from unittest import mock
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
    "civvis-spectator-runner.sh",
    "ladder_watchdog.py",
}

# ⚠ THIS WAS FIFTEEN SCRIPTS AND 55 HARDCODED PATHS, AND IT IS EMPTY NOW.
# `tools/ops/` was written on one machine and named that machine's home
# directory outright — every one of them `/Users/martin`, a single mechanical
# substitution that nobody had made. The ratchet below held the debt from
# growing while the supervisor was fixed; the debt is paid, and an empty table
# is what "the class is gone rather than the instance" looks like.
#
# It stays as a table rather than being deleted, because
# `test_a_new_script_is_classified` needs somewhere for a deliberate exception
# to go, and because an empty one records that the answer is zero rather than
# that nobody checked.
LEGACY_DEBT: dict[str, int] = {}

# `$HOME/civvis-x.sh` where `tools/ops/civvis-x.sh` exists is a script shadowing
# its own tracked self. The home copy is the one an operator hand-edits and the
# one no CI run has ever seen, so every fix landed in `tools/ops/` reaches a
# file nothing invokes — "a home copy is a dead ladder".
#
# ⚠⚠ IT WAS NOT THEORETICAL. On 2026-08-18 the sweep above emptied LEGACY_DEBT
# by mechanically replacing `/Users/martin` with `$HOME` across `tools/ops/`.
# Not one home copy was re-synced. Five days later `civvis-sync.sh` was still
# logging SCRIPT DRIFT on eleven of them every fifteen minutes, and for those
# five days `civvis-keeper.sh` — itself one of the eleven — was calling
# `$HOME/civvis-tabs.sh`, `$HOME/civvis-refresh.sh` and
# `$HOME/civvis-challenger-guard.sh`: the pre-sweep, unportable copies.
#
# A ratchet, for the same reason as LEGACY_DEBT: paying the rest of the debt is
# a different change from stopping it growing. The one entry left is deliberate
# and is a behaviour change, not hygiene — `tools/ops/civvis-game-supervisor.sh`
# is 249 lines different from `$HOME/civvis-game-supervisor.sh`, so pointing the
# safe-reload at the tracked copy swaps the running loop for another one and
# must be measured, not tidied.
HOME_SHADOW_DEBT: dict[str, int] = {
    "civvis-supervisor-safe-reload.sh": 1,
}

HOME_SHADOW = re.compile(r"\$HOME/(civvis-[A-Za-z0-9._-]+\.sh)")

# ⚠ The rule above sees `$HOME/civvis-<name>.sh` — a hand-edited copy of a
# SIBLING. It cannot see a reference into a whole CHECKOUT, which is the same
# defect one level up, and `civvis-popup-keeper.sh` carried one for months:
#
#     CLEARER=$HOME/CIVVIS/tools/civ6_control/popup_clear.py
#
# That is a path that exists on exactly one machine in the fleet, so the keeper
# was uninstallable anywhere else. On the host that does have it, it is worse
# than unportable: `~/CIVVIS` is the supervisor's HEAD tree and is
# `git checkout --detach origin/main`ed every cycle, so the clearer clicking on
# the screen need not be the revision the game is being played from.
HOME_CHECKOUT = re.compile(
    r"\$HOME/[A-Za-z0-9._-]+/((?:tools|src|web|data|mods)/[A-Za-z0-9._/-]+)")


def home_checkout_references(path: Path) -> list[str]:
    """`$HOME/<checkout>/<relpath>` references this repository also tracks.

    Executable lines only, like `home_shadowed_siblings`: the comments here and
    in the scripts quote the very paths they warn about.
    """
    repo = OPS.parent.parent
    found = []
    for number, line in enumerate(path.read_text(errors="replace").splitlines(), 1):
        if line.lstrip().startswith("#"):
            continue
        for relative in HOME_CHECKOUT.findall(line):
            if (repo / relative).is_file():
                found.append(f"{path.name}:{number}: $HOME/<checkout>/{relative}")
    return found



def home_shadowed_siblings(path: Path) -> list[str]:
    """`$HOME/<name>` references whose `tools/ops/<name>` exists.

    Only executable lines: a comment may quote the history it is warning about,
    and this file's own headers do.
    """
    found = []
    for number, line in enumerate(path.read_text(errors="replace").splitlines(), 1):
        if line.lstrip().startswith("#"):
            continue
        for name in HOME_SHADOW.findall(line):
            if (OPS / name).is_file():
                found.append(f"{path.name}:{number}: $HOME/{name}")
    return found


class TheTrackedCopyIsTheOneThatRuns(unittest.TestCase):
    """Discovered by glob, never by a list — the ops directory grows."""

    def _scripts(self) -> list[Path]:
        scripts = sorted(OPS.glob("*.sh"))
        self.assertTrue(scripts, "tools/ops/*.sh matched nothing; the glob is wrong")
        return scripts

    def test_no_script_invokes_the_home_copy_of_a_tracked_sibling(self):
        for path in self._scripts():
            allowed = HOME_SHADOW_DEBT.get(path.name, 0)
            found = home_shadowed_siblings(path)
            self.assertLessEqual(
                len(found), allowed,
                f"{path.name} runs the hand-edited home copy of a script this "
                f"repository tracks, so fixes to the tracked copy never run. "
                f"Call the sibling instead — `OPS=${{0:A:h}}` and "
                f"`$OPS/<name>`, as civvis-keeper.sh does:\n  "
                + "\n  ".join(found))

    def test_no_script_reaches_into_a_named_checkout_for_a_tracked_file(self):
        """One level up from the rule above, and it hid there for months.

        A `$HOME/<checkout>/tools/...` path is unportable everywhere and, on the
        one host that has the checkout, is very likely the supervisor's detached
        HEAD tree — so the helper and the game it helps can be different
        revisions. Derive from the script's own tree: `${0:A:h:h}/<relpath>`.
        """
        offenders = []
        for path in self._scripts():
            offenders.extend(home_checkout_references(path))
        self.assertEqual(offenders, [], "\n".join(
            ["an ops script reaches into a named checkout for a file this "
             "repository tracks:"] + offenders))

    def test_a_fixed_script_lowers_its_recorded_number(self):
        for name, allowed in sorted(HOME_SHADOW_DEBT.items()):
            path = OPS / name
            self.assertTrue(path.is_file(), f"{name} is gone; drop it from "
                                            f"HOME_SHADOW_DEBT")
            actual = len(home_shadowed_siblings(path))
            self.assertEqual(
                actual, allowed,
                f"{name} now shadows {actual} tracked sibling(s), not "
                f"{allowed}. Someone fixed some: set "
                f"HOME_SHADOW_DEBT['{name}'] = {actual} so the ratchet holds.")

    def test_the_keeper_calls_its_siblings(self):
        """The three call sites the 2026-08-18 sweep silently orphaned."""
        source = (OPS / "civvis-keeper.sh").read_text()
        self.assertIn("OPS=${0:A:h}", source,
                      "the keeper must derive the tracked ops directory")
        for name in ("civvis-tabs.sh", "civvis-refresh.sh",
                     "civvis-challenger-guard.sh"):
            self.assertIn(f"ops {name}", source,
                          f"the keeper must run the tracked {name}")


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
        repo = Path(__file__).resolve().parent.parent
        return {
            "memguard": civvis_collab.macos_memguard_plist(Path("/x/memguard.py")),
            "ladder-watchdog": civvis_collab.macos_ladder_watchdog_plist(
                Path("/x/ladder_watchdog.py")),
            "spectator": civvis_collab.macos_spectator_plist(Path("/x/run.sh"), repo),
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

    def test_popup_keeper_chases_a_covered_dialogue_quarter_second(self):
        source = (OPS / "civvis-popup-keeper.sh").read_text()
        self.assertIn("--interval 0.25", source)


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

    def test_keeping_the_window_skips_the_window_step_entirely(self):
        """The opt-out is real, so an operator can watch the loop live."""
        with TemporaryDirectory() as raw:
            log = Path(raw) / "ladder.log"
            with mock.patch.dict(os.environ, {"CIVVIS_LADDER_KEEP_WINDOW": "1"}):
                self._run("print -r -- hello\n", log)
            self.assertNotIn("window:", log.read_text())

    def test_the_window_step_is_valid_applescript(self):
        """A syntax error here would be invisible.

        The step runs `osascript` for its effect and the launcher must not die
        when a host has no window server, so a broken script would simply do
        nothing and the window would keep covering the game. Compiling it is the
        check that a green run cannot hide.
        """
        sources = (self.LAUNCHER, OPS / "civvis-verified-head-launcher.sh")
        for source in sources:
            scripts = source.read_text().split("<<'APPLESCRIPT'\n")[1:]
            expected = 2 if source == self.LAUNCHER else 1
            self.assertGreaterEqual(
                len(scripts), expected,
                f"{source.name} is missing its Terminal window cleanup")
            for index, chunk in enumerate(scripts):
                script = chunk.split("\nAPPLESCRIPT", 1)[0]
                with self.subTest(source=source.name, script=index), TemporaryDirectory() as raw:
                    done = subprocess.run(
                        ["osacompile", "-o", str(Path(raw) / "x.scpt"), "-"],
                        input=script, capture_output=True, text=True, timeout=60)
                self.assertEqual(done.returncode, 0, done.stderr)

    def test_the_window_it_calls_its_own_must_have_a_live_shell(self):
        """A tty is not an identity once its shell has exited.

        macOS reassigns a tty device number as soon as the shell using it exits,
        so a dead launcher window keeps reporting the tty that the newly opened
        one was just given. Matching on tty alone claimed three windows as
        "mine" — measured 2026-08-18, `minimised 3, reaped 0` — and the dead
        ones were never reaped because each had already been taken for self.
        """
        text = self.LAUNCHER.read_text()
        self.assertIn("(tty of t) is myTty and (busy of w) is true", text)

    def test_the_reaper_knows_the_operator_wrapper_title(self):
        """Terminal keeps the name of the opened wrapper after its hand-off."""
        text = self.LAUNCHER.read_text()
        self.assertIn('contains "civvis-ladder-terminal-launcher"', text)
        self.assertIn('contains "civvis-verified-head-launcher"', text)

    def test_completion_and_outer_refusal_schedule_idle_only_cleanup(self):
        """A closing shell is not safely identifiable by its old tty."""
        for source in (self.LAUNCHER, OPS / "civvis-verified-head-launcher.sh"):
            with self.subTest(source=source.name):
                text = source.read_text()
                self.assertIn("schedule_idle_window_reap()", text)
                self.assertIn("/usr/bin/nohup /usr/bin/osascript", text)
                self.assertIn("delay 1", text)
                self.assertIn("(busy of w) is false", text)
                self.assertIn("trap 'schedule_idle_window_reap || true' EXIT", text)

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


@unittest.skipUnless(HAS_ZSH, "the launcher is zsh; this runner has no /bin/zsh")
class TheExhibitionIsKeptAliveToo(unittest.TestCase):
    """The other shipped product had no keeper at all.

    On 2026-08-18 the spectator supervisor exited and civvis.ai's exhibition
    stayed down until somebody looked. Its own restart loop could not help,
    because the worktree it execs its supervisor from had been deleted by the
    worktree reaper.
    """

    def test_the_exhibition_job_keeps_itself_alive(self):
        repo = Path(__file__).resolve().parent.parent
        plist = civvis_collab.macos_spectator_plist(Path("/x/run.sh"), repo).decode()
        self.assertIn("<key>KeepAlive</key>", plist)
        self.assertIn("<key>ThrottleInterval</key>", plist,
                      "a runner that refuses a missing prerequisite must not spin")

    def test_it_runs_directly_rather_than_through_terminal(self):
        """Unlike the ladder, and for a stated reason.

        The ladder's supervisor must go through Terminal because installing the
        Civ 6 control mod writes inside `Civ6.app`. The exhibition drives no GUI
        — `--no-open`, build, serve, play headless — so it needs no such grant.
        """
        repo = Path(__file__).resolve().parent.parent
        plist = civvis_collab.macos_spectator_plist(Path("/x/run.sh"), repo).decode()
        self.assertNotIn("Terminal", plist)
        self.assertIn("civvis-spectator-runner", "civvis-spectator-runner.sh")

    def test_a_host_without_the_source_worktree_gets_no_job(self):
        from tempfile import TemporaryDirectory
        with TemporaryDirectory() as raw:
            self.assertFalse(
                civvis_collab.host_serves_the_exhibition(Path(raw)),
                "installing a service that can only log a missing prerequisite "
                "is worse than an honest absence",
            )

    def test_the_runner_refuses_a_missing_supervisor_rather_than_looping(self):
        import subprocess
        runner = OPS / "civvis-spectator-runner.sh"
        done = subprocess.run(
            ["/bin/zsh", str(runner)],
            env={**os.environ, "CIVVIS_SPECTATOR_SRC": "/tmp/civvis-absent-on-purpose"},
            capture_output=True, text=True, timeout=60,
        )
        self.assertEqual(done.returncode, 78, done.stderr)
        self.assertIn("SPECTATOR_DEPLOY.md", done.stderr)
