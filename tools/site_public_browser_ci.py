#!/usr/bin/env python3
"""Manually observe a full normal game at the actual public HTTPS website.

Manual CI uses a fresh fixed Chrome on a hosted Mac. No game starts on import.
The conservative wall clock includes navigation, download, setup, every normal
turn, rendering, observer overhead and receipt of the terminal screenshot.
This diagnostic establishes observations on its named host, not AI-strength
approval, an optimization comparison, or performance on every user's laptop.
"""
from __future__ import annotations

import argparse
import base64
import datetime
import hashlib
import importlib.util
import json
import os
import platform
import queue
import re
import shutil
import socket
import subprocess
import threading
import time
import urllib.parse
import urllib.request
from pathlib import Path

import site_wasm_browser_ci as browser
import site_wasm_pair as pair
import site_wasm_profile_ci as profile

ORIGIN = 'https://www.civvis.ai'
IDENTITY_PATHS = ('/', '/build.json', '/civvis.wasm', '/shim.js',
                  '/worker.js', '/assets/app.js')
UA = 'Mozilla/5.0 (CIVVIS public browser measurement)'
ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location('public_beta_verify', ROOT / 'beta/verify.py')
verify = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(verify)


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def fetch(path: str) -> tuple[bytes, dict]:
    """Read the real origin; redirects outside the stable origin are invalid."""
    request = urllib.request.Request(ORIGIN + path, headers={
        'User-Agent': UA, 'Cache-Control': 'no-cache'})
    with urllib.request.urlopen(request, timeout=60) as response:
        if response.status != 200 or urllib.parse.urlsplit(response.url).netloc != 'www.civvis.ai':
            raise ValueError('public identity request changed origin or failed')
        raw = response.read()
        return raw, dict(url=response.url, status=response.status, bytes=len(raw),
                         sha256=sha(raw), content_type=response.headers.get('Content-Type'))


def validate_identity(bodies: dict[str, bytes], source: str, wasm: str) -> dict:
    if not re.fullmatch('[0-9a-f]{40}', source) or not re.fullmatch('[0-9a-f]{64}', wasm):
        raise ValueError('full expected source and module hashes required')
    manifest = json.loads(bodies['/build.json'])
    module = bodies['/civvis.wasm']
    if (manifest.get('commit') != source or sha(module) != wasm
            or not module.startswith(b'\x00asm')
            or manifest.get('wasm_bytes') != len(module)
            or b'shim.js' not in bodies['/']):
        raise ValueError('actual public source/module/HTML identity differs from expectation')
    return {path: sha(raw) for path, raw in bodies.items()}


def snapshot(out: Path, source: str, wasm: str) -> dict:
    out.mkdir()
    bodies, requests = {}, {}
    for path in IDENTITY_PATHS:
        raw, receipt = fetch(path)
        bodies[path], requests[path] = raw, receipt
        target = out / ('index.html' if path == '/' else path.lstrip('/'))
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(raw)
    hashes = validate_identity(bodies, source, wasm)
    (out / 'requests.json').write_text(json.dumps(requests, indent=2) + '\n')
    return hashes


