#!/usr/bin/env python3
"""Native combat text calibrates Deity without rewriting the raw XML formula."""
import copy
import json
from pathlib import Path
import re
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_fidelity as audit

ROOT = Path(__file__).resolve().parents[1]
LADDER = ['DIFFICULTY_' + name.upper() for name in
          ['settler', 'chieftain', 'warlord', 'prince', 'king', 'emperor', 'immortal', 'deity']]
MODIFIER = 'HIGH_DIFFICULTY_COMBAT_SCALING'
ARGUMENT = {'ModifierId': MODIFIER, 'Name': 'Amount',
            'Type': 'LinearScaleFromDefaultHandicap', 'Value': '-1', 'Extra': '1'}


class NativeDeityCombat(unittest.TestCase):
    def test_raw_three_and_observed_four_are_kept_distinct(self):
        self.assertEqual(audit.handicap_amount(ARGUMENT, LADDER, LADDER[-1]), 3)
        self.assertEqual(audit.projected_handicap_amount(MODIFIER, ARGUMENT, LADDER, LADDER[-1]), 4)

    def test_other_rungs_and_modifiers_keep_their_raw_projection(self):
        for rung in LADDER:
            if rung != 'DIFFICULTY_DEITY':
                self.assertEqual(
                    audit.projected_handicap_amount(MODIFIER, ARGUMENT, LADDER, rung),
                    audit.handicap_amount(ARGUMENT, LADDER, rung))
        for modifier in ['HIGH_DIFFICULTY_SCIENCE_SCALING', 'LOW_DIFFICULTY_COMBAT_SCALING']:
            self.assertEqual(audit.projected_handicap_amount(modifier, ARGUMENT, LADDER, LADDER[-1]), 3)

    def test_changed_source_arguments_do_not_reuse_the_old_calibration(self):
        for change in [{'Value': '-2'}, {'Extra': '2'}, {'SecondExtra': 'DIFFICULTY_PRINCE'},
                       {'SecondExtra': 'DIFFICULTY_KING'}, {'Type': 'Constant'}]:
            argument = {**ARGUMENT, **change}
            with self.subTest(change=change):
                self.assertEqual(
                    audit.projected_handicap_amount(MODIFIER, argument, LADDER, LADDER[-1]),
                    audit.handicap_amount(argument, LADDER, LADDER[-1]))
        shorter = [rung for rung in LADDER if rung != 'DIFFICULTY_KING']
        self.assertEqual(audit.projected_handicap_amount(MODIFIER, ARGUMENT, shorter, shorter[-1]), 2)

    def test_actual_difficulty_projection_uses_the_runtime_component(self):
        db = audit.Database()
        rows = {
            'Difficulties': [{'DifficultyType': rung} for rung in LADDER],
            'Modifiers': [{'ModifierId': MODIFIER}],
            'ModifierArguments': [copy.deepcopy(ARGUMENT)],
        }
        for table, values in rows.items():
            db.tables[table] = {(i,): row for i, row in enumerate(values)}
        projected = audit.project_difficulties(db)
        self.assertEqual(projected['deity']['ai_combat_strength'], 4)
        for rung in LADDER[:-1]:
            self.assertEqual(projected[rung.removeprefix('DIFFICULTY_').lower()]['ai_combat_strength'],
                             audit.handicap_amount(ARGUMENT, LADDER, rung))

    def test_captured_native_text_and_totals_support_the_component(self):
        evidence = json.loads((ROOT / 'docs/fidelity/native-deity-combat-2026-11-05.json').read_text())
        self.assertEqual(evidence['native_seat']['difficulty'], 'DIFFICULTY_DEITY')
        self.assertEqual(evidence['native_seat']['ruleset'], 'RULESET_EXPANSION_2')
        self.assertTrue(evidence['native_seat']['is_human'])
        self.assertEqual(evidence['native_seat']['local_player'], 0)
        self.assertTrue(evidence['readback_valid'])
        self.assertRegex(evidence['binary_sha256'], r'^[0-9a-f]{64}$')
        self.assertGreaterEqual(len(evidence['observations']), 2)
        self.assertGreaterEqual(evidence['observation_count'], len(evidence['observations']))
        units = set()
        for event in evidence['observations']:
            self.assertNotIn(event['actor']['player'], [0, 63])
            units.add(event['actor']['kind'])
            preview, side = event['preview'], event['side']
            text = preview[side + '_modifier_text']['PREVIEW_TEXT_MODIFIER']
            bonus = [line for line in text if 'Combat Strength bonus due to difficulty.' in line]
            self.assertEqual(bonus, ['+4 [ICON_Strength] Combat Strength bonus due to difficulty.'])
            self.assertEqual(int(re.search(r'[+-]\d+', bonus[0]).group()), evidence['observed_bonus'])
            self.assertEqual(audit.projected_handicap_amount(MODIFIER, ARGUMENT, LADDER, LADDER[-1]),
                             evidence['observed_bonus'])
            for actor_side in ['attacker', 'defender']:
                self.assertEqual(preview[actor_side + '_total_strength'],
                                 max(0, preview[actor_side + '_strength']
                                     + preview.get(actor_side + '_strength_modifier', 0)))
        self.assertGreaterEqual(len(units), 2)

    def test_report_discloses_the_runtime_calibration(self):
        result = {'table': 'Difficulties', 'compared': 8, 'divergences': [],
                  'waived': 0, 'only_ours': [], 'only_theirs': []}
        report = audit.report([result], Path('/reference'))
        self.assertIn('native +4 readback', report)
        self.assertIn('raw XML projection is 3', report)
        self.assertIn('Other combat rungs remain uncalibrated', report)


if __name__ == '__main__':
    unittest.main()
