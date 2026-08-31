#!/usr/bin/env python3
"""The host-side verification automation installs from the tracked tree.

Until 2026-08-28 the pieces that keep a Mac playing verification games — the
operator's launch wrapper, the `civvis-games` switch, the run-retention job and
their launchd agents — lived only in one machine's home directory:
`~/civvis-verified-head-launcher.zsh`, `~/bin/civvis-games`,
`~/.local/bin/civvis_run_prune.sh`, two hand-written plists and a block of
`export`s in `~/.zprofile`. No other host could install them, no CI run had
ever executed them, and every one named that machine's home directory outright
— the class of defect `test_ops_portability.py` exists to stop. These tests
hold the tracked replacements to the contract that makes them shareable.

The scripts are zsh, which macOS ships and the Linux CI runner does not; those
tests skip there rather than assert about a shell the target host is guaranteed
to have and the runner is guaranteed not to. The plist templates and the static
contracts are checked everywhere.
"""

from __future__ import annotations

import os
import plistlib
import re
import shutil
import subprocess
import sys
import time
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

TOOLS = Path(__file__).resolve().parent
OPS = TOOLS / "ops"
REPO = TOOLS.parent
sys.path.insert(0, str(TOOLS))

import civvis_collab  # noqa: E402

WRAPPER = OPS / "civvis-verified-head-launcher.sh"
SWITCH = OPS / "civvis-games.sh"
PRUNE = OPS / "civvis-run-prune.sh"
INSTALLER = OPS / "civvis-install-host-automation.sh"
LABELS = ("com.civvis.keepplaying", "com.civvis.run-prune")
TEMPLATES = {label: REPO / "deploy" / f"{label}.plist" for label in LABELS}
GITHUB = "https://github.com/MartinHalvorson/CIVVIS.git"
HAS_ZSH = shutil.which("zsh") is not None
# The same rule as test_ops_portability.HARDCODED_HOME, restated rather than
# imported: importing that module here would re-register its cases under this
# one's name.
HARDCODED_HOME = re.compile(r"/Users/(?!\$)[A-Za-z][A-Za-z0-9._-]*/")


def zsh(script: Path, *args: str, env=None, timeout: int = 90):
    return subprocess.run(["zsh", str(script), *args], env=env,
                          capture_output=True, text=True, timeout=timeout)


def clean_env(**extra: str) -> dict:
    """The caller's environment minus every CIVVIS_* export, plus `extra`."""
    env = {k: v for k, v in os.environ.items() if not k.startswith("CIVVIS_")}
    env.update(extra)
    return env


def make_tree(root: Path, origin=GITHUB, branch: str = "scratch") -> Path:
    """A buildable-looking git tree with the given origin, on `branch`."""
    root.mkdir(parents=True, exist_ok=True)
    subprocess.run(["git", "init", "-q", "-b", branch, str(root)],
                   check=True, capture_output=True)
    if origin:
        subprocess.run(["git", "-C", str(root), "remote", "add", "origin", origin],
                       check=True, capture_output=True)
    (root / "Cargo.toml").write_text('[package]\nname = "stub"\n')
    return root


