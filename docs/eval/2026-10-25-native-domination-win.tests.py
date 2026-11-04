"""Reject a summary-only win, wrong winner/type and incomplete setup proof."""
import copy
import importlib.util
from pathlib import Path
import unittest

SOURCE = Path(__file__).with_name('2026-10-25-native-domination-win.verify.py')
SPEC = importlib.util.spec_from_file_location('win_verifier', SOURCE)
verify = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verify)


def fixture():
    outcome = dict(kind='victory', won=True, team=0, local_team=0, local_player=0,
                   run='fixture', victory=2, victory_type='VICTORY_CONQUEST', turn=210)
    seat = dict(local_player=0, run='fixture', difficulty='DIFFICULTY_EMPEROR',
                ruleset='RULESET_EXPANSION_2', victory_types=[dict(index=2, type='VICTORY_CONQUEST')],
                victories={key: True for key in ('conquest', 'culture', 'diplomatic', 'religious', 'score', 'technology')})
    return dict(tag='fixture', game_stopped=True, outcome=outcome, seat=seat,
                difficulty='DIFFICULTY_EMPEROR', ruleset='RULESET_EXPANSION_2',
                ruleset_requested='RULESET_EXPANSION_2', victory_target='domination')


class WinProofControls(unittest.TestCase):
    def test_consistent_host_callback_and_actual_setup_are_accepted(self):
        summary = fixture()
        self.assertEqual(verify.validate_host(summary, [summary['outcome']])['type'], 'VICTORY_CONQUEST')

    def test_summary_without_a_unique_raw_callback_is_rejected(self):
        summary = fixture()
        for callbacks in ([], [summary['outcome'], summary['outcome']], [dict(summary['outcome'], won=False)]):
            with self.subTest(callbacks=callbacks), self.assertRaises(ValueError):
                verify.validate_host(summary, callbacks)

    def test_wrong_team_type_difficulty_disabled_victory_or_nonterminal_is_rejected(self):
        for field in ('team', 'type', 'difficulty', 'disabled', 'terminal'):
            summary = copy.deepcopy(fixture())
            if field == 'team': summary['outcome']['team'] = 1
            if field == 'type': summary['seat']['victory_types'][0]['type'] = 'VICTORY_RELIGIOUS'
            if field == 'difficulty': summary['seat']['difficulty'] = 'DIFFICULTY_SETTLER'
            if field == 'disabled': summary['seat']['victories']['technology'] = False
            if field == 'terminal': summary['game_stopped'] = False
            with self.subTest(field=field), self.assertRaises(ValueError):
                verify.validate_host(summary, [summary['outcome']])


if __name__ == '__main__':
    unittest.main()
