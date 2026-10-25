"""Controls for ownership/terrain classification and fresh Native rosters."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SOURCE = Path(__file__).with_name('2026-10-25-forward-airstrip-census.py')
SPEC = importlib.util.spec_from_file_location('airstrip_census', SOURCE)
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)


class CensusControls(unittest.TestCase):
    def plot(self, **changes):
        return dict(x=3, y=5, o=-1, vis=True, t='TERRAIN_PLAINS', w=False, i=False, **changes)

    def test_owned_neutral_foreign_and_unknown_remain_distinct(self):
        for owner, expected in [(0, 'owned_immediate'), (-1, 'neutral_immediate'),
                                (1, None), (None, 'unknown_immediate')]:
            plot = self.plot()
            plot['o'] = owner
            self.assertEqual(audit.terrain_category(plot, 0), expected)
        plot = self.plot()
        del plot['o']
        self.assertEqual(audit.terrain_category(plot, 0), 'unknown_immediate')

    def test_clearance_and_all_exclusions_are_not_immediate_sites(self):
        self.assertEqual(audit.terrain_category(self.plot(f='FEATURE_VOLCANIC_SOIL'), 0), 'neutral_immediate')
        for feature in audit.CLEAR:
            self.assertEqual(audit.terrain_category(self.plot(f=feature), 0), 'neutral_needs_clearance')
        for field, value in [('f', 'FEATURE_FLOODPLAINS'), ('vis', False),
                             ('t', 'TERRAIN_PLAINS_HILLS'), ('w', True), ('i', True),
                             ('r', 'RESOURCE_IRON'), ('d', 'DISTRICT_CITY_CENTER'),
                             ('im', 'IMPROVEMENT_FARM'), ('wo', 'WONDER_X'),
                             ('flooded', True), ('submerged', True)]:
            with self.subTest(field=field):
                plot = self.plot()
                plot[field] = value
                self.assertIsNone(audit.terrain_category(plot, 0))

    def test_current_major_holder_does_not_promote_a_city_state_founder(self):
        major = dict(original_capital=True, original_owner=1)
        minor = dict(original_capital=True, original_owner=8)
        state = dict(rivals=[dict(player=2, cities=[major, minor])])
        self.assertEqual(audit.major_capitals(state, {0, 1, 2}), [(state['rivals'][0], major)])
        self.assertEqual(audit.major_capitals(dict(rivals=[dict(player=2, cities=[])]), {0, 1, 2}), [])

    def test_armory_and_technology_do_not_invent_a_native_engineer_offer(self):
        city = dict(id=1, buildings=['BUILDING_ARMORY'], buildable=[], purchasable=[])
        state = dict(cities=[city], techs=['TECH_FLIGHT', 'TECH_MILITARY_ENGINEERING'])
        row = audit.infrastructure(state)
        self.assertTrue(row['flight_known'])
        self.assertTrue(row['armory_cities'])
        self.assertEqual(row['engineer_offers'], [])
        self.assertIsNone(audit.infrastructure({})['flight_known'])
        city['buildable'] = [dict(t='UNIT_MILITARY_ENGINEER', c=85, n=3)]
        self.assertEqual(audit.infrastructure(state)['engineer_offers'][0]['offer']['c'], 85)

    def test_cylindrical_hex_geometry_matches_the_known_range_boundary(self):
        self.assertEqual(audit.legacy.distance(dict(x=37, y=15), dict(x=27, y=17), 60), 11)
        self.assertEqual(audit.legacy.distance(dict(x=0, y=0), dict(x=59, y=0), 60), 1)

    def test_whole_census_replaces_plots_and_rosters_and_excludes_unknown_owner(self):
        target = dict(id=3, x=7, y=4, original_capital=True, original_owner=1)
        wing = [dict(id=i, x=0, y=0, hp=100, moves=1, attacks_remaining=1,
                     range=10, **{'class': 'PROMOTION_CLASS_AIR_BOMBER'}) for i in (1, 2)]
        first = dict(kind='state', turn=1, frame=0, cities=[], units=wing,
                     rivals=[dict(player=1, at_war=True, cities=[target])], minors=[])
        later = dict(first, turn=2, rivals=[dict(player=1, at_war=True, cities=[])])
        unknown = dict(self.plot(), x=4, o=None)
        events = [dict(kind='tiles', width=60, plots=[dict(self.plot(), f='FEATURE_FOREST')]),
                  dict(kind='tiles', width=60, plots=[self.plot(), unknown]), first, later]
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'summary.json').write_text(json.dumps(dict(game_stopped=True,
                seat=dict(local_player=0, difficulty='CONTROL'))))
            (root / 'decisions.jsonl').write_text('')
            (root / 'events.jsonl').write_text(''.join(json.dumps(row) + '\n' for row in events))
            result = audit.census(root)
        self.assertEqual(result['counts']['war_capital_turns'], 1)
        self.assertEqual(result['counts']['blocked_capital_turns'], 1)
        row = result['blocked_capital_turns'][0]
        self.assertEqual(len(row['sites']['neutral_immediate']), 1)
        self.assertNotIn('neutral_needs_clearance', row['sites'])
        self.assertEqual(row['unknown_owner_sites_excluded'], {'unknown_immediate': 1})


if __name__ == '__main__':
    unittest.main()
