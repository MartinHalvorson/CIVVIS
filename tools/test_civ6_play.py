#!/usr/bin/env python3
"""Focused setup-contract checks for the live Civ VI launcher."""

from __future__ import annotations

import io
import builtins
import os
import re
import sys
import tempfile
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock
from unittest.mock import call, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_play
from civ6_control import orders  # noqa: E402

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


class AttachRunningTests(unittest.TestCase):
    """A loaded save has an ownership path that never touches its process."""

    def test_attach_mode_bypasses_vision_and_launcher_teardown(self) -> None:
        attached = SimpleNamespace(
            tag="saved-game", lock_wait=0.0, attach_running=True,
            restart_below_leader_ratio=0.0,
        )
        with patch.object(civ6_play, "hold_macos_awake") as awake, \
             patch.object(civ6_play.gamelock, "acquire", return_value=True), \
             patch.object(civ6_play.gamelock, "release") as release, \
             patch.object(civ6_play, "_attach_running_game", return_value=0) as attach, \
             patch.object(civ6_play.vision, "available") as vision, \
             patch.object(civ6_play.launcher, "stop") as stop:
            self.assertEqual(civ6_play.play(attached), 0)

        awake.assert_called_once_with()
        attach.assert_called_once_with(attached)
        release.assert_called_once_with()
        vision.assert_not_called()
        stop.assert_not_called()

    def test_cli_exposes_the_autosave_turn_to_the_attach_owner(self) -> None:
        with patch.object(civ6_play, "play", return_value=0) as play, \
             patch.object(civ6_play, "enforce_roman_leader",
                          return_value="LEADER_TRAJAN"):
            self.assertEqual(civ6_play.main([
                "--tag", "saved-game", "--attach-running",
                "--attach-replay-turn", "47",
            ]), 0)

        received = play.call_args.args[0]
        self.assertTrue(received.attach_running)
        self.assertEqual(received.attach_replay_turn, 47)


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
    def test_supervised_defaults_are_stock_and_aim_at_a_lane_that_lands(self) -> None:
        """The value itself is argued and pinned in `test_ops_ladder_objective.py`,
        which also holds the evidence trail for the lane. This asserts
        only that the supervised worker takes the chain's one default and stock
        weights, so a second copy cannot appear here."""
        self.assertEqual(civ6_play.DEFAULT_CIVVIS_VICTORY, "science")
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

    def test_a_stranded_create_game_page_ends_the_wait(self) -> None:
        """⚠⚠ A MISSED CLICK HAS A SECOND RESTING PLACE.

        `Start Game` failing to take does NOT return to Single Player -- it
        leaves the Create Game page up, which is neither the main menu nor a
        game. The menu read therefore says "not menu" and the caller waits out
        its entire budget on a screen that never changes on its own.

        Observed 2026-08-31 on run `civvis-20260831T025125Z`, the first
        science-lane attempt after the goal was set: every setup value selected
        and verified, then 480s of "silent, but the main menu is gone" while two
        desktop captures eight minutes apart both show Create Game with
        `Start Game` still sitting there unpressed. ~15 minutes and 207
        screenshots for nothing.
        """
        run_dir = Path(tempfile.mkdtemp())
        patience = {"left": 600.0, "spent": 0.0}
        bounds = (0, 0, 2880, 1864)
        with mock.patch.object(civ6_play, "screenshot", lambda _p: None), \
             mock.patch.object(civ6_play, "_main_menu_visible", lambda _p: False), \
             mock.patch.object(civ6_play, "_observed_label_point",
                               lambda *a, **k: (100, 200)):
            probe = civ6_play._loading_probe(run_dir, 1, patience, 120.0,
                                             bounds)
            self.assertFalse(probe(), "Create Game up means the click died")
        self.assertEqual(patience["left"], 600.0,
                         "a stranded setup page costs nothing but the shot")

    def test_without_bounds_the_probe_keeps_its_old_behaviour(self) -> None:
        """The Create Game read is additive; an old call site still waits."""
        run_dir = Path(tempfile.mkdtemp())
        patience = {"left": 600.0, "spent": 0.0}
        with mock.patch.object(civ6_play, "screenshot", lambda _p: None), \
             mock.patch.object(civ6_play, "_main_menu_visible", lambda _p: False), \
             mock.patch.object(civ6_play, "_observed_label_point",
                               lambda *a, **k: (100, 200)):
            probe = civ6_play._loading_probe(run_dir, 1, patience, 120.0)
            self.assertTrue(probe(), "no bounds, no Create Game read")

    def test_a_real_loading_screen_still_gets_its_patience(self) -> None:
        """The whole point is not to give up on a map that IS generating."""
        run_dir = Path(tempfile.mkdtemp())
        patience = {"left": 600.0, "spent": 0.0}
        bounds = (0, 0, 2880, 1864)
        with mock.patch.object(civ6_play, "screenshot", lambda _p: None), \
             mock.patch.object(civ6_play, "_main_menu_visible", lambda _p: False), \
             mock.patch.object(civ6_play, "_observed_label_point",
                               lambda *a, **k: None):
            probe = civ6_play._loading_probe(run_dir, 1, patience, 120.0,
                                             bounds)
            self.assertTrue(probe(), "neither menu nor setup page: keep waiting")
        self.assertEqual(patience["left"], 480.0, "and it costs one grant")

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

    def test_play_waits_for_unlock_then_launches(self) -> None:
        with patch.object(civ6_play.vision, "available", return_value=True), \
             patch.object(civ6_play, "hold_macos_awake") as hold_awake, \
             patch.object(civ6_play, "screen_locked",
                          side_effect=[True, True, False]), \
             patch.object(civ6_play, "wait_for_safe_screen_capture") as wait_capture, \
             patch.object(civ6_play.time, "sleep") as sleep, \
             patch.object(civ6_play.gamelock, "acquire", return_value=True) as acquire, \
             patch.object(civ6_play.gamelock, "release") as release, \
             patch.object(civ6_play.launcher, "stop") as stop, \
             patch.object(civ6_play, "_play", return_value=0) as run:
            result = civ6_play.play(args(tag="unlock-test", lock_wait=0.0))

        self.assertEqual(result, 0)
        hold_awake.assert_called_once_with()
        # Capture readiness is checked by _play after Civ VI reaches its menu,
        # so stale-stream recovery has a verified game target. Direct launch
        # and the log-backed startup wait do not need a screen frame.
        wait_capture.assert_not_called()
        sleep.assert_called_once_with(2.0)
        acquire.assert_called_once_with(
            "unlock-test", wait_s=0.0, require_verification_intent=True)
        run.assert_called_once()
        stop.assert_called_once()
        release.assert_called_once()

    def test_play_reports_an_explicit_operator_halt_instead_of_free_lock(self) -> None:
        halt = ("the game is explicitly halted since 2026-08-31T23:16:59Z "
                "(reason: protected run); run gamelock.py --resume before "
                "starting another game")
        error = io.StringIO()
        with patch.object(civ6_play.vision, "available", return_value=True), \
             patch.object(civ6_play, "hold_macos_awake"), \
             patch.object(civ6_play, "wait_for_unlocked_session"), \
             patch.object(civ6_play, "wait_for_safe_screen_capture"), \
             patch.object(civ6_play.gamelock, "acquire", return_value=False), \
             patch.object(civ6_play.gamelock, "operator_halt_description",
                          return_value=halt), \
             patch.object(civ6_play.gamelock, "foreign_run") as foreign, \
             patch.object(civ6_play.gamelock, "describe") as describe, \
             patch.object(civ6_play.sys, "stderr", error):
            result = civ6_play.play(args(tag="halt-test", lock_wait=0.0))

        self.assertEqual(result, 6)
        self.assertIn(f"game start blocked: {halt}", error.getvalue())
        self.assertNotIn("another run holds the game: free", error.getvalue())
        foreign.assert_not_called()
        describe.assert_not_called()

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

    def test_live_launcher_coerces_an_explicit_non_roman_leader(self) -> None:
        """A direct harness call must not bypass the standing Rome policy."""
        with patch.object(civ6_play, "play", return_value=0) as play:
            result = civ6_play.main(
                ["--tag", "rome-policy", "--leader", "LEADER_TOKUGAWA"])

        self.assertEqual(result, 0)
        self.assertEqual(play.call_args.args[0].leader, civ6_play.ROMAN_LEADER)

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

    def test_setup_reuses_last_verified_frame_when_final_capture_is_unreadable(self) -> None:
        """A transient ScreenCaptureKit miss must not discard a verified setup."""
        with tempfile.TemporaryDirectory() as temporary:
            fallback = Path(temporary) / "leader-selected.png"
            fallback.write_bytes(b"verified setup frame")

            def select_leader(*_args, panel_out=None, **_kwargs):
                panel_out["shot"] = fallback
                return True

            with patch.object(civ6_play, "set_dropdown", return_value=True), \
                 patch.object(civ6_play, "select_requested_leader",
                              side_effect=select_leader), \
                 patch.object(civ6_play, "screenshot", return_value=False), \
                 patch.object(civ6_play, "_observed_label_point",
                              side_effect=[(321, 432)]), \
                 patch.object(civ6_play, "focus_game") as focus, \
                 patch.object(civ6_play, "click_at") as click:
                started = civ6_play.configure_and_start(
                    (100, 33, 756, 480), args(), Path(temporary)
                )

            setup = Path(temporary) / "setup.png"
            self.assertTrue(started)
            self.assertEqual(setup.read_bytes(), fallback.read_bytes())
            focus.assert_called_once_with(civ6_play.GAME_SIDE, civ6_play.GAME_FRACTION)
            click.assert_called_once_with(321, 432)

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

    @needs_pillow
    def test_leader_picker_retries_an_unreadable_open_frame(self) -> None:
        """A recording-time capture miss is not evidence that the list stayed closed."""
        bounds = (756, 33, 756, 480)
        row = {"text": "Jadwiga", "x": 0.73, "y": 0.29,
               "width": 0.04, "height": 0.02}
        selected = {"text": "Jadwiga", "x": 0.73, "y": 0.155,
                    "width": 0.04, "height": 0.02}
        captures = iter([True, False, True, True, True, True])

        def screenshot(path: Path) -> bool:
            captured = next(captures)
            if captured:
                path.write_bytes(b"x")
            return captured

        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "screenshot", side_effect=screenshot) as shot, \
             patch.object(civ6_play, "_leader_picker_open",
                          side_effect=[False, True]) as picker_open, \
             patch.object(civ6_play, "_setup_current_leader",
                          return_value=("Random Leader", (1134, 140))), \
             patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play, "_leader_ocr", side_effect=[[row], [selected]]), \
             patch.object(civ6_play.macos_input, "move"), \
             patch.object(civ6_play.macos_input, "scroll"), \
             patch.object(civ6_play, "click_at") as click, \
             patch.object(civ6_play.time, "sleep"):
            found = civ6_play.select_requested_leader(
                bounds, "LEADER_JADWIGA", Path(temporary)
            )

        self.assertTrue(found)
        self.assertEqual(picker_open.call_count, 2)
        self.assertEqual(shot.call_count, 6)
        # Two verified field clicks bracket the missing frame; only then does
        # the selector click the OCR-confirmed Jadwiga row.
        self.assertEqual(click.call_count, 3)

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

    def test_selected_leader_readback_accepts_half_height_layout(self) -> None:
        """The lower half-height field still proves Trajan was selected."""
        bounds = (864, 33, 864, 542)
        observation = {
            "text": "Trajan", "x": 0.733, "y": 0.1779,
            "width": 0.011, "height": 0.0057,
        }
        with patch.object(civ6_play, "desktop_size", return_value=(1728, 1117)):
            self.assertIsNotNone(civ6_play._leader_observation(
                [observation], "Trajan", bounds, selected=True
            ))

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


