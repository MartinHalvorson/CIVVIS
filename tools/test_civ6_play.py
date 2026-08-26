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


class TermTakesTheBrainWithIt(unittest.TestCase):
    """A TERMed harness must not leak the brain that blocks the next game.

    `civ6_play` cleans up through `atexit.register(stop_brain)`, and **atexit
    does not run on SIGTERM** — CPython's default disposition terminates the
    process outright. TERM is the ordinary way this harness is stopped (the
    supervisor's teardown sends it), so every such stop leaked a brain.

    That is not a tidy-up nicety: the climb's `busy()` counts any live
    `civ6_brain.py` as a running game, so the next attempt dies on "something
    already holds the game; refusing to stop an unowned run" while the lock file
    is empty and Civilization VI is down. Measured 2026-08-19 — an orphan sat
    for 29 minutes and failed every launch in that window.
    """

    SCRIPT = (
        "import atexit, os, signal, sys\n"
        "marker = sys.argv[1]\n"
        "atexit.register(lambda: open(marker, 'w').write('cleaned'))\n"
        "{handler}"
        "os.kill(os.getpid(), signal.SIGTERM)\n"
        "import time; time.sleep(5)\n"
    )

    def _run(self, handler: str) -> bool:
        """Did the atexit cleanup run before the process died to SIGTERM?"""
        import subprocess
        with tempfile.TemporaryDirectory() as tmp:
            marker = os.path.join(tmp, "marker")
            subprocess.run(
                [sys.executable, "-c", self.SCRIPT.format(handler=handler), marker],
                capture_output=True, timeout=30,
            )
            return os.path.exists(marker)

    def test_the_default_disposition_skips_atexit(self):
        """The bug, demonstrated: without a handler the cleanup never runs."""
        self.assertFalse(
            self._run(""),
            "if atexit ran on a default SIGTERM this fix would be unnecessary",
        )

    def test_raising_systemexit_from_the_handler_runs_atexit(self):
        """The fix: SystemExit returns to the normal shutdown path."""
        handler = ("signal.signal(signal.SIGTERM,\n"
                   "  lambda s, f: (_ for _ in ()).throw(SystemExit(128 + s)))\n")
        self.assertTrue(
            self._run(handler),
            "the handler must let atexit — and so stop_brain — run",
        )

    def test_term_also_runs_the_finally_that_stops_the_game_and_frees_the_lock(self):
        """The orphan and the stale lock are ONE bug, and this fix clears both.

        `main` is `try: return _play(args) finally: launcher.stop();
        gamelock.release()`. A default SIGTERM skips that `finally` as surely as
        it skips atexit, so a TERMed harness left Civilization VI advancing AND
        the game lock held AND a brain running. That pair is what blocked the
        16:46 and 19:00 starts on 2026-08-19: "another run holds the game" with
        nothing actually playing.

        SystemExit is an exception, so it unwinds through `finally` first and
        reaches atexit after — game stopped, lock released, brain stopped, in
        that order. (Raised by the sibling session running the same ladder.)
        """
        import subprocess
        script = (
            "import atexit, os, signal, sys\n"
            "d = sys.argv[1]\n"
            "signal.signal(signal.SIGTERM,\n"
            "  lambda s, f: (_ for _ in ()).throw(SystemExit(128 + s)))\n"
            "atexit.register(lambda: open(os.path.join(d, 'brain'), 'w').write('stopped'))\n"
            "try:\n"
            "    os.kill(os.getpid(), signal.SIGTERM)\n"
            "    import time; time.sleep(5)\n"
            "finally:\n"
            "    open(os.path.join(d, 'game'), 'w').write('stopped')\n"
            "    open(os.path.join(d, 'lock'), 'w').write('released')\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            subprocess.run([sys.executable, "-c", script, tmp],
                           capture_output=True, timeout=30)
            for name, what in (("game", "launcher.stop()"),
                               ("lock", "gamelock.release()"),
                               ("brain", "atexit stop_brain")):
                self.assertTrue(
                    os.path.exists(os.path.join(tmp, name)),
                    f"a TERMed harness must still reach {what}")

    def test_the_harness_installs_the_handler(self):
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text()
        self.assertIn("signal.signal(signal.SIGTERM", source,
                      "civ6_play must catch TERM so its brain is stopped")
        self.assertIn("atexit.register(stop_brain)", source,
                      "and the cleanup it returns to must still be registered")


