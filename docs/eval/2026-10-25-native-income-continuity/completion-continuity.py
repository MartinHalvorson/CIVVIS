"""Read-only descriptive continuity audit of the entire frozen eight-run cohort."""
from pathlib import Path
import collections
import datetime
import gzip
import hashlib
import json

ROOT = Path('/Users/martbot-mbp-m5-max-128/civvis-tactics-results/2026-10-25/income-district-funnel')
TYPES = {'DISTRICT_COMMERCIAL_HUB', 'DISTRICT_HARBOR', 'BUILDING_MARKET', 'BUILDING_LIGHTHOUSE'}
index_path = ROOT / 'native-delivery/observer.json'
index = json.loads(index_path.read_text())
registration = json.loads((ROOT / 'completion-continuity-registration.json').read_text())
assert [r['run'] for r in index['attempts']] == registration['attempts']
assert len(index['attempts']) == 8 and all(r.get('archive_complete') for r in index['attempts'])


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_file(directory, row, name):
    data = (directory / (name + '.gz')).read_bytes()
    assert digest(data) == row['files'][name]['archive_sha256']
    raw = gzip.decompress(data)
    assert digest(raw) == row['files'][name]['sha256']
    return raw


report = {
    'observed_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'registration': registration,
    'index_sha256': digest(index_path.read_bytes()),
    'limits': [
        'All outcomes and the first-turn-100 census were known before this descriptive audit.',
        'Mixed revisions and overlapping cohorts do not isolate causal strength effects.',
        'Use the first exported state at each turn. A completion is first observed, not an exact engine completion timestamp.',
        'Later city absence is an ownership/visibility discontinuity, not by itself a classified combat capture.',
        'A different queue head after an order is an observation, not a qualified private caller diagnosis.',
        'A reissued production order is not an additional completed building.',
    ],
    'runs': [],
}
for row in index['attempts']:
    directory = ROOT / 'native-delivery' / row['run']
    event_path = directory / 'events.jsonl.gz'
    assert digest(event_path.read_bytes()) == row['files']['events.jsonl']['archive_sha256']
    raw_digest = hashlib.sha256()
    turns = {}
    verified = []
    with gzip.open(event_path, 'rb') as stream:
        for raw in stream:
            raw_digest.update(raw)
            if b'"kind": "state"' in raw or b'"kind":"state"' in raw:
                state = json.loads(raw)
                if state.get('kind') != 'state' or state['turn'] in turns:
                    continue
                turns[state['turn']] = {
                    'turn': state['turn'], 'frame': state.get('frame', 0),
                    'gold': state.get('gold'), 'income': state.get('gold_per_turn'),
                    'science': state.get('science'),
                    'cities': [{k: c.get(k) for k in [
                        'id', 'name', 'producing', 'production_progress', 'production_cost',
                        'production_turns', 'districts', 'buildings', 'buildable',
                    ]} for c in state['cities']],
                }
            elif b'order_verified' in raw:
                event = json.loads(raw)
                if event.get('kind') == 'order_verified' and event.get('verb') in TYPES:
                    verified.append(event)
    assert raw_digest.hexdigest() == row['files']['events.jsonl']['sha256']
    decisions = [json.loads(line) for line in read_file(directory, row, 'decisions.jsonl').splitlines()]
    requests = []
    for number, decision in enumerate(decisions, 1):
        for order in decision.get('orders', []):
            if order.get('kind') == 'produce' and order.get('verb') in TYPES:
                requests.append({
                    'turn': decision['decision']['turn'], 'frame': decision['decision']['frame'],
                    'decision_line': number, 'decision_sha256': decision['sha256'], 'order': order,
                    'matching_same_turn_city_verb_receipts': [r for r in verified if
                        r.get('turn') == decision['decision']['turn'] and
                        r.get('subject') == order['subject'] and r.get('verb') == order['verb']],
                })
    first_complete = {}
    complete_by_city = {}
    for turn, state in sorted(turns.items()):
        for city in state['cities']:
            completed = {d['type'] for d in city['districts'] or [] if d.get('complete', True)}
            completed.update(city['buildings'] or [])
            for kind in completed & TYPES:
                first_complete.setdefault(kind, turn)
                complete_by_city.setdefault((city['id'], kind), turn)
    by_city = {turn: {c['id']: c for c in state['cities']} for turn, state in turns.items()}
    for request in requests:
        start = request['turn']
        city_id = request['order']['subject']
        kind = request['order']['verb']
        completed = complete_by_city.get((city_id, kind))
        request['first_complete_observation_this_city_type'] = completed
        request['already_observed_complete_before_request'] = completed is not None and completed <= start
        after = [(t, cities.get(city_id)) for t, cities in sorted(by_city.items()) if t > start and
                 (completed is None or t <= completed)]
        request['first_city_absence_before_completion'] = next((t for t, c in after if c is None), None)
        request['first_later_observed_head'] = next(({'turn': t, 'producing': c['producing'],
            'progress': c['production_progress'], 'cost': c['production_cost'], 'remaining_turns': c['production_turns']}
            for t, c in after if c is not None), None)
        request['different_heads_before_completion'] = [
            {'turn': t, 'producing': c['producing'], 'progress': c['production_progress'],
             'requested_item_quote': next((q for q in c['buildable'] or [] if q.get('t') == kind), None)}
            for t, c in after if c is not None and t < (completed if completed is not None else float('inf'))
            and c['producing'] != kind
        ]
    budgets = [{k: s[k] for k in ['turn', 'frame', 'gold', 'income', 'science']} for _, s in sorted(turns.items())]
    result = {'run': row['run'], 'revision': row['source']['provenance']['revision'],
              'outcome': row['summary'].get('outcome'), 'first_completed_income_observation': first_complete,
              'requests': requests, 'verified_receipts': verified, 'first_observed_turn_budgets': budgets,
              'states_observed': len(turns), 'first_complete_by_city_type': [
                  {'city': cid, 'type': kind, 'turn': t} for (cid, kind), t in complete_by_city.items()]}
    report['runs'].append(result)
    print(row['run'], 'first complete', first_complete, 'requests', len(requests), flush=True)

out = ROOT / 'completion-continuity.json.gz'
assert not out.exists()
with gzip.GzipFile(filename=str(out), mode='wb', mtime=0) as stream:
    stream.write(json.dumps(report, sort_keys=True, separators=(',', ':')).encode())
assert json.loads(gzip.decompress(out.read_bytes())) == report
print('Saved', out, 'sha256', digest(out.read_bytes()), flush=True)
