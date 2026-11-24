"""Firaxis self-play: one real game with every seat played by the game's own
AI, timed by the lane (`civ6_play.py --firaxis-self-play`).

Three parts, and none of them starts a game:

* the flag: what it turns the parsed options into, the one configuration key
  the control mod reads, the run's own tag and folder, and what the run leaves
  out (the decider, the ladder row, the ledger publish);
* the reading: how the rows the mod writes become the run's times and result;
* the one-shot marker the climb consumes before it launches the game
  (`~/.civvis-firaxis-self-play-once`). The loop around it is held in
  `test_civ6_civvis_climb.py`, on that suite's own rig;
* the desktop popup clearer standing down while such a run holds the seat.

The mod's side is `civ6_control/mod/firaxis_self_play_test.lua`.
"""

from __future__ import annotations

import contextlib
import inspect
import io
import signal
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_civvis_climb as climb  # noqa: E402
import civ6_play  # noqa: E402
from civ6_control import game_speed, gamelock, popup_clear  # noqa: E402

MOD = Path(__file__).resolve().parent / "civ6_control" / "mod"


def parsed(*words: str):
    return civ6_play.build_parser().parse_args(list(words))


def self_play(*words: str):
    """The options of a self-play run, as `main` leaves them."""
    args = parsed("--firaxis-self-play", *words)
    refusal = civ6_play.apply_firaxis_self_play(args)
    assert refusal is None, refusal
    return args


class TheFlagTests(unittest.TestCase):
    def test_it_is_off_unless_asked_for(self) -> None:
        args = parsed("--tag", "civvis-20261124T000000Z")
        self.assertFalse(args.firaxis_self_play)
        self.assertFalse(civ6_play.firaxis_self_play(args))
        self.assertEqual(civ6_play.run_root(args), civ6_play.RUN_ROOT)

    def test_a_lane_game_s_configuration_has_none_of_its_keys(self) -> None:
        # Absent, not false: a lane game's installed configuration is the one
        # it had before this arm existed.
        config = civ6_play.build_config(parsed("--tag", "civvis-20261124T000000Z"))
        self.assertEqual([key for key in config if "SelfPlay" in key], [])
        self.assertIs(config["Play"], True)

    def test_the_mod_is_given_one_key_and_plays_nothing(self) -> None:
        config = civ6_play.build_config(self_play("--difficulty", "DIFFICULTY_DEITY"))
        self.assertIs(config["FiraxisSelfPlay"], True)
        # `tick` returns at once under `Play == false`: no order is issued.
        self.assertIs(config["Play"], False)
        self.assertIs(config["CivvisDecides"], False)
        self.assertIs(config["ExportState"], False)
        self.assertEqual(config["FiraxisSelfPlayObserve"], "seat")

    def test_the_setup_is_the_lane_s(self) -> None:
        # One human slot, ours, on the Create Game screen, as every lane game
        # has; the mod hands it over once the game is running.
        lane = civ6_play.build_config(parsed(
            "--tag", "civvis-x", "--difficulty", "DIFFICULTY_DEITY", "--max-turns", "250"))
        ours = civ6_play.build_config(self_play("--difficulty", "DIFFICULTY_DEITY"))
        for key in ("RuleSet", "MapScript", "MapSize", "Difficulty", "GameSpeed",
                    "MaxTurns", "HumanPlayers", "GameModes", "Rehost"):
            self.assertEqual(ours[key], lane[key], key)
        self.assertEqual(ours["HumanPlayers"], 1)
        self.assertEqual(ours["Difficulty"], "DIFFICULTY_DEITY")

    def test_the_game_runs_to_the_speed_s_own_turn_limit(self) -> None:
        # Where a game nobody has won ends on Score, as the lane's do; this
        # harness's own `--max-turns` default (150) is not that.
        self.assertEqual(self_play().max_turns, game_speed.turn_limit("GAMESPEED_ONLINE"))
        self.assertEqual(self_play("--speed", "GAMESPEED_STANDARD").max_turns, 500)
        self.assertEqual(self_play("--max-turns", "40").max_turns, 250)

    def test_the_tag_is_its_own(self) -> None:
        tag = self_play().tag
        self.assertRegex(tag, r"^firaxis-selfplay-\d{8}T\d{6}Z$")
        self.assertEqual(self_play("--tag", "firaxis-selfplay-x").tag, "firaxis-selfplay-x")
        # The wedge watchdog and the supervisor's "played turns" count read
        # `civvis-*` and nothing else.
        self.assertFalse(civ6_play.FIRAXIS_SELF_PLAY_PREFIX.startswith("civvis-"))

    def test_a_lane_tag_is_refused(self) -> None:
        args = parsed("--firaxis-self-play", "--tag", "civvis-t7-20261124T000000Z")
        self.assertIn("firaxis-selfplay-", civ6_play.apply_firaxis_self_play(args))

    def test_it_starts_its_own_fresh_game(self) -> None:
        for words in (["--load-save", "/tmp/a.Civ6Save"], ["--attach-running"]):
            args = parsed("--firaxis-self-play", *words)
            self.assertIn("fresh game", civ6_play.apply_firaxis_self_play(args))

    def test_an_unknown_speed_is_refused(self) -> None:
        args = parsed("--firaxis-self-play", "--speed", "GAMESPEED_GLACIAL")
        self.assertIn("GAMESPEED_GLACIAL", civ6_play.apply_firaxis_self_play(args))

    def test_whatever_the_command_line_asked_of_our_seat_is_dropped(self) -> None:
        args = self_play("--civvis-decides", "--export-state", "--stall-rescues", "3")
        self.assertFalse(args.civvis_decides)
        self.assertFalse(civ6_play.state_export_enabled(args))
        # A stall's rescue is a click into the game's window.
        self.assertEqual(args.stall_rescues, 0)

    def test_observing_nobody_reaches_the_mod(self) -> None:
        config = civ6_play.build_config(self_play("--firaxis-self-play-observe", "none"))
        self.assertEqual(config["FiraxisSelfPlayObserve"], "none")
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            parsed("--firaxis-self-play-observe", "everyone")

    def test_its_runs_live_beside_the_lane_s_not_among_them(self) -> None:
        root = civ6_play.run_root(self_play())
        self.assertEqual(root, civ6_play.FIRAXIS_SELF_PLAY_ROOT)
        self.assertEqual(root.parent, civ6_play.RUN_ROOT.parent)
        self.assertNotEqual(root, civ6_play.RUN_ROOT)
        self.assertNotIn(civ6_play.RUN_ROOT, root.parents)

    def test_the_lane_s_popup_and_window_values_are_the_climb_s(self) -> None:
        # What `play_command` passes for every lane game, read back through
        # this harness's own parser, against what the arm pins.
        lane_words = climb.play_command(
            SimpleNamespace(
                difficulty="DIFFICULTY_DEITY", map="Continents.lua",
                map_size="MAPSIZE_SMALL", speed="GAMESPEED_ONLINE", leader=None,
                max_turns=250, timeout=7200.0, timeout_ceiling=None,
                probe_citizens=False, campus_specialist=False, envoys=False,
                envoy_place=False, envoy_levy=False, envoy_consider=False,
                victory="science", strategy="auto", war_from_plan=False,
                tile_export_every=25, refresh_seconds=None,
                no_peace_deterrence=False, with_=[], without=[],
                no_counter_resolutions=False, combat_frames=0, replan_frames=2,
                settler_escort_cap_sync=True, restart_below_leader_ratio=None),
            "civvis-x", Path("orders.sqlite"), Path("civvis_orders"))
        lane = civ6_play.build_parser().parse_args(lane_words[3:])
        ours = self_play()
        self.assertTrue(civ6_play.FIRAXIS_SELF_PLAY_LANE_FLAGS)
        for name in civ6_play.FIRAXIS_SELF_PLAY_LANE_FLAGS:
            self.assertEqual(getattr(ours, name), getattr(lane, name), name)
        # And the game's clock is the lane's.
        self.assertEqual(ours.max_turns, lane.max_turns)


