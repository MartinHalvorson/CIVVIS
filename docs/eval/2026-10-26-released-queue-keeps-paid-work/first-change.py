"""Compare an exact progress-accounting backport until its first changed reply."""
import argparse
import copy
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path

parser = argparse.ArgumentParser()
for flag in ('harness', 'context', 'qualification', 'decisions', 'binary', 'registration', 'out'):
    parser.add_argument('--' + flag, type=Path, required=True)
args = parser.parse_args()
args.out.mkdir(parents=True, exist_ok=False)
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
spec = importlib.util.spec_from_file_location('exact_read_control', args.harness)
replay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay)
report = {'scope': 'Historical first-change comparison only; no changed native game, future execution or strength estimate.',
          'qualified': False, 'comparisons': []}
try:
    registration = json.loads(args.registration.read_text())
    original = json.loads(args.qualification.read_text())
    assert original['passed'] and original['through_turn'] == 100
    assert len(original['controls']) == 2 and all(c['passed'] for c in original['controls'])
    context = json.loads(args.context.read_text())
    records = replay.select_records(args.decisions, 100, context)
    earliest = tuple(registration['first_positive_work_release_request'])
    minimum = next(i for i, r in enumerate(records) if replay.frame_key(r) == earliest)
    assert minimum == registration['minimum_original_requests_before_possible_change']
    candidate = copy.deepcopy(context)
    candidate['binary_sha256'] = sha(args.binary)
    candidate['processes'][0]['argv'][0] = str(args.binary.resolve())
    candidate['provenance'] = {'kind': 'private_exact_progress_accounting_backport', 'sha256': candidate['binary_sha256']}
    (args.out / 'candidate-context.json').write_text(json.dumps(candidate, indent=2) + '\n')
    report.update(registration=registration, registration_sha256=sha(args.registration),
                  original_qualification_sha256=sha(args.qualification), original_decisions_sha256=sha(args.decisions),
                  candidate_binary_sha256=candidate['binary_sha256'])
    mapping, report['sealed_captures'] = replay.seal_captures(records, args.out)
    outcomes, payloads, traces = [], [], []
    for number in range(2):
        directory = args.out / f'comparison-{number}'
        result = replay.run_control(candidate, records, mapping, directory, 120)
        report['comparisons'].append(result)
        checks = result['requests']
        assert checks and all(not q['identity_errors'] and q['read_sequence_matches'] for q in checks)
        if result['passed']:
            assert len(checks) == len(records) and all(q['native_payload_matches'] for q in checks)
            outcomes.append(None)
            payloads.append(None)
        else:
            assert all(q['native_payload_matches'] for q in checks[:-1])
            assert not checks[-1]['native_payload_matches'], 'failure was not a changed planning payload'
            key = tuple(checks[-1]['request'])
            assert tuple(result['failed_request']) == key
            index = len(checks) - 1
            assert index >= minimum and replay.frame_key(records[index]) == key
            reply = json.loads((directory / 'replies.jsonl').read_text().splitlines()[-1])
            outcomes.append(key)
            payloads.append(replay.control_payload(reply))
        trace = [line for line in (directory / 'stderr.log').read_text().splitlines()
                 if line.startswith('[income-probe] ')]
        assert trace
        begins = [(e['turn'], e['frame']) for line in trace
                  if (e := json.loads(line[len('[income-probe] '):]))['event'] == 'root_begin']
        assert begins == [tuple(q['request']) for q in checks]
        traces.append(trace)
    assert outcomes[0] == outcomes[1]
    assert replay.exact_fact(payloads[0], payloads[1]), 'complete changed replies differ'
    assert traces[0] == traces[1], 'complete root traces differ'
    report.update(qualified=True, first_changed_request=outcomes[0],
                  identical_complete_requests_before_change=(len(records) if outcomes[0] is None
                                                             else len(report['comparisons'][0]['requests']) - 1),
                  unchanged_all_observed=outcomes[0] is None,
                  complete_changed_payload=payloads[0], complete_identical_root_trace=traces[0],
                  native_action_benefit='Unproven; unchanged replies establish no native action benefit.'
                                        if outcomes[0] is None else 'Changed historical request only; native result unproven.')
    if outcomes[0] is not None:
        report['original_changed_payload'] = replay.control_payload(records[len(report['comparisons'][0]['requests']) - 1])
except Exception as error:
    report['error'] = f'{type(error).__name__}: {error}'
with gzip.GzipFile(filename=str(args.out / 'comparison.json.gz'), mode='wb', mtime=0) as stream:
    stream.write((json.dumps(report, sort_keys=True, separators=(',', ':')) + '\n').encode())
summary = {k: report[k] for k in ('qualified', 'first_changed_request', 'identical_complete_requests_before_change',
                                'unchanged_all_observed', 'native_action_benefit', 'error') if k in report}
(args.out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary), flush=True)
raise SystemExit(0 if report['qualified'] else 2)
