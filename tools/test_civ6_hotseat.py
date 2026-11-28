#!/usr/bin/env python3
"""The hotseat launcher: settings, per-seat install, menu reading, turn record."""

from __future__ import annotations

import argparse
import contextlib
import io
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


class StagingRoom:
    """The staging room as the harness meets it: seats that read as live ones
    did on 2026-11-27 (window (0, 0, 1710, 1107): a name line every 45 points
    from 223, the seat type 17 under it; an open seat reads only "Open" on its
    name line, with its list a line lower), lists that open on a click on a
    seat's type line, and a screen reader that can be told to drop a row."""

    BOUNDS = (0, 0, 1710, 1107)

    def __init__(self, kinds: list[str], dropped: dict[int, int] | None = None):
        self.kinds = list(kinds)
        self.open_list: int | None = None
        self.dropped = dict(dropped or {})      # reading number -> seat index not read
        self.readings = 0
        self.clicks: list[tuple[int, int]] = []
        self.parked = 0

    def type_y(self, index: int) -> int:
        return 240 + 45 * index + (1 if self.kinds[index] == "open" else 0)

    def read(self, _run_dir, name: str, _fallback=None):
        staging = name.startswith("staging-")
        if staging:
            self.readings += 1
        rows = [("Players", 427, 196), ("ADD PLAYER", 857, 240 + 45 * len(self.kinds) + 20),
                ("START GAME", 855, 1090)]
        for i, kind in enumerate(self.kinds):
            if staging and self.dropped.get(self.readings) == i:
                continue
            if kind == "open":
                rows.append(("Open", 508, 223 + 45 * i))
            else:
                rows.append((f"Player {i + 1}" if kind != "computer" else "Random Leader", 508, 223 + 45 * i))
                rows.append((kind.capitalize(), 442, 240 + 45 * i))
        if self.open_list is not None:
            base = self.type_y(self.open_list)
            rows += [("Closed", 509, base + 46), ("Human", 508, base + 69)]
        return self.BOUNDS, ocr(*rows)

    def click(self, _log, point) -> None:
        x, y = point
        self.clicks.append((x, y))
        if self.open_list is not None:
            base, index = self.type_y(self.open_list), self.open_list
            self.open_list = None
            if abs(y - (base + 46)) <= 8:
                del self.kinds[index]
            elif abs(y - (base + 69)) <= 8:
                self.kinds[index] = "player"
            return
        if abs(y - (240 + 45 * len(self.kinds) + 20)) <= 8 and x > 700:
            self.kinds.append("open")
            return
        for i in range(len(self.kinds)):
            if abs(y - self.type_y(i)) <= 8 and 412 <= x <= 600:
                self.open_list = i


class SeatingTest(unittest.TestCase):
    def seat(self, room: StagingRoom, players: int) -> tuple[bool, list[str]]:
        said: list[str] = []
        with patch.object(hotseat, "read_screen", side_effect=room.read), \
                patch.object(hotseat, "click", side_effect=room.click), \
                patch.object(hotseat.time, "sleep"), \
                patch.object(hotseat.play, "ensure_game_frontmost", return_value=True) as front, \
                patch.object(hotseat.play, "park_setup_pointer",
                             side_effect=lambda bounds: setattr(room, "parked", room.parked + 1)), \
                patch.object(hotseat.play.macos_input, "press_key"):
            done = hotseat.seat_the_players(Path("/tmp/r"), room.BOUNDS, players, said.append)
            self.fronted = front.call_count
        return done, said

    def test_six_computer_seats_become_six_hotseat_humans(self) -> None:
        room = StagingRoom(["host"] + ["computer"] * 5)
        done, said = self.seat(room, 6)
        self.assertTrue(done, said)
        self.assertEqual(room.kinds, ["host"] + ["player"] * 5)
        self.assertIn("staging room: 6 hotseat seats and 0 Firaxis AI, verified", said[-1])
        self.assertEqual(self.fronted, room.readings, "the game is in front for every reading")
        self.assertEqual(room.parked, 5, "the pointer is parked after every pick")

    def test_a_row_dropped_for_one_reading_adds_no_seat(self) -> None:
        # 2026-11-27: one row unread for a frame made five seats of six, ADD
        # PLAYER was pressed, and a seventh seat stood in the room.
        room = StagingRoom(["host"] + ["computer"] * 5, dropped={1: 4, 4: 2})
        done, said = self.seat(room, 6)
        self.assertTrue(done, said)
        self.assertEqual(room.kinds, ["host"] + ["player"] * 5, "six seats, never seven")
        self.assertFalse([c for c in room.clicks if c[0] > 700], "ADD PLAYER was never pressed")

    def test_a_table_larger_than_the_map_offers_is_still_filled(self) -> None:
        room = StagingRoom(["host", "computer", "computer", "computer"])
        done, said = self.seat(room, 6)
        self.assertTrue(done, said)
        self.assertEqual(room.kinds, ["host"] + ["player"] * 5)
        self.assertEqual(len([c for c in room.clicks if c[0] > 700]), 2, "two seats added, on second readings")

    def test_an_open_seat_too_many_is_closed_by_its_own_list(self) -> None:
        # Its list is a line under its name: a click on the name never opened it.
        room = StagingRoom(["host", "player", "player", "player", "computer", "player", "open"])
        done, said = self.seat(room, 6)
        self.assertTrue(done, said)
        self.assertEqual(room.kinds, ["host"] + ["player"] * 5)
        first = room.clicks[0]
        self.assertEqual(first, (508 + 30, 223 + 45 * 6 + int(hotseat.OPEN_LIST_DROP * 1107)),
                         "the open seat's list, not its name")
        self.assertFalse([line for line in said if "did not offer" in line], said)

    def test_a_seated_seat_too_many_is_closed_on_its_type_line(self) -> None:
        room = StagingRoom(["host", "computer", "computer"])
        done, said = self.seat(room, 2)
        self.assertTrue(done, said)
        self.assertEqual(room.kinds, ["host", "player"])
        self.assertEqual(room.clicks[0], (442 + 30, 240 + 45 * 2))


class MenuReadingTest(unittest.TestCase):
    def test_seat_types_are_read_row_by_row(self) -> None:
        rows = hotseat.slot_rows(STAGING, BOUNDS)
        self.assertEqual([r["slot"] for r in rows],
                         ["host", "player", "computer", "computer", "computer"])
        self.assertEqual([r["y"] for r in rows], [443, 470, 495, 521, 547])

    def test_a_misread_seat_word_is_still_that_seat(self) -> None:
        # The first 6-player table (2026-11-27, Small map, window (0, 0, 1710, 1107)):
        # the fifth row's "Computer" read "Comouter" on every reading. Uncounted,
        # it made five seats of six, and ADD PLAYER was pressed for a seventh.
        six = ocr(("Players", 427, 196), ("Team", 627, 194), ("Player 1", 508, 223), ("HOST", 449, 240),
                  ("Player 2", 508, 268), ("Player", 432, 286),
                  ("Random Leader", 508, 313), ("Computer", 442, 331),
                  ("Random Leader", 507, 358), ("Computer", 441, 376),
                  ("Random Leader", 507, 403), ("Comouter", 441, 421),
                  ("Random Leader", 507, 450), ("Computer", 442, 467),
                  ("ADD PLAYER", 857, 505), ("START GAME", 855, 1090))
        rows = hotseat.slot_rows(six, (0, 0, 1710, 1107))
        self.assertEqual([r["slot"] for r in rows],
                         ["host", "player", "computer", "computer", "computer", "computer"])
        self.assertEqual(rows[4]["y"], 421, "the misread row, where it is")
        self.assertTrue(hotseat.slots_match([r["slot"] for r in rows][:2] + ["player"] * 4,
                                            hotseat.wanted_slots(6)))
        for read, want in {"computer": "computer", "comouter": "computer", "compuler": "computer",
                           "plaver": "player", "closad": "closed", "humon": "human",
                           "host": "host", "open": "open",
                           # Short words match exactly only; a name is not a seat.
                           "hast": None, "opan": None, "leader": None, "2": None, "": None,
                           "commuters": None}.items():
            self.assertEqual(hotseat.slot_word(read), want, read)
        # The seat's name line is never a seat type, misread or not.
        self.assertEqual([r["slot"] for r in hotseat.slot_rows(STAGING, BOUNDS)],
                         ["host", "player", "computer", "computer", "computer"])

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


class ParkUpperLeftTest(unittest.TestCase):
    """Once setup is over the game leaves the setup half for the saved corner."""

    SAVED = (0, 33, 864, 542)

    def park(self, windows, frame=SAVED, settle_s=3.0):
        placed, lines, now = [], [], [0.0]
        seen = iter(windows)

        def sleep(seconds):
            now[0] += seconds

        with patch.object(hotseat.play.window_layout, "frame_for",
                          lambda quadrant, screen: frame if quadrant == "upper-left" else None), \
                patch.object(hotseat.play, "GAME_CONFIGURED", False):
            got = hotseat.park_upper_left(
                lines.append, get_window=lambda: next(seen, windows[-1]),
                get_desktop=lambda: (1728, 1117), place=placed.append, settle_s=settle_s,
                poll_s=1.0, clock=lambda: now[0], sleep=sleep)
            configured = hotseat.play.GAME_CONFIGURED
        return got, placed, lines, configured

    def test_the_full_left_side_goes_to_the_saved_upper_left_frame(self) -> None:
        got, placed, lines, configured = self.park([(0, 33, 864, 1084), self.SAVED])
        self.assertEqual(got, self.SAVED)
        self.assertEqual(placed, [self.SAVED])
        self.assertTrue(configured)
        self.assertIn("upper-left", lines[0])

    def test_a_resize_while_the_map_loads_is_put_back(self) -> None:
        _, placed, _, _ = self.park([(0, 33, 864, 1084), self.SAVED, (0, 0, 1728, 1117), self.SAVED])
        self.assertEqual(placed, [self.SAVED, self.SAVED])

    def test_a_window_already_in_the_corner_is_left_alone(self) -> None:
        _, placed, lines, _ = self.park([(0, 34, 864, 541)])
        self.assertEqual((placed, lines), ([], []))

    def test_no_saved_frame_moves_nothing(self) -> None:
        got, placed, _, configured = self.park([(0, 33, 864, 1084)], frame=None)
        self.assertEqual((got, placed, configured), (None, [], False))


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


#: What `civvis_orders` writes first on stderr: its genome (the live game's
#: decider, 0512deb6, asked with the whole live list, 2026-11-28; shortened).
GENOME_LINE = json.dumps({"civ": None, "forced": ["age-closer-2", "recon-replacement"], "kind": "genome",
                          "lane": None, "ledger_withheld": ["war-reinforcement"]})
#: ...and what it says when a `--with` is one it does not know (the same binary).
REFUSED = ('civvis-orders: unknown --with treatment "no-such-tag"; this binary can force: '
           "war-reinforcement, come-ashore, recon-replacement, age-closer-2\n")


