#!/usr/bin/env python3
"""A refusal repeated for an hour is an outage, and it must say so."""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

OPS = Path(__file__).resolve().parent / "ops"
SUPERVISOR = OPS / "civvis-game-supervisor.sh"


class TheLaneSaysWhenItIsDown(unittest.TestCase):
    """★★★★★ THE CONDITION WAS DETECTED AND NEVER ESCALATED.

    2026-09-03: another agent left three uncommitted files in the head tree.
    The supervisor refused to run an unprovable head batch -- correctly -- every
    120 s from 22:38Z. At 23:00Z it was still refusing: ~45 identical lines,
    zero games, and nothing anywhere saying the lane was down. Exactly the blind
    spot the wedge watchdog had for an unowned run (#3145).
    """

    def _source(self) -> str:
        return SUPERVISOR.read_text(encoding="utf-8")

    def test_the_escalation_cadence_is_a_named_knob(self) -> None:
        self.assertIn(
            "BLOCKED_ESCALATE_EVERY=${CIVVIS_SUPERVISOR_ESCALATE_EVERY:-5}",
            self._source())

    def test_a_launch_clears_the_streak(self) -> None:
        """Without this the counter would escalate forever after one bad patch."""
        source = self._source()
        launch = source.index('say "starting $ATTEMPTS attempt(s)')
        window = source[launch - 400:launch]
        self.assertIn("LAUNCH_REACHED=1", window,
                      "reaching a launch must reset the stall counter")

    def _drive(self, outcomes: list[bool]) -> list[str]:
        """Replay the loop-top counter; each entry is whether a launch had
        happened when that pass began."""
        if shutil.which("zsh") is None:
            self.skipTest("zsh is needed here")
        source = self._source()
        start = source.index("BLOCKED_ESCALATE_EVERY=")
        end = source.index("  LAUNCH_REACHED=0\n") + len("  LAUNCH_REACHED=0\n")
        block = source[start:end]
        # The block is the preamble plus the body of `while true; do`. Replay it
        # over a fixed list of passes instead of looping forever.
        preamble, _, body = block.partition("while true; do\n")
        reached = " ".join("1" if flag else "0" for flag in outcomes)
        # `pass` is set BEFORE the block, so each entry is exactly "had a launch
        # happened when this pass began" -- the state the block reads. Setting it
        # after would make every expectation off by one, which is how the first
        # version of this test lied to me.
        script = (
            "say() { print -r -- \"$*\" }\n"
            + preamble
            + f"for pass in {reached}; do\n"
            + "  LAUNCH_REACHED=$pass\n"
            + body
            + "done\n")
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "drive.sh"
            path.write_text(script)
            done = subprocess.run(["zsh", str(path)], capture_output=True,
                                  text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr)
        return [line for line in done.stdout.splitlines() if line.strip()]

    def test_it_stays_quiet_below_the_cadence(self) -> None:
        """Four refusals is eight minutes. That is not yet news."""
        lines = self._drive([False] * 4)
        self.assertEqual(lines, [], f"unexpected output: {lines}")

    def test_it_escalates_on_the_fifth_consecutive_refusal(self) -> None:
        lines = self._drive([False] * 5)
        self.assertEqual(len(lines), 1, lines)
        self.assertIn("LANE STALLED", lines[0])
        self.assertIn("5 consecutive refusals", lines[0])

    def test_it_keeps_escalating_while_the_lane_stays_down(self) -> None:
        lines = self._drive([False] * 10)
        self.assertEqual(len(lines), 2, lines)
        self.assertIn("10 consecutive refusals", lines[1])

    def test_a_launch_between_refusals_resets_it(self) -> None:
        """Four refusals, a launch, four more: never five in a row, so silent."""
        lines = self._drive([False] * 4 + [True] + [False] * 4)
        self.assertEqual(lines, [], f"unexpected output: {lines}")

    def test_the_first_pass_is_not_a_refusal(self) -> None:
        """Nothing has had a chance to launch yet when the loop first runs."""
        source = self._source()
        self.assertIn("LAUNCH_REACHED=1\n\nwhile true; do", source)

    def test_the_script_is_valid_zsh(self) -> None:
        done = subprocess.run(["zsh", "-n", str(SUPERVISOR)],
                              capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stderr)


if __name__ == "__main__":
    unittest.main()