class DealSessionConfigTests(unittest.TestCase):
    """Unattended play uses non-modal deals unless a run opts in explicitly."""

    @staticmethod
    def _config(**changes):
        class Defaults(SimpleNamespace):
            def __getattr__(self, name):
                return None

        return civ6_play.build_config(
            Defaults(tag="t", game_mode=[], dialogue_seconds=0.25,
                     difficulty="DIFFICULTY_SETTLER", map_size="MAPSIZE_SMALL",
                     speed="GAMESPEED_ONLINE", map="Continents.lua",
                     leader="LEADER_TRAJAN", **changes))

    def test_ordinary_play_keeps_direct_deals_by_default(self):
        self.assertIs(self._config()["DealSessions"], False)

    def test_civvis_decider_keeps_non_modal_deals_by_default(self):
        self.assertIs(self._config(civvis_decides=True)["DealSessions"], False)

    def test_interactive_deal_sessions_can_be_explicitly_enabled(self):
        self.assertIs(self._config(deal_sessions=True)["DealSessions"], True)
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text()
        self.assertIn('ap.add_argument("--deal-sessions", dest="deal_sessions",', source)

    def test_civvis_decider_can_explicitly_opt_out(self):
        self.assertIs(
            self._config(civvis_decides=True, deal_sessions=False)["DealSessions"],
            False,
        )
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text()
        self.assertIn('ap.add_argument("--no-deal-sessions", dest="deal_sessions",', source)


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


