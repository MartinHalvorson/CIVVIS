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
diagnostic = json.loads(gzip.decompress((HERE / 'qualified-callers.json.gz').read_bytes()))
assert diagnostic['process']['exit_code'] == 0 and diagnostic['process']['full_traces_identical']
traces = []
for number, control in enumerate(diagnostic['qualification']['controls']):
    assert control['passed'] and len(control['requests']) == 264
    assert sum(q['recorded_reads'] for q in control['requests']) == 4587
    assert all(q['native_payload_matches'] and q['read_sequence_matches']
               and not q['identity_errors'] for q in control['requests'])
    files = diagnostic['controls'][str(number)]
    replies = [json.loads(line) for line in files['replies.jsonl'].splitlines()]
    assert len(replies) == 264
    assert all(exact_fact(control_payload(a), control_payload(b)) for a, b in zip(native, replies))
    traces.append([line for line in files['stderr.log'].splitlines() if line.startswith('[income-probe] ')])
assert len(traces[0]) == 417 and traces[0] == traces[1]
analysis = diagnostic['caller_analysis']
assert analysis['requests'] == analysis['root_frames'] == 264 and analysis['root_produce_calls'] == 153
assert analysis['caller_counts'] == {'reserve_prophet_site': 1, 'reserve_race_shrine': 1,
                                     'reserve_adopted_faith_sanctuary': 7}
for case in analysis['cases']:
    assert case['successful_root_produces'][-1]['item'] == case['final_model_actions'][-1]['item']
print('Both diagnostic264 controls, complete417-event traces and all9 faith callers verified.')
