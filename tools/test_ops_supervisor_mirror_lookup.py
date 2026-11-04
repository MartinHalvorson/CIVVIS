"""The supervisor's display-follower lookup (`display_follower_pid`).

Before each launch the loop restarts the CIVVIS mirror unless the follower it
finds already runs from this tree at this revision. The follower normally up
at a boundary is the one the previous climb started (`ensure_mirror`), which
writes `follow.log`; the lookup used to accept only the loop's own
`follow-nohup.log`, found nothing at every boundary, read cwd='absent', and
paid ~9 s of fixed sleeps per game restarting a mirror the climb restarts
anyway. The function is run under zsh with `pgrep` and `lsof` stubbed.
"""

from __future__ import annotations

import shutil
import subprocess
import unittest
from pathlib import Path

SUPERVISOR = Path(__file__).resolve().parent / "ops" / "civvis-game-supervisor.sh"
HOME = "/Users/someone/civvis-civ6-mirror"


def _function() -> str:
    source = SUPERVISOR.read_text(encoding="utf-8")
    start = source.index("display_follower_pid() {\n")
    end = source.index("\n}\n", start) + 3
    return source[start:end]


def _log_names() -> tuple[str, str]:
    source = SUPERVISOR.read_text(encoding="utf-8")
    own = source.split("\nFOLLOW_LOG=$MIRROR_HOME/", 1)[1].split("\n", 1)[0]
    batch = source.split("\nBATCH_FOLLOW_LOG=$MIRROR_HOME/", 1)[1].split("\n", 1)[0]
    return own, batch


@unittest.skipUnless(shutil.which("zsh"), "requires the production zsh shell")
class DisplayFollowerLookupTest(unittest.TestCase):
    def _lookup(self, followers: dict[int, str]) -> str:
        """The pid the lookup returns, given each follow.py pid's stdout path."""
        own, batch = _log_names()
        cases = "\n".join(f"    {pid}) print -r -- 'n{path}' ;;" for pid, path in followers.items())
        script = (
            f"MIRROR_HOME={HOME}\n"
            f"FOLLOW_LOG=$MIRROR_HOME/{own}\n"
            f"BATCH_FOLLOW_LOG=$MIRROR_HOME/{batch}\n"
            f"pgrep() {{ print -r -- '{chr(10).join(str(p) for p in followers)}' }}\n"
            "lsof() {\n  local pid=$3\n  case $pid in\n" + cases + "\n  esac\n}\n"
            + _function()
            + "print -r -- \"pid=$(display_follower_pid || print none)\"\n"
        )
        done = subprocess.run(["zsh", "-c", script], capture_output=True, text=True,
                              check=True)
        return done.stdout.strip()

    def test_the_two_log_names_are_the_climb_and_the_loop_followers(self) -> None:
        self.assertEqual(_log_names(), ("follow-nohup.log", "follow.log"))
        climb = (Path(__file__).resolve().parent / "civ6_civvis_climb.py").read_text(
            encoding="utf-8")
        self.assertIn('MIRROR_FOLLOW_LOG = MIRROR_HOME / "follow.log"', climb)

    def test_the_follower_a_climb_started_is_found(self) -> None:
        self.assertEqual(self._lookup({4242: f"{HOME}/follow.log"}), "pid=4242")

    def test_the_loops_own_follower_is_still_found(self) -> None:
        self.assertEqual(self._lookup({777: f"{HOME}/follow-nohup.log"}), "pid=777")

    def test_an_unrelated_follow_py_is_not_ours(self) -> None:
        # e.g. a spectator or another lane's follower writing elsewhere.
        self.assertEqual(self._lookup({555: "/tmp/other/follow.log"}), "pid=none")
        self.assertEqual(self._lookup({555: "/tmp/other/follow.log",
                                       556: f"{HOME}/follow.log"}), "pid=556")


if __name__ == "__main__":
    unittest.main()
