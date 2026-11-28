#!/usr/bin/env python3
"""`civ6_hotseat.py --manual-lobby`: a person sets the game up, nothing touches
the screen, and the installed mod is inert without its harness.

The Lua side of every inertness claim is `tools/civ6_control/mod/
harness_gate_test.lua`; this holds the harness, the installer and the one
query both sides share.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sqlite3
import subprocess
import sys
import tempfile
import textwrap
import time
import unittest
import xml.etree.ElementTree as ET
from contextlib import closing
from pathlib import Path
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import civ6_hotseat as hotseat  # noqa: E402
from civ6_control import install, screen_free  # noqa: E402

TAG = "hs-manual-test"


def manual_args(**overrides) -> argparse.Namespace:
    """The parser's own defaults for `--manual-lobby`, as `main` hands them on."""
    with patch.object(hotseat, "run_manual", side_effect=lambda a: a), \
            patch.object(hotseat.screen_free, "engaged", return_value=True), \
            patch.object(hotseat, "read_current", return_value=None):
        args = hotseat.main(["--manual-lobby", "--tag", TAG])
    for key, value in overrides.items():
        setattr(args, key, value)
    return args


class ScreenFreeTest(unittest.TestCase):
    """The screen modules are never imported on this path: pinned by importing
    the harness and a decision worker the way `--manual-lobby` runs them."""

    PROBE = textwrap.dedent("""
        import json, sys
        sys.path.insert(0, sys.argv[1])
        sys.argv = ["civ6_hotseat.py", "--manual-lobby"]
        import civ6_hotseat, civ6_brain
        from civ6_control import screen_free
        refused = []
        for call in (lambda: civ6_hotseat.play.click_at(1, 2),
                     lambda: civ6_hotseat.play.screenshot("/tmp/never.png"),
                     lambda: civ6_hotseat.play.game_window(),
                     lambda: civ6_hotseat.play.macos_input.press_key("escape"),
                     lambda: civ6_hotseat.play.macos_window.place_game("x"),
                     lambda: civ6_hotseat.popup_clear.frontmost()):
            try:
                call()
            except screen_free.ScreenAutomationOff:
                refused.append(True)
        print(json.dumps({
            "engaged": screen_free.engaged(),
            "real": screen_free.loaded_for_real(),
            "pil": sorted(m for m in sys.modules if m == "PIL" or m.startswith("PIL.")),
            "stand_ins": sorted(n for n in screen_free.SCREEN_MODULES
                                if isinstance(sys.modules.get(n), screen_free.StandIn)),
            "refused": len(refused),
        }))
    """)

    def test_the_harness_and_its_workers_load_no_screen_module(self) -> None:
        env = {k: v for k, v in os.environ.items() if k != screen_free.ENV}
        out = subprocess.run([sys.executable, "-c", self.PROBE, str(HERE)], env=env,
                             capture_output=True, text=True, timeout=120, check=True)
        report = json.loads(out.stdout.strip().splitlines()[-1])
        self.assertTrue(report["engaged"], "the flag alone engages the guard before civ6_control loads")
        self.assertEqual(report["real"], [], "no screen module was imported as itself")
        self.assertEqual(report["pil"], [], "Pillow is never loaded")
        self.assertEqual(report["stand_ins"], sorted(screen_free.SCREEN_MODULES))
        self.assertEqual(report["refused"], 6, "every click, capture and window move is refused")

    def test_pillow_reads_as_absent(self) -> None:
        probe = ("import sys; sys.path.insert(0, sys.argv[1]); import civ6_control\n"
                 "try:\n    import PIL\nexcept ModuleNotFoundError:\n    print('absent')\n")
        out = subprocess.run([sys.executable, "-c", probe, str(HERE)],
                             env=dict(os.environ, **{screen_free.ENV: "1"}),
                             capture_output=True, text=True, timeout=60, check=True)
        self.assertEqual(out.stdout.strip(), "absent")

    def test_engaging_after_a_real_import_is_refused(self) -> None:
        probe = ("import sys; sys.path.insert(0, sys.argv[1])\n"
                 "from civ6_control import macos_input, screen_free\n"
                 "try:\n    screen_free.engage()\nexcept RuntimeError as e:\n    print('refused')\n")
        env = {k: v for k, v in os.environ.items() if k != screen_free.ENV}
        out = subprocess.run([sys.executable, "-c", probe, str(HERE)], env=env,
                             capture_output=True, text=True, timeout=60, check=True)
        self.assertEqual(out.stdout.strip(), "refused")

    def test_unset_changes_nothing(self) -> None:
        # Every existing caller: the guard is not engaged in this test process.
        self.assertFalse(screen_free.engaged())
        self.assertIn("civ6_control.macos_input", screen_free.loaded_for_real())

    def test_manual_lobby_never_runs_with_the_screen_modules_loaded(self) -> None:
        with patch.object(hotseat, "run_manual") as run, \
                patch.object(hotseat, "read_current", return_value=None):
            self.assertEqual(hotseat.main(["--manual-lobby"]), 2)
        run.assert_not_called()

    def test_the_manual_path_names_no_screen_function(self) -> None:
        import ast
        import inspect
        names = set()
        for function in (hotseat.run_manual, hotseat.manual_stall_note, hotseat.manual_lobby_config):
            for node in ast.walk(ast.parse(textwrap.dedent(inspect.getsource(function)))):
                if isinstance(node, ast.Name):
                    names.add(node.id)
                elif isinstance(node, ast.Attribute):
                    names.add(node.attr)
        for name in ("set_up_game", "rescue_popup", "end_stalled_turn", "quit_civ", "launcher",
                     "park_upper_left", "fit_on_screen", "place_game", "read_screen", "click",
                     "apply_person_options", "clear_run_logs", "notify", "play", "popup_clear",
                     "operator_presence", "wait_for_hands_off", "launch"):
            self.assertNotIn(name, names, f"the manual path names {name}")