class Recorder:
    """Observe real Page/Worker network responses without replacing requests."""

    def __init__(self, url: str):
        self.ws = verify.WebSocket(url)
        self.lock = threading.Lock()
        self.next_id = 1
        self.answers = {}
        self.console = []
        self.responses = {}
        self.alive = True
        self.thread = threading.Thread(target=self._read, daemon=True)
        self.thread.start()

    def close(self):
        self.alive = False
        try:
            self.ws.sock.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.ws.sock.close()
        self.thread.join(timeout=2)

    def call(self, method: str, params: dict | None = None,
             wait: bool = True, session: str | None = None):
        with self.lock:
            ident = self.next_id
            self.next_id += 1
            box = queue.Queue()
            if wait:
                self.answers[ident] = box
            message = dict(id=ident, method=method, params=params or {})
            if session:
                message['sessionId'] = session
            # One writer at a time: worker attachment events can send commands.
            self.ws.send(json.dumps(message))
        if not wait:
            return None
        try:
            answer = box.get(timeout=60)
        except queue.Empty as error:
            raise RuntimeError(method + ' did not answer') from error
        if 'error' in answer:
            raise RuntimeError(method + ': ' + str(answer['error']))
        return answer.get('result', {})

    def _read(self):
        while self.alive:
            try:
                message = json.loads(self.ws.recv())
                if 'id' in message:
                    with self.lock:
                        box = self.answers.pop(message['id'], None)
                    if box:
                        box.put(message)
                    continue
                method, params = message.get('method'), message.get('params', {})
                session = message.get('sessionId')
                if method == 'Target.attachedToTarget':
                    child = params['sessionId']
                    self.call('Network.enable', network_options(), wait=False, session=child)
                    self.call('Runtime.enable', wait=False, session=child)
                    self.call('Runtime.runIfWaitingForDebugger', wait=False, session=child)
                elif method == 'Network.responseReceived':
                    self.responses[(session, params['requestId'])] = dict(
                        response=params['response'], resource_type=params.get('type'),
                        session=session, request_id=params['requestId'], finished=False)
                elif method == 'Network.loadingFinished':
                    row = self.responses.get((session, params['requestId']))
                    if row is not None:
                        row['finished'] = True
                        row['encoded_data_length'] = params['encodedDataLength']
                elif method == 'Page.screencastFrame':
                    self.call('Page.screencastFrameAck',
                              {'sessionId': params['sessionId']}, wait=False)
                elif method == 'Runtime.exceptionThrown':
                    self.console.append(params['exceptionDetails'])
            except BaseException as error:
                if self.alive:
                    self.console.append(dict(observer_error=repr(error)))
                self.alive = False

    def evaluate(self, expression: str):
        value = self.call('Runtime.evaluate', dict(
            expression=expression, returnByValue=True, awaitPromise=True))
        if value.get('exceptionDetails'):
            raise RuntimeError(str(value['exceptionDetails']))
        return value.get('result', {}).get('value')


def network_options() -> dict:
    return dict(maxTotalBufferSize=160 * 1024 * 1024,
                maxResourceBufferSize=32 * 1024 * 1024)


def response_path(row: dict) -> str | None:
    url = urllib.parse.urlsplit(row['response']['url'])
    if url.scheme != 'https' or url.netloc != 'www.civvis.ai':
        return None
    if (url.path in IDENTITY_PATHS or url.path.endswith(('.js', '.css', '.wasm'))
            or row.get('resource_type') == 'Document'):
        return url.path or '/'
    return None


def validate_loaded(loaded: list[dict], before: dict, after: dict, wasm: str) -> dict:
    """Every captured executable must equal the public post-game readback."""
    if any(after.get(path) != value for path, value in before.items()):
        raise ValueError('public deployment changed during this game')
    found = {}
    for row in loaded:
        path = row['path']
        if (not row['finished'] or row['status'] != 200
                or row['sha256'] != after.get(path)):
            raise ValueError('loaded public response differs from readback: ' + path)
        found.setdefault(path, []).append(row['sha256'])
    if any(path not in found for path in IDENTITY_PATHS):
        raise ValueError('missing original page/worker/module/frontend response body')
    if any(value != wasm for value in found['/civvis.wasm']):
        raise ValueError('browser loaded a different WebAssembly module')
    return found


def checked_wall(start: int, screenshot_received: int) -> float:
    if type(start) is not int or type(screenshot_received) is not int or screenshot_received <= start:
        raise ValueError('invalid conservative screenshot receipt clock')
    return (screenshot_received - start) / 1_000_000


