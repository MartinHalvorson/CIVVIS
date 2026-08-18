#!/usr/bin/env python3
"""Structural regressions for the in-game production actuator.

The Civilization VI API is only available inside the game, so Rust replay tests cover
the decision feedback and these checks protect the Lua-side authority boundaries: a
direct order must pass Firaxis's start-now predicate, army production must be bounded,
and a saturated fallback must retain science and culture work.
"""

from __future__ import annotations

import json
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

    def test_next_build_lease_is_deferred_until_the_blocker(self) -> None:
        start = self.source.index('if kind == "produce_next" then')
        end = self.source.index('if kind == "produce" then', start)
        hint = self.source[start:end]
        self.assertIn('civvisBuild[tostring(cityId) .. ":next"] = resolved', hint)
        self.assertIn('emit("build_hint"', hint)
        self.assertNotIn("CityManager.RequestOperation", hint)
        self.assertRegex(
            self.choose,
            r'civvisBuild\[cityId\]\s*\n\s*or civvisBuild\[tostring\(cityId\) .. ":next"\]',
        )
        self.assertNotIn("civvisNextBuild", self.source)

    def test_next_build_lease_is_not_counted_as_host_applied_rate(self) -> None:
        apply_orders = self.source.split("local function applyOrders", 1)[1]
        self.assertIn('kind == "produce_next"', apply_orders)
        self.assertRegex(
            apply_orders,
            re.compile(
                r'if kind == "produce_next" then.*?deferred = deferred \+ 1;.*?'
                r'else\s+applied = applied \+ 1;',
                re.DOTALL,
            ),
        )
        self.assertIn("seen = #rows - deferred", apply_orders)
        self.assertIn("orders_deferred = deferred", apply_orders)

    def test_next_build_lease_is_consumed_after_a_successful_start(self) -> None:
        consume = self.source.index('if civvisBuild[tostring(cityId) .. ":next"] == name then')
        request = self.source.rfind("CityManager.RequestOperation", 0, consume)
        self.assertLess(request, consume)
        self.assertIn('civvisBuild[tostring(cityId) .. ":next"] = nil', self.source[consume:])

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
        """⚠ THE MUSEUMS ARE `MUSEUM_ART`/`MUSEUM_ARTIFACT`, SUBJECT-LAST.

        This test spent since #782 asserting `BUILDING_ART_MUSEUM` and
        `BUILDING_ARCHAEOLOGICAL_MUSEUM`, which are not names Civilization VI
        has ever had. #782 corrected the ladder and the test kept pinning the
        invented spellings — red, but in a suite the gate does not run, so the
        one check standing guard over the museum rungs was itself broken for
        six days. `test_every_ladder_type_name_is_one_the_game_has` below is
        the general form of the defect, and would have caught both.
        """
        for item in (
            "BUILDING_LIBRARY",
            "BUILDING_UNIVERSITY",
            "BUILDING_RESEARCH_LAB",
            "BUILDING_AMPHITHEATER",
            "BUILDING_MUSEUM_ART",
            "BUILDING_MUSEUM_ARTIFACT",
            "BUILDING_BROADCAST_CENTER",
        ):
            self.assertIn(f'"{item}"', self.choose)

    def test_every_ladder_type_name_is_one_the_game_has(self) -> None:
        """A rung the game cannot resolve is invisible: the ladder moves on.

        That is the property that let a misspelled museum sit in the list
        across 50 live runs while museums stood in 0 of 119 end-of-game cities
        — nothing errors, the entry simply never fires. `data/civ6_type_names.json`
        is the 529 real names extracted from the game, so every quoted type in
        the whole agent can be checked against it directly.
        """
        real = json.loads((ROOT / "data/civ6_type_names.json").read_text(encoding="utf-8"))
        known = set(real) if isinstance(real, list) else {
            name for value in real.values()
            for name in ([value] if isinstance(value, str) else value)
        }
        quoted = set(re.findall(
            r'"((?:BUILDING|UNIT|DISTRICT|PROJECT)_[A-Z0-9_]+)"', self.source))
        self.assertTrue(quoted, "the agent should name types the game can resolve")
        invented = sorted(name for name in quoted if name not in known)
        self.assertEqual(
            invented, [],
            "these are not Civilization VI type names, so the rungs naming them "
            f"can never fire: {invented}")

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

    def test_standard_unit_eligibility_omits_the_request_formation(self) -> None:
        self.assertRegex(
            self.purchase,
            re.compile(
                r"local eligibilityParams = params;.*?"
                r'if row2\.Kind == "KIND_UNIT" and \(tonumber\(x\) or 0\) == 0.*?'
                r"key ~= CityCommandTypes\.PARAM_MILITARY_FORMATION_TYPE.*?"
                r"eligibilityParams\[key\] = value",
                re.DOTALL,
            ),
        )
        predicate = self.purchase.index(
            "false, eligibilityParams, true"
        )
        request = self.purchase.index(
            "CityManager.RequestCommand(city, CityCommandTypes.PURCHASE, params)",
            predicate,
        )
        self.assertLess(predicate, request)

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
