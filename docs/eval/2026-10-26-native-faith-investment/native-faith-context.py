"""Retain every distinct captured input behind the nine faith-build requests."""
from pathlib import Path
import argparse, collections, gzip, hashlib, json, sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[2] / 'tools'))
from civ6_decision_trace import captured_input

parser = argparse.ArgumentParser()
parser.add_argument('--root', type=Path, required=True)
args = parser.parse_args()
root = args.root
qualified = json.loads((root / 'original-control-t100/qualification.json').read_text())
assert qualified['passed'] and len(qualified['controls']) == 2
records = [json.loads(line) for line in (root / 'original-control-t100/native-decisions.jsonl').read_text().splitlines()]

def state_at(raw, turn, frame):
    end = len(raw)
    while end > 0:
        pos = max(raw.rfind(b'"kind": "state"', 0, end), raw.rfind(b'"kind":"state"', 0, end))
        if pos < 0:
            return None
        start = raw.rfind(b'\n', 0, pos) + 1
        stop = raw.find(b'\n', pos)
        state = json.loads(raw[start:len(raw) if stop < 0 else stop])
        if state.get('kind') == 'state' and (state.get('turn'), state.get('frame', 0)) == (turn, frame):
            return state
        end = start
    return None

report = {'scope': 'Every distinct actual captured read retained, including changing inputs; no selected lucky snapshot or private gate inference.', 'cases': []}
for decision in records:
    orders = [o for o in decision['orders'] if o.get('kind') == 'produce'
              and o.get('verb') in {'DISTRICT_HOLY_SITE', 'BUILDING_SHRINE', 'BUILDING_TEMPLE'}]
    if not orders:
        continue
    turn, frame = decision['decision']['turn'], decision['decision']['frame']
    capture = decision['input_capture']
    assert capture['complete'] and all('snapshot_id' in r for r in capture['reads'])
    counts = collections.Counter()
    for read in capture['reads']:
        counts[read['snapshot_id']] += read['count']
    case = {'turn': turn, 'frame': frame, 'orders': orders, 'decision_sha256': decision['sha256'],
            'capture': capture, 'observations': []}
    for snapshot, count in sorted(counts.items()):
        raw = captured_input(capture, snapshot)
        state = state_at(raw, turn, frame)
        case['observations'].append({'snapshot_id': snapshot, 'read_count': count,
            'prefix_sha256': hashlib.sha256(raw).hexdigest(), 'state': state,
            'status': 'exact matching native state' if state is not None else 'state identity unresolved; retained'})
    report['cases'].append(case)
assert sum(len(c['orders']) for c in report['cases']) == 9
out = root / 'native-faith-context.json.gz'
assert not out.exists()
with gzip.GzipFile(filename=str(out), mode='wb', mtime=0) as f:
    f.write(json.dumps(report, sort_keys=True, separators=(',', ':')).encode())
assert json.loads(gzip.decompress(out.read_bytes())) == report
print(json.dumps({'cases': len(report['cases']), 'distinct_observations': sum(len(c['observations']) for c in report['cases']),
                  'unresolved': sum(o['state'] is None for c in report['cases'] for o in c['observations']),
                  'sha256': hashlib.sha256(out.read_bytes()).hexdigest()}, indent=2))
