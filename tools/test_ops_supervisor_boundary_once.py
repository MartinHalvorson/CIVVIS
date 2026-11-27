"""Work the supervisor's game boundary does once, not twice.

Two steps of `tools/ops/civvis-game-supervisor.sh` repeated work at nearly
every boundary between live games:

* `# >>> head recheck`: the second `git fetch origin main`, which exists for a
  boundary that spent minutes compiling, also ran when nothing was built and
  the first fetch was a second old.
* the display refresh: the visual climb retires the follower and its server
  and starts its own (`civ6_civvis_climb.ensure_mirror`), so a refresh here
  was undone a few seconds later.

The blocks run under zsh with `git`, `kill`, `nohup` and the mirror lookups
stubbed, so no network, process or port is touched.
"""

from __future__ import annotations

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SUPERVISOR = Path(__file__).resolve().parent / "ops" / "civvis-game-supervisor.sh"
HEAD = "a" * 40
NEWER = "b" * 40


def _between(start: str, end: str) -> str:
    source = SUPERVISOR.read_text(encoding="utf-8")
    first = source.index(start)
    return source[first:source.index(end, first)]


@unittest.skipUnless(shutil.which("zsh"), "requires the production zsh shell")
class HeadRecheckTest(unittest.TestCase):
    """The post-build fetch runs only when the boundary spent time since its
    own fetch, which is what a build or a wait on a prebuild costs."""

    def _recheck(self, *, since_fetch: int, pin: str = "head", fetch_ok: bool = True,
                 origin: str = HEAD, fetched: bool = True) -> list[str]:
        block = _between("  # >>> head recheck\n", "  # <<< head recheck\n")
        fetch_status = 0 if fetch_ok else 1
        script = "\n".join([
            "set -u",
            "exec 3>&1",
            f"PIN={pin}", f"HEAD_REVISION={HEAD}", "SUP=/dev/null",
            "HEAD_RECHECK_AFTER_S=15",
            'say() { print -r -- "say $*" }',
            'sleep() { print -r -- "sleep $1" }',
            # Every `git fetch` is reported on fd 3; `rev-parse` answers origin.
            "git() {",
            '  case "$*" in',
            f"    *fetch*) print -u3 fetch; return {fetch_status} ;;",
            f"    *rev-parse*) print -r -- {origin} ;;",
            "  esac",
            "}",
            f"HEAD_FETCHED_AT=$(( SECONDS - {since_fetch} ))" if fetched else "",
            "for _ in 1; do",
            block,
            "print launch",
            "done",
        ])
        done = subprocess.run(["zsh", "-c", script], capture_output=True, text=True,
                              check=True, timeout=30)
        return done.stdout.splitlines()

    def test_a_boundary_that_built_nothing_launches_without_a_second_fetch(self) -> None:
        self.assertEqual(self._recheck(since_fetch=1), ["launch"])

    def test_a_boundary_build_is_rechecked_and_launches_an_unmoved_head(self) -> None:
        self.assertEqual(self._recheck(since_fetch=90), ["fetch", "launch"])

    def test_a_merge_during_the_build_rebuilds_instead_of_launching(self) -> None:
        out = self._recheck(since_fetch=90, origin=NEWER)
        self.assertEqual(out[0], "fetch")
        self.assertIn("during fresh build; rebuilding exact head before launch", out[1])
        self.assertNotIn("launch", out)

    def test_a_recheck_that_cannot_reach_github_launches_the_verified_head(self) -> None:
        out = self._recheck(since_fetch=90, fetch_ok=False)
        self.assertEqual(out[0], "fetch")
        self.assertIn("could not recheck origin/main after fresh build", out[1])
        self.assertEqual(out[-1], "launch")

    def test_the_threshold_itself_rechecks(self) -> None:
        self.assertEqual(self._recheck(since_fetch=15), ["fetch", "launch"])

    def test_a_pinned_tree_never_fetches(self) -> None:
        self.assertEqual(self._recheck(since_fetch=90, pin="/some/tree"), ["launch"])

    def test_an_unknown_fetch_moment_rechecks(self) -> None:
        # The safe side is the fetch the boundary always used to make.
        self.assertEqual(self._recheck(since_fetch=0, fetched=False), ["fetch", "launch"])

    def test_the_fetch_moment_is_taken_right_after_the_cycle_fetch(self) -> None:
        source = SUPERVISOR.read_text(encoding="utf-8")
        loop = source[source.index("rm -f status.json"):]
        fetch = loop.index("if ! git -c gc.auto=0 fetch --quiet origin main")
        moment = loop.index("HEAD_FETCHED_AT=$SECONDS")
        checkout = loop.index("if ! git checkout --quiet --detach origin/main")
        self.assertLess(fetch, moment)
        self.assertLess(moment, checkout)
        self.assertIn("HEAD_RECHECK_AFTER_S=${CIVVIS_HEAD_RECHECK_AFTER_S:-15}", source)
        # The recheck still follows the boundary build it exists for.
        self.assertLess(source.index("  # <<< head build\n"),
                        source.index("  # >>> head recheck\n"))


