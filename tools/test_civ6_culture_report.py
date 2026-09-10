#!/usr/bin/env python3
"""Synthetic retained host evidence; no Civilization VI install required."""
import json
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_culture_report as report


class CultureReportTest(unittest.TestCase):
    def test_only_completed_districts_and_distinct_major_markets_count(self):
        row = report.snapshot({
            "cities": [{"districts": [
                {"type": "DISTRICT_ACROPOLIS", "complete": True},
                {"type": "DISTRICT_THEATER", "complete": False},
            ], "great_works": [{"id": 3}], "buildings": [
                "BUILDING_AMPHITHEATER", "BUILDING_MUSEUM_ART", "BUILDING_LIBRARY"]}],
            "units": [{"kind": "UNIT_GREAT_WRITER"}, {"kind": "UNIT_GREAT_SCIENTIST"}],
            "rivals": [{"player": 2, "domestic_tourists": 100}],
            "trade_routes": [{"destination_player": p} for p in [0, 2, 2, 6]],
        }, 0)
        self.assertEqual(row["completed_theaters"], 1)
        self.assertEqual(row["great_works"], 1)
        self.assertEqual(row["amphitheaters"], 1)
        self.assertEqual(row["museums"], 1)
        self.assertEqual(row["cultural_great_people_on_map"], 1)
        self.assertEqual(row["known_major_route_markets"], [2])
        self.assertEqual(row["largest_known_rival_domestic"], 100)

    def test_missing_observations_are_not_zero(self):
        row = report.snapshot({}, 0)
        for key in ["tourism_per_turn", "cities", "great_works", "completed_theaters",
                    "known_major_route_markets", "largest_known_rival_domestic",
                    "amphitheaters", "museums", "cultural_great_people_on_map"]:
            self.assertIsNone(row[key], key)

    def test_segment_frames_outcomes_and_provenance(self):
        with TemporaryDirectory() as tmp:
            run = Path(tmp) / "civvis-20260910T000000Z-cont2"
            run.mkdir()
            summary = {
                "seat": {"leader": "LEADER_PERICLES", "local_player": 0,
                         "victory_types": [{"index": 8, "type": "VICTORY_CULTURE"}]},
                "victory_target": "culture", "decider_binaries": [{"revision": "abc"}],
                "outcome": {"kind": "victory", "won": True, "victory": 8},
            }
            path = run / "summary.json"
            path.write_text(json.dumps(summary))
            states = [{"kind": "state", "turn": 100, "frame": f, "tourism_per_turn": n}
                      for f, n in [(0, 0), (2, 4), (1, 0), (2, 6)]]
            (run / "events.jsonl").write_text(
                "\n".join(map(json.dumps, states)) + '\n{"partial":')
            row = report.report_run(run)
            self.assertEqual(row["game_family"], "civvis-20260910T000000Z")
            self.assertEqual(row["marks"]["100"]["tourism_per_turn"], 6)
            self.assertIsNone(row["marks"]["150"])
            self.assertEqual(row["first_observed_positive_tourism_turn"], 100)
            self.assertEqual(row["observed_turns"], 1)
            self.assertEqual(row["malformed_event_lines"], 1)
            self.assertEqual(row["decider_binaries"], [{"revision": "abc"}])
            self.assertTrue(row["won_culture"])
            for outcome, terminal, won in [
                ({"kind": "defeat", "ours": False}, False, None),
                ({"kind": "defeat", "ours": True}, True, False),
                ({"kind": "victory", "won": False, "victory": 8}, True, False),
                ({"kind": "victory", "won": True, "victory": 99}, True, None),
                ({}, False, None),
            ]:
                summary["outcome"] = outcome
                path.write_text(json.dumps(summary))
                row = report.report_run(run)
                self.assertEqual(row["terminal"], terminal)
                self.assertEqual(row["won_culture"], won)
            summary["seat"]["leader"] = "LEADER_TRAJAN"
            path.write_text(json.dumps(summary))
            self.assertIsNone(report.report_run(run))

    def test_missing_events_and_wrong_target(self):
        with TemporaryDirectory() as tmp:
            run = Path(tmp)
            self.assertIsNone(report.report_run(run))
            summary = {"seat": {"leader": "LEADER_PERICLES"}, "victory_target": "culture"}
            path = run / "summary.json"
            path.write_text(json.dumps(summary))
            row = report.report_run(run)
            self.assertIsNone(row["final_observed"])
            self.assertEqual(row["observed_turns"], 0)
            self.assertFalse(row["terminal"])
            summary["victory_target"] = "science"
            path.write_text(json.dumps(summary))
            self.assertIsNone(report.report_run(run))


if __name__ == "__main__":
    unittest.main()
