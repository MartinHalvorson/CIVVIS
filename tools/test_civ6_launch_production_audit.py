import unittest
from civ6_launch_production_audit import city_observation, summarize


class LaunchProductionAuditTests(unittest.TestCase):
    def test_regional_factory_attribution_does_not_invent_own_building(self):
        row = city_observation({"turn": 152}, {"buildings": ["BUILDING_WORKSHOP"],
            "yield_sources": {"production": "+12 from Buildings\n +6 from Factory\n +3 from Workshop"}})
        self.assertEqual(row["factory_production_in_english_ui"], 6)
        self.assertNotIn("BUILDING_FACTORY", row["buildings"])

    def test_missing_production_and_fuel_are_unknown_not_zero(self):
        row = city_observation({"turn": 1}, {})
        self.assertIsNone(row["production"])
        self.assertFalse(row["producing_observed"])
        self.assertIsNone(row["fuel_stocks"]["RESOURCE_COAL"])
        self.assertIsNone(row["factory_production_in_english_ui"])

    def test_incomplete_pad_and_duplicate_frames_do_not_move_census(self):
        def state(turn, complete, production):
            return {"turn": turn, "techs": ["TECH_ROCKETRY", "TECH_SATELLITES"],
                    "cities": [{"id": 7, "production": production,
                                "districts": [{"type": "DISTRICT_SPACEPORT", "complete": complete}]}]}
        result = summarize([state(160, False, 40), state(174, True, 46), state(174, True, 60)])
        self.assertEqual(result["first_spaceport"], {"turn": 174, "city_id": 7})
        self.assertEqual(result["launch_city_observations"][-1]["production"], 46)

    def test_no_pad_is_unknown_not_empty_launch_city(self):
        result = summarize([{"turn": 180, "cities": []}])
        self.assertIsNone(result["first_spaceport"])
        self.assertEqual(result["launch_city_observations"], [])


if __name__ == "__main__":
    unittest.main()
