#!/usr/bin/env python3
"""Tests for the evidence gate that drives the live difficulty ladder."""

from __future__ import annotations

import ast
import inspect
import json
import sys
import textwrap
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

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


def wins(difficulty: str, n: int, **extra):
    return [attempt(difficulty, True, **extra) for _ in range(n)]


class TheRuleIsThreeWinsOnTheHighestClaimedRung(unittest.TestCase):
    """Operator, 2026-08-23: play the highest claimed rung until it has three
    wins there, then move up. Losses are not evidence against a rung."""

    def test_unclaimed_ladder_starts_at_settler(self):
        target, statuses = policy.next_target({"attempts": [], "wins": {}})
        self.assertEqual(target, "DIFFICULTY_SETTLER")
        self.assertFalse(statuses[0]["claimed"])

    def test_one_win_claims_a_rung_and_keeps_the_seat_on_it(self):
        state = {"wins": {"DIFFICULTY_SETTLER": {"tag": "first"}},
                 "attempts": wins("DIFFICULTY_SETTLER", 1)
                 + [attempt("DIFFICULTY_SETTLER", False)] * 7}
        target, statuses = policy.next_target(state)
        self.assertEqual(target, "DIFFICULTY_SETTLER")
        self.assertTrue(statuses[0]["claimed"])
        self.assertFalse(statuses[0]["earned"])
        self.assertEqual(statuses[0]["wins"], 1)

    def test_two_wins_do_not_advance_and_three_do(self):
        two = {"wins": {}, "attempts": wins("DIFFICULTY_SETTLER", 2)}
        self.assertEqual(policy.next_target(two)[0], "DIFFICULTY_SETTLER")
        three = {"wins": {}, "attempts": wins("DIFFICULTY_SETTLER", 3)}
        target, statuses = policy.next_target(three)
        self.assertEqual(target, "DIFFICULTY_CHIEFTAIN")
        self.assertTrue(statuses[0]["earned"])

    def test_losses_however_many_are_not_evidence_against_a_rung(self):
        """Two wins in a window of eight used to advance the seat and eight
        losses used to hold it; now only the win count speaks."""
        state = {"wins": {}, "attempts": wins("DIFFICULTY_SETTLER", 3)
                 + [attempt("DIFFICULTY_SETTLER", False)] * 40}
        self.assertEqual(policy.next_target(state)[0], "DIFFICULTY_CHIEFTAIN")

    def test_the_seat_plays_the_highest_claimed_rung_not_the_lowest_unearned(self):
        """Settler 16, Chieftain 2, Warlord 1 — the record on 2026-08-23 once
        the Warlord win is published. The seat plays Warlord: the rung above
        Chieftain being claimed answers the question Chieftain asks."""
        state = {"wins": {}, "attempts": wins("DIFFICULTY_SETTLER", 16)
                 + wins("DIFFICULTY_CHIEFTAIN", 2) + wins("DIFFICULTY_WARLORD", 1)}
        target, statuses = policy.next_target(state)
        self.assertEqual(target, "DIFFICULTY_WARLORD")
        by = {s["difficulty"]: s for s in statuses}
        self.assertEqual(by["DIFFICULTY_CHIEFTAIN"]["wins"], 2)
        self.assertFalse(by["DIFFICULTY_CHIEFTAIN"]["earned"])

    def test_before_the_warlord_win_is_published_the_seat_plays_chieftain(self):
        state = {"wins": {}, "attempts": wins("DIFFICULTY_SETTLER", 16)
                 + wins("DIFFICULTY_CHIEFTAIN", 2)}
        self.assertEqual(policy.next_target(state)[0], "DIFFICULTY_CHIEFTAIN")

    def test_the_required_count_is_adjustable_and_the_ladder_is_finite(self):
        state = {"wins": {}, "attempts": wins("DIFFICULTY_SETTLER", 2)}
        self.assertEqual(policy.next_target(state, wins_required=2)[0],
                         "DIFFICULTY_CHIEFTAIN")
        everything = {"wins": {}, "attempts": sum(
            (wins(difficulty, 3) for difficulty, _ in civ6_ladder.LADDER), [])}
        self.assertEqual(policy.next_target(everything)[0], civ6_ladder.LADDER[-1][0])

    def test_a_game_that_was_not_the_game_asked_for_never_counts(self):
        """`configured` is read back from INSIDE the running session —
        difficulty, size, speed, map script, leader, modes, ruleset — so it is
        the gate that decides comparability, and a win under it is not evidence
        for the rung the run claims to be climbing."""
        state = {
            "wins": {"DIFFICULTY_SETTLER": {"tag": "first"}},
            "attempts": wins("DIFFICULTY_SETTLER", 3, configured=False),
        }
        target, statuses = policy.next_target(state)
        self.assertEqual(target, "DIFFICULTY_SETTLER")
        self.assertEqual(statuses[0]["comparable_attempts"], 0)
        self.assertEqual(statuses[0]["wins"], 0)

    def test_unconfigured_win_is_not_a_chieftain_claim(self):
        state = {
            "wins": {},
            "attempts": [attempt("DIFFICULTY_CHIEFTAIN", True, configured=False)],
        }
        target, _ = policy.next_target(state)
        self.assertEqual(target, "DIFFICULTY_SETTLER")

    def test_a_late_backfill_cannot_change_the_answer(self):
        """A win counts whenever its row arrives: there is no window for a
        merged publish to redefine."""
        recent = [attempt("DIFFICULTY_SETTLER", False, utc=f"2026-08-19T0{i}:00:00Z")
                  for i in range(8)]
        backfilled = [attempt("DIFFICULTY_SETTLER", True, utc=f"2026-07-0{i}T00:00:00Z")
                      for i in (1, 2, 3)]
        self.assertEqual(policy.next_target({"wins": {}, "attempts": recent + backfilled})[0],
                         "DIFFICULTY_CHIEFTAIN")
        self.assertEqual(policy.next_target({"wins": {}, "attempts": backfilled + recent})[0],
                         "DIFFICULTY_CHIEFTAIN")


