#!/usr/bin/env python3
"""The live seat's forced-on gene list is the repository's, not one account's.

Until 2026-09-09 the only force-on list was ``~/.civvis-live-force-on`` on
whichever account played, so what the seat played was invisible to the
repository and differed per machine: one night's ladder rows carried a
six-tag arm while another account's file named nine other tags.  The
supervisor now resolves ONE file per batch, in this order:

1. ``CIVVIS_WITH_FILE`` -- an explicit path in the process environment;
2. ``~/.civvis-live-force-on`` -- a local operator override, honoured only
   while it exists and is non-empty, and LOGGED as an override when it wins;
3. ``deploy/live-force-on.txt`` in the tree being built -- the versioned list;
4. none.

These tests replay the supervisor's own ``resolve_forced_arm`` under zsh
(skipped where zsh is absent -- the CI runner is Linux) and pin the shape of
the versioned file: one comma-separated line of tags that exist in the gene
registry, since a tag the decider cannot seat makes the supervisor refuse
every batch (``docs/LIVE_SCREEN.md``).
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

ROOT = Path(__file__).resolve().parent.parent
SUPERVISOR = ROOT / "tools" / "ops" / "civvis-game-supervisor.sh"
GAMES = ROOT / "tools" / "ops" / "civvis-games.sh"
FORCE_LIST = ROOT / "deploy" / "live-force-on.txt"
REGISTRY = ROOT / "src" / "ai" / "advanced" / "genes.rs"

TAG = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")


def resolver_block() -> str:
    """The supervisor's force-file preamble and both resolver functions."""
    source = SUPERVISOR.read_text()
    start = source.index("FORCED_ENV=${CIVVIS_WITH:-}")
    marker = "resolve_forced_arm() {"
    body = source.index(marker)
    end = source.index("\n}\n", body) + len("\n}\n")
    return source[start:end]


class TheVersionedList(unittest.TestCase):
    def test_it_is_one_line_of_registry_tags(self) -> None:
        text = FORCE_LIST.read_text()
        self.assertTrue(text.endswith("\n"), "end the file with one newline")
        line = text[:-1]
        self.assertNotIn("\n", line, "the supervisor refuses a multi-line file")
        self.assertFalse(re.search(r"\s", line), "no whitespace: it refuses the batch")
        tags = line.split(",")
        self.assertEqual(tags, sorted(tags), "keep the list sorted, as ladder rows record it")
        self.assertEqual(len(tags), len(set(tags)), "no duplicate tags")
        registry = REGISTRY.read_text()
        for tag in tags:
            self.assertRegex(tag, TAG)
            self.assertIn(f'tag: "{tag}"', registry,
                          f"{tag!r} is not a gene registry row; the decider would exit 2 "
                          "and the supervisor refuse every batch")

    def test_the_status_report_names_it(self) -> None:
        source = GAMES.read_text()
        self.assertIn("deploy/live-force-on.txt", source)
        self.assertIn("DIFFERS from the repo list", source,
                      "status must flag a local override that departs from the repo list")


