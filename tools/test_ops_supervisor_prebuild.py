"""The supervisor's next-head prebuild (`# >>> next-head prebuild` block).

While a game plays, a background loop builds any newer origin/main tip in a
sibling worktree and stamps it with the revision and both binaries' SHA-256.
At the boundary the supervisor adopts that build for the exact head it checked
out, or falls through to its ordinary build. These tests run the production
blocks under zsh against a throwaway Git origin and a stub `cargo` that writes
binaries naming the revision it compiled.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

SUPERVISOR = Path(__file__).resolve().parent / "ops" / "civvis-game-supervisor.sh"


def _between(start: str, end: str) -> str:
    source = SUPERVISOR.read_text(encoding="utf-8")
    first = source.index(start)
    return source[first:source.index(end, first)]


def _prebuild_block() -> str:
    return (_between("# >>> mirror refresh waits\n", "# <<< mirror refresh waits\n")
            + _between("# >>> next-head prebuild\n", "# <<< next-head prebuild\n"))


STUB_CARGO = textwrap.dedent("""\
    #!/bin/sh
    # Stand-in for `cargo build --release --bin civvis_orders --bin civvis`.
    echo "$*" >> "$CARGO_CALLS"
    rev=$(git rev-parse HEAD)
    mkdir -p target/release
    printf 'orders %s\\n' "$rev" > target/release/civvis_orders
    printf 'mirror %s\\n' "$rev" > target/release/civvis
    exit "${CARGO_EXIT:-0}"
    """)


def _git(cwd: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(cwd), *args], capture_output=True, text=True,
                          check=True).stdout.strip()


@unittest.skipUnless(shutil.which("zsh") and shutil.which("shasum"),
                     "requires the production zsh shell and shasum")
class NextHeadPrebuildTest(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.origin = self.root / "origin.git"
        self.repo = self.root / "repo"
        self.tree = self.root / "prebuild"
        self.logs = self.root / "logs"
        self.logs.mkdir()
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        cargo = bin_dir / "cargo"
        cargo.write_text(STUB_CARGO, encoding="utf-8")
        cargo.chmod(0o755)
        self.calls = self.root / "cargo-calls"
        self.env = {**os.environ,
                    "PATH": f"{bin_dir}:{os.environ['PATH']}",
                    "CARGO_CALLS": str(self.calls),
                    "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t",
                    "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}
        subprocess.run(["git", "init", "--quiet", "--bare", "-b", "main", str(self.origin)],
                       check=True, env=self.env)
        subprocess.run(["git", "clone", "--quiet", str(self.origin), str(self.repo)],
                       check=True, env=self.env, capture_output=True)
        (self.repo / ".gitignore").write_text("target/\n", encoding="utf-8")
        self.first = self._commit("first")

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def _commit(self, text: str) -> str:
        (self.repo / "file.txt").write_text(text, encoding="utf-8")
        subprocess.run(["git", "-C", str(self.repo), "add", "-A"], check=True, env=self.env)
        subprocess.run(["git", "-C", str(self.repo), "commit", "--quiet", "-m", text],
                       check=True, env=self.env)
        subprocess.run(["git", "-C", str(self.repo), "push", "--quiet", "origin", "HEAD:main"],
                       check=True, env=self.env, capture_output=True)
        return _git(self.repo, "rev-parse", "HEAD")

    def _zsh(self, body: str) -> subprocess.CompletedProcess[str]:
        script = "\n".join([
            "set -u",
            f"LOGS={self.logs}",
            f"REPO={self.repo}",
            f"SUP={self.logs}/supervisor.log",
            'say() { print -r -- "say: $*" }',
            f"CIVVIS_PREBUILD_TREE={self.tree}",
            _prebuild_block(),
            body,
        ])
        return subprocess.run(["zsh", "-c", script], capture_output=True, text=True,
                              env=self.env, cwd=self.root)

    def _cargo_calls(self) -> list[str]:
        if not self.calls.exists():
            return []
        return self.calls.read_text(encoding="utf-8").splitlines()

    def _make_tree(self) -> None:
        subprocess.run(["git", "-C", str(self.repo), "worktree", "add", "--quiet", "--detach",
                        str(self.tree), self.first], check=True, env=self.env)

    # -- stamps ---------------------------------------------------------------

    def test_a_stamp_vouches_only_for_its_revision_and_bytes(self) -> None:
        release = self.root / "release"
        release.mkdir()
        (release / "civvis_orders").write_text("a", encoding="utf-8")
        (release / "civvis").write_text("b", encoding="utf-8")
        done = self._zsh(textwrap.dedent(f"""\
            write_build_stamp {release} {'1' * 40} || exit 9
            build_stamp_matches {release} {'1' * 40} && print same
            build_stamp_matches {release} {'2' * 40} || print other-revision
            print -n changed > {release}/civvis_orders
            build_stamp_matches {release} {'1' * 40} || print other-bytes
            rm {release}/civvis
            build_stamp_matches {release} {'1' * 40} || print missing
            """))
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(done.stdout.split(), ["same", "other-revision", "other-bytes", "missing"])

    # -- the background pass ------------------------------------------------------

    def test_a_newer_tip_is_built_and_stamped_without_moving_origin_main(self) -> None:
        self._make_tree()
        tracking = _git(self.repo, "rev-parse", "refs/remotes/origin/main")
        newer = self._commit("second")
        # The repo's own tracking ref was moved by the push above; put it back
        # to where a supervisor that has not fetched yet would see it.
        _git(self.repo, "update-ref", "refs/remotes/origin/main", tracking)
        done = self._zsh(f"prebuild_once {self.first} && print built")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("built", done.stdout)
        self.assertEqual(_git(self.tree, "rev-parse", "HEAD"), newer)
        stamp = (self.tree / "target/release/.civvis-built-revision").read_text().split()
        self.assertEqual(stamp[0], newer)
        self.assertEqual((self.tree / "target/release/civvis_orders").read_text(),
                         f"orders {newer}\n")
        self.assertEqual(_git(self.repo, "rev-parse", "refs/remotes/origin/main"), tracking,
                         "the prebuild must not move the shared origin/main ref")
        self.assertFalse((self.tree / "target/.civvis-prebuilding").exists())
        self.assertEqual(self._cargo_calls(),
                         ["build --release --bin civvis_orders --bin civvis"])

    def test_the_revision_in_play_and_an_already_built_tip_are_not_rebuilt(self) -> None:
        self._make_tree()
        done = self._zsh(f"prebuild_once {self.first} && print skipped")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(self._cargo_calls(), [])
        self._commit("second")
        done = self._zsh(f"prebuild_once {self.first}; prebuild_once {self.first}")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(len(self._cargo_calls()), 1, "a stamped tip builds once")

    def test_a_failed_build_leaves_no_stamp(self) -> None:
        self._make_tree()
        self._commit("second")
        self.env["CARGO_EXIT"] = "101"
        done = self._zsh(f"prebuild_once {self.first} || print failed")
        self.assertIn("failed", done.stdout)
        self.assertFalse((self.tree / "target/release/.civvis-built-revision").exists())
        self.assertIn("FAILED", (self.logs / "prebuild.log").read_text())

    def test_a_dirty_prebuild_tree_is_left_alone(self) -> None:
        self._make_tree()
        self._commit("second")
        (self.tree / "file.txt").write_text("someone's edit", encoding="utf-8")
        done = self._zsh(f"prebuild_once {self.first} || print refused")
        self.assertIn("refused", done.stdout)
        self.assertEqual(self._cargo_calls(), [])
        self.assertEqual((self.tree / "file.txt").read_text(), "someone's edit")

    def test_the_loop_creates_its_worktree_and_stops_on_term(self) -> None:
        newer = self._commit("second")
        done = self._zsh(textwrap.dedent(f"""\
            CIVVIS_PREBUILD_POLL_S=30
            PREBUILD_POLL_S=30
            prebuild_start {self.first}
            for _ in {{1..100}}; do
              [[ -f {self.tree}/target/release/.civvis-built-revision ]] && break
              sleep 0.1
            done
            loop=$PREBUILD_PID
            prebuild_stop
            kill -0 $loop 2>/dev/null && print alive || print stopped
            """))
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(done.stdout.split(), ["stopped"])
        self.assertEqual(_git(self.tree, "rev-parse", "HEAD"), newer)
        self.assertEqual(
            (self.tree / "target/release/.civvis-built-revision").read_text().split()[0], newer)

    # -- adoption at the boundary -----------------------------------------------

    def test_a_stamped_prebuild_of_the_head_is_adopted_byte_for_byte(self) -> None:
        self._make_tree()
        newer = self._commit("second")
        dest = self.repo / "target/release"
        dest.mkdir(parents=True)
        (dest / "civvis_orders").write_text("old", encoding="utf-8")
        done = self._zsh(f"prebuild_once {self.first}; adopt_prebuilt {newer} {dest} && print adopted")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("adopted", done.stdout)
        for name in ("civvis_orders", "civvis", ".civvis-built-revision"):
            self.assertEqual((dest / name).read_bytes(),
                             (self.tree / "target/release" / name).read_bytes(), name)
        self.assertEqual(list(dest.glob(".*.tmp")), [])

    def test_a_prebuild_of_another_revision_is_not_adopted(self) -> None:
        self._make_tree()
        self._commit("second")
        dest = self.repo / "target/release"
        dest.mkdir(parents=True)
        (dest / "civvis_orders").write_text("old", encoding="utf-8")
        done = self._zsh(f"prebuild_once {self.first}; adopt_prebuilt {'9' * 40} {dest} || print declined")
        self.assertIn("declined", done.stdout)
        self.assertEqual((dest / "civvis_orders").read_text(), "old")
        self.assertFalse((dest / ".civvis-built-revision").exists())

    # -- the boundary build ------------------------------------------------------

    def _head_build(self, pin: str, stamp: bool, adopt: bool, cargo_ok: bool = True) -> list[str]:
        block = _between("  # >>> head build\n", "  # <<< head build\n")
        script = "\n".join([
            "set -u",
            "exec 3>&1",
            f"PIN={pin}", f"HEAD_REVISION={'a' * 40}", f"HEAD_SHA={'a' * 7}", "REPO=/r", "SUP=/dev/null",
            'say() { print -r -- "say $*" }',
            "sleep() { print \"sleep $1\" }",
            f"build_stamp_matches() {{ {'return 0' if stamp else 'return 1'} }}",
            f"adopt_prebuilt() {{ print adopt; {'return 0' if adopt else 'return 1'} }}",
            "write_build_stamp() { print stamp-written; return 0 }",
            "prebuild_stop() { print prebuild-stopped }",
            f"cargo() {{ print -u3 cargo; {'return 0' if cargo_ok else 'return 1'} }}",
            "for _ in 1; do",
            block,
            'print "args ${CLIMB_BUILD_ARGS[*]:-none}"',
            "done",
        ])
        done = subprocess.run(["zsh", "-c", script], capture_output=True, text=True, check=True)
        return done.stdout.splitlines()

    def test_head_binaries_already_vouched_for_skip_cargo(self) -> None:
        self.assertEqual(self._head_build("head", stamp=True, adopt=False),
                         ["args --no-build"])

    def test_an_adopted_prebuild_skips_the_boundary_build(self) -> None:
        out = self._head_build("head", stamp=False, adopt=True)
        self.assertNotIn("cargo", out)
        self.assertEqual(out[-1], "args --no-build")

    def test_without_a_prebuild_the_head_is_built_and_stamped(self) -> None:
        self.assertEqual(self._head_build("head", stamp=False, adopt=False),
                         ["adopt", "prebuild-stopped", "cargo", "stamp-written", "args --no-build"])

    def test_a_pinned_tree_builds_as_before_and_the_climb_still_builds(self) -> None:
        self.assertEqual(self._head_build("/some/tree", stamp=True, adopt=True),
                         ["prebuild-stopped", "cargo", "args none"])

    def test_a_failed_build_waits_and_retries_the_cycle(self) -> None:
        out = self._head_build("head", stamp=False, adopt=False, cargo_ok=False)
        self.assertIn("sleep 120", out)
        self.assertNotIn("stamp-written", out)

    def test_the_loop_is_wired_into_the_cycle(self) -> None:
        source = SUPERVISOR.read_text(encoding="utf-8")
        climb = source.index("python3 -u tools/civ6_civvis_climb.py")
        self.assertLess(source.index('prebuild_start "$HEAD_REVISION"'), climb)
        self.assertIn('"${CLIMB_BUILD_ARGS[@]}"', source[climb:climb + 2000])
        self.assertIn("prebuild_stop", source[source.index("trap "):source.index("trap ") + 200])
        self.assertEqual(source.count("cargo build --release --bin civvis_orders --bin civvis"), 2,
                         "the boundary build and the prebuild run the same command")


if __name__ == "__main__":
    unittest.main()
