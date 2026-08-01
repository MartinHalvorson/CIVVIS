#!/usr/bin/env python3

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("civvis_match_machine.py")
SPEC = importlib.util.spec_from_file_location("civvis_match_machine", MODULE_PATH)
machine = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = machine
SPEC.loader.exec_module(machine)


class MatchMachineTests(unittest.TestCase):
    def test_game_contract_is_standard_continents_free_for_all(self):
        command = machine.game_command(
            Path("/tmp/civvis"), Path("/tmp/league"), 42, 8870, visible=False
        )
        value = lambda flag: command[command.index(flag) + 1]
        self.assertEqual(value("--players"), "8")
        self.assertEqual((value("--width"), value("--height")), ("84", "54"))
        self.assertEqual(value("--city-states"), "12")
        self.assertEqual(value("--turns"), "500")
        self.assertEqual(value("--speed"), "standard")
        self.assertEqual(value("--map"), "continents")
        self.assertNotIn("--teams", command)
        self.assertIn("--league-record", command)
        self.assertIn("--no-open", command)

    def test_visible_game_is_the_only_command_that_opens_a_browser(self):
        visible = machine.game_command(Path("civvis"), Path("league"), 1, 2, visible=True)
        headless = machine.game_command(Path("civvis"), Path("league"), 1, 2, visible=False)
        self.assertNotIn("--no-open", visible)
        self.assertIn("--no-open", headless)

    def test_cpu_parser_uses_the_last_top_sample(self):
        report = "CPU usage: 10.0% user, 5.0% sys, 85.0% idle\nCPU usage: 20.0% user, 9.5% sys, 70.5% idle"
        self.assertEqual(machine.parse_top_cpu(report), 29.5)
        self.assertIsNone(machine.parse_top_cpu("not top"))

    def test_resource_ceiling_is_hard_and_resume_has_headroom(self):
        safe = machine.Resources(59.0, 20.0, 12.0, 0.0, False)
        edge = machine.Resources(70.0, 20.0, 12.0, 0.0, False)
        thermal = machine.Resources(1.0, 1.0, 1.0, 1.0, True)
        self.assertTrue(safe.comfortably_below(70))
        self.assertFalse(safe.overloaded(70))
        self.assertTrue(edge.overloaded(70))
        self.assertTrue(thermal.overloaded(70))

    def test_match_lookup_finds_a_concurrent_out_of_order_result(self):
        with tempfile.TemporaryDirectory() as directory:
            league = Path(directory)
            (league / "matches.csv").write_text(
                "round,seed,turns,victory,placements\n"
                "60,12,300,science,a@Trajan@Rome@0|b@Cleopatra@Egypt@1\n"
                "61,10,250,culture,b@Trajan@Rome@0|a@Cleopatra@Egypt@1\n",
                encoding="utf-8",
            )
            self.assertEqual(machine.match_row(league, 12)["victory"], "science")
            self.assertIsNone(machine.match_row(league, 99))

    def test_state_write_is_atomic_json(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "state.json"
            machine.atomic_json(target, {"active": 8})
            self.assertEqual(json.loads(target.read_text()), {"active": 8})
            self.assertFalse(target.with_suffix(".json.tmp").exists())


if __name__ == "__main__":
    unittest.main()
