#!/usr/bin/env python3
"""Regression checks for the live mirror's north-up staging."""

from __future__ import annotations

import json
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import follow  # noqa: E402


class FollowTest(unittest.TestCase):
    def test_visible_server_refreshes_an_existing_tab_after_starting(self) -> None:
        class LiveProcess:
            pid = 4242
            returncode = None

            @staticmethod
            def poll() -> None:
                return None

        with tempfile.TemporaryDirectory() as temporary:
            previous_rig = follow.RIG
            follow.RIG = temporary
            try:
                with mock.patch.object(follow, "read_events", return_value=([], 1, 4, 9, True)), \
                     mock.patch.object(follow, "stage_events", return_value=temporary), \
                     mock.patch.object(follow, "server_alive", return_value=True), \
                     mock.patch.object(follow.subprocess, "Popen", return_value=LiveProcess()), \
                     mock.patch.object(follow, "hold_the_frame") as hold, \
                     mock.patch.object(follow, "refresh_mirror_page") as refresh, \
                     mock.patch.object(follow.time, "sleep"):
                    self.assertTrue(follow.start_visible_server(temporary, 4))
            finally:
                follow.RIG = previous_rig

        refresh.assert_called_once_with(4242)
        self.assertEqual(hold.call_count, 2)

    def test_refreshing_an_existing_tab_uses_a_new_url_without_activating_chrome(self) -> None:
        with mock.patch.object(follow, "mirror_on_screen", return_value=True), \
             mock.patch.object(follow, "chrome") as chrome:
            follow.refresh_mirror_page(4242)

        chrome.assert_called_once()
        script = chrome.call_args.args[0]
        self.assertIn(f'{follow.MIRROR_URL}?instance=4242', script)
        self.assertIn("set URL of thisTab", script)
        self.assertNotIn("activate", script)

    def test_refresh_does_not_open_or_switch_to_chrome_when_mirror_is_absent(self) -> None:
        for shown in (False, None):
            with self.subTest(shown=shown), \
                 mock.patch.object(follow, "mirror_on_screen", return_value=shown), \
                 mock.patch.object(follow, "chrome") as chrome:
                follow.refresh_mirror_page(4242)
            chrome.assert_not_called()

    def test_visible_server_start_fails_fast_when_child_exits(self) -> None:
        class DeadProcess:
            returncode = 17

            @staticmethod
            def poll() -> int:
                return 17

        messages = []
        with tempfile.TemporaryDirectory() as temporary:
            previous_rig = follow.RIG
            follow.RIG = temporary
            try:
                with mock.patch.object(follow, "read_events", return_value=([], 1, 4, 9, True)), \
                     mock.patch.object(follow, "stage_events", return_value=temporary), \
                     mock.patch.object(follow, "server_alive", return_value=False), \
                     mock.patch.object(follow, "log", side_effect=messages.append), \
                     mock.patch.object(follow.subprocess, "Popen", return_value=DeadProcess()) as popen, \
                     mock.patch.object(follow.time, "sleep") as sleep:
                    self.assertFalse(follow.start_visible_server(temporary, 4))
            finally:
                follow.RIG = previous_rig

        popen.assert_called_once()
        sleep.assert_not_called()
        self.assertTrue(any("status 17" in message for message in messages))

    def test_a_finished_game_is_taken_off_the_screen(self) -> None:
        """The two windows must never show different games.

        Measured 2026-08-10: the follower adopted a brand-new run while :8610
        kept serving TURN 189 of the run before it, so the operator saw a
        finished five-city empire beside a live game still choosing its leader.
        """
        killed = []
        with mock.patch.object(follow.subprocess, "run") as run, \
             mock.patch.object(follow.os, "kill",
                               lambda pid, sig: killed.append(pid)), \
             mock.patch.object(follow, "server_alive", lambda _p: False), \
             mock.patch.object(follow, "log", lambda _m: None):
            run.return_value = mock.Mock(stdout="4242 4243\n")
            self.assertTrue(follow.stop_visible_server())
        self.assertEqual(killed, [4242, 4243],
                         "every mirror server holding the port must be stopped")

    def test_takedown_reports_nothing_to_stop_when_no_server_runs(self) -> None:
        with mock.patch.object(follow.subprocess, "run") as run, \
             mock.patch.object(follow, "log", lambda _m: None):
            run.return_value = mock.Mock(stdout="")
            self.assertFalse(follow.stop_visible_server())

    def test_read_events_reports_whether_the_run_has_exported_a_map(self) -> None:
        """The precondition `civvis play --mirror` refuses on, read from the run.

        `snapshot_from_events` builds revealed plots out of `tiles` events, so a
        run with none of them cannot clear `revealed_count() > 0` — which is the
        check `src/main.rs` exits 2 on.
        """
        with tempfile.TemporaryDirectory() as temporary:
            events = Path(temporary) / "events.jsonl"
            events.write_text(
                json.dumps({"kind": "seat", "players": 6}) + "\n"
                + json.dumps({"kind": "state", "turn": 3}) + "\n")
            self.assertEqual(follow.read_events(temporary)[4], False)

            with events.open("a") as handle:
                handle.write(json.dumps(
                    {"kind": "tiles", "height": 46, "plots": []}) + "\n")
            lines, turn, players, height, tiles = follow.read_events(temporary)
            self.assertEqual((turn, players, height, tiles), (3, 6, 46, True))
            self.assertEqual(len(lines), 3)

    def test_a_run_with_no_export_yet_does_not_spawn_a_mirror_server(self) -> None:
        """40 spawn-and-die cycles in four minutes, on run civvis-20260807T134625Z.

        Every one of them launched the engine binary, loaded the game database,
        hit "has no tiles to mirror", and exited 2 — while Civilization VI was
        generating its map, which is the window its frame budget least affords.
        """
        messages = []
        with mock.patch.object(follow, "read_events", return_value=([b"{}"], 1, 6, 38, False)), \
             mock.patch.object(follow, "newest_run", return_value=("/run/a", time.time())), \
             mock.patch.object(follow, "ensure_on_screen", return_value=0), \
             mock.patch.object(follow, "server_alive", return_value=False), \
             mock.patch.object(follow, "start_visible_server") as start, \
             mock.patch.object(follow, "log", side_effect=messages.append), \
             mock.patch.object(follow.time, "sleep", side_effect=[None, None, StopIteration]):
            with self.assertRaises(StopIteration):
                follow.main()

        start.assert_not_called()
        waits = [m for m in messages if "waiting for the run's first map export" in m]
        self.assertEqual(len(waits), 1, f"expected one wait line, got {messages}")

    def test_a_refused_start_names_the_binary_s_own_reason(self) -> None:
        """The status alone reads the same for a wait and a misconfiguration."""
        with tempfile.TemporaryDirectory() as temporary:
            previous_rig = follow.RIG
            follow.RIG = temporary
            try:
                Path(temporary, "server.log").write_text(
                    "mirroring 160 revealed plots of a 74x46 world at turn 4\n"
                    "/stage/events.jsonl has no tiles to mirror — the run needs "
                    "--export-state\n")
                self.assertIn("has no tiles to mirror", follow.server_log_reason())
            finally:
                follow.RIG = previous_rig

    def test_even_height_map_keeps_both_polar_rows_on_an_extra_staging_row(self) -> None:
        event = {
            "kind": "tiles", "turn": 7, "height": 46,
            "plots": [
                {"x": 3, "y": 0, "rv": 0},
                {"x": 4, "y": 45, "rv": 0},
            ],
        }
        with tempfile.TemporaryDirectory() as temporary:
            previous = follow.STAGE
            follow.STAGE = temporary
            try:
                follow.stage_events([json.dumps(event).encode()], 46)
                staged = json.loads((Path(temporary) / "events.jsonl").read_text())
            finally:
                follow.STAGE = previous

        self.assertEqual(staged["height"], 47)
        self.assertEqual(
            {(plot["x"], plot["y"]) for plot in staged["plots"]},
            {(3, 46), (4, 1)},
        )

    def test_north_up_reflection_transforms_qualified_coordinate_pairs(self) -> None:
        event = {
            "kind": "state", "turn": 7,
            "trade_routes": [{
                "trader": 42,
                "origin_x": 3, "origin_y": 1,
                "destination_x": 6, "destination_y": 6,
            }],
            "refusal": {
                "from_x": 2, "from_y": 3,
                "x": 4, "y": 4,
            },
        }
        with tempfile.TemporaryDirectory() as temporary:
            previous = follow.STAGE
            follow.STAGE = temporary
            try:
                follow.stage_events([json.dumps(event).encode()], 9)
                staged = json.loads((Path(temporary) / "events.jsonl").read_text())
            finally:
                follow.STAGE = previous

        route = staged["trade_routes"][0]
        self.assertEqual((route["origin_x"], route["origin_y"]), (3, 7))
        self.assertEqual((route["destination_x"], route["destination_y"]), (6, 2))
        self.assertEqual((staged["refusal"]["from_x"], staged["refusal"]["from_y"]), (2, 5))
        self.assertEqual((staged["refusal"]["x"], staged["refusal"]["y"]), (4, 4))

    def test_north_up_reflection_reencodes_river_on_the_other_endpoint(self) -> None:
        event = {
            "kind": "tiles", "turn": 7, "height": 9,
            "plots": [
                {"x": 3, "y": 3, "rv": 2, "ri": True},
                {"x": 4, "y": 2, "rv": 0, "ri": True},
            ],
        }
        with tempfile.TemporaryDirectory() as temporary:
            previous = follow.STAGE
            follow.STAGE = temporary
            try:
                follow.stage_events([json.dumps(event).encode()], 9)
                staged = json.loads((Path(temporary) / "events.jsonl").read_text())
            finally:
                follow.STAGE = previous

        plots = {(plot["x"], plot["y"]): plot for plot in staged["plots"]}
        self.assertEqual(plots[(3, 5)]["rv"], 32)
        self.assertEqual(plots[(4, 6)]["rv"], 0)
        self.assertTrue(plots[(3, 5)]["ri"])

    def test_north_up_reflection_keeps_a_boundary_edge_from_all_six_bits(self) -> None:
        event = {
            "kind": "tiles", "turn": 7, "height": 9,
            "plots": [{"x": 3, "y": 3, "rv": 8, "ri": True}],
        }
        with tempfile.TemporaryDirectory() as temporary:
            previous = follow.STAGE
            follow.STAGE = temporary
            try:
                follow.stage_events([json.dumps(event).encode()], 9)
                staged = json.loads((Path(temporary) / "events.jsonl").read_text())
            finally:
                follow.STAGE = previous

        self.assertEqual(staged["plots"][0]["rv"], 8)
        self.assertTrue(staged["plots"][0]["ri"])

    def test_a_pending_chrome_consent_does_not_kill_the_follower(self) -> None:
        # 2026-08-14: on a first-boot host, the queued osascript call behind the
        # unanswered Automation dialog raised TimeoutExpired out of chrome() and
        # took the whole follower down. The contract is mirror_on_screen's: an
        # answer that cannot be obtained is "cannot enumerate", never a crash.
        with mock.patch.object(
            follow.subprocess, "run",
            side_effect=follow.subprocess.TimeoutExpired(cmd="osascript", timeout=30),
        ), mock.patch.object(follow, "log") as log:
            self.assertEqual(follow.chrome("tell application \"Google Chrome\" to beep"), "")
        log.assert_called_once()
        self.assertIn("consent", log.call_args.args[0])

    def test_chrome_enumeration_timeout_reads_as_cannot_enumerate(self) -> None:
        with mock.patch.object(
            follow.subprocess, "run",
        ) as run, mock.patch.object(follow, "log"):
            run.side_effect = [
                mock.Mock(returncode=0),  # pgrep: Chrome is running
                follow.subprocess.TimeoutExpired(cmd="osascript", timeout=30),
            ]
            self.assertIsNone(follow.mirror_on_screen())