class VisualPopupCaptureFailureTests(unittest.TestCase):
    def test_transient_popup_capture_miss_leaves_the_game_controller_alive(self) -> None:
        """A blank ScreenCaptureKit frame is not a reason to end a live game."""
        with patch.object(civ6_play, "game_window", return_value=(0, 0, 864, 542)), \
             patch.object(civ6_play, "focus_game") as focus, \
             patch.object(civ6_play.time, "sleep") as sleep, \
             patch.object(civ6_play.popup_clear, "capture",
                          side_effect=civ6_play.macos_capture.CaptureUnavailable(
                              "CoreGraphics capture returned no image")) as capture, \
             patch.object(civ6_play.popup_clear, "classify") as classify, \
             patch.object(civ6_play.popup_clear, "held_click") as click:
            self.assertEqual(
                civ6_play.dismiss_visually_confirmed_popup(),
                (False, "popup capture unavailable"),
            )

        focus.assert_called_once_with(civ6_play.GAME_SIDE, civ6_play.GAME_FRACTION)
        sleep.assert_called_once_with(0.25)
        capture.assert_called_once_with((0, 0, 864, 542))
        classify.assert_not_called()
        click.assert_not_called()


class SafeScreenCaptureWaitTests(unittest.TestCase):
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

    def test_zero_dimension_dropdown_read_is_empty_and_cached(self) -> None:
        """The setup path in the live traceback must retry, not raise.

        Setup fields share one closed-panel capture.  When Vision reports that
        capture as 0x0, each field must see the same empty result rather than
        spending another native OCR pass or terminating the whole attempt.
        """
        civ6_play._OCR_CACHE.clear()
        with tempfile.TemporaryDirectory() as temporary:
            shot = Path(temporary) / "dropdown-difficulty-closed.png"
            shot.write_bytes(b"not-a-frame")
            with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
                 patch.object(civ6_play, "_menu_crop_ocr", return_value=[]), \
                 patch.object(
                     civ6_play.macos_ocr, "recognize",
                     side_effect=civ6_play.macos_ocr.OCRUnavailable("0 x 0"),
                 ) as recognize:
                value = civ6_play._setup_current_value(
                    shot, (756, 33, 756, 480), "difficulty")
                repeat = civ6_play._setup_current_value(
                    shot, (756, 33, 756, 480), "difficulty")
        self.assertIsNone(value)
        self.assertIsNone(repeat)
        recognize.assert_called_once_with(shot)

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

    def test_the_summary_records_the_dealt_arm_of_a_live_screen(self):
        """`withheld`/`forced` say which words reached the decider; they cannot
        say that an unarmed run was the default arm of a screened gene. The
        climb passes `--screen-gene`/`--screen-arm` beside the arm's word and
        the summary keeps both, plus the PLAYED treatment lists lifted from the
        decider's genome line (docs/LIVE_SCREEN.md)."""
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text()
        self.assertIn('"screen_gene": args.screen_gene if args.civvis_decides else None', source)
        self.assertIn('"screen_arm": args.screen_arm if args.civvis_decides else None', source)
        self.assertIn('ap.add_argument("--screen-arm", default=None, choices=("on", "off")', source)
        self.assertIn('summary["genome_treatments"] = {', source)
        for key in ("treatments", "ledger_withheld", "forced"):
            self.assertIn(f'"{key}"', source[source.index('summary["genome_treatments"]'):
                                             source.index('summary["genome"] = genome')])

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
        self.assertEqual(match.group(1), "science")
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



class AWorkingRetireMustNotLookBroken(unittest.TestCase):
    """⚠⚠ The teardown outruns the watcher, so the answer never lands.

    Run `civvis-20260830T083406Z` has NO `retired` event in its events.jsonl,
    while the raw `Automation.log` for the same run holds
    `"kind":"retired","why":"requested"`, our own `"kind":"defeat","ours":true`
    and the `EndGameMenu` opening. The retire had been working; four abandons
    were read as failures because `events.jsonl` was the wrong place to look.
    """

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.logs = Path(self.tmp.name)
        patcher = mock.patch.object(civ6_play.env, "logs_dir", return_value=self.logs)
        patcher.start()
        self.addCleanup(patcher.stop)

    def _write(self, text: str) -> None:
        (self.logs / "Automation.log").write_text(text, encoding="utf-8")

    def test_an_answered_retire_is_recognised(self) -> None:
        self._write('CIVVISJSON {"kind":"retired","run":"civvis-run","why":"requested"}\n')
        self.assertTrue(civ6_play._retire_was_answered("civvis-run"))

    def test_another_runs_retire_is_not_ours(self) -> None:
        self._write('CIVVISJSON {"kind":"retired","run":"civvis-other","why":"requested"}\n')
        self.assertFalse(civ6_play._retire_was_answered("civvis-run"))

    def test_a_missing_log_is_unknown_not_a_crash(self) -> None:
        """The game is stopped either way; this must never raise."""
        self.assertFalse(civ6_play._retire_was_answered("civvis-run"))

    def test_only_the_tail_is_read(self) -> None:
        """The log reaches tens of megabytes across a session."""
        self._write("x" * 2_000_000 + '\nCIVVISJSON {"kind":"retired","run":"civvis-run"}\n')
        self.assertTrue(civ6_play._retire_was_answered("civvis-run"))
        self._write('CIVVISJSON {"kind":"retired","run":"civvis-run"}\n' + "x" * 2_000_000)
        self.assertFalse(civ6_play._retire_was_answered("civvis-run"),
                         "an answer buried a megabyte back is not this run's")

    def test_the_record_separates_asking_from_landing(self) -> None:
        source = Path(civ6_play.__file__).read_text(encoding="utf-8")
        self.assertIn('"retire_requested": state.get("retire_requested"),', source)
        self.assertIn('"retire_confirmed": state.get("retire_confirmed"),', source)


