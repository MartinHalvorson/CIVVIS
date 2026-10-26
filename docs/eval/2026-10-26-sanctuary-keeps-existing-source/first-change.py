"""Require the registered redundant-sanctuary withdrawal on exact native reads."""
import argparse, copy, gzip, hashlib, importlib.util, json
from pathlib import Path

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

parser = argparse.ArgumentParser()
for flag in ['harness', 'context', 'qualification', 'decisions', 'binary', 'registration', 'out']:
    parser.add_argument('--' + flag, type=Path, required=True)
args = parser.parse_args()
args.out.mkdir(parents=True, exist_ok=False)
spec = importlib.util.spec_from_file_location('constant_read_control', args.harness)
replay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay)
report = {'scope': 'Two first-change comparisons on fixed historical native inputs. No native future execution, completion or strength estimate.',
          'qualified': False, 'comparisons': []}
try:
    registration = json.loads(args.registration.read_text())
    expected = tuple(registration['expected_first_changed_request'])
    original = json.loads(args.qualification.read_text())
    assert original['passed'] and len(original['controls']) == 2 and all(c['passed'] for c in original['controls'])
    context = json.loads(args.context.read_text())
    records = replay.select_records(args.decisions, original['through_turn'], context)
    index = next(n for n, q in enumerate(records) if replay.frame_key(q) == expected)
    assert index == registration['expected_identical_complete_requests_before_change']
    native = records[index]
    assert registration['expected_original_native_order'] in native['orders']
    model_city = registration['expected_model_city']
    assert any(a.get('type') == 'produce' and a.get('city') == model_city
               and a.get('item', {}).get('district') == 'holy_site' for a in native['decision']['native_actions'])
    candidate = copy.deepcopy(context)
    candidate['binary_sha256'] = sha(args.binary)
    candidate['processes'][0]['argv'][0] = str(args.binary.resolve())
    candidate['provenance'] = {'kind': 'private_exact_sanctuary_source_backport', 'sha256': candidate['binary_sha256']}
    (args.out / 'candidate-context.json').write_text(json.dumps(candidate, indent=2) + '\n')
    report.update(registration=registration, registration_sha256=sha(args.registration),
                  original_qualification_sha256=sha(args.qualification), original_decisions_sha256=sha(args.decisions),
                  candidate_binary_sha256=candidate['binary_sha256'])
    mapping, report['sealed_captures'] = replay.seal_captures(records, args.out)
    changed = []
    for number in range(2):
        directory = args.out / f'comparison-{number}'
        result = replay.run_control(candidate, records, mapping, directory, 120)
        report['comparisons'].append(result)
        assert not result['passed'] and tuple(result['failed_request']) == expected
        checks = result['requests']
        assert len(checks) == index + 1 and tuple(checks[-1]['request']) == expected
        assert all(not q['identity_errors'] and q['read_sequence_matches'] for q in checks)
        assert all(q['native_payload_matches'] for q in checks[:-1]) and not checks[-1]['native_payload_matches']
        reply = json.loads((directory / 'replies.jsonl').read_text().splitlines()[-1])
        report.setdefault('observed_changed_payloads', []).append(replay.control_payload(reply))
        assert not any(a.get('type') == 'produce' and a.get('item', {}).get('district') == 'holy_site'
                       for a in reply['decision']['native_actions']), 'a redundant Holy Site remains in final model actions'
        assert not any(o.get('kind') == 'produce' and o.get('verb') == 'DISTRICT_HOLY_SITE'
                       for o in reply['orders']), 'a Holy Site remains in final transported orders'
        # Native unit and city IDs overlap. Protect only city production/purchase orders.
        source = registration['existing_source_native_city']
        def source_orders(payload):
            return [o for o in payload['orders'] if o.get('kind') in ('produce', 'purchase') and o.get('subject') == source]
        assert replay.exact_fact(source_orders(native), source_orders(reply)), 'the existing source queue/purchase orders changed'
        changed.append(replay.control_payload(reply))
    assert replay.exact_fact(changed[0], changed[1]), 'the complete changed payloads differ'
    report.update(qualified=True, first_changed_request=expected,
                  identical_complete_requests_before_change=index,
                  original_changed_payload=replay.control_payload(native), candidate_changed_payload=changed[0])
except Exception as error:
    report['error'] = f'{type(error).__name__}: {error}'
(args.out / 'comparison.json.gz').write_bytes(gzip.compress(json.dumps(report, sort_keys=True).encode(), mtime=0))
(args.out / 'summary.json').write_text(json.dumps({k: v for k, v in report.items() if k not in
    ('comparisons', 'sealed_captures', 'original_changed_payload', 'candidate_changed_payload', 'observed_changed_payloads')}, indent=2) + '\n')
print(json.dumps({k: report[k] for k in ['qualified', 'first_changed_request', 'identical_complete_requests_before_change', 'error'] if k in report}), flush=True)
raise SystemExit(0 if report['qualified'] else 2)
