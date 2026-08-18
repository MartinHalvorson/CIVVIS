#!/usr/bin/env python3
"""Focused setup-contract checks for the live Civ VI launcher."""

from __future__ import annotations

import io
import builtins
import os
import re
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock
from unittest.mock import call, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_play  # noqa: E402

try:
    import PIL  # noqa: F401
    HAS_PILLOW = True
except ImportError:  # pragma: no cover - depends on the host, not the code
    HAS_PILLOW = False

# Three of the checks below read text off a screenshot, so they need an image
# library and say so. The other thirty-six are setup contracts that touch no
# pixels — and until `popup_clear` stopped demanding Pillow at import, all
# thirty-nine of them died at collection on any host without it.
needs_pillow = unittest.skipUnless(HAS_PILLOW, "Pillow is not installed on this host")


def args(**changes):
    values = {
        "difficulty": "DIFFICULTY_SETTLER",
        "map_size": "MAPSIZE_SMALL",
        "speed": "GAMESPEED_ONLINE",
        "map": "Continents.lua",
        "leader": "LEADER_TRAJAN",
        "game_mode": [],
        "ruleset": "RULESET_EXPANSION_2",
    }
    values.update(changes)
    return SimpleNamespace(**values)


class Civ6PlayTest(unittest.TestCase):
    def test_supervised_defaults_are_stock_and_aim_at_a_lane_that_lands(self) -> None:
        """The value itself is argued and pinned in `test_ops_ladder_objective.py`,
        which also holds the evidence that moved it off `science`. This asserts
        only that the supervised worker takes the chain's one default and stock
        weights, so a second copy cannot appear here."""
        self.assertEqual(civ6_play.DEFAULT_CIVVIS_VICTORY, "diplomatic")
        self.assertEqual(civ6_play.DEFAULT_CIVVIS_STRATEGY, "")

    def test_startup_ignores_auto_close_events_until_the_agent_is_loaded(self) -> None:
        self.assertFalse(civ6_play.startup_event_proves_game_started({
            "ctx": "autoclose", "kind": "autoclose_armed"
        }))
        self.assertTrue(civ6_play.startup_event_proves_game_started({
            "ctx": "agent", "kind": "loaded"
        }))
        self.assertTrue(civ6_play.startup_event_proves_game_started({
            "ctx": "agent", "kind": "seat"
        }))

    def test_only_a_live_board_can_end_a_missing_intro_probe(self) -> None:
        """The intro modal may coexist with the agent's early lifecycle events."""
        self.assertFalse(civ6_play.board_event_proves_intro_is_gone({
            "ctx": "agent", "kind": "loaded"
        }))
        self.assertFalse(civ6_play.board_event_proves_intro_is_gone({
            "ctx": "agent", "kind": "seat"
        }))
        self.assertTrue(civ6_play.board_event_proves_intro_is_gone({
            "ctx": "agent", "kind": "state", "turn": 1
        }))
        self.assertTrue(civ6_play.board_event_proves_intro_is_gone({
            "ctx": "agent", "kind": "turn", "turn": 1
        }))

    @staticmethod
    def _fake_clock():
        """A clock that only moves when the code under test sleeps.

        ⚠ Without this the tests are VACUOUS: with `time.sleep` stubbed to a
        no-op, real elapsed time is microseconds, so no deadline ever expires
        and a fixed-budget implementation passes every case identically.
        """
        now = {"t": 0.0}
        return now, (lambda: now["t"]), (lambda s: now.__setitem__("t", now["t"] + s))

    def _run_wait(self, polls, seconds, alive=True, still_loading=None):
        now, monotonic, sleep = self._fake_clock()
        script = iter(polls)
        tail = SimpleNamespace(poll=lambda: next(script, []))
        with mock.patch.object(civ6_play.time, "sleep", sleep), \
             mock.patch.object(civ6_play.time, "monotonic", monotonic), \
             mock.patch.object(civ6_play.env, "game_pids",
                               lambda: [1] if alive else []):
            return civ6_play.wait_for_agent_start(tail, lambda _e: None, seconds,
                                                  still_loading=still_loading)

    def test_a_loading_game_keeps_its_budget_while_the_mod_is_still_talking(self) -> None:
        """The regression from 2026-08-10: four starts died on a fixed deadline.

        The agent arrives only after a long map generation, but `autoclose`
        events keep arriving throughout it. A fixed budget expires mid-load and
        the caller then quits a game that was coming up fine. The wait loop
        sleeps 2 s a pass, so ten chattering passes is 20 s against a 6 s
        budget — impossible unless progress extends it.
        """
        polls = [[{"ctx": "autoclose", "kind": "autoclose_armed"}] for _ in range(10)]
        polls.append([{"ctx": "agent", "kind": "loaded"}])
        self.assertTrue(
            self._run_wait(polls, 6.0),
            "chatter while the game loads must extend the budget, not spend it",
        )

    def test_startup_drains_events_after_the_loaded_marker(self) -> None:
        """A single log read may contain the seat and first state as well."""
        seen = []
        polls = [[
            {"ctx": "agent", "kind": "loaded"},
            {"ctx": "agent", "kind": "seat"},
            {"ctx": "agent", "kind": "state", "turn": 1},
        ]]
        now, monotonic, sleep = self._fake_clock()
        script = iter(polls)
        tail = SimpleNamespace(poll=lambda: next(script, []))
        with mock.patch.object(civ6_play.time, "sleep", sleep), \
             mock.patch.object(civ6_play.time, "monotonic", monotonic), \
             mock.patch.object(civ6_play.env, "game_pids", return_value=[1]):
            self.assertTrue(
                civ6_play.wait_for_agent_start(tail, seen.append, seconds=6.0)
            )
        self.assertEqual([event["kind"] for event in seen],
                         ["loaded", "seat", "state"])

    def test_a_silent_game_still_gives_up(self) -> None:
        """Patience must not become a hang: real silence still ends the wait."""
        self.assertFalse(self._run_wait([[]] * 50, 6.0))

    def test_a_dead_game_is_not_waited_on(self) -> None:
        self.assertFalse(self._run_wait([[]] * 50, 600.0, alive=False))

    def test_endless_chatter_cannot_hang_the_harness(self) -> None:
        """The hard bound: a game that never seats an agent must still end."""
        polls = [[{"ctx": "autoclose", "kind": "autoclose_armed"}]] * 10000
        self.assertFalse(self._run_wait(polls, 6.0))

    def test_a_silently_loading_game_is_waited_out(self) -> None:
        """The case #1505 was written for and could never actually reach.

        A map being generated emits NOTHING -- the mod's in-game context has not
        loaded, so no event can extend the budget. Only the screen knows the
        difference, and it says the main menu is gone. The agent arrives on the
        16th pass, which is 30 s against a 6 s budget: unreachable unless the
        silent wait is extended on what the screen says.
        """
        polls = [[] for _ in range(15)]
        polls.append([{"ctx": "agent", "kind": "loaded"}])
        self.assertTrue(
            self._run_wait(polls, 6.0, still_loading=lambda: True),
            "a silent but visibly loading game must not be given up on",
        )

    def test_a_launch_that_missed_is_given_up_on_at_once(self) -> None:
        """The other half: silence plus a visible main menu is a dead click.

        Waiting cannot fix it, and every second spent here is one the retry does
        not get. The gate must return as soon as the first budget is spent, not
        run to the hard bound.
        """
        looks = []
        self.assertFalse(
            self._run_wait([[]] * 500, 6.0,
                           still_loading=lambda: looks.append(1) or False)
        )
        self.assertEqual(len(looks), 1,
                         "a game back at the main menu is asked about once")

    def test_the_screen_is_not_consulted_while_events_arrive(self) -> None:
        """Chatter is already proof of life; a screenshot per poll is waste."""
        looks = []
        polls = [[{"ctx": "autoclose", "kind": "autoclose_armed"}] for _ in range(10)]
        polls.append([{"ctx": "agent", "kind": "loaded"}])
        self.assertTrue(
            self._run_wait(polls, 6.0,
                           still_loading=lambda: looks.append(1) or True)
        )
        self.assertEqual(looks, [], "a talking game needs no screenshot")

    def test_a_loading_probe_cannot_defeat_the_hard_bound(self) -> None:
        """A screen stuck on neither the menu nor a game still has to end."""
        self.assertFalse(self._run_wait([[]] * 5000, 6.0,
                                        still_loading=lambda: True))

    def test_loading_patience_is_one_pool_for_the_whole_bootstrap(self) -> None:
        """Per-wait bounds alone do not bound a 16-attempt bootstrap.

        Six budgets per wait times sixteen attempts is over three hours in front
        of a screen that is neither a menu nor a game. The pool is what keeps
        the worst case to one extra bound however the attempts divide it, so a
        later attempt inherits what the earlier ones did not need.
        """
        run_dir = Path(tempfile.mkdtemp())
        patience = {"left": 3.0, "spent": 0.0}
        with mock.patch.object(civ6_play, "screenshot", lambda _p: None), \
             mock.patch.object(civ6_play, "_main_menu_visible", lambda _p: False):
            first = civ6_play._loading_probe(run_dir, 1, patience, 1.0)
            self.assertEqual([first(), first(), first()], [True, True, True])
            self.assertFalse(first(), "the pool is spent, so stop vouching")
            # A later attempt shares the same exhausted pool.
            second = civ6_play._loading_probe(run_dir, 2, patience, 1.0)
            self.assertFalse(second(),
                             "a fresh attempt must not refill the pool")

    def test_a_visible_main_menu_ends_the_wait_without_spending_patience(self) -> None:
        """A dead click is cheap to diagnose and must stay cheap."""
        run_dir = Path(tempfile.mkdtemp())
        patience = {"left": 600.0, "spent": 0.0}
        with mock.patch.object(civ6_play, "screenshot", lambda _p: None), \
             mock.patch.object(civ6_play, "_main_menu_visible", lambda _p: True):
            probe = civ6_play._loading_probe(run_dir, 1, patience, 120.0)
            self.assertFalse(probe())
        self.assertEqual(patience["left"], 600.0,
                         "seeing the menu costs nothing but the screenshot")

    def test_the_buffered_setup_batch_cannot_stand_in_for_a_live_game(self) -> None:
        """Exactly what the four dead starts looked like, and still look like.

        The mod's setup-screen chatter is still buffered when the gate opens, so
        its whole batch lands on the first poll -- at which point extending to
        `now + seconds` is the deadline the gate already had. That is why #1505's
        extension changed nothing for civvis-20260810T194817Z and ...T195339Z:
        22 `autoclose_armed`, zero `agent`, dead on the original budget. Only
        the screen can separate this from a map still generating.
        """
        batch = [{"ctx": "autoclose", "kind": "autoclose_armed"}] * 22
        agent = [{"ctx": "agent", "kind": "loaded"}]

        def script():
            return [batch] + [[] for _ in range(10)] + [agent]

        self.assertFalse(self._run_wait(script(), 6.0),
                         "a buffered setup batch is not proof of a live game")
        self.assertTrue(self._run_wait(script(), 6.0, still_loading=lambda: True),
                        "the same batch plus a vanished main menu is a live load")

    def test_leader_intro_requires_the_requested_leader(self) -> None:
        bounds = (864, 33, 864, 542)
        observations = [{
            "text": "TRAJAN", "x": 0.686, "y": 0.193,
            "width": 0.023, "height": 0.009,
        }]
        button = [{
            "text": "BEGIN GAME", "x": 0.70, "y": 0.41,
            "width": 0.08, "height": 0.02,
        }]
        with patch.object(civ6_play, "desktop_size", return_value=(1728, 1117)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=observations), \
             patch.object(civ6_play, "_leader_ocr", return_value=[]), \
             patch.object(civ6_play, "_leader_intro_button_ocr", return_value=button):
            self.assertTrue(
                civ6_play._leader_intro_visible(
                    Path("leader-intro.png"), bounds, "LEADER_TRAJAN"
                )
            )
            self.assertFalse(
                civ6_play._leader_intro_visible(
                    Path("leader-intro.png"), bounds, "LEADER_JADWIGA"
                )
            )

    def test_leader_intro_rejects_the_create_game_start_button(self) -> None:
        bounds = (864, 33, 864, 542)
        observations = [{
            "text": "TRAJAN", "x": 0.824, "y": 0.144,
            "width": 0.02, "height": 0.011,
        }]
        button = [{
            "text": "START GAME", "x": 0.74, "y": 0.506,
            "width": 0.03, "height": 0.01,
        }]
        with patch.object(civ6_play, "desktop_size", return_value=(1728, 1117)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=observations), \
             patch.object(civ6_play, "_leader_ocr", return_value=[]), \
             patch.object(civ6_play, "_leader_intro_button_ocr", return_value=button):
            self.assertFalse(
                civ6_play._leader_intro_visible(
                    Path("create-game.png"), bounds, "LEADER_TRAJAN"
                )
            )

    def test_leader_intro_click_uses_only_the_verified_card(self) -> None:
        bounds = (864, 33, 864, 542)
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "_leader_intro_visible", return_value=True), \
             patch.object(civ6_play, "click_at") as click:
            self.assertTrue(
                civ6_play.advance_leader_intro(
                    bounds, "LEADER_TRAJAN", Path(temporary), 2
                )
            )

        screenshot.assert_called_once_with(Path(temporary) / "leader-intro-attempt2-0.png")
        click.assert_called_once_with(
            864 + int(864 * civ6_play.LEADER_INTRO_BEGIN[0]),
            33 + int(542 * civ6_play.LEADER_INTRO_BEGIN[1]),
        )

    def test_leader_intro_stops_probing_once_the_board_is_confirmed(self) -> None:
        """A map state is safe to trust only after the screen rejected the card."""
        bounds = (864, 33, 864, 542)
        ready = iter([False, True])
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "_leader_intro_visible", return_value=False), \
             patch.object(civ6_play.time, "sleep") as sleep:
            self.assertFalse(
                civ6_play.advance_leader_intro(
                    bounds, "LEADER_TRAJAN", Path(temporary), 2,
                    retries=4, board_ready=lambda: next(ready),
                )
            )

        self.assertEqual(
            screenshot.call_args_list,
            [
                call(Path(temporary) / "leader-intro-attempt2-0.png"),
                call(Path(temporary) / "leader-intro-attempt2-1.png"),
            ],
        )
        sleep.assert_called_once_with(1.0)

    def test_live_run_holds_macos_awake_for_its_process_lifetime(self) -> None:
        with patch.object(civ6_play.sys, "platform", "darwin"), \
             patch.object(civ6_play.os, "getpid", return_value=4321), \
             patch.object(civ6_play.subprocess, "Popen") as popen:
            self.assertTrue(civ6_play.hold_macos_awake())

        popen.assert_called_once_with(
            ["/usr/bin/caffeinate", "-dims", "-w", "4321"],
            stdin=civ6_play.subprocess.DEVNULL,
            stdout=civ6_play.subprocess.DEVNULL,
            stderr=civ6_play.subprocess.DEVNULL,
            close_fds=True,
        )

    def test_desktop_size_rejects_a_multi_display_union(self) -> None:
        """⚠⚠ A SECOND MONITOR MUST NOT PLACE THE GAME OFF-SCREEN.

        `desktop_size` used to ask Finder for its desktop scroll area, which
        spans every attached display. On 2026-08-04 an external 2560x1440
        monitor was plugged in and Finder reported 3225x2557 — the union.
        `place_game` halved it, put Civ 6 at y=1333 on a 1117-point screen, and
        the setup vision could not read the difficulty dropdown. Every attempt
        in the batch died with "could not start a game from the main menu".

        The ceiling is the last line of defence if the source ever returns a
        union again.
        """
        with patch.object(civ6_play.subprocess, "run",
                          return_value=SimpleNamespace(stdout="3225,2557", returncode=0)):
            self.assertIsNone(
                civ6_play.desktop_size(),
                "a display-union-sized answer must be refused, not halved")

    def test_desktop_size_accepts_a_single_display(self) -> None:
        with patch.object(civ6_play.subprocess, "run",
                          return_value=SimpleNamespace(stdout="1728,1117", returncode=0)):
            self.assertEqual(civ6_play.desktop_size(), (1728, 1117))

    def test_desktop_size_refuses_an_unreadable_answer(self) -> None:
        """None means 'leave the window alone' — never a guess."""
        for bad in ("", "not,numbers", "1728", "0,0"):
            with patch.object(civ6_play.subprocess, "run",
                              return_value=SimpleNamespace(stdout=bad, returncode=0)):
                self.assertIsNone(civ6_play.desktop_size(), f"{bad!r} must be refused")

    def test_place_game_sizes_before_positioning_the_upper_quadrant(self) -> None:
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.subprocess, "run") as run:
            civ6_play.place_game("right", 0.5, 0.5)

        script = run.call_args.args[0][-1]
        self.assertLess(script.index("set size"), script.index("set position"))
        self.assertIn("set size to {756, 480}", script)
        self.assertIn("set position to {756, 33}", script)

    def test_screen_locked_reads_console_session_state(self) -> None:
        with patch.object(
            civ6_play.subprocess,
            "run",
            return_value=SimpleNamespace(
                stdout='"CGSSessionScreenIsLocked"=Yes', returncode=0
            ),
        ):
            self.assertTrue(civ6_play.screen_locked())

        with patch.object(
            civ6_play.subprocess,
            "run",
            return_value=SimpleNamespace(
                stdout='"CGSSessionScreenIsLocked"=No', returncode=0
            ),
        ):
            self.assertFalse(civ6_play.screen_locked())

    def test_play_waits_for_unlock_then_launches(self) -> None:
        with patch.object(civ6_play.vision, "available", return_value=True), \
             patch.object(civ6_play, "hold_macos_awake") as hold_awake, \
             patch.object(civ6_play, "screen_locked",
                          side_effect=[True, True, False]), \
             patch.object(civ6_play.time, "sleep") as sleep, \
             patch.object(civ6_play.gamelock, "acquire", return_value=True) as acquire, \
             patch.object(civ6_play.gamelock, "release") as release, \
             patch.object(civ6_play.launcher, "stop") as stop, \
             patch.object(civ6_play, "_play", return_value=0) as run:
            result = civ6_play.play(args(tag="unlock-test", lock_wait=0.0))

        self.assertEqual(result, 0)
        hold_awake.assert_called_once_with()
        sleep.assert_called_once_with(2.0)
        acquire.assert_called_once_with("unlock-test", wait_s=0.0)
        run.assert_called_once()
        stop.assert_called_once()
        release.assert_called_once()

    def test_world_congress_fallback_clicks_the_shipped_close_control(self) -> None:
        with patch.object(civ6_play, "focus_game") as focus, \
             patch.object(civ6_play, "game_window", return_value=(756, 33, 756, 480)), \
             patch.object(civ6_play, "click_at") as click:
            dismissed = civ6_play.dismiss_world_congress_between_turns()

        self.assertTrue(dismissed)
        focus.assert_called_once_with(civ6_play.GAME_SIDE, civ6_play.GAME_FRACTION)
        click.assert_called_once_with(1506, 66)

    def test_civvis_decision_mode_always_enables_state_export(self) -> None:
        self.assertTrue(civ6_play.state_export_enabled(
            SimpleNamespace(export_state=False, civvis_decides=True)
        ))
        self.assertTrue(civ6_play.state_export_enabled(
            SimpleNamespace(export_state=True, civvis_decides=False)
        ))
        self.assertFalse(civ6_play.state_export_enabled(
            SimpleNamespace(export_state=False, civvis_decides=False)
        ))

    def test_live_launcher_rejects_plan_forced_wars(self) -> None:
        error = io.StringIO()
        with patch.object(civ6_play.sys, "stderr", error):
            result = civ6_play.main(["--status", "--civvis-war-from-plan"])

        self.assertEqual(result, 2)
        self.assertIn("bypasses CIVVIS's war decision", error.getvalue())

    def test_setup_does_not_start_when_a_required_dropdown_is_unverified(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "set_dropdown", return_value=False) as setter, \
             patch.object(civ6_play, "select_requested_leader") as leader, \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "click_at") as click:
            started = civ6_play.configure_and_start((100, 33, 756, 480), args(), Path(temporary))

        self.assertFalse(started)
        setter.assert_called_once_with(
            (100, 33, 756, 480), "difficulty", "DIFFICULTY_SETTLER", Path(temporary)
        )
        screenshot.assert_not_called()
        leader.assert_not_called()
        click.assert_not_called()

    def test_setup_starts_only_after_every_required_dropdown_succeeds(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "set_dropdown", return_value=True) as setter, \
             patch.object(civ6_play, "select_requested_leader", return_value=True) as leader, \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "_observed_label_point",
                          return_value=(321, 432)) as observed, \
             patch.object(civ6_play, "focus_game") as focus, \
             patch.object(civ6_play, "click_at") as click:
            started = civ6_play.configure_and_start((100, 33, 756, 480), args(), Path(temporary))

        self.assertTrue(started)
        self.assertEqual(
            setter.call_args_list,
            [
                call((100, 33, 756, 480), "difficulty", "DIFFICULTY_SETTLER", Path(temporary)),
                call((100, 33, 756, 480), "map_size", "MAPSIZE_SMALL", Path(temporary)),
                call((100, 33, 756, 480), "speed", "GAMESPEED_ONLINE", Path(temporary)),
            ],
        )
        leader.assert_called_once_with(
            (100, 33, 756, 480), "LEADER_TRAJAN", Path(temporary)
        )
        screenshot.assert_called_once_with(Path(temporary) / "setup.png")
        observed.assert_called_once_with(
            Path(temporary) / "setup.png", "Start Game", (100, 33, 756, 480),
            strip=civ6_play.START_GAME_STRIP,
        )
        focus.assert_called_once_with(civ6_play.GAME_SIDE, civ6_play.GAME_FRACTION)
        click.assert_called_once_with(321, 432)

    def test_setup_refuses_to_start_without_a_visible_start_game_control(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "set_dropdown", return_value=True), \
             patch.object(civ6_play, "select_requested_leader", return_value=True), \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "_observed_label_point", return_value=None), \
             patch.object(civ6_play, "focus_game") as focus, \
             patch.object(civ6_play, "click_at") as click:
            started = civ6_play.configure_and_start(
                (100, 33, 756, 480), args(), Path(temporary)
            )

        self.assertFalse(started)
        screenshot.assert_called_once_with(Path(temporary) / "setup.png")
        focus.assert_not_called()
        click.assert_not_called()

    def test_setup_refuses_to_start_when_requested_leader_is_unverified(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "set_dropdown", return_value=True), \
             patch.object(civ6_play, "select_requested_leader", return_value=False), \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "click_at") as click:
            started = civ6_play.configure_and_start(
                (100, 33, 756, 480), args(), Path(temporary)
            )

        self.assertFalse(started)
        screenshot.assert_not_called()
        click.assert_not_called()

    def test_roster_resolves_requested_leader_to_rendered_name(self) -> None:
        self.assertEqual(civ6_play.leader_display_name("LEADER_JADWIGA"), "Jadwiga")

    @needs_pillow
    def test_leader_picker_scans_past_the_old_harald_cutoff(self) -> None:
        bounds = (756, 33, 756, 480)
        row = {"text": "Jadwiga", "x": 0.73, "y": 0.29,
               "width": 0.04, "height": 0.02}
        selected = {"text": "Jadwiga", "x": 0.73, "y": 0.155,
                    "width": 0.04, "height": 0.02}
        observations = [[] for _ in range(15)] + [[row], [selected]]

        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "screenshot",
                          side_effect=lambda p: Path(p).write_bytes(b"x") or True), \
             patch.object(civ6_play, "_leader_picker_open", return_value=True), \
             patch.object(
                 civ6_play, "_setup_current_leader",
                 return_value=("Random Leader", (1134, 140)),
             ), \
             patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", side_effect=observations), \
             patch.object(civ6_play.macos_input, "move"), \
             patch.object(civ6_play.macos_input, "scroll") as scroll, \
             patch.object(civ6_play, "click_at") as click, \
             patch.object(civ6_play.time, "sleep"):
            found = civ6_play.select_requested_leader(
                bounds, "LEADER_JADWIGA", Path(temporary)
            )

        self.assertTrue(found)
        self.assertEqual(scroll.call_count, 16)
        self.assertEqual(scroll.call_args_list[0], call(civ6_play.LEADER_SCROLL_RESET))
        self.assertTrue(all(
            invocation == call(civ6_play.LEADER_SCROLL_AMOUNT)
            for invocation in scroll.call_args_list[1:]
        ))
        self.assertEqual(click.call_count, 2)

    def test_leader_picker_open_requires_a_visible_roster_name(self) -> None:
        observations = [
            {"text": "Random Leader"},
            {"text": "Alexander"},
        ]
        with patch.object(civ6_play, "_leader_ocr", return_value=observations):
            self.assertTrue(civ6_play._leader_picker_open(
                Path("picker.png"), (756, 33, 756, 480)
            ))

        with patch.object(
            civ6_play, "_leader_ocr", return_value=[{"text": "Random Leader"}]
        ):
            self.assertFalse(civ6_play._leader_picker_open(
                Path("picker.png"), (756, 33, 756, 480)
            ))

    def test_setup_leader_readback_uses_the_rendered_random_leader_row(self) -> None:
        observation = {
            "text": "Random Leader", "x": 0.74, "y": 0.14,
            "width": 0.02, "height": 0.01,
        }
        bounds = (756, 33, 756, 480)
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=[]), \
             patch.object(civ6_play, "_menu_crop_ocr", return_value=[observation]):
            current = civ6_play._setup_current_leader(Path("setup.png"), bounds)

        self.assertEqual(current, ("Random Leader", (1134, 142)))

    def test_setup_leader_readback_uses_surrounding_headings_at_full_height(self) -> None:
        """A full-height window centres the leader row below its old fallback band."""
        observations = [
            {
                "text": "CHOOSE CIVILIZATION", "x": 0.20, "y": 0.359,
                "width": 0.05, "height": 0.008,
            },
            {
                "text": "?? Random Leader", "x": 0.230, "y": 0.374,
                "width": 0.010, "height": 0.010,
            },
            {
                "text": "CHOOSE GAME DIFFICULTY", "x": 0.20, "y": 0.389,
                "width": 0.06, "height": 0.010,
            },
        ]
        bounds = (0, 33, 864, 1084)
        with patch.object(civ6_play, "desktop_size", return_value=(1728, 1117)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=observations), \
             patch.object(civ6_play, "_menu_crop_ocr", return_value=[]):
            current = civ6_play._setup_current_leader(Path("setup.png"), bounds)

        self.assertEqual(current, ("Random Leader", (406, 423)))

    @needs_pillow
    def test_leader_ocr_maps_the_upscaled_crop_back_to_the_desktop(self) -> None:
        from PIL import Image

        observation = {
            "text": "Jadwiga", "x": 0.30, "y": 0.50,
            "width": 0.10, "height": 0.05,
        }
        with tempfile.TemporaryDirectory() as temporary:
            shot = Path(temporary) / "leader.png"
            Image.new("RGB", (3024, 1964), "white").save(shot)
            with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
                 patch.object(civ6_play.macos_ocr, "recognize", return_value=[observation]):
                mapped = civ6_play._leader_ocr(
                    shot, (756, 33, 756, 480)
                )

        self.assertEqual(mapped[0]["text"], "Jadwiga")
        self.assertAlmostEqual(mapped[0]["x"], 0.733, places=3)
        self.assertAlmostEqual(mapped[0]["y"], 0.293, places=3)
        self.assertAlmostEqual(mapped[0]["width"], 0.011, places=3)

    def test_setup_recovery_backs_out_until_main_menu_is_verified(self) -> None:
        bounds = (756, 33, 756, 480)
        observations = [[], [], [{"text": "Single Player"}]]
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "screenshot"), \
             patch.object(civ6_play.macos_ocr, "recognize", side_effect=observations), \
             patch.object(civ6_play, "click_at") as click, \
             patch.object(civ6_play.time, "sleep"):
            recovered = civ6_play.return_to_main_menu(
                bounds, Path(temporary), attempt=3
            )

        self.assertTrue(recovered)
        self.assertEqual(
            click.call_args_list,
            [call(1307, 116), call(1307, 116)],
        )

    def test_main_menu_click_uses_observed_row_center(self) -> None:
        observation = {
            "text": "Single Player", "x": 0.72, "y": 0.255,
            "width": 0.03, "height": 0.01,
        }
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=[observation]):
            point = civ6_play._main_menu_point(
                Path("menu.png"), (756, 33, 756, 480)
            )

        self.assertEqual(point, (1111, 255))

    def test_main_menu_click_tolerates_one_vision_glyph_error(self) -> None:
        observation = {
            "text": "Single Plaver", "x": 0.72, "y": 0.255,
            "width": 0.03, "height": 0.01,
        }
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=[observation]):
            point = civ6_play._main_menu_point(
                Path("menu.png"), (756, 33, 756, 480)
            )

        self.assertEqual(point, (1111, 255))

    def test_main_menu_click_tolerates_live_missing_vision_glyphs(self) -> None:
        observation = {
            "text": "Single Plave", "x": 0.72, "y": 0.255,
            "width": 0.03, "height": 0.01,
        }
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=[observation]):
            point = civ6_play._main_menu_point(
                Path("menu.png"), (756, 33, 756, 480)
            )

        self.assertEqual(point, (1111, 255))

    def test_main_menu_click_rejects_a_different_long_label(self) -> None:
        self.assertFalse(civ6_play._menu_label_matches("Multiplayer", "Single Player"))

    def test_create_game_click_uses_its_visible_label(self) -> None:
        observations = [
            {"text": "Resume Game", "x": 0.75, "y": 0.28,
             "width": 0.03, "height": 0.01},
            {"text": "Create Game", "x": 0.75, "y": 0.31,
             "width": 0.03, "height": 0.01},
        ]
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=observations):
            point = civ6_play._observed_label_point(
                Path("submenu.png"), "Create Game", (756, 33, 756, 480)
            )

        self.assertEqual(point, (1156, 309))

    def test_create_game_click_retries_with_enlarged_menu_crop(self) -> None:
        observation = {
            "text": "Create Game", "x": 0.75, "y": 0.31,
            "width": 0.03, "height": 0.01,
        }
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=[]), \
             patch.object(civ6_play, "_menu_crop_ocr", return_value=[observation]) as crop:
            point = civ6_play._observed_label_point(
                Path("submenu.png"), "Create Game", (756, 33, 756, 480)
            )

        self.assertEqual(point, (1156, 309))
        crop.assert_called_once_with(Path("submenu.png"), (756, 33, 756, 480))

    def test_setup_value_readback_distinguishes_standard_speed_from_map_size(self) -> None:
        observations = [
            {"text": "Standard", "x": 0.74, "y": 0.185,
             "width": 0.02, "height": 0.01},
            {"text": "Small", "x": 0.74, "y": 0.225,
             "width": 0.02, "height": 0.01},
        ]
        bounds = (756, 33, 756, 480)
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=[]), \
             patch.object(civ6_play, "_menu_crop_ocr", return_value=observations):
            speed = civ6_play._setup_current_value(
                Path("setup.png"), bounds, "speed"
            )
            size = civ6_play._setup_current_value(
                Path("setup.png"), bounds, "map_size"
            )

        self.assertEqual(speed[0], "GAMESPEED_STANDARD")
        self.assertEqual(size[0], "MAPSIZE_SMALL")

    @needs_pillow
    def test_saved_game_bootstrap_uses_named_row_and_lower_action_button(self) -> None:
        bounds = (756, 33, 756, 480)
        observations = [
            [{"text": "Single Player", "x": 0.72, "y": 0.255,
              "width": 0.03, "height": 0.01}],
            [{"text": "Single Player", "x": 0.72, "y": 0.255,
              "width": 0.03, "height": 0.01}],
            [{"text": "Load Game", "x": 0.75, "y": 0.30,
              "width": 0.03, "height": 0.01}],
            [{"text": "CivvisWriterRepro", "x": 0.68, "y": 0.18,
              "width": 0.04, "height": 0.01}],
            [
                {"text": "LOAD GAME", "x": 0.73, "y": 0.11,
                 "width": 0.04, "height": 0.01},
                {"text": "Load Game", "x": 0.69, "y": 0.49,
                 "width": 0.04, "height": 0.02},
            ],
        ]

        class Tail:
            def poll(self):
                return [{"ctx": "agent", "kind": "loaded"}]

        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "focus_game"), \
             patch.object(civ6_play, "game_window", return_value=bounds), \
             patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play, "screenshot"), \
             patch.object(civ6_play.macos_ocr, "recognize", side_effect=observations), \
             patch.object(civ6_play.env, "game_pids", return_value=[123]), \
             patch.object(civ6_play.time, "sleep"), \
             patch.object(civ6_play, "click_at") as click:
            loaded = civ6_play.bootstrap_saved_game(
                Tail(), lambda _event: None, Path(temporary),
                args(load_save="/saves/CivvisWriterRepro.Civ6Save"),
            )

        self.assertTrue(loaded)
        self.assertEqual(
            click.call_args_list,
            [call(869, 441), call(1111, 255), call(1156, 299),
             call(1058, 181), call(1073, 491)],
        )

    def test_latest_autosave_is_by_mtime_and_bounded_by_the_attempt_start(self) -> None:
        # The numbering wraps, so the newest file can carry a lower number; and a
        # resume must never reload a save from some earlier game.
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            older = folder / "AutoSave_0149.Civ6Save"
            newer = folder / "AutoSave_0003.Civ6Save"
            stray = folder / "notes.txt"
            for path in (older, newer, stray):
                path.write_bytes(b"x")
            os.utime(older, (1_000, 1_000))
            os.utime(newer, (2_000, 2_000))
            os.utime(stray, (3_000, 3_000))
            self.assertEqual(civ6_play.latest_autosave(folder), newer)
            self.assertEqual(civ6_play.latest_autosave(folder, newer_than=1_500), newer)
            self.assertIsNone(civ6_play.latest_autosave(folder, newer_than=2_500),
                              "nothing written since the attempt began")
            self.assertIsNone(civ6_play.latest_autosave(folder / "missing"))

    @needs_pillow
    def test_saved_game_bootstrap_ticks_the_autosaves_filter_when_the_row_is_hidden(self) -> None:
        # Measured on civvis-20260815T230003Z-cont: the Load Game list opens with
        # the Autosaves filter off, "(Quick Save)" alone, and the resume refused
        # "not visible" three times with AutoSave_0098 on disk.
        bounds = (756, 33, 756, 480)
        observations = [
            [{"text": "Single Player", "x": 0.72, "y": 0.255,
              "width": 0.03, "height": 0.01}],
            [{"text": "Single Player", "x": 0.72, "y": 0.255,
              "width": 0.03, "height": 0.01}],
            [{"text": "Load Game", "x": 0.75, "y": 0.30,
              "width": 0.03, "height": 0.01}],
            # The panel: filter unticked, no autosave row — read once for the
            # row (not found) and once more for the filter label.
            [{"text": "Autosaves", "x": 0.71, "y": 0.155,
              "width": 0.03, "height": 0.01},
             {"text": "(Quick Save)", "x": 0.71, "y": 0.20,
              "width": 0.03, "height": 0.01}],
            [{"text": "Autosaves", "x": 0.71, "y": 0.155,
              "width": 0.03, "height": 0.01},
             {"text": "(Quick Save)", "x": 0.71, "y": 0.20,
              "width": 0.03, "height": 0.01}],
            # After the tick: the row is listed (OCR drops the underscore).
            [{"text": "Autosaves", "x": 0.71, "y": 0.155,
              "width": 0.03, "height": 0.01},
             {"text": "AutoSave 0102", "x": 0.68, "y": 0.21,
              "width": 0.04, "height": 0.01}],
            [
                {"text": "LOAD GAME", "x": 0.73, "y": 0.11,
                 "width": 0.04, "height": 0.01},
                {"text": "Load Game", "x": 0.69, "y": 0.49,
                 "width": 0.04, "height": 0.02},
            ],
        ]

        class Tail:
            def poll(self):
                return [{"ctx": "agent", "kind": "loaded"}]

        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "focus_game"), \
             patch.object(civ6_play, "game_window", return_value=bounds), \
             patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play, "screenshot"), \
             patch.object(civ6_play.macos_ocr, "recognize", side_effect=observations), \
             patch.object(civ6_play, "_menu_crop_ocr", return_value=[]), \
             patch.object(civ6_play.env, "game_pids", return_value=[123]), \
             patch.object(civ6_play.time, "sleep"), \
             patch.object(civ6_play, "click_at") as click:
            loaded = civ6_play.bootstrap_saved_game(
                Tail(), lambda _event: None, Path(temporary),
                args(load_save="/saves/auto/AutoSave_0102.Civ6Save"),
            )

        self.assertTrue(loaded)
        # main-menu button, Single Player, Load Game, the Autosaves filter, the
        # row, then the lower Load Game action button.
        self.assertEqual(
            click.call_args_list,
            [call(869, 441), call(1111, 255), call(1156, 299),
             call(1096, 157), call(1058, 211), call(1073, 491)],
        )

    def test_seat_match_requires_map_and_leader(self) -> None:
        event = {
            "difficulty": "DIFFICULTY_SETTLER",
            "size": "MAPSIZE_SMALL",
            "speed": "GAMESPEED_ONLINE",
            "map": "Continents.lua",
            "leader": "LEADER_TRAJAN",
            "modes": [],
            "ruleset": "RULESET_EXPANSION_2",
        }

        self.assertEqual(
            civ6_play.seat_matches_requested(event, args()), (True, True, True))
        self.assertEqual(
            civ6_play.seat_matches_requested({**event, "leader": "LEADER_CLEOPATRA"}, args()),
            (False, True, True),
        )
        self.assertEqual(
            civ6_play.seat_matches_requested({**event, "map": "Pangaea.lua"}, args()),
            (False, True, True),
        )

    def test_seat_match_accepts_the_reported_leader_when_none_was_requested(self) -> None:
        event = {
            "difficulty": "DIFFICULTY_SETTLER",
            "size": "MAPSIZE_SMALL",
            "speed": "GAMESPEED_ONLINE",
            "map": "Continents.lua",
            "leader": "LEADER_GANDHI",
            "modes": [],
            "ruleset": "RULESET_EXPANSION_2",
        }

        self.assertEqual(
            civ6_play.seat_matches_requested(event, args(leader=None)),
            (True, True, True),
        )


