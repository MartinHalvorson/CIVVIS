#!/usr/bin/env python3
"""Structural regressions for the in-game production actuator.

The Civilization VI API is only available inside the game, so Rust replay tests cover
the decision feedback and these checks protect the Lua-side authority boundaries: a
direct order must pass Firaxis's start-now predicate, army production must be bounded,
and a saturated fallback must retain science and culture work.
"""

from __future__ import annotations

import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
AGENT = ROOT / "tools/civ6_control/mod/CivvisControlAgent.lua"


class ProductionActuatorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = AGENT.read_text(encoding="utf-8")
        start = cls.source.index("local function chooseProduction")
        end = cls.source.index("-- ★★★★★ WHERE A DISTRICT GOES", start)
        cls.choose = cls.source[start:end]
        direct = cls.source.index('if kind == "produce" then')
        cls.direct = cls.source[direct:]
        purchase = cls.source.index(
            'if kind == "purchase" or kind == "purchase_faith" then'
        )
        purchase_end = cls.source.index('if kind == "unit" then', purchase)
        cls.purchase = cls.source[purchase:purchase_end]

    def test_direct_orders_are_gated_before_they_are_requested(self) -> None:
        predicate = self.direct.index("CanProduce(row2.Hash, false, true)")
        request = self.direct.index("CityManager.RequestOperation", predicate)
        rejection = self.direct.index('emit("civvis_build_unplayable"', predicate)
        self.assertLess(predicate, rejection)
        self.assertLess(rejection, request)

    def test_army_rung_is_bounded_and_era_proof(self) -> None:
        self.assertRegex(
            self.choose,
            re.compile(
                r"if counts\.military < wantArmy then\s+.*?pushLandUnits\(\"army\"\);\s+end",
                re.DOTALL,
            ),
        )
        self.assertIn('row.Domain == "DOMAIN_LAND"', self.choose)
        self.assertIn("GameInfo.Units()", self.choose)
        self.assertIn('pushRangedLandUnits("ranged")', self.choose)
        self.assertIn('(row.Bombard or 0) > 0', self.source)

    def test_saturated_floor_contains_economic_projects_not_units(self) -> None:
        floor = self.choose.index(
            'for _, name in ipairs({ "PROJECT_ENHANCE_DISTRICT_CAMPUS"'
        )
        end = self.choose.index("-- Gated exactly like", floor)
        floor_code = self.choose[floor:end]
        for project in (
            "PROJECT_ENHANCE_DISTRICT_CAMPUS",
            "PROJECT_ENHANCE_DISTRICT_THEATER",
            "PROJECT_ENHANCE_DISTRICT_COMMERCIAL_HUB",
            "PROJECT_ENHANCE_DISTRICT_HARBOR",
            "PROJECT_ENHANCE_DISTRICT_INDUSTRIAL_ZONE",
        ):
            self.assertIn(project, floor_code)
        self.assertNotIn('"UNIT_', floor_code)

    def test_development_ladder_can_finish_science_and_culture_tiers(self) -> None:
        for item in (
            "BUILDING_LIBRARY",
            "BUILDING_UNIVERSITY",
            "BUILDING_RESEARCH_LAB",
            "BUILDING_AMPHITHEATER",
            "BUILDING_ART_MUSEUM",
            "BUILDING_ARCHAEOLOGICAL_MUSEUM",
            "BUILDING_BROADCAST_CENTER",
        ):
            self.assertIn(f'"{item}"', self.choose)

    def test_civilian_purchase_does_not_carry_a_military_formation(self) -> None:
        self.assertIn(
            "formationForCost = MilitaryFormationTypes.STANDARD_MILITARY_FORMATION",
            self.purchase,
        )
        self.assertRegex(
            self.purchase,
            re.compile(
                r"local militaryFormation = unitRow ~= nil.*?"
                r"if formation == 0 and militaryFormation then.*?"
                r"PARAM_MILITARY_FORMATION_TYPE\] = formationForCost",
                re.DOTALL,
            ),
        )
        self.assertRegex(
            self.purchase,
            re.compile(
                r"if formation == 1 then.*?PARAM_MILITARY_FORMATION_TYPE.*?"
                r"elseif formation == 2 then.*?PARAM_MILITARY_FORMATION_TYPE",
                re.DOTALL,
            ),
        )

    def test_purchase_refusal_is_structured_feedback(self) -> None:
        event = self.purchase.index('emit("purchase_refused"')
        rejection = self.purchase.index('return false, "cannot_buy_"', event)
        self.assertLess(event, rejection)
        for field in (
            "turn = turn",
            "city = subject",
            "item = resolved",
            "currency = yieldName",
            "cost = cost",
        ):
            self.assertIn(field, self.purchase[event:rejection])


if __name__ == "__main__":
    unittest.main()
