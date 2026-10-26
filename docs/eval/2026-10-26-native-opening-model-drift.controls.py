"""Controls for passive raw-versus-corrected comparison and unknown handling."""
import copy
import importlib.util
import pathlib
import unittest

path = pathlib.Path(__file__).with_name('2026-10-26-native-opening-model-drift.audit.py')
spec = importlib.util.spec_from_file_location('audit', path)
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class Controls(unittest.TestCase):
    def sample(self):
        host = {'x': 4, 'y': 5, 'yields': {'food': 5, 'production': 3}, 'housing': 4,
                'amenities': 0, 'worked': [], 'districts': []}
        model = {'x': 4, 'y': 5, 'model_yields': {'food': 2, 'production': 3},
                 'yields': {'food': 5, 'production': 3}, 'model_housing': 4.5,
                 'model_amenities': 0, 'worked': []}
        return host, model

    def test_corrected_agreement_does_not_hide_raw_drift(self):
        h, m = self.sample(); row = audit.compare({'cities': [h]}, {'cities': [m]})[0]
        self.assertEqual(row['raw_model_yield_delta']['food']['delta'], -3)
        self.assertTrue(row['raw_model_yield_delta']['food']['material'])
        self.assertFalse(row['corrected_display_delta']['food']['material'])

    def test_missing_and_duplicate_coordinates_do_not_become_zero(self):
        h, m = self.sample(); m['x'] = 6
        rows = audit.compare({'cities': [h]}, {'cities': [m]})
        self.assertEqual(len(rows), 2); self.assertTrue(all(not r['known'] for r in rows))
        self.assertFalse(audit.compare({'cities': [h, h]}, {'cities': []})[0]['known'])

    def test_nonfinite_missing_and_boolean_yields_are_unknown(self):
        for v in [None, float('nan'), True]:
            self.assertFalse(audit.delta(v, 0)['known'])
        self.assertFalse(audit.delta(0, None)['known'])

    def test_fractional_housing_is_retained_separately_from_display_floor(self):
        h, m = self.sample(); row = audit.compare({'cities': [h]}, {'cities': [m]})[0]
        self.assertEqual(row['housing']['delta'], .5)
        self.assertEqual(row['housing']['display_floor_delta'], 0)
        h['housing'] = 4.5; row = audit.compare({'cities': [h]}, {'cities': [m]})[0]
        self.assertEqual(row['housing']['delta'], 0)
        self.assertIsNone(row['housing']['display_floor_delta'])

    def test_minus_one_accessor_is_ambiguous_but_other_negative_amenities_remain(self):
        h, m = self.sample(); h['amenities'] = -1
        self.assertFalse(audit.compare({'cities': [h]}, {'cities': [m]})[0]['amenities']['known'])
        h['amenities'] = -2; m['model_amenities'] = -3
        self.assertEqual(audit.compare({'cities': [h]}, {'cities': [m]})[0]['amenities']['delta'], -1)

    def test_each_variant_remains_a_separate_comparison(self):
        h, m = self.sample(); other = copy.deepcopy(m); other['model_yields']['food'] = 5
        rows = [audit.compare({'cities': [h]}, {'cities': [v]})[0] for v in [m, other]]
        self.assertEqual([r['raw_model_yield_delta']['food']['delta'] for r in rows], [-3, 0])


if __name__ == '__main__':
    unittest.main()
