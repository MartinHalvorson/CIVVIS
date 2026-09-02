"""The evacuation lands: a MOVE_TO the host accepts and never walks is named
and answered in the same pass (`CivvisBoard.moveNoop` / `fallbackStep`).

These read the mod source the way the other mod suites do — the file only
runs inside Civilization VI — and pin the three facts that matter: the hook
sits BEFORE the silent watch drop, the fallback is taken on both the refused
leg and the accepted-but-unwalked leg, and `MoveFallback = false` keeps every
part of it off so a run can be compared against one that had it.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import shutil
import subprocess
import sys
import unittest
from types import SimpleNamespace

ROOT = pathlib.Path(__file__).resolve().parents[1]
AGENT = ROOT / "tools/civ6_control/mod/CivvisControlAgent.lua"
sys.path.insert(0, str(ROOT / "tools"))

import civ6_play  # noqa: E402


class MoveFallbackLuaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = AGENT.read_text(encoding="utf-8")

    def test_the_lua_still_parses(self) -> None:
        luac = shutil.which("luac")
        if luac is None:
            self.skipTest("luac not installed")
        result = subprocess.run([luac, "-p", str(AGENT)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_the_noop_is_answered_before_the_watch_is_dropped(self) -> None:
        drain = self.source[self.source.index("CivvisQueue.drain = function(player, pid, turn)"):]
        drain = drain[: drain.index("CivvisQueue.giveUp = function(turn)")]
        hook = drain.index("CivvisBoard.moveNoop(player, pid, subject, unit, entry, turn, ux, uy, moves)")
        drop = drain.index("elseif ready and CivvisQueue.dropWatch(subject, entry) then")
        self.assertLess(hook, drop, "the no-op must be named before the watch is dropped in silence")
        # Only a leg that was accepted, not arrived, not spent, and still on
        # its origin plot is a no-op; anything else is the host walking it.
        self.assertIn(
            "local noop = ready and entry.expect ~= nil and not arrived and not spent and atOrigin;",
            drain,
        )
        self.assertIn("local atOrigin = entry.origin ~= nil and ux == entry.origin.x and uy == entry.origin.y;", drain)

    def test_a_refused_leg_takes_the_fallback_in_the_same_pass(self) -> None:
        branch = self.source[self.source.index('if verb == "MOVE_TO" or verb == "ATTACK" or verb == "CAPTURE" then'):]
        branch = branch[: branch.index('if verb == "RANGE_ATTACK" then')]
        refused = branch.index('emit("move_refused", {')
        fallback = branch.index("CivvisBoard.fallbackStep(player, pid, unit, subject,")
        self.assertLess(refused, fallback, "the ledger's refusal is emitted first, then the unit is answered")
        self.assertIn('fromX, fromY, x, y, turn, "cannot_start");', branch)
        # The queue expects the plot actually sent, exactly as `move_capped` does.
        self.assertIn("row.x, row.y = sent.x, sent.y;", branch)
        # And the attempt records the plot and movement the leg left from.
        self.assertIn("CivvisBoard.noteMoveAttempt(subject, turn, fromX, fromY, x, y, movesBefore);", branch)
        self.assertLess(
            branch.index("local movesBefore = tonumber(try(function() return unit:GetMovesRemaining(); end, nil));"),
            branch.index('local moved = operate(unit, OP["UNITOPERATION_MOVE_TO"], params);'),
            "movement is read before the request, or a walked leg reads as a no-op",
        )

    def test_the_switch_keeps_every_part_off(self) -> None:
        for fn in ("CivvisBoard.fallbackStep = function", "CivvisBoard.moveNoop = function"):
            body = self.source[self.source.index(fn):]
            body = body[: body.index("\nend;")]
            self.assertIn("if cfg.MoveFallback == false then return", body, fn)

    def test_the_host_is_asked_why_and_the_answer_is_one_of_a_fixed_set(self) -> None:
        body = self.source[self.source.index("CivvisBoard.classifyNoop = function"):]
        body = body[: body.index("\nend;")]
        for why in ("no_moves", "cannot_start", "no_path", "beyond_turn", "occupied",
                    "hostile_on_plot", "zoc", "hostile_adjacent", "unknown"):
            self.assertIn(f'return "{why}"', body)
        # Each answer is the host's, not a guess from the mirror.
        self.assertIn('canOperate(unit, OP["UNITOPERATION_MOVE_TO"], params)', body)
        self.assertIn("UnitManager.GetMoveToPathEx(unit, destination)", body)

    def test_the_fallback_never_retries_the_failed_plot_or_walks_into_a_unit(self) -> None:
        body = self.source[self.source.index("CivvisBoard.fallbackStep = function"):]
        body = body[: body.index("\nend;")]
        self.assertIn("not (px == wantX and py == wantY)", body)
        self.assertIn("if not hostile[key] and not stacked then", body)
        self.assertIn('if canOperate(unit, OP["UNITOPERATION_MOVE_TO"], params) then', body)
        # Once per unit per turn: a second no-op after a fallback is only named.
        self.assertIn("if attempt ~= nil and attempt.turn == turn and attempt.fallback == true then return nil; end", body)
        ranking = self.source[self.source.index("CivvisBoard.fallbackBetter = function"):]
        ranking = ranking[: ranking.index("\nend;")]
        self.assertLess(ranking.index("dc < db"), ranking.index("candidate.exposed < best.exposed"),
                        "closer to the destination first, then fewer hostile neighbours")

    def test_both_answers_are_events_the_rust_side_reads(self) -> None:
        self.assertIn('emit("move_noop", {', self.source)
        self.assertIn('emit("move_fallback", {', self.source)
        for event in ('emit("move_noop", {', 'emit("move_fallback", {'):
            payload = self.source[self.source.index(event):]
            payload = payload[: payload.index("});")]
            self.assertIsNone(
                re.search(r"(?<![_a-z])kind\s*=", payload),
                "`emit` sets `kind`; a payload field of that name is clobbered",
            )
        orders = self.source[self.source.index("move_no_reach = CivvisBoard.stats.no_reach,"):]
        orders = orders[:200]
        self.assertIn("move_noop = CivvisBoard.stats.move_noop,", orders)
        self.assertIn("move_fallback = CivvisBoard.stats.move_fallback,", orders)
        rust = (ROOT / "src/bin/civvis_orders.rs").read_text(encoding="utf-8")
        kinds = rust[rust.index("const EVIDENCE_KINDS: &[&str] = &["):]
        kinds = kinds[: kinds.index("];")]
        self.assertIn('"move_noop"', kinds)
        self.assertIn('"move_fallback"', kinds)

    def test_the_per_turn_attempt_table_is_reset_with_the_board(self) -> None:
        reset = self.source[self.source.index("CivvisBoard.reset = function()"):]
        reset = reset[: reset.index("\nend;")]
        self.assertIn("CivvisBoard.moveAttempts = {};", reset)
        self.assertIn("move_noop = 0, move_fallback = 0", reset)


class MoveFallbackConfigTests(unittest.TestCase):
    def _config(self, **changes):
        class Defaults(SimpleNamespace):
            def __getattr__(self, name):
                return None

        return civ6_play.build_config(
            Defaults(tag="t", game_mode=[],
                     difficulty="DIFFICULTY_SETTLER", map_size="MAPSIZE_SMALL",
                     speed="GAMESPEED_ONLINE", map="Continents.lua",
                     leader="LEADER_TRAJAN", **changes))

    def test_the_switch_reaches_the_baked_mod_config(self) -> None:
        self.assertIs(self._config(move_fallback=True)["MoveFallback"], True)
        self.assertIs(self._config(move_fallback=False)["MoveFallback"], False)

    def test_the_switch_is_on_by_default_and_withholdable(self) -> None:
        parser = argparse.ArgumentParser()
        source = pathlib.Path(civ6_play.__file__).read_text(encoding="utf-8")
        self.assertIn('ap.add_argument("--no-move-fallback", dest="move_fallback",', source)
        # The arm is recorded on the run summary beside the other mod switches.
        arms = source[source.index('"mod_arms": {'):]
        arms = arms[: arms.index("},")]
        self.assertIn('"MoveFallback": args.move_fallback,', arms)
        del parser


if __name__ == "__main__":
    unittest.main()
