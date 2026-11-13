"""Reject mixed releases, incomplete observations and altered original evidence."""
import base64
import http.server
import json
import os
import subprocess
import threading
import time
import urllib.request
from pathlib import Path
import tempfile
import unittest

import site_public_browser_ci as public
import site_wasm_browser_ci as browser
from test_site_wasm_browser_ci import observation, save_game

SOURCE = 'a' * 40
MODULE = b'\x00asm\x01\x00\x00\x00'
WASM = public.sha(MODULE)


def bodies():
    return {
        '/': b'<script src="shim.js"></script>',
        '/build.json': json.dumps(dict(commit=SOURCE, wasm_bytes=len(MODULE))).encode(),
        '/civvis.wasm': MODULE, '/shim.js': b'shim', '/worker.js': b'worker',
        '/assets/app.js': b'app',
    }


def loaded():
    return [dict(path=path, status=200, finished=True, sha256=public.sha(raw),
                 bytes=len(raw), body_file='loaded-responses/' + str(index) + '.body')
            for index, (path, raw) in enumerate(bodies().items())]


def fixtures(root):
    before = public.validate_identity(bodies(), SOURCE, WASM)
    for case, (roster, seed) in public.profile.CASES.items():
        for repeat in (1, 2):
            folder = root / (case + '-' + str(repeat))
            folder.mkdir()
            view = observation()
            game = save_game()
            for frame in view['frames'] + view['paints']:
                frame['seed'] = seed
            game['game']['seed'] = seed
            if roster == 'six':
                view['settings'].update(majors=6, cityStates=9, map='continents',
                                        width=90, height=38)
                game['game'].update(map_script='continents',
                                   players=[dict(is_minor=False)] * 6 + [dict(is_minor=True)] * 9)
                game['game']['map'].update(width=90, height=38)
            raw = json.dumps(game).encode()
            receipt = dict(phase='complete', source=SOURCE, wasm_sha256=WASM,
                           case=case, repeat=repeat, public_before=before,
                           public_after=before, chrome='fixed Chrome',
                           chrome_executable_sha256='b' * 64,
                           settings_guard_sha256=public.sha(public.pair.DRIVER.read_bytes()),
                           tooling_source='c' * 40,
                           loaded_response_hashes={r['path']: [r['sha256']] for r in loaded()},
                           raw_save=browser.validate_save(raw, view, case),
                           monotonic_ns=dict(navigation=1, terminal_screenshot_received=2_000_000_001),
                           navigation_to_terminal_screenshot_received_ms=2000)
            (folder / 'receipt.json').write_text(json.dumps(receipt))
            (folder / 'observation.json').write_text(json.dumps(dict(observation=view)))
            (folder / 'game.save.json').write_bytes(raw)
            (folder / 'terminal.png').write_bytes(b'\x89PNG\r\n\x1a\n' + b'fixture' * 200)
            (folder / 'loaded-responses.json').write_text(json.dumps(loaded()))
            (folder / 'loaded-responses').mkdir()
            for row, raw_body in zip(loaded(), bodies().values()):
                (folder / row['body_file']).write_bytes(raw_body)
            for phase in ('before', 'after'):
                for asset, raw_body in bodies().items():
                    name = 'index.html' if asset == '/' else asset.lstrip('/')
                    body_path = folder / ('public-' + phase) / name
                    body_path.parent.mkdir(parents=True, exist_ok=True)
                    body_path.write_bytes(raw_body)


