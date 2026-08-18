#!/usr/bin/env python3
"""Tests for the evidence gate that drives the live difficulty ladder."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

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

    def test_wrong_settings_and_blocked_starts_never_count(self):
        state = {
            "wins": {"DIFFICULTY_SETTLER": {"tag": "first"}},
            "attempts": [
                attempt("DIFFICULTY_SETTLER", True, settings_mismatch={"difficulty": {}}),
                attempt("DIFFICULTY_SETTLER", True, blocked="no game"),
                attempt("DIFFICULTY_SETTLER", True, configured=False),
            ],
        }
        target, statuses = policy.next_target(state)
        self.assertEqual(target, "DIFFICULTY_SETTLER")
        self.assertEqual(statuses[0]["comparable_attempts"], 0)

    def test_unconfigured_win_is_not_a_chieftain_claim(self):
        state = {
            "wins": {},
            "attempts": [attempt("DIFFICULTY_CHIEFTAIN", True, configured=False)],
        }
        target, _ = policy.next_target(state)
        self.assertEqual(target, "DIFFICULTY_SETTLER")


if __name__ == "__main__":
    unittest.main()
