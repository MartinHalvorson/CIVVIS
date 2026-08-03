#!/usr/bin/env python3
"""Focused persistence, strategy-selection and decider protocol checks."""

from __future__ import annotations

import json
import io
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_brain  # noqa: E402


class FakeProc:
    def __init__(self, lines: list[str]) -> None:
        self.stdout = io.StringIO("".join(lines))
        self.stdin = io.StringIO()

    def poll(self):
        return None


class _Decider(civ6_brain.Decider):
    def __init__(self, lines: list[str]) -> None:
        self.proc = FakeProc(lines)
        self.binary = Path("/nonexistent")
        self.run_dir = Path("/nonexistent")
        self.victory = "domination"

    def start(self) -> None:  # pragma: no cover - must never be reached
        raise AssertionError("the canned process must not be replaced")


class DeciderProtocolTest(unittest.TestCase):
    def test_a_plain_response_is_read(self) -> None:
        decider = _Decider([
            '{"turn":1,"orders":[{"kind":"unit","subject":7,"verb":"MOVE_TO",'
            '"x":3,"y":4}],"note":"ok"}\n'
        ])
        rows, note = decider.ask(1)
        self.assertEqual(rows, [("unit", 7, "MOVE_TO", 3, 4)])
        self.assertEqual(note, "ok")

    def test_non_response_json_is_skipped(self) -> None:
        decider = _Decider([
            '{"kind":"genome","strategy":"stock"}\n',
            '{"turn":1,"orders":[],"note":"real"}\n',
        ])
        rows, note = decider.ask(1)
        self.assertEqual(rows, [])
        self.assertEqual(note, "real")