@unittest.skipUnless(shutil.which("zsh"), "requires the production zsh shell")
class DisplayRefreshOwnerTest(unittest.TestCase):
    """The supervisor refreshes the display mirror only for the capture-free
    lane; the visual climb's own `ensure_mirror` replaces it either way."""

    def _refresh(self, *, capture_free: int, revision_on_disk: str) -> tuple[list[str], str]:
        source = SUPERVISOR.read_text(encoding="utf-8")
        start = source.index("  FOLLOW_PID=$(display_follower_pid || true)\n")
        end = source.index("  # The ownership scan above runs before", start)
        block = source[start:end]
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            revision_file = root / "follower-runtime-revision"
            revision_file.write_text(revision_on_disk + "\n", encoding="utf-8")
            script = "\n".join([
                "set -u",
                "exec 3>&1",
                f"CAPTURE_FREE={capture_free}",
                f"REPO={root}", f"HEAD_SHA={HEAD[:7]}", f"HEAD_REVISION={HEAD}",
                "HEAD_COMMIT_TIME=2026-11-27T00:00:00Z",
                f"MIRROR_HOME={root}", f"FOLLOW_LOG={root}/follow-nohup.log",
                f"FOLLOW_REVISION_FILE={revision_file}",
                'say() { print -r -- "say $*" }',
                "display_follower_pid() { print 4242 }",
                "display_mirror_server_pid() { print 4343 }",
                f"lsof() {{ print -r -- n{root} }}",
                'kill() { print -u3 "kill $*"; return 0 }',
                "wait_for_pid_exit() { return 0 }",
                "wait_for_mirror_listener() { print -u3 listening; return 0 }",
                "nohup() { return 0 }",
                block,
                "print done",
            ])
            done = subprocess.run(["zsh", "-c", script], capture_output=True, text=True,
                                  check=True, timeout=30)
            return done.stdout.splitlines(), revision_file.read_text(encoding="utf-8").strip()

    def test_the_visual_lane_leaves_the_refresh_to_the_climb(self) -> None:
        out, revision = self._refresh(capture_free=0, revision_on_disk="0000000")
        self.assertEqual(out, ["done"])
        self.assertEqual(revision, "0000000")

    def test_the_capture_free_lane_still_refreshes_a_changed_revision(self) -> None:
        out, revision = self._refresh(capture_free=1, revision_on_disk="0000000")
        self.assertTrue(any("refreshing exact mirror owners" in line for line in out), out)
        self.assertIn("kill -TERM 4242", out)
        self.assertIn("listening", out)
        self.assertEqual(revision, HEAD[:7])

    def test_the_capture_free_lane_keeps_a_current_mirror(self) -> None:
        out, revision = self._refresh(capture_free=1, revision_on_disk=HEAD[:7])
        self.assertEqual(out, ["done"])
        self.assertEqual(revision, HEAD[:7])

    def test_the_visual_climb_still_replaces_the_mirror_before_its_first_attempt(self) -> None:
        climb = (SUPERVISOR.parent.parent / "civ6_civvis_climb.py").read_text(encoding="utf-8")
        main = climb[climb.index("def main() -> int:"):]
        ensure = main.index("    ensure_mirror()\n")
        self.assertLess(ensure, main.index("    while played < args.attempts:\n"))
        body = climb[climb.index("def ensure_mirror() -> None:"):
                     climb.index("def commits_behind_main()")]
        self.assertIn("retire_mirror()", body)
        self.assertIn("mirror_follower_environment()", body)


if __name__ == "__main__":
    unittest.main()
