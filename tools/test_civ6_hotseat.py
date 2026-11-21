#!/usr/bin/env python3
"""The hotseat launcher: settings, per-seat install, menu reading, turn record."""

from __future__ import annotations

import argparse
import json
import os
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


class NearMatchTest(unittest.TestCase):
    """OCR misreads a letter now and then; a label is still found."""

    def test_a_misread_letter_still_finds_the_button(self) -> None:
        # Read on a 1710x1107 MacBook Air (2026-11-19): setup stalled here.
        rows = ocr(("Load Game", 520, 1088), ("Confirm Settinas", 1194, 1088))
        self.assertEqual([r["text"] for r in hotseat.find(rows, "Confirm Settings")],
                         ["Confirm Settinas"])

    def test_an_exact_reading_wins_over_a_near_one(self) -> None:
        rows = ocr(("START GANE", 900, 700), ("START GAME", 900, 740))
        self.assertEqual([r["y"] for r in hotseat.find(rows, "START GAME")], [740])

    def test_short_labels_match_exactly_only(self) -> None:
        rows = ocr(("OR", 10, 10), ("Kong", 10, 30), ("YEA", 10, 50))
        for label in ("OK", "King", "YES"):
            self.assertEqual(hotseat.find(rows, label), [], label)

    def test_a_different_label_is_not_a_misread(self) -> None:
        rows = ocr(("Load Game", 10, 10), ("STAGING ROOM", 10, 30))
        self.assertEqual(hotseat.find(rows, "Load Configuration"), [])
        self.assertEqual(hotseat.find(rows, "START GAME"), [])

    def test_the_setup_page_reads_back_through_a_misread(self) -> None:
        rows = ocr(("Game Speed", 336, 562), ("Onlime", 439, 563), ("Map", 350, 576),
                   ("Pangaea", 440, 577), ("Map Size", 342, 590), ("Tiny", 440, 590))
        self.assertEqual(hotseat.check_host_settings(rows, game_args()), [])


class PersonOptionsTest(unittest.TestCase):
    """A person plays a hotseat game: only the front end's options move."""

    def test_how_the_game_plays_and_looks_stays_theirs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            user = Path(tmp)
            (user / "AppOptions.txt").write_text(
                "PlayIntroVideo 1\nAcceptedUnknownDevice 0\nAcceptedOutdatedDriver 0\n")
            mine = "AutoSaveFrequency 1\nAutoSaveKeepCount 10\nPlayHistoricMomentAnimation 1\n"
            (user / "UserOptions.txt").write_text(mine)
            graphics = "EnableShadows 1\nEnableCloudShadows 1\nVSync 1\n"
            (user / "GraphicsOptions.txt").write_text(graphics)
            with patch.object(hotseat.env, "user_dir", return_value=user), \
                    patch.object(hotseat.env, "game_pids", return_value=[]):
                applied = hotseat.apply_person_options(lambda _message: None)
            self.assertEqual(set(applied), {"AppOptions.txt"})
            self.assertEqual((user / "UserOptions.txt").read_text(), mine)
            self.assertEqual((user / "GraphicsOptions.txt").read_text(), graphics)
            self.assertIn("PlayIntroVideo 0", (user / "AppOptions.txt").read_text())


class FitOnScreenTest(unittest.TestCase):
    """A window as tall as the display is lifted until its bottom shows."""

    def fit(self, bounds, screen=(1710, 1107)):
        placed = []
        frame = hotseat.fit_on_screen(get_window=lambda: bounds, get_desktop=lambda: screen,
                                      place=placed.append)
        return frame, placed

    def test_a_full_height_window_under_the_menu_bar_moves_up(self) -> None:
        # Measured 2026-11-19: windowed Civ VI kept 1710x1107 at y=33, and the
        # Confirm Settings row sat below the screen.
        frame, placed = self.fit((0, 33, 1710, 1107))
        self.assertEqual(frame, (0, 0, 1710, 1107))
        self.assertEqual(placed, [(0, 0, 1710, 1107)])

    def test_a_window_that_fits_is_left_alone(self) -> None:
        self.assertEqual(self.fit((0, 33, 1710, 1074)), (None, []))
        self.assertEqual(self.fit((0, 0, 1710, 1107)), (None, []))

    def test_only_as_far_as_needed(self) -> None:
        frame, _ = self.fit((40, 200, 1200, 1000))
        self.assertEqual(frame, (40, 107, 1200, 1000))

    def test_no_window_no_move(self) -> None:
        self.assertEqual(self.fit(None), (None, []))


