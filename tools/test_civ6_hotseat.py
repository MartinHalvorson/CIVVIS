#!/usr/bin/env python3
"""The hotseat launcher: settings, per-seat install, menu reading, turn record."""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_hotseat as hotseat  # noqa: E402
from civ6_control import gamelock  # noqa: E402
from civ6_control import install  # noqa: E402


def game_args(**overrides) -> argparse.Namespace:
    values = dict(tag="hotseat-test", difficulty="DIFFICULTY_KING", map="Pangaea.lua",
                  map_size="MAPSIZE_TINY", speed="GAMESPEED_ONLINE", max_turns=250,
                  victory="civvis", leader="LEADER_SALADIN", players=4,
                  auto_human=None, save_every=10, no_notify=True)
    values.update(overrides)
    return argparse.Namespace(**values)


class HotseatConfigTest(unittest.TestCase):
    def test_the_operator_is_seat_zero_and_jack_plays_the_rest(self) -> None:
        config = hotseat.hotseat_config(game_args(), Path("/tmp/run"), Path("/tmp/run/o.sqlite"))
        self.assertTrue(config["Hotseat"])
        self.assertEqual(config["HotseatSeats"], [1, 2, 3])
        self.assertEqual(config["HotseatHumanSeats"], [0])
        self.assertEqual(config["HotseatStubSeats"], [])
        self.assertEqual(config["HotseatNames"],
                         {"1": "CIVVIS jack 1", "2": "CIVVIS jack 2", "3": "CIVVIS jack 3"})
        # The live seat's controller, deciding through the orders channel.
        self.assertTrue(config["CivvisDecides"])
        self.assertEqual(config["OrdersDb"], "/tmp/run/o.sqlite")
        self.assertEqual(config["VictoryLane"], "civvis")
        # The game the operator asked for, every seat at their difficulty.
        self.assertEqual((config["MapScript"], config["MapSize"], config["GameSpeed"]),
                         ("Pangaea.lua", "MAPSIZE_TINY", "GAMESPEED_ONLINE"))
        self.assertEqual(config["PlayerDifficulty"], "DIFFICULTY_KING")
        self.assertEqual(config["Leader"], "LEADER_SALADIN")
        self.assertEqual(config["HotseatSaveEvery"], 10)
        # Nothing that changes how the game runs for the person at it.
        self.assertIsNone(config["DebugTimeScale"])
        self.assertIsNone(config["DebugTimeScaleAB"])
        self.assertIsNone(config["VSyncABTurns"])

    def test_the_test_stub_takes_the_operators_seat(self) -> None:
        config = hotseat.hotseat_config(game_args(auto_human="stub"), Path("/tmp/r"),
                                        Path("/tmp/r/o.sqlite"))
        self.assertEqual(config["HotseatHumanSeats"], [])
        self.assertEqual(config["HotseatStubSeats"], [0])
        self.assertEqual(hotseat.agent_seats(4, "stub"), [1, 2, 3, 0])
        gated = hotseat.hotseat_config(game_args(auto_human="gated"), Path("/tmp/r"),
                                       Path("/tmp/r/o.sqlite"))
        self.assertEqual((gated["HotseatHumanSeats"], gated["HotseatStubSeats"]), ([0], [0]))
        self.assertEqual(hotseat.agent_seats(3, False), [1, 2])

    def test_a_random_leader_pins_nothing(self) -> None:
        config = hotseat.hotseat_config(game_args(leader=None), Path("/tmp/r"),
                                        Path("/tmp/r/o.sqlite"))
        self.assertIsNone(config["Leader"])

    def test_the_turn_limit_follows_the_speed(self) -> None:
        self.assertEqual(hotseat.default_max_turns("GAMESPEED_ONLINE"), 250)
        self.assertEqual(hotseat.default_max_turns("GAMESPEED_STANDARD"), 500)

    def test_the_brain_is_the_live_seats_brain_pinned_to_one_binary(self) -> None:
        command = hotseat.brain_command(Path("/r/hs-p2"), Path("/r/o.sqlite"),
                                        Path("/r/civvis_orders"), "civvis")
        self.assertIn("civ6_brain.py", command[1])
        joined = " ".join(command)
        self.assertIn("--run-dir /r/hs-p2 --orders-db /r/o.sqlite --mode civvis", joined)
        self.assertIn("--github-refresh-seconds 0", joined)
        self.assertIn("--github-cache /r/runtime-cache", joined)
        # jack is the deployment genome, which civvis_orders plays unasked.
        self.assertNotIn("--strategy", command)

    def test_an_explicit_binary_must_exist(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / "civvis_orders"
            self.assertIsNone(hotseat.resolve_orders_bin(str(binary)))
            binary.write_text("#!/bin/sh\n")
            self.assertEqual(hotseat.resolve_orders_bin(str(binary)), binary)


class HotseatInstallTest(unittest.TestCase):
    def install(self, config: dict) -> Path:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        with patch.object(install, "check_syntax", return_value=None):
            install._write_mod(Path(tmp.name), config)
        return Path(tmp.name)

    def test_each_jack_seat_gets_its_own_agent_context(self) -> None:
        config = hotseat.hotseat_config(game_args(), Path("/tmp/r"), Path("/tmp/r/o.sqlite"))
        out = self.install(config)
        modinfo = (out / "CivvisControl.modinfo").read_text()
        tree = ET.fromstring(modinfo)
        contexts = [f.text for f in tree.findall(
            "./InGameActions/AddUserInterfaces[@id='CivvisControlAgentUI']/File")]
        self.assertEqual(contexts, [f"CivvisControlAgentSeat{s}.xml" for s in (1, 2, 3)])
        files = {f.text for f in tree.findall("./Files/File")}
        for seat in (1, 2, 3):
            self.assertIn(f"CivvisControlAgentSeat{seat}.lua", files)
            self.assertIn(f"CivvisControlAgentSeat{seat}.xml", files)
            script = (out / f"CivvisControlAgentSeat{seat}.lua").read_text()
            head = script.split("}\n", 1)[0]
            self.assertIn(f'RunTag = "hotseat-test-p{seat}",', head)
            self.assertIn(f"HotseatSeat = {seat},", head)
            self.assertIn(f'HotseatName = "CIVVIS jack {seat}",', head)
            self.assertIn("CivvisDecides = true,", head)
        replaced = {p.findtext("LuaContext") for p in
                    tree.findall("./InGameActions/ReplaceUIScript/Properties")}
        self.assertIn("PlayerChange", replaced)
        # The screens and the HUD keep the run's own tag.
        closer = (out / "CivvisControlAutoClose.lua").read_text().split("}\n", 1)[0]
        self.assertIn('RunTag = "hotseat-test",', closer)
        self.assertIn("HotseatHumanSeats = { 0 },", closer)
        frontend = (out / "CivvisControlConfig.xml").read_text()
        self.assertIn("<DefaultValue>DIFFICULTY_KING</DefaultValue>", frontend)

    def test_the_stub_seat_plays_the_fallback_ladder(self) -> None:
        config = hotseat.hotseat_config(game_args(auto_human="stub"), Path("/tmp/r"),
                                        Path("/tmp/r/o.sqlite"))
        out = self.install(config)
        stub = (out / "CivvisControlAgentSeat0.lua").read_text().split("}\n", 1)[0]
        self.assertIn("CivvisDecides = false,", stub)
        self.assertIn('RunTag = "hotseat-test-p0",', stub)

    def test_a_single_player_install_is_unchanged(self) -> None:
        out = self.install({"RunTag": "civvis-solo", "Play": True, "CivvisDecides": True})
        self.assertEqual((out / "CivvisControl.modinfo").read_text(),
                         (install.MOD_SOURCE / "CivvisControl.modinfo").read_text())
        self.assertEqual(list(out.glob("CivvisControlAgentSeat*")), [])
        frontend = (out / "CivvisControlConfig.xml").read_text()
        update = re.search(r'ParameterId="PlayerDifficulty".*?<DefaultValue>(\w+)<', frontend, re.S)
        self.assertEqual(update.group(1), "DIFFICULTY_PRINCE",
                         "the per-player row renders to the shipped default")

    def test_seat_tags_are_shared_by_installer_and_harness(self) -> None:
        self.assertEqual(install.hotseat_seat_tag("hs", 3), "hs-p3")
        self.assertEqual(install.hotseat_agent_seats({"Hotseat": True, "HotseatSeats": [1, 2],
                                                      "HotseatStubSeats": [0]}), [1, 2, 0])
        self.assertEqual(install.hotseat_agent_seats({"HotseatSeats": [1]}), [])


def ocr(*rows) -> list[dict]:
    return [{"text": t, "x": x, "y": y, "norm": hotseat._norm(t)} for t, x, y in rows]


#: The staging room as it read on this Mac (2026-11-17), window (0, 33, 864, 1084).
STAGING = ocr(("Players", 191, 419), ("Team", 302, 419), ("Difficulty", 507, 419),
              ("Player 1", 237, 435), ("Random Leader", 415, 439), ("Prince", 526, 440),
              ("™ Host", 201, 443),
              ("Player 2", 237, 460), ("Random Leader", 416, 465), ("Player", 195, 470),
              ("Random Leader", 236, 485), ("Computer", 201, 495),
              ("Random Leader", 236, 511), ("computer", 200, 521),
              ("Random Leader", 236, 536), ("Computer", 199, 547),
              ("ADD PLAYER", 432, 592), ("START GAME", 432, 859))
BOUNDS = (0, 33, 864, 1084)


class MenuReadingTest(unittest.TestCase):
    def test_seat_types_are_read_row_by_row(self) -> None:
        rows = hotseat.slot_rows(STAGING, BOUNDS)
        self.assertEqual([r["slot"] for r in rows],
                         ["host", "player", "computer", "computer", "computer"])
        self.assertEqual([r["y"] for r in rows], [443, 470, 495, 521, 547])

    def test_the_setup_page_is_read_back(self) -> None:
        rows = ocr(("Game Speed", 336, 562), ("Online", 439, 563), ("Map", 350, 576),
                   ("Pangaea", 440, 577), ("Map Size", 342, 590), ("Small", 440, 590))
        self.assertEqual(hotseat.value_beside(rows, "Map"), "Pangaea")
        problems = hotseat.check_host_settings(rows, game_args())
        self.assertEqual(problems, ["Map Size shows 'Small', wanted 'Tiny'"])
        rows[-1] = dict(rows[-1], text="Tiny", norm="tiny")
        self.assertEqual(hotseat.check_host_settings(rows, game_args()), [])


class DifficultyCellTest(unittest.TestCase):
    def test_each_seat_reads_its_own_difficulty(self) -> None:
        rows = STAGING + ocr(("King", 526, 466), ("Prince", 526, 492), ("Prince", 526, 517))
        cells = hotseat.difficulty_cells(rows, BOUNDS)
        self.assertEqual([c["norm"] for c in cells[:3]], ["prince", "king", "prince"])
        self.assertEqual([c["y"] for c in cells[:3]], [440, 466, 492])

    def test_another_seats_cell_is_not_a_list_entry(self) -> None:
        # Seat 1 already at King; seat 2's list open below it (2026-11-17 frames).
        before = STAGING + ocr(("King", 527, 441), ("Prince", 526, 467),
                               ("Prince", 526, 492), ("Prince", 526, 517))
        cells = hotseat.difficulty_cells(before, BOUNDS)
        listed = before + ocr(("Settler", 526, 479), ("Chieftain", 527, 492),
                              ("Warlord", 527, 504), ("King", 527, 528),
                              ("Emperor", 526, 540))
        picks = hotseat.dropdown_picks(listed, cells, cells[1], "king", 1084)
        self.assertEqual([(p["x"], p["y"]) for p in picks], [(527, 528)])


class MonitorTest(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.run_dir = Path(tmp.name)
        self.lines: list[str] = []
        self.monitor = hotseat.Monitor(self.run_dir, "hs", [1, 2, 3], game_args(),
                                       lambda m: self.lines.append(m))
        self.addCleanup(self.monitor.close)

    def base(self, **event) -> None:
        with (self.run_dir / "events.jsonl").open("a") as out:
            out.write(json.dumps(dict(event, run="hs")) + "\n")

    def test_turns_are_followed_from_the_mods_events(self) -> None:
        with patch.object(hotseat, "notify") as notified, patch("builtins.print"):
            self.base(kind="hotseat_turn", phase="begin", player=0, turn=4, human=True)
            self.monitor.poll()
            self.assertEqual((self.monitor.active, self.monitor.active_human), (0, True))
            notified.assert_not_called()  # --no-notify
            self.assertTrue(any("your turn" in line for line in self.lines))
            self.assertFalse(self.monitor.stalled(0.0), "a human turn never stalls")
            self.base(kind="hotseat_turn", phase="end", player=0, turn=4, human=True)
            self.base(kind="hotseat_turn", phase="begin", player=1, turn=4, human=False)
            self.monitor.poll()
            self.assertEqual(self.monitor.active, 1)
            self.assertTrue(self.monitor.stalled(-1.0))
            self.base(kind="hotseat_turn", phase="end", player=1, turn=4, human=False)
            self.monitor.poll()
        self.assertIsNone(self.monitor.active)
        self.monitor.timings.flush()
        rows = [json.loads(l) for l in (self.run_dir / "turns.jsonl").read_text().splitlines()]
        self.assertEqual([(r["player"], r["human"]) for r in rows], [(0, True), (1, False)])
        self.assertIsNotNone(rows[1]["seconds"])

    def test_a_seats_turn_record_is_counted(self) -> None:
        seat_dir = self.run_dir / "hs-p2"
        seat_dir.mkdir()
        with (seat_dir / "events.jsonl").open("a") as out:
            out.write(json.dumps({"kind": "turn", "turn": 3, "orders_applied": 7,
                                  "orders_source": "civvis", "cities": 1, "units": 4}) + "\n")
            out.write(json.dumps({"kind": "found", "turn": 3}) + "\n")
        self.monitor.poll()
        stats = self.monitor.stats[2]
        self.assertEqual((stats["turns"], stats["orders_applied"], stats["found"]), (1, 7, 1))
        self.assertEqual(stats["last"]["orders_source"], "civvis")


class ResumeTest(unittest.TestCase):
    def test_the_newest_hotseat_save_is_staged_under_one_name(self) -> None:
        import os
        with tempfile.TemporaryDirectory() as tmp:
            saves = Path(tmp)
            (saves / "auto").mkdir()
            old = saves / "hotseat-x-T010.Civ6Save"
            new = saves / "auto" / "AutoSave_0012.Civ6Save"
            old.write_text("old")
            new.write_text("new")
            os.utime(old, (1, 1))
            self.assertEqual(hotseat.newest_save(saves), new)
            staged = hotseat.stage_save(new, saves)
            self.assertEqual(staged.name, "civvis-hotseat-resume.Civ6Save")
            self.assertEqual(staged.read_text(), "new")
            # The staged copy is never itself "the newest save".
            os.utime(staged, (10 ** 10, 10 ** 10))
            self.assertEqual(hotseat.newest_save(saves), new)


class LockTest(unittest.TestCase):
    def test_the_operators_own_game_is_not_held_by_the_lane_halt(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            halt = Path(tmp) / "halt.json"
            halt.write_text(json.dumps({"pid": 1, "since": "x", "reason": "lanes off"}))
            with patch.object(gamelock, "LOCK", Path(tmp) / "lock"), \
                    patch.object(gamelock, "OPERATOR_HALT", halt), \
                    patch.object(gamelock, "foreign_run", return_value=None):
                self.assertFalse(gamelock.acquire("lane"))
                self.assertTrue(gamelock.acquire("hotseat-x", honour_operator_halt=False))
                gamelock.release()


if __name__ == "__main__":
    unittest.main()
