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
context = json.loads(gzip.decompress((HERE / 'native-faith-context.json.gz').read_bytes()))
assert len(context['cases']) == 9
assert sum(len(c['observations']) for c in context['cases']) == 10
assert all(o['state'] is not None for c in context['cases'] for o in c['observations'])
case = next(c for c in context['cases'] if (c['turn'], c['frame']) == (60, 0))
assert len(case['observations']) == 2
for observation in case['observations']:
    state = observation['state']
    assert state['faith'] == 202 and state['gold'] == 157
    city = next(c for c in state['cities'] if c['id'] == 131073)
    assert city['producing'] == 'DISTRICT_CAMPUS' and 'BUILDING_SHRINE' in city['buildings']
    assert {'f': 70, 't': 'UNIT_MISSIONARY'} in city['purchasable']
print('All10 distinct captured native contexts retained; counterfaith safety inference remains separate.')
admission = json.loads(gzip.decompress((HERE / 'qualified-admission.json.gz').read_bytes()))
assert admission['analysis']['process']['exit_code'] == 0
query_traces = []
for number, control in enumerate(admission['qualification']['controls']):
    assert control['passed'] and len(control['requests']) == 264
    assert sum(q['recorded_reads'] for q in control['requests']) == 4587
    assert all(q['native_payload_matches'] and q['read_sequence_matches']
               and not q['identity_errors'] for q in control['requests'])
    files = admission['controls'][str(number)]
    replies = [json.loads(line) for line in files['replies.jsonl'].splitlines()]
    assert len(replies) == 264
    assert all(exact_fact(control_payload(a), control_payload(b)) for a, b in zip(native, replies))
    query_traces.append([line for line in files['stderr.log'].splitlines() if line.startswith('[income-probe] ')])
assert len(query_traces[0]) == 554 and query_traces[0] == query_traces[1]
events = [json.loads(line[len('[income-probe] '):]) for line in query_traces[0]]
held = [e for e in events if e['event'] == 'sanctuary_admission' and any(
    c['safe_for_this_threat'] and c['has_shrine'] and c['queue_held']
    and c['city'] != e['threatened_city'] for c in e['owned'])]
assert (held[0]['turn'], held[0]['frame']) == (60, 0)
cases = admission['analysis']['cases']
assert len(cases) == 7
confirmed = [c for c in cases if c['admission'] is not None and any(
    s['queue_held'] and s['city'] != c['admission']['threatened_city'] for s in c['safe_completed_sources'])]
assert [(c['turn'], c['frame']) for c in confirmed] == [(60, 0), (73, 0), (81, 0)]
for case in confirmed:
    e = case['admission']
    source = case['safe_completed_sources'][0]
    assert source['city'] not in e['eligible'] and not source['free_for_production']
    assert not e['one_sanctuary'] and source['missionary_faith_quote'] == 70
    native_case = next(c for c in context['cases'] if (c['turn'], c['frame']) == (case['turn'], case['frame']))
    for observation in native_case['observations']:
        state = observation['state']
        city = next(c for c in state['cities'] if c['id'] == 131073)
        assert {'f': 70, 't': 'UNIT_MISSIONARY'} in city['purchasable'] and state['faith'] >= 70
late = next(c for c in cases if c['turn'] == 96)
assert late['safe_completed_sources'][0]['city'] == late['admission']['threatened_city']
assert next(c for c in cases if c['turn'] == 83)['admission'] is None
print('Both264 admission controls and full554 traces verify held-source failures60/73/81; threatened96 and unresolved83 retained.')
