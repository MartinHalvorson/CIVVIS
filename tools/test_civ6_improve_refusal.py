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

    def test_the_refusal_records_the_movement_that_actually_explains_it(self) -> None:
        """The disagreement between the two gates was the builder's movement.

        On `civvis-20260811T103914Z`, across all 26 refusals: the builder stood
        on the ordered tile 26 of 26, had build charges every time, and had
        `movesRemaining == 0` on 25 of 26. A Civilization VI Builder needs
        movement left to place an improvement, so `canOperate` was right and the
        5-arg probe — `plots = nil` — answers a weaker question that ignores it.

        `why: can_start=true` therefore invites the reader to conclude the
        harness refuses legal work, which is the opposite of the truth. A ledger
        that misleads is worse than one that is silent, so the cause goes in.
        """
        handler = self.handler
        self.assertIn("local moves = try(function() return unit:GetMovesRemaining(); end, -1);",
                      handler)
        self.assertIn("moves = moves, charges = charges,", handler)

        # The ungated experiment from #1547 is removed, not left vestigial: the
        # run data answered its question before it ever fired.
        self.assertNotIn('emit("improve_ungated"', handler)
        self.assertNotIn("UnitManager.RequestOperation(unit,", handler)

    def test_the_refusal_reads_the_tile_at_the_moment_it_is_refused(self) -> None:
        """⚠⚠⚠ READ AT THE DECISION POINT — a later join is not a measurement.

        #1557 reopened this: the builder has movement, has charges, and stands on
        the ordered tile, and `canOperate` still refuses. Two ordinary
        explanations remain — the tile is not ours, or it is already improved —
        and neither was in the record.

        I tried to answer the ownership half from the periodic tile export and it
        cannot be done: 23 of 25 refused tiles appear there as BOTH unowned and
        ours at different points in the same run. That is the same mistake that
        produced a false movement measurement and three PRs resting on it. So the
        readings are taken here, and the test pins that they are taken from the
        plot the ORDER named rather than wherever the builder ended up.
        """
        handler = self.handler
        self.assertIn("Map.GetPlot(params[UnitOperationTypes.PARAM_X],", handler)
        self.assertIn("params[UnitOperationTypes.PARAM_Y]);", handler)
        self.assertIn("plot:GetOwner();", handler)
        self.assertIn("plot:GetImprovementType();", handler)

        payload = handler[handler.index('emit("improve_refused"'):]
        self.assertIn("tile_owner = tile_owner,", payload)
        self.assertIn("tile_improvement = tile_improvement,", payload)

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