class TheAbandonWaitsForTheRetireToLand(unittest.TestCase):
    """⚠⚠ THE ROW IS NOT THE RETIRE.

    Writing it and returning ends the watch loop, which tears the game down —
    so the mod never reaches its next tick, never sees the row, and the game
    dies exactly as unfinished as before. Measured in run
    `civvis-20260829T194002Z`: the row was on disk as
    `154|99000|retire|below_leader_score|990` and no `retired` event ever
    followed it.
    """

    def test_the_wait_is_bounded_and_long_enough_to_be_seen(self) -> None:
        # The mod polls on `GameCoreEventPublishComplete`, which fires many
        # times per frame while the game is live, so a few seconds is ample.
        # The bound exists so a game that has ALREADY parked cannot hold the
        # loop open — a parked core cannot answer a retire at all, and only the
        # outside watchdog helps there.
        self.assertGreaterEqual(civ6_play.ABANDON_RETIRE_WAIT_S, 5.0)
        self.assertLessEqual(civ6_play.ABANDON_RETIRE_WAIT_S, 60.0)

    def test_the_abandon_path_sleeps_only_when_the_row_was_written(self) -> None:
        """An unwritable channel is not a reason to pause a game that is over."""
        source = Path(civ6_play.__file__).read_text(encoding="utf-8")
        block = source.split("state[\"retire_requested\"] = bool(asked)", 1)[1]
        block = block.split("return True", 1)[0]
        self.assertIn("if asked:", block)
        self.assertIn("time.sleep(ABANDON_RETIRE_WAIT_S)", block)

    def test_the_run_record_says_whether_a_retire_was_asked(self) -> None:
        """A game filed as a loss must be distinguishable from one that stopped."""
        source = Path(civ6_play.__file__).read_text(encoding="utf-8")
        self.assertIn('"retire_requested": state.get("retire_requested"),', source)


class AnAbandonedGameIsRetiredSoItCounts(unittest.TestCase):
    """⚠⚠ Stopping alone leaves the attempt UNFINISHED, not lost.

    Civilization VI files no defeat for a game whose controller simply went
    away: `tools/civ6_ladder.py` records nothing, and a game abandoned on the
    operator's own rule is indistinguishable from one that crashed. The mod
    answers a `retire` row with the shipped
    `UI.RequestAction(ActionTypes.ACTION_RETIRE)` — the single call the stock
    `InGameTopOptionsMenu.lua` makes in `OnReallyRetire`.
    """

    def _db(self) -> Path:
        path = Path(self.tmp.name) / "orders.sqlite"
        import sqlite3
        with sqlite3.connect(str(path)) as db:
            db.execute("CREATE TABLE orders (run TEXT NOT NULL, turn INTEGER NOT NULL,"
                       " seq INTEGER NOT NULL, kind TEXT NOT NULL, subject INTEGER,"
                       " verb TEXT, x INTEGER, y INTEGER,"
                       " frame INTEGER NOT NULL DEFAULT 0,"
                       " PRIMARY KEY (run, turn, seq))")
        return path

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def test_the_row_is_written_where_the_mod_will_find_it(self) -> None:
        import sqlite3
        path = self._db()
        self.assertTrue(orders.request_retire(path, "civvis-run", 150, "below_leader_score"))
        with sqlite3.connect(str(path)) as db:
            rows = list(db.execute("SELECT run, turn, seq, kind, verb, frame FROM orders"))
        self.assertEqual(len(rows), 1)
        run, turn, seq, kind, verb, frame = rows[0]
        self.assertEqual((run, turn, kind, verb), ("civvis-run", 150, "retire", "below_leader_score"))
        # The mod matches a retire on the RUN alone, but the sentinel frame keeps
        # the row out of any real batch: `fetchOrders` filters on an exact frame
        # and the decider's replan frames are single digits.
        self.assertEqual(seq, orders.RETIRE_SEQ)
        self.assertEqual(frame, orders.RETIRE_FRAME)
        self.assertGreater(frame, 9)

    def test_repeats_on_one_turn_collapse_and_the_mod_needs_only_one(self) -> None:
        """The key is (run, turn, seq), so a repeat on the SAME turn replaces.

        A second call on a LATER turn does add a row, and that is harmless on
        purpose: the mod matches `kind = 'retire'` on the run and latches after
        it has asked once, so any number of rows still means exactly one
        `ACTION_RETIRE`. Asserting a single row would be asserting a property
        the code does not have and does not need.
        """
        import sqlite3
        path = self._db()
        orders.request_retire(path, "civvis-run", 150)
        orders.request_retire(path, "civvis-run", 150)
        with sqlite3.connect(str(path)) as db:
            same_turn = list(db.execute(
                "SELECT count(*) FROM orders WHERE kind = 'retire'"))[0][0]
        self.assertEqual(same_turn, 1, "a repeat on one turn replaces its row")
        orders.request_retire(path, "civvis-run", 151)
        with sqlite3.connect(str(path)) as db:
            rows = list(db.execute(
                "SELECT count(*) FROM orders WHERE kind = 'retire'"))[0][0]
        self.assertEqual(rows, 2)
        # What the mod actually asks is "is there one at all", so both states
        # answer the same way.
        self.assertGreater(rows, 0)

    def test_an_unwritable_database_is_not_a_reason_to_keep_playing(self) -> None:
        """The rule has already called the game; filing it is best effort."""
        missing = Path(self.tmp.name) / "no-such-dir" / "orders.sqlite"
        self.assertFalse(orders.request_retire(missing, "civvis-run", 150))


