#!/usr/bin/env python3
"""The last-resort forced end turn, and the rules that keep it safe."""

from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_nudge_end_turn as nudge  # noqa: E402
from civ6_control import macos_input  # noqa: E402

OPS = Path(__file__).resolve().parent / "ops" / "civvis-agent-wedge-watchdog.sh"


class NudgeTest(unittest.TestCase):
    def setUp(self) -> None:
        """⚠⚠ Stub the raise for every test that is not about the raise.

        Without this, `nudge()` runs the REAL `focus_game`, which osascripts a
        live Civilization VI to the front — a unit test with a side effect on
        the running game. It also fails on CI, where there is no `osascript`:
        the raise returns False, `nudge` correctly sends nothing, and the
        assertion reads `False is not true` with no hint that focus is why.
        """
        patcher = mock.patch.object(nudge, "focus_game", return_value=True)
        self.focus = patcher.start()
        self.addCleanup(patcher.stop)

    def test_it_sends_the_forced_end_turn_and_nothing_else(self) -> None:
        """SHIFT+RETURN is the one end-turn form the engine does not refuse
        while a blocker stands. ⚠ Escape is not an alternative: with nothing to
        close it opens the pause menu, which stops the game advancing."""
        with mock.patch.object(macos_input, "press_key") as press:
            press.return_value = mock.Mock(returncode=0)
            self.assertTrue(nudge.nudge(presses=2, interval_s=0, focus_settle_s=0))
        self.assertEqual(press.call_count, 2)
        for call in press.call_args_list:
            self.assertEqual(call.args[0], "return")
            self.assertEqual(call.kwargs["modifier"], "shift")

    def test_the_game_is_raised_before_any_key_is_sent(self) -> None:
        """⚠⚠⚠ Without this the key goes to whatever is FRONTMOST.

        `cliclick` exits zero whoever receives the keystroke, so the harness
        reports "sent" either way. The first live firing (2026-08-30T04:39)
        logged `sent SHIFT+RETURN to a parked game` then `the forced end turn
        changed nothing` — which is exactly what an unfocused keystroke also
        looks like, so that result proved nothing until the raise existed.
        """
        calls = []
        with mock.patch.object(macos_input, "press_key") as press:
            press.side_effect = lambda *a, **k: calls.append("key") or mock.Mock(returncode=0)
            nudge.nudge(presses=1, interval_s=0, focus_settle_s=0)
        self.focus.assert_called_once()
        self.assertEqual(calls, ["key"])

    def test_a_game_that_cannot_be_raised_gets_no_keystroke_at_all(self) -> None:
        """Better to send nothing than to send a forced end turn somewhere else."""
        self.focus.return_value = False
        with mock.patch.object(macos_input, "press_key") as press:
            self.assertFalse(nudge.nudge(presses=2, interval_s=0, focus_settle_s=0))
        press.assert_not_called()

    def test_the_raise_does_not_move_or_resize_the_window(self) -> None:
        """`civ6_play` records that re-placing on every focus pass resized the
        window between a menu read and its click and cost a whole run."""
        source = Path(nudge.__file__).read_text(encoding="utf-8")
        body = source.split("def focus_game(", 1)[1].split("def nudge(", 1)[0]
        self.assertIn("set frontmost", body)
        for forbidden in ("set position", "set size", "AXPosition", "AXSize"):
            self.assertNotIn(forbidden, body)

    def test_no_test_here_touches_the_live_game(self) -> None:
        """⚠ These tests ran the REAL raise before `setUp` stubbed it, which
        osascripted a live Civilization VI to the front — a unit test with a
        side effect on the running ladder — and failed on CI, where there is no
        `osascript`, as a bare `False is not true`."""
        source = Path(__file__).read_text(encoding="utf-8")
        # Every nudge call passes a zero settle, so no test sleeps on the raise.
        for call in re.findall(r"nudge\.nudge\([^)]*\)", source):
            self.assertIn("focus_settle_s=0", call, call)

    def test_the_process_name_is_not_a_second_copy(self) -> None:
        """`civ6_play` takes it from `popup_clear` too; a duplicate would drift."""
        from civ6_control import popup_clear
        self.assertEqual(nudge.GAME_PROCESS, popup_clear.GAME_PROCESS)

    def test_a_backend_that_cannot_send_is_reported_not_raised(self) -> None:
        """The watchdog is about to kill the game; a failure here must not
        become an exception that skips the restart."""
        with mock.patch.object(macos_input, "press_key",
                               side_effect=OSError("no input backend")):
            self.assertFalse(nudge.nudge(presses=2, interval_s=0, focus_settle_s=0))

    def test_a_refused_press_is_not_reported_as_sent(self) -> None:
        with mock.patch.object(macos_input, "press_key") as press:
            press.return_value = mock.Mock(returncode=1)
            self.assertFalse(nudge.nudge(presses=1, interval_s=0, focus_settle_s=0))