class SetupRowReadbackTest(unittest.TestCase):
    """The Create Game panel, at the geometry that broke every batch on 2026-08-02.

    Screen 1728x1117 logical points, game window (864, 46, 864, 528) -- Civilization
    VI running windowed in a screen corner rather than filling it.  Every row sits in
    the central column at x=1296, and the rows are only ~29 points apart, so the
    historical vertical bands (map_size 0.34-0.56, speed 0.24-0.44) overlap and both
    contain the speed row.  Taken from run civvis-20260802T131519Z.
    """

    SCREEN = (1728, 1117)
    BOUNDS = (864, 46, 864, 528)

    @staticmethod
    def _row(text: str, y: int) -> dict:
        # Centre the observation on (1296, y) in logical points, expressed the way
        # macos_ocr reports: normalized fractions of the whole screen.
        return {
            "text": text,
            "x": 1296 / 1728 - 0.02, "width": 0.04,
            "y": y / 1117 - 0.005, "height": 0.01,
        }

    def _panel(self) -> list[dict]:
        return [
            self._row("Choose Game Difficulty", 205),
            self._row("Settler", 216),
            self._row("Choose Game Speed", 234),
            self._row("Standard", 245),
            self._row("Choose Map Type", 263),
            self._row("Continents", 274),
            self._row("Choose Map Size", 292),
            self._row("Small", 303),
        ]

    def _read(self, name: str, observations: list[dict]):
        with patch.object(civ6_play, "desktop_size", return_value=self.SCREEN), \
             patch.object(civ6_play.macos_ocr, "recognize", return_value=observations), \
             patch.object(civ6_play, "_menu_crop_ocr", return_value=[]):
            return civ6_play._setup_current_value(
                Path("setup.png"), self.BOUNDS, name
            )

    def test_map_size_does_not_read_the_game_speed_row(self) -> None:
        """The regression itself.

        "Standard" is a legal value for BOTH speed and map size, and the speed row
        is the first one an overlapping band reaches.  Reading it as the map size
        made `set_dropdown` click the speed dropdown to correct a map size that was
        already correct, never read Small back, and refuse to start the game --
        4 ledger rows, 0 games, on a panel that said Small the whole time.
        """
        value = self._read("map_size", self._panel())
        self.assertIsNotNone(value)
        self.assertEqual(value[0], "MAPSIZE_SMALL")
        # And the POINT matters as much as the name: `set_dropdown` clicks whatever
        # this returns, so a right answer at the speed row's coordinates would still
        # open the wrong list.  Map size sits at y=303, speed at y=245.
        self.assertAlmostEqual(value[1][1], 303, delta=2)
        self.assertEqual(value[1][0], 1296)

    def test_every_row_reads_its_own_value(self) -> None:
        panel = self._panel()
        self.assertEqual(self._read("difficulty", panel)[0], "DIFFICULTY_SETTLER")
        self.assertEqual(self._read("speed", panel)[0], "GAMESPEED_STANDARD")
        self.assertEqual(self._read("map_size", panel)[0], "MAPSIZE_SMALL")

    def test_a_row_whose_value_is_missing_does_not_borrow_the_next_row(self) -> None:
        """Silence beats the wrong answer.

        With no value under "Choose Map Size" the reader must say nothing, so the
        caller retries and then refuses.  Returning the row below -- or a band guess
        -- is how a game gets started on settings nobody asked for.
        """
        panel = [row for row in self._panel() if row["text"] != "Small"]
        self.assertIsNone(self._read("map_size", panel))

    def test_bands_still_answer_when_no_heading_survives_ocr(self) -> None:
        """The fallback keeps a headingless panel readable rather than blind."""
        headings = set(civ6_play.SETUP_HEADINGS.values())
        panel = [row for row in self._panel() if row["text"] not in headings]
        self.assertEqual(self._read("difficulty", panel)[0], "DIFFICULTY_SETTLER")

    def test_a_heading_hidden_by_an_open_list_is_placed_from_the_others(self) -> None:
        """An open dropdown covers the rows beneath it.

        On the live open-list shot only `difficulty` and `speed` came back; without
        recovery the reader falls through to the bands and reads the speed row again.
        difficulty=205 and speed=234 give a pitch of 29, which puts map_size two rows
        below speed at 292 -- where it measures when nothing covers it.
        """
        covered = {"Choose Map Type", "Choose Map Size"}
        panel = [row for row in self._panel() if row["text"] not in covered]
        rows = civ6_play._setup_rows(panel, self.BOUNDS, self.SCREEN)
        self.assertEqual(rows["map_type"], 263)
        self.assertEqual(rows["map_size"], 292)
        self.assertEqual(self._read("map_size", panel)[0], "MAPSIZE_SMALL")

    def test_one_legible_heading_is_not_enough_to_invent_a_pitch(self) -> None:
        """Two points make a scale; one makes a guess. Keep the band instead."""
        keep = {"Choose Game Difficulty"}
        headings = set(civ6_play.SETUP_HEADINGS.values())
        panel = [row for row in self._panel()
                 if row["text"] not in headings or row["text"] in keep]
        rows = civ6_play._setup_rows(panel, self.BOUNDS, self.SCREEN)
        self.assertEqual(set(rows), {"difficulty"})


