#!/usr/bin/env python3
"""Guard the distinction between research, original-capital ownership and sight."""
import importlib.util
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

PATH = Path(__file__).with_name('2026-10-25-capital-watch-native.audit.py')
SPEC = importlib.util.spec_from_file_location('watch_audit', PATH)
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)


class ObservedTimings(unittest.TestCase):
    def run_census(self, events):
        with TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'summary.json').write_text(json.dumps(dict(game_stopped=True,
                seat=dict(local_player=0), genome_treatments=dict(
                    treatments=['capital-walls-before-the-surprise']))))
            (root / 'events.jsonl').write_text(''.join(json.dumps(e) + '\n' for e in events))
            (root / 'decisions.jsonl').write_text('')
            (root / 'why.log').write_text('')
            return audit.census(root)

    def state(self, turn, cities, **extra):
        return dict(kind='state', turn=turn, frame=0, cities=cities,
                    military=10, rivals=[], techs=[], **extra)

    def capital(self):
        return dict(id=1, original_capital=True, original_owner=0, x=0, y=0,
                    buildings=[], damage=0, max_wall_damage=0)

    def test_research_readback_survives_loss_without_using_replacement_walls(self):
        replacement = dict(id=2, original_capital=False, original_owner=0,
                           x=2, y=2, buildings=['BUILDING_WALLS'], max_wall_damage=100)
        final = self.state(3, [replacement])
        final['techs'] = ['TECH_MASONRY']
        result = self.run_census([
            dict(kind='tiles', width=10, plots=[]), self.state(1, [self.capital()]),
            dict(kind='city_lost', city=1, turn=2), self.state(2, []), final])
        self.assertEqual(result['timing']['masonry_readback']['turn'], 3)
        self.assertIsNone(result['timing']['walls_readback'])
        self.assertEqual(len(result['losses']), 1)
        self.assertEqual(len(result['first_state_rows']), 1)

    def test_exported_unit_presence_does_not_become_current_plot_sight(self):
        state = self.state(1, [self.capital()])
        state['rivals'] = [dict(player=1, at_war=False, military=30,
            units=[dict(kind='UNIT_WARRIOR', **{'class': 'PROMOTION_CLASS_MELEE'},
                        x=9, y=0)])]
        result = self.run_census([
            dict(kind='tiles', width=10, plots=[dict(x=9, y=0, vis=False)]), state])
        self.assertEqual(result['timing']['peace_2x_exported_land8']['turn'], 1)
        self.assertIsNone(result['timing']['peace_2x_visible_land8'])
        self.assertEqual(result['first_state_rows'][0]['peace_twice_power'][0]
                         ['within8'][0]['distance'], 1)


if __name__ == '__main__':
    unittest.main()