class Civ6BrainTest(unittest.TestCase):
    def test_new_government_progression_is_not_blocked(self) -> None:
        seen: set[str] = set()
        rows = [("government", None, "GOVERNMENT_CLASSICAL_REPUBLIC", None, None)]

        guarded, blocked = civ6_brain.guard_government_orders(
            {"government": "GOVERNMENT_CHIEFDOM"}, rows, seen
        )

        self.assertEqual(guarded, rows)
        self.assertEqual(blocked, [])
        self.assertEqual(seen, {"GOVERNMENT_CHIEFDOM"})

    def test_return_to_an_observed_government_is_blocked(self) -> None:
        seen = {
            "GOVERNMENT_MONARCHY",
            "GOVERNMENT_THEOCRACY",
            "GOVERNMENT_MERCHANT_REPUBLIC",
        }
        rows = [
            ("research", None, "TECH_INDUSTRIALIZATION", None, None),
            ("government", None, "GOVERNMENT_MONARCHY", None, None),
        ]

        guarded, blocked = civ6_brain.guard_government_orders(
            {"government": "GOVERNMENT_MERCHANT_REPUBLIC"}, rows, seen
        )

        self.assertEqual(
            guarded,
            [("research", None, "TECH_INDUSTRIALIZATION", None, None)],
        )
        self.assertEqual(
            blocked,
            ["GOVERNMENT_MONARCHY: return to a previously used government"],
        )

    def test_anarchy_does_not_restart_the_previous_government(self) -> None:
        seen = {"GOVERNMENT_MERCHANT_REPUBLIC"}
        rows = [
            ("government", None, "GOVERNMENT_MERCHANT_REPUBLIC", None, None),
            ("unit", 7, "MOVE_TO", 3, 4),
        ]

        guarded, blocked = civ6_brain.guard_government_orders(
            {"government": None, "policy_slots": 0}, rows, seen
        )

        self.assertEqual(guarded, [("unit", 7, "MOVE_TO", 3, 4)])
        self.assertEqual(
            blocked,
            ["GOVERNMENT_MERCHANT_REPUBLIC: government transition in progress"],
        )

    def test_opening_government_choice_remains_available(self) -> None:
        seen: set[str] = set()
        rows = [("government", None, "GOVERNMENT_CHIEFDOM", None, None)]

        guarded, blocked = civ6_brain.guard_government_orders(
            {"government": None, "policy_slots": 0}, rows, seen
        )

        self.assertEqual(guarded, rows)
        self.assertEqual(blocked, [])

    def test_resume_checkpoint_contains_only_ready_turns_for_this_run(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            conn = civ6_brain.connect(Path(temporary) / "orders.sqlite")
            civ6_brain.write_turn(conn, "live", 3, [("research", None, "TECH_MINING", None, None)])
            conn.execute("INSERT INTO ready (run, turn, count) VALUES (?,?,?)", ("other", 4, 0))
            conn.commit()

            self.assertEqual(civ6_brain.completed_turns(conn, "live"), {3})
            self.assertEqual(civ6_brain.completed_turns(conn, "other"), {4})
            self.assertEqual(civ6_brain.completed_turns(conn, "missing"), set())
            conn.close()

    def test_completed_game_turns_recovers_only_finished_turns_for_the_run(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            events = Path(temporary) / "events.jsonl"
            events.write_text("\n".join([
                json.dumps({"kind": "state", "run": "live", "turn": 5}),
                json.dumps({"kind": "turn", "run": "other", "turn": 6}),
                json.dumps({"kind": "turn", "run": "live", "turn": 7}),
                json.dumps({"kind": "turn", "run": "live", "turn": "8"}),
                json.dumps({"kind": "turn", "run": "live", "turn": "bad"}),
                "not json",
            ]) + "\n")

            self.assertEqual(civ6_brain.completed_game_turns(events, "live"), {7, 8})
            self.assertEqual(civ6_brain.completed_game_turns(events, "other"), {6})

    def test_default_orders_database_is_scoped_to_its_run(self) -> None:
        run = Path("/tmp/civvis-run")

        self.assertEqual(civ6_brain.orders_db_path(run), run / "orders.sqlite")
        self.assertEqual(
            civ6_brain.orders_db_path(run, "/tmp/explicit-orders.sqlite"),
            Path("/tmp/explicit-orders.sqlite"),
        )

    def test_decider_passes_the_selected_strategy_and_reported_civilization(self) -> None:
        decider = civ6_brain.Decider(
            Path("/tmp/civvis-orders"), Path("/tmp/live-run"), "civvis", strategy="auto"
        )
        decider.set_civ("CIVILIZATION_ROME")

        command = decider.command()

        self.assertEqual(command[0], "/tmp/civvis-orders")
        self.assertIn("--fresh-board", command)
        self.assertEqual(command[command.index("--strategy") + 1], "auto")
        self.assertEqual(command[command.index("--civ") + 1], "CIVILIZATION_ROME")


class SeatCivTest(unittest.TestCase):
    """The civ Civilization VI dealt must reach the decider, or `--strategy auto`
    answers only half the brief and reports `per_civ:false`."""

    def _run(self, *lines: str) -> Path:
        run = Path(tempfile.mkdtemp())
        (run / "events.jsonl").write_text("\n".join(lines))
        return run

    def test_the_dealt_civ_is_read_and_stripped_to_the_league_name(self) -> None:
        run = self._run(
            '{"kind":"tiles","turn":1}',
            '{"kind":"seat","civ":"CIVILIZATION_ROME","leader":"LEADER_JULIUS_CAESAR"}',
        )
        self.assertEqual(civ6_brain.seat_civ(run), "Rome")

    def test_a_run_with_no_seat_event_yet_is_none_not_a_guess(self) -> None:
        """⚠ None, never a default. A wrong civ would narrow the league to a table
        that does not describe this game; no civ correctly falls back to the
        overall pick."""
        self.assertIsNone(civ6_brain.seat_civ(self._run('{"kind":"tiles","turn":1}')))

    def test_a_missing_run_directory_does_not_raise(self) -> None:
        """The decider starts lazily and this runs on the way in; an exception here
        would take the whole turn down over a naming detail."""
        self.assertIsNone(civ6_brain.seat_civ(Path("/nonexistent-run-dir")))

    def test_an_unprefixed_civ_is_passed_through(self) -> None:
        run = self._run('{"kind":"seat","civ":"Rome"}')
        self.assertEqual(civ6_brain.seat_civ(run), "Rome")


if __name__ == "__main__":
    unittest.main()