class ManualConfigTest(unittest.TestCase):
    def test_every_seat_is_installed_and_gated_on_this_harness(self) -> None:
        config = hotseat.manual_lobby_config(manual_args(), Path("/r"), Path("/r/o.sqlite"))
        self.assertTrue(config["RequireHarness"])
        self.assertEqual(config["HarnessRun"], TAG)
        self.assertEqual(config["HarnessFreshSeconds"], hotseat.HARNESS_FRESH_S)
        self.assertEqual(config["HotseatSeats"], hotseat.MANUAL_SEATS)
        self.assertEqual(config["HotseatHumanSeats"], [0])
        self.assertEqual(config["HotseatStubSeats"], [])
        self.assertEqual(config["HotseatNames"],
                         {str(s): f"CIVVIS jack {s}" for s in hotseat.MANUAL_SEATS})
        self.assertEqual(config["OrdersDb"], "/r/o.sqlite")
        # #5943: a person's game gets the cover and their hand-over pressed,
        # both gated on this harness in the mod (harness_gate_test.lua part 6).
        self.assertTrue(config["HotseatCover"] and config["HotseatAutoStart"])
        self.assertTrue(install.hotseat_cover_wanted(config))
        # And the cover's own: quiet, with a wait bar (asked otherwise, as asked).
        self.assertEqual((config["HotseatCoverSound"], config["HotseatCoverTimer"]), ("music", True))

    def test_nothing_is_set_on_the_persons_game(self) -> None:
        config = hotseat.manual_lobby_config(manual_args(), Path("/r"), Path("/r/o.sqlite"))
        for key in ("MaxTurns", "Difficulty", "PlayerDifficulty", "GameSpeed", "MapScript",
                    "MapSize", "Leader", "HumanPlayers"):
            self.assertIsNone(config[key], key)
        self.assertEqual(config["VictoriesOff"], [])
        for key in ("DebugTimeScale", "DebugTimeScaleAB", "VSyncABTurns"):
            self.assertIsNone(config[key], key)

    def test_the_seats_do_not_depend_on_any_count_the_person_gave(self) -> None:
        one = hotseat.manual_lobby_config(manual_args(players=2), Path("/r"), Path("/r/o"))
        six = hotseat.manual_lobby_config(manual_args(players=6), Path("/r"), Path("/r/o"))
        self.assertEqual(one["HotseatSeats"], six["HotseatSeats"])

    def test_the_stub_plays_row_one_for_a_test(self) -> None:
        config = hotseat.manual_lobby_config(manual_args(auto_human="stub"), Path("/r"), Path("/r/o"))
        self.assertEqual(config["HotseatStubSeats"], [0])
        self.assertEqual(config["HotseatHumanSeats"], [])
        self.assertFalse(install.hotseat_cover_wanted(config), "no person, no cover")

    def test_the_default_paths_carry_no_new_key(self) -> None:
        from test_civ6_hotseat import game_args
        config = hotseat.hotseat_config(game_args(), Path("/r"), Path("/r/o.sqlite"))
        for key in ("RequireHarness", "HarnessRun", "HarnessFreshSeconds"):
            self.assertNotIn(key, config)
        self.assertEqual(config["MaxTurns"], 250)

    def test_flags_the_lobby_decides_are_refused(self) -> None:
        for extra in (["--attach"], ["--resume", "x.Civ6Save"], ["--civvis-seats", "1"],
                      ["--victories-off", "VICTORY_DIPLOMATIC"]):
            with patch.object(hotseat, "run_manual") as run:
                self.assertEqual(hotseat.main(["--manual-lobby", *extra]), 2, extra)
            run.assert_not_called()