class EndGameScreenHoldTests(unittest.TestCase):
    """The victory/defeat screen is the only screen that states the OUTCOME.

    ⚠⚠ It had no clock of its own, so it took the general announcement clock —
    which `civ6_civvis_climb` sets to 0.05s on purpose, so no popup sits on the map
    the operator is comparing against CIVVIS. The result a whole run exists to
    produce was on screen for a twentieth of a second.
    """

    def test_the_end_screen_hold_is_ten_seconds_by_default(self):
        """Asserted on the shipped parser line, not on a copy of the number."""
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text()
        self.assertIn('"--end-game-seconds", type=float, default=10.0', source)

    def test_the_hold_reaches_the_baked_mod_config(self):
        """⚠ A flag the mod never receives is a flag that does nothing. `civ6_play`
        bakes the config the Lua shim reads; the key has to be in it.

        Built from a namespace that answers None for anything not named here, so
        this stays about `EndGameSeconds` and does not become a second, drifting
        copy of every unrelated setting `build_config` happens to read.
        """
        class Defaults(SimpleNamespace):
            def __getattr__(self, name):    # only for attributes not set below
                return None

        config = civ6_play.build_config(
            Defaults(tag="t", game_mode=[], end_game_seconds=10.0,
                     difficulty="DIFFICULTY_SETTLER", map_size="MAPSIZE_SMALL",
                     speed="GAMESPEED_ONLINE", map="Continents.lua",
                     leader="LEADER_TRAJAN"))
        self.assertEqual(config["EndGameSeconds"], 10.0)

    def test_the_shim_gives_the_end_screen_its_own_clock(self):
        """The far end of the wire, asserted on the Lua that actually runs.

        ⚠ Order matters and is asserted: the era table can only SHORTEN a clock and
        the dialogue rule takes a `math.min`, so an end-screen line placed after the
        dialogue one would be clamped back to 0.25s and the hold would silently not
        happen.
        """
        lua = (Path(__file__).resolve().parent / "civ6_control" / "mod"
               / "CivvisControlAutoClose.lua").read_text()
        self.assertIn("local END_SECONDS = tonumber(cfg.EndGameSeconds) or 10.0;", lua)
        self.assertIn("local END_SCREENS = { EndGameMenu = true };", lua)
        end_at = lua.index("if END_SCREENS[NAME] then")
        era_at = lua.index("if ERA_SCREENS[NAME] then SECONDS = ERA_SECONDS; end")
        dialogue_at = lua.index('if NAME == "DiplomacyActionView"')
        self.assertLess(era_at, end_at, "the era clock must not overwrite the hold")
        self.assertLess(end_at, dialogue_at,
                        "a later math.min would clamp the hold back to 0.25s")

    def test_end_screens_declared_after_name_exists(self):
        """⚠ `ERA_SCREENS[nil]` reads as nil without complaint — that is exactly how
        the era clock went a whole project unapplied. The lookup must sit below the
        line that gives `NAME` a value."""
        lua = (Path(__file__).resolve().parent / "civ6_control" / "mod"
               / "CivvisControlAutoClose.lua").read_text()
        self.assertLess(lua.index('local NAME = "unknown";'),
                        lua.index("if END_SCREENS[NAME] then"))


