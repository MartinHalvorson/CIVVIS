#!/usr/bin/env python3
"""Regression tests for live Civilization VI failure detectors."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_watchdogs


class DroppedUnitTest(unittest.TestCase):
    def report(self, notes: list[str]) -> dict:
        with tempfile.TemporaryDirectory() as directory:
            run = Path(directory)
            rows = [json.dumps({"note": note}) for note in notes]
            (run / "civvis_notes.jsonl").write_text("\n".join(rows) + "\n")
            return civ6_watchdogs.dropped_units(run)

    def test_bridge_managed_great_people_are_not_unordered_unit_failures(self) -> None:
        report = self.report([
            "dropped_units=2 [UNIT_GREAT_PROPHET@8,4:great_person "
            "UNIT_GREAT_SCIENTIST@2,3:great_person]"
        ])

        self.assertEqual(report["turns_with_drops"], 0)
        self.assertEqual(report["worst_on_one_turn"], 0)
        self.assertEqual(report["by_reason"], {})
        self.assertEqual(report["bridge_managed_great_person_observations"], 2)

    def test_real_drop_remains_loud_beside_a_managed_great_person(self) -> None:
        report = self.report([
            "dropped_units=2 [UNIT_GREAT_WRITER@8,4:great_person "
            "UNIT_UNKNOWN@2,3:untranslatable]"
        ])

        self.assertEqual(report["turns_with_drops"], 1)
        self.assertEqual(report["worst_on_one_turn"], 1)
        self.assertEqual(report["by_reason"], {"untranslatable": 1})
        self.assertEqual(report["bridge_managed_great_person_observations"], 1)


class ReachVerdictTest(unittest.TestCase):
    @staticmethod
    def report(first: int, last: int) -> dict:
        return {
            "idle_stack": {
                "reach": {
                    "furthest_ever": 7,
                    "furthest_ever_turn": first,
                    "last_turn": last,
                    "observed_turn_span": last - first,
                }
            }
        }

    def test_late_loaded_replay_is_not_mistaken_for_a_whole_game(self) -> None:
        verdicts = civ6_watchdogs.verdicts(self.report(89, 96), 0.35, 0.98)

        self.assertFalse(any("EMPIRE NEVER REACHED" in verdict for verdict in verdicts))

    def test_long_observation_still_detects_an_empire_that_never_reached(self) -> None:
        verdicts = civ6_watchdogs.verdicts(self.report(1, 60), 0.35, 0.98)

        self.assertTrue(any("EMPIRE NEVER REACHED" in verdict for verdict in verdicts))


if __name__ == "__main__":
    unittest.main()