class ScoreGapsRemainTelemetryForLiveVerification(unittest.TestCase):
    """Full games retain score gaps instead of treating them as a loss call.

    The former predicate stays tested below because the census uses it to show
    what an early cutoff would have hidden, never to end a live game.
    """

    @staticmethod
    def _turn(turn, score, rival, ctx="agent", kind="turn"):
        return {"kind": kind, "ctx": ctx, "turn": turn, "score": score,
                "rival_best": rival}

    def test_live_default_is_disabled_but_the_historical_floor_is_stable(self):
        self.assertEqual(civ6_play.DEFAULT_LEADER_SCORE_RATIO, 0.0)
        self.assertEqual(civ6_play.LEADER_SCORE_MIN_TURN, 51)

    def test_targeted_science_is_not_auto_retired_for_a_score_gap(self):
        self.assertFalse(civ6_play.leader_score_stop_allowed(
            civvis_decides=True, victory_target="science"))

    def test_every_other_lane_also_plays_score_gaps_out(self):
        for decides, target in ((False, "science"), (True, "civvis"),
                                (True, "culture"), (True, None)):
            with self.subTest(decides=decides, target=target):
                self.assertFalse(civ6_play.leader_score_stop_allowed(
                    civvis_decides=decides, victory_target=target))

    def test_a_historical_reading_marks_where_the_former_line_would_have_fired(self):
        state = {}
        verdict = civ6_play.below_leader_score_reading(
            state, self._turn(51, 49, 100), 0.50)
        self.assertEqual(verdict, {
            "rule": "below_leader_score", "turn": 51, "score": 49,
            "rival_best": 100, "score_ratio": 0.49,
            "score_ratio_ceiling": 0.50, "min_turn": 51,
        })
        self.assertEqual(state, {})

    def test_nothing_fires_through_turn_50_however_far_behind(self):
        state = {}
        for turn in range(1, 51):
            self.assertIsNone(civ6_play.below_leader_score_reading(
                state, self._turn(turn, 10, 500), 0.50))
        self.assertEqual(state, {})

    def test_at_the_line_is_not_under_it_but_the_next_low_reading_ends_it(self):
        state = {}
        # Exactly half is not more than 50 % behind.
        self.assertIsNone(civ6_play.below_leader_score_reading(
            state, self._turn(51, 50, 100), 0.50))
        self.assertEqual(civ6_play.below_leader_score_reading(
            state, self._turn(52, 49, 100), 0.50)["turn"], 52)

    def test_only_a_readable_agent_turn_is_a_termination_reading(self):
        state = {}
        # No standing is not a decision to abandon.
        self.assertIsNone(civ6_play.below_leader_score_reading(
            state, self._turn(171, 300, None), 0.50))
        self.assertIsNone(civ6_play.below_leader_score_reading(
            state, self._turn(172, 300, 0), 0.50))
        self.assertIsNone(civ6_play.below_leader_score_reading(
            state, self._turn(173, None, 500), 0.50))
        # Other contexts and event kinds are not termination readings either.
        # 100/500 is 20 %: it fires on an agent `turn` event and must stay
        # silent on every other one.
        self.assertIsNone(civ6_play.below_leader_score_reading(
            state, self._turn(174, 100, 500, ctx="spectator"), 0.50))
        self.assertIsNone(civ6_play.below_leader_score_reading(
            state, self._turn(174, 100, 500, kind="state"), 0.50))
        self.assertEqual(civ6_play.below_leader_score_reading(
            state, self._turn(174, 100, 500), 0.50)["turn"], 174)
        self.assertEqual(state, {})

    def test_zero_or_an_invalid_line_has_no_historical_reading(self):
        for ceiling in (civ6_play.DEFAULT_LEADER_SCORE_RATIO, 0, 0.0, -1,
                        1.5, True, None, "0.6"):
            with self.subTest(ceiling=ceiling):
                state = {}
                for turn in range(51, 190):
                    self.assertIsNone(civ6_play.below_leader_score_reading(
                        state, self._turn(turn, 10, 500), ceiling))
                self.assertEqual(state, {})

    def test_a_historical_abandoned_game_keeps_its_original_reason(self):
        """Old records remain legible; new score gaps cannot create one."""
        abandoned = {"rule": "below_leader_score", "turn": 51,
                     "score_ratio": 0.49, "score_ratio_ceiling": 0.50}
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

    def test_the_live_loop_keeps_score_telemetry_behind_the_full_game_policy(self):
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        self.assertIn("below_leader_score_reading(\n"
                      "            state, event, args.restart_below_leader_ratio", source)
        self.assertIn("leader_score_stop_allowed(\n"
                      "            civvis_decides=args.civvis_decides", source)
        self.assertIn('"--restart-below-leader-ratio"', source)
        self.assertIn("default=DEFAULT_LEADER_SCORE_RATIO", source)
        self.assertIn("DEFAULT_LEADER_SCORE_RATIO = 0.0", source)
        policy = source.split("def leader_score_stop_allowed", 1)[1].split(
            "\ndef _nonnegative_metric", 1)[0]
        self.assertIn("return False", policy)
        self.assertIn('"abandoned": state.get("abandoned"),', source)
        self.assertNotIn("LEADER_SCORE_PATIENCE", source)
        for scrapped in ("opening_city_target_reading(state",
                         "second_settler_loss_reading(state",
                         "behind_all_metrics_reading(",
                         "abandon_reading(state", "record_opening_settlers(",
                         "ABANDON_CELLS", 'add_argument("--abandon-below-win-rate"'):
            self.assertNotIn(scrapped, source, scrapped)
        # The deal lane's tally rides the summary beside the orders totals.
        self.assertIn('deals = civ6_ladder.deal_totals(run_dir / "events.jsonl")', source)
        self.assertIn('summary["deals"] = deals', source)


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
        input_order = []
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "_setup_current_value", side_effect=lambda *a: next(reads)), \
             patch.object(civ6_play, "_observed_label_point", return_value=(700, 340)), \
             patch.object(civ6_play, "focus_game",
                          side_effect=lambda *_args: input_order.append("focus")), \
             patch.object(civ6_play, "click_at",
                          side_effect=lambda *_args: input_order.append("click")) as click, \
             patch.object(civ6_play.macos_input, "move") as move, \
             patch.object(civ6_play.time, "sleep"):
            ok = civ6_play.set_dropdown((0, 0, 756, 480), "speed", "GAMESPEED_ONLINE",
                                        Path(temporary), panel_out=out)
            again = Path(temporary) / "dropdown-speed-selected-again.png"

        self.assertTrue(ok)
        # One click on the closed row, one on the option -- the list was NOT reopened.
        self.assertEqual(click.call_args_list, [call(700, 300), call(700, 340)])
        self.assertEqual(input_order, ["focus", "click", "focus", "click"])
        self.assertEqual(move.call_args_list, [call(113, 408), call(113, 408)])
        self.assertEqual(screenshot.call_args_list[-1], call(again))
        self.assertEqual(out["shot"], again)

    def test_a_late_open_dropdown_is_selected_without_closing_it(self) -> None:
        """A slow UI may process the first proved click after its first read."""
        reads = iter([
            ("GAMESPEED_STANDARD", (700, 300)),
            ("GAMESPEED_ONLINE", (700, 300)),
        ])
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(civ6_play, "screenshot") as screenshot, \
             patch.object(civ6_play, "_setup_current_value",
                          side_effect=lambda *a: next(reads)), \
             patch.object(civ6_play, "_observed_label_point",
                          side_effect=[None, (700, 340)]) as observed, \
             patch.object(civ6_play, "focus_game"), \
             patch.object(civ6_play, "click_at") as click, \
             patch.object(civ6_play.macos_input, "move") as move, \
             patch.object(civ6_play.time, "sleep") as sleep:
            ok = civ6_play.set_dropdown((0, 0, 756, 480), "speed", "GAMESPEED_ONLINE",
                                        Path(temporary))

        self.assertTrue(ok)
        # First click opens the list late.  The retry waits, proves Online is
        # now rendered, and clicks that row rather than toggling Standard again.
        self.assertEqual(click.call_args_list, [call(700, 300), call(700, 340)])
        self.assertEqual(move.call_args_list, [call(113, 408), call(113, 408)])
        self.assertEqual(observed.call_count, 2)
        self.assertIn(call(2.0), sleep.call_args_list)
        self.assertEqual(screenshot.call_args_list[-1],
                         call(Path(temporary) / "dropdown-speed-selected.png"))

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