class MainTests(unittest.TestCase):
    def _main(self, *words: str):
        with mock.patch.object(civ6_play, "play", return_value=0) as play, \
                mock.patch.object(civ6_play, "apply_tree_mod_arms", return_value=[]), \
                contextlib.redirect_stderr(io.StringIO()) as said:
            code = civ6_play.main(list(words))
        return code, play, said.getvalue()

    def test_the_flag_is_applied_before_the_run_starts(self) -> None:
        code, play, _ = self._main("--firaxis-self-play", "--difficulty", "DIFFICULTY_DEITY")
        self.assertEqual(code, 0)
        args = play.call_args.args[0]
        self.assertTrue(args.tag.startswith("firaxis-selfplay-"))
        self.assertFalse(args.civvis_decides)
        self.assertEqual(args.max_turns, 250)
        # The window is placed from these, which `main` reads after the flag.
        self.assertEqual((civ6_play.GAME_SIDE, civ6_play.GAME_FRACTION,
                          civ6_play.GAME_VFRACTION), ("left", 0.5, 0.5))

    def test_a_refused_run_starts_nothing(self) -> None:
        code, play, said = self._main("--firaxis-self-play", "--tag", "civvis-mine")
        self.assertEqual(code, 2)
        play.assert_not_called()
        self.assertIn("firaxis-selfplay-", said)

    def test_a_lane_run_is_not_touched(self) -> None:
        code, play, _ = self._main("--tag", "civvis-mine", "--max-turns", "250",
                                   "--civvis-decides")
        self.assertEqual(code, 0)
        args = play.call_args.args[0]
        self.assertEqual(args.tag, "civvis-mine")
        self.assertTrue(args.civvis_decides)
        self.assertFalse(args.firaxis_self_play)