def observe(case: str, source: str, wasm: str, chrome: Path, out: Path,
            seconds: float = 1200, repeat: int = 1) -> dict:
    if case not in profile.CASES or repeat not in (1, 2) or not 0 < seconds <= 1800:
        raise ValueError('invalid normal case or deadline')
    if out.exists():
        raise FileExistsError('preserve original observation; never replay into its output')
    chrome = chrome.resolve()
    if not chrome.is_file():
        raise FileNotFoundError('fixed Chrome executable required')
    out.mkdir(parents=True)
    receipt = out / 'receipt.json'
    result = dict(phase='verify public identity before game', source=source,
                  wasm_sha256=wasm, case=case, repeat=repeat, tooling_source=os.environ.get('GITHUB_SHA'),
                  platform=platform.platform(), machine=platform.machine(),
                  logical_cpus=os.cpu_count(),
                  chrome=subprocess.check_output([str(chrome), '--version'], text=True).strip(),
                  chrome_executable_sha256=sha(chrome.read_bytes()),
                  settings_guard_sha256=sha(pair.DRIVER.read_bytes()),
                  scope=__doc__, observer='render/rAF witness and CDP screencast; worker debugger '
                  'briefly waits while network observation attaches. All observer overhead '
                  'is inside conservative game wall. Identity HTTP requests precede navigation '
                  'and may warm host DNS/CDN; each Chrome profile starts empty.',
                  website_goal_milestones_verified=False)
    def record():
        receipt.write_text(json.dumps(result, indent=2) + '\n')
    record()
    proc, dev, gpu_dev, user_profile = None, None, None, None
    try:
        before = snapshot(out / 'public-before', source, wasm)
        result['public_before'] = before
        user_profile = verify.fresh_profile('civvis-actual-public-')
        port = verify.free_port()
        arguments = [str(chrome), '--headless=new', '--remote-debugging-port=' + str(port),
                     '--user-data-dir=' + str(user_profile), '--no-first-run',
                     '--no-default-browser-check', '--window-size=1600,1000',
                     '--autoplay-policy=no-user-gesture-required', 'about:blank']
        result['browser_arguments'] = arguments
        proc = subprocess.Popen(arguments, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        verify.tether(proc)
        deadline = time.monotonic() + 30
        target = None
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                raise RuntimeError('Chrome exited before observation')
            try:
                targets = json.loads(urllib.request.urlopen(
                    'http://127.0.0.1:' + str(port) + '/json/list', timeout=2).read())
                target = next((x for x in targets if x.get('type') == 'page'
                               and x.get('webSocketDebuggerUrl')), None)
                if target:
                    break
            except (OSError, ValueError):
                pass
            time.sleep(.25)
        if not target:
            raise RuntimeError('Chrome page debugging target missing')
        info = json.loads(urllib.request.urlopen(
            'http://127.0.0.1:' + str(port) + '/json/version', timeout=2).read())
        gpu_dev = Recorder(info['webSocketDebuggerUrl'])
        result['gpu'] = gpu_dev.call('SystemInfo.getInfo')
        dev = Recorder(target['webSocketDebuggerUrl'])
        dev.call('Runtime.enable')
        dev.call('Page.enable')
        dev.call('Network.enable', network_options())
        dev.call('Target.setAutoAttach', dict(autoAttach=True,
                                             waitForDebuggerOnStart=True, flatten=True))
        dev.call('Page.addScriptToEvaluateOnNewDocument', {'source': browser.OBSERVER_JS})
        dev.call('Page.startScreencast', dict(format='jpeg', quality=40, maxWidth=800, maxHeight=500))
        result.update(phase='observe actual public normal game', url=browser.case_url(ORIGIN, case))
        record()
        started = time.monotonic_ns()
        dev.call('Page.navigate', {'url': result['url']})
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if not dev.alive:
                raise RuntimeError('browser observation connection ended')
            observed = dev.evaluate(browser.READ_REPORT_JS) or {}
            (out / 'observation.json').write_text(json.dumps(observed, indent=2) + '\n')
            view = observed.get('observation') or {}
            delivery = observed.get('delivery') or {}
            if view.get('error') or delivery.get('error') or delivery.get('presetError'):
                raise RuntimeError('original browser reported an error: ' + json.dumps(observed))
            if view.get('phase') == 'complete':
                break
            time.sleep(.2)
        else:
            raise TimeoutError('natural terminal frame missing; no game retry')
        screenshot = dev.call('Page.captureScreenshot', {'format': 'png'})
        received = time.monotonic_ns()
        (out / 'terminal.png').write_bytes(base64.b64decode(screenshot['data'], validate=True))
        result['navigation_to_terminal_screenshot_received_ms'] = checked_wall(started, received)
        held = dev.evaluate(browser.SAVE_JS)
        raw = base64.b64decode(held['base64'], validate=True)
        (out / 'game.save.json').write_bytes(raw)
        if len(raw) != held['bytes']:
            raise ValueError('raw save byte count changed in transfer')
        runtime = observed.get('runtime') or {}
        if runtime != dict(hostname='www.civvis.ai', secure=True, publishedSite=True):
            raise ValueError('actual stable public HTTPS branch was not selected')
        timing = browser.validate_observation(view, case, source)
        saved = browser.validate_save(raw, view, case)
        subprocess.run(['node', '--input-type=module', '-e', profile.SETTINGS_CHECK,
                        pair.DRIVER.resolve().as_uri(), json.dumps(timing['settings']),
                        timing['roster']], check=True, capture_output=True, text=True)
        if verify.map_delivery_problems(delivery):
            raise ValueError('public map delivery contract failed')
        if dev.console:
            raise ValueError('uncaught browser or observer exception; original evidence retained')
        after = snapshot(out / 'public-after', source, wasm)
        loaded = []
        # The terminal screenshot and save have already been preserved.
        for index, row in enumerate(list(dev.responses.values())):
            path = response_path(row)
            if path is None:
                continue
            if not row['finished']:
                raise ValueError('incomplete original frontend response: ' + path)
            body = dev.call('Network.getResponseBody', {'requestId': row['request_id']},
                            session=row['session'])
            data = (base64.b64decode(body['body'], validate=True) if body.get('base64Encoded')
                    else body['body'].encode('utf-8'))
            folder = out / 'loaded-responses'
            folder.mkdir(exist_ok=True)
            body_file = Path('loaded-responses') / (str(index) + '.body')
            (out / body_file).write_bytes(data)
            if path not in after:
                fetched, _ = fetch(path)
                after[path] = sha(fetched)
                discovered = out / 'public-after/discovered'
                discovered.mkdir(exist_ok=True)
                (discovered / (sha(path.encode()) + '.body')).write_bytes(fetched)
            loaded.append(dict(path=path, status=row['response']['status'],
                               finished=row['finished'], sha256=sha(data), bytes=len(data),
                               body_file=str(body_file),
                               session=row['session'], request_id=row['request_id'],
                               response=row['response']))
        (out / 'loaded-responses.json').write_text(json.dumps(loaded, indent=2) + '\n')
        found = validate_loaded(loaded, before, after, wasm)
        if sha(chrome.read_bytes()) != result['chrome_executable_sha256']:
            raise ValueError('Chrome executable changed during observation')
        result.update(phase='complete', timing=timing, raw_save=saved, runtime=runtime,
                      delivery=delivery, public_after=after, loaded_response_hashes=found,
                      monotonic_ns=dict(navigation=started, terminal_screenshot_received=received),
                      at=datetime.datetime.now(datetime.timezone.utc).isoformat())
        record()
        return result
    except BaseException as error:
        result.update(phase='failed; preserve original public evidence; no replay', error=repr(error))
        record()
        raise
    finally:
        for connection in (dev, gpu_dev):
            if connection:
                connection.close()
        if proc:
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
        if user_profile:
            shutil.rmtree(user_profile, ignore_errors=True)



def summarize(root: Path, out: Path) -> dict:
    """Audit exactly the twelve original observations; report every result."""
    if out.exists():
        raise FileExistsError('preserve original summary')
    paths = sorted(root.rglob('receipt.json'))
    if len(paths) != 2 * len(profile.CASES):
        raise ValueError('all twelve original observations required')
    receipts = [json.loads(p.read_text()) for p in paths]
    identities = [(r['case'], r['repeat']) for r in receipts]
    if (len(set(identities)) != len(identities)
            or set(identities) != {(case, repeat) for case in profile.CASES for repeat in (1, 2)}
            or any(r['phase'] != 'complete' for r in receipts)):
        raise ValueError('missing, duplicate, partial or failed original game')
    common = receipts[0]
    seen_assets, saves, rows = {}, {}, []
    for path, receipt in zip(paths, receipts):
        if any(receipt[key] != common[key] for key in
               ('source', 'wasm_sha256', 'public_before', 'chrome',
                'chrome_executable_sha256', 'settings_guard_sha256', 'tooling_source')):
            raise ValueError('mixed public release, browser or observer')
        for phase in ('before', 'after'):
            raw_snapshot = {}
            for asset, digest in receipt['public_' + phase].items():
                name = ('index.html' if asset == '/' else asset.lstrip('/')) if asset in IDENTITY_PATHS else 'discovered/' + sha(asset.encode()) + '.body'
                raw_snapshot[asset] = (path.parent / ('public-' + phase) / name).read_bytes()
                if sha(raw_snapshot[asset]) != digest:
                    raise ValueError('original public identity body changed')
            validate_identity(raw_snapshot, receipt['source'], receipt['wasm_sha256'])
        loaded = json.loads((path.parent / 'loaded-responses.json').read_text())
        for row in loaded:
            name = row['body_file']
            if not re.fullmatch(r'loaded-responses/[0-9]+[.]body', name):
                raise ValueError('invalid original response body path')
            raw_body = (path.parent / name).read_bytes()
            if sha(raw_body) != row['sha256'] or len(raw_body) != row['bytes']:
                raise ValueError('original loaded response body changed')
        if validate_loaded(loaded, receipt['public_before'], receipt['public_after'],
                           receipt['wasm_sha256']) != receipt['loaded_response_hashes']:
            raise ValueError('loaded response inventory changed')
        for asset, hashes in receipt['loaded_response_hashes'].items():
            if len(set(hashes)) != 1 or (asset in seen_assets and seen_assets[asset] != hashes[0]):
                raise ValueError('loaded frontend assets differ across original games')
            seen_assets[asset] = hashes[0]
        observation = json.loads((path.parent / 'observation.json').read_text())['observation']
        timing = browser.validate_observation(observation, receipt['case'], receipt['source'])
        raw = (path.parent / 'game.save.json').read_bytes()
        if browser.validate_save(raw, observation, receipt['case']) != receipt['raw_save']:
            raise ValueError('original raw save changed')
        subprocess.run(['node', '--input-type=module', '-e', profile.SETTINGS_CHECK,
                        pair.DRIVER.resolve().as_uri(), json.dumps(timing['settings']),
                        timing['roster']], check=True, capture_output=True, text=True)
        if receipt['case'] in saves and saves[receipt['case']] != raw:
            raise ValueError('repeated original normal games diverged')
        saves[receipt['case']] = raw
        image = (path.parent / 'terminal.png').read_bytes()
        if not image.startswith(b'\x89PNG\r\n\x1a\n') or len(image) < 1024:
            raise ValueError('original terminal screenshot missing')
        clock = receipt['monotonic_ns']
        wall = checked_wall(clock['navigation'], clock['terminal_screenshot_received'])
        if wall != receipt['navigation_to_terminal_screenshot_received_ms']:
            raise ValueError('original wall receipt changed')
        rows.append(dict(case=receipt['case'], repeat=receipt['repeat'],
                         screenshot_receipt_wall_ms=wall, timing=timing,
                         raw_save_sha256=sha(raw), screenshot_sha256=sha(image)))
    result = dict(phase='all twelve original actual-public games audited',
                  source=common['source'], wasm_sha256=common['wasm_sha256'],
                  public_identity=common['public_before'], loaded_assets=seen_assets,
                  observations=rows, website_goal_milestones_verified=False,
                  scope='Named hosted Macs, actual public HTTPS, fresh fixed Chrome; '
                  'all twelve conservative wall observations retained. This does not '
                  'approve AI strength or establish universal laptop performance.')
    out.mkdir()
    (out / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', choices=sorted(profile.CASES))
    parser.add_argument('--repeat', type=int, choices=(1, 2))
    parser.add_argument('--source')
    parser.add_argument('--wasm-sha256')
    parser.add_argument('--chrome', type=Path)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--summarize', type=Path)
    args = parser.parse_args()
    if args.summarize:
        result = summarize(args.summarize.resolve(), args.out.resolve())
    else:
        if not all((args.case, args.repeat, args.source, args.wasm_sha256, args.chrome)):
            parser.error('observation requires case/repeat/source/module/Chrome')
        result = observe(args.case, args.source, args.wasm_sha256, args.chrome,
                         args.out.resolve(), repeat=args.repeat)
    print(json.dumps(result))


if __name__ == '__main__':
    main()