class NothingNamesAHome(unittest.TestCase):
    """A script that names its author's home directory installs nowhere else."""

    def test_the_four_scripts_derive_their_paths(self):
        for script in (WRAPPER, SWITCH, PRUNE, INSTALLER):
            executable = [line for line in script.read_text().splitlines()
                          if not line.lstrip().startswith("#")]
            hits = [hit for line in executable for hit in HARDCODED_HOME.findall(line)]
            self.assertEqual(hits, [], f"{script.name} names a home directory")
        for script in (WRAPPER, SWITCH, INSTALLER):
            self.assertIn("OPS=${0:A:h}", script.read_text(),
                          f"{script.name} calls siblings and must derive the tracked ops directory")

    def test_the_link_name_is_the_one_the_keeper_honours(self):
        """Three files agree on one name, and civvis_collab is the authority.

        The keeper is handed `--supervisor <wrapper>` only when
        LADDER_OPERATOR_WRAPPER exists; the installer creates that name and
        the switch opens it. A rename in any one of them silently splits the
        keeper's loop from the operator's.
        """
        name = civvis_collab.LADDER_OPERATOR_WRAPPER.name
        for script in (INSTALLER, SWITCH, WRAPPER):
            self.assertIn(name, script.read_text(),
                          f"{script.name} does not name {name}")

    def test_the_templates_render_to_valid_plists_that_do_not_run_the_loop(self):
        for label, template in TEMPLATES.items():
            raw = template.read_text()
            self.assertIn("__HOME__", raw)
            self.assertIn("__OPS__", raw)
            rendered = raw.replace("__HOME__", "/x/home").replace("__OPS__", "/x/ops")
            data = plistlib.loads(rendered.encode("utf-8"))
            self.assertEqual(data["Label"], label)
            self.assertEqual(data["ProgramArguments"][0], "/bin/zsh")
            self.assertTrue(data["ProgramArguments"][1].startswith("/x/ops/"))
            self.assertEqual(data["EnvironmentVariables"]["HOME"], "/x/home")
            self.assertTrue(data["StandardOutPath"].startswith("/x/home/"))
            # A LaunchAgent cannot install the control mod, so it cannot play
            # (test_ops_portability.ManagedServicesCanBeUpdated).
            self.assertNotIn("civvis-game-supervisor.sh", rendered)
            self.assertNotIn("civvis-ladder-terminal-launcher.sh", rendered)
        keep = plistlib.loads(TEMPLATES["com.civvis.keepplaying"].read_text()
                              .replace("__HOME__", "/h").replace("__OPS__", "/o")
                              .encode("utf-8"))
        self.assertEqual(keep["StartInterval"], 300)
        self.assertEqual(keep["ProgramArguments"][-1], "ensure")
        prune = plistlib.loads(TEMPLATES["com.civvis.run-prune"].read_text()
                               .replace("__HOME__", "/h").replace("__OPS__", "/o")
                               .encode("utf-8"))
        self.assertEqual(prune["StartCalendarInterval"], {"Hour": 3, "Minute": 17})


class _Host:
    """A throwaway home with a game tree, a pin, a policy slot and a stub launcher."""

    def __init__(self, raw: str):
        self.home = Path(raw)
        self.tree = make_tree(self.home / "tree")
        self.pin = self.home / "pin"
        self.pin.write_text("head\n")
        self.policy = self.home / "policy"
        self.log = self.home / "ladder.log"
        self.out = self.home / "stub-env"
        self.stub = self.home / "stub.sh"
        self.stub.write_text('#!/bin/zsh\nenv > "$STUB_OUT"\nprint -r -- stub-ran\n')
        self.stub.chmod(0o755)

    def env(self, **extra: str) -> dict:
        # ⚠ CIVVIS_FOREGROUND_GUARD=0, always. The wrapper starts the real
        # foreground guard detached; without this, every wrapper run here left
        # one behind in a HOME that was deleted a moment later — 28 of them on
        # 2026-08-28, each hammering System Events until nothing answered.
        env = clean_env(HOME=str(self.home), CIVVIS_PINFILE=str(self.pin),
                        CIVVIS_VERIFICATION_POLICY=str(self.policy),
                        CIVVIS_LADDER_LOG=str(self.log),
                        CIVVIS_LADDER_LAUNCHER=str(self.stub),
                        CIVVIS_FOREGROUND_GUARD="0",
                        STUB_OUT=str(self.out))
        env.update(extra)
        return env

    def write_policy(self, *lines: str, tree=None) -> None:
        head = f"CIVVIS_HEAD_REPO={tree or self.tree}"
        self.policy.write_text("".join(f"{line}\n" for line in (head, *lines)))

    def launched(self) -> dict:
        return dict(line.split("=", 1) for line in self.out.read_text().splitlines()
                    if "=" in line)

    def logged(self) -> str:
        return self.log.read_text() if self.log.exists() else ""


