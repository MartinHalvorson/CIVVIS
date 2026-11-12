"""Synthetic completion/identity/paint/raw-byte guards; no browser games."""
import copy
import hashlib
import json
import re
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from urllib.parse import parse_qs, urlparse

import site_wasm_browser_ci as browser

SOURCE = 'a' * 40


def observation():
    frames = [dict(seed=7, sequence=0, turn=1, finished=False, atMs=100, pace=500,
                   winner=None, victoryType=None),
              dict(seed=7, sequence=1, turn=2, finished=True, atMs=1500, pace=0,
                   winner=1, victoryType='science')]
    settings = dict(majors=4, cityStates=6, speed='online', turns=250,
                    map='tenins_ball', shape='planet', width=75, height=32,
                    difficulty='prince', turnStructure='sequential',
                    victoryConditions=dict.fromkeys(
                        ['culture', 'diplomatic', 'domination', 'religious', 'science', 'score'], True))
    return dict(phase='complete', frames=frames,
                paints=[dict(frame, atMs=frame['atMs'] + 16) for frame in frames],
                serverCommit=SOURCE, settings=settings, error=None)


def save_game():
    return dict(game=dict(seed=7, turn=2, game_speed='online', max_turns=250,
                          human_seats=[], winner=1, victory_type='science',
                          difficulty='prince', turn_structure='sequential',
                          map_script='tenins_ball',
                          map=dict(width=75, height=32, topology=dict(globe=15)),
                          victory_conditions=observation()['settings']['victoryConditions'],
                          players=[dict(is_minor=False)] * 4 + [dict(is_minor=True)] * 6,
                          rng={'state': 18446744073709551615}))