class CounterResolutionConfigTests(unittest.TestCase):
    """⚠ A flag the mod never receives is a flag that does nothing."""

    @staticmethod
    def _config(**changes):
        class Defaults(SimpleNamespace):
            def __getattr__(self, name):
                return None

        return civ6_play.build_config(
            Defaults(tag="t", game_mode=[],
                     difficulty="DIFFICULTY_SETTLER", map_size="MAPSIZE_SMALL",
                     speed="GAMESPEED_ONLINE", map="Continents.lua",
                     leader="LEADER_TRAJAN", **changes))

    def test_both_keys_reach_the_baked_config(self) -> None:
        cfg = self._config(counter_resolutions=True, counter_resolution_bar=60.0)
        self.assertIs(cfg["CounterResolutions"], True)
        self.assertEqual(cfg["CounterResolutionBar"], 60.0)

    def test_the_withheld_arm_reaches_the_config_too(self) -> None:
        cfg = self._config(counter_resolutions=False, counter_resolution_bar=60.0)
        self.assertIs(cfg["CounterResolutions"], False)


class PeaceDeterrenceConfigTests(unittest.TestCase):
    """⚠ A flag the mod never receives is a flag that does nothing (#1098's
    lesson): the key has to reach the baked config, and the Lua has to read it.
    """

    @staticmethod
    def _config(**changes):
        class Defaults(SimpleNamespace):
            def __getattr__(self, name):
                return None

        return civ6_play.build_config(
            Defaults(tag="t", game_mode=[],
                     difficulty="DIFFICULTY_SETTLER", map_size="MAPSIZE_SMALL",
                     speed="GAMESPEED_ONLINE", map="Continents.lua",
                     leader="LEADER_TRAJAN", **changes))

    def test_the_flag_reaches_the_baked_mod_config(self):
        self.assertIs(self._config(peace_deterrence=True)["PeaceDeterrence"], True)
        self.assertIs(self._config(peace_deterrence=False)["PeaceDeterrence"], False)

    def test_the_lua_gate_is_bounded_and_withholdable(self):
        lua = (Path(__file__).resolve().parent / "civ6_control" / "mod"
               / "CivvisControlAgent.lua").read_text()
        self.assertIn("if cfg.PeaceDeterrence and not atWar and strongestMet > 0", lua)
        self.assertIn("and ourStrength * 2 < strongestMet then", lua)
        # Bounded: unlike the losingWar lift, deterrence stays under ArmyCap.
        gate = lua.split("if cfg.PeaceDeterrence and not atWar", 1)[1]
        gate = gate.split("-- ★★★ A BATTERING RAM", 1)[0]
        self.assertIn("math.min((counts.military or 0) + 2,", gate)
        self.assertIn("cfg.ArmyCap or ((cfg.WarArmy or 4) + 6)));", gate)
        # And the strength it weighs is met-gated, war or peace.
        self.assertIn("try(function() return diplomacy:HasMet(otherId); end, false)", lua)
        self.assertIn("return atWar, ours, worst, strongestMet;", lua)


