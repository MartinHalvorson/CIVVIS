"""Attribute transported faith builds only after complete diagnostic qualification."""
from pathlib import Path
import argparse, collections, json, re

parser = argparse.ArgumentParser()
parser.add_argument('--root', type=Path, required=True)
args = parser.parse_args()
root = args.root
process = json.loads((root / 'diagnostic-pipeline-process.json').read_text())
assert process['exit_code'] == 0 and process['full_traces_identical']
qualification = json.loads((root / 'diagnostic-control-t100/qualification.json').read_text())
assert qualification['passed'] and len(qualification['controls']) == 2
for control in qualification['controls']:
    assert control['passed'] and len(control['requests']) == 264
    assert sum(q['recorded_reads'] for q in control['requests']) == 4587
    assert all(q['native_payload_matches'] and q['read_sequence_matches']
               and not q['identity_errors'] for q in control['requests'])
traces = [[line for line in (root / f'diagnostic-control-t100/control-{n}/stderr.log').read_text().splitlines()
           if line.startswith('[income-probe] ')] for n in range(2)]
assert traces[0] and traces[0] == traces[1]
events = [json.loads(line[len('[income-probe] '):]) for line in traces[0]]
records = [json.loads(line) for line in (root / 'original-control-t100/native-decisions.jsonl').read_text().splitlines()]
begins = [(e['turn'], e['frame']) for e in events if e['event'] == 'root_begin']
assert begins == [(q['decision']['turn'], q['decision']['frame']) for q in records]
faith = {'DISTRICT_HOLY_SITE', 'BUILDING_SHRINE', 'BUILDING_TEMPLE'}
cases = []
for q in records:
    turn, frame = q['decision']['turn'], q['decision']['frame']
    for order in q['orders']:
        if order.get('kind') != 'produce' or order.get('verb') not in faith:
            continue
        cid = q['decision']['city_ids'][str(order['subject'])]
        actions = [a for a in q['decision']['native_actions'] if a.get('type') == 'produce' and a.get('city') == cid]
        candidates = [e for e in events if e['event'] == 'produce' and e['turn'] == turn
                      and e['frame'] == frame and e['city'] == cid and e['ok']]
        case = {'turn': turn, 'frame': frame, 'native_order': order, 'model_city': cid,
                'final_model_actions': actions, 'successful_root_produces': candidates,
                'decision_sha256': q['sha256']}
        if not actions or not candidates or candidates[-1]['item'] != actions[-1]['item']:
            case['status'] = 'unresolved final action/caller correspondence; retain'
        else:
            caller = candidates[-1]
            code = (root / 'diagnostic-source' / caller['caller_file']).read_text().splitlines()
            line = caller['caller_line']
            functions = [(i + 1, m.group(1)) for i, text in enumerate(code[:line])
                         if (m := re.match(r'\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+([A-Za-z0-9_]+)\b', text))]
            assert functions, caller
            case.update(status='last successful root Produce matches final model action and transported native order',
                        caller_function=functions[-1][1], function_start_line=functions[-1][0],
                        source_context='\n'.join(code[max(0, line - 9):line + 4]))
        cases.append(case)
report = {'scope': 'Qualified actual planning caller, not a counterfactual native completion or strength result.',
          'requests': len(records), 'root_frames': len(begins), 'trace_events': len(events),
          'root_produce_calls': sum(e['event'] == 'produce' for e in events), 'cases': cases,
          'caller_counts': dict(collections.Counter(c.get('caller_function', 'unresolved') for c in cases))}
out = root / 'faith-caller-analysis.json'
assert not out.exists()
out.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'requests': report['requests'], 'root_frames': report['root_frames'],
                  'trace_events': report['trace_events'], 'caller_counts': report['caller_counts'],
                  'cases': [{k: c.get(k) for k in ['turn', 'frame', 'model_city', 'caller_function', 'status']}
                            for c in cases]}, indent=2))