class BrowserGuards(unittest.TestCase):
    def test_complete_game_has_two_distinct_clock_scopes(self):
        result = browser.validate_observation(observation(), 'default-7', SOURCE)
        self.assertEqual(result['frames'], 2)
        self.assertEqual(result['navigation_to_final_paint_ms'], 1516)
        self.assertEqual(result['first_paint_to_final_paint_ms'], 1400)

    def test_incomplete_missed_reordered_wrong_seed_or_unpainted_games_fail(self):
        mutations = [lambda o: o.update(phase='observing'),
                     lambda o: o.update(error='render failed'),
                     lambda o: o['frames'][0].update(sequence=1),
                     lambda o: o['frames'][0].update(turn=2),
                     lambda o: o['frames'][1].update(seed=8),
                     lambda o: o['frames'][1].update(sequence=2),
                     lambda o: (o['frames'][1].update(turn=195), o['paints'][1].update(turn=195)),
                     lambda o: o['frames'][1].update(pace=500),
                     lambda o: o['frames'][0].update(finished=True),
                     lambda o: o['frames'][1].update(finished=False),
                     lambda o: o['paints'].pop(),
                     lambda o: o['paints'][1].update(sequence=0),
                     lambda o: o['paints'][1].update(atMs=10),
                     lambda o: o['frames'][0].update(atMs=float('nan')),
                     lambda o: o.update(serverCommit='b' * 40)]
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                value = observation()
                mutate(value)
                with self.assertRaises(ValueError):
                    browser.validate_observation(value, 'default-7', SOURCE)

    def test_url_requests_full_normal_six_player_game(self):
        query = parse_qs(urlparse(browser.case_url('http://localhost:123', 'six-7311003')).query)
        self.assertEqual(query, dict(game=['7311003'], players=['6'], map=['continents'],
                                     shape=['planet'], speed=['online'], turns=['250'], mode=['watch']))

    def test_served_hostname_selects_the_actual_public_lightning_default(self):
        # Regression for real failed run43218507411: localhost selected the
        # desktop/Blitz branch for all2136frames. Exercise the frontend's own
        # hostname classifier and restorePace implementation, not a replica.
        root = Path(__file__).resolve().parents[1]
        page = (root / 'web/index.html').read_text()
        assignment = next(line.strip() for line in page.splitlines()
                          if 'window.CIVVIS_PUBLISHED_SITE =' in line)
        app = (root / 'web/assets/app.js').read_text()
        default_pace = next(line.strip() for line in app.splitlines()
                            if line.startswith('const PUBLISHED_SITE_PACE_MS ='))
        start = app.index('function restorePace() {')
        restore = app[start:app.index('\n}\n', start) + 2]
        script = r"""
const {default: assert} = await import('node:assert/strict');
const hostname = new URL(process.argv[1]).hostname;
const location = {hostname}, window = {};
eval(process.argv[2]);
assert.equal(window.CIVVIS_PUBLISHED_SITE, true);
const PACE_KEY = 'civvis-watch-pace';
const localStorage = {getItem: () => null};
const select = {value: '500', options: [{value: '0'}, {value: '500'}]};
const document = {getElementById: () => select};
const paceOffered = (select, ms) => select.options.some(o => +o.value === ms);
let paceChoice = null, pacePreference = null;
eval(process.argv[4] + '\n' + process.argv[3] + '\nrestorePace();');
assert.equal(paceChoice, 0);
assert.equal(select.value, '0');
"""
        subprocess.run(['node', '--input-type=module', '-e', script,
                        browser.browser_origin(12345), assignment, restore, default_pace],
                       check=True, capture_output=True, text=True)
        failed = subprocess.run(['node', '--input-type=module', '-e', script,
                                 'http://127.0.0.1:12345', assignment, restore, default_pace], capture_output=True)
        self.assertNotEqual(failed.returncode, 0)

    def test_actual_browser_runtime_must_match_published_secure_context(self):
        valid = dict(hostname=browser.PUBLISHED_HOST, secure=True, publishedSite=True)
        browser.validate_runtime(valid)
        for key, wrong in [('hostname', '127.0.0.1'), ('secure', False), ('publishedSite', False)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                browser.validate_runtime(dict(valid, **{key: wrong}))

    def test_save_bytes_keep_all_u64_bits(self):
        raw = json.dumps(save_game()).encode()
        value = browser.validate_save(raw, observation(), 'default-7')
        self.assertEqual(value, dict(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
        self.assertIn(b'18446744073709551615', raw)

    def test_wrong_or_human_controlled_saved_world_fails(self):
        for key, value in [('seed', 8), ('turn', 194), ('human_seats', [0]),
                           ('game_speed', 'quick'), ('max_turns', 20), ('winner', 2),
                           ('map_script', 'continents'), ('difficulty', 'settler'),
                           ('turn_structure', 'simultaneous'), ('victory_type', 'culture')]:
            with self.subTest(key=key):
                game = save_game()
                game['game'][key] = value
                with self.assertRaises(ValueError):
                    browser.validate_save(json.dumps(game).encode(), observation(), 'default-7')
        for mutate in [lambda g: g['players'].pop(),
                       lambda g: g['map'].update(width=20),
                       lambda g: g['map'].update(topology='flat'),
                       lambda g: g['victory_conditions'].update(science=False)]:
            game = save_game()
            mutate(game['game'])
            with self.assertRaises((ValueError, AttributeError)):
                browser.validate_save(json.dumps(game).encode(), observation(), 'default-7')
        game, value = save_game(), observation()
        game['game'].update(winner=None, victory_type=None)
        value['frames'][-1].update(winner=None, victoryType=None)
        with self.assertRaises(ValueError):
            browser.validate_save(json.dumps(game).encode(), value, 'default-7')

    def test_existing_output_is_never_replayed(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(browser, 'verify_module') as verify:
            with self.assertRaises(FileExistsError):
                browser.browser_game(Path(directory), SOURCE, 1, 'o/r', 'default-7', Path(directory))
            verify.assert_not_called()

    def test_settings_use_existing_pure_driver_guard(self):
        command = ['node', '--input-type=module', '-e', browser.profile.SETTINGS_CHECK,
                   browser.pair.DRIVER.resolve().as_uri()]
        settings = observation()['settings']
        subprocess.run(command + [json.dumps(settings), 'default'], check=True, capture_output=True)
        settings['cityStates'] = 0
        result = subprocess.run(command + [json.dumps(settings), 'default'], capture_output=True)
        self.assertNotEqual(result.returncode, 0)

    def test_observer_delegates_original_renderer_and_waits_for_animation_frame(self):
        # No browser or WASM: exercise only the installed renderer wrapper.
        script = r"""
const {default: vm} = await import('node:vm');
const {default: assert} = await import('node:assert/strict');
let install, clock = 100, calls = [], raf = [];
const returned = {};
const ctx = {window: {}, paintedFrame: null,
 setInterval: fn => { install = fn; return 1; }, clearInterval: () => {},
 requestAnimationFrame: fn => raf.push(fn), performance: {now: () => clock}};
ctx.window.render = function(st, ...args) {
 calls.push({self: this, st, args});
 clock += 7;
 ctx.paintedFrame = {seed: st.seed, turn: st.turn, sequence: st.frame_sequence, finished: st.finished};
 return returned;
};
vm.createContext(ctx);
vm.runInContext(process.argv[1], ctx);
install();
const st = JSON.parse(process.argv[2]);
const self = {};
assert.equal(ctx.window.render.call(self, st, false, true), returned);
assert.equal(calls[0].self, self);
assert.equal(calls[0].st, st);
assert.deepEqual(calls[0].args, [false, true]);
assert.equal(ctx.window.__civvisFullGame.frames.length, 1);
assert.equal(ctx.window.__civvisFullGame.paints.length, 0);
clock = 116; raf.shift()();
assert.equal(ctx.window.__civvisFullGame.paints.length, 1);
ctx.window.render(st); // A duplicate render is delegated but not a new publication.
assert.equal(ctx.window.__civvisFullGame.frames.length, 1);
assert.equal(raf.length, 0);
const end = {...st, turn: 2, frame_sequence: 1, finished: true, pace: 0};
clock = 1500; ctx.window.render(end);
assert.equal(ctx.window.__civvisFullGame.phase, 'observing');
clock = 1516; raf.shift()();
assert.equal(ctx.window.__civvisFullGame.phase, 'complete');
assert.equal(calls.length, 3);
for (const frame of ctx.window.__civvisFullGame.frames) {
 if (ctx.window.__civvisProfileMainCpu) assert.equal(frame.renderMs, 7);
 else assert.equal('renderMs' in frame, false);
}
assert.equal(ctx.window.__civvisFullGame.paints.length, 2);
"""
        st = dict(seed=7, turn=1, frame_sequence=0, finished=False, pace=500,
                  players=[dict(is_minor=False)], map=dict(script='tenins_ball', shape='planet', width=75, height=32))
        for prefix in ['', 'window.__civvisProfileMainCpu = true;']:
            subprocess.run(['node', '--input-type=module', '-e', script,
                            prefix + browser.OBSERVER_JS, json.dumps(st)],
                           check=True, capture_output=True, text=True)




def cpu_evidence():
    nodes = [dict(id=1, callFrame=dict(functionName='(root)', url=''), children=[2, 4, 5]),
             dict(id=2, callFrame=dict(functionName='render', url='http://example/assets/app.js'), children=[3]),
             dict(id=3, callFrame=dict(functionName='(garbage collector)', url='')),
             dict(id=4, callFrame=dict(functionName='(idle)', url='')),
             dict(id=5, callFrame=dict(functionName='(program)', url=''))]
    cpu = dict(nodes=nodes, samples=[3, 4, 5], timeDeltas=[500, 1000, 500], startTime=100, endTime=2100)
    metrics = {phase: dict(metrics=[dict(name=key, value=value) for key in browser.DURATION_METRICS])
               for phase, value in [('before', 1), ('after', 2)]}
    value = observation()
    for frame in value['frames']:
        frame['renderMs'] = 20
    return cpu, metrics, value


class MainCpuGuards(unittest.TestCase):
    def test_native_leaf_with_frontend_ancestor_is_attributed_without_idle_correction(self):
        cpu, metrics, value = cpu_evidence()
        result = browser.main_cpu_summary(cpu, metrics, value)
        self.assertEqual(result['sample_count'], 3)
        self.assertEqual(result['sampled_span_us'], 2000)
        self.assertEqual(result['thread_cpu_seconds']['ThreadTime'], 1)
        self.assertEqual(result['render_wall_ms']['total'], 40)
        self.assertEqual([r['node'] for r in result['top_self_samples']], [3, 4, 5])
        # Program samples are retained separately, never silently classified idle.
        self.assertEqual(result['top_self_samples'][2]['callFrame']['functionName'], '(program)')

    def test_real_chrome_timestamp_inversions_are_preserved_and_reported(self):
        cpu, metrics, value = cpu_evidence()
        cpu['timeDeltas'] = [500, -12, 1000]
        original = copy.deepcopy(cpu)
        result = browser.main_cpu_summary(cpu, metrics, value)
        self.assertEqual(cpu, original)
        self.assertEqual(result['timestamp_inversions'], 1)
        self.assertEqual(result['largest_backward_step_us'], 12)
        self.assertEqual(result['sampled_span_us'], 1488)
        self.assertEqual(sum(row['samples'] for row in result['top_self_samples']), 3)

    def test_malformed_profiles_fail(self):
        mutations = [lambda c: c.update(samples=[]),
                     lambda c: c.update(timeDeltas=[1]),
                     lambda c: c.update(timeDeltas=[500, -501, 1000]),
                     lambda c: c.update(timeDeltas=[500, 1000, 501]),
                     lambda c: c.update(timeDeltas=[500, float('nan'), 500]),
                     lambda c: c.update(timeDeltas=[0, 0, 0]),
                     lambda c: c.update(endTime=0),
                     lambda c: c.update(startTime=float('nan')),
                     lambda c: c['samples'].__setitem__(0, 99),
                     lambda c: c['nodes'].append(copy.deepcopy(c['nodes'][0])),
                     lambda c: c['nodes'][0]['children'].append(99),
                     lambda c: c['nodes'][1]['children'].append(4),
                     lambda c: c['nodes'][2].update(children=[2]),
                     lambda c: c['nodes'][1]['callFrame'].update(url='http://example/other.js')]
        for mutate in mutations:
            cpu, metrics, value = cpu_evidence()
            mutate(cpu)
            with self.subTest(mutation=mutate), self.assertRaises(ValueError):
                browser.main_cpu_summary(cpu, metrics, value)

    def test_cpu_metrics_require_thread_counters_but_heap_gauges_may_fall(self):
        cpu, metrics, value = cpu_evidence()
        metrics['before']['metrics'].append(dict(name='JSHeapUsedSize', value=100))
        metrics['after']['metrics'].append(dict(name='JSHeapUsedSize', value=1))
        browser.main_cpu_summary(cpu, metrics, value)
        for wrong in [-1, 0, True, float('inf')]:
            altered = copy.deepcopy(metrics)
            altered['after']['metrics'][0]['value'] = wrong
            with self.subTest(value=wrong), self.assertRaises(ValueError):
                browser.main_cpu_summary(cpu, altered, value)
        metrics['after']['metrics'].pop(0)
        with self.assertRaises(ValueError):
            browser.main_cpu_summary(cpu, metrics, value)

    def test_complete_render_durations_cannot_be_missing_or_nonfinite(self):
        for wrong in [None, -1, True, float('nan')]:
            cpu, metrics, value = cpu_evidence()
            value['frames'][0]['renderMs'] = wrong
            with self.subTest(value=wrong), self.assertRaises(ValueError):
                browser.main_cpu_summary(cpu, metrics, value)
        cpu, metrics, value = cpu_evidence()
        value['frames'][0]['renderMs'] = 0
        browser.main_cpu_summary(cpu, metrics, value)

    def test_cli_passes_explicit_browser_and_only_opt_in_profiling(self):
        common = ['observer', '--qualified', '.', '--source', SOURCE, '--run', '1',
                  '--repo', 'o/r', '--case', 'default-7', '--out', 'out']
        for extra, expected in [([], dict(chrome=None, main_cpu_profile=False)),
                                (['--chrome', '/fixed-chrome', '--main-cpu-profile'],
                                 dict(chrome=Path('/fixed-chrome'), main_cpu_profile=True))]:
            with patch('sys.argv', common + extra), patch.object(browser, 'browser_game', return_value={}) as game, \
                    patch('builtins.print'):
                browser.main()
            self.assertEqual(game.call_args.kwargs, expected)

    def test_browser_cpu_workflow_runs_one_browser_game_and_preserves_evidence(self):
        # Match actual step conditions: an accidentally included CPU step would
        # silently play an additional game, so inspect both execution and upload.
        workflow = (Path(__file__).resolve().parents[1] /
                    '.github/workflows/wasm-profile.yml').read_text()
        steps = re.split(r'\n      - ', workflow)[1:]
        engine_steps = [step for step in steps if 'tools/site_wasm_profile_ci.py ' in step
                        or 'name: civvis-wasm-cpu-profile' in step]
        self.assertEqual(len(engine_steps), 2)
        for step in engine_steps:
            self.assertIn("inputs.mode != 'browser-cpu'", step)
        profiled = [step for step in steps if '--main-cpu-profile' in step]
        self.assertEqual(len(profiled), 1)
        self.assertIn("if: ${{ inputs.mode == 'browser-cpu' }}", profiled[0])
        self.assertIn('--chrome "$CIVVIS_BROWSER_EXECUTABLE"', profiled[0])
        upload = next(step for step in steps if 'name: civvis-wasm-browser-game' in step)
        self.assertIn("inputs.mode == 'browser-cpu'", upload)



if __name__ == '__main__':
    unittest.main()