class StaleWorkerTest(unittest.TestCase):
    """A seat never ends up with two decision workers."""

    def test_a_leftover_worker_on_the_seat_is_stopped(self) -> None:
        import subprocess
        with tempfile.TemporaryDirectory() as tmp:
            seat_dir = Path(tmp) / "run-p1"
            seat_dir.mkdir()
            # Stands in for an orphaned `civ6_brain.py --run-dir <seat_dir>`.
            script = Path(tmp) / "civ6_brain.py"
            script.write_text("import time\ntime.sleep(60)\n")
            worker = subprocess.Popen([sys.executable, str(script), "--run-dir", str(seat_dir)])
            try:
                (seat_dir / "brain.pid").write_text(f"{worker.pid}\n")
                self.assertEqual(hotseat.stop_stale_worker(seat_dir, grace_s=2.0), worker.pid)
                self.assertIsNotNone(worker.wait(timeout=5))
            finally:
                if worker.poll() is None:
                    worker.kill()

    def test_a_reused_pid_is_not_touched(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            seat_dir = Path(tmp) / "run-p1"
            seat_dir.mkdir()
            (seat_dir / "brain.pid").write_text(f"{os.getpid()}\n")
            self.assertIsNone(hotseat.stop_stale_worker(seat_dir))

    def test_no_record_no_action(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            self.assertIsNone(hotseat.stop_stale_worker(Path(tmp)))


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




class UnattendedTableTest(unittest.TestCase):
    """Operator 2026-11-21: "2 civvis agents and 4 deity ais", unattended."""

    def test_civvis_plays_the_first_seats_and_no_seat_is_a_persons(self) -> None:
        args = game_args(players=6, civvis_seats=2, leader=None)
        config = hotseat.hotseat_config(args, Path("/tmp/run"), Path("/tmp/run/o.sqlite"))
        self.assertEqual(config["HotseatSeats"], [0, 1])
        self.assertEqual(config["HotseatHumanSeats"], [])
        self.assertEqual(config["HotseatStubSeats"], [])
        self.assertEqual(config["HumanPlayers"], 2)
        self.assertEqual(hotseat.agent_seats(6, None, 2), [0, 1])
        self.assertEqual(hotseat.jack_seats(4), [1, 2, 3], "the operator's table is unchanged")

    def test_play_flags_reach_every_seats_settings(self) -> None:
        args = game_args(players=6, civvis_seats=2, leader=None,
                         play_flags="--dv-join-winner --luxury-quote-proposals")
        config = hotseat.hotseat_config(args, Path("/tmp/run"), Path("/tmp/run/o.sqlite"))
        plain = hotseat.hotseat_config(game_args(players=6, civvis_seats=2, leader=None),
                                       Path("/tmp/run"), Path("/tmp/run/o.sqlite"))
        changed = {k for k in config if config[k] != plain.get(k)}
        self.assertTrue(changed, "the flags change the seats' settings")
        self.assertTrue(any("Dv" in k or "Luxury" in k for k in changed), changed)

    def test_the_staging_room_wants_two_humans_and_four_computers(self) -> None:
        want = hotseat.wanted_slots(6, 2)
        self.assertEqual(want, ["host", "player"] + ["computer"] * 4)
        default = [r["slot"] for r in hotseat.slot_rows(STAGING, BOUNDS)]
        self.assertFalse(hotseat.slots_match(default, want), "one AI seat short")
        self.assertTrue(hotseat.slots_match(default + ["computer"], want))
        self.assertEqual(hotseat.wanted_slots(4), ["host", "player", "player", "player"])

    def monitor(self, run_dir: Path) -> "hotseat.Monitor":
        monitor = hotseat.Monitor(run_dir, "hs", [0, 1], game_args(players=6, civvis_seats=2),
                                  lambda m: None)
        self.addCleanup(monitor.close)
        return monitor

    def seat_event(self, run_dir: Path, seat: int, **event) -> None:
        seat_dir = run_dir / f"hs-p{seat}"
        seat_dir.mkdir(exist_ok=True)
        with (seat_dir / "events.jsonl").open("a") as out:
            out.write(json.dumps(event) + "\n")

    def test_either_civvis_seat_winning_is_our_win(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            monitor = self.monitor(run_dir)
            self.seat_event(run_dir, 0, kind="seat", civ="CIVILIZATION_GAUL", team=0)
            self.seat_event(run_dir, 1, kind="seat", civ="CIVILIZATION_ROME", team=1)
            self.seat_event(run_dir, 1, kind="victory", team=1, victory=3, turn=180)
            monitor.poll()
            self.assertEqual(monitor.result["ours"], True)
            self.assertEqual(monitor.result["winner_seat"], 1)
            self.assertEqual(monitor.seat_info[0]["civ"], "CIVILIZATION_GAUL")

    def test_a_rival_victory_is_a_loss(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            monitor = self.monitor(run_dir)
            with (run_dir / "events.jsonl").open("a") as out:
                out.write(json.dumps({"kind": "victory", "team": 4, "victory": 0, "turn": 190}) + "\n")
            monitor.poll()
            self.assertEqual((monitor.result["ours"], monitor.result["team"]), (False, 4))

    def test_a_stalled_civvis_seat_presses_next_turn(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            monitor = self.monitor(Path(tmp))
            rows = [{"text": "NEXT TURN", "norm": "next turn", "x": 820, "y": 770}]
            clicks = []
            with patch.object(hotseat, "click", lambda _log, point: clicks.append(point)):
                monitor.active, monitor.active_human = 1, False
                self.assertTrue(hotseat.end_stalled_turn(monitor, lambda m: None, rows))
                self.assertEqual(clicks, [(820, 770)])
                monitor.active_human = True
                self.assertFalse(hotseat.end_stalled_turn(monitor, lambda m: None, rows),
                                 "never a person's turn")
                monitor.active, monitor.active_human = 4, False
                self.assertFalse(hotseat.end_stalled_turn(monitor, lambda m: None, rows),
                                 "never a Firaxis AI seat")
                monitor.active = 0
                self.assertFalse(hotseat.end_stalled_turn(monitor, lambda m: None, []),
                                 "no NEXT TURN on the map")
            self.assertEqual(len(clicks), 1)

    def test_the_huds_seat_watch_is_told_in_the_log(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            lines: list[str] = []
            monitor = hotseat.Monitor(run_dir, "hs", [0, 1], game_args(players=6, civvis_seats=2),
                                      lines.append)
            self.addCleanup(monitor.close)
            with (run_dir / "events.jsonl").open("a") as out:
                for event in (
                    {"kind": "seat_quiet", "turn": 61, "player": 1, "quiet": 45, "paused": False,
                     "sent": False, "blocker": None, "handover_hidden": True, "controller_ran": True},
                    {"kind": "seat_quiet_end_turn", "turn": 61, "player": 1, "asked": True, "forces": 1},
                    {"kind": "hotseat_pause_released", "turn": 180, "player": 0, "waited": 6,
                     "still_paused": False},
                ):
                    out.write(json.dumps(event) + "\n")
            monitor.poll()
            self.assertEqual(monitor.seat_stalls, 1)
            text = "\n".join(lines)
            self.assertIn("seat 1 quiet for 45 s on its own turn", text)
            self.assertIn("the HUD ended seat 1's quiet turn (asked True, request 1)", text)
            self.assertIn("seat 0's hand-over pause stood 6 s with no Start Turn on screen", text)

    def test_the_table_is_lost_only_when_both_seats_are_eliminated(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            monitor = self.monitor(run_dir)
            self.seat_event(run_dir, 0, kind="defeat", ours=True, player=0, turn=60)
            monitor.poll()
            self.assertIsNone(monitor.result)
            self.seat_event(run_dir, 1, kind="defeat", ours=True, player=1, turn=90)
            monitor.poll()
            self.assertEqual(monitor.result, {"kind": "defeat", "turn": 90, "ours": False})

if __name__ == "__main__":
    unittest.main()
