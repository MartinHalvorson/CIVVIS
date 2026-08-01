#!/usr/bin/env python3
"""Focused persistence checks for CIVVIS brain restarts."""

from __future__ import annotations

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
