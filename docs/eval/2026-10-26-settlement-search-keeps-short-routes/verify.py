"""Check complete replay evidence and retained rejections for the settlement search."""
import argparse,gzip,hashlib,json,sys
from pathlib import Path
P=argparse.ArgumentParser();P.add_argument('--baseline-only',action='store_true');P.add_argument('--shared-input-dir',type=Path);args=P.parse_args()
ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(Path(__file__).resolve().parents[3]/'tools'))
from civ6_decision_trace import exact_fact
load=lambda p:json.loads(gzip.decompress(p.read_bytes()))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
d=load(ROOT/'diagnostic.json.gz');m=d['source_manifest'];i=d['instrumentation']
assert len(m['files'])==830 and len(m['discovered_literal_includes'])==20
assert set(i['files'])=={'src/game/actions.rs','src/bin/civvis_orders.rs','src/ai.rs'}
assert set(d['root_instrumentation']['files'])=={'src/game/actions.rs','src/bin/civvis_orders.rs'}
for name,entry in i['files'].items():assert entry['before_sha256']==m['files'][name]['sha256']
q=d['qualified_controls'];assert q['passed'] and q['through_turn']==100 and len(q['controls'])==2
for c in q['controls']:
 assert c['passed'] and len(c['requests'])==266
 assert sum(x['recorded_reads'] for x in c['requests'])==4621
 for x in c['requests']:
  assert not x['identity_errors'] and x['native_payload_matches'] and x['read_sequence_matches']
  assert x['expected_read_sha256']==x['actual_read_sha256']
  assert x['recorded_reads']==x['replayed_reads']==len(x['expected_read_sha256'])
rejected=d['rejected_qualification'];assert not rejected['passed']
assert rejected['controls'][0]['passed'] and len(rejected['controls'][0]['requests'])==266
failed=rejected['controls'][1];assert len(failed['requests'])==134
assert failed['failed_request']==[53,0] and failed['error']=='TimeoutError: decider reply deadline expired'
assert failed['input_delivery']['served']==5 and failed['input_delivery']['expected']==18
assert d['build_and_rejected_attempt']['exit_code']==1 and d['retry_process']['exit_code']==0
assert d['retry_process']['binary_sha256']==d['build_and_rejected_attempt']['binary_sha256']==q['diagnostic_binary_sha256']
f=d['findings'];assert f['qualified'] and len(f['complete_identical_root_trace'])==1403
assert len(f['searches'])==970 and len(f['disagreements'])==537
assert len({(x['turn'],x['frame']) for x in f['disagreements']})==92
first=f['disagreements'][0];assert (first['turn'],first['frame'])==(17,3)
assert first['original_result'] is False and first['breadth_first_site_and_steps']==[[6,16],4]
queue=[x for x in f['queue_revalidation'] if (x['turn'],x['frame'])==(17,3)]
assert len(queue)==1 and not queue[0]['practical_settle_site']
assert queue[0]['width']==2 and queue[0]['active_settlers']==0 and queue[0]['queue']==[{'unit':'settler'}]
produces=[x for x in f['complete_identical_root_trace'] if x['event']=='produce' and (x['turn'],x['frame'])==(17,3) and x['city']==33]
assert len(produces)==1 and produces[0]['item']=={'unit':'builder'} and produces[0]['queue_before']==[{'unit':'settler'}]
assert [(x['turn'],x['frame']) for x in f['complete_identical_root_trace'] if x['event']=='root_begin']==[tuple(x['request']) for x in q['controls'][0]['requests']]
shared=args.shared_input_dir or ROOT/d['shared_inputs']['relative_directory']
for name,entry in d['shared_inputs']['files'].items():assert sha(shared/name)==entry['sha256'] and (shared/name).stat().st_size==entry['bytes'],name
native=load(shared/'qualified-opening.json.gz')['native_decisions']
assert len(native)==266 and len(d['both_observed_diagnostic_replies'])==2
payload=lambda row:{k:row[k] for k in ('orders','decision') if k in row}
for observed in d['both_observed_diagnostic_replies']:
 assert len(observed)==len(native)
 for expected,actual in zip(native,observed):
  assert exact_fact(payload(expected),payload(actual))
  assert actual['turn']==expected['decision']['turn']
assert exact_fact(d['both_observed_root_traces'][0],d['both_observed_root_traces'][1])
assert exact_fact(d['both_observed_root_traces'][0],f['complete_identical_root_trace'])
line='replay.run_control(diagnostic, records, mapping, args.out / f"control-{number}", 120)'
assert d['original_diagnostic_replay_script'].count(line)==1
assert d['original_diagnostic_replay_script'].replace(line,line.replace('120','600'),1)==d['retry_diagnostic_replay_script']
assert hashlib.sha256(d['original_diagnostic_replay_script'].encode()).hexdigest()==d['retry_registration']['original_script_sha256']
assert hashlib.sha256(d['retry_diagnostic_replay_script'].encode()).hexdigest()==d['retry_registration']['retry_script_sha256']
if not args.baseline_only:
 v=load(ROOT/'validation.json.gz');c=load(ROOT/'first-change.json.gz')
 assert v['red']['qualified_assertion_red'] and v['red']['cargo_exit_code']==101
 assert v['green']['exit_code']==0 and v['quality']['exit_code']==0
 assert v['red_registration']['test_sha256']==v['green']['test_sha256']
 assert c['comparison']['qualified'] and len(c['comparison']['comparisons'])==2
 for run in c['comparison']['comparisons']:
  assert all(not x['identity_errors'] and x['read_sequence_matches'] and x['expected_read_sha256']==x['actual_read_sha256'] for x in run['requests'])
  assert all(x['native_payload_matches'] for x in run['requests'][:-1])
 print('Original regression RED, same-test GREEN, source quality and historical first-change evidence verified.')
else:print('Exact original/diagnostic controls, captured-read hashes, retained timeout and actual first false-negative cause verified. Final source validation/comparison still required.')
