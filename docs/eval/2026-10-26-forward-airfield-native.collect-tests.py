"""Keep played arm, requested orders, host execution and terminal outcome distinct."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SOURCE = Path(__file__).with_name('2026-10-26-forward-airfield-native.collect.py')
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
            self.assertFalse(row['game_stopped_from_summary'])
            self.assertEqual(row['feature_journal_lines'],[])
            self.assertEqual(row['issued_support_stage_orders'],[])
            self.assertEqual(row['construction_attribution_status'],'not_verified_by_this_collector')

    def test_requests_remain_requests_and_later_frames_supply_maximums(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'summary.json').write_text(json.dumps({'game_stopped':True,'outcome':{'won':False}}))
            (root/'why.log').write_text('[why] '+collector.GENE+': proposal\n')
            (root/'decisions.jsonl').write_text(json.dumps({'decision':{'turn':20,'frame':0},'orders':[{'kind':'produce','verb':'DISTRICT_ENCAMPMENT','subject':7}]})+'\n')
            bomber={'class':'PROMOTION_CLASS_AIR_BOMBER'}
            events=[{'kind':'state','turn':20,'frame':0,'units':[bomber]},
                    {'kind':'state','turn':20,'frame':1,'units':[bomber]*4,'cities':[{'id':7,'x':4,'y':5,'districts':[{'type':'DISTRICT_ENCAMPMENT','complete':False}],'buildable':[{'t':'BUILDING_ARMORY','p':3,'n':11}]}]},
                    {'kind':'tiles','turn':20,'plots':[{'x':4,'y':6,'im':'IMPROVEMENT_AIRSTRIP','p':False,'o':-1,'io':0}]},
                    {'kind':'tiles_delta','turn':21,'plots':[{'x':4,'y':6,'im':'IMPROVEMENT_AIRSTRIP'}]}]
            (root/'events.jsonl').write_text(''.join(json.dumps(e)+'\n' for e in events))
            row=collector.attempt(root,lambda path: {'treatments':[collector.GENE]})
            self.assertTrue(row['game_stopped_from_summary'])
            self.assertEqual(len(row['issued_support_stage_orders']),1)
            self.assertEqual(row['maximum_observed_bombers'],4)
            self.assertEqual(len(row['native_support_observations']),1)
            quote=row['native_support_observations'][0]['cities'][0]['offered_stages'][0]
            self.assertEqual(quote,{'t':'BUILDING_ARMORY','p':3,'n':11})
            self.assertEqual(len(row['native_airstrip_observations']),1)
            self.assertFalse(row['native_support_observations'][0]['cities'][0]['encampments'][0]['complete'])
            self.assertEqual(row['construction_attribution_status'],'not_verified_by_this_collector')

    def test_malformed_live_rows_and_summary_remain_counted_unknown(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'summary.json').write_text('{partial')
            (root/'events.jsonl').write_text('{partial\n')
            row=collector.attempt(root,lambda path: None)
            self.assertEqual(row['malformed_rows']['summary'],1)
            self.assertEqual(row['malformed_rows']['events'],1)
            self.assertFalse(row['game_stopped_from_summary'])

    def test_engineer_requests_need_the_unique_matching_native_frame(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'decisions.jsonl').write_text(''.join(json.dumps({'decision':{'turn':20,'frame':f},'orders':[{'kind':'unit','subject':9,'verb':'MOVE_TO'}]})+'\n' for f in [0,1,2]))
            states=[{'kind':'state','turn':20,'frame':0,'units':[{'id':9,'kind':'UNIT_BUILDER'}]},
                    {'kind':'state','turn':20,'frame':1,'units':[{'id':9,'kind':'UNIT_MILITARY_ENGINEER'}]},
                    {'kind':'state','turn':20,'frame':2,'units':[{'id':9,'kind':'UNIT_MILITARY_ENGINEER'}]}]
            states.append(states[-1])
            (root/'events.jsonl').write_text(''.join(json.dumps(s)+'\n' for s in states))
            row=collector.attempt(root,lambda path: None)
            self.assertEqual([v['frame'] for v in row['issued_engineer_orders']],[1])
            self.assertEqual(row['construction_attribution_status'],'not_verified_by_this_collector')

    def test_retirement_and_rival_defeat_are_not_local_host_outcomes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'summary.json').write_text(json.dumps({'game_stopped':True,'reason':'operator_retired','outcome':None}))
            rival={'kind':'defeat','turn':52,'player':4,'ours':False}
            (root/'events.jsonl').write_text(json.dumps(rival)+'\n')
            row=collector.attempt(root,lambda path: None)
            self.assertTrue(row['game_stopped_from_summary'])
            self.assertEqual(row['observed_host_outcome_events'],[])
            local={'kind':'defeat','turn':53,'player':0,'ours':True}
            (root/'events.jsonl').write_text(json.dumps(rival)+'\n'+json.dumps(local)+'\n')
            row=collector.attempt(root,lambda path: None)
            self.assertEqual(row['observed_host_outcome_events'],[local])

    def test_another_trial_or_replaced_cohort_is_rejected(self):
        for state in [{'gene':'other','limit':48,'assigned':[]},
                      {'gene':collector.GENE,'limit':8,'assigned':[]},
                      {'gene':collector.GENE,'limit':48,'assigned':['same','same']}]:
            with self.subTest(state=state),self.assertRaises(ValueError):
                collector.validate_assignment(state)


if __name__=='__main__':
    unittest.main()
