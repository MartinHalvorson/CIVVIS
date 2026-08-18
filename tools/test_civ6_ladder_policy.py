#!/usr/bin/env python3
"""Tests for the evidence gate that drives the live difficulty ladder."""

from __future__ import annotations

import ast
import inspect
import sys
import textwrap
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402
import civ6_ladder_policy as policy  # noqa: E402


def attempt(difficulty: str, won: bool, **extra):
    return {
        "configured": True,
        "difficulty": difficulty,
        "won": won,
        "reason": "stopped",
        **extra,
    }


class LadderPolicyTests(unittest.TestCase):
    def test_unclaimed_ladder_starts_at_settler(self):
        target, statuses = policy.next_target({"attempts": [], "wins": {}})
        self.assertEqual(target, "DIFFICULTY_SETTLER")
        self.assertFalse(statuses[0]["claimed"])

    def test_one_historical_win_does_not_advance(self):
        state = {
            "wins": {"DIFFICULTY_SETTLER": {"tag": "first"}},
            "attempts": [
                attempt("DIFFICULTY_SETTLER", True),
                attempt("DIFFICULTY_SETTLER", False),
                attempt("DIFFICULTY_SETTLER", False),
            ],
        }
        target, statuses = policy.next_target(state)
        self.assertEqual(target, "DIFFICULTY_SETTLER")
        self.assertFalse(statuses[0]["repeatable"])
        self.assertEqual(statuses[0]["window_wins"], 1)

    def test_two_wins_in_a_comparable_window_advance_to_chieftain(self):
        state = {
            "wins": {"DIFFICULTY_SETTLER": {"tag": "first"}},
            "attempts": [
                attempt("DIFFICULTY_SETTLER", True),
                attempt("DIFFICULTY_SETTLER", False),
                attempt("DIFFICULTY_SETTLER", True),
            ],
        }
        target, statuses = policy.next_target(
            state, window=3, repeat_wins=2, min_attempts=3
        )
        self.assertEqual(target, "DIFFICULTY_CHIEFTAIN")
        self.assertTrue(statuses[0]["repeatable"])

    def test_a_game_that_was_not_the_game_asked_for_never_counts(self):
        """`configured` is read back from INSIDE the running session —
        difficulty, size, speed, map script, leader, modes, ruleset — so it is
        the gate that decides comparability, and a win under it is not evidence
        for the rung the run claims to be climbing."""
        state = {
            "wins": {"DIFFICULTY_SETTLER": {"tag": "first"}},
            "attempts": [
                attempt("DIFFICULTY_SETTLER", True, configured=False),
                attempt("DIFFICULTY_SETTLER", True, configured=False),
                attempt("DIFFICULTY_SETTLER", True, configured=False),
            ],
        }
        target, statuses = policy.next_target(state)
        self.assertEqual(target, "DIFFICULTY_SETTLER")
        self.assertEqual(statuses[0]["comparable_attempts"], 0)

    def test_every_key_the_gate_reads_is_a_key_the_ledger_writes(self):
        """★★★★★ A FILTER KEY NOTHING WRITES IS A GUARD THAT CANNOT FIRE.

        `comparable_attempt` filtered on `settings_mismatch` and `blocked` for
        as long as it existed and `civ6_ladder.entry_from` wrote neither: of the
        325 rows in the live ledger, **0 carried either key**. They read as two
        extra safety conditions and were two no-ops in the file that decides
        when a difficulty rung has been beaten — this repository's own "a claim
        is not a check", one level up from the tools it is usually about.
        (`civ6_civvis_climb.py` does write both, onto `civvis_ladder.jsonl`,
        which is a different file this module never reads.)

        This is the check that keeps that from coming back: every key the
        predicate consults must be a key a ledger row actually has.
        """
        row = civ6_ladder.entry_from({})
        function = ast.parse(textwrap.dedent(
            inspect.getsource(policy.comparable_attempt))).body[0]
        # Drop the docstring: it quotes the two deleted conditions verbatim so
        # the reason they went is recorded where the next reader will look, and
        # a text scan would read that history as live code.
        if (function.body and isinstance(function.body[0], ast.Expr)
                and isinstance(function.body[0].value, ast.Constant)):
            function.body = function.body[1:]
        read = {
            node.args[0].value
            for node in ast.walk(ast.Module(body=function.body, type_ignores=[]))
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute) and node.func.attr == "get"
            and isinstance(node.func.value, ast.Name)
            and node.func.value.id == "attempt"
            and node.args and isinstance(node.args[0], ast.Constant)
            and isinstance(node.args[0].value, str)
        }
        self.assertTrue(read, "the predicate reads no ledger keys at all")
        for key in sorted(read):
            self.assertIn(
                key, row,
                f"comparable_attempt filters on {key!r} and "
                f"civ6_ladder.entry_from never writes it, so the condition can "
                f"never be anything but true")

    def test_unconfigured_win_is_not_a_chieftain_claim(self):
        state = {
            "wins": {},
            "attempts": [attempt("DIFFICULTY_CHIEFTAIN", True, configured=False)],
        }
        target, _ = policy.next_target(state)
        self.assertEqual(target, "DIFFICULTY_SETTLER")


if __name__ == "__main__":
    unittest.main()