def row(kind: str, utc: str, **fields) -> dict:
    return {"ctx": "agent", "kind": kind, "utc": utc, **fields}


MAJORS = [{"player": n, "team": n, "civ": f"CIVILIZATION_{n}", "leader": f"LEADER_{n}",
           "score": 100 + n} for n in range(6)]


def read(rows: list[dict]) -> dict:
    reading: dict = {}
    for event in rows:
        civ6_play.firaxis_self_play_unseat(event)
        civ6_play.firaxis_self_play_note(reading, event)
    return reading


def summary(rows: list[dict], **more) -> dict:
    return civ6_play.firaxis_self_play_summary(
        read(rows), load_start=more.get("load_start", [9.0, 9.5, 10.0]),
        load_end=more.get("load_end", [11.0, 10.0, 10.0]),
        run_seconds=more.get("run_seconds", 560.4))


#: A game of 187 turns: handed over at 12:00:10, won at 12:07:55 by team 3.
WON = [
    row("seat", "2026-11-24T12:00:05.000Z", local_player=0),
    row("autoplay", "2026-11-24T12:00:12.250Z", t=1000, started=True, turn=1,
        turns=260, seat=0, observe=0, manager="table", scale=3, majors=MAJORS),
    row("turn", "2026-11-24T12:00:12.250Z", t=1000, turn=1, self_play=True, majors=MAJORS),
    row("autoplay_start", "2026-11-24T12:00:12.300Z", t=1000, turn=1),
    row("turn", "2026-11-24T12:00:13.900Z", t=1003, turn=2, self_play=True, majors=MAJORS),
    row("turn", "2026-11-24T12:07:54.000Z", t=1464, turn=187, self_play=True, majors=MAJORS),
    row("victory", "2026-11-24T12:07:55.500Z", t=1465, turn=187, team=3, victory=5,
        victory_type="VICTORY_TECHNOLOGY", won=False, local_player=-1),
]


