#!/usr/bin/env python3
"""Reject unbound engines, altered games, lossy saves and unbalanced comparisons.

These tests only inspect synthetic records and exported JSON helpers. They do
not instantiate a game or execute WebAssembly, including on a paused host.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import site_wasm_pair as pair

SOURCE = 'a' * 40


def fixture(root: Path) -> dict:
    (root / 'qualification').mkdir(parents=True)
    (root / 'wasm-symbols').mkdir()
    data, symbols = b'\0asm\1\0\0\0', b'0:example\n'
    (root / 'qualification/civvis.wasm').write_bytes(data)
    (root / 'wasm-symbols/functions.map').write_bytes(symbols)
    manifest = {'format': 1, 'source_commit': SOURCE, 'wasm_bytes': len(data),
                'wasm_sha256': hashlib.sha256(data).hexdigest(),
                'function_map_sha256': hashlib.sha256(symbols).hexdigest()}
    receipt = {'source_commit': SOURCE, 'publisher_commit': 'b' * 40,
               'rustc': 'rustc 1.99.1', 'binaryen': 'wasm-opt version 120', 'engine': manifest}
    (root / 'wasm-symbols/manifest.json').write_text(json.dumps(manifest))
    (root / 'qualification-info.json').write_text(json.dumps(receipt))
    return receipt


def record() -> dict:
    return {'harnessVersion': 7, 'seed': 7, 'roster': 'default',
            'moduleSha256': 'a' * 64, 'finished': True, 'frames': 191, 'turn': 191,
            'traceHash': 'b' * 64, 'saveHash': 'c' * 64, 'cpuMs': 1000.0, 'wallMs': 1100.0,
            'settings': {'majors': 4, 'cityStates': 6}, 'winner': 1, 'victoryType': 'science'}


class BoundArtifacts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'input'
        fixture(self.root)

    def test_qualified_bytes_and_source_are_required(self):
        module, receipt = pair.qualified(self.root, SOURCE)
        self.assertEqual(module.name, 'civvis.wasm')
        self.assertEqual(receipt['source_commit'], SOURCE)
        with self.assertRaisesRegex(ValueError, 'requested source'):
            pair.qualified(self.root, 'b' * 40)
        with self.assertRaisesRegex(ValueError, 'exact commit'):
            pair.qualified(self.root, 'main')

    def test_changed_engine_or_map_is_rejected(self):
        module = self.root / 'qualification/civvis.wasm'
        old = module.read_bytes()
        module.write_bytes(old + b'changed')
        with self.assertRaisesRegex(ValueError, 'different WebAssembly'):
            pair.qualified(self.root, SOURCE)
        module.write_bytes(old)
        (self.root / 'wasm-symbols/functions.map').write_text('0:changed\n')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            pair.qualified(self.root, SOURCE)

    def test_receipt_cannot_disagree_with_its_manifest(self):
        path = self.root / 'qualification-info.json'
        receipt = json.loads(path.read_text())
        receipt['engine']['wasm_sha256'] = '0' * 64
        path.write_text(json.dumps(receipt))
        with self.assertRaisesRegex(ValueError, 'bound manifest'):
            pair.qualified(self.root, SOURCE)

    def test_existing_evidence_is_never_overwritten(self):
        out = Path(self.temp.name) / 'result'
        out.mkdir()
        (out / 'keep').write_text('measured evidence')
        args = argparse.Namespace(baseline=self.root, candidate=self.root,
                                  baseline_source=SOURCE, candidate_source=SOURCE,
                                  out=out, self_test=True)
        with self.assertRaises(FileExistsError):
            pair.compare(args)
        self.assertEqual(list(out.iterdir()), [out / 'keep'])
        self.assertEqual((out / 'keep').read_text(), 'measured evidence')

    def test_toolchain_mismatch_stops_before_any_game(self):
        candidate = Path(self.temp.name) / 'candidate'
        receipt = fixture(candidate)
        receipt['rustc'] = 'a different compiler'
        (candidate / 'qualification-info.json').write_text(json.dumps(receipt))
        args = argparse.Namespace(baseline=self.root, candidate=candidate,
                                  baseline_source=SOURCE, candidate_source=SOURCE,
                                  out=Path(self.temp.name) / 'result', self_test=True)
        with self.assertRaisesRegex(ValueError, 'toolchains differ'):
            pair.compare(args)
        self.assertFalse(args.out.exists())


class ExactGames(unittest.TestCase):
    def test_each_seed_is_measured_in_both_orders(self):
        planned = pair.plan(pair.SEEDS)
        self.assertEqual(len(planned), 12)
        for roster, seed in pair.SEEDS:
            entries = [entry for entry in planned if entry[:2] == (roster, seed)]
            self.assertEqual({entry[2] for entry in entries}, {0, 1})
            self.assertEqual(entries[0][3], entries[1][3][::-1])
            self.assertEqual(set(entries[0][3]), {'baseline', 'candidate'})

    def test_game_differences_are_not_hidden_by_matching_final_hashes(self):
        base = record()
        pair.same_game(base, copy.deepcopy(base))
        for key in ['seed', 'roster', 'settings', 'frames', 'turn', 'finished',
                    'winner', 'victoryType', 'traceHash', 'saveHash']:
            changed = copy.deepcopy(base)
            changed[key] = None
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, key):
                pair.same_game(base, changed)

    def test_incomplete_wrong_module_or_invalid_clock_cannot_be_timed(self):
        pair.validate_run(record(), 'baseline', 'default', 7, 'a' * 64)
        cases = [('finished', False), ('moduleSha256', 'd' * 64), ('seed', 8),
                 ('turn', 252), ('frames', 0), ('cpuMs', 0), ('wallMs', float('nan')),
                 ('traceHash', ''), ('saveHash', '')]
        for key, value in cases:
            row = record()
            row[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                pair.validate_run(row, 'candidate', 'default', 7, 'a' * 64)

    def test_geometric_pair_balances_both_orders(self):
        rows = [{'roster': 'default', 'seed': 7, 'cpu_ratio': 0.8, 'wall_ratio': 0.5,
                 'repeat': 0, 'first_arm': 'baseline'},
                {'roster': 'default', 'seed': 7, 'cpu_ratio': 1.25, 'wall_ratio': 2.0,
                 'repeat': 1, 'first_arm': 'candidate'}]
        summary = pair.paired_summary(rows, [('default', 7)])
        self.assertTrue(summary['complete'])
        self.assertAlmostEqual(summary['by_roster']['default']['median_cpu_pct'], 0)
        self.assertAlmostEqual(summary['by_roster']['default']['median_wall_pct'], 0)
        partial = pair.paired_summary(rows[:1], [('default', 7)])
        self.assertFalse(partial['complete'])
        self.assertEqual(partial['by_roster'], {})
        rows[1]['first_arm'] = 'baseline'
        with self.assertRaisesRegex(ValueError, 'opposite execution orders'):
            pair.paired_summary(rows, [('default', 7)])


class BrowserJSONGuards(unittest.TestCase):
    def js(self, body: str):
        uri = Path(pair.DRIVER).resolve().as_uri()
        source = ('import assert from "node:assert/strict";\n'
                  f'import {{hashSave,normalizeFrame,validateSettings}} from {json.dumps(uri)};\n'
                  + body)
        answer = subprocess.run(['node', '--input-type=module', '-e', source],
                                capture_output=True, text=True)
        self.assertEqual(answer.returncode, 0, answer.stdout + answer.stderr)

    def test_raw_save_hash_retains_u64_bits_that_javascript_rounds(self):
        self.js('''
const a='{"rng":18446744073709551614}', b='{"rng":18446744073709551615}';
assert.equal(JSON.parse(a).rng,JSON.parse(b).rng);
assert.notEqual(hashSave(a),hashSave(b));
''')

    def test_only_named_metadata_is_removed(self):
        self.js('''
const state={turn:191,players:[{id:1,score:120}],server_commit:'old',turn_compute_ms:12,
 ai_reasoning:{cursor:3,thoughts:[{id:3,topic:'other',headline:'kept',detail:'kept'}]}};
const before=structuredClone(state), normalized=normalizeFrame(state);
assert.equal(normalized.server_commit,undefined); assert.equal(normalized.turn_compute_ms,undefined);
assert.deepEqual(state,before); assert.equal(normalized.turn,191);
for(const change of [{...state,turn:192},{...state,players:[{id:1,score:121}]},
 {...state,ai_reasoning:{...state.ai_reasoning,cursor:4}}]) assert.notDeepEqual(normalizeFrame(change),normalized);
''')

    def test_only_adjacent_independent_refusal_notices_are_canonicalized(self):
        self.js('''
const note=(id,unit)=>({id,player:1,turn:100,topic:'military',level:'detail',
 headline:`Warrior ${unit} stops re-asking a move the host keeps refusing`,
 detail:'ordered from 1,2 so the route must change'});
const a={ai_reasoning:{cursor:9,thoughts:[note(8,2),note(9,1)]}};
const b={ai_reasoning:{cursor:9,thoughts:[note(8,1),note(9,2)]}};
assert.deepEqual(normalizeFrame(a),normalizeFrame(b));
const changed=structuredClone(b); changed.ai_reasoning.thoughts[0].detail='different action';
assert.notDeepEqual(normalizeFrame(a),normalizeFrame(changed));
const bad=structuredClone(b); bad.ai_reasoning.thoughts[1].id=8;
assert.throws(()=>normalizeFrame(bad),/stream ID order/);
''')

    def test_normal_settings_reject_smaller_rosters_and_weakened_rules(self):
        self.js('''
const victories={culture:true,diplomatic:true,domination:true,religious:true,science:true,score:true};
const settings={majors:4,cityStates:6,speed:'online',turns:250,map:'tenins_ball',shape:'planet',
 width:75,height:32,difficulty:'prince',turnStructure:'sequential',victoryConditions:victories};
validateSettings(settings,'default');
validateSettings({...settings,majors:6,cityStates:9,map:'continents',width:90,height:38},'six');
for(const [key,value] of Object.entries({majors:2,cityStates:0,speed:'quick',turns:100,
 width:24,height:16,difficulty:'settler',turnStructure:'simultaneous'}))
 assert.throws(()=>validateSettings({...settings,[key]:value},'default'));
assert.throws(()=>validateSettings({...settings,victoryConditions:{...victories,science:false}},'default'));
''')



class PortableEntrypoints(unittest.TestCase):
    def test_actual_workflow_script_supports_manual_and_pr_modes(self):
        workflow = Path(__file__).resolve().parent.parent / '.github/workflows/wasm-paired-cost.yml'
        block = workflow.read_text().split('      - name: Compare complete games in both orders\n', 1)[1]
        script = textwrap.dedent(block.split('        run: |\n', 1)[1].split('      - name:', 1)[0])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / 'python3'
            binary.write_text("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$ARGUMENT_REPORT\"\n")
            binary.chmod(0o755)
            for event in ['workflow_dispatch', 'pull_request']:
                with self.subTest(event=event):
                    report = root / event
                    env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ['PATH'],
                               ARGUMENT_REPORT=str(report), EVENT_NAME=event,
                               BASELINE_SOURCE='a' * 40, CANDIDATE_SOURCE='b' * 40)
                    answer = subprocess.run(['/bin/bash', '-c', script], cwd=root, env=env,
                                            capture_output=True, text=True)
                    self.assertEqual(answer.returncode, 0, answer.stderr)
                    arguments = report.read_text().splitlines()
                    self.assertEqual('--self-test' in arguments, event == 'pull_request')
                    self.assertIn('a' * 40, arguments)
                    self.assertIn('b' * 40, arguments)

    def test_symlinked_cli_reaches_the_driver_and_rejects_missing_module(self):
        with tempfile.TemporaryDirectory() as directory:
            alias = Path(directory) / 'entry.mjs'
            alias.symlink_to(pair.DRIVER.resolve())
            missing = str(Path(directory) / 'missing.wasm')
            answer = subprocess.run(['node', str(alias), missing], capture_output=True, text=True)
            self.assertNotEqual(answer.returncode, 0, 'CLI silently skipped its entry point')
            self.assertIn('ENOENT', answer.stderr)

    def test_import_with_unrelated_nonexistent_argv_never_enters_game(self):
        source = f'await import({json.dumps(pair.DRIVER.resolve().as_uri())});'
        answer = subprocess.run(['node', '--input-type=module', '-e', source,
                                 '/nonexistent/non-entry-script'], capture_output=True, text=True)
        self.assertEqual(answer.returncode, 0, answer.stderr)
        self.assertEqual(answer.stdout, '')

if __name__ == '__main__':
    unittest.main()