class EveryArgparseHelpStringCanActuallyBeRendered(unittest.TestCase):
    """`--help` must not traceback, and for two harness CLIs it did.

    ⚠ `civ6_play.py --help` and `civ6_civvis_climb.py --help` both died with a
    traceback on EVERY interpreter on this host, because argparse formats each
    help string as `help % params` and both said "60% of the leader's score".
    Python 3.9 (the production `/usr/bin/python3`) raised
    `TypeError: %o format: an integer is required, not dict` from `_expand_help`
    at print time; Python 3.14 raises `ValueError: badly formed help string`
    eagerly from `add_argument`, which took the climb's whole parser down and
    turned 24 of its unit tests into errors. A literal percent in an argparse
    help string must be written `%%`.

    Scanned by AST rather than by importing each tool, because most of them
    reach the screen, the Steam install or the network at import time.
    """

    #: argparse substitutes `%(name)s`-style mapping keys; everything else that
    #: follows a `%` has to be an escaped `%%` or the format blows up.
    _SPEC = re.compile(
        r"%(?:%|\((?P<name>\w+)\)[-#0 +]*\d*(?:\.\d+)*[hlL]?"
        r"[diouxXeEfFgGcrsa])")

    @classmethod
    def _unformattable_offsets(cls, text: str) -> list[int]:
        offsets, index = [], 0
        while True:
            found = text.find("%", index)
            if found < 0:
                return offsets
            matched = cls._SPEC.match(text, found)
            if matched:
                index = matched.end()
            else:
                offsets.append(found)
                index = found + 1

    def test_no_tool_hides_a_bare_percent_in_an_argparse_help_string(self):
        import ast

        tools = Path(__file__).resolve().parent
        offenders = []
        for source_file in sorted(tools.rglob("*.py")):
            if "__pycache__" in source_file.parts:
                continue
            try:
                tree = ast.parse(source_file.read_text(encoding="utf-8"))
            except SyntaxError:  # pragma: no cover - a parse failure is its own test
                continue
            for node in ast.walk(tree):
                if not isinstance(node, ast.Call):
                    continue
                name = getattr(node.func, "attr", None) or getattr(
                    node.func, "id", None)
                if name != "add_argument":
                    continue
                for keyword in node.keywords:
                    if keyword.arg != "help":
                        continue
                    try:
                        value = ast.literal_eval(keyword.value)
                    except Exception:
                        continue
                    if isinstance(value, str) and self._unformattable_offsets(value):
                        offenders.append(
                            f"{source_file.name}:{node.lineno}: {value}")
        self.assertEqual(offenders, [], "write a literal percent as %%:\n"
                         + "\n".join(offenders))

    def test_the_two_harness_entry_points_render_their_own_help(self):
        """The regression itself: both CLIs must print help and exit 0."""
        import subprocess

        tools = Path(__file__).resolve().parent
        for script in ("civ6_play.py", "civ6_civvis_climb.py"):
            with self.subTest(script=script):
                done = subprocess.run(
                    [sys.executable, str(tools / script), "--help"],
                    capture_output=True, text=True, timeout=120)
                self.assertEqual(done.returncode, 0, done.stderr[-2000:])
                self.assertIn("--restart-below-leader-ratio", done.stdout)


