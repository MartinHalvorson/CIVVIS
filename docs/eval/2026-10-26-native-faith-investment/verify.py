"""Verify the complete original native payload controls in this draft packet."""
from pathlib import Path
import gzip, hashlib, json, sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[2] / 'tools'))
from civ6_decision_trace import exact_fact
from native_city_route_replay import control_payload

registration = json.loads((HERE / 'registration.json').read_text())
for name, expected in registration['artifact_sha256'].items():
    assert hashlib.sha256((HERE / name).read_bytes()).hexdigest() == expected
packet = json.loads(gzip.decompress((HERE / 'original-control.json.gz').read_bytes()))
qualification = packet['qualification']
assert qualification['passed'] and len(qualification['controls']) == 2
native = [json.loads(line) for line in packet['native_decisions'].splitlines()]
assert len(native) == 264
for number, control in enumerate(qualification['controls']):
    assert control['passed'] and len(control['requests']) == 264
    assert sum(r['recorded_reads'] for r in control['requests']) == 4587
    assert all(r['native_payload_matches'] and r['read_sequence_matches']
               and not r['identity_errors'] for r in control['requests'])
    replies = [json.loads(line) for line in packet['controls'][str(number)]['replies.jsonl'].splitlines()]
    assert len(replies) == len(native)
    assert all(exact_fact(control_payload(a), control_payload(b)) for a, b in zip(native, replies))
scope = json.loads((HERE / 'diagnostic-scope.json').read_text())
assert scope['all_other_blobs_exact'] and scope['all_source_blobs'] == 830
assert set(scope['only_changed_files']) == {'src/game/actions.rs', 'src/bin/civvis_orders.rs'}
assert scope['trace_turns'] == [1, 100]
print('Both264 complete native payload controls and4587 ordered-read receipts verified.')
print('Exact830-input assembly and two-file diagnostic scope retained; caller results pending.')
