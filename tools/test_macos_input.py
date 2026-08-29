#!/usr/bin/env python3
"""What the keyboard helper sends, and why SHIFT+RETURN is in it."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import macos_input  # noqa: E402


class PressKeyTest(unittest.TestCase):
    """⚠ SHIFT+RETURN is Civilization VI's forced end turn.

    It is the request the shipped UI sends, and the one end-turn form the engine
    does not refuse while a blocker stands. It is reachable from here so a
    parked turn can be nudged from OUTSIDE the game — the harness keeps its
    input grants even when the mod has stopped ticking, which is exactly the
    state that has ended three games in a row.
    """

    def test_the_two_backends_agree_on_every_key(self) -> None:
        for name, (code, cli) in macos_input.KEY_CODES.items():
            with self.subTest(key=name):
                self.assertIsInstance(code, int)
                self.assertTrue(cli)

    def test_every_modifier_uses_the_name_its_backend_knows(self) -> None:
        """⚠ The vocabularies differ and only `shift` overlaps.

        cliclick takes `alt`, `cmd`, `ctrl`, `fn`, `shift`; the native helper
        switches on the spellings that name the `CGEventFlags`. Passing the
        canonical name straight through silently refused three of the four.
        """
        for canonical, cli in macos_input.MODIFIERS.items():
            with self.subTest(modifier=canonical):
                with mock.patch.object(macos_input, "_cliclick", return_value="/bin/cliclick"), \
                     mock.patch.object(macos_input, "_run_cliclick") as run:
                    macos_input.press_key("return", modifier=canonical)
                self.assertEqual(run.call_args.args[0],
                                 [f"kd:{cli}", "kp:return", f"ku:{cli}"])
        self.assertEqual(macos_input.MODIFIERS["control"], "ctrl")
        self.assertEqual(macos_input.MODIFIERS["option"], "alt")
        self.assertEqual(macos_input.MODIFIERS["command"], "cmd")

    def test_cliclick_holds_the_modifier_in_one_invocation(self) -> None:
        """Two invocations could leave shift held if the first one died."""
        with mock.patch.object(macos_input, "_cliclick", return_value="/bin/cliclick"), \
             mock.patch.object(macos_input, "_run_cliclick") as run:
            macos_input.press_key("return", modifier="shift")
        self.assertEqual(run.call_args.args[0], ["kd:shift", "kp:return", "ku:shift"])

    def test_a_bare_key_sends_no_modifier(self) -> None:
        with mock.patch.object(macos_input, "_cliclick", return_value="/bin/cliclick"), \
             mock.patch.object(macos_input, "_run_cliclick") as run:
            macos_input.press_key("escape")
        self.assertEqual(run.call_args.args[0], ["kp:esc"])

    def test_the_native_backend_is_told_the_modifier_too(self) -> None:
        with mock.patch.object(macos_input, "_cliclick", return_value=None), \
             mock.patch.object(macos_input, "_run_native") as run:
            macos_input.press_key("return", modifier="shift")
        self.assertEqual(run.call_args.args[0], ["key", "36", "shift"])

    def test_an_unknown_key_or_modifier_is_refused_before_anything_is_sent(self) -> None:
        with mock.patch.object(macos_input, "_run_cliclick") as run, \
             mock.patch.object(macos_input, "_run_native") as native:
            with self.assertRaises(ValueError):
                macos_input.press_key("f13")
            with self.assertRaises(ValueError):
                macos_input.press_key("return", modifier="hyper")
        run.assert_not_called()
        native.assert_not_called()

    def test_the_swift_helper_sets_the_flag_on_both_events(self) -> None:
        """A bare key-up leaves the shift state ambiguous for whatever follows."""
        source = Path(macos_input.__file__).read_text(encoding="utf-8")
        swift = source.split('case "key":', 1)[1].split('case "scroll":', 1)[0]
        self.assertEqual(swift.count("flags = flags"), 2)
        self.assertIn("keyDown: true", swift)
        self.assertIn("keyDown: false", swift)
        for modifier in macos_input.MODIFIERS:
            with self.subTest(modifier=modifier):
                self.assertIn(f'case "{modifier}"', swift)


if __name__ == "__main__":
    unittest.main()
