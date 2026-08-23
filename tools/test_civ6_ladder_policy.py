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


class TheWindowIsTheNewestGamesNotTheNewestRows(unittest.TestCase):
    """★★★★★ INSERTION ORDER WEARING CHRONOLOGY'S NAME, IN THE RUNG GATE.

    `civ6_ladder.apply` carries this correction in its own words and applied it
    to the rung MILESTONE: the earliest win stands "by the clock, not by the
    order attempts happened to reach this function", because `sync` exists to
    record attempts late. The gate kept reading `attempts[-window:]`, so the
    same late arrival that could not move a milestone could still redefine
    "the last eight attempts" as eight games from a week ago -- and a merged
    publish, which appends another seat's history in one go, is that arrival
    at scale.
    """

    def _rows(self, spec):
        return [attempt("DIFFICULTY_SETTLER", won, utc=utc, tag=tag)
                for tag, utc, won in spec]

    def test_a_batch_of_old_wins_appended_late_is_not_the_recent_record(self):
        recent = self._rows([(f"new-{i}", f"2026-08-19T0{i}:00:00Z", False)
                             for i in range(8)])
        backfilled = self._rows([
            ("old-win-1", "2026-07-01T00:00:00Z", True),
            ("old-win-2", "2026-07-02T00:00:00Z", True),
            ("old-win-3", "2026-07-03T00:00:00Z", True)])
        state = {"wins": {}, "attempts": recent + backfilled}
        status = policy.rung_status(state, "DIFFICULTY_SETTLER")
        self.assertEqual(status["window_wins"], 0)
        self.assertFalse(status["repeatable"])
        self.assertEqual(policy.next_target(state)[0], "DIFFICULTY_SETTLER")

    def test_recent_wins_still_count_when_older_rows_arrive_after_them(self):
        """The correction must not cut the other way: a real recent win is
        still in the window however late the rows around it were recorded."""
        state = {"wins": {}, "attempts": self._rows([
            ("old-loss", "2026-07-01T00:00:00Z", False),
            ("win-a", "2026-08-19T01:00:00Z", True),
            ("win-b", "2026-08-19T02:00:00Z", True),
            ("older-loss", "2026-07-02T00:00:00Z", False)])}
        status = policy.rung_status(state, "DIFFICULTY_SETTLER", window=3)
        self.assertEqual(status["window_wins"], 2)
        self.assertTrue(status["repeatable"])

    def test_a_row_with_no_stamp_cannot_claim_to_be_the_newest_game(self):
        state = {"wins": {}, "attempts":
                 self._rows([(f"new-{i}", f"2026-08-19T0{i}:00:00Z", False)
                             for i in range(3)])
                 + [attempt("DIFFICULTY_SETTLER", True, tag="undated")]}
        status = policy.rung_status(state, "DIFFICULTY_SETTLER", window=3)
        self.assertEqual(status["window_wins"], 0)


class TheGateReadsTheFleetsRecordNotOneSeatsCopy(unittest.TestCase):
    """★★★★★ THE TWO SEATS ANSWERED DIFFERENT RUNGS FOR THE SAME CONTROLLER.

    `load_live` read `<runs>/ladder.json` alone. `civ6_ladder.load` seeds that
    file from the committed snapshot the first time and never looks at it
    again, so a second Civilization VI seat gates on the record as it stood the
    day it was seeded. On 2026-08-23 the published snapshot said Settler was
    repeatable -- two wins in its trailing eight -- while 76 Settler games from
    the other seat, whose newest eight were all losses, were in no published
    record at all. `mbp-m5-max-128` answered `DIFFICULTY_SETTLER` and the
    publishing seat answered `DIFFICULTY_CHIEFTAIN`, on the same day, about the
    same controller.
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

    def test_the_other_seats_losses_reach_this_seats_gate(self):
        self._write(self.snapshot, [
            attempt("DIFFICULTY_SETTLER", True, tag="pub-win-1",
                    utc="2026-08-18T01:00:00Z"),
            attempt("DIFFICULTY_SETTLER", True, tag="pub-win-2",
                    utc="2026-08-18T02:00:00Z"),
            attempt("DIFFICULTY_SETTLER", False, tag="pub-loss",
                    utc="2026-08-18T03:00:00Z")])
        # This seat's own newer games, none of them published, all losses.
        self._write(civ6_ladder.live_ledger_for(self.runs), [
            attempt("DIFFICULTY_SETTLER", False, tag=f"local-{i}",
                    utc=f"2026-08-19T0{i}:00:00Z") for i in range(3)])
        state = policy.load_live(self.runs)
        self.assertEqual(len(state["attempts"]), 6)
        status = policy.rung_status(state, "DIFFICULTY_SETTLER", window=3)
        self.assertEqual(status["window_wins"], 0)
        self.assertFalse(status["repeatable"])

    def test_the_published_record_alone_would_have_advanced_the_rung(self):
        """The same fixture, gated the old way, hands out Chieftain."""
        published = [
            attempt("DIFFICULTY_SETTLER", True, tag="pub-win-1",
                    utc="2026-08-18T01:00:00Z"),
            attempt("DIFFICULTY_SETTLER", True, tag="pub-win-2",
                    utc="2026-08-18T02:00:00Z"),
            attempt("DIFFICULTY_SETTLER", False, tag="pub-loss",
                    utc="2026-08-18T03:00:00Z")]
        target, _ = policy.next_target({"wins": {}, "attempts": published},
                                       window=3)
        self.assertEqual(target, "DIFFICULTY_CHIEFTAIN")

    def test_a_seat_with_no_committed_snapshot_still_gates_on_its_own_rows(self):
        self._write(civ6_ladder.live_ledger_for(self.runs), [
            attempt("DIFFICULTY_SETTLER", False, tag="only-local",
                    utc="2026-08-19T00:00:00Z")])
        state = policy.load_live(self.runs)
        self.assertEqual([a["tag"] for a in state["attempts"]], ["only-local"])
        self.assertEqual(policy.next_target(state)[0], "DIFFICULTY_SETTLER")


if __name__ == "__main__":
    unittest.main()