@unittest.skipUnless(HAS_ZSH, "the wrapper is zsh; this runner has no zsh")
class TheWrapperAppliesThePolicyAndNothingElse(unittest.TestCase):
    def test_defaults_when_the_policy_only_names_the_tree(self):
        with TemporaryDirectory() as raw:
            host = _Host(raw)
            host.write_policy()
            done = zsh(WRAPPER, env=host.env())
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertIn("stub-ran", done.stdout)
            env = host.launched()
            self.assertEqual(env["CIVVIS_PLAY_ATTEMPTS"], "1")
            self.assertEqual(env["CIVVIS_PLAY_TIMEOUT"], "10800")
            self.assertEqual(env["CIVVIS_PLAY_TIMEOUT_CEILING"], "14400")
            self.assertEqual(env["CIVVIS_HEAD_REPO"], str(host.tree))
            self.assertEqual(env["CIVVIS_PINFILE"], str(host.pin))
            self.assertNotIn("CIVVIS_DIFFICULTY", env,
                             "absent policy leaves the rung to the ladder policy")
            self.assertNotIn("CIVVIS_RESTART_BELOW_LEADER_RATIO", env,
                             "absent policy leaves the abandon line to the harness")
            self.assertIn("launching from", host.logged())

    def test_stale_exports_never_reach_the_launcher(self):
        """The 2026-08-2x failure: a window's `export CIVVIS_STRATEGY=g40-37`."""
        with TemporaryDirectory() as raw:
            host = _Host(raw)
            host.write_policy()
            done = zsh(WRAPPER, env=host.env(
                CIVVIS_STRATEGY="g40-37", CIVVIS_WITH="barbarian-hunt",
                CIVVIS_DIFFICULTY="DIFFICULTY_DEITY",
                CIVVIS_RESTART_BELOW_LEADER_RATIO="0",
                CIVVIS_LADDER_HOST="/nowhere/host.sh",
                CIVVIS_HEAD_REPO="/a/reaped/worktree"))
            self.assertEqual(done.returncode, 0, done.stderr)
            env = host.launched()
            for key in ("CIVVIS_STRATEGY", "CIVVIS_WITH", "CIVVIS_DIFFICULTY",
                        "CIVVIS_RESTART_BELOW_LEADER_RATIO", "CIVVIS_LADDER_HOST"):
                self.assertNotIn(key, env, f"{key} leaked through the wrapper")
            self.assertEqual(env["CIVVIS_HEAD_REPO"], str(host.tree),
                             "the game tree is the policy's, not the window's")

    def test_the_policy_is_applied(self):
        with TemporaryDirectory() as raw:
            host = _Host(raw)
            host.write_policy("CIVVIS_DIFFICULTY = DIFFICULTY_KING  # the rung",
                              "CIVVIS_RESTART_BELOW_LEADER_RATIO=0",
                              "CIVVIS_PLAY_ATTEMPTS=3", "CIVVIS_VICTORY=science",
                              "", "# a comment line", "CIVVIS_PLAY_TIMEOUT=7200")
            done = zsh(WRAPPER, env=host.env())
            self.assertEqual(done.returncode, 0, done.stderr)
            env = host.launched()
            self.assertEqual(env["CIVVIS_DIFFICULTY"], "DIFFICULTY_KING")
            self.assertEqual(env["CIVVIS_RESTART_BELOW_LEADER_RATIO"], "0")
            self.assertEqual(env["CIVVIS_PLAY_ATTEMPTS"], "3")
            self.assertEqual(env["CIVVIS_VICTORY"], "science")
            self.assertEqual(env["CIVVIS_PLAY_TIMEOUT"], "7200")
            self.assertEqual(env["CIVVIS_PLAY_TIMEOUT_CEILING"], "14400")

    def test_an_invalid_value_refuses_and_says_so_in_the_log(self):
        cases = ("CIVVIS_DIFFICULTY=DIFFICULTY_GODLIKE",
                 "CIVVIS_RESTART_BELOW_LEADER_RATIO=1.5",
                 "CIVVIS_PLAY_ATTEMPTS=0",
                 "CIVVIS_PLAY_TIMEOUT=soon",
                 "just words")
        for bad in cases:
            with self.subTest(bad=bad), TemporaryDirectory() as raw:
                host = _Host(raw)
                host.write_policy(bad)
                done = zsh(WRAPPER, env=host.env())
                self.assertEqual(done.returncode, 64, done.stdout + done.stderr)
                self.assertFalse(host.out.exists(), "the launcher must not run")
                self.assertIn("REFUSING", host.logged())
                # the wrapper strips whitespace before it judges a line
                self.assertIn(bad.replace(" ", "").split("=")[-1], host.logged())

    def test_an_unknown_key_is_ignored_loudly(self):
        with TemporaryDirectory() as raw:
            host = _Host(raw)
            host.write_policy("CIVVIS_STRATEGY=g40-37")
            done = zsh(WRAPPER, env=host.env())
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertNotIn("CIVVIS_STRATEGY", host.launched())
            self.assertIn("ignoring unknown policy key 'CIVVIS_STRATEGY'", host.logged())

    def test_the_pin_must_be_head(self):
        for pin in ("/some/other/tree\n", None):
            with self.subTest(pin=pin), TemporaryDirectory() as raw:
                host = _Host(raw)
                host.write_policy()
                if pin is None:
                    host.pin.unlink()
                else:
                    host.pin.write_text(pin)
                done = zsh(WRAPPER, env=host.env())
                self.assertEqual(done.returncode, 64, done.stderr)
                self.assertIn("must contain exactly 'head'", host.logged())

    def test_the_origin_must_be_the_github_civvis(self):
        with TemporaryDirectory() as raw:
            host = _Host(raw)
            other = make_tree(host.home / "fork", origin="https://example.com/x/CIVVIS.git")
            host.write_policy(tree=other)
            done = zsh(WRAPPER, env=host.env())
            self.assertEqual(done.returncode, 64, done.stderr)
            self.assertIn("not the GitHub CIVVIS", host.logged())
        for origin in ("git@github.com:MartinHalvorson/CIVVIS.git",
                       "ssh://git@github.com/MartinHalvorson/CIVVIS",
                       "https://github.com/MartinHalvorson/CIVVIS"):
            with self.subTest(origin=origin), TemporaryDirectory() as raw:
                host = _Host(raw)
                ssh = make_tree(host.home / "ssh", origin=origin)
                host.write_policy(tree=ssh)
                done = zsh(WRAPPER, env=host.env())
                self.assertEqual(done.returncode, 0, done.stderr)

    def test_a_tree_attached_to_main_is_refused(self):
        """The supervisor detaches the game tree every cycle; the freshness
        service's main_worktree() then finds no `main` and syncs nothing."""
        with TemporaryDirectory() as raw:
            host = _Host(raw)
            attached = make_tree(host.home / "main-tree", branch="main")
            host.write_policy(tree=attached)
            done = zsh(WRAPPER, env=host.env())
            self.assertEqual(done.returncode, 64, done.stderr)
            self.assertIn("attached to branch main", host.logged())

    def test_a_missing_launcher_is_its_own_status(self):
        with TemporaryDirectory() as raw:
            host = _Host(raw)
            host.write_policy()
            done = zsh(WRAPPER, env=host.env(CIVVIS_LADDER_LAUNCHER=str(host.home / "absent")))
            self.assertEqual(done.returncode, 66, done.stderr)

    def test_no_wrapper_run_here_starts_a_real_guard(self):
        """The wrapper starts the foreground guard detached; a test that lets it
        would leave a real guard running in a HOME that is deleted a moment
        later. Every _Host environment must say no."""
        with TemporaryDirectory() as raw:
            host = _Host(raw)
            host.write_policy()
            done = zsh(WRAPPER, env=host.env())
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertNotIn("foreground guard started", host.logged())
            self.assertEqual(host.env()["CIVVIS_FOREGROUND_GUARD"], "0")

    def test_it_hands_over_to_the_sibling_launcher_by_default(self):
        text = WRAPPER.read_text()
        self.assertIn("LAUNCHER=${CIVVIS_LADDER_LAUNCHER:-$OPS/civvis-ladder-terminal-launcher.sh}",
                      text)
        self.assertIn('exec /bin/zsh "$LAUNCHER"', text)

