"""Locate actual root sanctuary admissions after two full probe controls pass."""
from pathlib import Path
import argparse, json

parser = argparse.ArgumentParser()
parser.add_argument('--root', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
root = args.root
process = json.loads((root / 'sanctuary-query-process.json').read_text())
assert process['exit_code'] == 0 and process['full_traces_identical']
qualification = json.loads((root / 'sanctuary-query-control-t100/qualification.json').read_text())
assert qualification['passed'] and len(qualification['controls']) == 2
for control in qualification['controls']:
    assert control['passed'] and len(control['requests']) == 264
    assert sum(r['recorded_reads'] for r in control['requests']) == 4587
    assert all(r['native_payload_matches'] and r['read_sequence_matches']
               and not r['identity_errors'] for r in control['requests'])
traces = [[line for line in (root / f'sanctuary-query-control-t100/control-{n}/stderr.log').read_text().splitlines()
           if line.startswith('[income-probe] ')] for n in range(2)]
assert traces[0] and traces[0] == traces[1]
events = [json.loads(line[len('[income-probe] '):]) for line in traces[0]]
previous = json.loads((root / 'faith-caller-analysis.json').read_text())
cases = []
for case in previous['cases']:
    if case['caller_function'] != 'reserve_adopted_faith_sanctuary':
        continue
    wanted = case['successful_root_produces'][-1]
    indices = [i for i, event in enumerate(events) if event['event'] == 'produce' and event['ok']
               and (event['turn'], event['frame'], event['city'], event['item']) ==
               (case['turn'], case['frame'], case['model_city'], wanted['item'])]
    assert indices
    prior = [event for event in events[:indices[-1]] if event['event'] == 'sanctuary_admission'
             and (event['turn'], event['frame']) == (case['turn'], case['frame'])]
    admission = prior[-1] if prior else None
    cases.append({'turn': case['turn'], 'frame': case['frame'], 'native_order': case['native_order'],
        'model_city': case['model_city'], 'admission': admission,
        'safe_completed_sources': [] if admission is None else [city for city in admission['owned']
            if city['safe_for_this_threat'] and city['has_shrine']],
        'status': 'exact preceding root admission' if prior else 'early-return path: no admission event, retain unresolved'})
report = {'scope': 'Qualified exact root admission state. A native quote still must corroborate purchase eligibility; no intervention or strength result.',
          'process': process, 'events': len(events), 'cases': cases}
assert not args.out.exists()
args.out.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'events': len(events), 'cases': len(cases),
    'held_unthreatened_safe_sources': [[c['turn'], c['frame']] for c in cases if c['admission'] is not None
        and any(s['city'] != c['admission']['threatened_city'] and s['queue_held']
                for s in c['safe_completed_sources'])]}))