class TheReadingTests(unittest.TestCase):
    def test_a_won_game(self) -> None:
        block = summary(WON)
        self.assertIs(block["autoplay_started"], True)
        self.assertIs(block["autoplay_engine_confirmed"], True)
        self.assertEqual(block["outcome"], "victory")
        self.assertEqual(block["victory_type"], "VICTORY_TECHNOLOGY")
        self.assertEqual(block["winner_team"], 3)
        self.assertEqual(block["winner"], [
            {"player": 3, "civ": "CIVILIZATION_3", "leader": "LEADER_3", "score": 103}])
        self.assertEqual(block["first_turn"], 1)
        self.assertEqual(block["final_turn"], 187)
        # By the mod's own clock: the second each row was written.
        self.assertEqual(block["play_seconds"], 465.0)
        # And by when each reached the harness, which started 2.25 s late here.
        self.assertEqual(block["play_seconds_by_receipt"], 463.2)
        self.assertEqual(block["seconds_per_turn"], round(465.0 / 187, 2))
        self.assertEqual(block["run_seconds"], 560.4)
        self.assertEqual(block["load_start"], [9.0, 9.5, 10.0])
        self.assertEqual(block["load_end"], [11.0, 10.0, 10.0])
        self.assertEqual(block["first_turn_utc"], "2026-11-24T12:00:12.250Z")
        self.assertEqual(block["outcome_utc"], "2026-11-24T12:07:55.500Z")
        self.assertEqual(block["autoplay"]["turns"], 260)
        self.assertEqual(block["autoplay"]["observe"], 0)

    def test_a_victory_ends_the_run(self) -> None:
        self.assertEqual([civ6_play.firaxis_self_play_over(event) for event in WON],
                         [False, False, False, False, False, False, True])

    def test_rows_without_the_mod_s_clock_are_timed_by_receipt(self) -> None:
        rows = [{key: value for key, value in event.items() if key != "t"} for event in WON]
        block = summary(rows)
        self.assertEqual(block["play_seconds"], 463.2)
        self.assertEqual(block["play_seconds_by_receipt"], 463.2)

    def test_turns_after_the_victory_do_not_move_the_result(self) -> None:
        later = row("turn", "2026-11-24T12:08:00.000Z", t=1470, turn=188,
                    self_play=True, majors=MAJORS)
        block = summary(WON + [later])
        self.assertEqual(block["final_turn"], 187)
        self.assertEqual(block["play_seconds"], 465.0)

    def test_the_manager_handing_the_seat_back_with_no_winner_ends_the_game(self) -> None:
        ended = row("autoplay_end", "2026-11-24T12:09:00.000Z", t=1530, turn=250,
                    after_victory=False, majors=MAJORS)
        rows = WON[:6] + [ended]
        self.assertTrue(civ6_play.firaxis_self_play_over(ended))
        block = summary(rows)
        self.assertEqual(block["outcome"], "autoplay_end")
        self.assertEqual(block["final_turn"], 250)
        self.assertEqual(block["play_seconds"], 530.0)
        self.assertIsNone(block["winner"])
        self.assertIsNone(block["winner_team"])
        self.assertEqual(block["autoplay_end"]["turn"], 250)

    def test_an_autoplay_end_after_a_victory_changes_nothing(self) -> None:
        ended = row("autoplay_end", "2026-11-24T12:07:56.000Z", t=1466, turn=187,
                    after_victory=True)
        self.assertFalse(civ6_play.firaxis_self_play_over(ended))
        block = summary(WON + [ended])
        self.assertEqual(block["outcome"], "victory")
        self.assertEqual(block["play_seconds"], 465.0)
        self.assertIs(block["autoplay_end"]["after_victory"], True)

    def test_a_hand_over_that_did_not_happen_ends_the_run_at_once(self) -> None:
        failed = row("autoplay", "2026-11-24T12:00:12.250Z", t=1000, started=False,
                     turn=1, turns=260, seat=0, observe=0, manager="nil",
                     error="AutoplayManager is not available in this context")
        self.assertTrue(civ6_play.firaxis_self_play_over(failed))
        block = summary([failed])
        self.assertIs(block["autoplay_started"], False)
        self.assertIn("not available", block["autoplay"]["error"])
        self.assertIsNone(block["outcome"])
        self.assertIsNone(block["play_seconds"])
        self.assertIsNone(block["final_turn"])

    def test_a_started_hand_over_does_not_end_it(self) -> None:
        self.assertFalse(civ6_play.firaxis_self_play_over(WON[1]))

    def test_a_stopped_run_says_how_far_it_got(self) -> None:
        block = summary(WON[:6])
        self.assertIsNone(block["outcome"])
        self.assertIsNone(block["play_seconds"])
        self.assertEqual(block["final_turn"], 187)
        self.assertEqual(block["first_turn_utc"], "2026-11-24T12:00:12.250Z")

    def test_only_the_mod_s_own_rows_are_read(self) -> None:
        harness = {"kind": "victory", "utc": "2026-11-24T12:00:00.000Z", "team": 1}
        lane_turn = row("turn", "2026-11-24T12:00:00.000Z", turn=9)
        self.assertFalse(civ6_play.firaxis_self_play_over(harness))
        self.assertEqual(read([harness, lane_turn]), {})

    def test_nobody_s_win_and_nobody_s_defeat(self) -> None:
        # Should the engine name the seat it observes as the local player.
        won = row("victory", "2026-11-24T12:07:55.500Z", turn=187, team=0, won=True)
        lost = row("defeat", "2026-11-24T12:03:00.000Z", turn=90, player=0, ours=True)
        civ6_play.firaxis_self_play_unseat(won)
        civ6_play.firaxis_self_play_unseat(lost)
        self.assertIs(won["won"], False)
        self.assertIs(won["won_by_local_player"], True)
        self.assertIs(lost["ours"], False)
        self.assertIs(lost["local_player_defeated"], True)
        # So the harness holds no victory film and does not end on the seat's
        # elimination.
        self.assertFalse(civ6_play.is_terminal_result(lost))
        self.assertEqual(civ6_play.end_game_hold_seconds(won, 10.0),
                         civ6_play.LOSS_SCREEN_HOLD_S)
        other = row("turn", "2026-11-24T12:00:00.000Z", turn=3)
        civ6_play.firaxis_self_play_unseat(other)
        self.assertNotIn("won", other)