class Civ6PlayTest(unittest.TestCase):
    def setUp(self) -> None:
        # Every production harness is a fresh process; reset its equivalent
        # cache between unit tests so each geometry contract observes its own
        # mocked display result.
        civ6_play._desktop_size_cache = None

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

    def test_desktop_size_reuses_one_verified_measurement_for_the_run(self) -> None:
        """Setup OCR must not repeatedly start a slow AppKit interpreter."""
        with patch.object(civ6_play.subprocess, "run",
                          return_value=SimpleNamespace(stdout="1728,1117", returncode=0)) as run:
            self.assertEqual(civ6_play.desktop_size(), (1728, 1117))
            self.assertEqual(civ6_play.desktop_size(), (1728, 1117))
        run.assert_called_once()

    def test_desktop_size_refuses_an_unreadable_answer(self) -> None:
        """None means 'leave the window alone' — never a guess."""
        for bad in ("", "not,numbers", "1728", "0,0"):
            with patch.object(civ6_play.subprocess, "run",
                              return_value=SimpleNamespace(stdout=bad, returncode=0)):
                self.assertIsNone(civ6_play.desktop_size(), f"{bad!r} must be refused")

    def test_place_game_sizes_before_positioning_the_upper_quadrant(self) -> None:
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play, "game_window", return_value=None), \
             patch.object(civ6_play.subprocess, "run") as run:
            civ6_play.place_game("right", 0.5, 0.5)

        script = run.call_args.args[0][-1]
        self.assertLess(script.index("set size"), script.index("set position"))
        self.assertIn("set size to {756, 480}", script)
        self.assertIn("set position to {756, 33}", script)

    def test_place_game_does_not_rewrite_an_unchanged_frame(self) -> None:
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play, "game_window", return_value=(756, 33, 756, 480)), \
             patch.object(civ6_play.subprocess, "run") as run:
            civ6_play.place_game("right", 0.5, 0.5)

        run.assert_not_called()

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
            (100, 33, 756, 480), "difficulty", "DIFFICULTY_SETTLER", Path(temporary),
            panel=None, panel_out=mock.ANY,
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
        # Every row is handed the capture the previous row proved on (the mocks
        # prove on nothing, so it stays None) and the same dict to prove into.
        self.assertEqual(
            setter.call_args_list,
            [
                call((100, 33, 756, 480), "difficulty", "DIFFICULTY_SETTLER", Path(temporary),
                     panel=None, panel_out=mock.ANY),
                call((100, 33, 756, 480), "map_size", "MAPSIZE_SMALL", Path(temporary),
                     panel=None, panel_out=mock.ANY),
                call((100, 33, 756, 480), "speed", "GAMESPEED_ONLINE", Path(temporary),
                     panel=None, panel_out=mock.ANY),
            ],
        )
        shared = setter.call_args_list[0].kwargs["panel_out"]
        self.assertTrue(all(c.kwargs["panel_out"] is shared for c in setter.call_args_list))
        leader.assert_called_once_with(
            (100, 33, 756, 480), "LEADER_TRAJAN", Path(temporary),
            panel=None, panel_out=shared, hint_dir=Path(temporary).parent,
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

    def test_load_action_retries_with_its_enlarged_bottom_strip(self) -> None:
        observation = {
            "text": "Load Game", "x": 0.70, "y": 0.49,
            "width": 0.04, "height": 0.02,
        }
        bounds = (756, 33, 756, 480)
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play, "recognize_once", return_value=[]), \
             patch.object(civ6_play, "_menu_crop_ocr",
                          side_effect=[[], [observation]]) as crop:
            points = civ6_play._observed_label_points(
                Path("load-selected.png"), "Load Game", bounds,
                strip=civ6_play.LOAD_GAME_ACTION_STRIP,
            )

        self.assertEqual(points, [(1088, 491)])
        self.assertEqual(
            crop.call_args_list,
            [call(Path("load-selected.png"), bounds),
             call(Path("load-selected.png"), bounds,
                  strip=civ6_play.LOAD_GAME_ACTION_STRIP, tag="strip")],
        )

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
            self.assertEqual(civ6_play.recent_autosaves(folder), [newer, older])
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


class DialogueCloseConfigTests(unittest.TestCase):
    @staticmethod
    def _config(dialogue_seconds):
        class Defaults(SimpleNamespace):
            def __getattr__(self, name):
                return None

        return civ6_play.build_config(
            Defaults(tag="t", game_mode=[], dialogue_seconds=dialogue_seconds,
                     difficulty="DIFFICULTY_SETTLER", map_size="MAPSIZE_SMALL",
                     speed="GAMESPEED_ONLINE", map="Continents.lua",
                     leader="LEADER_TRAJAN"))

    def test_dialogue_timer_reaches_the_baked_config(self):
        self.assertEqual(self._config(0.25)["DialogueSeconds"], 0.25)

    def test_dialogue_timer_cannot_be_configured_past_two_seconds(self):
        self.assertEqual(self._config(9.0)["DialogueSeconds"], 2.0)
        self.assertEqual(self._config(-1.0)["DialogueSeconds"], 0.0)

    def test_the_shim_waits_for_fade_in_without_spending_close_attempts(self):
        lua = (Path(__file__).resolve().parent / "civ6_control" / "mod"
               / "CivvisControlAutoClose.lua").read_text()
        launcher = (Path(__file__).resolve().parent / "civ6_play.py").read_text()
        self.assertIn('"--dialogue-seconds", type=float, default=0.25', launcher)
        self.assertIn("local MAX_DIALOGUE_SECONDS = 2.0;", lua)
        self.assertIn("local DIALOGUE_READY_RETRY_SECONDS = 0.05;", lua)
        self.assertIn("Controls.BlackFadeAnim:IsStopped()", lua)
        self.assertIn("Controls.TradePanelFade:IsStopped()", lua)
        self.assertIn("and not dialogueReady()", lua)
        self.assertIn(
            "math.min(SECONDS, math.max(0, DIALOGUE_SECONDS), MAX_DIALOGUE_SECONDS)",
            lua,
        )


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