@unittest.skipUnless(HAS_ZSH, "the installer is zsh; this runner has no zsh")
class TheInstallerWiresAHostToTheTrackedTree(unittest.TestCase):
    def _install(self, home: Path, *args: str, **extra: str):
        return zsh(INSTALLER, "--no-launchctl", *args, env=clean_env(HOME=str(home), **extra))

    def test_it_links_renders_and_seeds(self):
        with TemporaryDirectory() as raw:
            home = Path(raw) / "home"
            home.mkdir()
            game = make_tree(Path(raw) / "game")
            done = self._install(home, "--head-repo", str(game),
                                 CIVVIS_DIFFICULTY="DIFFICULTY_KING", CIVVIS_PLAY_ATTEMPTS="1")
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertEqual((home / "bin" / "civvis-games").resolve(), SWITCH)
            link = home / civvis_collab.LADDER_OPERATOR_WRAPPER.name
            self.assertTrue(link.is_symlink())
            self.assertEqual(link.resolve(), WRAPPER)
            for label in LABELS:
                plist = home / "Library" / "LaunchAgents" / f"{label}.plist"
                text = plist.read_text()
                self.assertNotIn("__HOME__", text)
                self.assertNotIn("__OPS__", text)
                data = plistlib.loads(plist.read_bytes())
                self.assertEqual(data["Label"], label)
                self.assertTrue(data["ProgramArguments"][1].startswith(str(OPS) + "/"),
                                "the job must run the tracked script")
                self.assertEqual(data["EnvironmentVariables"]["HOME"], str(home))
            policy = (home / ".civvis-verification-policy").read_text()
            self.assertIn(f"CIVVIS_HEAD_REPO={game}", policy)
            self.assertIn("CIVVIS_DIFFICULTY=DIFFICULTY_KING", policy,
                          "a .zprofile-style export migrates into the policy")
            self.assertIn("CIVVIS_PLAY_ATTEMPTS=1", policy)
            self.assertIn("#CIVVIS_VICTORY=", policy, "unset keys are left as prompts")
            # A second run is a no-op that says so, and leaves the policy alone.
            (home / ".civvis-verification-policy").write_text(policy + "# edited by hand\n")
            again = self._install(home, "--head-repo", str(game))
            self.assertEqual(again.returncode, 0, again.stdout + again.stderr)
            self.assertIn("plist unchanged", again.stdout)
            self.assertIn("keeping", again.stdout)
            self.assertIn("# edited by hand", (home / ".civvis-verification-policy").read_text())

    def test_an_operators_own_wrapper_is_kept_unless_told(self):
        with TemporaryDirectory() as raw:
            home = Path(raw) / "home"
            home.mkdir()
            game = make_tree(Path(raw) / "game")
            own = home / civvis_collab.LADDER_OPERATOR_WRAPPER.name
            own.write_text("#!/bin/zsh\nexec true\n")
            done = self._install(home, "--head-repo", str(game))
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertFalse(own.is_symlink(), "an operator's own wrapper survives")
            self.assertIn("--replace-wrapper", done.stdout)
            done = self._install(home, "--head-repo", str(game), "--replace-wrapper")
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertTrue(own.is_symlink())
            retired = list(home.glob(own.name + ".retired-*"))
            self.assertEqual(len(retired), 1, "the operator's copy is kept, not deleted")
            self.assertEqual(retired[0].read_text(), "#!/bin/zsh\nexec true\n")

    def test_the_pre_repo_labels_are_retired(self):
        with TemporaryDirectory() as raw:
            home = Path(raw) / "home"
            agents = home / "Library" / "LaunchAgents"
            agents.mkdir(parents=True)
            old = agents / "com.martbot.civvis-keepplaying.plist"
            old.write_text("<plist/>")
            game = make_tree(Path(raw) / "game")
            done = self._install(home, "--head-repo", str(game))
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertFalse(old.exists())
            self.assertEqual(len(list(agents.glob("com.martbot.civvis-keepplaying.plist.retired-*"))), 1)
            self.assertTrue((agents / "com.civvis.keepplaying.plist").is_file())

    def test_a_dry_run_changes_nothing(self):
        with TemporaryDirectory() as raw:
            home = Path(raw) / "home"
            home.mkdir()
            game = make_tree(Path(raw) / "game")
            done = self._install(home, "--dry-run", "--head-repo", str(game))
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertIn("dry run", done.stdout)
            self.assertEqual(sorted(p.name for p in home.iterdir()), [],
                             "a dry run must create nothing")

    def test_a_main_tree_needs_a_separate_game_tree(self):
        """Run a COPY of the installer from a tree attached to `main`."""
        with TemporaryDirectory() as raw:
            tree = make_tree(Path(raw) / "tree", branch="main")
            (tree / "tools" / "ops").mkdir(parents=True)
            (tree / "deploy").mkdir()
            for script in (WRAPPER, SWITCH, PRUNE, INSTALLER):
                shutil.copy2(script, tree / "tools" / "ops" / script.name)
            for label, template in TEMPLATES.items():
                shutil.copy2(template, tree / "deploy" / template.name)
            home = Path(raw) / "home"
            home.mkdir()
            copy = tree / "tools" / "ops" / INSTALLER.name
            done = zsh(copy, "--no-launchctl", env=clean_env(HOME=str(home)))
            self.assertEqual(done.returncode, 64, done.stdout + done.stderr)
            self.assertIn("attached to branch main", done.stderr)
            game = make_tree(Path(raw) / "game")
            done = zsh(copy, "--no-launchctl", "--head-repo", str(game),
                       env=clean_env(HOME=str(home)))
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertEqual((home / "bin" / "civvis-games").resolve(),
                             (tree / "tools" / "ops" / SWITCH.name).resolve(),
                             "the links point at the tree the installer ran from")
            attached = make_tree(Path(raw) / "attached", branch="main")
            done = zsh(copy, "--no-launchctl", "--head-repo", str(attached),
                       env=clean_env(HOME=str(home)))
            self.assertEqual(done.returncode, 64, "a game tree on main is refused too")

    def test_an_ephemeral_tree_is_refused(self):
        with TemporaryDirectory() as raw:
            tree = make_tree(Path(raw) / ".civvis-batch-scratch" / "repo")
            (tree / "tools" / "ops").mkdir(parents=True)
            (tree / "deploy").mkdir()
            for script in (WRAPPER, SWITCH, PRUNE, INSTALLER):
                shutil.copy2(script, tree / "tools" / "ops" / script.name)
            for template in TEMPLATES.values():
                shutil.copy2(template, tree / "deploy" / template.name)
            home = Path(raw) / "home"
            home.mkdir()
            done = zsh(tree / "tools" / "ops" / INSTALLER.name, "--no-launchctl",
                       env=clean_env(HOME=str(home)))
            self.assertEqual(done.returncode, 64, done.stdout + done.stderr)
            self.assertIn("ephemeral", done.stderr)


