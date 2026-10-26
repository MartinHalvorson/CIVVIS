"""Prevent invented next-stage permissions, timing, and unknown-as-zero budgets."""
import importlib.util
from pathlib import Path
import unittest

SOURCE = Path(__file__).with_name('2026-10-26-forward-airfield-preparation.py')
SPEC = importlib.util.spec_from_file_location('preparation', SOURCE)
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)


class PreparationControls(unittest.TestCase):
    def city(self, **changes):
        return dict(id=1, name='fixture', x=3, y=4, **changes)

    def test_district_turns_use_p_and_never_the_placement_count(self):
        city=self.city(buildable=[dict(t='DISTRICT_ENCAMPMENT', c=100, p=1, n=10)], purchasable=[])
        row=audit.city_offer(city,[],60)
        self.assertEqual(row['most_advanced_offered_stage'],'encampment')
        self.assertEqual(row['minimum_quoted_production_turns_for_any_offered_stage'],1)

    def test_missing_and_null_menus_are_unknown_not_zero_turns(self):
        for city in [self.city(),self.city(buildable=None,purchasable=None),
                     self.city(buildable=[dict(t='BUILDING_ARMORY',c=98,p=-1)],purchasable=[])]:
            row=audit.city_offer(city,[],60)
            self.assertIsNone(row['minimum_quoted_production_turns_for_any_offered_stage'])
        self.assertEqual(audit.city_offer(self.city(),[],60)['most_advanced_offered_stage'],'menus_unknown')

    def test_an_existing_armory_cannot_invent_an_engineer_offer(self):
        city=self.city(buildings=['BUILDING_ARMORY'],buildable=[],purchasable=[])
        self.assertEqual(audit.city_offer(city,[],60)['most_advanced_offered_stage'],'no_offered_stage')
        city['buildable']=[dict(t='UNIT_MILITARY_ENGINEER',c=85,p=2)]
        self.assertEqual(audit.city_offer(city,[],60)['most_advanced_offered_stage'],'engineer')

    def test_missing_income_and_technology_remain_unknown(self):
        row=dict(turn=1,frame=0,capital={},holder=1,sites={})
        result=audit.describe({'cities':[],'rivals':[],'techs':None},row,60)
        self.assertIsNone(result['positive_income'])
        self.assertIsNone(result['military_engineering_known'])
        result=audit.describe({'cities':[],'rivals':[],'gold_per_turn':-3,'techs':[]},row,60)
        self.assertFalse(result['positive_income'])
        self.assertFalse(result['military_engineering_known'])


if __name__=='__main__':
    unittest.main()