class SettlerEscortCapSyncConfigTests(unittest.TestCase):
    """The host protects an unambiguously stacked settler by default."""

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

    def test_the_safe_default_and_explicit_opt_out_reach_the_mod(self) -> None:
        self.assertIs(self._config(settler_escort_cap_sync=False)
                      ["SettlerEscortCapSync"], False)
        self.assertIs(self._config(settler_escort_cap_sync=True)
                      ["SettlerEscortCapSync"], True)

    def test_the_cli_declares_a_safe_default_and_preserves_the_opt_out(self) -> None:
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text()
        self.assertIn('ap.add_argument("--settler-escort-cap-sync", dest="settler_escort_cap_sync",\n'
                      '                    action="store_true", default=True,', source)
        self.assertIn('ap.add_argument("--no-settler-escort-cap-sync", dest="settler_escort_cap_sync",\n'
                      '                    action="store_false",', source)


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
        self.assertEqual(run.call_count, len(civ6_play.SHOT_BACKOFF_SECONDS) + 1,
                         "every backoff step is spent before giving up")
        self.assertEqual([call.args[0] for call in sleep.call_args_list],
                         list(civ6_play.SHOT_BACKOFF_SECONDS),
                         "and it waits longer each time")

    def test_the_capture_rides_out_a_spike_that_outlasts_one_retry(self) -> None:
        """The 2026-08-19 failure: two captures a second apart sample one spike
        twice, and the launch dies. A shot that lands on the third attempt is
        the whole point of the backoff."""
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play.time, "sleep"):
            path = Path(temporary) / "late.png"
            attempts = []

            def spike(cmd, **_):
                attempts.append(1)
                if len(attempts) >= 3:
                    path.write_bytes(b"x")

            with patch.object(civ6_play.subprocess, "run", side_effect=spike):
                self.assertTrue(civ6_play.screenshot(path),
                                "the shot that lands late still counts")
        self.assertEqual(len(attempts), 3)

    def test_the_backoff_escalates_and_is_bounded(self) -> None:
        """Escalating, because a flat retry samples one spike twice; bounded,
        because a poll that sleeps forever is worse than an unreadable one."""
        steps = civ6_play.SHOT_BACKOFF_SECONDS
        self.assertEqual(list(steps), sorted(steps), "each wait is longer")
        self.assertLess(sum(steps), 10.0, "and the whole schedule stays short")

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

    def test_menu_label_probe_survives_an_unavailable_ocr(self) -> None:
        """A zero-dimensioned menu shot must consume a poll, not the attempt."""
        civ6_play._OCR_CACHE.clear()
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play, "_menu_crop_ocr", return_value=[]), \
             patch.object(
                 civ6_play.macos_ocr, "recognize",
                 side_effect=civ6_play.macos_ocr.OCRUnavailable("0 x 0")) as recognize:
            point = civ6_play._observed_label_point(
                Path("zero-sized-menu.png"), "Single Player", (756, 33, 756, 480))
        self.assertIsNone(point)
        recognize.assert_called_once_with(Path("zero-sized-menu.png"))

    def test_main_menu_visibility_survives_an_unavailable_ocr(self) -> None:
        civ6_play._OCR_CACHE.clear()
        with patch.object(
                civ6_play.macos_ocr, "recognize",
                side_effect=civ6_play.macos_ocr.OCRUnavailable("0 x 0")):
            visible = civ6_play._main_menu_visible(Path("zero-sized-menu.png"))
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
            "civvis_with": [], "civvis_without": [], "timeout": 7200.0,
        }
        values.update(changes)
        return SimpleNamespace(**values)

    def _cmd(self, **changes):
        return civ6_play.supervised_brain_command(
            self._args(**changes), Path("/tmp/run"),
            Path("/tmp/orders.sqlite"), Path("/tmp/civvis_orders"))

    def test_the_full_bundle_withholds_nothing(self) -> None:
        self.assertNotIn("--without", self._cmd())
        self.assertNotIn("--with", self._cmd())

    def test_each_forced_ledger_treatment_reaches_the_decider(self) -> None:
        cmd = self._cmd(civvis_with=["stacked-escort", "settler-stack-discipline"])
        pairs = [(cmd[i], cmd[i + 1]) for i, tok in enumerate(cmd)
                 if tok == "--with"]
        self.assertEqual(
            pairs,
            [("--with", "stacked-escort"),
             ("--with", "settler-stack-discipline")],
            "each force-on treatment needs its own flag for an attributable arm",
        )

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
            "civvis_with": [],
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