class TheGateReadsOnlyKeysTheLedgerWrites(unittest.TestCase):
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



class TheGateReadsTheFleetsRecordNotOneSeatsCopy(unittest.TestCase):
    """★★★★★ THE TWO SEATS ANSWERED DIFFERENT RUNGS FOR THE SAME CONTROLLER.

    `load_live` read `<runs>/ladder.json` alone. `civ6_ladder.load` seeds that
    file from the committed snapshot the first time and never looks at it
    again, so a second Civilization VI seat gates on the record as it stood the
    day it was seeded. On 2026-08-23 the published snapshot said Settler was
    repeatable while 76 Settler games from the other seat were in no published
    record at all. `mbp-m5-max-128` answered `DIFFICULTY_SETTLER` and the
    publishing seat answered `DIFFICULTY_CHIEFTAIN`, on the same day, about the
    same controller. Under the win-count rule the union matters for the wins
    it adds: a win the other seat recorded is a win.
    """

    def setUp(self):
        self._tmp = TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        root = Path(self._tmp.name)
        self.runs = root / "control"
        self.runs.mkdir()
        self.snapshot = root / "civ6_ladder.json"
        self._data, civ6_ladder.DATA = civ6_ladder.DATA, self.snapshot
        self.addCleanup(lambda: setattr(civ6_ladder, "DATA", self._data))

    def _write(self, path, rows):
        path.write_text(json.dumps({"attempts": rows, "wins": {}}))

    def test_the_other_seats_wins_reach_this_seats_gate(self):
        """Two published Settler wins plus one recorded only here make three:
        the union earns the rung where either copy alone would not."""
        self._write(self.snapshot, [
            attempt("DIFFICULTY_SETTLER", True, tag="pub-win-1",
                    utc="2026-08-18T01:00:00Z"),
            attempt("DIFFICULTY_SETTLER", True, tag="pub-win-2",
                    utc="2026-08-18T02:00:00Z"),
            attempt("DIFFICULTY_SETTLER", False, tag="pub-loss",
                    utc="2026-08-18T03:00:00Z")])
        self._write(civ6_ladder.live_ledger_for(self.runs), [
            attempt("DIFFICULTY_SETTLER", True, tag="local-win",
                    utc="2026-08-19T01:00:00Z"),
            attempt("DIFFICULTY_SETTLER", False, tag="local-loss",
                    utc="2026-08-19T02:00:00Z")])
        state = policy.load_live(self.runs)
        self.assertEqual(len(state["attempts"]), 5)
        status = policy.rung_status(state, "DIFFICULTY_SETTLER")
        self.assertEqual(status["wins"], 3)
        self.assertTrue(status["earned"])
        self.assertEqual(policy.next_target(state)[0], "DIFFICULTY_CHIEFTAIN")

    def test_one_copy_of_the_record_alone_would_hold_the_seat_back(self):
        published = [
            attempt("DIFFICULTY_SETTLER", True, tag="pub-win-1",
                    utc="2026-08-18T01:00:00Z"),
            attempt("DIFFICULTY_SETTLER", True, tag="pub-win-2",
                    utc="2026-08-18T02:00:00Z")]
        target, _ = policy.next_target({"wins": {}, "attempts": published})
        self.assertEqual(target, "DIFFICULTY_SETTLER")

    def test_a_seat_with_no_committed_snapshot_still_gates_on_its_own_rows(self):
        self._write(civ6_ladder.live_ledger_for(self.runs), [
            attempt("DIFFICULTY_SETTLER", False, tag="only-local",
                    utc="2026-08-19T00:00:00Z")])
        state = policy.load_live(self.runs)
        self.assertEqual([a["tag"] for a in state["attempts"]], ["only-local"])
        self.assertEqual(policy.next_target(state)[0], "DIFFICULTY_SETTLER")


if __name__ == "__main__":
    unittest.main()