class PersonFacingInstallTest(unittest.TestCase):
    def install(self, config: dict) -> Path:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        with patch.object(install, "check_syntax", return_value=None):
            install._write_mod(Path(tmp.name), config)
        return Path(tmp.name)

    def test_no_create_game_default_is_changed(self) -> None:
        config = hotseat.manual_lobby_config(manual_args(), Path("/r"), Path("/r/o.sqlite"))
        out = self.install(config)
        tree = ET.fromstring((out / "CivvisControl.modinfo").read_text())
        self.assertIsNone(tree.find("./FrontEndActions"), "no UpdateDatabase of the setup screen")
        self.assertIsNotNone(tree.find("./InGameActions"))
        self.assertEqual(tree.findtext("./Properties/EnabledByDefault"), "1")
        contexts = [f.text for f in tree.findall(
            "./InGameActions/AddUserInterfaces[@id='CivvisControlAgentUI']/File")]
        self.assertEqual(contexts, [f"CivvisControlAgentSeat{s}.xml" for s in hotseat.MANUAL_SEATS])
        files = {f.text for f in tree.findall("./Files/File")}
        for action in tree.iter():
            for child in action.findall("File"):
                if action.tag != "Files":
                    self.assertIn(child.text, files, "every file an action names is listed")
        covers = [f.text for f in tree.findall(
            "./InGameActions/AddUserInterfaces[@id='CivvisHotseatCoverUI']/File")]
        self.assertEqual(covers, ["CivvisHotseatCover.xml"], "the person's cover is installed")
        self.assertIn("CivvisHotseatCover.lua", files)
        for script in ["CivvisControlAutoClose.lua", "CivvisControlHeartbeat.lua",
                       "CivvisHotseatCover.lua"] + [
                f"CivvisControlAgentSeat{s}.lua" for s in hotseat.MANUAL_SEATS]:
            head = (out / script).read_text().split("}\n", 1)[0]
            self.assertIn("RequireHarness = true,", head, script)
            self.assertIn(f'HarnessRun = "{TAG}",', head, script)
            self.assertIn("MaxTurns = nil,", head, "no turn limit set in the game: " + script)
            size = len((out / script).read_bytes())
            self.assertLess(size, install.LUA_SIZE_KNOWN_GOOD, script)

    def test_the_operator_install_keeps_its_frontend_defaults(self) -> None:
        from test_civ6_hotseat import game_args
        out = self.install(hotseat.hotseat_config(game_args(), Path("/r"), Path("/r/o")))
        tree = ET.fromstring((out / "CivvisControl.modinfo").read_text())
        self.assertIsNotNone(tree.find("./FrontEndActions/UpdateDatabase"))
        self.assertNotIn("RequireHarness", (out / "CivvisControlAutoClose.lua").read_text()
                         .split("}\n", 1)[0])

    def test_require_harness_without_seats_is_refused(self) -> None:
        with self.assertRaises(SystemExit):
            self.install({"RunTag": "x", "RequireHarness": True})
        with self.assertRaises(SystemExit):
            self.install({"RunTag": "x", "RequireHarness": True, "Hotseat": True,
                          "HotseatSeats": [1]})


