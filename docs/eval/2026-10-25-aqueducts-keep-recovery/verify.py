"""Verify complete archived payloads and the registered first changed order."""
from pathlib import Path
import gzip
import hashlib
import json
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[2] / 'tools'))
from civ6_decision_trace import exact_fact
from native_city_route_replay import control_payload


def packed(name):
    return json.loads(gzip.decompress((HERE / name).read_bytes()))


registration = json.loads((HERE / 'registration.json').read_text())
for name, expected in registration['artifact_sha256'].items():
    assert hashlib.sha256((HERE / name).read_bytes()).hexdigest() == expected, name

history = packed('regression-history.json.gz')
for name, raw in history['files'].items():
    assert hashlib.sha256(raw.encode()).hexdigest() == history['sha256'][name]
red = json.loads(history['files']['red-provenance-qualified.json'])
green = json.loads(history['files']['green-provenance-process.json'])
assert red['qualified_red'] and red['passed'] == 13 and red['intended_failures'] == 1
assert green['exit_code'] == 0 and green['expected_tests'] == 14
assert green['tests_identical_to_red']
assert green['test_sha256'] == red['tests_required_identical']
assert '14 passed; 0 failed' in history['files']['green-provenance.log']
assert hashlib.sha256(history['final_tests'].encode()).hexdigest() == green['test_sha256']
assert hashlib.sha256(history['final_module'].encode()).hexdigest() == green['source_sha256']

helper = packed('qualified-helper-control.json.gz')
native = [json.loads(line) for line in helper['original_native_decisions'].splitlines()]
assert len(native) == 263 and helper['qualification']['passed']
for key, files in helper['controls'].items():
    replies = [json.loads(line) for line in files['replies.jsonl'].splitlines()]
    assert len(replies) == 263
    for original, reply in zip(native, replies):
        assert exact_fact(control_payload(original), control_payload(reply)), key
for control in helper['qualification']['controls']:
    assert control['passed'] and len(control['requests']) == 263
    assert all(q['native_payload_matches'] and q['read_sequence_matches']
               and not q['identity_errors'] for q in control['requests'])
traces = [[line for line in helper['controls'][str(n)]['stderr.log'].splitlines()
           if line.startswith('[income-probe] ')] for n in range(2)]
assert traces[0] and traces[0] == traces[1]
first = helper['analysis']['first_depleted_available_recovery']
assert (first['turn'], first['frame'], first['city']) == (71, 0, 34)
assert first['recovery'] == {'district': 'commercial_hub', 'pos': [13, 11]}
for observation in helper['native_quotes']['observations']:
    assert observation['gold'] == 105 and observation['income'] == -0.746094
    quote = next(q for q in observation['quotes'] if q['t'] == 'DISTRICT_COMMERCIAL_HUB')
    assert quote['p'] == 11 and {'x': 18, 'y': 11} in quote['s']

failed = packed('rejected-first-change.json.gz')
assert not failed['comparison']['qualified']
failed_reply = json.loads(failed['comparisons']['comparison-0']['replies.jsonl'].splitlines()[-1])
assert any(o.get('kind') == 'produce' and o.get('subject') == 131073
           and o.get('verb') == 'BUILDING_LIBRARY' for o in failed_reply['orders'])

qualified = packed('qualified-first-change.json.gz')
report = qualified['comparison']
assert report['qualified'] and report['first_changed_request'] == [71, 0]
assert report['identical_complete_requests_before_change'] == 191
assert len(report['comparisons']) == 2
changed = []
for key, files in qualified['comparisons'].items():
    replies = [json.loads(line) for line in files['replies.jsonl'].splitlines()]
    assert len(replies) == 192
    for original, reply in zip(native[:191], replies[:191]):
        assert exact_fact(control_payload(original), control_payload(reply)), key
    reply = replies[-1]
    assert (reply['decision']['turn'], reply['decision']['frame']) == (71, 0)
    actions = [a for a in reply['decision']['native_actions']
               if a.get('type') == 'produce' and a.get('city') == 34]
    assert actions[-1]['item'] == {'district': 'commercial_hub', 'pos': [13, 11]}
    orders = [o for o in reply['orders'] if o.get('kind') == 'produce'
              and o.get('subject') == 131073]
    assert orders == [qualified['registration']['expected_native_order']]
    changed.append(control_payload(reply))
assert exact_fact(changed[0], changed[1])
for comparison in report['comparisons']:
    rows = comparison['requests']
    assert len(rows) == 192 and rows[-1]['request'] == [71, 0]
    assert all(row['read_sequence_matches'] and not row['identity_errors'] for row in rows)
    assert all(row['native_payload_matches'] for row in rows[:-1])
    assert not rows[-1]['native_payload_matches']

context = packed('industrial-first-eight-context.json.gz')['analysis']
assert context['wins'] == 0 and context['losses'] == 8
assert context['missing_or_unclassified'] == 0
assert context['all_recorded_hashes_match_frozen_binary']
assert sum(context['source_revisions'].values()) == 8
print('Verified both complete263 controls, both191-prefix/71-Hub comparisons,')
print('all ordered-read receipts, rejected Library case,14-case GREEN and fixed8 native outcomes.')