class LiveGenomeTest(unittest.TestCase):
    """The live six-player game (hotseat-20261128T031456Z): every seat's
    brain.log said `forced=none`, every why.log the stock genes; 182 of the
    ladder's 285 live tags were off (`recon-replacement` among them: no seat
    built a Scout), and its config held `StalledOperationRelease` false, so a
    stalled Warrior held seat 3 for 7.9 s of the person's wait on turn 15."""

    def test_the_list_is_resolved_as_the_ladders_supervisor_resolves_it(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo, local, empty = Path(tmp) / "repo.txt", Path(tmp) / "local", Path(tmp) / "empty"
            repo.write_text("recon-replacement,monument-first\n")
            empty.write_text("")

            def resolve(environ, local_text=None):
                if local_text is None:
                    local.unlink(missing_ok=True)
                else:
                    local.write_text(local_text)
                return hotseat.live_treatments(environ, repo, local)

            self.assertEqual(resolve({}), (["recon-replacement", "monument-first"], "deploy/live-force-on.txt"))
            # A host's own list wins while it is not empty; an empty one is no override.
            self.assertEqual(resolve({}, "chop-for-expansion")[0], ["chop-for-expansion"])
            self.assertEqual(resolve({}, "  \n")[0], ["recon-replacement", "monument-first"])
            # The environment's, and a named file's even when empty (the stock genome).
            self.assertEqual(resolve({"CIVVIS_WITH": "a,b,a"}, "x")[0], ["a", "b"])
            self.assertEqual(resolve({"CIVVIS_WITH_FILE": str(empty)}, "x"), ([], f"CIVVIS_WITH_FILE ({empty})"))
            repo.unlink()
            self.assertEqual(resolve({}), ([], "no live list in this install"))

    @staticmethod
    def decider(*answers):
        """A `civvis_orders` that answers each ask in turn: (exit code, stderr)."""
        asked = []

        def run(command, **_):
            asked.append([word for word in command[3:] if word != "--with"])
            code, stderr = answers[min(len(asked), len(answers)) - 1]
            return argparse.Namespace(returncode=code, stderr=stderr, stdout="")
        return run, asked

    def test_the_decider_is_asked_first_and_keeps_only_what_it_can_play(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run, asked = self.decider((0, GENOME_LINE + "\n"))
            armed = hotseat.armed_treatments(Path("/bin/civvis_orders"), ["age-closer-2", "recon-replacement",
                                                                         "monument-first"], Path(tmp), run)
            self.assertEqual(armed["with"], ["age-closer-2", "recon-replacement", "monument-first"])
            self.assertEqual(armed["forced"], ["age-closer-2", "recon-replacement"], "the decider's own word")
            self.assertEqual(asked, [["age-closer-2", "recon-replacement", "monument-first"]])
            self.assertFalse((Path(tmp) / "genome-check").exists(), "the empty board is cleaned up")
            # A tag this binary does not know would kill every worker at its start.
            # Only the tags it names go: `monument-first` is not in its "can force"
            # list because its deployment genome already plays it, and it is kept.
            held = ('civvis-orders: --with treatment "not-held" is not a ledger-held live treatment or '
                    'held-off opt-in (tag "not-held"); this binary can force: recon-replacement\n')
            run, asked = self.decider((2, REFUSED), (2, held), (0, GENOME_LINE))
            armed = hotseat.armed_treatments(Path("/bin/civvis_orders"),
                                             ["recon-replacement", "no-such-tag", "monument-first", "not-held"],
                                             Path(tmp), run)
            self.assertEqual((armed["with"], armed["dropped"]),
                             (["recon-replacement", "monument-first"], ["no-such-tag", "not-held"]))
            self.assertEqual(asked[-1], ["recon-replacement", "monument-first"])
            # A refusal that never ends is not asked forever.
            run, asked = self.decider((2, REFUSED))
            armed = hotseat.armed_treatments(Path("/bin/x"), ["no-such-tag"] * 1 + ["a"], Path(tmp), run)
            self.assertEqual(armed["with"], [])
            self.assertLessEqual(len(asked), hotseat.MOST_REFUSED)
            # Refused with no list of its own, or not runnable: the stock genome, said.
            run, _ = self.decider((2, "civvis-orders: something else\n"))
            armed = hotseat.armed_treatments(Path("/bin/x"), ["recon-replacement"], Path(tmp), run)
            self.assertEqual(armed["with"], [])
            self.assertIn("the decider refused the list (civvis-orders: something else)", armed["note"])

            def broken(command, **_):
                raise OSError("no such file")
            armed = hotseat.armed_treatments(Path("/bin/x"), ["recon-replacement"], Path(tmp), broken)
            self.assertEqual((armed["with"], armed["dropped"]), ([], ["recon-replacement"]))
            run, _ = self.decider((2, "civvis-orders: --with treatment(s) x already ship in the "
                                      "deployment genome; name at least one\n"))
            self.assertEqual(hotseat.armed_treatments(Path("/bin/x"), ["x"], Path(tmp), run)["with"], [])

    def test_every_seats_worker_and_the_mod_play_the_live_arms(self) -> None:
        command = hotseat.brain_command(Path("/r/hs-p2"), Path("/r/o.sqlite"), Path("/r/civvis_orders"),
                                        "culture", ["recon-replacement", "culture-seat-allies"])
        self.assertEqual(command[-4:], ["--with", "recon-replacement", "--with", "culture-seat-allies"])
        with tempfile.TemporaryDirectory() as tmp:
            arms = Path(tmp) / "live-mod-arms.txt"
            arms.write_text("stalled-operation-release\ndebug-timescale-3\npoll-20ms\n")
            with patch.object(hotseat, "MOD_ARMS_FILE", arms):
                live = hotseat.hotseat_config(game_args(genome="live", treatments=["culture-seat-allies"]),
                                              Path("/tmp/r"), Path("/tmp/r/o.sqlite"))
                stock = hotseat.hotseat_config(game_args(genome="stock", treatments=[]),
                                               Path("/tmp/r"), Path("/tmp/r/o.sqlite"))
        self.assertTrue(live["StalledOperationRelease"])
        self.assertEqual(live["OrdersPeekSeconds"], 0.02)
        # The key a live gene switches on in the mod, as on the ladder (`civ6_play.build_config`).
        self.assertTrue(live["CultureAllies"])
        # The ladder's debug timescale is not a person's game's.
        self.assertIsNone(live["DebugTimeScale"])
        self.assertIsNone(live["DebugTimeScaleAB"])
        self.assertFalse(stock["StalledOperationRelease"])
        self.assertIsNone(stock["OrdersPeekSeconds"])
        self.assertFalse(stock["CultureAllies"])

    def test_a_run_says_and_keeps_what_its_seats_play_and_an_attach_plays_the_same(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            lines: list[str] = []
            args = game_args(genome="live")
            run, _ = self.decider((2, REFUSED), (0, GENOME_LINE))
            with patch.object(hotseat, "live_treatments",
                              return_value=(["age-closer-2", "recon-replacement", "no-such-tag"],
                                            "deploy/live-force-on.txt")), \
                    patch.object(hotseat.subprocess, "run", side_effect=run):
                hotseat.live_genome(args, Path("/bin/civvis_orders"), run_dir, lines.append)
            self.assertEqual(args.treatments, ["age-closer-2", "recon-replacement"])
            self.assertIn("decider genome: the live ladder's, from deploy/live-force-on.txt: 2 of 3 listed "
                          "genes armed, 2 of them beyond this decider's deployment genome", lines[0])
            self.assertIn("cannot play 1 listed gene(s), so they are off: no-such-tag", lines[1])
            # What a rated game was against is on its record.
            monitor = hotseat.Monitor(run_dir, "hs", [1], game_args(players=2, name="dev", victories_off=[]),
                                      lambda m: None)
            self.addCleanup(monitor.close)
            record = hotseat.table_record(monitor, args, [1], {})
            self.assertEqual((record["genome"]["genome"], record["genome"]["forced"]),
                             ("live", ["age-closer-2", "recon-replacement"]))
            # A re-attached harness restarts the workers on the same list.
            again = game_args()
            hotseat.kept_genome(again, run_dir)
            self.assertEqual((again.genome, again.treatments), ("live", ["age-closer-2", "recon-replacement"]))
            # A run from before this armed nothing, and its re-attach arms nothing.
            hotseat.kept_genome(again, run_dir / "older")
            self.assertEqual((again.genome, again.treatments), ("stock", []))
            # `--genome stock` asks nothing and arms nothing.
            stock, lines = game_args(genome="stock"), []
            with patch.object(hotseat.subprocess, "run") as asked:
                hotseat.live_genome(stock, Path("/bin/civvis_orders"), run_dir, lines.append)
            asked.assert_not_called()
            self.assertEqual(stock.treatments, [])
            self.assertIn("decider genome: stock", lines[0])

    def test_live_is_the_default_and_stock_is_asked_for(self) -> None:
        for flags, want in (([], "live"), (["--genome", "stock"], "stock")):
            with patch.object(hotseat, "run", side_effect=lambda a: 0) as run, \
                    patch.object(hotseat, "read_current", return_value=None):
                hotseat.main(flags)
            self.assertEqual(run.call_args.args[0].genome, want)
        source = (hotseat.HERE / "civ6_hotseat.py").read_text()
        # Both loops' workers are started with the list...
        self.assertEqual(source.count("treatments=args.treatments)"), 2)
        # ...decided before the install that carries its mod keys, and kept by an attach.
        run = source[source.index("def run(args"):]
        self.assertLess(run.index("live_genome(args, binary, run_dir, log)"), run.index("config = hotseat_config("))
        self.assertLess(run.index("kept_genome(args, run_dir)"), run.index("brains = Brains("))
        manual = source[source.index("def run_manual("):source.index("def main(")]
        self.assertLess(manual.index("live_genome(args, binary, run_dir, log)"),
                        manual.index("config = manual_lobby_config("))


class RelayTest(unittest.TestCase):
    """The live six-player game (hotseat-20261128T031456Z, turns 1-15): a
    seat's next board reached its decider 523 ms (median) after its last
    answer, one loop pass, and the next seat's first board 1057 ms after, two
    passes; the decider itself answers in 10 ms. The relay now runs every
    20 ms between passes, and nothing else runs more often than it did."""

    class Bridge:
        def __init__(self, clock: list) -> None:
            self.clock, self.at = clock, []

        def pump(self) -> int:
            self.at.append(self.clock[0])
            return 0

    def test_the_log_is_relayed_every_20_ms_through_the_pass(self) -> None:
        clock = [100.0]
        slept: list[float] = []

        def sleep(seconds: float) -> None:
            slept.append(seconds)
            clock[0] += seconds

        bridges = [self.Bridge(clock) for _ in range(6)]
        hotseat.relay_for(bridges, 0.5, clock=lambda: clock[0], sleep=sleep)
        self.assertAlmostEqual(clock[0], 100.5, places=6, msg="the pass keeps its length")
        self.assertLessEqual(max(slept), 0.02 + 1e-9)
        for bridge in bridges:
            self.assertGreaterEqual(len(bridge.at), 25)
            gaps = [b - a for a, b in zip([100.0] + bridge.at, bridge.at)]
            self.assertLessEqual(max(gaps), 0.02 + 1e-9, "no board waits more than 20 ms")

    def test_the_turn_loop_relays_between_its_passes_and_keeps_its_upkeep_cadence(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            config = hotseat.hotseat_config(game_args(players=6, agents=["holy", "tourist", "moonshot",
                                                                          "siege", "jack"], name="dev"),
                                            root / "hs", root / "hs" / "orders.sqlite")
            config["RunTag"] = "hs"
            clock = [1000.0]
            bridges: list = []
            polls, alive_calls, ps_calls = [], [], []
            passes = 8

            def sleep(seconds: float) -> None:
                clock[0] += seconds

            class Liveness:
                def alive(self) -> bool:
                    alive_calls.append(clock[0])
                    return len(alive_calls) <= passes

            def bridge(run_dir, tag):
                made = self.Bridge(clock)
                bridges.append(made)
                return made

            def pids():
                ps_calls.append(clock[0])
                return [4242]

            class Brains:
                def __init__(self, *a, **k): pass
                def start_all(self): pass
                def keep_alive(self): pass
                def stop(self): pass

            # `run` leaves SIGTERM/SIGINT ignored on its way out: not in this process.
            with patch.object(hotseat.signal, "signal"), \
                    patch.object(hotseat, "RUN_ROOT", root), patch.object(hotseat, "CURRENT", root / "current.json"), \
                    patch.object(hotseat.modinstall, "installed_config", return_value=config), \
                    patch.object(hotseat.env, "game_pids", side_effect=pids), \
                    patch.object(hotseat.gamelock, "acquire", return_value=True), \
                    patch.object(hotseat.gamelock, "release"), \
                    patch.object(hotseat.gamelock, "operator_halt_description", return_value=None), \
                    patch.object(hotseat, "resolve_orders_bin", return_value=Path("/bin/sh")), \
                    patch.object(hotseat.watch, "EventLogBridge", side_effect=bridge), \
                    patch.object(hotseat.watch, "GameLiveness", Liveness), \
                    patch.object(hotseat, "Brains", Brains), \
                    patch.object(hotseat.Monitor, "poll", side_effect=lambda: polls.append(clock[0])), \
                    patch.object(hotseat.time, "monotonic", side_effect=lambda: clock[0]), \
                    patch.object(hotseat.time, "sleep", side_effect=sleep), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(hotseat.run(game_args(attach=True, tag=None, orders_bin=None,
                                                       stall_seconds=180.0, turns=None)), 0)
            self.assertEqual(len(bridges), 6, "the table's own log and one per CIVVIS seat")
            loop_polls = len(polls) - 2           # less catch_up's and the last one
            self.assertEqual(loop_polls, passes + 1, "the monitor reads once a pass")
            self.assertEqual(len(alive_calls), passes + 1, "Civ VI is looked for once a pass")
            self.assertEqual(len(ps_calls), 2, "no `ps` in the loop: the attach check and the cleanup")
            for made in bridges:
                self.assertGreaterEqual(len(made.at), 25 * passes, "the relay runs 25 times a pass")
            self.assertIn("Civ VI has closed", (root / "hs" / "hotseat.log").read_text())


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

    def test_a_quicksave_is_a_save_to_go_on_from(self) -> None:
        # The live six-player game of 2026-11-28 made Saves/Hotseat/quick/quicksave
        # at 21:44, newer than every autosave: the save its person means.
        import os
        with tempfile.TemporaryDirectory() as tmp:
            saves = Path(tmp)
            for name, stamp in (("auto/AutoSave_0016.Civ6Save", 100), ("quick/quicksave.Civ6Save", 200),
                                ("hotseat-x-T010.Civ6Save", 50)):
                (saves / name).parent.mkdir(exist_ok=True)
                (saves / name).write_text(name)
                os.utime(saves / name, (stamp, stamp))
            self.assertEqual([p.name for p in hotseat.hotseat_saves(saves)],
                             ["quicksave.Civ6Save", "AutoSave_0016.Civ6Save", "hotseat-x-T010.Civ6Save"])
            self.assertEqual(hotseat.newest_save(saves).name, "quicksave.Civ6Save")


#: The live six-player game's installed config (hotseat-20261128T031456Z/config.json,
#: 2026-11-28), the keys a resume reads.
LIVE_6P = {
    "RunTag": "hotseat-20261128T031456Z", "Hotseat": True, "HotseatSeats": [1, 2, 3, 4, 5],
    "HotseatHumanSeats": [0], "HotseatStubSeats": [],
    "HotseatNames": {"1": "CIVVIS HolyRoller", "2": "CIVVIS TouristTrap", "3": "CIVVIS MoonshotMaven",
                     "4": "CIVVIS SiegeTheDay", "5": "CIVVIS JackOfAllTrades"},
    "HotseatSeatAgents": {"1": "holy", "2": "tourist", "3": "moonshot", "4": "siege", "5": "jack"},
    "HotseatPersonName": "dev", "HotseatPlayers": 6, "HotseatSaveEvery": 10,
    "HotseatSaveName": "hotseat-20261128T031456Z", "HotseatCover": True, "HotseatAutoStart": True,
    "HotseatCoverQueue": True, "HotseatCoverSound": "music", "HotseatCoverTimer": True,
    "PlayerDifficulty": "DIFFICULTY_PRINCE", "Difficulty": "DIFFICULTY_PRINCE",
    "Leader": "LEADER_GILGAMESH", "HumanPlayers": 6, "MapScript": "Continents.lua",
    "MapSize": "MAPSIZE_SMALL", "GameSpeed": "GAMESPEED_ONLINE", "MaxTurns": 250,
    "VictoryLane": "civvis", "VictoriesOff": [], "HotseatStarted": "2026-11-28T03:14:56Z",
    "HotseatRevision": "0512deb6"}
#: An evening rehearsal on the same Mac (smoke-6p-20261128T0244Zb): the test stub in seat 0.
SMOKE_6P = dict(LIVE_6P, RunTag="smoke-6p-20261128T0244Zb", HotseatStubSeats=[0],
                HotseatPersonName="", HotseatStarted="2026-11-28T02:44:00Z")


def fake_save(path: Path, *names: str, stamp: float) -> Path:
    """A save as Civ VI writes one, for what a resume reads of it: the names
    its seats carry, in among compressed bytes (in the live game's saves
    "CIVVIS HolyRoller" stands at byte 39729 and "dev" at 57232, and the mod's
    own "CIVVIS Control" ahead of them)."""
    import os
    import random
    filler = random.Random(7)
    body = bytearray(b"CIV6" + b'{"CIVVIS Control":[]}')
    for name in names:
        body += bytes(filler.randrange(256) for _ in range(4000)) + name.encode() + b"\x00"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(bytes(body))
    os.utime(path, (stamp, stamp))
    return path


class ResumeRestoresTheGameTest(unittest.TestCase):
    """A resumed run is a fresh harness. Before 2026-11-28 a bare `--resume`
    of the person's six-player game (dev as Gilgamesh against holy, tourist,
    moonshot, siege and jack) would have been played as the wrapper's duel
    defaults with jack on its one CIVVIS seat, no name, and no record."""

    PERSON_SAVE_NAMES = ("CIVVIS HolyRoller", "CIVVIS JackOfAllTrades", "CIVVIS SiegeTheDay", "dev",
                         "CIVVIS TouristTrap", "CIVVIS MoonshotMaven")

    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name) / "runs"
        self.saves = Path(tmp.name) / "Saves" / "Hotseat"
        self.saves.mkdir(parents=True)
        for target, value in ((hotseat, "RUN_ROOT"), (hotseat, "HOTSEAT_SAVES")):
            patcher = patch.object(target, value, self.root if value == "RUN_ROOT" else self.saves)
            patcher.start()
            self.addCleanup(patcher.stop)

    def a_run(self, config: dict, stamp: float, **files) -> Path:
        import os
        run_dir = self.root / config["RunTag"]
        run_dir.mkdir(parents=True)
        (run_dir / "config.json").write_text(json.dumps(config))
        os.utime(run_dir / "config.json", (stamp, stamp))
        for name, payload in files.items():
            (run_dir / name).write_text(json.dumps(payload))
        return run_dir

    def resumed(self, *flags: str) -> argparse.Namespace:
        with patch.object(hotseat, "run", side_effect=lambda a: 0) as run, \
                patch.object(hotseat, "read_current", return_value=None), \
                patch.object(hotseat, "strongest_agent", return_value="holy"):
            self.assertEqual(hotseat.main(list(flags)), 0)
        return run.call_args.args[0]

    def test_a_bare_resume_plays_the_game_the_save_is_of(self) -> None:
        self.a_run(LIVE_6P, 1000)
        fake_save(self.saves / "auto" / "AutoSave_0016.Civ6Save", *self.PERSON_SAVE_NAMES, stamp=2000)
        # Exactly what the wrapper sends for `civ6-hotseat.sh --resume`.
        args = self.resumed("--players", "2", "--map-size", "MAPSIZE_DUEL", "--resume")
        self.assertEqual(args.resume_save.name, "AutoSave_0016.Civ6Save")
        self.assertEqual((args.players, args.map, args.map_size, args.speed, args.difficulty),
                         (6, "Continents.lua", "MAPSIZE_SMALL", "GAMESPEED_ONLINE", "DIFFICULTY_PRINCE"))
        self.assertEqual(args.agents, ["holy", "tourist", "moonshot", "siege", "jack"])
        self.assertEqual((args.name, args.leader, args.max_turns, args.victories_off), ("dev", "LEADER_GILGAMESH", 250, []))
        self.assertEqual(hotseat.seat_lanes(args, hotseat.jack_seats(args.players)),
                         {1: "religious", 2: "culture", 3: "science", 4: "domination", 5: "civvis"})
        self.assertEqual(hotseat.person_name(args), "dev", "recorded for the people's Elo")
        self.assertEqual(args.resumed["started"], "2026-11-28T03:14:56Z")
        said = "\n".join(args.resume_notes)
        self.assertIn("the game of hotseat-20261128T031456Z", said)
        self.assertIn("seat 1 holy, seat 2 tourist, seat 3 moonshot, seat 4 siege, seat 5 jack", said)
        self.assertIn("dev as LEADER_GILGAMESH, recorded for the people's Elo", said)
        self.assertIn("not used, the save decides them: --players 2 (the game has 6); "
                      "--map-size MAPSIZE_DUEL (the game has MAPSIZE_SMALL)", said)

    def test_what_is_asked_again_wins_and_the_saves_own_table_does_not_change(self) -> None:
        self.a_run(LIVE_6P, 1000)
        fake_save(self.saves / "hotseat-20261128T031456Z-T010.Civ6Save", stamp=2000)
        args = self.resumed("--resume", "--agents", "jack,jack,jack,jack,jack", "--no-cover-timer",
                            "--duel", "--leader", "LEADER_SALADIN")
        self.assertEqual(args.agents, ["jack"] * 5)
        self.assertTrue(args.no_cover_timer)
        # The name, the cover and the save cadence come back as they were.
        self.assertEqual((args.name, args.no_cover, args.cover_sound, args.save_every), ("dev", False, "music", 10))
        # The leader and the table are the save's: --duel and --leader cannot change them.
        self.assertEqual((args.players, args.map_size, args.leader), (6, "MAPSIZE_SMALL", "LEADER_GILGAMESH"))
        said = "\n".join(args.resume_notes)
        self.assertIn("as asked now: --agents, --no-cover-timer", said)
        self.assertIn("--leader LEADER_SALADIN (the game has LEADER_GILGAMESH)", said)
        self.assertIn("--duel (the game has 6 players on MAPSIZE_SMALL)", said)

    def test_an_autosave_is_the_game_of_the_run_that_was_playing_when_it_was_written(self) -> None:
        person = self.a_run(LIVE_6P, 1000)
        # A rehearsal started before the person's run, one after the save, and
        # one between whose game never got going (no save names its seats).
        self.a_run(dict(SMOKE_6P, RunTag="smoke-before"), 500)
        self.a_run(dict(SMOKE_6P, RunTag="smoke-after"), 3000)
        self.a_run(dict(LIVE_6P, RunTag="failed-setup", HotseatNames={"1": "CIVVIS jack 1"},
                        HotseatSeatAgents={}), 1500)
        save = fake_save(self.saves / "auto" / "AutoSave_0016.Civ6Save", *self.PERSON_SAVE_NAMES, stamp=2000)
        self.assertEqual(hotseat.save_run(save), person)
        # A save the mod made says its run outright.
        tagged = fake_save(self.saves / "smoke-after-T010.Civ6Save", stamp=4000)
        self.assertEqual(hotseat.save_run(tagged), self.root / "smoke-after")

    def test_a_persons_resume_passes_over_a_rehearsals_newer_save(self) -> None:
        self.a_run(LIVE_6P, 1000)
        self.a_run(SMOKE_6P, 3000)
        fake_save(self.saves / "quick" / "quicksave.Civ6Save", *self.PERSON_SAVE_NAMES, stamp=2000)
        fake_save(self.saves / "smoke-6p-20261128T0244Zb-T010.Civ6Save", "Test stub", stamp=4000)
        fake_save(self.saves / "auto" / "AutoSave_0001.Civ6Save", "Test stub",
                  *self.PERSON_SAVE_NAMES[:3], *self.PERSON_SAVE_NAMES[4:], stamp=3500)
        args = self.resumed("--resume")
        self.assertEqual(args.resume_save.name, "quicksave.Civ6Save")
        self.assertEqual(args.name, "dev")
        said = "\n".join(args.resume_notes)
        self.assertIn("passed over smoke-6p-20261128T0244Zb-T010.Civ6Save", said)
        self.assertIn("passed over AutoSave_0001.Civ6Save", said)
        # A test's command takes the newest save, whoever's it is; so does a path.
        self.assertEqual(self.resumed("--resume", "--auto-human", "gated").resume_save.name,
                         "smoke-6p-20261128T0244Zb-T010.Civ6Save")
        given = str(self.saves / "smoke-6p-20261128T0244Zb-T010.Civ6Save")
        self.assertEqual(str(self.resumed("--resume", given).resume_save), given)

    def test_a_save_no_run_recorded_is_played_as_typed_only_when_it_fits(self) -> None:
        fake_save(self.saves / "auto" / "AutoSave_0016.Civ6Save", *self.PERSON_SAVE_NAMES, stamp=2000)
        with patch.object(hotseat, "run") as run, patch.object(hotseat, "read_current", return_value=None), \
                contextlib.redirect_stderr(io.StringIO()) as err:
            self.assertEqual(hotseat.main(["--players", "2", "--map-size", "MAPSIZE_DUEL", "--resume"]), 2)
            run.assert_not_called()
        said = err.getvalue()
        self.assertIn("the save has 5 CIVVIS seats", said)
        self.assertIn("where this command plays 1", said)
        self.assertIn("--players 6 --agents holy,jack,siege,tourist,moonshot --name YOU", said)
        args = self.resumed("--resume", "--players", "6", "--name", "dev")
        self.assertEqual((args.players, args.name, args.agents), (6, "dev", None))
        self.assertIn("no run in", args.resume_notes[0])
        self.assertIn("played as this command says (6 players, agents jack, name dev)", args.resume_notes[0])

    def test_a_resumed_run_keeps_the_persons_leader_and_the_games_beginning(self) -> None:
        self.a_run(LIVE_6P, 1000)
        fake_save(self.saves / "auto" / "AutoSave_0016.Civ6Save", *self.PERSON_SAVE_NAMES, stamp=2000)
        args = self.resumed("--resume", "--tag", "resumed-1")
        installed = {}
        stage = self.saves / "civvis-hotseat-resume.Civ6Save"
        # `run` sets SIGTERM/SIGINT handlers and leaves them ignored: in this
        # process that would outlive the test (CI runs every tool test in one).
        with patch.object(hotseat.signal, "signal"), \
                patch.object(hotseat, "CURRENT", self.root / "current.json"), \
                patch.object(hotseat.env, "game_pids", return_value=[]), \
                patch.object(hotseat.gamelock, "acquire", return_value=True), \
                patch.object(hotseat.gamelock, "release"), \
                patch.object(hotseat.gamelock, "operator_halt_description", return_value=None), \
                patch.object(hotseat, "resolve_orders_bin", return_value=Path("/bin/sh")), \
                patch.object(hotseat, "freeze_binary", return_value=Path("/bin/sh")), \
                patch.object(hotseat, "checkout_revision", return_value="abcd1234"), \
                patch.object(hotseat, "reset_orders_db"), \
                patch.object(hotseat.modinstall, "install", side_effect=lambda c: installed.update(c) or "DLC"), \
                patch.object(hotseat.modinstall, "uninstall"), \
                patch.object(hotseat, "stage_save", return_value=stage), \
                patch.object(hotseat, "host_strain", return_value={}), \
                patch.object(hotseat, "apply_person_options"), \
                patch.object(hotseat.launcher, "clear_run_logs"), \
                patch.object(hotseat.launcher, "launch", side_effect=hotseat.Stop("test")), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(hotseat.run(args), 0)
        run_dir = self.root / "resumed-1"
        # The install names no leader (the save's staging room would refuse one)...
        self.assertIsNone(installed["Leader"])
        self.assertEqual(installed["HotseatSeatAgents"], LIVE_6P["HotseatSeatAgents"])
        self.assertEqual((installed["HotseatPersonName"], installed["HotseatPlayers"]), ("dev", 6))
        # ...and the game began when its first run did, not at the resume.
        self.assertEqual(installed["HotseatStarted"], "2026-11-28T03:14:56Z")
        table = json.loads((run_dir / "table.json").read_text())
        self.assertEqual((table["leader"], table["name"], table["resumed_from"], table["started"]),
                         ("LEADER_GILGAMESH", "dev", "hotseat-20261128T031456Z", "2026-11-28T03:14:56Z"))
        self.assertEqual(table["names"], LIVE_6P["HotseatNames"])
        self.assertIn("resuming AutoSave_0016.Civ6Save, the game of hotseat-20261128T031456Z",
                      (run_dir / "hotseat.log").read_text())
        # A resume of the resumed game reads its table.json: the leader is still there.
        fake_save(self.saves / "resumed-1-T020.Civ6Save", stamp=5000)
        again = self.resumed("--resume")
        self.assertEqual((again.resume_save.name, again.leader, again.name, again.resumed["started"]),
                         ("resumed-1-T020.Civ6Save", "LEADER_GILGAMESH", "dev", "2026-11-28T03:14:56Z"))
        # ...and so does a harness that re-attaches to it.
        with patch.object(hotseat.signal, "signal"), \
                patch.object(hotseat.modinstall, "installed_config", return_value=installed), \
                patch.object(hotseat.env, "game_pids", return_value=[4242]), \
                patch.object(hotseat.gamelock, "acquire", return_value=False), \
                patch.object(hotseat.gamelock, "foreign_run", return_value="another run"), \
                patch.object(hotseat, "CURRENT", self.root / "current.json"), \
                contextlib.redirect_stdout(io.StringIO()):
            attach = game_args(attach=True, leader="LEADER_SALADIN", name=None, tag=None)
            self.assertEqual(hotseat.run(attach), 6)
        self.assertEqual((attach.leader, attach.name, attach.agents), ("LEADER_GILGAMESH", "dev",
                                                                        ["holy", "tourist", "moonshot", "siege", "jack"]))


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

    def test_one_civvis_seat_and_five_firaxis_ai(self) -> None:
        # Operator 2026-11-21 ~20:38Z: "for tournament 6 we should just have 1
        # of our civvis in the game. other 5 are ai."
        args = game_args(players=6, civvis_seats=1, leader=None)
        config = hotseat.hotseat_config(args, Path("/tmp/run"), Path("/tmp/run/o.sqlite"))
        self.assertEqual(config["HotseatSeats"], [0])
        self.assertEqual(config["HumanPlayers"], 1)
        self.assertEqual(config["HotseatHumanSeats"], [])
        self.assertEqual(hotseat.wanted_slots(6, 1), ["host"] + ["computer"] * 5)

    def test_victories_off_reaches_the_setup_defaults(self) -> None:
        # Operator 2026-11-21 ~20:40Z: "no diplo allowed for any level".
        args = game_args(players=6, civvis_seats=1, leader=None,
                         victories_off=["VICTORY_DIPLOMATIC"])
        config = hotseat.hotseat_config(args, Path("/tmp/run"), Path("/tmp/run/o.sqlite"))
        self.assertEqual(config["VictoriesOff"], ["VICTORY_DIPLOMATIC"])
        template = (install.MOD_SOURCE / "CivvisControlConfig.xml").read_text()
        rendered = install._frontend_defaults_xml(template, config)
        self.assertIn('<Where VictoryType="VICTORY_DIPLOMATIC"/>', rendered)
        self.assertIn("V.EnabledByDefault AS DefaultValue", rendered)
        plain = hotseat.hotseat_config(game_args(players=6, civvis_seats=1, leader=None),
                                       Path("/tmp/run"), Path("/tmp/run/o.sqlite"))
        self.assertEqual(plain["VictoriesOff"], [], "every victory as shipped unless asked")

    def test_an_unknown_victory_type_is_refused_before_anything_starts(self) -> None:
        with patch.object(hotseat, "run") as run:
            self.assertEqual(hotseat.main(["--civvis-seats", "1", "--players", "6",
                                           "--victories-off", "VICTORY_DIPLO"]), 2)
            run.assert_not_called()

    def victory_monitor(self, run_dir: Path) -> "hotseat.Monitor":
        lines: list[str] = []
        monitor = hotseat.Monitor(run_dir, "hs", [0],
                                  game_args(players=6, civvis_seats=1,
                                            victories_off=["VICTORY_DIPLOMATIC"]),
                                  lines.append)
        monitor.lines = lines
        self.addCleanup(monitor.close)
        return monitor

    def test_a_victory_asked_off_is_read_back_from_the_game(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            monitor = self.victory_monitor(run_dir)
            self.seat_event(run_dir, 0, kind="seat", civ="CIVILIZATION_GAUL",
                            victories={"diplomatic": False, "technology": True})
            monitor.poll()
            self.assertIsNone(monitor.setup_fault)
            self.assertEqual(monitor.victories, {"diplomatic": False, "technology": True})
            self.assertTrue(any("read back from the game: VICTORY_DIPLOMATIC" in line
                                for line in monitor.lines), monitor.lines)

    def test_a_victory_still_on_is_a_setup_fault(self) -> None:
        for switches in ({"diplomatic": True}, {"technology": True}, None):
            with tempfile.TemporaryDirectory() as tmp:
                run_dir = Path(tmp)
                monitor = self.victory_monitor(run_dir)
                self.seat_event(run_dir, 0, kind="seat", civ="CIVILIZATION_GAUL",
                                **({} if switches is None else {"victories": switches}))
                monitor.poll()
                self.assertIn("VICTORY_DIPLOMATIC", monitor.setup_fault or "", switches)
                self.assertTrue(any("victory switch not applied" in line
                                    for line in monitor.lines), monitor.lines)

    def test_a_table_that_asks_nothing_off_has_no_fault(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            monitor = self.monitor(run_dir)
            self.seat_event(run_dir, 0, kind="seat", civ="CIVILIZATION_GAUL",
                            victories={"diplomatic": True})
            monitor.poll()
            self.assertIsNone(monitor.setup_fault)
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


class CoverTest(unittest.TestCase):
    """Operator 2026-11-26: a person's hotseat game plays like single player --
    a cover over every turn that is not theirs, and no hand-over screen."""

    def config(self, **overrides) -> dict:
        return hotseat.hotseat_config(game_args(**overrides), Path("/tmp/r"),
                                      Path("/tmp/r/o.sqlite"))

    def install(self, config: dict) -> Path:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        with patch.object(install, "check_syntax", return_value=None):
            install._write_mod(Path(tmp.name), config)
        return Path(tmp.name)

    def test_a_persons_game_is_covered_and_starts_itself_by_default(self) -> None:
        cases = {
            "person": ({}, (True, True)),
            "no cover": ({"no_cover": True}, (False, True)),
            "keep the hand-over": ({"keep_handover": True}, (True, False)),
            "both old ways": ({"no_cover": True, "keep_handover": True}, (False, False)),
            # `gated` stands in for the person: their game, testable unattended.
            "gated stub": ({"auto_human": "gated"}, (True, True)),
            # A stub that is nobody, and the unattended tables: no person, no cover.
            "stub": ({"auto_human": "stub"}, (False, False)),
            "unattended": ({"players": 6, "civvis_seats": 2, "leader": None}, (False, False)),
        }
        for name, (overrides, want) in cases.items():
            with self.subTest(name):
                config = self.config(**overrides)
                self.assertEqual((config["HotseatCover"], config["HotseatAutoStart"]), want)
        # The diagnostic: the screens under the cover keep their shipped queueing.
        self.assertTrue(self.config()["HotseatCoverQueue"])
        self.assertFalse(self.config(no_cover_queue=True)["HotseatCoverQueue"])
        self.assertFalse(self.config(players=6, civvis_seats=2, leader=None)["HotseatCoverQueue"])

    def test_the_flags_reach_the_config(self) -> None:
        with patch.object(hotseat, "run", side_effect=lambda args: args) as ran, \
                patch.object(hotseat, "read_current", return_value=None):
            hotseat.main(["--no-cover", "--keep-handover", "--no-cover-queue"])
            args = ran.call_args.args[0]
        self.assertTrue(args.no_cover and args.keep_handover and args.no_cover_queue)
        config = hotseat.hotseat_config(args, Path("/tmp/r"), Path("/tmp/r/o.sqlite"))
        self.assertEqual((config["HotseatCover"], config["HotseatAutoStart"],
                          config["HotseatCoverQueue"]), (False, False, False))

    def test_the_cover_is_quiet_and_says_how_long_by_default(self) -> None:
        cases = {
            # A person's game: CIVVIS's sounds silenced, the soundtrack on; a wait bar.
            "person": ({}, ("music", True)),
            "gated stub": ({"auto_human": "gated"}, ("music", True)),
            "no music either": ({"cover_sound": "silent"}, ("silent", True)),
            "every sound, as before": ({"cover_sound": "all"}, ("all", True)),
            "no bar": ({"no_cover_timer": True}, ("music", False)),
            # They are the cover's own: without it nothing is quieted or timed.
            "no cover": ({"no_cover": True}, ("all", False)),
            "no cover, asked silent": ({"no_cover": True, "cover_sound": "silent"}, ("all", False)),
            "stub": ({"auto_human": "stub"}, ("all", False)),
            "unattended": ({"players": 6, "civvis_seats": 2, "leader": None}, ("all", False)),
        }
        for name, (overrides, want) in cases.items():
            with self.subTest(name):
                config = self.config(**overrides)
                self.assertEqual((config["HotseatCoverSound"], config["HotseatCoverTimer"]), want)
        # The modes are the ones the cover script knows (its BUSES table), plus `all`.
        script = (Path(install.__file__).parent / "mod" / "CivvisHotseatCover.lua").read_text()
        buses = script.split("CivvisHotseatCover.BUSES = {", 1)[1].split("};", 1)[0]
        for mode in hotseat.COVER_SOUNDS:
            self.assertEqual(f"\t{mode} = {{" in buses, mode != "all", mode)
        self.assertEqual(hotseat.COVER_SOUND, "music")

    def test_the_sound_and_timer_flags_reach_the_install(self) -> None:
        with patch.object(hotseat, "run", side_effect=lambda args: args) as ran, \
                patch.object(hotseat, "read_current", return_value=None):
            hotseat.main([])
            plain = ran.call_args.args[0]
            hotseat.main(["--cover-sound", "silent", "--no-cover-timer"])
            asked = ran.call_args.args[0]
            with self.assertRaises(SystemExit), patch("sys.stderr"):
                hotseat.main(["--cover-sound", "loud"])
        self.assertEqual((plain.cover_sound, plain.no_cover_timer), ("music", False))
        self.assertEqual((asked.cover_sound, asked.no_cover_timer), ("silent", True))
        config = hotseat.hotseat_config(asked, Path("/tmp/r"), Path("/tmp/r/o.sqlite"))
        self.assertEqual((config["HotseatCoverSound"], config["HotseatCoverTimer"]), ("silent", False))
        # In the cover context's own settings, as the script reads them.
        head = (self.install(self.config()) / "CivvisHotseatCover.lua").read_text().split("}\n", 1)[0]
        self.assertIn('HotseatCoverSound = "music",', head)
        self.assertIn("HotseatCoverTimer = true,", head)
        head = (self.install(config) / "CivvisHotseatCover.lua").read_text().split("}\n", 1)[0]
        self.assertIn('HotseatCoverSound = "silent",', head)
        self.assertIn("HotseatCoverTimer = false,", head)

    def test_the_cover_context_is_installed_for_a_persons_game(self) -> None:
        out = self.install(self.config())
        tree = ET.fromstring((out / "CivvisControl.modinfo").read_text())
        contexts = [f.text for f in tree.findall(
            "./InGameActions/AddUserInterfaces[@id='CivvisHotseatCoverUI']/File")]
        self.assertEqual(contexts, ["CivvisHotseatCover.xml"])
        files = {f.text for f in tree.findall("./Files/File")}
        # Every file an action names is listed, or the game drops the action.
        for action in tree.find("./InGameActions"):
            for named in action.findall("File"):
                self.assertIn(named.text, files)
            replace = action.findtext("./Properties/LuaReplace")
            if replace:
                self.assertIn(replace, files)
        self.assertIn("CivvisHotseatCover.lua", files)
        script = (out / "CivvisHotseatCover.lua").read_text()
        head = script.split("}\n", 1)[0]
        self.assertIn("HotseatCover = true,", head)
        self.assertIn("HotseatAutoStart = true,", head)
        self.assertIn('RunTag = "hotseat-test",', head, "the cover reports under the run's own tag")
        self.assertIn("CivvisHotseatCover.arm();", script)
        self.assertTrue((out / "CivvisHotseatCover.xml").is_file())
        # The screens it keeps drawn under it carry the same switch.
        closer = (out / "CivvisControlAutoClose.lua").read_text().split("}\n", 1)[0]
        self.assertIn("HotseatCover = true,", closer)

    def test_no_other_install_carries_it(self) -> None:
        for name, config in {
            "single player": {"RunTag": "civvis-solo", "Play": True, "CivvisDecides": True},
            "unattended": self.config(players=6, civvis_seats=2, leader=None),
            "stub": self.config(auto_human="stub"),
            "both old ways": self.config(no_cover=True, keep_handover=True),
        }.items():
            with self.subTest(name):
                out = self.install(config)
                self.assertNotIn("CivvisHotseatCover", (out / "CivvisControl.modinfo").read_text())
                self.assertEqual(list(out.glob("CivvisHotseatCover*")), [])
        # `--no-cover` alone still keeps the person's view: the context comes, the cover stays off.
        out = self.install(self.config(no_cover=True))
        head = (out / "CivvisHotseatCover.lua").read_text().split("}\n", 1)[0]
        self.assertIn("HotseatCover = false,", head)

    def monitor(self, run_dir: Path, **overrides) -> tuple[hotseat.Monitor, list[str]]:
        lines: list[str] = []
        monitor = hotseat.Monitor(run_dir, "hs", [1], game_args(players=2, **overrides),
                                  lines.append)
        self.addCleanup(monitor.close)
        return monitor, lines

    def write(self, run_dir: Path, *events: dict) -> None:
        with (run_dir / "events.jsonl").open("a") as out:
            for event in events:
                out.write(json.dumps(dict(event, run="hs")) + "\n")

    def test_the_cover_is_followed_and_a_stand_aside_is_told(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            monitor, lines = self.monitor(run_dir)
            self.write(run_dir, {"kind": "hotseat_cover", "up": True, "reason": "turn_end",
                                 "player": 0, "turn": 5})
            monitor.poll()
            self.assertTrue(monitor.covered())
            self.assertEqual(lines, [], "the cover's ordinary comings and goings are quiet")
            self.write(run_dir, {"kind": "hotseat_cover", "up": False, "reason": "yield_quiet",
                                 "player": 1, "turn": 5, "quiet": 90, "between": 0})
            monitor.poll()
            self.assertFalse(monitor.covered())
            self.assertEqual(monitor.cover_yields, 1)
            self.assertIn("the cover stood aside for seat 1 (yield_quiet", "\n".join(lines))

    def test_the_screen_rescue_does_not_read_the_cover(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            monitor, lines = self.monitor(run_dir)
            self.write(run_dir,
                       {"kind": "hotseat_turn", "phase": "begin", "player": 1, "turn": 5,
                        "human": False},
                       {"kind": "hotseat_cover", "up": True, "reason": "civvis_turn",
                        "player": 1, "turn": 5})
            with patch.object(hotseat, "read_screen", side_effect=AssertionError("read")), \
                    patch.object(hotseat.popup_clear, "frontmost", return_value="Civ6"), \
                    patch.object(hotseat.operator_presence, "operator_active", return_value=False):
                hotseat.rescue_popup(monitor, lines.append)
            self.assertIn("under the cover", lines[-1])
            # A cover believed up for too long no longer stops the rescue.
            monitor.cover_since -= hotseat.Monitor.COVER_TRUST_S + 1
            self.assertFalse(monitor.covered())

    def test_the_banner_says_where_the_person_is(self) -> None:
        for overrides, want in (({}, "Your map is on the screen."),
                                ({"keep_handover": True}, 'Press "Start Turn" in Civ VI.')):
            with tempfile.TemporaryDirectory() as tmp, patch("builtins.print") as printed:
                monitor, _ = self.monitor(Path(tmp), **overrides)
                monitor.your_turn(7)
                self.assertIn(want, printed.call_args.args[0])


class DuelUnderYourNameTest(unittest.TestCase):
    """`--duel --name dev`: one against one with the strongest agent, the
    person's seat under their name, and the ending kept for their Elo."""

    def test_the_strongest_agent_is_the_roster_agent_the_published_fit_rates_highest(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            fit = Path(tmp) / "elo.json"
            # The game's own AI and an agent no live seat plays are not candidates.
            fit.write_text(json.dumps({"ratings": {
                "civ6:deity": {"elo": 1900}, "forward": {"elo": 1500},
                "holy": {"elo": 1400}, "tourist": {"elo": 1450}, "jack": {"elo": 1100}}}))
            self.assertEqual(hotseat.strongest_agent(fit), "tourist")
            self.assertEqual(hotseat.strongest_agent(Path(tmp) / "no-fit.json"), "jack")
        # The repository's own published fit rates a roster agent.
        self.assertIn(hotseat.strongest_agent(), hotseat.AGENT_LANES)

    def started_with(self, *flags: str) -> argparse.Namespace:
        with patch.object(hotseat, "run", return_value=0) as run, \
                patch.object(hotseat, "read_current", return_value=None), \
                patch.object(hotseat, "strongest_agent", return_value="holy"):
            self.assertEqual(hotseat.main(list(flags)), 0)
        return run.call_args.args[0]

    def test_a_duel_is_two_players_on_a_duel_map_against_the_strongest(self) -> None:
        args = self.started_with("--duel", "--name", "dev", "--leader", "LEADER_GILGAMESH")
        self.assertEqual((args.players, args.map_size, args.map), (2, "MAPSIZE_DUEL", "Pangaea.lua"))
        self.assertEqual((args.agents, args.name, args.leader), (["holy"], "dev", "LEADER_GILGAMESH"))
        # Another agent can be named, and `strongest` is a name anywhere in the list.
        self.assertEqual(self.started_with("--duel", "--agents", "tourist").agents, ["tourist"])
        self.assertEqual(self.started_with("--agents", "jack,strongest").agents, ["jack", "holy"])

    def test_a_name_the_game_and_the_record_cannot_carry_is_refused(self) -> None:
        for bad in ("", " dev", "x" * 25, 'de"v', "dev\n", "<b>"):
            with patch.object(hotseat, "run") as run, \
                    patch.object(hotseat, "read_current", return_value=None):
                self.assertEqual(hotseat.main(["--name", bad]), 2, repr(bad))
                run.assert_not_called()

    def test_the_hand_over_screen_is_told_the_persons_name(self) -> None:
        config = hotseat.hotseat_config(game_args(players=2, name="dev", agents=["holy"]),
                                        Path("/tmp/r"), Path("/tmp/r/o.sqlite"))
        self.assertEqual(config["HotseatPersonName"], "dev")
        self.assertEqual(config["HotseatHumanSeats"], [0])
        self.assertEqual(config["HotseatNames"], {"1": "CIVVIS HolyRoller"})
        self.assertEqual(config["HotseatSeatAgents"], {"1": "holy"})
        # The agent's lane reaches its seat's context in the mod, as it reaches
        # its decision worker: the seat plays one lane, not two.
        self.assertEqual(config["HotseatSeatLanes"], {"1": "religious"})
        self.assertEqual(install.hotseat_seat_config(config, 1)["VictoryLane"], "religious")
        self.assertNotIn("HotseatSeatLanes", install.hotseat_seat_config(config, 1))
        self.assertEqual(hotseat.seat_lanes(game_args(players=2, agents=["holy"]), [1]), {1: "religious"})
        mixed = hotseat.hotseat_config(game_args(agents=["tourist"]), Path("/tmp/r"), Path("/tmp/r/o.sqlite"))
        self.assertEqual([install.hotseat_seat_config(mixed, seat)["VictoryLane"] for seat in (1, 2, 3)],
                         ["culture", "civvis", "civvis"])
        # No `--agents`: every seat keeps the table's lane.
        plain = hotseat.hotseat_config(game_args(), Path("/tmp/r"), Path("/tmp/r/o.sqlite"))
        self.assertEqual((plain["HotseatSeatLanes"], install.hotseat_seat_config(plain, 2)["VictoryLane"]),
                         ({}, "civvis"))
        # A test stub's seat and an unattended table have no person to name.
        for nobody in (game_args(name="dev", auto_human="stub"),
                       game_args(name="dev", players=6, civvis_seats=1, leader=None)):
            self.assertEqual(hotseat.hotseat_config(nobody, Path("/tmp/r"),
                                                    Path("/tmp/r/o.sqlite"))["HotseatPersonName"], "")
        # Asked for no name, the seat stays the game's own "Player 1".
        self.assertEqual(hotseat.hotseat_config(game_args(), Path("/tmp/r"),
                                                Path("/tmp/r/o.sqlite"))["HotseatPersonName"], "")

    def duel(self, run_dir: Path, **more) -> tuple["hotseat.Monitor", argparse.Namespace, list]:
        args = game_args(players=2, map_size="MAPSIZE_DUEL", name="dev", agents=["holy"],
                         leader="LEADER_GILGAMESH", victories_off=[], **more)
        lines: list[str] = []
        monitor = hotseat.Monitor(run_dir, "hs", [1], args, lines.append)
        self.addCleanup(monitor.close)
        return monitor, args, lines

    @staticmethod
    def event(run_dir: Path, where: str, **event) -> None:
        target = run_dir / where
        target.parent.mkdir(exist_ok=True)
        with target.open("a") as out:
            out.write(json.dumps(event) + "\n")

    def test_a_persons_table_keeps_its_ending_and_who_played(self) -> None:
        import people_elo

        began = {"started": "2026-11-27T06:30:00Z", "revision": "945f056d"}
        for team, winner in ((1, "holy"), (0, "person")):
            with tempfile.TemporaryDirectory() as tmp:
                run_dir = Path(tmp)
                monitor, args, lines = self.duel(run_dir)
                self.event(run_dir, "events.jsonl", kind="hotseat_person", seat=0,
                           applied=True, shown="dev")
                self.event(run_dir, "hs-p1/events.jsonl", kind="seat",
                           civ="CIVILIZATION_ARABIA", team=1)
                self.event(run_dir, "hs-p1/events.jsonl", kind="victory", team=team, victory=4,
                           victory_type="VICTORY_RELIGIOUS", turn=143)
                monitor.poll()
                self.assertIn("seat 0 is named 'dev' in the game", lines)
                self.assertEqual(monitor.name(0), "dev (you, seat 0)")
                record = hotseat.table_record(monitor, args, [1], began)
                self.assertEqual(record["result"]["ours"], team == 1)
                self.assertEqual(record["result"]["victory_type"], "VICTORY_RELIGIOUS")
                self.assertEqual((record["person"], record["leader"], record["seat_agents"]),
                                 ("dev", "LEADER_GILGAMESH", {1: "holy"}))
                self.assertEqual((record["started"], record["revision"]),
                                 ("2026-11-27T06:30:00Z", "945f056d"))
                # ...and it is the line the people's record takes, through JSON
                # as summary.json carries it.
                summary = json.loads(json.dumps({"tag": "hs", "utc": "2026-11-27T09:00:00Z", **record}))
                row = people_elo.game_row(summary)
                self.assertEqual((row["person"], row["agents"], row["winner"], row["victory"],
                                  row["turn"], row["at"], row["diplomatic"]),
                                 ("dev", ["holy"], winner, "VICTORY_RELIGIOUS", 143,
                                  "2026-11-27T06:30:00Z", True))

    def test_a_test_stubs_game_keeps_no_ending_and_a_name_not_taken_is_said(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            monitor, args, lines = self.duel(run_dir)
            self.event(run_dir, "events.jsonl", kind="hotseat_person", seat=0,
                       applied=False, shown="Player 1")
            monitor.poll()
            self.assertIn("seat 0 is named 'Player 1' in the game (the name was NOT taken)", lines)
        with tempfile.TemporaryDirectory() as tmp:
            monitor, args, _ = self.duel(Path(tmp), auto_human="stub")
            self.assertEqual(hotseat.table_record(monitor, args, [1], {}), {})

    def test_a_finished_game_under_a_name_is_put_on_the_peoples_record(self) -> None:
        source = (hotseat.HERE / "civ6_hotseat.py").read_text()
        # At the harness's end, before the wait for Civ VI to close...
        self.assertIn('if summary.get("person") and (summary.get("result") or request.get("resign")):\n'
                      '            record_person_game(run_dir, log, resigned=not summary.get("result"))',
                      source)
        end = source.index('record_person_game(run_dir, log, resigned=not summary.get("result"))')
        self.assertLess(end, source.index('log("closing Civ VI")'),
                        "the record does not wait on a quit that can take minutes")
        # ...and in the loop the moment the game is decided, for a person's table only.
        self.assertIn("if monitor.result is not None and not posted and not monitor.unattended \\\n"
                      "                    and person_name(args):", source)
        lines: list[str] = []
        done = argparse.Namespace(returncode=2, stdout="", stderr="people_elo: not recorded: why\n")
        with patch.object(hotseat.subprocess, "run", return_value=done) as ran:
            hotseat.record_person_game(Path("/tmp/run"), lines.append)
        self.assertEqual(ran.call_args.args[0][1:], [str(hotseat.HERE / "people_elo.py"), "record", "/tmp/run"])
        self.assertEqual(lines[0], "[people] people_elo: not recorded: why")
        self.assertIn("NOT put on the people's record (exit 2)", lines[1])
        # A game given up is CIVVIS's, and says so.
        with patch.object(hotseat.subprocess, "run", return_value=done) as ran:
            hotseat.record_person_game(Path("/tmp/run"), lines.append, resigned=True)
        self.assertEqual(ran.call_args.args[0][2:],
                         ["record", "/tmp/run", "--winner", "civvis", "--victory", "resignation"])
        self.assertIn("`tools/people_elo.py record /tmp/run --winner civvis --victory resignation` "
                      "tries again", lines[-1])

    def test_resigning_is_a_stop_that_says_so(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            current = {"pid": 4242, "tag": "duel-dev", "run_dir": str(run_dir)}
            alive = iter([True, True, False, False])
            with patch.object(hotseat, "read_current", return_value=current), \
                    patch.object(hotseat, "harness_alive", side_effect=lambda pid: next(alive, False)), \
                    patch.object(hotseat.os, "kill") as kill, patch.object(hotseat.time, "sleep"), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(hotseat.main(["--stop", "--quit-game", "--resign"]), 0)
            kill.assert_called_once()
            request = json.loads((run_dir / "stop.json").read_text())
            self.assertEqual((request["quit_game"], request["resign"]), (True, True))
            with patch.object(hotseat, "read_current", return_value=current), \
                    patch.object(hotseat, "harness_alive", side_effect=[True, False, False]), \
                    patch.object(hotseat.os, "kill"), patch.object(hotseat.time, "sleep"), \
                    contextlib.redirect_stdout(io.StringIO()):
                hotseat.main(["--stop"])
            self.assertIs(json.loads((run_dir / "stop.json").read_text())["resign"], False)

    def test_the_ending_carries_the_games_own_time_for_it(self) -> None:
        self.assertEqual(hotseat.ended_at({"t": 1795762751}), {"at": "2026-11-27T06:59:11Z"})
        self.assertEqual(hotseat.ended_at({"turn": 3}), {})
        self.assertEqual(hotseat.ended_at({"t": "soon"}), {})
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            monitor, args, _ = self.duel(run_dir)
            self.event(run_dir, "hs-p1/events.jsonl", kind="victory", team=0, victory=1,
                       victory_type="VICTORY_CULTURE", turn=150, t=1795762751)
            monitor.poll()
            self.assertEqual(monitor.result["at"], "2026-11-27T06:59:11Z")
        # Every table's record says when it began and at which revision.
        with tempfile.TemporaryDirectory() as tmp:
            args = game_args(players=2, civvis_seats=2, agents=["jack", "holy"], victories_off=[])
            monitor = hotseat.Monitor(Path(tmp), "hs", [0, 1], args, lambda m: None)
            self.addCleanup(monitor.close)
            record = hotseat.table_record(monitor, args, [0, 1],
                                          {"started": "2026-11-27T06:54:17Z", "revision": "945f056d"})
            self.assertEqual((record["started"], record["revision"]), ("2026-11-27T06:54:17Z", "945f056d"))
            self.assertNotIn("person", record)


def text(*labels: str) -> list[dict]:
    """A read screen: one row per label."""
    return [{"text": label, "x": 855, "y": 1040, "norm": hotseat._norm(label)} for label in labels]


class StartGameTest(unittest.TestCase):
    """Live 2026-11-27 (duel-dev-20261127T0643Z): the window query went
    unanswered, the screen read came back empty, and the empty read was taken
    for a staging room that had gone. START GAME was never pressed."""

    BOUNDS = (0, 33, 1710, 1074)

    def start(self, screens: list, **more) -> tuple[bool, list, list]:
        """`start_game` over these screen reads in turn: its answer, its
        clicks, and what it logged."""
        reads = iter(screens)
        clicks, lines, asked = [], [], []

        def read(run_dir, name, fallback=None):
            asked.append(fallback)
            return self.BOUNDS, next(reads, [])

        with patch.object(hotseat, "read_screen", side_effect=read), \
                patch.object(hotseat, "click", side_effect=lambda log, point: clicks.append(point)), \
                patch.object(hotseat, "wait_for_hands_off"), \
                patch.object(hotseat.play, "ensure_game_frontmost") as front, \
                patch.object(hotseat.time, "sleep"):
            answer = hotseat.start_game(Path("/tmp/r"), self.BOUNDS, lines.append, **more)
        self.assertTrue(front.called, "the game is brought forward before it is read")
        self.assertEqual(set(asked), {self.BOUNDS}, "a read falls back on the window it was given")
        return answer, clicks, lines

    def test_a_screen_that_cannot_be_read_is_not_a_staging_room_that_has_gone(self) -> None:
        answer, clicks, lines = self.start([[], [], []], attempts=3)
        self.assertFalse(answer)
        self.assertEqual(clicks, [])
        self.assertEqual(len([line for line in lines if "could not be read" in line]), 3)

    def test_it_presses_then_sees_the_room_go(self) -> None:
        staging, loading = text("STAGING ROOM", "START GAME"), text("Loading, Please Wait...")
        answer, clicks, _ = self.start([[], staging, loading])
        self.assertTrue(answer)
        # The orb above the label, once; the unreadable first frame pressed nothing.
        self.assertEqual(clicks, [(855, 1040 - int(0.02 * 1074))])

    def test_a_press_that_did_not_take_is_made_again_on_the_label_itself(self) -> None:
        staging = text("STAGING ROOM", "START GAME")
        answer, clicks, _ = self.start([staging, staging, [], text("Gilgamesh")])
        self.assertTrue(answer)
        self.assertEqual(clicks, [(855, 1019), (855, 1040)])

    def test_another_page_is_not_a_started_game_until_start_game_was_pressed(self) -> None:
        # A Terminal in front of the staging room reads as that Terminal.
        answer, clicks, _ = self.start([text("martin@air ~ %")] * 4, attempts=3)
        self.assertFalse(answer)
        self.assertEqual(clicks, [])
        # ...and a room still up after every press is not started either.
        answer, clicks, _ = self.start([text("STAGING ROOM", "START GAME")] * 4, attempts=3)
        self.assertFalse(answer)
        self.assertEqual(len(clicks), 3)

    def again(self, rows: list, *, active: bool = False, bounds=BOUNDS) -> tuple[bool, list, list]:
        clicks, lines = [], []
        with patch.object(hotseat, "read_screen", return_value=(bounds, rows)), \
                patch.object(hotseat, "click", side_effect=lambda log, point: clicks.append(point)), \
                patch.object(hotseat.play, "ensure_game_frontmost"), \
                patch.object(hotseat.operator_presence, "operator_active", return_value=active):
            pressed = hotseat.press_start_again(Path("/tmp/r"), lines.append, 61.0)
        return pressed, clicks, lines

    def test_a_staging_room_still_up_a_minute_on_is_pressed_again(self) -> None:
        pressed, clicks, lines = self.again(text("STAGING ROOM", "START GAME"))
        self.assertTrue(pressed)
        self.assertEqual(clicks, [(855, 1019)])
        self.assertIn("still up 61s after START GAME", lines[0])
        # A game that is loading, an unreadable screen, and a person at the
        # keyboard are all left alone.
        for rows, more in ((text("Loading, Please Wait..."), {}), ([], {"bounds": None}),
                           (text("START GAME"), {"active": True})):
            pressed, clicks, _ = self.again(rows, **more)
            self.assertEqual((pressed, clicks), (False, []))

    def test_the_backstop_runs_until_the_game_reports_and_never_on_a_held_start(self) -> None:
        source = (hotseat.HERE / "civ6_hotseat.py").read_text()
        self.assertIn("if not monitor.started and ours_to_start \\\n"
                      "                    and time.monotonic() - checked_start > START_CHECK_SECONDS:", source)
        self.assertIn('ours_to_start = not args.attach and not getattr(args, "hold_start", False)', source)
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            monitor = hotseat.Monitor(run_dir, "hs", [1], game_args(players=2), lambda m: None)
            self.addCleanup(monitor.close)
            self.assertFalse(monitor.started)
            with (run_dir / "events.jsonl").open("a") as out:
                out.write(json.dumps({"kind": "hotseat_armed"}) + "\n")
            monitor.poll()
            self.assertTrue(monitor.started, "an armed hand-over screen is a game that is up")


class SelfPlayTableTest(unittest.TestCase):
    """Operator 2026-11-27: "practice self playing both side this time
    (normally you will just play 1 side)"."""

    def started_with(self, *flags: str) -> argparse.Namespace:
        with patch.object(hotseat, "run", return_value=0) as run, \
                patch.object(hotseat, "read_current", return_value=None), \
                patch.object(hotseat, "strongest_agent", return_value="holy"):
            self.assertEqual(hotseat.main(list(flags)), 0)
        return run.call_args.args[0]

    def test_self_play_seats_civvis_on_every_seat(self) -> None:
        args = self.started_with("--duel", "--self-play", "--leader", "LEADER_GILGAMESH")
        self.assertEqual((args.players, args.civvis_seats, args.map_size), (2, 2, "MAPSIZE_DUEL"))
        # The strongest agent across the table, the flagship in the person's seat.
        self.assertEqual(args.agents, ["jack", "holy"])
        self.assertEqual(hotseat.jack_seats(args.players, args.civvis_seats), [0, 1])
        config = hotseat.hotseat_config(game_args(players=2, civvis_seats=2, agents=["jack", "holy"],
                                                  leader="LEADER_GILGAMESH"),
                                        Path("/tmp/r"), Path("/tmp/r/o.sqlite"))
        self.assertEqual((config["HotseatSeats"], config["HotseatHumanSeats"], config["HumanPlayers"]),
                         ([0, 1], [], 2))
        self.assertEqual(config["HotseatNames"], {"0": "CIVVIS JackOfAllTrades", "1": "CIVVIS HolyRoller"})
        self.assertEqual([install.hotseat_seat_config(config, seat)["VictoryLane"] for seat in (0, 1)],
                         ["civvis", "religious"])
        # Every seat a hotseat human's row in the staging room, no Firaxis AI.
        self.assertEqual(hotseat.wanted_slots(2, 2), ["host", "player"])
        self.assertEqual(self.started_with("--players", "4", "--self-play").civvis_seats, 4)
        self.assertEqual(self.started_with("--duel", "--self-play", "--agents", "holy,holy").agents,
                         ["holy", "holy"])

    def test_a_table_nobody_is_at_takes_no_persons_name(self) -> None:
        for flags in (["--duel", "--self-play", "--name", "dev"],
                      ["--players", "6", "--civvis-seats", "2", "--name", "dev"],
                      ["--players", "4", "--self-play", "--civvis-seats", "2"],
                      ["--players", "4", "--civvis-seats", "5"]):
            with patch.object(hotseat, "run") as run, patch.object(hotseat, "read_current", return_value=None), \
                    contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(hotseat.main(flags), 2, flags)
                run.assert_not_called()

    def test_attach_picks_a_table_back_up_as_the_table_it_is(self) -> None:
        def installed(**flags) -> dict:
            return hotseat.hotseat_config(game_args(**flags), Path("/tmp/r"), Path("/tmp/r/o.sqlite"))

        self.assertEqual(hotseat.attached_table(installed(players=2, agents=["holy"])), (2, None))
        self.assertEqual(hotseat.attached_table(installed(players=4, auto_human="stub")), (4, None))
        self.assertEqual(hotseat.attached_table(installed(players=2, civvis_seats=2, leader=None)), (2, 2))
        self.assertEqual(hotseat.attached_table(installed(players=6, civvis_seats=2, leader=None)), (6, 2))
        # An install from before the table's size was written down.
        old = installed(players=4)
        del old["HotseatPlayers"]
        self.assertEqual(hotseat.attached_table(old), (4, None))


class HostStrainTest(unittest.TestCase):
    """Live 2026-11-27: macOS ran out of application memory with Civ VI at
    6.87 GB, and at a load average of 40 System Events stopped answering.
    Neither failure looked like what it was, so the harness says what the Mac
    has to spare before it starts a game on it."""

    @staticmethod
    def host(free: str, swap: str = "total = 4096.00M  used = 3178.94M  free = 917.06M  (encrypted)"):
        answers = {"memory_pressure": f"...\nSystem-wide memory free percentage: {free}\n",
                   "hw.memsize": "17179869184\n", "vm.swapusage": swap}

        def run(command, **_):
            return argparse.Namespace(stdout=answers[command[-1]])
        return run

    def test_the_reading_is_macos_own(self) -> None:
        with patch.object(hotseat.play, "host_load", return_value=[8.2, 18.9, 23.7]), \
                patch.object(hotseat.os, "cpu_count", return_value=10):
            strain = hotseat.host_strain(self.host("62%"))
        self.assertEqual(strain, {"memory_free": 0.62, "memory_free_gb": 9.9, "swap_used_mb": 3179,
                                  "swap_total_mb": 4096, "load": [8.2, 18.9, 23.7], "cores": 10})
        self.assertEqual(hotseat.strain_warnings(strain), [])
        self.assertEqual(hotseat.strain_line(strain),
                         "host: 9.9 GB memory free (62%), swap 3179 of 4096 MB, load 8 on 10 cores")

    def test_too_little_memory_or_too_much_load_is_said_before_the_game_starts(self) -> None:
        with patch.object(hotseat.play, "host_load", return_value=[41.0, 60.0, 66.0]), \
                patch.object(hotseat.os, "cpu_count", return_value=10):
            strain = hotseat.host_strain(self.host("30%"))
        memory, load = hotseat.strain_warnings(strain)
        self.assertIn("only 4.8 GB of memory is free (30%)", memory)
        self.assertIn("load average is 41 on 10 cores", load)
        source = (hotseat.HERE / "civ6_hotseat.py").read_text()
        self.assertLess(source.index('began["host"] = host_strain()'),
                        source.index("process = launcher.launch("), "said before Civ VI is started")

    def test_a_host_that_does_not_say_is_not_a_host_under_strain(self) -> None:
        def nothing(command, **_):
            raise OSError("no such tool")
        with patch.object(hotseat.play, "host_load", return_value=None):
            strain = hotseat.host_strain(nothing)
        self.assertIsNone(strain["memory_free"])
        self.assertEqual(hotseat.strain_warnings(strain), [])
        self.assertEqual(hotseat.strain_line(strain), "host: memory unread")


class ClosingCivTest(unittest.TestCase):
    """Live 2026-11-27 (selfplay-duel-20261127T0654Z): a re-attached harness
    left Civ VI on the Victory screen, the supervisor re-attached to the
    finished game every 25 s, and the quit's sixty seconds ran out on a
    loaded host as ARE YOU SURE? came up."""

    def test_a_reattached_harness_owns_the_civ_its_run_launched(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run_dir = Path(tmp)
            self.assertFalse(hotseat.launched_here(run_dir, [27184]), "no launch on record")
            hotseat.note_launched(run_dir, 27184)
            self.assertTrue(hotseat.launched_here(run_dir, [27184]))
            # A Civ VI the person opened since is not this run's to close.
            self.assertFalse(hotseat.launched_here(run_dir, [31337]))
            self.assertFalse(hotseat.launched_here(run_dir, []))
            (run_dir / hotseat.LAUNCHED).write_text("not json")
            self.assertFalse(hotseat.launched_here(run_dir, [27184]))
        source = (hotseat.HERE / "civ6_hotseat.py").read_text()
        self.assertIn("launched = bool(args.attach) and launched_here(run_dir)", source)
        self.assertIn("launched = True\n            note_launched(run_dir, process.pid)", source)

    def quit(self, screens: list, *, pids_after: int, seconds_a_look: float = 10.0,
             stop_closes: bool = False) -> tuple[bool, list, int, list]:
        """`quit_civ` against these screens in turn, a look taking
        `seconds_a_look`; Civ VI exits once `pids_after` clicks have landed."""
        clock = [0.0]
        clicks, lines = [], []
        reads = iter(screens)
        asked = []

        def read(run_dir, name, fallback=None):
            clock[0] += seconds_a_look
            return (0, 0, 1710, 1107), next(reads, [])

        def running():
            return [] if len(clicks) >= pids_after else [27184]

        def stop(timeout_s):
            asked.append(timeout_s)
            if stop_closes:
                clicks.extend([None] * pids_after)

        with patch.object(hotseat, "read_screen", side_effect=read), \
                patch.object(hotseat.play, "click_at", side_effect=lambda x, y: clicks.append((x, y))), \
                patch.object(hotseat.env, "game_pids", side_effect=running), \
                patch.object(hotseat.env, "request_macos_quit") as menu_quit, \
                patch.object(hotseat.launcher, "stop", side_effect=stop), \
                patch.object(hotseat.time, "monotonic", side_effect=lambda: clock[0]), \
                patch.object(hotseat.time, "sleep", side_effect=lambda s: clock.__setitem__(0, clock[0] + s)):
            closed = hotseat.quit_civ(Path("/tmp/r"), lines.append)
        return closed, [c for c in clicks if c], menu_quit.call_count, asked

    def test_the_question_that_comes_up_late_is_still_answered(self) -> None:
        victory = text("VICTORY", "Just One More Turn...")
        question = text("ARE YOU SURE?", "YES", "NO")
        # As measured: five looks at the Victory screen, then the question.
        closed, clicks, asked_menu, stops = self.quit([victory] * 5 + [question], pids_after=1)
        self.assertTrue(closed)
        self.assertEqual(clicks, [(855, 1040)])
        self.assertEqual(asked_menu, 2, "asked again after three looks with no menu")
        self.assertEqual(stops, [], "the launcher's stop is not needed")

    def test_a_question_up_at_the_deadline_is_given_its_time(self) -> None:
        victory, question = text("VICTORY"), text("ARE YOU SURE?", "YES", "NO")
        looks = int(hotseat.QUIT_SECONDS // 12)
        # The question is read as the clock runs out (a look is twelve seconds
        # here), and its first press does not take: the clock gives a question
        # that is up its time, so it is pressed again rather than given up on.
        closed, clicks, _, stops = self.quit([victory] * looks + [question, question],
                                             pids_after=2)
        self.assertTrue(closed)
        self.assertEqual(len(clicks), 2)
        self.assertEqual(stops, [])

    @staticmethod
    def question(*, yes: bool) -> list[dict]:
        """The pause menu with its inline question, as read on 2026-11-27."""
        rows = [("MENU", 855, 333), ("RETURN TO GAME", 855, 397), ("ARE YOU SURE?", 856, 502),
                ("Are you sure you wish to quit your game?", 856, 536), ("No", 953, 588),
                ("EXIT TO DESKTOP", 853, 641)]
        if yes:
            rows.append(("YES", 759, 588))
        return [{"text": label, "x": x, "y": y, "norm": hotseat._norm(label)} for label, x, y in rows]

    def test_a_yes_the_screen_reader_misses_is_pressed_where_it_stands(self) -> None:
        # duel-dev-smoke-20261127T0845Z: nine looks, never the white-on-red YES.
        unread = self.question(yes=False)
        self.assertEqual([(row["x"], row["y"]) for row in
                          hotseat.mirrored_yes(unread, hotseat.find(unread, "ARE YOU SURE", exact=False))],
                         [(759, 588)], "NO mirrored about the heading")
        closed, clicks, _, stops = self.quit([unread], pids_after=1)
        self.assertTrue(closed)
        self.assertEqual((clicks, stops), ([(759, 588)], []))
        # A YES that is read is pressed where it was read.
        closed, clicks, _, _ = self.quit([self.question(yes=True)], pids_after=1)
        self.assertEqual(clicks, [(759, 588)])
        # No question, or no NO under it: nothing is made up.
        self.assertEqual(hotseat.mirrored_yes(unread, []), [])
        above = [row for row in unread if row["text"] != "No"] + text("NO")[:0]
        self.assertEqual(hotseat.mirrored_yes(above, hotseat.find(above, "ARE YOU SURE", exact=False)), [])

    def test_the_quits_clock_has_an_end_whatever_is_read(self) -> None:
        # A menu that never goes away renews the answer window every look; the
        # clock still runs out, and the launcher's stop gets its turn.
        menu = [row for row in self.question(yes=False) if row["text"] not in
                ("ARE YOU SURE?", "Are you sure you wish to quit your game?", "No")]
        closed, clicks, _, stops = self.quit([menu] * 500, pids_after=10 ** 6, stop_closes=False)
        self.assertFalse(closed)
        self.assertEqual(stops, [hotseat.QUIT_STOP_SECONDS])
        looks = len(clicks)
        self.assertLessEqual(looks * 12, hotseat.QUIT_SECONDS + hotseat.QUIT_OVERTIME_SECONDS + 12)
        self.assertGreater(looks * 12, hotseat.QUIT_SECONDS, "past the clock, as far as it may go")

    def test_a_civ_that_shows_no_menu_gets_the_launchers_stop_and_a_longer_one(self) -> None:
        closed, clicks, asked_menu, stops = self.quit([], pids_after=1, stop_closes=True)
        self.assertTrue(closed)
        self.assertEqual((clicks, stops), ([], [hotseat.QUIT_STOP_SECONDS]))
        self.assertGreaterEqual(asked_menu, 3)
        self.assertGreaterEqual(hotseat.QUIT_SECONDS, 120)
        closed, _, _, _ = self.quit([], pids_after=1, stop_closes=False)
        self.assertFalse(closed, "a Civ VI that will not go is reported, not assumed gone")

    def test_a_read_the_host_does_not_answer_uses_the_window_last_read(self) -> None:
        window = (0, 0, 1710, 1107)
        observation = {"text": "START GAME"}
        asked = []

        def recognize(shot, bounds):
            asked.append(bounds)
            return [observation]

        answers = iter([window, None, None])
        with tempfile.TemporaryDirectory() as tmp, \
                patch.dict(hotseat.LAST_WINDOW, {"bounds": None}), \
                patch.object(hotseat.play, "game_window", side_effect=lambda: next(answers)), \
                patch.object(hotseat.play, "screenshot", return_value=True), \
                patch.object(hotseat.play, "desktop_size", return_value=(1710, 1107)), \
                patch.object(hotseat.play, "recognize_once", side_effect=recognize), \
                patch.object(hotseat.play, "_observation_point", return_value=(0.5, 0.94)):
            run_dir = Path(tmp)
            first = hotseat.read_screen(run_dir, "a")
            again = hotseat.read_screen(run_dir, "b")
            given = hotseat.read_screen(run_dir, "c", (10, 10, 800, 600))
        self.assertEqual(first[0], window)
        self.assertEqual(again[0], window, "System Events did not answer: the window as last read")
        self.assertEqual([row["text"] for row in again[1]], ["START GAME"])
        self.assertEqual(given[0], (10, 10, 800, 600), "a step's own window comes first")
        with patch.dict(hotseat.LAST_WINDOW, {"bounds": None}), \
                patch.object(hotseat.play, "game_window", return_value=None):
            self.assertEqual(hotseat.read_screen(Path("/tmp/r"), "never"), (None, []))


if __name__ == "__main__":
    unittest.main()