class HarnessBeatTest(unittest.TestCase):
    def test_the_beat_reads_back_as_the_mod_reads_it(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            db = Path(tmp) / "orders.sqlite"
            beat = hotseat.HarnessBeat(db, TAG)
            self.assertIsNone(beat.age(), "no beat yet")
            self.assertTrue(beat.beat())
            age = beat.age()
            self.assertIsNotNone(age)
            self.assertTrue(0 <= age <= 2, age)
            beat.clear()
            self.assertIsNone(beat.age(), "a stop deletes the row")

    def test_a_beat_is_rewritten_only_every_two_seconds(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            clock = [100.0]
            beat = hotseat.HarnessBeat(Path(tmp) / "o.sqlite", TAG, clock=lambda: clock[0])
            beat.beat()
            with closing(sqlite3.connect(str(Path(tmp) / "o.sqlite"))) as db:
                db.execute("UPDATE harness SET beat = 1")
                db.commit()
            beat.beat()
            self.assertGreater(beat.age(), 1000, "inside the interval the row is not touched")
            clock[0] += hotseat.HARNESS_BEAT_S
            beat.beat()
            self.assertLessEqual(beat.age(), 2)

    def test_the_mod_sends_this_exact_query(self) -> None:
        head, tail = hotseat.HARNESS_AGE_SQL.split("{run}")
        self.assertEqual(tail, "'")
        select, _, rest = head.partition(" FROM")
        for name in ("CivvisControlAgent.lua", "CivvisControlAutoClose.lua",
                     "CivvisControlHeartbeat.lua"):
            source = (install.MOD_SOURCE / name).read_text()
            self.assertIn(f'DB.Query("{select}"\n\t\t\t.. " FROM{rest}" .. run .. "\'")', source, name)
            self.assertIn(f"tonumber(cfg.HarnessFreshSeconds) or {hotseat.HARNESS_FRESH_S}", source)


MODDING = """\
[10.0] Stage: Configure Content
[10.0] Game Mode: {mode}
[10.0] Map Script: Pangaea.lua
[10.0] Map Size: 0xdc23bbc1
[10.0] Human Players: {humans}
[10.0] AI Players: 1
[10.0] Applied in-game actions (in order of application):
{ours}[12.0] Applying Component - CivvisControlHeartbeatHUD (ReplaceUIScript)
[16.0] Stage: Initializing a new game
"""


class ModdingLogTest(unittest.TestCase):
    def log(self, tmp: str, text: str, mode: str = "a") -> Path:
        path = Path(tmp) / "Modding.log"
        with path.open(mode) as handle:
            handle.write(text)
        return path

    def test_a_hotseat_game_with_the_mod(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = self.log(tmp, "[1.0] earlier launch\n", "w")
            reader = hotseat.ModdingLog(path)
            self.log(tmp, MODDING.format(mode="HOTSEAT", humans=3, ours=(
                "[12.0] Applying Component - CivvisControlAgentUI (AddUserInterfaces)\n")))
            (block,) = reader.poll()
            self.assertEqual((block["mode"], block["map"], block["humans"], block["ais"]),
                             ("HOTSEAT", "Pangaea.lua", 3, 1))
            self.assertTrue(block["civvis"])
            self.assertEqual(block["stage"], "Initializing a new game")
            self.assertIsNone(hotseat.modding_verdict(block))
            self.assertEqual(reader.poll(), [], "each game once")

    def test_a_hotseat_game_without_the_mod_says_restart(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            reader = hotseat.ModdingLog(self.log(tmp, "", "w"))
            self.log(tmp, MODDING.format(mode="HOTSEAT", humans=2, ours=""))
            (block,) = reader.poll()
            self.assertFalse(block["civvis"])
            verdict = hotseat.modding_verdict(block)
            self.assertIn("without the CIVVIS mod", verdict)
            self.assertIn("quit Civ VI completely, start it again", verdict)

    def test_a_single_player_game_is_not_ours(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            reader = hotseat.ModdingLog(self.log(tmp, "", "w"))
            self.log(tmp, MODDING.format(mode="Single-Player", humans=1, ours=""))
            (block,) = reader.poll()
            self.assertIn("plays only Hot Seat games", hotseat.modding_verdict(block))

    def test_a_new_launch_starts_a_new_log(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            reader = hotseat.ModdingLog(self.log(tmp, "x" * 5000 + "\n", "w"))
            Path(tmp, "Modding.log").unlink()
            self.log(tmp, MODDING.format(mode="HOTSEAT", humans=2, ours=""), "w")
            self.assertEqual(len(reader.poll()), 1, "a recreated log is read from its start")

    def test_a_block_with_no_closing_stage_is_settled(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            clock = [0.0]
            reader = hotseat.ModdingLog(self.log(tmp, "", "w"), clock=lambda: clock[0])
            self.log(tmp, "[1.0] Stage: Configure Content\n[1.0] Game Mode: HOTSEAT\n")
            self.assertEqual(reader.poll(), [])
            clock[0] = 31.0
            self.assertEqual(len(reader.poll()), 1)


def lobby(*slots: str, **extra) -> dict:
    players = [{"id": i, "slot": s, "human": s == "human", "difficulty": "DIFFICULTY_PRINCE"}
               for i, s in enumerate(slots)]
    return dict({"kind": "hotseat_lobby", "players": players, "hotseat": True,
                 "map": "Pangaea.lua", "size": "MAPSIZE_TINY", "speed": "GAMESPEED_ONLINE",
                 "max_turns": 250}, **extra)


class LobbyTest(unittest.TestCase):
    def test_every_human_row_after_the_first_is_civvis(self) -> None:
        seats, problems = hotseat.lobby_seats(lobby("human", "human", "computer", "human"))
        self.assertEqual(seats, [1, 3])
        self.assertEqual(problems, [])

    def test_only_the_person_is_human(self) -> None:
        seats, problems = hotseat.lobby_seats(lobby("human", "computer", "computer"))
        self.assertEqual(seats, [])
        (problem,) = problems
        self.assertIn("no seat to play", problem)
        self.assertIn("at least one more player set to Human", problem)

    def test_a_human_row_past_the_installed_seats(self) -> None:
        seats, problems = hotseat.lobby_seats(lobby(*["human"] * 8))
        self.assertEqual(seats, hotseat.MANUAL_SEATS)
        (problem,) = problems
        self.assertIn("row 7 and row 8 are Human", problem)
        self.assertIn("set them to AI", problem)

    def test_the_first_row_must_be_the_persons(self) -> None:
        seats, problems = hotseat.lobby_seats(lobby("computer", "human"))
        self.assertEqual(seats, [1])
        self.assertIn("The first row is not Human", problems[0])

    def test_the_game_is_read_back_in_one_line(self) -> None:
        line = hotseat.describe_lobby(lobby("human", "human", "computer"), [1])
        self.assertIn("Hot Seat, 3 players on Pangaea.lua MAPSIZE_TINY at GAMESPEED_ONLINE", line)
        self.assertIn("turn limit 250", line)
        self.assertIn("row 1 you (human, DIFFICULTY_PRINCE)", line)
        self.assertIn("row 2 CIVVIS jack 1 (human", line)
        self.assertIn("row 3 Civ VI AI (computer", line)

    def test_a_stall_is_said_not_clicked(self) -> None:
        lines: list[str] = []
        with tempfile.TemporaryDirectory() as tmp:
            args = manual_args(auto_human=None, no_notify=True)
            monitor = hotseat.Monitor(Path(tmp), TAG, hotseat.MANUAL_SEATS, args, lines.append)
            monitor.active, monitor.last_event = 1, 0.0
            with patch.object(hotseat, "rescue_popup") as rescue, \
                    patch.object(hotseat, "end_stalled_turn") as end:
                self.assertTrue(hotseat.manual_stall_note(monitor, lines.append, 400.0))
                self.assertFalse(hotseat.manual_stall_note(monitor, lines.append, 500.0))
            monitor.close()
            rescue.assert_not_called()
            end.assert_not_called()
        self.assertEqual(len(lines), 1)
        self.assertIn("Nothing here clicks", lines[0])
        self.assertIn("NEXT TURN", lines[0])

    def test_a_stall_is_said_on_a_machine_booted_moments_ago(self) -> None:
        # `time.monotonic()` counts from boot: a CI runner up 40 s reads 40.
        lines: list[str] = []
        with tempfile.TemporaryDirectory() as tmp:
            monitor = hotseat.Monitor(Path(tmp), TAG, hotseat.MANUAL_SEATS,
                                      manual_args(auto_human=None, no_notify=True), lines.append)
            monitor.active, monitor.last_event = 1, 0.0
            self.assertTrue(hotseat.manual_stall_note(monitor, lines.append, 40.0))
            monitor.close()
        self.assertEqual(len(lines), 1)

    def test_stop_leaves_the_persons_game_up(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            current = {"pid": 1234, "tag": TAG, "run_dir": tmp, "manual_lobby": True}
            alive = iter([True, False])
            out: list[str] = []
            with patch.object(hotseat, "read_current", return_value=current), \
                    patch.object(hotseat, "harness_alive", side_effect=lambda _p: next(alive, False)), \
                    patch.object(hotseat.os, "kill") as kill, \
                    patch("builtins.print", side_effect=lambda *a, **k: out.append(" ".join(map(str, a)))):
                self.assertEqual(hotseat.stop_command(argparse.Namespace(quit_game=True, resign=False)), 0)
            kill.assert_called_once()
            self.assertTrue(any("they stay up" in line for line in out))


#: The end-to-end run: `--manual-lobby` in its own process, screen-free as it
#: runs for a person, against a scripted Civ VI (its two logs and its pid).
CHILD = textwrap.dedent(r"""
    import json, os, signal, subprocess, sys, time
    from pathlib import Path
    tools, tmp, scenario = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
    sys.path.insert(0, str(tools))
    sys.argv = ["civ6_hotseat.py", "--manual-lobby"]
    import civ6_hotseat as hotseat
    import civ6_env as env
    from civ6_control import install, screen_free

    REFUSED = {"swiftc", "cliclick", "screencapture"}
    spawned = []
    RealPopen = subprocess.Popen
    class Watched(RealPopen):
        def __init__(self, args, *rest, **kw):
            argv = [str(a) for a in ([args] if isinstance(args, (str, bytes)) else args)]
            spawned.append(argv)
            if os.path.basename(argv[0]) in REFUSED or "System Events" in " ".join(argv):
                raise AssertionError(f"a screen tool was started: {argv}")
            super().__init__(args, *rest, **kw)
    subprocess.Popen = Watched

    logs = tmp / "Logs"; logs.mkdir()
    (logs / "Modding.log").write_text("[1.0] an earlier launch\n")
    automation = logs / "Automation.log"; automation.write_text("")
    env.logs_dir = lambda: logs
    env.assets_dir = lambda: tmp / "Assets"
    (tmp / "Assets" / "DLC").mkdir(parents=True)
    install._drop_mod_index = lambda: None
    install.check_syntax = lambda path: None
    pids = []
    def listed(*a, **k):
        counts["ps"] += 1
        return list(pids)
    env.game_pids = listed
    # The scripted game's pids, as `os.kill(pid, 0)` would find them.
    hotseat.watch.GameLiveness._running = staticmethod(lambda pid: pid in pids)
    def refuse(*a, **k):
        raise AssertionError("the manual lobby launched or set something up")
    hotseat.launcher.launch = refuse
    hotseat.apply_person_options = refuse
    started, lanes, recorded = [], [], []
    class FakeBrains:
        def __init__(self, run_dir, tag, seats, orders_db, binary, victory, log, treatments=None):
            started.append(list(seats))
            lanes.append(victory)
        def start_all(self): pass
        def keep_alive(self): pass
        def stop(self): pass
    hotseat.Brains = FakeBrains
    # The people's record as the harness posts to it: what the summary said then.
    def record(run_dir, log, resigned=False):
        recorded.append({"resigned": resigned,
                         "summary": json.loads((run_dir / "summary.json").read_text())})
    hotseat.record_person_game = record
    # How often the log is relayed against how often the loop's upkeep runs.
    counts = {"pumps": 0, "beats": 0, "ps": 0}
    RealPump, RealBeat = hotseat.watch.EventLogBridge.pump, hotseat.HarnessBeat.beat
    def counted_pump(self):
        counts["pumps"] += 1
        return RealPump(self)
    def counted_beat(self):
        counts["beats"] += 1
        return RealBeat(self)
    hotseat.watch.EventLogBridge.pump = counted_pump
    hotseat.HarnessBeat.beat = counted_beat

    TAG = "hs-manual-e2e"
    def event(**fields):
        with automation.open("a") as handle:
            handle.write("CIVVISJSON " + json.dumps(fields) + "\n")
    def modding(ours):
        with (logs / "Modding.log").open("a") as handle:
            handle.write("[9.0] Stage: Configure Content\n[9.0] Game Mode: HOTSEAT\n"
                         "[9.0] Map Script: Pangaea.lua\n[9.0] Human Players: 3\n[9.0] AI Players: 1\n"
                         + ("[9.5] Applying Component - CivvisControlAgentUI (AddUserInterfaces)\n"
                            if ours else "") + "[9.9] Stage: Initializing a new game\n")
    players = {"alone": ["human", "computer"]}.get(scenario, ["human", "human", "human", "computer"])
    LOBBY = dict(ctx="heartbeat", kind="hotseat_lobby", run=TAG, turn=1, hotseat=True,
                 map="Pangaea.lua", size="MAPSIZE_TINY", speed="GAMESPEED_ONLINE", max_turns=250,
                 t=1795800000,
                 players=[{"id": i, "slot": s, "human": s == "human", "leader": f"LEADER_{i}",
                           "difficulty": "DIFFICULTY_KING" if i == 0 else "DIFFICULTY_PRINCE"}
                          for i, s in enumerate(players)])
    def turn(seat, phase, human):
        event(ctx="autoclose", screen="PlayerChange", kind="hotseat_turn", run=TAG, phase=phase,
              player=seat, turn=1, human=human, civvis=not human)
    steps = [lambda: pids.append(4242), lambda: modding(scenario != "nomod")]
    if scenario not in ("nomod",):
        steps += [lambda: event(**LOBBY), lambda: turn(0, "begin", True), lambda: turn(0, "end", True),
                  lambda: turn(1, "begin", False)]
    if scenario == "stall":
        steps += [lambda: None] * 30           # 15 s on the harness's clock, past --stall-seconds 5
    if scenario not in ("nomod", "alone"):
        steps += [lambda: event(ctx="agent", kind="turn", run=TAG + "-p1", turn=1, orders_applied=3,
                                orders_source="civvis", cities=1, units=2, score=5),
                  lambda: turn(1, "end", False)]
    if scenario == "won":
        # The table's own record of the person's victory (CivvisControlHeartbeat.lua
        # `CivvisEnding.onVictory`), then "one more turn" and Civ VI closed later.
        steps += [lambda: event(ctx="heartbeat", kind="victory", run=TAG, turn=1, t=1795803600,
                                team=0, victory=3, victory_type="VICTORY_TECHNOLOGY", members=[0],
                                source="table", local_player=0)] + [lambda: None] * 4
    if scenario == "resign":
        # `--stop --resign`: its request, then the signal (stop_command).
        steps += [lambda: (hotseat.RUN_ROOT / TAG / "stop.json").write_text(
            json.dumps({"quit_game": False, "resign": True})), lambda: os.kill(os.getpid(), signal.SIGTERM)]
    elif scenario in ("close", "stall", "won"):
        steps += [lambda: pids.clear()]
    else:
        steps += [lambda: None] * 6 + [lambda: os.kill(os.getpid(), signal.SIGTERM)]
    # The game drives the harness's clock: each of its sleeps advances
    # `time.monotonic` by exactly what it asked for, and the game plays its
    # next step each time the clock passes a loop's pass (the relay sleeps in
    # 20 ms steps between passes), so nothing depends on how fast this machine
    # runs. It starts at 10 s, as on a CI runner booted moments ago. A run that
    # never ends on its own is stopped after 10 minutes of its clock.
    clock = [10.0]
    due = [10.0 + hotseat.PASS_S]
    time.monotonic = lambda: clock[0]
    def fake_sleep(seconds):
        clock[0] += seconds
        if clock[0] > 610.0:
            raise SystemExit("the run never ended")
        if clock[0] + 1e-9 >= due[0]:
            due[0] += hotseat.PASS_S
            if steps:
                steps.pop(0)()
    time.sleep = fake_sleep
    binary = tmp / "civvis_orders"; binary.write_text("#!/bin/sh\n"); binary.chmod(0o755)
    extra = ["--stall-seconds", "5"] if scenario == "stall" else []
    if scenario in ("won", "resign"):
        extra += ["--name", "dev", "--agents", "holy,tourist"]
    rc = hotseat.main(["--manual-lobby", "--tag", TAG, "--orders-bin", str(binary),
                       "--no-notify", *extra])
    run_dir = hotseat.RUN_ROOT / TAG
    rows = []
    try:
        import sqlite3
        with sqlite3.connect(str(run_dir / "orders.sqlite")) as db:
            rows = db.execute("SELECT run FROM harness").fetchall()
    except Exception as error:
        rows = [str(error)]
    print("RESULT " + json.dumps({
        "rc": rc, "started": started, "lanes": lanes, "recorded": recorded, "spawned": spawned,
        "real": screen_free.loaded_for_real(),
        "pil": [m for m in sys.modules if m == "PIL" or m.startswith("PIL.")],
        "installed": (tmp / "Assets" / "DLC" / "CivvisControl").exists(),
        "log": (run_dir / "hotseat.log").read_text(),
        "summary": json.loads((run_dir / "summary.json").read_text()),
        "harness_rows": rows, "counts": counts,
    }))
""")


class ManualLobbyEndToEndTest(unittest.TestCase):
    """A whole `--manual-lobby` run, screen-free, against a scripted game."""

    def run_child(self, scenario: str) -> dict:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        base = Path(tmp.name)
        env = {k: v for k, v in os.environ.items() if k != screen_free.ENV}
        env.update({"CIVVIS_HOTSEAT_RUNS": str(base / "runs"),
                    "CIVVIS_GAME_LOCK_DIR": str(base / "lock"),
                    "CIVVIS_OPERATOR_HALT_FILE": str(base / "halt.json")})
        done = subprocess.run([sys.executable, "-c", CHILD, str(HERE), str(base), scenario],
                              env=env, capture_output=True, text=True, timeout=180)
        line = next((ln for ln in done.stdout.splitlines() if ln.startswith("RESULT ")), None)
        self.assertIsNotNone(line, f"no result:\n{done.stdout[-3000:]}\n{done.stderr[-3000:]}")
        result = json.loads(line[len("RESULT "):])
        result["stdout"] = done.stdout
        self.assertEqual(result["real"], [], "no screen module was imported as itself")
        self.assertEqual(result["pil"], [], "Pillow was never loaded")
        for argv in result["spawned"]:
            self.assertNotIn(Path(argv[0]).name, {"swiftc", "cliclick", "screencapture"})
        self.assertEqual(result["harness_rows"], [], "the beat is gone when the harness stops")
        return result

    def test_a_game_the_person_set_up_is_read_back_and_played(self) -> None:
        result = self.run_child("close")
        self.assertEqual(result["rc"], 0)
        self.assertIn("Main menu -> Multiplayer -> Hot Seat", result["stdout"])
        self.assertIn(hotseat.START_CIV, result["stdout"])
        log = result["log"]
        self.assertIn("Civ VI set up a game: HOTSEAT, 3 human and 1 AI players on Pangaea.lua, "
                      "CIVVIS mod loaded", log)
        self.assertIn("the game: Hot Seat, 4 players on Pangaea.lua MAPSIZE_TINY", log)
        self.assertEqual(result["started"], [[1, 2]], "workers for the Human rows after yours")
        self.assertIn("CIVVIS plays row 2 and row 3", log)
        self.assertIn("3 opening orders applied (civvis)", log)
        self.assertIn("Civ VI has closed", log)
        self.assertIn("removed the CIVVIS mod", log)
        self.assertFalse(result["installed"], "removed once Civ VI is not running")
        self.assertEqual(result["summary"]["civvis_seats"], [1, 2])
        self.assertEqual(result["summary"]["reason"], "Civ VI closed")
        # Six logs relayed (the table's and seats 1-5's) 25 times a pass and
        # once at its start, against one beat a pass: a board reaches its
        # decider within 20 ms, not at the next pass (live 2026-11-28: 523 ms).
        counts = result["counts"]
        self.assertGreater(counts["beats"], 5)
        self.assertGreaterEqual(counts["pumps"], 6 * 25 * (counts["beats"] - 2))


    def test_a_named_persons_game_reaches_the_peoples_record_at_its_ending(self) -> None:
        import people_elo  # noqa: PLC0415
        result = self.run_child("won")
        log = result["log"]
        self.assertIn("row 1 dev (you) (human, DIFFICULTY_KING, LEADER_0)", log)
        self.assertIn("row 2 CIVVIS HolyRoller", log)
        self.assertIn("row 3 CIVVIS TouristTrap", log)
        # Each seat plays its own agent's lane, as the install names it.
        self.assertEqual(result["lanes"], [{"1": "religious", "2": "culture"}])
        # Posted at the decision, before Civ VI closed, and again at the end
        # (the record keeps one line however often it is posted).
        self.assertEqual([r["resigned"] for r in result["recorded"]], [False, False])
        decided = result["recorded"][0]["summary"]
        self.assertEqual(decided["reason"], "game over")
        for summary in (decided, result["summary"]):
            self.assertEqual((summary["result"]["kind"], summary["result"]["person"]), ("victory", True))
            self.assertEqual((summary["person"], summary["person_seat"], summary["leader"]),
                             ("dev", 0, "LEADER_0"))
            self.assertEqual((summary["players"], summary["map"], summary["map_size"], summary["speed"],
                              summary["max_turns"], summary["difficulty"]),
                             (4, "Pangaea.lua", "MAPSIZE_TINY", "GAMESPEED_ONLINE", 250, "DIFFICULTY_KING"))
            self.assertEqual(summary["seat_agents"], {"1": "holy", "2": "tourist"})
            self.assertEqual(summary["started"], "2026-11-27T17:20:00Z", "the HUD's clock at the read-back")
        row = people_elo.game_row(result["summary"])
        self.assertEqual((row["winner"], row["agents"], row["victory"], row["person"]),
                         ("person", ["holy", "tourist"], "VICTORY_TECHNOLOGY", "dev"))

    def test_stop_resign_gives_a_manual_lobby_game_to_civvis(self) -> None:
        import people_elo  # noqa: PLC0415
        result = self.run_child("resign")
        self.assertEqual([r["resigned"] for r in result["recorded"]], [True])
        summary = result["recorded"][0]["summary"]
        self.assertIsNone(summary["result"])
        self.assertEqual(summary["person"], "dev")
        row = people_elo.game_row(summary, winner="civvis", victory="resignation")
        self.assertEqual((row["winner"], row["agents"]), ("civvis", ["holy", "tourist"]))

    def test_a_game_with_no_name_is_not_recorded(self) -> None:
        result = self.run_child("close")
        self.assertEqual(result["recorded"], [])
        self.assertIsNone(result["summary"]["person"])
        self.assertEqual(result["lanes"], ["civvis"], "without --agents every seat plays --victory")

    def test_name_and_agents_are_read_as_a_launched_table_reads_them(self) -> None:
        with patch.object(hotseat, "run_manual", side_effect=lambda a: a), \
                patch.object(hotseat.screen_free, "engaged", return_value=True), \
                patch.object(hotseat, "read_current", return_value=None):
            args = hotseat.main(["--manual-lobby", "--agents", "Holy, jack", "--name", "dev"])
            self.assertEqual(args.agents, ["holy", "jack"])
            for bad in (["--agents", "holy,nobody"], ["--name", "'); drop"]):
                with patch("sys.stderr"):
                    self.assertEqual(hotseat.main(["--manual-lobby", *bad]), 2, bad)
        # Row 3 is the third agent whether or not row 2 is seated.
        self.assertEqual(hotseat.manual_seat_agents(argparse.Namespace(agents=["holy", "tourist", "siege"]),
                                                    [1, 3]), {1: "holy", 3: "siege"})

    def test_stopping_with_civ_up_leaves_the_mod_and_the_game(self) -> None:
        result = self.run_child("stop")
        self.assertIn("stopping: signal", result["log"])
        self.assertIn("the CIVVIS mod stays installed; it does nothing without this harness",
                      result["log"])
        self.assertTrue(result["installed"], "never removed under a running Civ VI")

    def test_a_game_started_without_the_mod_is_named(self) -> None:
        result = self.run_child("nomod")
        self.assertIn("without the CIVVIS mod", result["log"])
        self.assertEqual(result["started"], [])

    def test_a_lobby_with_no_civvis_seat_says_how_to_fix_it(self) -> None:
        result = self.run_child("alone")
        self.assertIn("Only your row is Human, so CIVVIS has no seat to play", result["log"])
        self.assertEqual(result["started"], [], "no worker for a lobby with no CIVVIS seat")

    def test_a_stalled_civvis_turn_is_reported_not_clicked(self) -> None:
        result = self.run_child("stall")
        self.assertIn("has made no move for", result["log"])
        self.assertIn("Nothing here clicks in --manual-lobby mode", result["log"])
        # 20 s of a running game on the harness's clock with no `ps` every pass
        # (`watch.GameLiveness`, as `run` asks since #6005): the relay reads
        # nothing while one runs. Before, one listing a pass.
        counts = result["counts"]
        self.assertGreater(counts["beats"], 35, counts)
        self.assertLessEqual(counts["ps"], 8, counts)


if __name__ == "__main__":
    unittest.main()
