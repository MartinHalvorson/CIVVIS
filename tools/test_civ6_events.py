#!/usr/bin/env python3
"""The event report adds up what a planted game recorded."""

from __future__ import annotations

import contextlib
import io
import json
import os
import sqlite3
import tempfile
import unittest
from pathlib import Path
from unittest import mock

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


def plot_chunk(turn: int, plots: list[dict]) -> dict:
    return {"kind": "tiles", "turn": turn, "plots": plots}


def standing(turn: int, unit: int, x: int, y: int) -> dict:
    return {"kind": "state", "turn": turn, "units": [{"id": unit, "x": x, "y": y}]}


def strike(unit: int, length: int = 2, end: tuple[int, int] = (3, 4), kind: str = "UNIT_APOSTLE") -> dict:
    return {"kind": "strike", "verb": "ATTACK", "turn": 5, "unit": unit, "x": 3, "y": 4, "unit_kind": kind,
            "native_move_path_known": True, "native_move_path_length": length,
            "native_move_path_this_turn_end": {"x": end[0], "y": end[1], "turn": 1}}


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

    def test_a_strike_with_no_host_path_is_named(self):
        rows = [strike(1, length=0), order("order_failed", 1, verb="ATTACK", reason="target_unharmed")]
        table = ev.strike_paths([game(self.root, "a", rows)])
        self.assertEqual(dict(table[("religious", "no_path")]), {"missed": 1})

    def test_a_strike_reaching_its_target_lands(self):
        rows = [strike(1, end=(3, 4), kind="UNIT_WARRIOR"), order("order_verified", 1, verb="ATTACK")]
        table = ev.strike_paths([game(self.root, "a", rows)])
        self.assertEqual(dict(table[("military", "reaches_target")]), {"landed": 1})

    def test_a_path_ending_elsewhere_stops_short(self):
        rows = [strike(1, end=(2, 4)), order("order_failed", 1, verb="ATTACK", reason="x")]
        self.assertEqual(dict(ev.strike_paths([game(self.root, "a", rows)])[("religious", "stops_short")]), {"missed": 1})

    def test_a_resent_strike_counts_once(self):
        rows = [strike(1, length=0), strike(1, length=0), order("order_failed", 1, verb="ATTACK", reason="x")]
        self.assertEqual(ev.strike_paths([game(self.root, "a", rows)])[("religious", "no_path")]["missed"], 1)

    def test_a_retry_that_worked_is_a_landed_strike(self):
        rows = [strike(1, end=(3, 4)), order("order_failed", 1, verb="ATTACK", reason="x"),
                order("order_verified", 1, verb="ATTACK")]
        self.assertEqual(dict(ev.strike_paths([game(self.root, "a", rows)])[("religious", "reaches_target")]), {"landed": 1})

    def test_a_strike_with_no_verdict_is_not_guessed(self):
        self.assertEqual(dict(ev.strike_paths([game(self.root, "a", [strike(1, length=0)])])), {})

    def test_unread_path_data_is_unknown(self):
        rows = [{**strike(1), "native_move_path_known": False}, order("order_verified", 1, verb="ATTACK")]
        self.assertIn(("religious", "unknown"), ev.strike_paths([game(self.root, "a", rows)]))

    def test_a_failed_order_is_filed_under_the_plot_it_stood_on(self):
        rows = [
            plot_chunk(4, [{"x": 3, "y": 4, "im": "IMPROVEMENT_LUMBER_MILL", "f": "FEATURE_FOREST"}]),
            standing(5, 1, 3, 4),
            order("order_failed", 1, verb="REMOVE_FEATURE", reason="no_improvement"),
        ]
        table = ev.order_plots([game(self.root, "a", rows)])
        self.assertEqual(dict(table[("REMOVE_FEATURE", "IMPROVEMENT_LUMBER_MILL", "FEATURE_FOREST", "-")]), {"failed": 1})

    def test_an_order_that_worked_is_filed_under_the_plot_before_it(self):
        rows = [
            plot_chunk(4, [{"x": 3, "y": 4, "f": "FEATURE_FOREST"}]),
            standing(5, 1, 3, 4),
            plot_chunk(5, [{"x": 3, "y": 4}]),
            order("order_verified", 1, verb="REMOVE_FEATURE"),
        ]
        table = ev.order_plots([game(self.root, "a", rows)])
        self.assertEqual(dict(table[("REMOVE_FEATURE", "-", "FEATURE_FOREST", "-")]), {"worked": 1})

    def test_the_unit_stands_where_its_own_turn_left_it(self):
        rows = [
            plot_chunk(4, [{"x": 3, "y": 4, "r": "RESOURCE_STONE"}, {"x": 9, "y": 9, "r": "RESOURCE_DEER"}]),
            standing(5, 1, 0, 0), standing(5, 1, 3, 4), standing(6, 1, 9, 9),
            order("order_failed", 1, verb="HARVEST_RESOURCE", reason="x"),
        ]
        self.assertIn(("HARVEST_RESOURCE", "-", "-", "RESOURCE_STONE"), ev.order_plots([game(self.root, "a", rows)]))

    def test_named_verbs_leave_the_rest_out(self):
        rows = [
            standing(5, 1, 3, 4),
            order("order_failed", 1, verb="HARVEST_RESOURCE", reason="x"),
            order("order_failed", 1, verb="FORTIFY", reason="x"),
        ]
        table = ev.order_plots([game(self.root, "a", rows)], ["FORTIFY"])
        self.assertEqual([key[0] for key in table], ["FORTIFY"])

    def test_a_city_order_has_no_plot(self):
        rows = [standing(5, 1, 3, 4), {**order("order_failed", 1, reason="x"), "order_kind": "produce"}]
        self.assertEqual(dict(ev.order_plots([game(self.root, "a", rows)])), {})

    def test_a_unit_no_frame_saw_is_left_out(self):
        self.assertEqual(dict(ev.order_plots([game(self.root, "a", [order("order_failed", 1, reason="x")])])), {})

    def test_flags_can_sit_among_the_verbs(self):
        game(self.root, "a", [standing(5, 1, 3, 4), order("order_failed", 1, reason="x")])
        out = io.StringIO()
        argv = ["civ6_events.py", "plots", "FORTIFY", "--root", str(self.root), "IMPROVE"]
        with mock.patch("sys.argv", argv), contextlib.redirect_stdout(out):
            self.assertEqual(ev.main(), 0)
        self.assertIn("FORTIFY", out.getvalue())

    def stall(self, units_last, frames=None):
        """One unit told to step onto (3, 4) on turn 5, which stayed put."""
        first = [{"id": 1, "x": 3, "y": 3, "moves": 2}, {"id": 2, "x": 3, "y": 4, "moves": 2}]
        rows = [
            {"kind": "state", "turn": 5, "units": first},
            {"kind": "state", "turn": 5, "units": units_last},
            order("order_failed", 1, verb="MOVE_TO", reason="did_not_move"),
        ]
        path = game(self.root, "a", rows)
        planted = sqlite3.connect(path.parent / "orders.sqlite")
        planted.execute("create table orders (turn, seq, kind, subject, verb, x, y, frame)")
        planted.execute("insert into orders values (5, 1, 'unit', 1, 'MOVE_TO', 3, 4, 0)")
        planted.commit()
        planted.close()
        return ev.stalled_moves([path])

    def test_a_stall_behind_a_unit_that_left(self):
        table = self.stall([{"id": 1, "x": 3, "y": 3, "moves": 2}, {"id": 2, "x": 9, "y": 9, "moves": 0}])
        self.assertEqual(dict(table), {"behind_a_unit_that_left": 1})

    def test_a_stall_behind_a_unit_that_stayed(self):
        table = self.stall([{"id": 1, "x": 3, "y": 3, "moves": 2}, {"id": 2, "x": 3, "y": 4, "moves": 2}])
        self.assertEqual(dict(table), {"behind_a_unit_that_stayed": 1})

    def test_a_unit_that_spent_its_movement_is_not_a_stall(self):
        self.assertEqual(dict(self.stall([{"id": 1, "x": 3, "y": 3, "moves": 0}])), {})

    def test_a_run_with_no_order_database_is_not_guessed(self):
        rows = [standing(5, 1, 3, 4), order("order_failed", 1, verb="MOVE_TO", reason="did_not_move")]
        self.assertEqual(dict(ev.stalled_moves([game(self.root, "a", rows)])), {})

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