class OpeningRestartPoliciesAreEnforcedBeforeTheGameCanDrift(unittest.TestCase):
    """The operator's opening rules use ledger evidence, never a screen guess."""

    @staticmethod
    def _turn(turn, cities, ctx="agent"):
        return {"kind": "turn", "ctx": ctx, "turn": turn, "cities": cities}

    @staticmethod
    def _state(turn, settlers, ctx="agent"):
        return {
            "kind": "state", "ctx": ctx, "turn": turn,
            "units": [{"id": unit_id, "kind": "UNIT_SETTLER"}
                      for unit_id in settlers],
        }

    @staticmethod
    def _found(turn, unit, ctx="agent"):
        return {"kind": "found", "ctx": ctx, "turn": turn, "unit": unit}

    @staticmethod
    def _lost(turn, unit, kind="UNIT_SETTLER", ctx="agent"):
        return {
            "kind": "unit_lost", "ctx": ctx, "turn": turn,
            "unit": unit, "unit_kind": kind,
        }

    def test_three_cities_are_required_on_the_first_readable_turn_at_deadline(self):
        state = {}
        self.assertIsNone(civ6_play.opening_city_target_reading(
            state, self._turn(31, 1)))
        self.assertNotIn("opening_city_target_checked", state)
        verdict = civ6_play.opening_city_target_reading(state, self._turn(32, 2))
        self.assertEqual(verdict, {
            "rule": "three_cities_by_turn_32", "turn": 32, "cities": 2,
            "required_cities": 3, "deadline_turn": 32,
        })
        # It is a one-shot decision, not a duplicate restart print every poll.
        self.assertIsNone(civ6_play.opening_city_target_reading(
            state, self._turn(33, 1)))

    def test_city_target_does_not_treat_missing_or_invalid_count_as_a_failure(self):
        state = {}
        self.assertIsNone(civ6_play.opening_city_target_reading(
            state, self._turn(32, None)))
        self.assertNotIn("opening_city_target_checked", state)
        self.assertIsNone(civ6_play.opening_city_target_reading(
            state, self._turn(33, True)))
        self.assertNotIn("opening_city_target_checked", state)
        verdict = civ6_play.opening_city_target_reading(state, self._turn(34, 1))
        self.assertEqual((verdict["turn"], verdict["cities"]), (34, 1))

    def test_city_target_accepts_three_or_more_cities_and_ignores_other_contexts(self):
        state = {}
        self.assertIsNone(civ6_play.opening_city_target_reading(
            state, self._turn(32, 2, ctx="spectator")))
        self.assertNotIn("opening_city_target_checked", state)
        self.assertIsNone(civ6_play.opening_city_target_reading(
            state, self._turn(32, 3)))
        self.assertTrue(state["opening_city_target_checked"])

    def test_only_the_settler_after_the_capital_settler_is_tracked(self):
        state = {}
        civ6_play.record_opening_settlers(state, self._state(1, [10]))
        self.assertEqual(state["initial_settler_id"], 10)
        civ6_play.record_opening_settlers(state, self._found(1, 10))
        # Founding consumes the starting settler through `unit_lost`; it must
        # remain a successful opening, not look like a capture.
        self.assertIsNone(civ6_play.second_settler_loss_reading(
            state, self._lost(1, 10)))
        civ6_play.record_opening_settlers(state, self._state(18, [11]))
        self.assertEqual(state["second_settler_id"], 11)
        verdict = civ6_play.second_settler_loss_reading(state, self._lost(20, 11))
        self.assertEqual(verdict, {
            "rule": "second_settler_captured", "turn": 20, "unit": 11,
            "unit_kind": "UNIT_SETTLER",
        })

    def test_a_second_settler_that_founds_is_not_restarted(self):
        state = {}
        civ6_play.record_opening_settlers(state, self._state(1, [10]))
        civ6_play.record_opening_settlers(state, self._found(1, 10))
        civ6_play.record_opening_settlers(state, self._state(18, [11]))
        civ6_play.record_opening_settlers(state, self._found(20, 11))
        self.assertIsNone(civ6_play.second_settler_loss_reading(
            state, self._lost(20, 11)))

    def test_settler_rule_requires_the_tracked_agent_settler(self):
        state = {}
        civ6_play.record_opening_settlers(state, self._state(1, [10]))
        civ6_play.record_opening_settlers(state, self._found(1, 10))
        civ6_play.record_opening_settlers(state, self._state(18, [11]))
        self.assertIsNone(civ6_play.second_settler_loss_reading(
            state, self._lost(20, 12)))
        self.assertIsNone(civ6_play.second_settler_loss_reading(
            state, self._lost(20, 11, kind="UNIT_WARRIOR")))
        self.assertIsNone(civ6_play.second_settler_loss_reading(
            state, self._lost(20, 11, ctx="spectator")))

    def test_the_live_loop_records_and_enforces_both_opening_policies(self):
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        self.assertIn("record_opening_settlers(state, event)", source)
        self.assertIn("opening_city_target_reading(state, event)", source)
        self.assertIn("second_settler_loss_reading(state, event)", source)
        self.assertIn('"rule": "three_cities_by_turn_32"', source)
        self.assertIn('"rule": "second_settler_captured"', source)


