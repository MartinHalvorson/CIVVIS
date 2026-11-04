"""Verify retained bytes and comparison evidence without accessing a native game."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--baseline-only', action='store_true')
args = parser.parse_args()
here = Path(__file__).resolve().parent
read = lambda name: json.loads(gzip.decompress((here / name).read_bytes()))
packet = read('qualified-opening.json.gz')
captures = packet['portable_captures']
prefixes = captures['capture_prefix_manifest']
archive = (here / 'native-events.jsonl.gz').read_bytes()
assert hashlib.sha256(archive).hexdigest() == prefixes['source_events_archive_sha256']
raw = gzip.decompress(archive)
assert len(raw) == prefixes['source_events_bytes']
assert hashlib.sha256(raw).hexdigest() == prefixes['source_events_raw_sha256']
assert hashlib.sha256(raw[:captures['archive_prefix_bytes']]).hexdigest() == captures['archive_prefix_sha256']
snapshots = {row['snapshot_id']: row for row in prefixes['unique_snapshots']}
assert len(snapshots) == len(captures['snapshot_index']) == 278
lengths = {}
for row in captures['snapshot_index']:
    parent = 0 if row['parent'] is None else lengths[row['parent']]
    end = row['offset'] + row['append_bytes']
    assert parent + row['append_bytes'] == row['total_bytes']
    assert raw[row['offset']:end] == raw[parent:row['total_bytes']]
    lengths[row['id']] = row['total_bytes']
    expected = snapshots[row['id']]
    assert expected['exact_archived_native_prefix'] and expected['bytes'] == row['total_bytes']
    assert hashlib.sha256(raw[:row['total_bytes']]).hexdigest() == expected['sha256']
records = packet['native_decisions']
assert len(records) == 266
keys = [(r['decision']['turn'], r['decision']['frame']) for r in records]
assert keys == sorted(set(keys))
expected_reads = []
for record in records:
    reads = []
    for read_group in record['input_capture']['reads']:
        reads.extend([snapshots[read_group['snapshot_id']]['sha256']] * read_group['count'])
    expected_reads.append(reads)
assert sum(map(len, expected_reads)) == 4621
request19 = records[keys.index((19, 0))]
snapshot19 = snapshots[request19['input_capture']['reads'][0]['snapshot_id']]
captured_events = [json.loads(line) for line in raw[:snapshot19['bytes']].splitlines()]
state19 = next(e for e in reversed(captured_events) if e['kind'] == 'state')
capital19 = next(c for c in state19['cities'] if c['id'] == 65536)
assert capital19['producing'] == 'UNIT_SETTLER' and capital19['production_progress'] == 23
scout_quote = next(q for q in capital19['buildable'] if q['t'] == 'UNIT_SCOUT')
assert scout_quote['c'] == 15 and scout_quote['p'] == 3
for name in ('original_controls', 'diagnostic_controls'):
    qualification = packet[name]
    assert qualification['passed'] and qualification['through_turn'] == 100
    assert len(qualification['controls']) == 2
    for control in qualification['controls']:
        assert control['passed'] and len(control['requests']) == len(records)
        for index, request in enumerate(control['requests']):
            assert tuple(request['request']) == keys[index]
            assert not request['identity_errors']
            assert request['native_payload_matches'] and request['read_sequence_matches']
            assert request['expected_read_sha256'] == request['actual_read_sha256'] == expected_reads[index]
trace = packet['qualified_trace_lines']
assert len(trace) == 397
assert hashlib.sha256(('\n'.join(trace) + '\n').encode()).hexdigest() == packet['corrected_trace_qualification']['trace_sha256']
events = [json.loads(line[len('[income-probe] '):]) for line in trace]
assert [(e['turn'], e['frame']) for e in events if e['event'] == 'root_begin'] == keys
assert sum(e['event'] == 'produce' for e in events) == 131
assert len(packet['source_manifest']['files']) == 830
assert set(packet['instrumentation_manifest']['files']) == {'src/game/actions.rs', 'src/bin/civvis_orders.rs'}
cases = packet['capital_callers']['cases']
assert next(c for c in cases if (c['turn'], c['frame']) == (19, 0))['caller_function'] == 'reserve_idle_land_recon'
assert all(c['all_root_produces'][-1]['queue_before'] == [] for c in cases if (c['turn'], c['frame']) in ((18, 0), (19, 0), (20, 0)))
assert packet['corrected_trace_qualification']['passed']
assert packet['diagnostic_process_with_retained_postprocess_failure']['exit_code'] == 1
if args.baseline_only:
    print('Verified portable Native bytes,2x266 original controls,2x266 diagnostic controls and397 trace events.')
    raise SystemExit(0)
validation = read('validation.json.gz')
assert validation['red']['qualified'] and validation['red']['exit_code'] == 101
assert 'test result: FAILED. 2 passed; 2 failed;' in validation['red_log']
assert validation['green']['exit_code'] == 0 and all(s['exit_code'] == 0 for s in validation['green']['steps'])
assert 'test result: ok. 4 passed;' in validation['green_logs']['focused']
assert 'test result: ok. 315 passed;' in validation['green_logs']['all-bin']
assert validation['source_quality']['quality_exit_code'] == 0
assert validation['same_four_red_green_test_bytes']
assert validation['post_merge_python_tests']['exit_code'] == 0 and validation['post_merge_python_tests']['tests'] == 16
comparison = read('first-change.json.gz')
assert comparison['qualified'] and len(comparison['comparisons']) == 2
assert comparison['source_process']['exit_code'] == 0
assert hashlib.sha256(comparison['first_change_script'].encode()).hexdigest() == comparison['registration']['first_change_script_sha256']
assert comparison['observed_candidate_traces'][0] == comparison['observed_candidate_traces'][1] == comparison['complete_identical_root_trace']
assert comparison['identical_complete_requests_before_change'] >= 43
assert comparison['registration']['first_positive_work_release_request'] == [18, 0]
for control in comparison['comparisons']:
    requests = control['requests']
    assert all(not r['identity_errors'] and r['read_sequence_matches'] for r in requests)
    if comparison['unchanged_all_observed']:
        assert control['passed'] and len(requests) == 266 and all(r['native_payload_matches'] for r in requests)
    else:
        assert not control['passed'] and all(r['native_payload_matches'] for r in requests[:-1])
        assert not requests[-1]['native_payload_matches']
        assert requests[-1]['request'] == comparison['first_changed_request']
if comparison['unchanged_all_observed']:
    for reply in comparison['observed_candidate_last_replies']:
        assert reply['orders'] == records[-1]['orders'] and reply['decision'] == records[-1]['decision']
assert all('(0.0 turns)' in line for line in comparison['journal_price_lines']['original'])
assert all('(2.5 turns)' in line for line in comparison['journal_price_lines']['corrected'])
assert comparison['journal_price_lines']['original'] and comparison['journal_price_lines']['corrected']
print('Verified original/diagnostic bytes, genuine assertion RED,4 GREEN tests,315 bin tests and historical first-change comparison.')
