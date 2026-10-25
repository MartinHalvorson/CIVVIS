"""Controls for census identity, observation replacement and geometry."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('audit', Path(__file__).with_name('2026-10-25-capital-assault-audit.py'))
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


def state(turn, cities):
    return dict(kind='state', turn=turn, frame=0, units=[], cities=[],
                rivals=[dict(player=1, at_war=True, cities=cities)], minors=[dict(player=4)])


def capital(original_owner=1):
    return dict(id=1, x=7, y=4, name='target', original_capital=True,
                original_owner=original_owner, capital=True)


class CensusControls(unittest.TestCase):
    def run_census(self, events):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'summary.json').write_text(json.dumps(dict(game_stopped=True,
                seat=dict(local_player=0, difficulty='CONTROL'))))
            (root / 'decisions.jsonl').write_text('')
            events.insert(0, dict(kind='tiles', width=60, plots=[]))
            (root / 'events.jsonl').write_text(''.join(json.dumps(e) + '\n' for e in events))
            return audit.census(root)

    def test_native_offset_and_cylindrical_geometry(self):
        self.assertEqual(audit.distance(dict(x=37,y=15),dict(x=27,y=17),60),11)
        self.assertEqual(audit.distance(dict(x=59,y=0),dict(x=0,y=0),60),1)
        self.assertEqual(audit.distance(dict(x=3,y=3),dict(x=3,y=2),60),1)

    def test_a_razed_city_does_not_survive_in_a_later_roster(self):
        result = self.run_census([state(1,[capital()]),state(2,[])])
        self.assertEqual([r['turn'] for r in result['capital_turns']],[1])

    def test_captured_city_state_capital_is_not_a_major_original_capital(self):
        result = self.run_census([state(1,[capital(4),capital(1)])])
        self.assertEqual(len(result['capital_turns']),1)
        self.assertEqual(result['capital_turns'][0]['capital']['original_owner'],1)

    def test_unknown_founder_is_not_guessed_to_be_major(self):
        result = self.run_census([state(1,[capital(22)])])
        self.assertEqual(result['capital_turns'],[])

    def test_only_first_native_state_each_turn_is_counted(self):
        later=state(1,[capital()]);later['frame']=2
        result=self.run_census([state(1,[capital()]),later])
        self.assertEqual(len(result['capital_turns']),1)
        self.assertEqual(result['capital_turns'][0]['frame'],0)

    def test_feature_removal_clears_the_whole_plot_record(self):
        plot=dict(x=5,y=4,o=0,vis=True,t='TERRAIN_PLAINS',w=False,i=False)
        self.assertTrue(audit.terrain_candidate(plot,0))
        wooded=dict(plot,f='FEATURE_FOREST')
        self.assertFalse(audit.terrain_candidate(wooded,0))
        self.assertTrue(audit.terrain_candidate(wooded,0,True))
        self.assertFalse(audit.terrain_candidate(dict(wooded,r='RESOURCE_ALUMINUM'),0,True))


if __name__=='__main__':
    unittest.main()
