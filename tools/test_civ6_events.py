#!/usr/bin/env python3
"""The event report adds up what a planted game recorded."""

from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path

import civ6_events as ev


def game(root: Path, name: str, rows: list) -> Path:
    path = root / name / "events.jsonl"
    path.parent.mkdir(parents=True)
    text = "\n".join(row if isinstance(row, str) else json.dumps(row) for row in rows) + "\n"
    path.write_text(text.ljust(ev.MIN_GAME_BYTES, " ") + "\n")
    return path


def state(units: list[tuple[int, int, int]]) -> dict:
    return {"kind": "state", "units": [{"id": i, "x": x, "y": y} for i, x, y in units]}


def order(kind: str, subject: int, verb: str = "FORTIFY", turn: int = 5, reason: str | None = None) -> dict:
    row = {"kind": kind, "order_kind": "unit", "verb": verb, "subject": subject, "turn": turn}
    return row if reason is None else {**row, "reason": reason}


def noop(unit: int, frm: list[int], want: list[int], why: str = "cannot_start") -> dict:
    return {"kind": "move_noop", "unit": unit, "from": frm, "want": want, "why": why}


class Reports(unittest.TestCase):
    def setUp(self):
        self.holder = tempfile.TemporaryDirectory()
        self.root = Path(self.holder.name)

    def tearDown(self):
        self.holder.cleanup()

    def test_a_fragment_is_not_a_game(self):
        game(self.root, "whole", [{"kind": "state"}])
        small = self.root / "tiny" / "events.jsonl"
        small.parent.mkdir()
        small.write_text("{}\n")
        self.assertEqual([path.parent.name for path in ev.recent_games(self.root, 9)], ["whole"])

    def test_newest_games_come_first(self):
        old = game(self.root, "old", [{"kind": "x"}])
        new = game(self.root, "new", [{"kind": "x"}])
        os.utime(old, (1000, 1000))
        os.utime(new, (2000, 2000))
        self.assertEqual([path.parent.name for path in ev.recent_games(self.root, 1)], ["new"])

    def test_kinds_count_games_and_fields(self):
        logs = [game(self.root, "a", [{"kind": "turn", "n": 1}, {"kind": "turn", "m": 2}])]
        table = ev.kinds(logs)
        self.assertEqual(table["turn"]["count"], 2)
        self.assertEqual(len(table["turn"]["games"]), 1)
        self.assertTrue({"n", "m"} <= table["turn"]["fields"])

    def test_a_torn_line_is_skipped(self):
        logs = [game(self.root, "a", ['{"kind": "tu', {"kind": "turn"}])]
        self.assertEqual(ev.kinds(logs)["turn"]["count"], 1)

    def test_failures_give_a_rate_and_collapse_numbers_in_reasons(self):
        rows = [
            order("order_verified", 1),
            order("order_failed", 2, reason="gone:12"),
            order("order_failed", 3, reason="gone:99"),
        ]
        row = ev.failures([game(self.root, "a", rows)])[0]
        self.assertEqual((row["total"], row["failed"]), (3, 2))
        self.assertAlmostEqual(row["rate"], 2 / 3)
        self.assertEqual(row["reasons"], [("gone:N", 2)])

    def test_a_retried_order_is_one_order(self):
        rows = [order("order_failed", 1, reason="stuck") for _ in range(3)]
        row = ev.failures([game(self.root, "a", rows)])[0]
        self.assertEqual((row["total"], row["failed"]), (1, 1))
        self.assertEqual(row["reasons"], [("stuck", 1)])

    def test_an_order_that_fails_then_works_did_not_fail(self):
        rows = [order("order_failed", 1, reason="stuck"), order("order_verified", 1)]
        row = ev.failures([game(self.root, "a", rows)])[0]
        self.assertEqual((row["total"], row["failed"], row["recovered"]), (1, 0, 1))
        self.assertEqual(row["reasons"], [])

    def test_the_same_order_next_turn_is_another_order(self):
        rows = [order("order_failed", 1, turn=5, reason="stuck"), order("order_failed", 1, turn=6, reason="stuck")]
        self.assertEqual(ev.failures([game(self.root, "a", rows)])[0]["failed"], 2)

    def test_the_same_turn_in_another_log_is_another_order(self):
        logs = [game(self.root, name, [order("order_failed", 1, reason="stuck")]) for name in ("a", "b")]
        self.assertEqual(ev.failures(logs)[0]["failed"], 2)

    def test_most_failures_come_first_whatever_the_rate(self):
        rows = [order("order_failed", 1, verb="RARE", reason="x")]
        rows += [order("order_failed", n, verb="COMMON", reason="x") for n in (2, 3)]
        rows += [order("order_verified", n, verb="COMMON") for n in (4, 5, 6, 7)]
        self.assertEqual([row["verb"] for row in ev.failures([game(self.root, "a", rows)])], ["COMMON", "RARE"])

    def test_every_sitting_of_a_game_belongs_to_that_game(self):
        names = ["civvis-20261110T024805Z", "civvis-20261110T024805Z-cont1", "civvis-20261110T024805Z-cont12"]
        self.assertEqual({ev.game_of(Path("/runs") / name / "events.jsonl") for name in names},
                         {"civvis-20261110T024805Z"})

    def test_a_verb_with_arguments_groups_under_its_head(self):
        rows = [{"kind": "order_verified", "order_kind": "unit", "verb": "IMPROVE:FARM"}]
        self.assertEqual(ev.failures([game(self.root, "a", rows)])[0]["verb"], "IMPROVE")

    def test_a_noop_is_judged_by_the_next_state(self):
        rows = [
            noop(1, [0, 0], [1, 1]), noop(2, [0, 0], [1, 1]), noop(3, [0, 0], [1, 1]), noop(4, [0, 0], [1, 1]),
            state([(1, 1, 1), (2, 0, 0), (3, 5, 5)]),
        ]
        table = ev.noop_outcomes([game(self.root, "a", rows)])
        self.assertEqual(dict(table["cannot_start"]), {"reached": 1, "stayed": 1, "elsewhere": 1, "gone": 1})

    def test_a_noop_with_no_later_state_is_not_guessed(self):
        self.assertEqual(dict(ev.noop_outcomes([game(self.root, "a", [noop(1, [0, 0], [1, 1])])])), {})


if __name__ == "__main__":
    unittest.main()