@unittest.skipUnless(HAS_ZSH, "the switch is zsh; this runner has no zsh")
class TheSwitchIsTheTrackedOne(unittest.TestCase):
    def test_a_bad_verb_is_usage(self):
        with TemporaryDirectory() as raw:
            done = zsh(SWITCH, "bogus", env=clean_env(HOME=raw))
            self.assertEqual(done.returncode, 64)
            self.assertIn("usage:", done.stdout)

    def test_it_opens_the_operator_wrapper_when_the_host_has_one(self):
        text = SWITCH.read_text()
        self.assertIn('WRAPPER=${CIVVIS_LADDER_WRAPPER:-$HOME/'
                      + civvis_collab.LADDER_OPERATOR_WRAPPER.name + "}", text)
        self.assertIn('open -g -j -a Terminal "$(launcher)"', text)
        self.assertNotIn("$HOME/CIVVIS", text, "no named checkout")

    def test_retire_is_a_recorded_one_game_exit_not_the_off_teardown(self):
        text = SWITCH.read_text()
        start = text.index("retire)\n")
        end = text.index("\noff)\n", start)
        retire = text[start:end]
        self.assertIn("operator_retire.py", retire)
        self.assertIn("--runs-root \"$RUN_ROOT\"", retire)
        self.assertIn("set_intent running", retire)
        self.assertIn("no process was stopped", retire)
        self.assertNotIn("term_wait", retire)
        self.assertNotIn("--halt --reason", retire)

    def test_retire_keeps_the_lane_intent_only_after_a_real_request(self):
        with TemporaryDirectory() as raw:
            home = Path(raw) / "home"
            home.mkdir()
            fake_bin = home / "fake-bin"
            fake_bin.mkdir()
            python = fake_bin / "python3"
            python.write_text(
                "#!/bin/zsh\n"
                "if [[ \"$1\" == *gamelock.py && \"$2\" == --halt-status ]]; then exit 1; fi\n"
                "if [[ \"$1\" == *operator_retire.py ]]; then exit ${RETIRE_EXIT:-0}; fi\n"
                "exit 0\n")
            python.chmod(0o755)
            base = clean_env(HOME=str(home), CIVVIS_REPO=str(REPO),
                             PATH=str(fake_bin) + os.pathsep + os.environ["PATH"])

            done = zsh(SWITCH, "retire", env=base)
            self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
            self.assertEqual((home / ".civvis-operator-intent").read_text(), "running\n")
            self.assertIn("no process was stopped", done.stdout)

            (home / ".civvis-operator-intent").unlink()
            refused = zsh(SWITCH, "retire", env={**base, "RETIRE_EXIT": "7"})
            self.assertEqual(refused.returncode, 7, refused.stdout + refused.stderr)
            self.assertFalse((home / ".civvis-operator-intent").exists(),
                             "a failed binding cannot start an idle lane")

    def test_turning_on_writes_a_head_pin_before_starting_terminal(self):
        """A wrapper refuses anything but `head`, so its launch must see it.

        A missing pin used to be treated as though it were already `head`, and
        a stale tree pin was only reset after `open` had already handed the
        wrapper to Terminal.  Both cases made the operator's `on` command look
        successful while the wrapper immediately refused its launch.
        """
        for old_pin in ("/a/stale/tree\n", None):
            with self.subTest(old_pin=old_pin), TemporaryDirectory() as raw:
                home = Path(raw) / "home"
                home.mkdir()
                agents = home / "Library" / "LaunchAgents"
                agents.mkdir(parents=True)
                for label in ("com.civvis.ladder-watchdog", "com.civvis.spectator"):
                    (agents / f"{label}.plist").write_text("<plist/>")
                pin = home / ".civvis-play-pin"
                if old_pin is not None:
                    pin.write_text(old_pin)
                wrapper = home / civvis_collab.LADDER_OPERATOR_WRAPPER.name
                wrapper.write_text("#!/bin/zsh\nexit 0\n")
                wrapper.chmod(0o755)

                fake_bin = home / "fake-bin"
                fake_bin.mkdir()
                for name, source in {
                    "python3": "#!/bin/zsh\nexit 0\n",
                    "launchctl": "#!/bin/zsh\nexit 0\n",
                    "pgrep": "#!/bin/zsh\nexit 1\n",
                    "open": ("#!/bin/zsh\n"
                             "cat \"$HOME/.civvis-play-pin\" > \"$OPEN_PIN\"\n"),
                }.items():
                    command = fake_bin / name
                    command.write_text(source)
                    command.chmod(0o755)
                open_pin = home / "pin-seen-by-terminal"
                done = zsh(
                    SWITCH,
                    "on",
                    env=clean_env(
                        HOME=str(home),
                        CIVVIS_REPO=str(REPO),
                        OPEN_PIN=str(open_pin),
                        PATH=str(fake_bin) + os.pathsep + os.environ["PATH"],
                    ),
                )
                self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
                self.assertEqual(pin.read_text(), "head\n")
                self.assertEqual(open_pin.read_text(), "head\n",
                                 "Terminal must receive a verifiable head pin")


