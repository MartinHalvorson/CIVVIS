"""Pure paired-browser orchestration guards; no browser games are started."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import site_wasm_browser_pair_ci as paired

RECIPE = dict(publisher_commit='f' * 40, rustc='rustc same', binaryen='wasm-opt same')
RAW = b'original opaque save bytes with u64 18446744073709551615'


def records():
    return [dict(repeat=repeat, arm=arm, save_sha256='same', publication_identities=[[7, 1, 0, False]],
                 chrome='Chrome synthetic', chrome_executable_sha256='browser binary hash',
                 runtime=dict(hostname=paired.browser.PUBLISHED_HOST, secure=True, publishedSite=True),
                 first_paint_to_final_paint_ms=1000 if arm == 'baseline' else 800,
                 navigation_to_final_paint_ms=1200 if arm == 'baseline' else 1000)
            for repeat, arm in paired.plan('default-7')]


class PairGuards(unittest.TestCase):
    def test_both_orders_are_balanced_for_both_rosters(self):
        self.assertEqual(paired.plan('default-7'), [(0, 'candidate'), (0, 'baseline'), (1, 'baseline'), (1, 'candidate')])
        self.assertEqual(paired.plan('six-7311003'), [(0, 'baseline'), (0, 'candidate'), (1, 'candidate'), (1, 'baseline')])

    def test_recipe_requires_same_publisher_and_both_tools(self):
        paired.recipe_matches(RECIPE, dict(RECIPE))
        for key in RECIPE:
            with self.subTest(key=key), self.assertRaises(ValueError):
                paired.recipe_matches(RECIPE, dict(RECIPE, **{key: 'different'}))
        with self.assertRaises(ValueError):
            paired.recipe_matches({}, {})

    def test_summary_keeps_both_clock_scopes_and_uncorrected_ratios(self):
        summary = paired.summarize(records(), 'default-7')
        self.assertTrue(summary['complete'])
        self.assertAlmostEqual(summary['both_order_geometric_change']['first_paint_to_final_paint_ms_pct'], -20)
        self.assertAlmostEqual(summary['both_order_geometric_change']['navigation_to_final_paint_ms_pct'], -100 / 6)

    def test_missing_wrong_order_or_duplicate_game_cannot_complete(self):
        for mutate in [lambda r: r.pop(), lambda r: r.reverse(),
                       lambda r: r[0].update(arm='baseline')]:
            value = records()
            mutate(value)
            with self.assertRaises(ValueError):
                paired.summarize(value, 'default-7')

    def test_changed_save_or_publication_identity_rejects_comparison(self):
        for field, value in [('save_sha256', 'different'), ('publication_identities', [[7, 99, 4, True]]),
                             ('chrome', 'different browser'), ('chrome_executable_sha256', 'changed binary'),
                             ('runtime', dict(secure=False))]:
            value_rows = records()
            value_rows[2][field] = value
            with self.assertRaises(ValueError):
                paired.summarize(value_rows, 'default-7')

    def test_bad_or_nonfinite_time_is_not_a_speed_result(self):
        for value in [0, -1, float('nan'), float('inf'), True]:
            rows = records()
            rows[0]['first_paint_to_final_paint_ms'] = value
            with self.assertRaises(ValueError):
                paired.summarize(rows, 'default-7')

    def test_finite_inputs_must_not_produce_infinite_ratios(self):
        rows = records()
        rows[0]['first_paint_to_final_paint_ms'] = 1e308
        rows[1]['first_paint_to_final_paint_ms'] = 1e-308
        with self.assertRaises(ValueError):
            paired.summarize(rows, 'default-7')

    def test_identical_times_form_valid_aa_summary(self):
        rows = records()
        for row in rows:
            row['first_paint_to_final_paint_ms'] = 1000
            row['navigation_to_final_paint_ms'] = 1200
        self.assertEqual(paired.summarize(rows, 'default-7')['both_order_geometric_change'],
                         dict(first_paint_to_final_paint_ms_pct=0, navigation_to_final_paint_ms_pct=0))

    def fake_guard(self, path, source, run, repo, case, out, chrome=None):
        out.mkdir()
        (out / 'game.save.json').write_bytes(RAW)
        (out / 'observation.json').write_text(json.dumps({'observation': {'frames': [
            dict(seed=7, turn=1, sequence=0, finished=False, winner=None, victoryType=None),
            dict(seed=7, turn=2, sequence=1, finished=True, winner=0, victoryType='science')]}}))
        elapsed = 1000 if source == 'a' * 40 else 800
        return dict(phase='complete', source=source, qualification_run=run, case=case,
                    chrome='Chrome synthetic', chrome_executable_sha256='browser binary hash',
                    runtime=dict(hostname=paired.browser.PUBLISHED_HOST, secure=True, publishedSite=True),
                    raw_save=dict(sha256=hashlib.sha256(RAW).hexdigest()),
                    timing=dict(first_paint_to_final_paint_ms=elapsed, navigation_to_final_paint_ms=elapsed + 200))

    def test_four_games_delegate_to_existing_guard_and_preserve_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            module = root / 'module.wasm'
            module.write_bytes(b'mocked module')
            with patch.object(paired.browser, 'verify_module', return_value=(module, RECIPE)), \
                    patch.object(paired.browser, 'browser_game', side_effect=self.fake_guard) as game:
                result = paired.compare(root, 'a' * 40, 1, root, 'b' * 40, 2,
                                        'o/r', 'default-7', root / 'out', chrome=root / 'fixed-chrome')
            self.assertEqual(game.call_count, 4)
            self.assertTrue(all(call.kwargs == {'chrome': root / 'fixed-chrome'} for call in game.call_args_list))
            self.assertEqual(result['phase'], 'complete')
            self.assertTrue(result['same_engine_bytes'])
            self.assertFalse(result['identical_qualification_control'])
            for row in result['records']:
                self.assertEqual((Path(row['path']) / 'game.save.json').read_bytes(), RAW)

    def test_identical_qualifications_are_valid_full_aa_controls(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            module = root / 'module.wasm'
            module.write_bytes(b'mocked module')
            with patch.object(paired.browser, 'verify_module', return_value=(module, RECIPE)), \
                    patch.object(paired.browser, 'browser_game', side_effect=self.fake_guard):
                result = paired.compare(root, 'a' * 40, 1, root, 'a' * 40, 1,
                                        'o/r', 'default-7', root / 'out')
            self.assertTrue(result['identical_qualification_control'])
            self.assertEqual(result['summary']['both_order_geometric_change']['first_paint_to_final_paint_ms_pct'], 0)

    def test_failed_subgame_preserves_partial_evidence_and_is_not_replayed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            module = root / 'module.wasm'
            module.write_bytes(b'mocked module')
            calls = []
            def fail_second(*args, **kwargs):
                calls.append(args)
                if len(calls) == 2:
                    raise RuntimeError('actual subgame failure')
                return self.fake_guard(*args, **kwargs)
            with patch.object(paired.browser, 'verify_module', return_value=(module, RECIPE)), \
                    patch.object(paired.browser, 'browser_game', side_effect=fail_second):
                with self.assertRaises(RuntimeError):
                    paired.compare(root, 'a' * 40, 1, root, 'b' * 40, 2,
                                   'o/r', 'default-7', root / 'out')
                receipt = json.loads((root / 'out/receipt.json').read_text())
                self.assertEqual(len(receipt['records']), 1)
                self.assertTrue(receipt['phase'].startswith('failed'))
                with self.assertRaises(FileExistsError):
                    paired.compare(root, 'a' * 40, 1, root, 'b' * 40, 2,
                                   'o/r', 'default-7', root / 'out')
            self.assertEqual(len(calls), 2)

    def test_existing_output_fails_before_any_module_or_browser_work(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(paired.browser, 'verify_module') as verify:
            root = Path(temporary)
            with self.assertRaises(FileExistsError):
                paired.compare(root, 'a' * 40, 1, root, 'b' * 40, 2, 'o/r', 'default-7', root)
            verify.assert_not_called()


if __name__ == '__main__':
    unittest.main()