class AnAbandonedGameIsOneTheLadderChoseNotToPlayOut(unittest.TestCase):
    """Operator request 2026-08-19: "ok to abandon games early if expected win
    rate <5%". The rule is a measured table (`ABANDON_CELLS`), the estimate is
    a Laplace rate so thin evidence never clears the floor, patience guards a
    one-turn dip, and the ending is filed as its own reason. The fit script, so
    the table can be re-measured when the ladder climbs::

        for each run with summary.json + events.jsonl that reached a terminal
        result (victory, our defeat, or `stopped` at max_turns): walk the agent
        `turn` events; a cell (T, R) FIRES on the first turn >= T whose
        score/rival_best < R for five consecutive turns; count fired runs and
        the wins among them. 2026-08-19, 48 runs, 7 wins: (100, 0.60) 0/25,
        (120, 0.75) 0/34; the wins' low-water marks after t120 were 0.87 and
        0.88. (120, 0.90) also read 0/38 and was NOT taken — a tenth above a
        real comeback is no margin at all.
    """

    def test_the_estimate_is_the_laplace_rate_of_the_best_evidenced_cell(self):
        # (120, 0.75): 0 of 34 → 1/36
        self.assertAlmostEqual(civ6_play.expected_win_rate(150, 300, 500), 1 / 36)
        # (100, 0.60) alone: 0 of 25 → 1/27
        self.assertAlmostEqual(civ6_play.expected_win_rate(105, 290, 500), 1 / 27)
        # both match: the thinnest (best-evidenced zero) decides
        self.assertAlmostEqual(civ6_play.expected_win_rate(130, 290, 500), 1 / 36)

    def test_the_table_does_not_speak_where_it_counted_nothing(self):
        # level, ahead, or early: no cell, no estimate — and never an abandon
        self.assertIsNone(civ6_play.expected_win_rate(150, 500, 500))
        self.assertIsNone(civ6_play.expected_win_rate(150, 400, 500))   # 0.80
        self.assertIsNone(civ6_play.expected_win_rate(90, 100, 500))    # early
        # unreadable standing: the mirror has not reported a rival yet
        self.assertIsNone(civ6_play.expected_win_rate(150, 100, None))
        self.assertIsNone(civ6_play.expected_win_rate(150, None, 500))
        self.assertIsNone(civ6_play.expected_win_rate(150, 100, 0))

    def test_every_cell_clears_the_operators_floor_on_its_own_evidence(self):
        """A cell that could not put its own Laplace rate under 5% would fire
        on nothing but thin evidence; refuse to carry one."""
        for floor, ceiling, wins, games in civ6_play.ABANDON_CELLS:
            with self.subTest(cell=(floor, ceiling)):
                self.assertLess((wins + 1) / (games + 2), 0.05)
                self.assertGreaterEqual(games, 20)

    def _turn(self, turn, score, rival, ctx="agent"):
        return {"kind": "turn", "ctx": ctx, "turn": turn, "score": score,
                "rival_best": rival}

    def test_five_consecutive_hopeless_turns_abandon_and_one_recovery_resets(self):
        state = {}
        for turn in range(120, 124):
            self.assertIsNone(
                civ6_play.abandon_reading(state, self._turn(turn, 300, 500), 0.05))
        # a fifth: the verdict, carrying what it saw
        verdict = civ6_play.abandon_reading(state, self._turn(124, 300, 500), 0.05)
        self.assertEqual(verdict["turn"], 124)
        self.assertEqual(verdict["consecutive_turns"], 5)
        self.assertEqual((verdict["score"], verdict["rival_best"]), (300, 500))
        self.assertEqual(verdict["floor"], 0.05)
        self.assertAlmostEqual(verdict["expected_win_rate"], round(1 / 36, 4))
        # a readable turn back over the floor resets the count
        state = {}
        for turn in range(120, 124):
            civ6_play.abandon_reading(state, self._turn(turn, 300, 500), 0.05)
        self.assertIsNone(
            civ6_play.abandon_reading(state, self._turn(124, 450, 500), 0.05))
        self.assertEqual(state["abandon_streak"], 0)
        self.assertIsNone(
            civ6_play.abandon_reading(state, self._turn(125, 300, 500), 0.05))

    def test_a_repeated_turn_counts_once_and_silence_is_not_recovery(self):
        state = {}
        for _ in range(5):   # the agent re-reports one turn five times
            self.assertIsNone(
                civ6_play.abandon_reading(state, self._turn(130, 300, 500), 0.05))
        self.assertEqual(state["abandon_streak"], 1)
        # a turn with no standing neither counts nor resets
        self.assertIsNone(
            civ6_play.abandon_reading(state, self._turn(131, 300, None), 0.05))
        self.assertEqual(state["abandon_streak"], 1)
        # a non-agent context is ignored entirely
        self.assertIsNone(
            civ6_play.abandon_reading(state, self._turn(132, 300, 500, ctx="x"), 0.05))
        self.assertEqual(state["abandon_streak"], 1)

    def test_no_floor_means_every_game_is_played_out(self):
        state = {}
        for turn in range(120, 140):
            self.assertIsNone(
                civ6_play.abandon_reading(state, self._turn(turn, 100, 500), 0.0))
        self.assertNotIn("abandon_streak", state)

    def test_an_abandoned_game_is_filed_as_abandoned_and_nothing_else_is(self):
        """`reason` is the only field saying how a game ended. The harness's
        own stop takes it; a game that exited or stalled in the same poll keeps
        that ending; a refusal still outranks everything."""
        abandoned = {"turn": 124, "expected_win_rate": 0.0278, "floor": 0.05}
        state = {"abandoned": abandoned, "seat": {"x": 1}, "configured": True,
                 "ruleset_match": True, "mode_mismatch": False}
        self.assertEqual(civ6_play.summary_reason(state, "stopped"), "abandoned")
        self.assertEqual(civ6_play.summary_reason(state, "game exited"),
                         "game exited")
        self.assertEqual(civ6_play.summary_reason(state, "stalled: no event for 240s"),
                         "stalled: no event for 240s")
        state["ruleset_match"] = False
        self.assertEqual(civ6_play.summary_reason(state, "stopped"), "wrong_ruleset")
        clean = {"seat": {"x": 1}, "configured": True, "ruleset_match": True,
                 "mode_mismatch": False}
        self.assertEqual(civ6_play.summary_reason(clean, "stopped"), "stopped")

    def test_the_flag_and_the_summary_field_exist(self):
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        self.assertIn('"--abandon-below-win-rate"', source)
        self.assertIn('"--restart-below-leader-ratio"', source)
        self.assertIn('"abandoned": state.get("abandoned"),', source)
        self.assertIn("abandon_reading(state, event, args.abandon_below_win_rate)",
                      source)
        self.assertIn("behind_all_metrics_reading(", source)
        # The deal lane's tally rides the summary beside the orders totals.
        self.assertIn('deals = civ6_ladder.deal_totals(run_dir / "events.jsonl")', source)
        self.assertIn('summary["deals"] = deals', source)