class PublicIdentityGuards(unittest.TestCase):
    def test_expected_actual_public_bytes_are_bound(self):
        hashes = public.validate_identity(bodies(), SOURCE, WASM)
        self.assertEqual(hashes['/civvis.wasm'], WASM)
        self.assertEqual(public.validate_loaded(loaded(), hashes, hashes, WASM)['/worker.js'],
                         [public.sha(b'worker')])

    def test_wrong_source_module_size_or_html_fail(self):
        for path, raw in [
                ('/build.json', json.dumps(dict(commit='b' * 40, wasm_bytes=len(MODULE))).encode()),
                ('/build.json', json.dumps(dict(commit=SOURCE, wasm_bytes=1)).encode()),
                ('/civvis.wasm', b'not a module'), ('/', b'landing page')]:
            with self.subTest(path=path):
                changed = bodies()
                changed[path] = raw
                with self.assertRaises(ValueError):
                    public.validate_identity(changed, SOURCE, WASM)

    def test_full_hashes_and_source_required(self):
        for source, wasm in [(SOURCE[:7], WASM), (SOURCE, WASM[:12]), ('X' * 40, WASM)]:
            with self.assertRaises(ValueError):
                public.validate_identity(bodies(), source, wasm)

    def test_page_and_worker_responses_cannot_be_omitted_or_changed(self):
        hashes = public.validate_identity(bodies(), SOURCE, WASM)
        mutations = [
            lambda rows: rows.pop(2),
            lambda rows: rows[2].update(sha256='d' * 64),
            lambda rows: rows[2].update(finished=False),
            lambda rows: rows[2].update(status=404),
            lambda rows: rows.append(dict(rows[2], sha256='d' * 64)),
        ]
        for mutate in mutations:
            rows = loaded()
            mutate(rows)
            with self.assertRaises(ValueError):
                public.validate_loaded(rows, hashes, hashes, WASM)
        after = dict(hashes, **{'/worker.js': 'e' * 64})
        with self.assertRaises(ValueError):
            public.validate_loaded(loaded(), hashes, after, WASM)

    def test_discovered_executables_are_bound_and_other_origins_rejected(self):
        hashes = public.validate_identity(bodies(), SOURCE, WASM)
        rows = loaded() + [dict(path='/extra.css', status=200, finished=True, sha256='d' * 64)]
        after = dict(hashes, **{'/extra.css': 'd' * 64})
        self.assertIn('/extra.css', public.validate_loaded(rows, hashes, after, WASM))
        for url in ['http://www.civvis.ai/worker.js', 'https://evil.invalid/civvis.wasm']:
            self.assertIsNone(public.response_path(dict(response=dict(url=url))))
        self.assertEqual(public.response_path(dict(
            response=dict(url=public.ORIGIN + '/worker.js?build=123'))), '/worker.js')

    def test_screenshot_receipt_uses_monotonic_conservative_wall(self):
        self.assertEqual(public.checked_wall(100, 2_000_000_100), 2000)
        for start, end in [(1, 1), (2, 1), (True, 9), (1, float('nan'))]:
            with self.assertRaises(ValueError):
                public.checked_wall(start, end)


