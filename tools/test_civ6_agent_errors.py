#!/usr/bin/env python3
"""The census of the agent's swallowed errors adds up what the games reported."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import civ6_agent_errors as errors


def game(root: Path, name: str, rows: list[object]) -> None:
    path = root / name / "events.jsonl"
    path.parent.mkdir(parents=True)
    path.write_text("\n".join(row if isinstance(row, str) else json.dumps(row) for row in rows) + "\n")


def report(message: str, count: int = 1, where: str = "guard", turn: int = 5) -> dict:
    return {"kind": "agent_error", "message": message, "count": count, "where": where, "turn": turn}


class Census(unittest.TestCase):
    def setUp(self):
        self.holder = tempfile.TemporaryDirectory()
        self.root = Path(self.holder.name)

    def tearDown(self):
        self.holder.cleanup()

    def test_a_site_counts_games_and_its_worst_repeat(self):
        game(self.root, "a", [report("X:1: boom"), report("X:1: boom", 10, turn=9)])
        game(self.root, "b", [report("X:1: boom", turn=3)])
        read, reporting, sites = errors.census(self.root, 10)
        self.assertEqual((read, reporting, len(sites)), (2, 2, 1))
        self.assertEqual(len(sites[0].games), 2)
        self.assertEqual(sites[0].worst, 10)
        self.assertEqual(sites[0].first_turn, 3)

    def test_the_commonest_site_comes_first(self):
        game(self.root, "a", [report("rare"), report("common")])
        game(self.root, "b", [report("common")])
        _, _, sites = errors.census(self.root, 10)
        self.assertEqual([site.message for site in sites], ["common", "rare"])

    def test_a_quiet_game_counts_as_read_not_reporting(self):
        game(self.root, "a", [{"kind": "state", "turn": 1}])
        self.assertEqual(errors.census(self.root, 10)[:2], (1, 0))

    def test_a_torn_line_is_skipped(self):
        game(self.root, "a", ['{"kind": "agent_error", "mess', report("whole")])
        _, _, sites = errors.census(self.root, 10)
        self.assertEqual([site.message for site in sites], ["whole"])

    def test_only_the_newest_games_are_read(self):
        game(self.root, "a", [report("old")])
        game(self.root, "b", [report("new")])
        old = self.root / "a" / "events.jsonl"
        new = self.root / "b" / "events.jsonl"
        import os

        os.utime(old, (1000, 1000))
        os.utime(new, (2000, 2000))
        _, _, sites = errors.census(self.root, 1)
        self.assertEqual([site.message for site in sites], ["new"])


if __name__ == "__main__":
    unittest.main()