@unittest.skipUnless(HAS_ZSH, "the prune job is zsh; this runner has no zsh")
class RetentionKeepsWhatTheLadderReads(unittest.TestCase):
    def test_old_runs_go_and_the_ledgers_the_newest_and_the_young_stay(self):
        with TemporaryDirectory() as raw:
            root = Path(raw) / "control"
            root.mkdir()
            old, older, young = (root / f"civvis-{n}" for n in ("old", "older", "young"))
            for run in (old, older, young):
                run.mkdir()
                (run / "events.jsonl").write_text('{"kind": "state"}\n')
            now = time.time()
            os.utime(older, (now - 5 * 86400, now - 5 * 86400))
            os.utime(old, (now - 3 * 86400, now - 3 * 86400))
            for ledger in ("ladder.json", "civvis_ladder.jsonl"):
                (root / ledger).write_text("{}")
                os.utime(root / ledger, (now - 9 * 86400, now - 9 * 86400))
            log = Path(raw) / "prune.log"
            env = clean_env(HOME=raw, CIVVIS_RUNS_ROOT=str(root),
                            CIVVIS_RUN_PRUNE_LOG=str(log))
            dry = zsh(PRUNE, "--dry-run", env=env)
            self.assertEqual(dry.returncode, 0, dry.stderr)
            self.assertIn("would prune", dry.stdout)
            self.assertIn("civvis-older", dry.stdout)
            self.assertIn("civvis-old", dry.stdout)
            self.assertNotIn("civvis-young", dry.stdout)
            self.assertTrue(older.is_dir() and old.is_dir(), "a dry run deletes nothing")
            self.assertFalse(log.exists(), "a dry run writes no ledger line")

            done = zsh(PRUNE, env=env)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertFalse(older.exists())
            self.assertFalse(old.exists())
            self.assertTrue(young.is_dir())
            for ledger in ("ladder.json", "civvis_ladder.jsonl"):
                self.assertTrue((root / ledger).is_file(), f"{ledger} is the ladder's memory")
            self.assertIn("pruned 2 run dir(s)", log.read_text())

    def test_the_newest_run_survives_whatever_its_age(self):
        with TemporaryDirectory() as raw:
            root = Path(raw) / "control"
            root.mkdir()
            a, b = root / "civvis-a", root / "civvis-b"
            a.mkdir()
            b.mkdir()
            now = time.time()
            os.utime(a, (now - 6 * 86400, now - 6 * 86400))
            os.utime(b, (now - 4 * 86400, now - 4 * 86400))
            env = clean_env(HOME=raw, CIVVIS_RUNS_ROOT=str(root),
                            CIVVIS_RUN_PRUNE_LOG=str(Path(raw) / "prune.log"))
            done = zsh(PRUNE, env=env)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertFalse(a.exists())
            self.assertTrue(b.is_dir(), "the newest run is never pruned")

    def test_an_absent_root_is_a_quiet_exit(self):
        with TemporaryDirectory() as raw:
            done = zsh(PRUNE, env=clean_env(HOME=raw, CIVVIS_RUNS_ROOT=str(Path(raw) / "none")))
            self.assertEqual(done.returncode, 0)


if __name__ == "__main__":
    unittest.main()
