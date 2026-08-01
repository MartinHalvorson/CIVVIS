#!/usr/bin/env python3
"""Focused persistence checks for CIVVIS brain restarts."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_brain  # noqa: E402


class Civ6BrainTest(unittest.TestCase):
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