class FinishedRunsStopKeepingTheirScreenshots(unittest.TestCase):
    """⚠ There was no retention at all, and it filled 179 GB.

    `~/civvis-civ6-runs` held 852 runs on 2026-08-28. 97 % of a run is PNG: the
    setup polls photograph the whole Retina desktop at ~10 MB a frame, and one
    run that struggled through its leader intro left 360 of them — 2 GB for a
    single game. Nothing read those images after the day they were taken and
    nothing deleted them, so "run games indefinitely" was also "fill the disk".

    Every `events.jsonl` is kept forever: that is what the censuses read.
    """

    @staticmethod
    def _run(root: Path, name: str, age_days: float, shots: int = 3) -> Path:
        run = root / name
        run.mkdir(parents=True)
        for index in range(shots):
            (run / f"setup-{index}.png").write_bytes(b"\x89PNG" + b"0" * 1000)
        (run / "events.jsonl").write_text('{"kind":"turn"}\n')
        (run / "why.log").write_text("because\n")
        stamp = time.time() - age_days * 86400
        os.utime(run, (stamp, stamp))
        return run

    def test_old_runs_lose_their_pngs_and_keep_every_other_artefact(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            old = self._run(root, "civvis-old", age_days=30)
            fresh = self._run(root, "civvis-fresh", age_days=1)

            runs, freed = civ6_play.prune_old_run_screenshots(root, days=7)

            self.assertEqual(runs, 1)
            self.assertGreater(freed, 3000)
            self.assertEqual(list(old.glob("*.png")), [])
            self.assertTrue((old / "events.jsonl").is_file(),
                            "the census evidence must survive")
            self.assertTrue((old / "why.log").is_file())
            self.assertEqual(len(list(fresh.glob("*.png"))), 3,
                             "a run inside the window keeps its screenshots")

    def test_a_zero_window_keeps_everything(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            old = self._run(root, "civvis-old", age_days=400)
            self.assertEqual(civ6_play.prune_old_run_screenshots(root, days=0),
                             (0, 0))
            self.assertEqual(len(list(old.glob("*.png"))), 3)

    def test_a_missing_root_is_not_an_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(
                civ6_play.prune_old_run_screenshots(Path(tmp) / "absent", days=7),
                (0, 0))

    def test_the_window_comes_from_the_environment_when_asked(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            old = self._run(root, "civvis-old", age_days=3)
            with mock.patch.dict(os.environ, {"CIVVIS_RUN_SHOT_DAYS": "1"}):
                runs, _ = civ6_play.prune_old_run_screenshots(root)
            self.assertEqual(runs, 1)
            self.assertEqual(list(old.glob("*.png")), [])
            # A value that is not a number must not take the harness down.
            with mock.patch.dict(os.environ, {"CIVVIS_RUN_SHOT_DAYS": "soon"}):
                self.assertEqual(
                    civ6_play.prune_old_run_screenshots(root), (0, 0))

    def test_a_new_run_prunes_before_it_starts_writing(self):
        source = (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")
        self.assertIn("run_dir.mkdir(parents=True, exist_ok=True)\n"
                      "    # Bound the corpus before adding to it.", source)
        self.assertIn("prune_old_run_screenshots()", source)


class AnUnreadableMenuLooksForTheBackItRenders(unittest.TestCase):
    """⚠ The recovery click was itself an unverified coordinate.

    Two lines above it, the same branch prints "refusing a blind menu click".
    Then, after three wasted attempts, it clicked `(0.723, 0.177)` — where BACK
    sits on the setup pages — guessed.

    Measured on 2026-08-28, attempts 12-14 of run civvis-20260828T144631Z: the
    top menu was unreadable because a SELECT MAP modal was covering it, and
    that modal renders "Back" in the same corner, legibly. The screenshots were
    3456x2234 with 256 distinct luma values, so nothing was wrong with them —
    the harness spent three full-Retina Vision passes discovering nothing.
    """

    def _source(self) -> str:
        return (Path(__file__).resolve().parent / "civ6_play.py").read_text(
            encoding="utf-8")

    def test_the_rendered_back_is_tried_before_the_guessed_ratio(self):
        source = self._source()
        observed = source.index(
            'back_point = _observed_label_point(menushot, "Back", bounds)')
        guessed = source.index('click_at(int(x + w * 0.723), int(y + h * 0.177))')
        self.assertLess(observed, guessed,
                        "the read label must be tried before the guess")

    def test_a_read_back_does_not_spend_a_blind_strike(self):
        """The strike counter exists for a screen nothing can be read on."""
        source = self._source()
        block = source[source.index(
            'back_point = _observed_label_point(menushot, "Back", bounds)'):]
        block = block[:block.index("blind_strikes += 1")]
        self.assertIn("blind_strikes = 0", block)
        self.assertIn("continue", block)

    def test_the_guessed_ratio_survives_as_the_last_resort(self):
        """A screen where even Back cannot be read must behave as before."""
        source = self._source()
        self.assertIn("if blind_strikes >= 3:", source)
        self.assertIn('click_at(int(x + w * 0.723), int(y + h * 0.177))', source)

    def test_the_reason_names_the_dialog_rather_than_the_ocr(self):
        """The log said "not readable", which read as an OCR fault. It was not."""
        self.assertIn("a dialog is covering the menu", self._source())


class OperatorRetireNativeTest(unittest.TestCase):
    """A host retirement must stay inside Civ VI's native action channel."""

    @staticmethod
    def _source() -> str:
        return Path(civ6_play.__file__).read_text()

    def test_the_host_writes_a_native_order_and_waits_for_the_mod_ack(self):
        source = self._source()
        self.assertIn("request_retire(\n                orders_db_path", source)
        self.assertIn("operator_retire.record_retired(", source)
        self.assertIn('elif kind == "retired":', source)
        self.assertIn("OPERATOR_RETIRE_SETTLE_S", source)

    def test_no_pause_menu_or_screen_click_path_remains(self):
        source = self._source()
        self.assertNotIn("retire_from_game_menu", source)
        self.assertNotIn("operator-retire-menu.png", source)

    def test_a_native_acknowledgement_cannot_be_reclassified_as_setup_failure(self):
        self.assertEqual(
            civ6_play.summary_reason(
                {"operator_retire_event": {"kind": "retired"},
                 "ruleset_match": False, "mode_mismatch": True,
                 "seat": {"difficulty": "DIFFICULTY_PRINCE"}, "configured": False},
                "stopped"),
            "operator_retired")


class AStoppedRunStillLeavesARecord(unittest.TestCase):
    """⚠⚠⚠ A KILLED RUN LEFT NO RECORD AT ALL, AND THAT IS MOST OF THEM.

    `summary.json` is written near the end of `main`, so a run stopped by a
    signal left an events file and nothing else. Measured 2026-08-30 over the
    08-29/30 runs: **53 of 64 runs had no summary** — 17% coverage — and the
    missing 83% are precisely the parked cores the wedge watchdog kills, the
    dominant way a run dies. Every "how our games end" tally, the abandon rate
    and the win rate were computed over the survivors only.
    """

    def _config(self) -> dict:
        return {"Difficulty": "DIFFICULTY_KING", "MapSize": "MAPSIZE_SMALL",
                "GameSpeed": "GAMESPEED_ONLINE", "MaxTurns": 250,
                "MapSeed": None}

    def test_it_records_what_the_run_had_reached(self):
        state = {"turn": 118, "score": 240, "cities_at_60": 3,
                 "outcome": None, "abandoned": None}
        row = civ6_play.partial_summary("civvis-x", self._config(), state)
        self.assertEqual(row["last_turn"], 118)
        self.assertEqual(row["last_score"], 240)
        self.assertEqual(row["cities_at_60"], 3)
        self.assertEqual(row["difficulty"], "DIFFICULTY_KING")
        self.assertEqual(row["tag"], "civvis-x")

    def test_it_is_marked_partial_and_killed(self):
        """A stopped run must never be mistaken for a played one."""
        row = civ6_play.partial_summary("civvis-x", self._config(),
                                        {"turn": 1, "score": -1})
        self.assertIs(row["partial"], True)
        self.assertEqual(row["reason"], "killed")

    def test_a_run_that_never_played_is_still_honest(self):
        """A run that never reached a turn records nothing rather than a zero:
        a missing key stays None so the ledger cannot read it as a played
        game at turn 0."""
        row = civ6_play.partial_summary("civvis-x", self._config(), {})
        self.assertIsNone(row["last_turn"])
        self.assertIsNone(row["cities_at_60"])

    def test_the_fallback_is_registered_and_never_overwrites(self):
        source = (Path(__file__).resolve().parent
                  / "civ6_play.py").read_text(encoding="utf-8")
        self.assertIn("atexit.register(_partial_summary_if_stopped)", source)
        block = source[source.index("def _partial_summary_if_stopped"):
                       source.index("atexit.register(_partial_summary_if_stopped)")]
        # It must bail out when a real summary is already on disk.
        self.assertIn("if path.exists():", block)
        self.assertLess(block.index("if path.exists():"),
                        block.index("partial_summary("))


class TheBottomStripRunsWhenTheHeadingHidesTheButton(unittest.TestCase):
    """⚠⚠⚠ THE STRIP RAN ONLY WHEN THE EARLIER PASSES FOUND *NOTHING*.

    `LOAD_GAME_ACTION_STRIP` exists because the full-screen pass and the general
    menu crop both miss the small Load Game BUTTON on the bottom edge. But the
    same screen carries a large LOAD GAME HEADING that every pass reads easily,
    so `points` came back non-empty, the strip pass was skipped, and the caller
    rejected the screen with "only the Load Game heading is visible". The repair
    could not run on the only screen that needed it.

    Live 2026-08-30: the first autosave reload the watchdog handoff ever produced
    (`civvis-20260830T112732Z-cont1`) died exactly there, with
    `load-selected-attempt2.png` showing `civvis-resume` selected, the preview
    reading TURN 75 Renaissance Era, and the button plainly rendered.
    """

    BOUNDS = (756, 33, 756, 480)
    HEADING = {"text": "Load Game", "x": 0.70, "y": 0.16,
               "width": 0.06, "height": 0.02}
    BUTTON = {"text": "Load Game", "x": 0.70, "y": 0.49,
              "width": 0.04, "height": 0.02}

    def test_the_strip_still_runs_when_only_the_heading_was_read(self) -> None:
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play, "recognize_once", return_value=[self.HEADING]), \
             patch.object(civ6_play, "_menu_crop_ocr",
                          return_value=[self.BUTTON]) as crop:
            points = civ6_play._observed_label_points(
                Path("load-selected.png"), "Load Game", self.BOUNDS,
                strip=civ6_play.LOAD_GAME_ACTION_STRIP,
            )

        # Both are kept, and the caller's `max(..., key=y)` now reaches the
        # button instead of refusing the screen.
        self.assertEqual(len(points), 2)
        self.assertEqual(max(points, key=lambda point: point[1]), (1088, 491))
        crop.assert_called_once_with(
            Path("load-selected.png"), self.BOUNDS,
            strip=civ6_play.LOAD_GAME_ACTION_STRIP, tag="strip")

    def test_a_button_already_read_costs_no_extra_pass(self) -> None:
        """The band it covers is the band the earlier passes are unreliable in;
        when one of them did read something there, the extra OCR is waste."""
        with patch.object(civ6_play, "desktop_size", return_value=(1512, 982)), \
             patch.object(civ6_play, "recognize_once", return_value=[self.BUTTON]), \
             patch.object(civ6_play, "_menu_crop_ocr") as crop:
            points = civ6_play._observed_label_points(
                Path("load-selected.png"), "Load Game", self.BOUNDS,
                strip=civ6_play.LOAD_GAME_ACTION_STRIP,
            )

        self.assertEqual(points, [(1088, 491)])
        crop.assert_not_called()


class PollCadenceKeepsItsWallClock(unittest.TestCase):
    """The mod polls for CIVVIS's answer 7.5× as often; nothing else moved.

    Every poll budget the mod reads (`OrdersWaitPolls`, `OrdersFallbackPolls`,
    `CombatFramePolls`) is a count of polls, so shortening the poll interval
    without scaling them would have cut the stale-answer, fallback and combat
    frame allowances to a fraction of their measured wall clock. The products
    below are the pre-2026-09-01 values (40×30, 120×30, 20×30) in ticks.
    """

    def test_poll_budgets_keep_their_wall_clock(self) -> None:
        self.assertEqual(civ6_play.ORDERS_POLL_TICKS * civ6_play.ORDERS_WAIT_POLLS, 1200)
        self.assertEqual(civ6_play.ORDERS_POLL_TICKS * civ6_play.ORDERS_FALLBACK_POLLS, 3600)
        self.assertEqual(civ6_play.ORDERS_POLL_TICKS * civ6_play.COMBAT_FRAME_POLLS, 600)

    def test_the_mod_never_polls_on_every_tick(self) -> None:
        # The every-publish query deadlocked run civvis-20260730T110209Z.
        self.assertGreaterEqual(civ6_play.ORDERS_POLL_TICKS, 2)

    def test_the_poll_is_still_many_publish_batches_apart(self) -> None:
        '''★ THE SAFETY QUANTITY IS PUBLISH BATCHES, NOT TICKS.

        An agent tick is `TickEvery` (16) game-core publish batches, and what
        deadlocked run civvis-20260730T110209Z was querying SQLite on every
        publish. Two ticks is 32 batches apart; the tick count alone does not
        say that, so assert the product the deadlock was actually about.
        '''
        lua = (Path(civ6_play.__file__).resolve().parent / "civ6_control" / "mod"
               / "CivvisControlAgent.lua").read_text()
        self.assertIn("cfg.TickEvery or 16)", lua,
                      "the batches-per-tick figure this arithmetic rests on moved")
        self.assertGreaterEqual(civ6_play.ORDERS_POLL_TICKS * 16, 32)

    def test_the_lua_fallbacks_match_the_harness_defaults(self) -> None:
        lua = (Path(civ6_play.__file__).resolve().parent / "civ6_control" / "mod"
               / "CivvisControlAgent.lua").read_text()
        self.assertIn(f"cfg.OrdersPollTicks or {civ6_play.ORDERS_POLL_TICKS};", lua)
        self.assertIn(f"cfg.OrdersWaitPolls or {civ6_play.ORDERS_WAIT_POLLS})", lua)
        self.assertIn(f"cfg.OrdersFallbackPolls or {civ6_play.ORDERS_FALLBACK_POLLS})", lua)
        self.assertIn(f"tonumber(cfg.CombatFramePolls) or {civ6_play.COMBAT_FRAME_POLLS})", lua)
