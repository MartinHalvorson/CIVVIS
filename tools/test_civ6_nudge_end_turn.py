#!/usr/bin/env python3
"""The last-resort forced end turn, and the rules that keep it safe."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_nudge_end_turn as nudge  # noqa: E402
from civ6_control import macos_input  # noqa: E402

OPS = Path(__file__).resolve().parent / "ops" / "civvis-agent-wedge-watchdog.sh"


class NudgeTest(unittest.TestCase):
    def test_it_sends_the_forced_end_turn_and_nothing_else(self) -> None:
        """SHIFT+RETURN is the one end-turn form the engine does not refuse
        while a blocker stands. ⚠ Escape is not an alternative: with nothing to
        close it opens the pause menu, which stops the game advancing."""
        with mock.patch.object(macos_input, "press_key") as press:
            press.return_value = mock.Mock(returncode=0)
            self.assertTrue(nudge.nudge(presses=2, interval_s=0))
        self.assertEqual(press.call_count, 2)
        for call in press.call_args_list:
            self.assertEqual(call.args[0], "return")
            self.assertEqual(call.kwargs["modifier"], "shift")

    def test_a_backend_that_cannot_send_is_reported_not_raised(self) -> None:
        """The watchdog is about to kill the game; a failure here must not
        become an exception that skips the restart."""
        with mock.patch.object(macos_input, "press_key",
                               side_effect=OSError("no input backend")):
            self.assertFalse(nudge.nudge(presses=2, interval_s=0))

    def test_a_refused_press_is_not_reported_as_sent(self) -> None:
        with mock.patch.object(macos_input, "press_key") as press:
            press.return_value = mock.Mock(returncode=1)
            self.assertFalse(nudge.nudge(presses=1, interval_s=0))


class OnlyTheWatchdogMayCallIt(unittest.TestCase):
    """⚠⚠ A forced end turn on a HEALTHY game would be a real hazard.

    The safety of this tool is entirely in where it is called: after the
    watchdog's no-progress test has condemned the run and immediately before
    the kill, where the game is lost either way.
    """

    def test_the_watchdog_runs_it_before_the_kill_and_never_after(self) -> None:
        script = OPS.read_text(encoding="utf-8")
        self.assertIn("civ6_nudge_end_turn.py", script)
        nudge_at = script.index("civ6_nudge_end_turn.py")
        kill_at = script.index('kill -TERM "$climb"')
        self.assertLess(nudge_at, kill_at,
                        "the nudge must precede the kill; afterwards there is "
                        "nothing left to nudge")

    def test_its_failure_never_blocks_the_restart(self) -> None:
        script = OPS.read_text(encoding="utf-8")
        block = script[script.index("civ6_nudge_end_turn.py"):]
        block = block[:block.index('kill -TERM "$climb"')]
        # No early return, no exit: the restart follows whatever happened.
        self.assertNotIn("return 1", block)
        self.assertNotIn("exit ", block)

    def test_nothing_else_in_the_tree_sends_a_forced_end_turn(self) -> None:
        """Discovered, not listed: a second caller fails here rather than
        quietly gaining the power to end a live turn."""
        root = Path(__file__).resolve().parent
        callers = []
        # Tests are excluded: asserting the mechanism is not the same as being
        # able to end a live turn, and `test_macos_input.py` legitimately names
        # the modifier while pinning the backend argument shapes.
        for path in list(root.glob("*.py")) + list(root.glob("ops/*.sh")):
            if path.name.startswith("test_") or path.name == "civ6_nudge_end_turn.py":
                continue
            text = path.read_text(encoding="utf-8", errors="ignore")
            if "civ6_nudge_end_turn" in text or 'modifier="shift"' in text:
                callers.append(path.name)
        self.assertEqual(callers, ["civvis-agent-wedge-watchdog.sh"], callers)


if __name__ == "__main__":
    unittest.main()