class ScreenshotFailureTests(unittest.TestCase):
    """A silently failed `screencapture` must become a retried, unreadable
    poll — three consecutive ladder attempts died on 2026-08-17 when a missing
    shot cascaded into an uncaught `OCRUnavailable`."""

    def test_a_capture_that_writes_nothing_is_retried_then_reported(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play.subprocess, "run") as run, \
             patch.object(civ6_play.time, "sleep") as sleep:
            landed = civ6_play.screenshot(Path(temporary) / "missing.png")
        self.assertFalse(landed)
        self.assertEqual(run.call_count, 2, "the capture is retried once")
        sleep.assert_called_once()

    def test_a_capture_that_lands_is_true_first_try(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "shot.png"

            def fake_capture(cmd, **_):
                path.write_bytes(b"x")

            with patch.object(civ6_play.subprocess, "run",
                              side_effect=fake_capture) as run:
                self.assertTrue(civ6_play.screenshot(path))
            self.assertEqual(run.call_count, 1)

    def test_a_missing_shot_reads_as_nothing_not_a_crash(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play.macos_ocr, "recognize") as recognize:
            observations = civ6_play._leader_ocr(
                Path(temporary) / "never-written.png", (756, 33, 756, 480))
        self.assertEqual(observations, [])
        recognize.assert_not_called()

    def test_a_host_without_pillow_loses_the_crop_not_the_read(self) -> None:
        """No Pillow must cost the 4x enhancement, never the whole read.

        ⚠ `from PIL import Image` raised `ModuleNotFoundError` past a handler
        that already returned `[]` for exactly this shape of failure, out of a
        function whose sibling test is named "a missing shot reads as nothing,
        not a crash". On ubuntu — no Pillow — it crashed, and the shared
        `collaboration-policy` gate went red for every open pull request.
        """
        real_import = builtins.__import__

        def no_pillow(name, *args, **kwargs):
            if name == "PIL" or name.startswith("PIL."):
                raise ModuleNotFoundError("No module named 'PIL'")
            return real_import(name, *args, **kwargs)

        # An existing file falls back to the native OCR: the crop is gone, the
        # read is not.
        with tempfile.TemporaryDirectory() as temporary:
            shot = Path(temporary) / "shot.png"
            shot.write_bytes(b"not really a png")
            with patch.object(builtins, "__import__", side_effect=no_pillow), \
                 patch.object(civ6_play.macos_ocr, "recognize",
                              return_value=[{"text": "TRAJAN"}]) as recognize:
                observations = civ6_play._leader_ocr(shot, (756, 33, 756, 480))
            self.assertEqual(observations, [{"text": "TRAJAN"}])
            recognize.assert_called_once_with(shot)

        # A missing file still reads as nothing, and never reaches the OCR.
        with tempfile.TemporaryDirectory() as temporary:
            with patch.object(builtins, "__import__", side_effect=no_pillow), \
                 patch.object(civ6_play.macos_ocr, "recognize") as recognize:
                observations = civ6_play._leader_ocr(
                    Path(temporary) / "never-written.png", (756, 33, 756, 480))
            self.assertEqual(observations, [])
            recognize.assert_not_called()

    def test_intro_probe_survives_an_unavailable_ocr(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(
                 civ6_play.macos_ocr, "recognize",
                 side_effect=civ6_play.macos_ocr.OCRUnavailable("0 x 0")):
            visible = civ6_play._leader_intro_visible(
                Path(temporary) / "intro.png", (756, 33, 756, 480),
                "LEADER_TRAJAN")
        self.assertFalse(visible)


class LiveControlArmTests(unittest.TestCase):
    """The control arm's route to a live game. `civvis_orders --without` has
    always existed and no launcher could ask for it, so no live treatment was
    ever withholdable in a real game."""

    @staticmethod
    def _args(**changes):
        values = {
            "civvis_victory": "science", "civvis_strategy": "auto",
            "civvis_war_from_plan": False, "civvis_refresh_seconds": None,
            "civvis_without": [], "timeout": 7200.0,
        }
        values.update(changes)
        return SimpleNamespace(**values)

    def _cmd(self, **changes):
        return civ6_play.supervised_brain_command(
            self._args(**changes), Path("/tmp/run"),
            Path("/tmp/orders.sqlite"), Path("/tmp/civvis_orders"))

    def test_the_full_bundle_withholds_nothing(self) -> None:
        self.assertNotIn("--without", self._cmd())

    def test_each_withheld_treatment_reaches_the_decider(self) -> None:
        cmd = self._cmd(civvis_without=["peacetime-deterrence", "stacked-escort"])
        pairs = [(cmd[i], cmd[i + 1]) for i, tok in enumerate(cmd)
                 if tok == "--without"]
        self.assertEqual(
            pairs,
            [("--without", "peacetime-deterrence"),
             ("--without", "stacked-escort")],
            "each treatment needs its own flag; the decider takes one name each",
        )
class OpeningTempoRecordTests(unittest.TestCase):
    """`civ6_play` records the opening tempo from its own event stream, so the
    number describes the run being recorded rather than a later reading."""

    def test_the_turn_sixty_sample_is_what_the_empire_HELD(self) -> None:
        # Deliberately not derived from the founding list: a city founded and
        # then LOST before turn 60 must not be counted as held.
        self.assertEqual(civ6_play.OPENING_TEMPO_TURN, 60)

    def test_second_city_turn_is_the_second_founding(self) -> None:
        founds = [2, 41, 19, 55]
        self.assertEqual(sorted(founds)[1], 19)

    def test_a_run_that_founded_once_has_no_second_city_turn(self) -> None:
        founds = [2]
        self.assertIsNone(sorted(founds)[1] if len(founds) >= 2 else None)


class SupervisedBrainCommandTests(unittest.TestCase):
    """Every flag that reaches the decision worker is decided in one builder."""

    @staticmethod
    def _args(**changes):
        values = {
            "civvis_victory": "science",
            "civvis_strategy": "auto",
            "civvis_war_from_plan": False,
            "civvis_refresh_seconds": None,
            "civvis_without": [],
            "timeout": 7200.0,
        }
        values.update(changes)
        return SimpleNamespace(**values)

    def _command(self, **changes):
        return civ6_play.supervised_brain_command(
            self._args(**changes), Path("/tmp/run"),
            Path("/tmp/orders.sqlite"), Path("/tmp/civvis_orders"))

    def test_the_brain_default_cadence_is_left_alone(self):
        self.assertNotIn("--github-refresh-seconds", self._command())

    def test_refresh_seconds_reach_the_brain(self):
        cmd = self._command(civvis_refresh_seconds=120.0)
        self.assertEqual(cmd[cmd.index("--github-refresh-seconds") + 1], "120.0")

    def test_zero_is_a_choice_not_an_absence(self):
        # 0 is falsy, and 0 is exactly the value a pinned batch depends on —
        # an `if args.civvis_refresh_seconds:` implementation would drop it.
        cmd = self._command(civvis_refresh_seconds=0.0)
        self.assertEqual(cmd[cmd.index("--github-refresh-seconds") + 1], "0.0")

    def test_every_lane_reaches_the_worker_verbatim(self):
        """The launcher forwards the objective; it does not translate it."""
        for lane in civ6_play.VICTORY_LANES:
            with self.subTest(lane=lane):
                cmd = self._command(civvis_victory=lane)
                self.assertEqual(cmd[cmd.index("--victory") + 1], lane)


class TheSummaryNamesTheObjective(unittest.TestCase):
    """The summary is what the ladder is built from, so the lane has to be in it.

    ⚠ `civ6_civvis_climb.py` stamps `victory_target` on its own JSONL row, which
    is a DIFFERENT FILE from the summary `civ6_ladder.py` reads — which is why
    `docs/civ6_ladder.json` carried 307 rows and no lane on any of them.
    """

    def test_the_builder_writes_the_lane_it_was_told_to_play(self):
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        self.assertIn('"victory_target": args.civvis_victory if args.civvis_decides',
                      source)

    def test_a_run_that_is_not_civvis_deciding_claims_no_lane(self):
        """The flag names what the CIVVIS worker plays for. A run the worker did
        not decide has no lane to report, and reporting one would file a Firaxis
        AI game under an objective CIVVIS never held."""
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        self.assertIn("if args.civvis_decides else None", source)


class TheRulesetIsReadBackFromTheGame(unittest.TestCase):
    """★★★★★ The one setting this harness set and never read back.

    Difficulty, size, speed, map, leader and modes are all verified from inside
    the game, because `setup: "(absent)"` means a requested setting can silently
    fail to apply. `--ruleset` was passed to the mod and taken on trust — and
    CIVVIS models Gathering Storm and nothing else, so a Vanilla or Rise & Fall
    game is a different game with different technologies, costs and units, not a
    weaker measurement of the same one.
    """

    @staticmethod
    def _seat(**changes):
        event = {
            "difficulty": "DIFFICULTY_SETTLER",
            "size": "MAPSIZE_SMALL",
            "speed": "GAMESPEED_ONLINE",
            "map": "Continents.lua",
            "leader": "LEADER_TRAJAN",
            "modes": [],
            "ruleset": "RULESET_EXPANSION_2",
        }
        event.update(changes)
        return event

    def test_the_asked_for_ruleset_matches(self):
        self.assertEqual(
            civ6_play.seat_matches_requested(self._seat(), args()), (True, True, True))

    def test_a_vanilla_game_is_refused_and_fails_the_whole_config(self):
        configured, modes, ruleset = civ6_play.seat_matches_requested(
            self._seat(ruleset="RULESET_STANDARD"), args())
        self.assertFalse(ruleset)
        self.assertFalse(configured, "a different ruleset is not a configured game")
        self.assertTrue(modes, "the modes axis is unaffected")

    def test_an_unreported_ruleset_is_unverified_not_agreement(self):
        """An older mod build reports nothing. That is not the same as a match,
        for the same reason a missing `modes` list is not an empty one."""
        for absent in (None, "?"):
            with self.subTest(absent=absent):
                _, _, ruleset = civ6_play.seat_matches_requested(
                    self._seat(ruleset=absent), args())
                self.assertFalse(ruleset)

    def test_the_mod_actually_exports_it(self):
        """The Python half is useless if the survey never sends the field."""
        lua = (Path(__file__).resolve().parent / "civ6_control" / "mod"
               / "CivvisControlAgent.lua").read_text(encoding="utf-8")
        self.assertIn("ruleset = typeName(GameInfo.Rulesets,", lua)
        self.assertIn("GameConfiguration.GetRuleSet()", lua)
        self.assertIn("row.RulesetType", lua, "typeName must resolve a Ruleset row")

    def test_string_ruleset_readback_is_not_treated_as_a_hash(self):
        """GetRuleSet returns a type name, not the numeric hash used by other axes.

        The old generic lookup indexed ``GameInfo.Rulesets`` with that string.
        The Lua error was swallowed by ``try``, so a valid live game reported
        ``?`` and was rejected as an unconfigured ruleset.
        """
        lua = (Path(__file__).resolve().parent / "civ6_control" / "mod"
               / "CivvisControlAgent.lua").read_text(encoding="utf-8")
        self.assertIn('if type(hash) == "string" then return hash; end', lua)

    def test_an_unreadable_ruleset_is_unverified_not_a_mismatch(self):
        """★★★★★ `?` MEANS THE READBACK FAILED, NOT THAT THE RULESET DIFFERED.

        Three complete games were thrown away on 2026-08-18 because the two were
        the same answer: `civvis-20260818T032030Z` (223 turns, score 937, ended
        on a rival's VICTORY_CULTURE), `040903Z` (250 turns, score 1138, lead
        -24) and `045332Z` (250 turns, score 683). Every other axis of their
        seat events was correct.
        """
        for absent in (None, "?"):
            with self.subTest(absent=absent):
                configured, modes, ruleset = civ6_play.seat_matches_requested(
                    self._seat(ruleset=absent), args())
                self.assertIsNone(
                    ruleset, "an unreadable readback is neither agreement nor "
                             "disagreement")
                self.assertTrue(
                    configured,
                    "a seat correct on every axis it could report is the game "
                    "that was asked for")
                self.assertTrue(modes, "the modes axis is unaffected")

    def test_a_different_ruleset_is_still_a_mismatch(self):
        """The weakening must stop at unreadable. A game that reports a ruleset
        CIVVIS does not model is still a different game."""
        configured, _, ruleset = civ6_play.seat_matches_requested(
            self._seat(ruleset="RULESET_STANDARD"), args())
        self.assertIs(ruleset, False)
        self.assertFalse(configured)

    def test_a_wrong_ruleset_run_is_a_refusal_not_a_result(self):
        """A ruleset the game reported and that differs still takes the column:
        the run never played the game being measured."""
        self.assertEqual(
            civ6_play.summary_reason(
                {"ruleset_match": False, "mode_mismatch": False,
                 "seat": {"civ": "CIVILIZATION_ROME"}, "configured": False},
                "stopped"),
            "wrong_ruleset")
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        self.assertIn('"ruleset_requested": args.ruleset,', source)

    def test_an_unreadable_ruleset_does_not_overwrite_the_real_ending(self):
        """⚠ `reason` IS THE ONLY FIELD SAYING HOW A GAME ENDED, and
        `wrong_ruleset` sat first in the precedence chain, so an unreadable
        readback erased it. `civvis-20260818T032030Z` ended on a rival's culture
        victory at turn 223 and the ledger recorded a refusal."""
        state = {"ruleset_match": None, "mode_mismatch": False,
                 "seat": {"ruleset": "?"}, "configured": True}
        for ending in ("stopped", "stalled: no event for 240s", "timeout"):
            with self.subTest(ending=ending):
                self.assertEqual(civ6_play.summary_reason(state, ending), ending)

    def test_the_seat_report_still_decides_the_refusals_it_can_prove(self):
        """Deleting one false refusal must not delete the true ones."""
        self.assertEqual(
            civ6_play.summary_reason(
                {"mode_mismatch": True, "seat": {}, "configured": False},
                "stopped"),
            "wrong_game_modes")
        self.assertEqual(
            civ6_play.summary_reason(
                {"seat": {"difficulty": "DIFFICULTY_PRINCE"}, "configured": False},
                "stopped"),
            "wrong_game_configuration")


class VictoryLaneListTests(unittest.TestCase):
    """The launchers' objective list against the engine's own.

    ⚠ THIS IS THE TEST THAT WOULD HAVE CAUGHT THE ORIGINAL BUG. The Python
    launchers offered four objectives while `VictoryTarget` had six, and
    `advanced.rs` gates each lane's machinery on being targeted at it — so
    Culture, Religion and Diplomacy were not merely missing from a menu, they
    were unplayable in the live seat. Nothing failed, because no test compared
    the two lists.
    """

    REPO = Path(__file__).resolve().parent.parent

    def _rust_source(self, relative):
        return (self.REPO / relative).read_text(encoding="utf-8")

    def test_the_launcher_list_matches_the_orders_binary(self):
        source = self._rust_source("src/bin/civvis_orders.rs")
        match = re.search(r'const VICTORY_LANES: &str = "([^"]+)";', source)
        self.assertIsNotNone(match, "civvis_orders.rs no longer names its lanes")
        self.assertEqual(match.group(1).split("|"), civ6_play.VICTORY_LANES)

    def test_every_direct_or_high_level_default_uses_one_launcher_value(self):
        """A bare `civvis_orders` launch must use the launch chain's current
        central default. The direct fallback is reachable from manual and
        recovery paths, so it is part of the live controller contract rather
        than merely a CLI convenience."""
        binary = self._rust_source("src/bin/civvis_orders.rs")
        match = re.search(r'const DEFAULT_VICTORY: &str = "([^"]+)";', binary)
        self.assertIsNotNone(match, "civvis_orders.rs has no named default")
        self.assertEqual(match.group(1), civ6_play.DEFAULT_CIVVIS_VICTORY)
        self.assertEqual(match.group(1), "diplomatic")
        self.assertIn(
            "unwrap_or_else(|| DEFAULT_VICTORY.to_string())",
            binary,
            "the direct invocation must use its named default rather than a detached literal",
        )

        here = Path(__file__).resolve().parent
        for launcher in ("civ6_civvis_climb.py", "civ6_brain.py"):
            with self.subTest(launcher=launcher):
                source = (here / launcher).read_text(encoding="utf-8")
                self.assertRegex(
                    source,
                    r"from civ6_play import [^\n]*DEFAULT_CIVVIS_VICTORY as DEFAULT_VICTORY",
                    f"{launcher} must import the central default",
                )
                self.assertNotRegex(
                    source,
                    re.compile(r"^DEFAULT_VICTORY\s*=\s*['\"]", re.MULTILINE),
                    f"{launcher} declared a second default",
                )

    def test_the_launcher_list_matches_the_engine_enum(self):
        """Every `VictoryTarget` variant, spelled the way the enum prints it."""
        source = self._rust_source("src/ai/advanced.rs")
        # Anchored on the impl block, not on `as_str`: several enums in this file
        # have an `as_str`, and the first one is `WarPhase`'s.
        block = re.search(r"\nimpl VictoryTarget \{(.*?)\n\}\n", source, re.DOTALL)
        self.assertIsNotNone(block, "impl VictoryTarget no longer parses")
        spellings = re.findall(r'VictoryTarget::\w+ => "(\w+)"', block.group(1))
        self.assertEqual(len(spellings), 6, spellings)
        # `civvis` is not a target — it is the absence of one — so it leads the
        # list and is not expected among the enum's spellings.
        self.assertEqual(civ6_play.VICTORY_LANES[0], "civvis")
        self.assertEqual(sorted(civ6_play.VICTORY_LANES[1:]), sorted(spellings))

    def test_no_launcher_in_the_chain_keeps_its_own_copy(self):
        """`climb --victory` → `play --civvis-victory` → `brain --victory` →
        `civvis_orders --victory`. A lane has to survive all four; the two
        launchers in the middle each used to restate the names, and the middle
        one is where Culture, Religion and Diplomacy were actually refused."""
        here = Path(__file__).resolve().parent
        for launcher in ("civ6_civvis_climb.py", "civ6_brain.py"):
            with self.subTest(launcher=launcher):
                source = (here / launcher).read_text(encoding="utf-8")
                # The fact, not one spelling of it: the name has to arrive from
                # `civ6_play`, and the launcher must not declare a list itself.
                self.assertRegex(
                    source,
                    r"from civ6_play import [^\n]*\bVICTORY_LANES\b",
                    f"{launcher} no longer imports the lane list",
                )
                self.assertIn("choices=VICTORY_LANES", source)
                self.assertNotRegex(
                    source,
                    re.compile(r"^VICTORY_LANES\s*=", re.MULTILINE),
                    f"{launcher} declared its own copy of the lane list",
                )
                # ⚠ THE SAME CLASS, ONE LEVEL DOWN. The list was collapsed here
                # in #1871 and the DEFAULT was not: all three launchers declared
                # `science` while `tools/ops/` held a fourth value that disagreed.
                # `test_ops_ladder_objective.py` owns that fact; this only stops a
                # launcher growing a literal of its own again.
                self.assertNotRegex(
                    source,
                    re.compile(r"^DEFAULT_(CIVVIS_)?VICTORY\s*=\s*[\"']",
                               re.MULTILINE),
                    f"{launcher} declared its own default objective",
                )


if __name__ == "__main__":
    unittest.main()
