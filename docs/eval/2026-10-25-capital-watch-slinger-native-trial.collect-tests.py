"""Keep played arm, requested orders, host execution and terminal outcome distinct."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SOURCE = Path(__file__).with_name('2026-10-25-capital-watch-slinger-native-trial.collect.py')
SPEC = importlib.util.spec_from_file_location('collector', SOURCE)
collector = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(collector)


class CollectorControls(unittest.TestCase):
    def test_on_genome_alone_is_not_activation_or_terminal_execution(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'why.log').write_text('[genome] '+collector.GENE+'\n')
            (root/'decisions.jsonl').write_text(json.dumps({'decision': {'turn': 1, 'native_actions': [{'type': 'upgrade_unit'}]}, 'orders': []})+'\n')
            row=collector.attempt(root,lambda path: {'treatments':[collector.GENE]})
            self.assertTrue(row['played_candidate'])
            self.assertFalse(row['terminal_from_summary'])
            self.assertEqual(row['feature_journal_lines'],[])
            self.assertEqual(row['issued_upgrade_orders'],[])
            self.assertEqual(row['upgrade_execution_status'],'not_observed_by_this_collector')

    def test_requests_remain_requests_and_later_frames_supply_maximums(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'summary.json').write_text(json.dumps({'game_stopped':True,'outcome':{'won':False}}))
            (root/'why.log').write_text('[why] '+collector.GENE+': proposal\n')
            (root/'decisions.jsonl').write_text(json.dumps({'decision':{'turn':20,'frame':0},'orders':[{'kind':'unit','verb':'UPGRADE','subject':7}]})+'\n')
            bomber={'class':'PROMOTION_CLASS_AIR_BOMBER'}
            events=[{'kind':'state','turn':20,'frame':0,'units':[bomber]},
                    {'kind':'state','turn':20,'frame':1,'units':[bomber]*4}]
            (root/'events.jsonl').write_text(''.join(json.dumps(e)+'\n' for e in events))
            row=collector.attempt(root,lambda path: {'treatments':[collector.GENE]})
            self.assertTrue(row['terminal_from_summary'])
            self.assertEqual(len(row['issued_upgrade_orders']),1)
            self.assertEqual(row['maximum_observed_bombers'],4)
            self.assertEqual(row['upgrade_execution_status'],'not_observed_by_this_collector')

    def test_malformed_live_rows_and_summary_remain_counted_unknown(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'summary.json').write_text('{partial')
            (root/'events.jsonl').write_text('{partial\n')
            row=collector.attempt(root,lambda path: None)
            self.assertEqual(row['malformed_rows']['summary'],1)
            self.assertEqual(row['malformed_rows']['events'],1)
            self.assertFalse(row['terminal_from_summary'])

    def test_another_trial_or_replaced_cohort_is_rejected(self):
        for state in [{'gene':'other','limit':16,'assigned':[]},
                      {'gene':collector.GENE,'limit':8,'assigned':[]},
                      {'gene':collector.GENE,'limit':16,'assigned':['same','same']}]:
            with self.subTest(state=state),self.assertRaises(ValueError):
                collector.validate_assignment(state)


if __name__=='__main__':
    unittest.main()