class AThreeSignalRestartDoesNotTreatScoreAsEnough(unittest.TestCase):
    """The operator's 70 % rule must lose on every named axis, consecutively."""

    @staticmethod
    def _state(turn, science, culture, rivals, ctx="agent"):
        return {"kind": "state", "ctx": ctx, "turn": turn,
                "science": science, "culture": culture, "rivals": rivals}

    @staticmethod
    def _turn(turn, score, rival_best, ctx="agent"):
        return {"kind": "turn", "ctx": ctx, "turn": turn,
                "score": score, "rival_best": rival_best}

    def _reading(self, state, turn, score=69, rival_best=100,
                 science=9, culture=8, rivals=None):
        if rivals is None:
            rivals = [{"science": 10, "culture": 10}]
        self.assertIsNone(civ6_play.behind_all_metrics_reading(
            state, self._state(turn, science, culture, rivals), 0.70))
        return civ6_play.behind_all_metrics_reading(
            state, self._turn(turn, score, rival_best), 0.70)

    def test_score_science_and_culture_must_all_be_deficits(self):
        for label, values in (
            ("score at ceiling", {"score": 70}),
            ("science tied", {"science": 10}),
            ("culture tied", {"culture": 10}),
        ):
            with self.subTest(label=label):
                state = {}
                self.assertIsNone(self._reading(state, 100, **values))
                self.assertEqual(state["behind_all_metrics_streak"], 0)

    def test_five_current_readings_fire_and_a_recovery_resets(self):
        state = {}
        rivals = [{"science": 8, "culture": 10},
                  {"science": 10, "culture": 8}]
        for turn in range(100, 104):
            self.assertIsNone(self._reading(state, turn, rivals=rivals))
        verdict = self._reading(state, 104, rivals=rivals)
        self.assertEqual(verdict["rule"], "score_science_culture_deficit")
        self.assertEqual(verdict["consecutive_turns"], 5)
        self.assertAlmostEqual(verdict["score_ratio"], 0.69)
        self.assertEqual((verdict["rival_best_science"],
                          verdict["rival_best_culture"]), (10, 10))
        # A current state sample that is no longer behind on culture resets it.
        self.assertIsNone(self._reading(state, 105, culture=10, rivals=rivals))
        self.assertEqual(state["behind_all_metrics_streak"], 0)
        self.assertIsNone(self._reading(state, 106, rivals=rivals))
        self.assertEqual(state["behind_all_metrics_streak"], 1)

    def test_stale_or_unreadable_standings_never_count(self):
        state = {}
        self.assertIsNone(civ6_play.behind_all_metrics_reading(
            state, self._state(99, 9, 8, [{"science": 10, "culture": 10}]), 0.70))
        self.assertIsNone(civ6_play.behind_all_metrics_reading(
            state, self._turn(100, 69, 100), 0.70))
        self.assertNotIn("behind_all_metrics_streak", state)
        self.assertIsNone(self._reading(
            state, 101, rivals=[{"science": -1, "culture": -1}]))
        self.assertNotIn("behind_all_metrics_streak", state)
        self.assertIsNone(self._reading(state, 102))
        self.assertEqual(state["behind_all_metrics_streak"], 1)
        # Disabled remains a complete no-op, including on a fully bad reading.
        disabled = {}
        self.assertIsNone(civ6_play.behind_all_metrics_reading(
            disabled, self._state(102, 9, 8, [{"science": 10, "culture": 10}]), 0.0))
        self.assertIsNone(civ6_play.behind_all_metrics_reading(
            disabled, self._turn(102, 69, 100), 0.0))
        self.assertNotIn("behind_all_metrics_streak", disabled)


class AResumeStagesTheAutosaveWhereTheListShowsIt(unittest.TestCase):
    """★ The Load Game list opens on the manual saves and hides the autosave
    rotation behind a filter checkbox the screen reader misses at the operator
    layout's scale. Both freeze-resumes of 2026-08-19 died there, 0 turns each
    — one of them costing a live t139 game at 75 % of the leader. The staged
    copy in ``Saves/Single`` is the row the default list already shows, as the
    manual recovery of 2026-08-16 (``resume-autosave-0189.Civ6Save``) proved."""

    def _dirs(self, base):
        single = Path(base) / "Single"
        (single / "auto").mkdir(parents=True)
        return single

    def test_an_autosave_is_staged_under_the_constant_stem(self):
        with tempfile.TemporaryDirectory() as base:
            single = self._dirs(base)
            source = single / "auto" / "AutoSave_0062.Civ6Save"
            source.write_bytes(b"save-bytes")
            staged = civ6_play.stage_resume_save(source, single_dir=single)
            self.assertEqual(staged, single / "civvis-resume.Civ6Save")
            self.assertEqual(staged.read_bytes(), b"save-bytes")
            # and the source stays where the rotation owns it
            self.assertTrue(source.is_file())

    def test_a_second_resume_overwrites_rather_than_accumulates(self):
        with tempfile.TemporaryDirectory() as base:
            single = self._dirs(base)
            first = single / "auto" / "AutoSave_0010.Civ6Save"
            first.write_bytes(b"one")
            second = single / "auto" / "AutoSave_0020.Civ6Save"
            second.write_bytes(b"two")
            civ6_play.stage_resume_save(first, single_dir=single)
            staged = civ6_play.stage_resume_save(second, single_dir=single)
            self.assertEqual(staged.read_bytes(), b"two")
            saves = [p.name for p in single.iterdir() if p.is_file()]
            self.assertEqual(saves, ["civvis-resume.Civ6Save"])

    def test_a_manual_save_is_the_row_the_caller_meant(self):
        """A --load-save naming a save outside the rotation is not rewritten:
        the operator asked for that exact row."""
        with tempfile.TemporaryDirectory() as base:
            single = self._dirs(base)
            manual = single / "my-regression.Civ6Save"
            manual.write_bytes(b"manual")
            self.assertEqual(
                civ6_play.stage_resume_save(manual, single_dir=single), manual)
            self.assertFalse((single / "civvis-resume.Civ6Save").exists())

    def test_a_failed_copy_falls_back_to_the_filter_path(self):
        """Weak resume beats no resume: the original path keeps the old
        Autosaves-filter attempt in force."""
        with tempfile.TemporaryDirectory() as base:
            single = self._dirs(base)
            source = single / "auto" / "AutoSave_0062.Civ6Save"
            source.write_bytes(b"save-bytes")
            with mock.patch.object(civ6_play.shutil, "copy2",
                                   side_effect=OSError("disk full")):
                self.assertEqual(
                    civ6_play.stage_resume_save(source, single_dir=single),
                    source)

    def test_bootstrap_reads_the_staged_stem_not_the_autosave_name(self):
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        self.assertIn("save_path = stage_resume_save(Path(args.load_save))", source)
        self.assertIn("save_label = save_path.stem", source)
        self.assertNotIn("save_label = Path(args.load_save).stem", source)