class TheRunTests(unittest.TestCase):
    """How `_play` is wired to the arm, read from its source: it only runs
    against a live game."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.source = inspect.getsource(civ6_play._play)

    def test_the_run_s_folder_is_the_arm_s_own(self) -> None:
        self.assertIn("run_dir = run_root(args) / args.tag", self.source)
        self.assertNotIn("RUN_ROOT / args.tag", self.source)

    def test_nothing_of_ours_is_published(self) -> None:
        leave = self.source.index("return 0 if reading[\"outcome\"] else 1")
        self.assertLess(self.source.index("(run_dir / \"summary.json\").write_text"), leave)
        self.assertLess(leave, self.source.index("civ6_ladder.record_summary(run_dir"))
        self.assertLess(leave, self.source.index("publish_run_in_background(run_dir)"))

    def test_the_summary_carries_the_reading_and_both_loads(self) -> None:
        self.assertIn('summary["firaxis_self_play"] = firaxis_self_play_summary(', self.source)
        self.assertIn('{"load_start": host_load(), "began": time.monotonic()}', self.source)
        self.assertIn("load_end=host_load()", self.source)
        # A run that was stopped says how far it got as well.
        self.assertIn('partial["firaxis_self_play"] = firaxis_self_play_summary(', self.source)

    def test_the_run_ends_with_the_game_and_on_nothing_else(self) -> None:
        finished = self.source[self.source.index("    def finished(event: dict) -> bool:"):]
        finished = finished[:finished.index("    def ", 10)]
        gate = finished.index("if self_play is not None:")
        self.assertLess(gate, finished.index("if is_terminal_result(event):"))
        self.assertIn("firaxis_self_play_over(event)", finished[gate:])

    def test_a_parked_game_is_never_restarted_in_place_as_one(self) -> None:
        claim = self.source.index("restart_handoff.claim(config, env.game_pids())")
        self.assertIn("if handoff is not None and self_play is not None:",
                      self.source[claim:claim + 900])

    def test_the_decider_starts_only_when_asked_for(self) -> None:
        # And the flag turns the asking off (`TheFlagTests`).
        self.assertIn("    if args.civvis_decides:\n        binary = ", self.source)


class TheMarkerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.marker = Path(self.tmp.name) / ".civvis-firaxis-self-play-once"
        # The seat is ready unless a test says otherwise; never this machine's
        # own answer.
        ready = mock.patch.object(climb, "firaxis_self_play_blocked", return_value=None)
        self.blocked = ready.start()
        self.addCleanup(ready.stop)

    def _take(self):
        said = io.StringIO()
        with contextlib.redirect_stdout(said):
            request = climb.take_firaxis_self_play_once(self.marker)
        return request, said.getvalue()

    def test_its_place_is_one_file_in_the_home_folder(self) -> None:
        self.assertEqual(climb.FIRAXIS_SELF_PLAY_ONCE.name, ".civvis-firaxis-self-play-once")

    def test_no_marker_no_game(self) -> None:
        self.assertEqual(self._take(), (None, ""))

    def test_it_is_consumed_by_being_read(self) -> None:
        self.marker.touch()
        request, _ = self._take()
        self.assertEqual(request, {"observe": "seat"})
        self.assertFalse(self.marker.exists())
        self.assertEqual(self._take(), (None, ""))

    def test_it_may_ask_for_nobody_s_view(self) -> None:
        self.marker.write_text("observe-none\n")
        self.assertEqual(self._take()[0], {"observe": "none"})
        self.marker.write_text("armed by the coordinator 2026-11-24\n")
        self.assertEqual(self._take()[0], {"observe": "seat"})

    def test_a_marker_that_cannot_be_removed_starts_nothing(self) -> None:
        # A folder with something in it: `unlink` refuses, as it would a file
        # this user may not remove.
        self.marker.mkdir()
        (self.marker / "kept").touch()
        request, said = self._take()
        self.assertIsNone(request)
        self.assertTrue(self.marker.exists())
        self.assertIn("cannot be removed", said)
        self.assertIn("every cycle", said)

    def test_a_dangling_link_is_a_marker_too_and_is_removed(self) -> None:
        self.marker.symlink_to(Path(self.tmp.name) / "nowhere")
        self.assertEqual(self._take()[0], {"observe": "seat"})
        self.assertFalse(self.marker.is_symlink())

    def test_a_seat_that_is_not_ready_keeps_the_marker_for_the_next_cycle(self) -> None:
        self.marker.touch()
        self.blocked.return_value = "the popup clearer does not stand down yet"
        request, said = self._take()
        self.assertIsNone(request)
        self.assertTrue(self.marker.exists())
        self.assertIn("not yet: the popup clearer does not stand down yet", said)
        self.assertIn("stays for the next cycle", said)
        # Ready a cycle later: taken then.
        self.blocked.return_value = None
        self.assertEqual(self._take()[0], {"observe": "seat"})
        self.assertFalse(self.marker.exists())

    def test_readiness_is_not_asked_when_nothing_is_asked_for(self) -> None:
        self._take()
        self.blocked.assert_not_called()


class TheSeatIsReadyTests(unittest.TestCase):
    """A self-play game waits for a popup clearer that stands down for one."""

    def _tree(self, root: Path, clearer: str | None) -> Path:
        keeper = root / "tools" / "ops" / "civvis-popup-keeper.sh"
        keeper.parent.mkdir(parents=True)
        keeper.write_text("#!/bin/zsh\n")
        if clearer is not None:
            path = root / "tools" / "civ6_control" / "popup_clear.py"
            path.parent.mkdir(parents=True)
            path.write_text(clearer)
        return keeper

    def _blocked(self, clearer: str | None):
        with tempfile.TemporaryDirectory() as tmp:
            keeper = self._tree(Path(tmp).resolve(), clearer)
            with mock.patch.object(climb, "interactive_popup_keeper_pid", return_value=4242), \
                    mock.patch.object(climb, "run", return_value=f"/bin/zsh {keeper}\n"):
                self.assertEqual(climb.popup_clearer_file(),
                                 keeper.parent.parent / "civ6_control" / "popup_clear.py")
                return climb.firaxis_self_play_blocked()

    def test_the_clearer_read_is_the_keeper_s_own(self) -> None:
        self.assertIsNone(self._blocked("def self_play_holds_game(holder=None):\n    ...\n"))

    def test_a_keeper_s_clearer_from_before_this_arm_blocks_it(self) -> None:
        said = self._blocked("def operator_holds_game():\n    ...\n")
        self.assertIn("does not stand down for a self-play game yet", said)
        self.assertIn("popup_clear.py", said)

    def test_a_clearer_that_cannot_be_read_blocks_it(self) -> None:
        self.assertIn("cannot be read", self._blocked(None))

    def test_a_seat_with_no_keeper_runs_this_tree_s_clearer(self) -> None:
        with mock.patch.object(climb, "interactive_popup_keeper_pid", return_value=None):
            self.assertEqual(climb.popup_clearer_file(),
                             climb.HERE / "civ6_control" / "popup_clear.py")
            self.assertIsNone(climb.firaxis_self_play_blocked())


class ThePopupClearerTests(unittest.TestCase):
    """The clearer clicks the real game, and a self-play run is not among the
    runs it reads: it stands down while one holds the seat."""

    def test_it_reads_the_harness_s_own_lock_and_tag(self) -> None:
        self.assertEqual(popup_clear.GAME_LOCK_HOLDER, gamelock.LOCK / "holder.json")
        self.assertEqual(popup_clear.SELF_PLAY_TAG_PREFIX, civ6_play.FIRAXIS_SELF_PLAY_PREFIX)

    def _holds(self, text: str | None) -> bool:
        with tempfile.TemporaryDirectory() as tmp:
            holder = Path(tmp) / "holder.json"
            if text is not None:
                holder.write_text(text)
            return popup_clear.self_play_holds_game(holder)

    def test_a_live_self_play_run_holds_the_game(self) -> None:
        import json
        import os
        self.assertTrue(self._holds(json.dumps(
            {"pid": os.getpid(), "tag": "firaxis-selfplay-20261124T170000Z"})))

    def test_a_lane_game_does_not(self) -> None:
        import json
        import os
        self.assertFalse(self._holds(json.dumps(
            {"pid": os.getpid(), "tag": "civvis-t7-20261124T170000Z"})))

    def test_a_self_play_run_that_died_does_not_keep_it_standing_down(self) -> None:
        import json
        gone = subprocess.Popen([sys.executable, "-c", "pass"])
        gone.wait()
        self.assertFalse(self._holds(json.dumps(
            {"pid": gone.pid, "tag": "firaxis-selfplay-20261124T170000Z"})))

    def test_no_lock_or_an_unreadable_one_is_no(self) -> None:
        self.assertFalse(self._holds(None))
        self.assertFalse(self._holds("{not json"))
        self.assertFalse(self._holds("[]"))
        self.assertFalse(self._holds('{"tag": 7}'))

    def test_a_held_seat_is_neither_captured_nor_clicked(self) -> None:
        with tempfile.TemporaryDirectory() as root, \
                mock.patch.object(popup_clear.macos_capture, "prepare", return_value=True), \
                mock.patch.object(popup_clear, "operator_holds_game", return_value=False), \
                mock.patch.object(popup_clear, "self_play_holds_game", return_value=True), \
                mock.patch.object(popup_clear, "newest_run_is_pre_turn") as pre_turn, \
                mock.patch.object(popup_clear, "capture_pause_reason") as pause_reason, \
                mock.patch.object(popup_clear, "window_box") as window_box, \
                mock.patch.object(popup_clear.macos_input, "press_key") as press_key, \
                mock.patch.object(sys, "argv", [
                    "popup_clear.py", "--once", "--runs", root,
                    "--log", str(Path(root) / "popup.log")]), \
                contextlib.redirect_stdout(io.StringIO()):
            popup_clear.main()
            ledger = (Path(root) / "popup.log").read_text()
        pre_turn.assert_not_called()
        pause_reason.assert_not_called()
        window_box.assert_not_called()
        press_key.assert_not_called()
        self.assertIn("a Firaxis self-play run holds the game", ledger)

    def test_the_check_comes_before_the_runs_are_read(self) -> None:
        # The runs say "a game is being played" through a self-play game's
        # setup: the lane's last game is seconds old then.
        source = inspect.getsource(popup_clear.main)
        self.assertLess(source.index("if self_play_holds_game():"),
                        source.index("if newest_run_is_pre_turn(args.runs):"))
        self.assertLess(source.index("if self_play_holds_game():"),
                        source.index("game_in_progress(args.runs"))


class TheLaunchTests(unittest.TestCase):
    ARGS = SimpleNamespace(difficulty="DIFFICULTY_PRINCE", map="Continents.lua",
                           map_size="MAPSIZE_SMALL", speed="GAMESPEED_ONLINE",
                           leader="LEADER_PERICLES", timeout=7200.0, timeout_ceiling=None)

    def test_the_command_is_the_lane_s_world_at_deity(self) -> None:
        words = climb.firaxis_self_play_command(self.ARGS, "firaxis-selfplay-x", "seat")
        self.assertEqual(words[:3], [sys.executable, "-u", str(climb.HERE / "civ6_play.py")])
        args = civ6_play.build_parser().parse_args(words[3:])
        self.assertIsNone(civ6_play.apply_firaxis_self_play(args))
        # Deity, whatever level the lane's next game is dealt.
        self.assertEqual(args.difficulty, "DIFFICULTY_DEITY")
        self.assertEqual((args.map, args.map_size, args.speed),
                         ("Continents.lua", "MAPSIZE_SMALL", "GAMESPEED_ONLINE"))
        self.assertEqual(args.leader, "LEADER_PERICLES")
        self.assertEqual(args.tag, "firaxis-selfplay-x")
        self.assertEqual(args.timeout, 7200.0)
        self.assertEqual(args.timeout_ceiling, climb.attempt_ceiling(self.ARGS))
        # Nothing of our seat's is asked for, and no order channel is named.
        for word in ("--civvis-decides", "--civvis-bin", "--orders-db", "--export-state",
                     "--load-save"):
            self.assertNotIn(word, words)

    def test_the_view_asked_for_is_passed_on(self) -> None:
        words = climb.firaxis_self_play_command(self.ARGS, "firaxis-selfplay-x", "none")
        self.assertEqual(words[words.index("--firaxis-self-play-observe") + 1], "none")

    def _play(self, proc, installed=None):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            logs = root / "logs"
            logs.mkdir()
            runs = root / "control"
            runs.mkdir()
            order: list[str] = []
            with mock.patch.object(climb.subprocess, "Popen", return_value=proc) as popen, \
                    mock.patch.object(climb, "ensure_popup_clear",
                                      side_effect=lambda: order.append("clearer")) as clearer, \
                    mock.patch.object(climb, "RUN_ROOT", runs), \
                    mock.patch.object(climb, "FIRAXIS_SELF_PLAY_ROOT", root / "firaxis-selfplay"), \
                    mock.patch.object(climb, "installed_run_tag",
                                      side_effect=lambda: installed(popen) if installed else None), \
                    mock.patch.object(climb, "teardown") as teardown, \
                    mock.patch.object(climb.env, "game_pids", return_value=[]), \
                    mock.patch.object(climb.time, "sleep"), \
                    contextlib.redirect_stdout(io.StringIO()) as said:
                popen.side_effect = lambda *a, **k: (order.append("game"), proc)[1]
                status = climb.play_firaxis_self_play(self.ARGS, logs, {"observe": "seat"})
            return SimpleNamespace(
                status=status, popen=popen, teardown=teardown, said=said.getvalue(),
                clearer=clearer, order=order,
                logs=sorted(path.name for path in logs.iterdir()),
                lane_runs=list(runs.iterdir()))

    def test_one_game_is_launched_and_waited_for(self) -> None:
        proc = mock.Mock()
        proc.wait.return_value = 0
        done = self._play(proc)
        self.assertEqual(done.status, 0)
        done.popen.assert_called_once()
        words = done.popen.call_args.args[0]
        self.assertIn("--firaxis-self-play", words)
        tag = words[words.index("--tag") + 1]
        self.assertRegex(tag, r"^firaxis-selfplay-\d{8}T\d{6}Z$")
        # Its log is not one the supervisor counts as a played lane game.
        self.assertEqual(done.logs, [f"{tag}-play.log"])
        # And nothing of it is left among the lane's runs.
        self.assertEqual(done.lane_runs, [])
        proc.wait.assert_called_once_with(timeout=climb.attempt_ceiling(self.ARGS) + 600)
        proc.send_signal.assert_not_called()
        # The harness cleaned up after itself: nothing for the climb to stop.
        done.teardown.assert_not_called()
        self.assertIn("summary.json", done.said)
        # The popup clearer is restarted from the file that was just read,
        # before the game is.
        self.assertEqual(done.order, ["clearer", "game"])

    def test_a_run_that_left_its_tag_installed_is_cleaned_up_by_that_tag(self) -> None:
        proc = mock.Mock()
        proc.wait.return_value = 143

        def installed(popen):
            words = popen.call_args.args[0]
            return words[words.index("--tag") + 1]

        done = self._play(proc, installed)
        tag = done.popen.call_args.args[0]
        tag = tag[tag.index("--tag") + 1]
        done.teardown.assert_called_once_with(tag)
        self.assertEqual(done.status, 143)

    def test_somebody_else_s_installed_tag_is_left_alone(self) -> None:
        proc = mock.Mock()
        proc.wait.return_value = 0
        done = self._play(proc, lambda popen: "civvis-t7-20261124T000000Z")
        done.teardown.assert_not_called()

    def test_a_game_past_its_ceiling_is_asked_to_stop(self) -> None:
        proc = mock.Mock()
        proc.wait.side_effect = [subprocess.TimeoutExpired("civ6_play", 1.0), 143]
        done = self._play(proc)
        proc.send_signal.assert_called_once_with(signal.SIGTERM)
        self.assertEqual(done.status, 143)
        self.assertIn("past its ceiling", done.said)

    def test_a_game_that_cannot_be_started_is_said_and_survived(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            logs = Path(tmp)
            with mock.patch.object(climb.subprocess, "Popen", side_effect=OSError("no python")), \
                    mock.patch.object(climb, "ensure_popup_clear"), \
                    mock.patch.object(climb, "installed_run_tag", return_value=None), \
                    mock.patch.object(climb.env, "game_pids", return_value=[]), \
                    mock.patch.object(climb.time, "sleep"), \
                    contextlib.redirect_stdout(io.StringIO()) as said:
                status = climb.play_firaxis_self_play(self.ARGS, logs, {"observe": "seat"})
        self.assertIsNone(status)
        self.assertIn("could not start the game", said.getvalue())


class TheLoopWiringTests(unittest.TestCase):
    """Where the climb looks for the marker, read from its source; the loop is
    run on the climb suite's rig (`FiraxisSelfPlayOnceTests`)."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.source = inspect.getsource(climb.main)

    def test_the_marker_is_read_between_the_gate_and_the_attempt(self) -> None:
        gate = self.source.index("reason = blocked_reason()")
        take = self.source.index("take_firaxis_self_play_once()")
        self.assertLess(gate, take)
        self.assertLess(take, self.source.index("heal_the_ladder()"))
        self.assertLess(take, self.source.index('tag = f"{run_tag_prefix()}{stamp()}"'))

    def test_it_is_taken_before_the_game_is_launched_and_the_loop_goes_on(self) -> None:
        take = self.source.index("take_firaxis_self_play_once()")
        after = self.source[take:take + 400]
        self.assertIn("if self_play_request is not None:\n"
                      "            play_firaxis_self_play(args, logs, self_play_request)\n"
                      "            continue\n", after)

    def test_nothing_else_asks_for_one(self) -> None:
        whole = Path(climb.__file__).read_text(encoding="utf-8")
        self.assertEqual(whole.count("take_firaxis_self_play_once()"), 1)
        self.assertEqual(
            whole.count("play_firaxis_self_play(args, logs, self_play_request)"), 1)
        # No flag of the climb's, and a saved-game diagnostic never takes it.
        self.assertNotIn("--firaxis-self-play", " ".join(
            action.option_strings[0] for action in self._climb_actions()
            if action.option_strings))
        self.assertIn("None if args.load_save else take_firaxis_self_play_once()",
                      self.source)

    @staticmethod
    def _climb_actions():
        captured = {}
        real = climb.argparse.ArgumentParser.parse_args

        def capture(parser, *args, **kwargs):
            captured["actions"] = list(parser._actions)
            raise SystemExit(0)

        with mock.patch.object(climb.argparse.ArgumentParser, "parse_args", capture), \
                mock.patch.object(climb.sys, "argv", ["climb"]):
            try:
                climb.main()
            except SystemExit:
                pass
        assert real is not None
        return captured["actions"]


