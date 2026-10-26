"""Semantic controls for the retrospective census's conservative data boundaries."""
import importlib.util
import pathlib
import unittest

path = pathlib.Path(__file__).with_name('2026-10-26-native-opening-production.analyze.py')
spec = importlib.util.spec_from_file_location('census', path)
census = importlib.util.module_from_spec(spec)
spec.loader.exec_module(census)


class Controls(unittest.TestCase):
    def city(self):
        return {'id': 7, 'x': 5, 'y': 5, 'pop': 2, 'food_surplus': 3,
                'worked': [{'x': 5, 'y': 5, 'yields': dict(zip(census.YIELDS, [2, 1, 0, 0, 0, 0]))},
                           {'x': 6, 'y': 5, 'yields': dict(zip(census.YIELDS, [2, 1, 0, 0, 0, 0]))},
                           {'x': 5, 'y': 6, 'yields': dict(zip(census.YIELDS, [0, 0, 0, 0, 0, 0]))}],
                'districts': [{'x': 5, 'y': 6, 'type': 'DISTRICT_CAMPUS'}], 'specialists': ['DISTRICT_CAMPUS']}

    def plot(self, turn=20, **changes):
        p = {'x': 4, 'y': 5, 'vis': True, 'o': 0, 'oc': 7, 'i': False,
             'flooded': False, 'submerged': False, 'yl': [2, 3, 0, 0, 0, 0]}
        p.update(changes)
        return {'kind': 'tiles', 'ctx': 'agent', 'turn': turn, 'width': 60, 'plots': [p]}

    def test_unmet_global_leader_is_not_replaced_by_known_rival(self):
        standing = census.major_standing({'local_player': 0, 'majors': 2, 'major_production': {'0': 25, '1': 30, '2': 100}})
        self.assertEqual(standing['leader'], 2)
        self.assertEqual(standing['queue_ratio'], .25)
        state = {'public_stats': {'city_count': 2, 'population': 6, 'production': 25},
                 'rivals': [{'player': 1, 'public_stats': {'city_count': 2, 'population': 5, 'production': 30}}]}
        self.assertFalse(census.public_decomposition(state, standing)['known'])
        self.assertFalse(census.major_standing({'local_player': 0, 'majors': 2, 'major_production': {'0': 25, '1': 30}})['known'])

    def test_city_yield_factorization_keeps_queue_reading_separate(self):
        standing = census.major_standing({'local_player': 0, 'majors': 1, 'major_production': {'0': 50, '1': 100}})
        state = {'public_stats': {'city_count': 2, 'population': 6, 'production': 30},
                 'rivals': [{'player': 1, 'public_stats': {'city_count': 4, 'population': 20, 'production': 80}}]}
        row = census.public_decomposition(state, standing)
        self.assertAlmostEqual(row['product'], 30 / 80)
        self.assertEqual(row['separate_turn_queue_ratio'], .5)

    def test_free_center_and_staffed_district_are_not_terrain_workers(self):
        self.assertEqual([(p['x'], p['y']) for p in census.terrain_workers(self.city())], [(6, 5)])

    def test_explicit_replacement_does_not_inherit_ownership_or_delta_freshness(self):
        events = [self.plot(10), {'kind': 'tiles_delta', 'ctx': 'agent', 'turn': 20}]
        plots, width = census.explicit_plots(events, 20)
        self.assertEqual(census.allocation_bounds(self.city(), 0, 20, plots, width)['eligible_unworked'], 0)
        replacement = self.plot(20)
        del replacement['plots'][0]['oc']
        plots, width = census.explicit_plots([self.plot(10), replacement], 20)
        self.assertNotIn('oc', plots[(4, 5)]['plot'])
        self.assertEqual(census.allocation_bounds(self.city(), 0, 20, plots, width)['eligible_unworked'], 0)

    def test_food_tradeoff_is_one_worker_bound_not_free_yield(self):
        plots, width = census.explicit_plots([self.plot(yl=[1, 4, 0, 0, 0, 0])], 20)
        row = census.allocation_bounds(self.city(), 0, 20, plots, width)
        self.assertEqual(row['pareto'], [])
        self.assertEqual(row['maximum_one_worker_food_floor_tradeoffs_production_delta'], 3)
        city = self.city(); city['food_surplus'] = 2
        self.assertEqual(census.allocation_bounds(city, 0, 20, plots, width)['food_floor_tradeoffs'], [])

    def test_future_foreign_or_nonfinite_quotes_do_not_create_opportunities(self):
        for event in [self.plot(21), self.plot(o=1), self.plot(yl=[2, float('nan'), 0, 0, 0, 0])]:
            with self.subTest(event=event):
                plots, width = census.explicit_plots([event], 20)
                row = census.allocation_bounds(self.city(), 0, 20, plots, width)
                self.assertEqual(row['pareto'], [])

    def test_primary_selection_preserves_repeats_and_uses_continuation_only_when_needed(self):
        row = lambda t, f: {'decision': {'turn': t, 'frame': f}}
        attempts = [{'ordinal': 1, 'tag': 'continued', 'decisions': [row(40, 0), row(50, 0)]},
                    {'ordinal': 0, 'tag': 'original', 'decisions': [row(40, 0), row(40, 0), row(50, 1)]}]
        chosen, alternatives = census.select_primary(attempts, 40)
        self.assertEqual((chosen[2], chosen[1]), ('original', 0))
        self.assertEqual(len(alternatives), 3)
        self.assertEqual(census.select_primary(attempts, 50)[0][2], 'continued')
        self.assertIsNone(census.select_primary(attempts, 30)[0])


if __name__ == '__main__':
    unittest.main()