class TheResolutionOrder(unittest.TestCase):
    def setUp(self) -> None:
        if shutil.which("zsh") is None:
            self.skipTest("zsh is needed here")
        self.tmp = tempfile.TemporaryDirectory()
        self.home = Path(self.tmp.name) / "home"
        self.repo = Path(self.tmp.name) / "repo"
        (self.repo / "deploy").mkdir(parents=True)
        self.home.mkdir()

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def repo_list(self, text: str) -> None:
        (self.repo / "deploy" / "live-force-on.txt").write_text(text)

    def local_list(self, text: str) -> None:
        (self.home / ".civvis-live-force-on").write_text(text)

    def resolve(self, **env: str) -> tuple[int, dict[str, str], list[str]]:
        script = (
            'say() { print -r -- "LOG $*" }\n'
            + resolver_block()
            + 'REPO=$1\n'
            + 'if resolve_forced_arm; then rc=0; else rc=$?; fi\n'
            + 'print -r -- "FORCED=$FORCED"\n'
            + 'print -r -- "SOURCE=$FORCE_SOURCE"\n'
            + 'print -r -- "ARGS=${(j: :)WITH_ARGS}"\n'
            + 'exit $rc\n')
        path = Path(self.tmp.name) / "drive.sh"
        path.write_text(script)
        run_env = {k: v for k, v in os.environ.items()
                   if not k.startswith("CIVVIS_")}
        run_env["HOME"] = str(self.home)
        run_env.update(env)
        done = subprocess.run(["zsh", str(path), str(self.repo)], capture_output=True,
                              text=True, timeout=60, env=run_env)
        self.assertEqual(done.stderr, "")
        values: dict[str, str] = {}
        logs: list[str] = []
        for line in done.stdout.splitlines():
            if line.startswith("LOG "):
                logs.append(line[4:])
            else:
                key, _, value = line.partition("=")
                values[key] = value
        return done.returncode, values, logs

    def test_the_repo_list_plays_when_nothing_overrides_it(self) -> None:
        self.repo_list("alpha-one,beta-two\n")
        rc, got, logs = self.resolve()
        self.assertEqual(rc, 0)
        self.assertEqual(got["FORCED"], "alpha-one,beta-two")
        self.assertEqual(got["SOURCE"], "repo:deploy/live-force-on.txt")
        self.assertEqual(got["ARGS"], "--with alpha-one --with beta-two")
        self.assertEqual(logs, [], "the default needs no override notice")

    def test_no_file_at_all_is_the_stock_genome(self) -> None:
        rc, got, _ = self.resolve()
        self.assertEqual(rc, 0)
        self.assertEqual(got["FORCED"], "")
        self.assertEqual(got["SOURCE"], "none")
        self.assertEqual(got["ARGS"], "")

    def test_a_non_empty_local_file_overrides_and_is_logged(self) -> None:
        self.repo_list("alpha-one,beta-two\n")
        self.local_list("gamma-three\n")
        rc, got, logs = self.resolve()
        self.assertEqual(rc, 0)
        self.assertEqual(got["FORCED"], "gamma-three")
        self.assertEqual(got["SOURCE"], f"local-override:{self.home}/.civvis-live-force-on")
        self.assertEqual(len(logs), 1, logs)
        self.assertIn("LOCAL OVERRIDE", logs[0])
        self.assertIn("gamma-three", logs[0])
        self.assertIn("alpha-one,beta-two", logs[0], "the notice names what it displaced")

    def test_a_local_file_equal_to_the_repo_list_is_still_called_an_override(self) -> None:
        self.repo_list("alpha-one\n")
        self.local_list("alpha-one\n")
        rc, got, logs = self.resolve()
        self.assertEqual(rc, 0)
        self.assertEqual(got["FORCED"], "alpha-one")
        self.assertTrue(got["SOURCE"].startswith("local-override:"))
        self.assertEqual(len(logs), 1)
        self.assertIn("equals the repo list", logs[0])

    def test_an_empty_local_file_falls_through_to_the_repo_list(self) -> None:
        self.repo_list("alpha-one\n")
        self.local_list("")
        rc, got, logs = self.resolve()
        self.assertEqual(rc, 0)
        self.assertEqual(got["FORCED"], "alpha-one")
        self.assertEqual(got["SOURCE"], "repo:deploy/live-force-on.txt")
        self.assertEqual(logs, [])

    def test_the_environment_file_beats_both(self) -> None:
        self.repo_list("alpha-one\n")
        self.local_list("gamma-three\n")
        explicit = Path(self.tmp.name) / "explicit"
        explicit.write_text("delta-four\n")
        rc, got, logs = self.resolve(CIVVIS_WITH_FILE=str(explicit))
        self.assertEqual(rc, 0)
        self.assertEqual(got["FORCED"], "delta-four")
        self.assertEqual(got["SOURCE"], f"env-file:{explicit}")
        self.assertEqual(logs, [])

    def test_an_empty_environment_file_plays_stock(self) -> None:
        # The documented way to play the deployment genome on a seat whose
        # tree carries a repo list.
        self.repo_list("alpha-one\n")
        self.local_list("gamma-three\n")
        rc, got, _ = self.resolve(CIVVIS_WITH_FILE="/dev/null")
        self.assertEqual(rc, 0)
        self.assertEqual(got["FORCED"], "")
        self.assertEqual(got["ARGS"], "")

    def test_a_multi_line_repo_file_refuses_the_batch(self) -> None:
        self.repo_list("alpha-one\nbeta-two\n")
        rc, _, logs = self.resolve()
        self.assertEqual(rc, 1)
        self.assertTrue(any("whitespace" in line and "refusing batch" in line for line in logs), logs)

    def test_an_explicit_civvis_with_overrides_the_repo_default(self) -> None:
        self.repo_list("alpha-one\n")
        rc, got, logs = self.resolve(CIVVIS_WITH="beta-two")
        self.assertEqual(rc, 0)
        self.assertEqual(got["FORCED"], "beta-two")
        self.assertEqual(got["SOURCE"], "environment")
        self.assertTrue(any("overrides the repo list" in line for line in logs), logs)

    def test_civvis_with_conflicting_with_a_local_override_refuses(self) -> None:
        # Two explicit operator acts that disagree: stop, as before.
        self.local_list("gamma-three\n")
        rc, _, logs = self.resolve(CIVVIS_WITH="beta-two")
        self.assertEqual(rc, 1)
        self.assertTrue(any("conflicts with CIVVIS_WITH" in line for line in logs), logs)

    def test_the_scripts_are_valid_zsh(self) -> None:
        for script in (SUPERVISOR, GAMES):
            done = subprocess.run(["zsh", "-n", str(script)], capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr)


if __name__ == "__main__":
    unittest.main()