if __name__ == "__main__":
    unittest.main()


class TheSetupScreenIsReadOnceAndLookedAtNotSleptThrough(unittest.TestCase):
    """The setup levers of 2026-08-24: no flat sleeps, one OCR pass per capture."""

    def setUp(self) -> None:
        civ6_play._OCR_CACHE.clear()

    def test_poll_screen_returns_the_moment_the_screen_answers(self) -> None:
        clock = {"now": 0.0}
        reads = iter([None, None, ("point", ["a", "b", "c", "d"])])
        slept = []

        def sleep(seconds: float) -> None:
            slept.append(seconds)
            clock["now"] += seconds

        with patch.object(civ6_play.time, "monotonic", lambda: clock["now"]), \
             patch.object(civ6_play.time, "sleep", sleep):
            result = civ6_play._poll_screen(lambda: next(reads), budget_s=20.0, poll_s=3.0)

        self.assertEqual(result, ("point", ["a", "b", "c", "d"]))
        self.assertEqual(slept, [3.0, 3.0])

    def test_poll_screen_gives_up_after_its_budget_without_a_flat_sleep(self) -> None:
        clock = {"now": 0.0}
        looks = []

        def sleep(seconds: float) -> None:
            clock["now"] += seconds

        with patch.object(civ6_play.time, "monotonic", lambda: clock["now"]), \
             patch.object(civ6_play.time, "sleep", sleep):
            result = civ6_play._poll_screen(lambda: looks.append(1), budget_s=20.0, poll_s=3.0)

        self.assertIsNone(result)
        # Looks at 0, 3, 6, 9, 12, 15 and 18 s, and one last look once the
        # budget has run out at 21 s -- eight, where one flat sleep gave one.
        self.assertEqual(len(looks), 8)

    def test_the_bootstrap_polls_the_menu_instead_of_sleeping_twenty_seconds(self) -> None:
        import inspect
        source = inspect.getsource(civ6_play.bootstrap_game)
        self.assertIn("_poll_screen(read_top_menu)", source)
        self.assertIn("_poll_screen(read_submenu)", source)
        # Two flat waits remain and neither is on the path every game takes:
        # no game WINDOW yet (the launcher has already seen the menu in the
        # log by then), and a submenu that read rows but no Create Game label.
        # The unreadable-menu and no-submenu branches -- the ones the ledger's
        # first attempt of every game hit -- sleep nothing after the poll.
        self.assertEqual(source.count("time.sleep(20.0)"), 2)
        unreadable = source.split("refusing a blind menu click")[1].split("click_at(*menu_point)")[0]
        self.assertNotIn("time.sleep(20.0)", unreadable)
        no_submenu = source.split("the menu is not ready yet")[1].split("blind_strikes = 0\n        if len(rows) > 6")[0]
        self.assertNotIn("time.sleep(20.0)", no_submenu)

    def test_a_capture_is_recognized_once_until_it_changes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            shot = Path(temporary) / "panel.png"
            shot.write_bytes(b"first capture")
            with patch.object(civ6_play.macos_ocr, "recognize",
                              return_value=[{"text": "Settler"}]) as recognize:
                first = civ6_play.recognize_once(shot)
                second = civ6_play.recognize_once(shot)
                first.append({"text": "mutated by the caller"})
                third = civ6_play.recognize_once(shot)
                shot.write_bytes(b"a fresh capture under the same name!")
                fourth = civ6_play.recognize_once(shot)

        self.assertEqual(recognize.call_count, 2)
        self.assertEqual(second, [{"text": "Settler"}])
        self.assertEqual(third, [{"text": "Settler"}])
        self.assertEqual(fourth, [{"text": "Settler"}])

    def test_a_missing_capture_is_not_cached_and_still_raises(self) -> None:
        with patch.object(civ6_play.macos_ocr, "recognize",
                          side_effect=OSError("no such file")):
            with self.assertRaises(OSError):
                civ6_play.recognize_once(Path("/nowhere/at/all.png"))
        self.assertEqual(civ6_play._OCR_CACHE, {})

    def test_a_dropdown_starts_from_the_capture_another_row_proved_on(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            proved = Path(temporary) / "dropdown-difficulty-selected.png"
            proved.write_bytes(b"x")
            out: dict = {}
            with patch.object(civ6_play, "screenshot") as screenshot, \
                 patch.object(civ6_play, "_setup_current_value",
                              return_value=("MAPSIZE_SMALL", (10, 20))) as read, \
                 patch.object(civ6_play, "click_at") as click:
                ok = civ6_play.set_dropdown((0, 0, 756, 480), "map_size", "MAPSIZE_SMALL",
                                            Path(temporary), panel=proved, panel_out=out)

        self.assertTrue(ok)
        screenshot.assert_not_called()
        read.assert_called_once_with(proved, (0, 0, 756, 480), "map_size")
        click.assert_not_called()
        self.assertEqual(out["shot"], proved)

    def test_a_selection_is_read_back_twice_before_the_list_is_reopened(self) -> None:
        reads = iter([
            ("GAMESPEED_STANDARD", (700, 300)),   # closed panel: not yet Online
            None,                                  # first readback: list still closing
            ("GAMESPEED_ONLINE", (700, 300)),     # second look: it took
        ])
        out: dict = {}
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "_setup_current_value", side_effect=lambda *a: next(reads)), \
             patch.object(civ6_play, "_observed_label_point", return_value=(700, 340)), \
             patch.object(civ6_play, "click_at") as click, \
             patch.object(civ6_play.time, "sleep"):
            ok = civ6_play.set_dropdown((0, 0, 756, 480), "speed", "GAMESPEED_ONLINE",
                                        Path(temporary), panel_out=out)
            again = Path(temporary) / "dropdown-speed-selected-again.png"

        self.assertTrue(ok)
        # One click on the closed row, one on the option -- the list was NOT reopened.
        self.assertEqual(click.call_args_list, [call(700, 300), call(700, 340)])
        self.assertEqual(screenshot.call_args_list[-1], call(again))
        self.assertEqual(out["shot"], again)

    def test_the_leader_picker_walks_straight_to_where_it_found_the_leader_last_game(self) -> None:
        bounds = (756, 33, 756, 480)
        row = {"text": "Jadwiga", "x": 0.73, "y": 0.30, "width": 0.04, "height": 0.02}
        selected = {"text": "Jadwiga", "x": 0.73, "y": 0.155, "width": 0.04, "height": 0.02}
        with tempfile.TemporaryDirectory() as temporary:
            hint_dir = Path(temporary) / "control"
            hint_dir.mkdir()
            civ6_play.write_leader_hint(hint_dir, "LEADER_JADWIGA", 15)
            run_dir = hint_dir / "run"
            run_dir.mkdir()
            with patch.object(civ6_play, "screenshot",
                              side_effect=lambda p: Path(p).write_bytes(b"x") or True) as shots, \
                 patch.object(civ6_play, "_leader_picker_open", return_value=True), \
                 patch.object(civ6_play, "_setup_current_leader",
                              return_value=("Random Leader", (1134, 140))), \
                 patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
                 patch.object(civ6_play, "_leader_ocr", side_effect=[[row], [selected]]), \
                 patch.object(civ6_play.macos_input, "move"), \
                 patch.object(civ6_play.macos_input, "scroll") as scroll, \
                 patch.object(civ6_play, "click_at") as click, \
                 patch.object(civ6_play.time, "sleep"):
                found = civ6_play.select_requested_leader(bounds, "LEADER_JADWIGA", run_dir,
                                                          hint_dir=hint_dir)

            self.assertTrue(found)
            # One reset, fifteen wheel steps with no photograph, then ONE picker capture.
            self.assertEqual(scroll.call_args_list[0], call(civ6_play.LEADER_SCROLL_RESET))
            self.assertEqual(scroll.call_count, 16)
            picker_shots = [c for c in shots.call_args_list
                            if "leader-picker-" in str(c.args[0]) and "-1" in str(c.args[0])]
            self.assertEqual([Path(c.args[0]).name for c in picker_shots], ["leader-picker-15.png"])
            self.assertEqual(click.call_count, 2)
            self.assertEqual(civ6_play.read_leader_hint(hint_dir, "LEADER_JADWIGA"), 15)

    def test_a_stale_hint_falls_back_to_the_whole_roster(self) -> None:
        bounds = (756, 33, 756, 480)
        row = {"text": "Jadwiga", "x": 0.73, "y": 0.30, "width": 0.04, "height": 0.02}
        selected = {"text": "Jadwiga", "x": 0.73, "y": 0.155, "width": 0.04, "height": 0.02}
        # Three hinted looks miss, then the roster walk finds it at step 2.
        looks = [[], [], [], [], [], [row], [selected]]
        with tempfile.TemporaryDirectory() as temporary:
            hint_dir = Path(temporary)
            civ6_play.write_leader_hint(hint_dir, "LEADER_JADWIGA", 15)
            run_dir = hint_dir / "run"
            run_dir.mkdir()
            with patch.object(civ6_play, "screenshot",
                              side_effect=lambda p: Path(p).write_bytes(b"x") or True), \
                 patch.object(civ6_play, "_leader_picker_open", return_value=True), \
                 patch.object(civ6_play, "_setup_current_leader",
                              return_value=("Random Leader", (1134, 140))), \
                 patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
                 patch.object(civ6_play, "_leader_ocr", side_effect=looks), \
                 patch.object(civ6_play.macos_input, "move"), \
                 patch.object(civ6_play.macos_input, "scroll") as scroll, \
                 patch.object(civ6_play, "click_at"), \
                 patch.object(civ6_play.time, "sleep"):
                found = civ6_play.select_requested_leader(bounds, "LEADER_JADWIGA", run_dir,
                                                          hint_dir=hint_dir)

            self.assertTrue(found)
            resets = [c for c in scroll.call_args_list if c == call(civ6_play.LEADER_SCROLL_RESET)]
            self.assertEqual(len(resets), 2)
            self.assertEqual(civ6_play.read_leader_hint(hint_dir, "LEADER_JADWIGA"), 2)

    def test_without_a_hint_directory_nothing_is_remembered(self) -> None:
        self.assertEqual(civ6_play.read_leader_hint(None, "LEADER_TRAJAN"), 0)
        civ6_play.write_leader_hint(None, "LEADER_TRAJAN", 4)  # must not raise
        with tempfile.TemporaryDirectory() as temporary:
            (Path(temporary) / civ6_play.LEADER_HINT_FILE).write_text("not json")
            self.assertEqual(civ6_play.read_leader_hint(Path(temporary), "LEADER_TRAJAN"), 0)
            civ6_play.write_leader_hint(Path(temporary), "LEADER_TRAJAN", 99)  # out of range
            self.assertEqual(civ6_play.read_leader_hint(Path(temporary), "LEADER_TRAJAN"), 0)
