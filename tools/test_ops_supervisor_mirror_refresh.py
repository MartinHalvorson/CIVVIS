"""The supervisor's display-mirror refresh waits (`# >>> mirror refresh waits`).

The refresh used to sleep a flat 2 + 2 + 5 s wherever the built revision had
changed. Each wait now ends when its condition holds and never runs past the
flat sleep it replaced. The helpers are run under zsh in real time with short
bounds; `display_mirror_server_pid` is stubbed so no port is touched.
"""

from __future__ import annotations

import shutil
import subprocess
import unittest
from pathlib import Path

SUPERVISOR = Path(__file__).resolve().parent / "ops" / "civvis-game-supervisor.sh"


def _helpers() -> str:
    source = SUPERVISOR.read_text(encoding="utf-8")
    start = source.index("# >>> mirror refresh waits\n")
    end = source.index("# <<< mirror refresh waits\n")
    return source[start:end]


def _refresh_block() -> str:
    source = SUPERVISOR.read_text(encoding="utf-8")
    start = source.index("refreshing exact mirror owners")
    end = source.index("verification_intent_running", start)
    return source[start:end]


@unittest.skipUnless(shutil.which("zsh"), "requires the production zsh shell")
class MirrorRefreshWaitsTest(unittest.TestCase):
    def _elapsed(self, body: str) -> tuple[float, str]:
        script = ("zmodload zsh/datetime\nlocal -F t0=$EPOCHREALTIME\n" + _helpers()
                  + body + "\nprint \"elapsed $(( EPOCHREALTIME - t0 ))\"\n")
        done = subprocess.run(["zsh", "-c", script], capture_output=True, text=True,
                              check=True, timeout=30)
        lines = done.stdout.strip().splitlines()
        return float(lines[-1].split()[1]), "\n".join(lines[:-1])

    def test_a_process_that_is_already_gone_costs_no_wait(self) -> None:
        elapsed, _ = self._elapsed("sleep 0 & gone=$!; wait $gone\nwait_for_pid_exit $gone 2")
        self.assertLess(elapsed, 0.5)

    def test_a_process_that_exits_on_term_ends_the_wait(self) -> None:
        elapsed, _ = self._elapsed(
            "sleep 30 & pid=$!\nkill -TERM $pid\nwait_for_pid_exit $pid 2\n"
            "kill -0 $pid 2>/dev/null && print still-running || print exited")
        self.assertLess(elapsed, 1.5)

    def test_a_process_that_ignores_term_is_waited_on_only_up_to_the_bound(self) -> None:
        elapsed, _ = self._elapsed(
            "sleep 30 & pid=$!\nwait_for_pid_exit $pid 0.6\nkill $pid 2>/dev/null")
        self.assertGreaterEqual(elapsed, 0.6)
        self.assertLess(elapsed, 2.0)

    def test_the_listener_wait_ends_when_the_server_listens(self) -> None:
        elapsed, out = self._elapsed(
            "calls=0\n"
            "display_mirror_server_pid() { calls=$((calls + 1)); (( calls >= 3 )) && print 4242 }\n"
            "wait_for_mirror_listener 5 && print listening calls=$calls")
        self.assertIn("listening calls=3", out)
        self.assertLess(elapsed, 2.0)

    def test_no_listener_returns_failure_at_the_bound(self) -> None:
        elapsed, out = self._elapsed(
            "display_mirror_server_pid() { return 1 }\n"
            "wait_for_mirror_listener 0.6 || print gave-up")
        self.assertIn("gave-up", out)
        self.assertGreaterEqual(elapsed, 0.6)
        self.assertLess(elapsed, 2.0)

    def test_the_refresh_uses_the_bounded_waits_with_the_old_ceilings(self) -> None:
        block = _refresh_block()
        self.assertIn('wait_for_pid_exit "$FOLLOW_PID" 2', block)
        self.assertIn('wait_for_pid_exit "$MIRROR_PID" 2', block)
        self.assertIn("wait_for_mirror_listener 5 || true", block)
        for flat in ("\n      sleep 2\n", "\n    sleep 5\n"):
            self.assertNotIn(flat, block)


if __name__ == "__main__":
    unittest.main()
