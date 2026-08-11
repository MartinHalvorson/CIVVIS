#!/usr/bin/env python3
"""Structural regression for the builder-refusal feedback.

The Civilization VI API only exists inside the game, so this checks the Lua-side
authority boundary the same way `test_civ6_production.py` does: by reading the
actuator's source.

The defect: `improve_refused` named `unit:GetX()`/`unit:GetY()` — where the builder
was standing — rather than the tile the order asked for. `x`/`y` on the order carry
the target and only fall back to the unit's own tile, so the refusal named the wrong
ground in exactly the case the feedback exists for, a builder that cannot reach its
target. `Game::valid_improvements` already returns nothing for a tile with a city on
it, so the entry changed no decision and the real tile stayed unblocked forever.

Measured on run `civvis-20260802T041527Z`: 286 refusals, 118 + 84 + 59 + 23 + 13 of
them naming the same tile (63,11) — the capital centre — across three builders.
"""

from __future__ import annotations

import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
AGENT = ROOT / "tools/civ6_control/mod/CivvisControlAgent.lua"


class ImproveRefusalTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        source = AGENT.read_text(encoding="utf-8")
        start = source.index('if verb == "IMPROVE" or string.sub(verb, 1, 8) == "IMPROVE:"')
        # ⚠ Slice to the END OF THE EMIT, not to a fixed number of characters
        # after its start. The window used to be `+ 400`, so the first field
        # added to the payload pushed `PARAM_Y` outside it and this test failed
        # on a change it was not testing. A magic length is a tripwire for
        # whoever edits next, not a bound on what is being asserted.
        emit_at = source.index('emit("improve_refused"', start)
        cls.handler = source[start : source.index("});", emit_at) + 3]

    def test_the_refusal_names_the_ordered_tile_not_the_builders_own(self) -> None:
        emit = self.handler.index('emit("improve_refused"')
        payload = self.handler[emit:]
        self.assertIn("PARAM_X", payload)
        self.assertIn("PARAM_Y", payload)
        self.assertNotIn(
            "unit:GetX()",
            payload,
            "the refusal must name the tile the order asked for; the builder's own "
            "tile is the capital centre whenever the builder is stuck, which is "
            "precisely when this feedback is needed",
        )

    def test_the_refusal_records_both_answers_the_engine_gave(self) -> None:
        """Two forms of `CanStartOperation` disagree, and only one gates the work.

        `civvis-20260811T094304Z`, the first live run on #1542, recorded
        `can_start=true,no_reasons [p4r]` on all thirteen refusals — the engine
        saying the operation CAN start at the moment we tell CIVVIS the tile is
        dead. But the probe passes a results argument and `canOperate` does not,
        and only `canOperate` decides whether the work is attempted:

            canOperate     CanStartOperation(unit, hash, nil, params)
            refusalReason  CanStartOperation(unit, hash, nil, params, ALL)

        Reaching this emit means the 4-arg form said false. Either the results
        argument changes what is tested, or the gate under-reports and this
        harness has been refusing improvements the game would have allowed.
        Recording both is what lets a live run answer that instead of another
        argument about an overload.
        """
        emit = self.handler.index('emit("improve_refused"')
        payload = self.handler[emit:]
        self.assertIn("why = why,", payload)
        self.assertIn("can_operate = canOperate(unit,", payload)
        self.assertIn('OP["UNITOPERATION_BUILD_IMPROVEMENT"],', payload)

    def test_the_last_resort_proves_acceptance_from_a_spent_charge(self) -> None:
        """The two `CanStartOperation` forms disagree 15 times out of 15.

        `canOperate` (4-arg) says false and gates the work; the 5-arg probe says
        `can_start=true`. Only reading cannot settle which is right — this file
        has been wrong three times doing exactly that — so the last resort issues
        the operation UNGATED and reads the one observable that cannot lie: a
        Builder spends a CHARGE when an improvement is placed.

        `pcall` returning true only means nothing raised, which is the trap this
        whole file is built around, so acceptance must come from the charge.
        """
        handler = self.handler
        self.assertIn("local before = try(function() return unit:GetBuildCharges(); end, -1);",
                      handler)
        self.assertIn("UnitManager.RequestOperation(unit,", handler)
        self.assertIn("if before > 0 and after >= 0 and after < before then", handler)
        self.assertIn('emit("improve_ungated"', handler)

        # It must be reported under its own name. Counting a gate bypass as
        # CIVVIS's own IMPROVE would hide it in the very ledger that exists to
        # separate the model's work from the harness's.
        self.assertIn('return true, (wanted or "IMPROVE") .. "_UNGATED";', handler)

        # And it must sit BEFORE the refusal: the whole point is that the tile
        # was about to be declared dead anyway, so trying costs nothing.
        self.assertLess(
            handler.index('emit("improve_ungated"'),
            handler.index('emit("improve_refused"'),
            "the ungated attempt must come before giving up, or it is not free",
        )

    def test_the_slice_covers_the_whole_emit(self) -> None:
        """Guards the fixture itself: a truncated window silently stops testing."""
        self.assertTrue(self.handler.rstrip().endswith("});"))
        for field in ("turn =", "unit =", "want =", "why =", "x =", "y ="):
            self.assertIn(field, self.handler[self.handler.index('emit("improve_refused"'):])

    def test_the_operation_target_still_defaults_to_where_the_builder_stands(self) -> None:
        # The payload is only correct because PARAM_X/PARAM_Y are already the
        # order's target with the unit's tile as fallback. If that ever stops
        # being true the emit above starts lying again.
        params = self.handler[: self.handler.index('emit("improve_refused"')]
        assign = params.index("params[UnitOperationTypes.PARAM_X]")
        self.assertIn("x or", params[assign : assign + 120])


if __name__ == "__main__":
    unittest.main()
