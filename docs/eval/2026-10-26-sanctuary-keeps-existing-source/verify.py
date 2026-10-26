"""Verify both regression stages and complete final historical payloads/reads."""
from pathlib import Path
import gzip, hashlib, json, sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[2] / 'tools'))
from civ6_decision_trace import exact_fact
from native_city_route_replay import control_payload

def packed(name):
    return json.loads(gzip.decompress((HERE / name).read_bytes()))

registration = json.loads((HERE / 'registration.json').read_text())
for name, expected in registration['artifact_sha256'].items():
    assert hashlib.sha256((HERE / name).read_bytes()).hexdigest() == expected, name

initial = packed('regressions.json.gz')
fallback = packed('fallback-preservation.json.gz')
for archive in [initial, fallback]:
    for name, raw in archive['files'].items():
        assert hashlib.sha256(raw.encode()).hexdigest() == archive['sha256'][name], name
red = json.loads(initial['files']['red-qualified.json'])
green = json.loads(initial['files']['green-v1-process.json'])
assert red['qualified_red'] and red['passed'] == 26 and red['intended_failures'] == 2
assert green['exit_code'] == 0 and green['tests_identical_to_red']
assert red['tests_required_identical_for_green'] == green['test_sha256']
assert '28 passed; 0 failed' in initial['files']['green-v1-sanctuary.log']
assert '6 passed; 0 failed' in initial['files']['green-v1-founder.log']
assert '7 passed; 0 failed' in initial['files']['green-v1-shrine.log']
assert hashlib.sha256(initial['identical_red_green_tests'].encode()).hexdigest() == green['test_sha256']

red2 = json.loads(fallback['files']['fallback-red-qualified.json'])
green2 = json.loads(fallback['files']['green-v2-process.json'])
assert red2['qualified_red'] and red2['passed'] == 28 and red2['intended_failures'] == 1
assert green2['exit_code'] == 0 and green2['tests_identical_to_fallback_red']
assert green2['test_sha256'] == red2['tests_required_identical_for_green']
assert len(green2['steps']) == 4 and all(s['exit_code'] == 0 for s in green2['steps'])
for family, count in [('sanctuary', 29), ('first-convert', 4), ('founder', 6), ('shrine', 7)]:
    assert f'{count} passed; 0 failed' in fallback['files'][f'green-v2-{family}.log']
assert hashlib.sha256(fallback['identical_fallback_red_green_tests'].encode()).hexdigest() == green2['test_sha256']
assert ''.join(fallback['identical_fallback_red_green_tests'].split()) == ''.join(fallback['formatted_final_tests'].split())
assert hashlib.sha256(fallback['formatted_final_tests'].encode()).hexdigest() == registration['final_tests_sha256']

original_path = HERE.parent / '2026-10-26-native-faith-investment/original-control.json.gz'
assert hashlib.sha256(original_path.read_bytes()).hexdigest() == registration['original_control_archive_sha256']
original = json.loads(gzip.decompress(original_path.read_bytes()))
assert original['qualification']['passed'] and len(original['qualification']['controls']) == 2
native = [json.loads(line) for line in original['native_decisions'].splitlines()]
assert len(native) == 264

candidate = packed('qualified-first-change.json.gz')
comparison = candidate['comparison']
assert candidate['process']['exit_code'] == 0 and comparison['qualified']
assert comparison['first_changed_request'] == [60, 0]
assert comparison['identical_complete_requests_before_change'] == 153
assert candidate['scope']['source_blobs_verified'] == 830
assert len(candidate['scope']['modified']) == 3
assert hashlib.sha256(candidate['exact_module'].encode()).hexdigest() == registration['source_sha256']
assert candidate['exact_module'] == fallback['v2_source']
changed, traces = [], []
for number, result in enumerate(comparison['comparisons']):
    checks = result['requests']
    assert not result['passed'] and result['failed_request'] == [60, 0]
    assert len(checks) == 154 and checks[-1]['request'] == [60, 0]
    for index, row in enumerate(checks):
        assert row['read_sequence_matches'] and not row['identity_errors']
        assert row['expected_read_sha256'] == row['actual_read_sha256']
        assert row['recorded_reads'] == row['replayed_reads'] == sum(r['count'] for r in native[index]['input_capture']['reads'])
    assert all(row['native_payload_matches'] for row in checks[:-1])
    assert not checks[-1]['native_payload_matches']
    files = candidate['comparisons'][str(number)]
    replies = [json.loads(line) for line in files['replies.jsonl'].splitlines()]
    assert len(replies) == 154
    assert all(exact_fact(control_payload(a), control_payload(b)) for a, b in zip(native[:153], replies[:153]))
    reply = replies[-1]
    assert (reply['decision']['turn'], reply['decision']['frame']) == (60, 0)
    assert not any('HOLY_SITE' in json.dumps(order) for order in reply['orders'])
    assert not any('holy_site' in json.dumps(action) for action in reply['decision']['native_actions'])
    city_orders = lambda payload: [o for o in payload['orders'] if o.get('kind') in ('produce', 'purchase') and o.get('subject') == 131073]
    assert exact_fact(city_orders(native[153]), city_orders(reply))
    changed.append(control_payload(reply))
    traces.append([line for line in files['stderr.log'].splitlines() if line.startswith('[income-probe] ')])
assert exact_fact(changed[0], changed[1]) and traces[0] and traces[0] == traces[1]

validation = json.loads((HERE / 'validation.json').read_text())
assert validation['process']['exit_code'] == 0
assert all(step['exit_code'] == 0 for step in validation['process']['steps'])
assert validation['process']['source_sha256'] == registration['source_sha256']
assert validation['process']['tests_sha256'] == registration['final_tests_sha256']
assert '29 passed; 0 failed' in validation['logs']['tests']
print('Both genuine RED stages, final46 family tests, final29 exact-byte tests/quality/check verified.')
print('Both153-prior/60-withdrawal complete payload/read comparisons verified; no Holy Site hint/order remains.')