class FinishedRunStaysOffTheScreen(unittest.TestCase):
    """A run that has stopped writing must not be put back on the screen.

    2026-08-17: the stale-run teardown was gated on `server_alive(PORT)` while
    the start path below it carried no staleness guard of its own. The tick
    that took a finished game down left the port free, so the next tick fell
    through and served that same finished run again — a 21 MB `civvis play`
    process spawned every ~6 seconds for as long as the seat sat between
    games. Measured on the idle science-domination-20260817T010000Z seat: the
    mirror pid advanced 38418 -> 38584 -> 38620 -> 38657 inside 20 seconds,
    and a tab on `?instance=<server_pid>` was stale the moment it loaded.
    """

    def drive_idle_seat(self, *, serving: bool, ticks: int = 10) -> dict:
        """Run `main()` over a run whose events file is older than the window."""
        seat = {"up": serving, "starts": 0, "stops": 0, "reads": 0}

        def start(_run_dir, _players) -> bool:
            seat["starts"] += 1
            seat["up"] = True
            return True

        def stop() -> bool:
            seat["stops"] += 1
            seat["up"] = False
            return True

        def read_events(_run_dir):
            seat["reads"] += 1
            return ([b"{}"], 222, 6, 38, True)  # a finished game HAS a map

        with mock.patch.object(follow, "newest_run",
                               return_value=("/run/science-domination", 0.0)), \
             mock.patch.object(follow, "ensure_on_screen", return_value=0), \
             mock.patch.object(follow, "ensure_watching", return_value=None), \
             mock.patch.object(follow, "read_events", side_effect=read_events), \
             mock.patch.object(follow, "server_alive", lambda _p: seat["up"]), \
             mock.patch.object(follow, "start_visible_server", side_effect=start), \
             mock.patch.object(follow, "stop_visible_server", side_effect=stop), \
             mock.patch.object(follow, "log", lambda _m: None), \
             mock.patch.object(follow.time, "sleep",
                               side_effect=[None] * ticks + [StopIteration]):
            with self.assertRaises(StopIteration):
                follow.main()
        return seat

    def test_a_finished_run_is_taken_down_once_and_never_re_served(self) -> None:
        seat = self.drive_idle_seat(serving=True)

        self.assertEqual(seat["stops"], 1, "take the finished game down once")
        self.assertEqual(seat["starts"], 0, "never re-serve a finished run")
        self.assertFalse(seat["up"], "the port stays free for the next attempt")

    def test_an_idle_seat_with_a_free_port_is_left_alone(self) -> None:
        """Steady state after the teardown: no start, no redundant stop."""
        seat = self.drive_idle_seat(serving=False)

        self.assertEqual(seat["starts"], 0)
        self.assertEqual(seat["stops"], 0)

    def test_an_idle_seat_does_not_reread_its_events_file(self) -> None:
        """The stale tick returns before reopening a 17 MB events.jsonl."""
        seat = self.drive_idle_seat(serving=False)

        self.assertEqual(seat["reads"], 0)

    def test_a_live_run_with_a_map_still_goes_on_screen(self) -> None:
        """The guard against over-correcting into never serving anything."""
        with mock.patch.object(follow, "newest_run",
                               return_value=("/run/live", time.time())), \
             mock.patch.object(follow, "ensure_on_screen", return_value=0), \
             mock.patch.object(follow, "ensure_watching", return_value=None), \
             mock.patch.object(follow, "read_events",
                               return_value=([b"{}"], 7, 6, 38, True)), \
             mock.patch.object(follow, "server_alive", return_value=False), \
             mock.patch.object(follow, "start_visible_server") as start, \
             mock.patch.object(follow, "log", lambda _m: None), \
             mock.patch.object(follow.time, "sleep", side_effect=[StopIteration]):
            with self.assertRaises(StopIteration):
                follow.main()

        start.assert_called_once()
