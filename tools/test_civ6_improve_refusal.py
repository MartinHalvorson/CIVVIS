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
        cls.handler = source[start : source.index('emit("improve_refused"', start) + 400]

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

    def test_the_operation_target_still_defaults_to_where_the_builder_stands(self) -> None:
        # The payload is only correct because PARAM_X/PARAM_Y are already the
        # order's target with the unit's tile as fallback. If that ever stops
        # being true the emit above starts lying again.
        params = self.handler[: self.handler.index('emit("improve_refused"')]
        assign = params.index("params[UnitOperationTypes.PARAM_X]")
        self.assertIn("x or", params[assign : assign + 120])


if __name__ == "__main__":
    unittest.main()
