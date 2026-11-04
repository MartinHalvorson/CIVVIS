"""Conservative native identity controls for the terminal cohort auditor."""
import importlib.util
import json
from pathlib import Path
import unittest

path = Path(__file__).with_name('2026-10-25-native-material-screen.audit.py')
spec = importlib.util.spec_from_file_location('material_audit', path)
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class UpgradeIdentity(unittest.TestCase):
    def test_native_successor_requires_unique_preserved_identity(self):
        old = dict(id=7, kind='old', upgrade_to='new', x=2, y=3, xp=11,
                   level=2, formation=0, promotions=['veteran'])
        successor = dict(old, id=9, kind='new')
        without = lambda row, key: {k: v for k, v in row.items() if k != key}
        cases = [
            ('replacement', [old], [successor], 'unique_exact_successor'),
            ('same ID', [old], [dict(successor, id=7)], 'same_id_exact_successor'),
            ('two candidates', [old], [successor, dict(successor, id=10)], 'ambiguous_successor'),
            ('old survives', [old], [old, successor], 'same_id_transition_unresolved'),
            ('XP changed', [old], [dict(successor, xp=12)], 'no_exact_successor'),
            ('promotions missing', [old], [without(successor, 'promotions')], 'no_exact_successor'),
            ('preexisting unit', [old, successor], [successor], 'no_exact_successor'),
            ('native block', [dict(old, upgrade_blocked_reason='not_friendly')], [successor], 'native_quote_blocked'),
            ('quote missing', [without(old, 'upgrade_to')], [successor], 'missing_native_quote'),
            ('position missing', [without(old, 'x')], [successor], 'missing_veteran_or_position'),
            ('position changed', [old], [dict(successor, x=3)], 'no_exact_successor'),
            ('formation changed', [old], [dict(successor, formation=1)], 'no_exact_successor'),
            ('promotion lost', [old], [dict(successor, promotions=[])], 'no_exact_successor'),
        ]
        for label, before, after, expected in cases:
            with self.subTest(label=label):
                self.assertEqual(audit.transition({'units': before}, {'units': after}, 7)[0], expected)

    def test_captured_read_uses_the_last_actual_native_export(self):
        first, last = dict(kind='state', turn=1), dict(kind='state', turn=2)
        data = '\n'.join(map(json.dumps, [first, dict(kind='tiles'), last, dict(kind='await')]))
        self.assertEqual(audit.last_state(data.encode()), last)

    def test_missing_and_malformed_native_exports_remain_errors(self):
        for data in [b'{"kind":"await"}', b'{incomplete']:
            with self.subTest(data=data), self.assertRaises(ValueError):
                audit.last_state(data)


if __name__ == '__main__':
    unittest.main()