class OriginalTwelveGameGuards(unittest.TestCase):
    def test_all_original_cases_and_raw_repeats_pass_without_playing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'originals'
            root.mkdir()
            fixtures(root)
            result = public.summarize(root, Path(tmp) / 'summary')
            self.assertEqual(len(result['observations']), 12)
            self.assertFalse(result['website_goal_milestones_verified'])

    def test_missing_partial_duplicate_or_mixed_originals_fail(self):
        for change in ['missing', 'partial', 'duplicate', 'changed_frontend', 'changed_clock']:
            with self.subTest(change=change), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp) / 'originals'
                root.mkdir()
                fixtures(root)
                p = sorted(root.rglob('receipt.json'))[0]
                receipt = json.loads(p.read_text())
                if change == 'missing':
                    p.unlink()
                else:
                    if change == 'partial':
                        receipt['phase'] = 'observing'
                    elif change == 'duplicate':
                        receipt['repeat'] = 2
                    elif change == 'changed_frontend':
                        receipt['public_before'] = dict(receipt['public_before'], **{'/worker.js': 'f' * 64})
                    else:
                        receipt['navigation_to_terminal_screenshot_received_ms'] = 19
                    p.write_text(json.dumps(receipt))
                with self.assertRaises(ValueError):
                    public.summarize(root, Path(tmp) / 'summary')

    def test_modified_raw_save_or_divergent_repeated_game_fails(self):
        for update_receipt in (False, True):
            with self.subTest(update_receipt=update_receipt), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp) / 'originals'
                root.mkdir()
                fixtures(root)
                folder = sorted(root.iterdir())[0]
                p = folder / 'game.save.json'
                game = json.loads(p.read_text())
                game['game']['rng']['state'] = 3
                raw = json.dumps(game).encode()
                p.write_bytes(raw)
                if update_receipt:
                    rp = folder / 'receipt.json'
                    receipt = json.loads(rp.read_text())
                    view = json.loads((folder / 'observation.json').read_text())['observation']
                    receipt['raw_save'] = browser.validate_save(raw, view, receipt['case'])
                    rp.write_text(json.dumps(receipt))
                with self.assertRaises(ValueError):
                    public.summarize(root, Path(tmp) / 'summary')

    def test_changed_original_loaded_or_identity_body_fails(self):
        for name in ('loaded-responses/2.body', 'public-before/civvis.wasm',
                     'public-after/assets/app.js'):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp) / 'originals'
                root.mkdir()
                fixtures(root)
                folder = sorted(root.iterdir())[0]
                (folder / name).write_bytes(b'changed original bytes')
                with self.assertRaises(ValueError):
                    public.summarize(root, Path(tmp) / 'summary')

    def test_altered_normal_setting_fails_even_with_matching_raw_save(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'originals'
            root.mkdir()
            fixtures(root)
            folder = sorted(root.iterdir())[0]
            p = folder / 'observation.json'
            data = json.loads(p.read_text())
            data['observation']['settings']['victoryConditions']['science'] = False
            p.write_text(json.dumps(data))
            with self.assertRaises(ValueError):
                public.summarize(root, Path(tmp) / 'summary')


@unittest.skipUnless(os.environ.get('CIVVIS_PUBLIC_TRANSPORT_SMOKE_CHROME'),
                     'explicit fixed Chrome transport fixture; no games')
class WorkerNetworkIntegration(unittest.TestCase):
    def test_real_worker_response_body_is_observable_before_game_use(self):
        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                responses = {
                    '/': ('text/html', b"<script>window.fixtureDone=false;const w=new Worker('/worker.js');w.onmessage=()=>window.fixtureDone=true;</script>"),
                    '/worker.js': ('text/javascript', b"fetch('/fixture.wasm').then(r=>r.arrayBuffer()).then(()=>postMessage('ready'));"),
                    '/fixture.wasm': ('application/wasm', MODULE),
                }
                mime, raw = responses.get(self.path, ('text/plain', b'missing'))
                self.send_response(200 if self.path in responses else 404)
                self.send_header('Content-Type', mime)
                self.send_header('Content-Length', str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)

            def log_message(self, *_):
                pass

        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        chrome = os.environ['CIVVIS_PUBLIC_TRANSPORT_SMOKE_CHROME']
        port = public.verify.free_port()
        dev, process = None, None
        with tempfile.TemporaryDirectory() as tmp:
            try:
                process = subprocess.Popen([chrome, '--headless=new',
                    '--remote-debugging-port=' + str(port), '--user-data-dir=' + tmp,
                    '--no-first-run', '--no-default-browser-check', 'about:blank'],
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                deadline = time.monotonic() + 20
                target = None
                while time.monotonic() < deadline:
                    try:
                        rows = json.loads(urllib.request.urlopen(
                            'http://127.0.0.1:' + str(port) + '/json/list', timeout=1).read())
                        target = next((r for r in rows if r.get('type') == 'page'), None)
                        if target:
                            break
                    except OSError:
                        pass
                    time.sleep(.1)
                self.assertIsNotNone(target)
                dev = public.Recorder(target['webSocketDebuggerUrl'])
                dev.call('Page.enable')
                dev.call('Runtime.enable')
                dev.call('Network.enable', public.network_options())
                dev.call('Target.setAutoAttach', dict(
                    autoAttach=True, waitForDebuggerOnStart=True, flatten=True))
                dev.call('Page.navigate', dict(
                    url='http://127.0.0.1:' + str(server.server_port) + '/'))
                deadline = time.monotonic() + 20
                while time.monotonic() < deadline:
                    if dev.evaluate('window.fixtureDone === true'):
                        break
                    time.sleep(.1)
                else:
                    self.fail('worker was not resumed or its original fetch failed')
                deadline = time.monotonic() + 5
                rows = []
                while time.monotonic() < deadline:
                    rows = [r for r in list(dev.responses.values())
                            if r['response']['url'].endswith('/fixture.wasm') and r['finished']]
                    if rows:
                        break
                    time.sleep(.1)
                self.assertEqual(len(rows), 1)
                self.assertIsNotNone(rows[0]['session'], 'must observe the real worker target')
                body = dev.call('Network.getResponseBody',
                                {'requestId': rows[0]['request_id']}, session=rows[0]['session'])
                actual = (base64.b64decode(body['body']) if body.get('base64Encoded')
                          else body['body'].encode())
                self.assertEqual(actual, MODULE)
                self.assertFalse(dev.console)
            finally:
                if dev:
                    dev.close()
                if process:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
                server.shutdown()
                server.server_close()


if __name__ == '__main__':
    unittest.main()