class TheModTests(unittest.TestCase):
    """The mod's branch is run by `firaxis_self_play_test.lua` (lua5.1, in CI's
    control-mod job). Here: that the key the harness writes is the key the mod
    reads, and that the test is there to be discovered."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.agent = (MOD / "CivvisControlAgent.lua").read_text(encoding="utf-8")

    def test_the_mod_reads_the_keys_the_harness_writes(self) -> None:
        config = civ6_play.build_config(self_play())
        for key in ("FiraxisSelfPlay", "FiraxisSelfPlayObserve"):
            self.assertIn(key, config)
            self.assertIn(f"cfg.{key}", self.agent)
        self.assertIn('cfg.FiraxisSelfPlayObserve == "none"', self.agent)

    def test_the_hand_over_is_the_shipped_test_s_four_calls(self) -> None:
        begin = self.agent[self.agent.index("function CivvisSelfPlay.begin()"):]
        begin = begin[:begin.index("\nend\n")]
        calls = ["manager.SetTurns(turns);", "manager.SetReturnAsPlayer(pid);",
                 "manager.SetObserveAsPlayer(observe);", "manager.SetActive(true);"]
        places = [begin.index(call) for call in calls]
        self.assertEqual(places, sorted(places))

    def test_the_victory_and_defeat_handlers_are_the_stock_ones(self) -> None:
        # The mod's ending rows are what every game writes; the harness reads
        # a self-play game's as nobody's (`firaxis_self_play_unseat`).
        self.assertIn("\t\twon = (team == ourTeam),\n", self.agent)
        self.assertIn("\t\tours = (player == pid),\n", self.agent)

    def test_the_mod_s_own_test_ships(self) -> None:
        self.assertTrue((MOD / "firaxis_self_play_test.lua").is_file())


if __name__ == "__main__":
    unittest.main()