class TheWatchdogPicksUpItsOwnRepairs(unittest.TestCase):
    """⚠⚠ This process outlives its own source, and that has cost real fixes.

    zsh parses a function once, so a long-running loop keeps whatever the file
    said when it started. The supervisor refreshes the mirror every cycle, so a
    merged repair lands on disk while the running watchdog goes on executing the
    old one — the ladder looks patched when it is not. It has happened to the
    mirror, the popup clearer, and this watchdog twice; on 2026-08-30 it delayed
    the parked-core repair across three consecutive wedges.
    """

    def test_it_notices_its_own_script_changing(self) -> None:
        script = OPS.read_text(encoding="utf-8")
        self.assertIn("SELF_STAMP", script)
        self.assertIn('exec /bin/zsh "$SELF_PATH"', script)
        # Compared inside the poll loop, not only at startup.
        loop = script[script.index("while true; do"):]
        self.assertIn("SELF_STAMP", loop)

    def test_it_releases_the_lock_before_handing_over(self) -> None:
        """⚠ `exec` keeps the pid and skips the EXIT trap, so the new image
        would find the lock held by a live pid — its own — and exit 0."""
        script = OPS.read_text(encoding="utf-8")
        arm = script[script.index("own script changed on disk"):]
        arm = arm[:arm.index('exec /bin/zsh "$SELF_PATH"')]
        self.assertIn('rm -rf -- "$LOCK"', arm)

    def test_the_stamp_is_cheap_and_not_a_torn_hash(self) -> None:
        """mtime+size from one stat: a rewritten script always changes one, and
        hashing a file mid-refresh could read it torn."""
        script = OPS.read_text(encoding="utf-8")
        self.assertIn("stat -f '%m %z'", script)
        for hashing in ("shasum", "md5", "cksum"):
            self.assertNotIn(hashing, script)


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

    def test_the_game_is_given_time_to_act_before_it_is_killed(self) -> None:
        """⚠⚠ Without a wait this is a gesture, not a repair.

        The first live firing sent the keystroke and killed in the SAME SECOND
        — `02:10:34 sent SHIFT+RETURN` and `02:10:34 restarting` — so a forced
        end turn could never be observed to work either way. The watchdog now
        nudges, settles, and asks the progress question again; a turn that
        moved resets the strikes and the game plays on.
        """
        script = OPS.read_text(encoding="utf-8")
        self.assertIn("NUDGE_SETTLE_S", script)
        arm = script[script.index("if nudge_end_turn; then"):]
        arm = arm[:arm.index('restart_attempt "$tag NO GAME PROGRESS')]
        self.assertIn('sleep "$NUDGE_SETTLE_S"', arm)
        # It must re-read progress, not re-use the reading that condemned it.
        self.assertIn("--progress", arm)
        # And a moved turn must skip the kill outright.
        self.assertIn("not restarting", arm)
        self.assertIn("continue", arm)

    def test_only_one_copy_of_the_nudge_survives(self) -> None:
        """It was inline in `restart_attempt` first; two copies would send the
        keystroke twice on the path that still kills."""
        script = OPS.read_text(encoding="utf-8")
        self.assertEqual(script.count("civ6_nudge_end_turn.py"), 1)

    def test_its_failure_never_blocks_the_restart(self) -> None:
        """The nudge is attempted, not depended on.

        ⚠ This used to assert that no `return 1` appeared between the nudge and
        the kill, which was right while the nudge was inline in
        `restart_attempt`. It is now a function whose own `return 1` means
        "could not send" — so the property has to be asserted at the CALL SITE
        instead: the failure path falls through to the restart.
        """
        script = OPS.read_text(encoding="utf-8")
        arm = script[script.index("if nudge_end_turn; then"):]
        # `restart_attempt` is DEFINED earlier in the file than it is called, so
        # the kill text never appears after the call site; anchor on the call.
        arm = arm[:arm.index('restart_attempt "$tag NO GAME PROGRESS')]
        # The only early exit is the recovery one, and it is conditioned on the
        # progress signal having actually changed.
        self.assertEqual(arm.count("continue"), 1)
        self.assertIn('"$after" != "$progress_signal"', arm)

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
