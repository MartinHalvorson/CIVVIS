#!/usr/bin/env python3
"""Observe a complete qualified website game without changing its turn loop.

This manual CI diagnostic serves the exact qualified bundle, observes render
completion and the following animation frame, then retrieves original save
bytes through the website's IndexedDB save path. It starts no game on import.
Headless browser timings describe the named runner, not a visitor's laptop.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import importlib.util
import json
import math
import os
import platform
import re
import shutil
import statistics
import subprocess
import time
import urllib.parse
import urllib.request
from pathlib import Path

import site_wasm_pair as pair
import site_wasm_profile_ci as profile

PUBLISHED_HOST = 'browser-measurement.civvis.ai'


OBSERVER_JS = r"""
(() => {
  'use strict';
  const report = window.__civvisFullGame = {phase: 'waiting', frames: [], paints: [], error: null};
  const timer = setInterval(() => {
    if (typeof window.render !== 'function') return;
    clearInterval(timer);
    const original = window.render;
    window.render = function(st, ...args) {
      const renderStarted = window.__civvisProfileMainCpu ? performance.now() : null;
      const result = Reflect.apply(original, this, [st, ...args]);
      try {
        if (!st || st.turn === undefined || st.seed === undefined) return result;
        const painted = typeof paintedFrame === 'undefined' ? null : paintedFrame;
        if (!painted || painted.seed !== st.seed || painted.sequence !== st.frame_sequence)
          throw new Error('renderer did not acknowledge this complete frame');
        const previous = report.frames.at(-1);
        if (previous && previous.sequence === painted.sequence && previous.seed === painted.seed)
          return result;
        const frame = {...painted, atMs: performance.now(), pace: st.pace,
                       winner: st.winner, victoryType: st.victory_type};
        if (renderStarted !== null) frame.renderMs = frame.atMs - renderStarted;
        if (!previous) {
          report.settings = {
            majors: st.players.filter(p => !p.is_minor && !p.is_barbarian).length,
            cityStates: st.players.filter(p => p.is_minor && !p.is_barbarian).length,
            speed: st.game_speed, turns: st.max_turns, map: st.map.script,
            shape: st.map.shape, width: st.map.width, height: st.map.height,
            difficulty: st.difficulty, turnStructure: st.turn_structure,
            victoryConditions: st.victory_conditions
          };
          report.serverCommit = st.server_commit;
        }
        report.frames.push(frame);
        requestAnimationFrame(() => {
          report.paints.push({...frame, atMs: performance.now()});
          if (frame.finished) report.phase = 'complete';
        });
      } catch (error) { report.error = String(error); }
      return result;
    };
    report.phase = 'observing';
  }, 1);
})();
"""

READ_REPORT_JS = """(() => ({observation: window.__civvisFullGame || null,
  runtime: {hostname: location.hostname, secure: window.isSecureContext,
            publishedSite: window.CIVVIS_PUBLISHED_SITE === true},
  delivery: window.__civvisBeta ? {...window.__civvisBeta, seenTurns: undefined} : null}))()"""

# POST /save calls ask(..., raw=true); read its stored bytes without ever
# parsing a save through JavaScript's binary64 numbers. Extraction is after
# the terminal paint and outside the measured game interval.
SAVE_JS = r"""
(async () => {
  const name = 'ci-full-browser-game';
  const response = await fetch('/save', {method: 'POST', body: JSON.stringify({name})});
  const answer = await response.json();
  if (answer.error) throw new Error(answer.error);
  const db = await new Promise((resolve, reject) => {
    const request = indexedDB.open('civvis-saves', 1);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  const held = await new Promise((resolve, reject) => {
    const request = db.transaction('body').objectStore('body').get(name);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  db.close();
  if (!held) throw new Error('raw save missing from IndexedDB');
  const bytes = new Uint8Array(held);
  let text = '';
  for (let i = 0; i < bytes.length; i += 32768)
    text += String.fromCharCode(...bytes.subarray(i, i + 32768));
  return {base64: btoa(text), bytes: bytes.byteLength};
})()
"""


def case_url(origin: str, case: str) -> str:
    roster, seed = profile.CASES[case]
    query = dict(game=seed, players=6 if roster == 'six' else 4,
                 map='continents' if roster == 'six' else 'tenins_ball',
                 shape='planet', speed='online', turns=250, mode='watch')
    return origin + '/?' + urllib.parse.urlencode(query)


def browser_origin(port: int) -> str:
    # The unmodified page selects public defaults by hostname. Chrome maps
    # this isolated test hostname to our local server; no DNS or site changes.
    return f'http://{PUBLISHED_HOST}:{port}'


def validate_runtime(runtime: dict) -> None:
    if (runtime.get('hostname') != PUBLISHED_HOST or runtime.get('secure') is not True
            or runtime.get('publishedSite') is not True):
        raise ValueError('browser did not select the normal secure public-site branch')


def validate_observation(observation: dict, case: str, source: str) -> dict:
    if observation.get('error') or observation.get('phase') != 'complete':
        raise ValueError('browser did not reach a naturally finished painted game')
    roster, seed = profile.CASES[case]
    frames, paints = observation.get('frames', []), observation.get('paints', [])
    if not frames or len(frames) != len(paints):
        raise ValueError('not every completed render has a paint witness')
    if frames[0].get('sequence') != 0 or frames[0].get('turn') != 1:
        raise ValueError('observer missed the opening frame')
    for index, (frame, paint) in enumerate(zip(frames, paints)):
        if (frame.get('seed') != seed or frame.get('sequence') != index
                or any(paint.get(key) != frame.get(key) for key in ('seed', 'turn', 'sequence', 'finished'))
                or (index and frame.get('pace') != 0)):
            raise ValueError('game identity, publication order or normal Lightning pace changed')
        for key in ('atMs',):
            if (not isinstance(frame.get(key), (int, float))
                    or not isinstance(paint.get(key), (int, float))
                    or not math.isfinite(frame[key]) or not math.isfinite(paint[key])
                    or frame[key] < 0 or paint[key] < frame[key]):
                raise ValueError('invalid render/paint clock')
        if index and (frame['atMs'] < frames[index - 1]['atMs']
                      or paint['atMs'] < paints[index - 1]['atMs']):
            raise ValueError('browser clocks ran backwards')
        if index:
            advance = frame['turn'] - frames[index - 1]['turn']
            if advance != 1 and not (frame.get('finished') and advance == 0):
                raise ValueError('normal Lightning game skipped a round')
        if index < len(frames) - 1 and frame.get('finished'):
            raise ValueError('a successor game was included')
    if frames[-1].get('finished') is not True or not 1 < frames[-1]['turn'] <= 251:
        raise ValueError('missing natural terminal frame')
    commit = observation.get('serverCommit', '')
    if not isinstance(commit, str) or len(commit) < 7 or not source.startswith(commit):
        raise ValueError('browser source metadata differs from qualification')
    return {'roster': roster, 'seed': seed, 'frames': len(frames),
            'terminal_turn': frames[-1]['turn'], 'settings': observation['settings'],
            'navigation_to_final_paint_ms': paints[-1]['atMs'],
            'first_paint_to_final_paint_ms': paints[-1]['atMs'] - paints[0]['atMs']}


def validate_save(raw: bytes, observation: dict, case: str) -> dict:
    game = json.loads(raw)['game']  # Python preserves original u64 integers.
    roster, seed = profile.CASES[case]
    terminal = observation['frames'][-1]
    expected = 6 if roster == 'six' else 4
    if (game.get('seed') != seed or game.get('turn') != terminal['turn']
            or game.get('game_speed') != 'online' or game.get('max_turns') != 250
            or game.get('human_seats')
            or (game.get('winner') is None and game.get('victory_type') != 'draw')
            or game.get('winner') != terminal.get('winner')
            or game.get('victory_type') != terminal.get('victoryType')
            or game.get('map_script') != observation['settings']['map']
            or any(game['map'].get(key) != observation['settings'][key] for key in ('width', 'height'))
            or not game['map'].get('topology', {}).get('globe')
            or game.get('victory_conditions') != observation['settings']['victoryConditions']
            or game.get('difficulty') != 'prince' or game.get('turn_structure') != 'sequential'
            or sum(p['is_minor'] and not p.get('is_barbarian', False)
                   for p in game['players']) != (9 if roster == 'six' else 6)
            or sum(not p['is_minor'] and not p.get('is_barbarian', False)
                   for p in game['players']) != expected):
        raise ValueError('stored raw save is not the measured complete normal game')
    return {'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}


def verify_module(qualified: Path, source: str, run_id: int, repo: str) -> tuple[Path, dict]:
    module, receipt = pair.qualified(qualified, source)
    run = json.loads(subprocess.check_output(
        ['gh', 'api', f'repos/{repo}/actions/runs/{run_id}'], text=True))
    profile.check_run(run, run_id, repo, receipt)
    return module, receipt


DURATION_METRICS = ('ThreadTime', 'TaskDuration', 'ScriptDuration', 'LayoutDuration',
                    'RecalcStyleDuration')


def finite_number(value) -> bool:
    return type(value) in (int, float) and math.isfinite(value)


def main_cpu_summary(cpu: dict, metrics: dict, observation: dict) -> dict:
    """Validate main-thread evidence; sampling intervals are not CPU seconds.

    CDP Profiler timestamps/deltas are microseconds. Performance durations use
    threadTicks (CPU seconds), including native work absent from JS samples.
    The renderer process's engine worker is outside both measurements.
    """
    nodes, samples, deltas = cpu.get('nodes', []), cpu.get('samples', []), cpu.get('timeDeltas', [])
    if (not nodes or not samples or len(samples) != len(deltas)
            or not all(finite_number(cpu.get(k)) for k in ('startTime', 'endTime'))
            or cpu['endTime'] <= cpu['startTime']
            or not all(finite_number(d) and d >= 0 for d in deltas) or sum(deltas) <= 0):
        raise ValueError('invalid main-thread profile clocks or samples')
    by_id = {node['id']: node for node in nodes}
    if len(by_id) != len(nodes) or any(type(i) is not int for i in by_id):
        raise ValueError('duplicate or invalid CPU node IDs')
    parents = {}
    for node in nodes:
        for child in node.get('children', []):
            if child not in by_id or child in parents:
                raise ValueError('invalid CPU profile tree')
            parents[child] = node['id']
    if len(set(by_id) - parents.keys()) != 1:
        raise ValueError('CPU profile must have one root')
    frontend = set()
    for identity in by_id:
        visited, current, in_app = set(), identity, False
        while current is not None:
            if current in visited:
                raise ValueError('cycle in CPU profile')
            visited.add(current)
            frame = by_id[current].get('callFrame', {})
            in_app |= urllib.parse.urlparse(frame.get('url', '')).path == '/assets/app.js'
            current = parents.get(current)
        if in_app:
            frontend.add(identity)
    if any(type(i) is not int or i not in by_id for i in samples):
        raise ValueError('sample refers to unknown CPU node')
    if not frontend.intersection(samples):
        raise ValueError('profile has no sampled frontend app stack')
    weights = {}
    for identity, delta in zip(samples, deltas):
        weights[identity] = weights.get(identity, 0) + delta
    top = [dict(node=identity, interval_us=weight, callFrame=by_id[identity]['callFrame'])
           for identity, weight in sorted(weights.items(), key=lambda item: -item[1])]
    duration = {}
    for key in DURATION_METRICS:
        before, after = [dict((m['name'], m['value']) for m in metrics[phase]['metrics']).get(key)
                         for phase in ('before', 'after')]
        if not finite_number(before) or not finite_number(after) or before < 0 or after < before:
            raise ValueError('missing or invalid thread CPU duration: ' + key)
        duration[key] = after - before
    renders = [frame.get('renderMs') for frame in observation['frames']]
    if not renders or not all(finite_number(value) and value >= 0 for value in renders):
        raise ValueError('missing or invalid complete-render durations')
    ordered = sorted(renders)
    return dict(scope='Main renderer thread, startup through detection of terminal paint; '
                'excludes engine worker and screenshot/save extraction. Sampling adds overhead; '
                'interval shares include idle and are not CPU shares or a speed result.',
                sample_count=len(samples), profile_interval_us=cpu['endTime'] - cpu['startTime'],
                sampled_interval_us=sum(deltas), thread_cpu_seconds=duration,
                render_wall_ms=dict(total=sum(renders), median=statistics.median(ordered),
                                    p95=ordered[math.ceil(.95*len(ordered))-1], maximum=max(renders)),
                top_self_intervals=top[:50])


def browser_game(qualified: Path, source: str, run_id: int, repo: str,
                 case: str, out: Path, seconds: float = 1200, chrome: Path | None = None,
                 main_cpu_profile: bool = False) -> dict:
    if (case not in profile.CASES or run_id <= 0 or not 0 < seconds <= 1800
            or not re.fullmatch(r'[\w.-]+/[\w.-]+', repo)):
        raise ValueError('invalid normal case, qualification run or deadline')
    if out.exists():
        raise FileExistsError('inspect original browser evidence; output already exists')
    module, qualification = verify_module(qualified, source, run_id, repo)
    spec = importlib.util.spec_from_file_location('civvis_beta_verify', Path(__file__).resolve().parents[1] / 'beta/verify.py')
    verify = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(verify)
    chrome = str(chrome.resolve()) if chrome is not None else verify.find_chrome()
    if not Path(chrome).exists():
        raise FileNotFoundError('Chrome required; no browser check may silently skip')
    out.mkdir(parents=True)
    (out / 'qualification').mkdir()
    shutil.copyfile(module, out / 'qualification/civvis.wasm')
    shutil.copytree(qualified / 'wasm-symbols', out / 'wasm-symbols')
    shutil.copyfile(qualified / 'qualification-info.json', out / 'qualification-info.json')
    manifest = {str(p.relative_to(module.parent)): hashlib.sha256(p.read_bytes()).hexdigest()
                for p in sorted(module.parent.rglob('*')) if p.is_file()}
    result = {'phase': 'starting', 'source': source, 'qualification': qualification,
              'qualification_run': run_id, 'case': case, 'bundle_sha256': manifest,
              'tooling_source': os.environ.get('GITHUB_SHA'), 'platform': platform.platform(),
              'settings_guard_sha256': hashlib.sha256(pair.DRIVER.read_bytes()).hexdigest(),
              'logical_cpus': os.cpu_count(), 'chrome': subprocess.check_output([chrome, '--version'], text=True).strip(),
              'chrome_executable_sha256': hashlib.sha256(Path(chrome).read_bytes()).hexdigest(),
              'scope': 'Exact qualified website bundle served locally under a browser-mapped civvis.ai hostname on named remote CI host. '
                       'Natural normal Online game at offered Lightning pace, all publications rendered. '
                       'Headless Chrome, GPU disabled,1600x1000,CDP screencast. '
                       'Includes real renderer/worker work; excludes public network variability. '
                       'Observed diagnostic timing, not a paired engine speed claim or laptop milestone.'}
    receipt = out / 'receipt.json'
    def record():
        receipt.write_text(json.dumps(result, indent=2) + '\n')
    record()
    httpd = verify.serve(module.parent, verify.free_port())
    port = httpd.server_address[1]
    browser_profile = verify.fresh_profile('civvis-full-browser-')
    debug_port = verify.free_port()
    command = [chrome, '--headless=new', f'--remote-debugging-port={debug_port}',
               f'--host-resolver-rules=MAP {PUBLISHED_HOST} 127.0.0.1',
               '--no-proxy-server', '--disable-features=HttpsUpgrades',
               f'--unsafely-treat-insecure-origin-as-secure={browser_origin(port)}',
               f'--user-data-dir={browser_profile}', '--no-first-run', '--no-default-browser-check',
               '--disable-gpu', '--window-size=1600,1000', '--hide-scrollbars',
               '--autoplay-policy=no-user-gesture-required', 'about:blank']
    browser = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    verify.tether(browser)
    result['browser_arguments'] = command
    dev = None
    try:
        deadline = time.monotonic() + 30
        target = None
        while time.monotonic() < deadline:
            try:
                targets = json.loads(urllib.request.urlopen(f'http://127.0.0.1:{debug_port}/json/list', timeout=2).read())
                target = next((x for x in targets if x.get('type') == 'page' and x.get('webSocketDebuggerUrl')), None)
                if target:
                    break
            except (OSError, ValueError):
                pass
            time.sleep(.25)
        if not target:
            raise RuntimeError('Chrome debugging target missing')
        dev = verify.Devtools(target['webSocketDebuggerUrl'])
        dev.call('Runtime.enable')
        dev.call('Page.enable')
        observer = ('window.__civvisProfileMainCpu = true;\n' if main_cpu_profile else '') + OBSERVER_JS
        dev.call('Page.addScriptToEvaluateOnNewDocument', {'source': observer})
        dev.call('Page.startScreencast', {'format': 'jpeg', 'quality': 40, 'maxWidth': 800, 'maxHeight': 500})
        url = case_url(browser_origin(port), case)
        result.update(phase='observing normal game', url=url)
        record()
        metrics = {}
        if main_cpu_profile:
            dev.call('Performance.enable', {'timeDomain': 'threadTicks'})
            dev.call('Profiler.enable')
            dev.call('Profiler.setSamplingInterval', {'interval': 500})
            metrics['before'] = dev.call('Performance.getMetrics')
            dev.call('Profiler.start')
        dev.call('Page.navigate', {'url': url})
        deadline = time.monotonic() + seconds
        snapshot = {}
        while time.monotonic() < deadline:
            snapshot = dev.evaluate(READ_REPORT_JS) or {}
            (out / 'observation.json').write_text(json.dumps(snapshot, indent=2) + '\n')
            observed, delivery = snapshot.get('observation') or {}, snapshot.get('delivery') or {}
            if observed.get('error') or delivery.get('error') or delivery.get('presetError'):
                raise RuntimeError(json.dumps(snapshot))
            if observed.get('phase') == 'complete':
                break
            time.sleep(.2)
        else:
            raise TimeoutError('natural finished paint not observed; partial evidence retained')
        observation = snapshot['observation']
        if main_cpu_profile:
            cpu = dev.call('Profiler.stop')['profile']
            (out / 'main.cpuprofile').write_text(json.dumps(cpu) + '\n')
            metrics['after'] = dev.call('Performance.getMetrics')
            (out / 'main-thread-metrics.json').write_text(json.dumps(metrics, indent=2) + '\n')
        # Preserve the terminal picture and raw world even when a settings or
        # cadence guard rejects this observation afterward.
        shot = dev.call('Page.captureScreenshot', {'format': 'png'})
        (out / 'terminal.png').write_bytes(base64.b64decode(shot['data']))
        held = dev.evaluate(SAVE_JS)
        raw = base64.b64decode(held['base64'], validate=True)
        if len(raw) != held['bytes']:
            raise ValueError('IndexedDB byte count differs from transferred raw save')
        (out / 'game.save.json').write_bytes(raw)
        if main_cpu_profile:
            result['main_cpu'] = main_cpu_summary(cpu, metrics, observation)
        validate_runtime(snapshot.get('runtime') or {})
        timing = validate_observation(observation, case, source)
        # Reuse the frozen driver's pure settings guard without executing its CLI.
        subprocess.run(['node', '--input-type=module', '-e', profile.SETTINGS_CHECK,
                        pair.DRIVER.resolve().as_uri(), json.dumps(timing['settings']), timing['roster']],
                       check=True, text=True, capture_output=True)
        saved = validate_save(raw, observation, case)
        colours = dev.evaluate(verify.MAP_CANVAS_COLOURS_JS)
        if not isinstance(colours, int) or colours < 8:
            raise ValueError('terminal map did not produce the normal painted canvas')
        problems = verify.map_delivery_problems(snapshot.get('delivery') or {})
        if problems:
            raise ValueError('browser map delivery failed: ' + '; '.join(problems))
        if hashlib.sha256(module.read_bytes()).hexdigest() != qualification['engine']['wasm_sha256']:
            raise ValueError('served module changed during browser observation')
        if (hashlib.sha256(Path(chrome).read_bytes()).hexdigest() != result['chrome_executable_sha256']
                or subprocess.check_output([chrome, '--version'], text=True).strip() != result['chrome']):
            raise ValueError('browser changed during observation')
        result.update(phase='complete', timing=timing, raw_save=saved,
                      console=dev.console, delivery=snapshot.get('delivery'), runtime=snapshot.get('runtime'),
                      terminal_canvas_colours=colours)
        record()
        return result
    except BaseException as error:
        result.update(phase='failed; partial browser evidence retained', error=repr(error))
        record()
        raise
    finally:
        if dev:
            try:
                dev.call('Page.stopScreencast', wait=False)
            except (OSError, RuntimeError):
                pass
            dev.alive = False
        browser.terminate()
        try:
            browser.wait(timeout=10)
        except subprocess.TimeoutExpired:
            browser.kill()
            browser.wait()
        httpd.shutdown()
        httpd.server_close()
        shutil.rmtree(browser_profile, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qualified', type=Path, required=True)
    parser.add_argument('--source', required=True)
    parser.add_argument('--run', type=int, required=True)
    parser.add_argument('--repo', required=True)
    parser.add_argument('--case', choices=sorted(profile.CASES), required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--chrome', type=Path)
    parser.add_argument('--main-cpu-profile', action='store_true')
    args = parser.parse_args()
    print(json.dumps(browser_game(args.qualified.resolve(), args.source, args.run,
                                  args.repo, args.case, args.out.resolve(),
                                  chrome=args.chrome, main_cpu_profile=args.main_cpu_profile)))


if __name__ == '__main__':
    main()
