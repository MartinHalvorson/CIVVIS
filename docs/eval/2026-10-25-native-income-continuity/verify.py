"""Independently verify the compact evidence; never open a live game."""
from pathlib import Path
import gzip
import hashlib
import json
import statistics
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[2] / 'tools'))
from civ6_decision_trace import exact_fact
from native_city_route_replay import control_payload


def packed(name):
    return json.loads(gzip.decompress((HERE / name).read_bytes()))


registration = json.loads((HERE / 'registration.json').read_text())
for name, expected in registration['artifacts'].items():
    assert hashlib.sha256((HERE / name).read_bytes()).hexdigest() == expected, name
manifest = packed('source-manifest.json.gz')
patch = gzip.decompress((HERE / 'diagnostic-root.patch.gz').read_bytes())
assert hashlib.sha256(patch).hexdigest() == manifest['diagnostic_patch_sha256']

qualified = packed('qualified-market.json.gz')
traces = packed('complete-market-traces.json.gz')
native = [json.loads(line) for line in traces['native_decisions'].splitlines()]
assert len(native) == 307
for name, control in traces['controls'].items():
    replies = [json.loads(line) for line in control['files']['replies.jsonl'].splitlines()]
    assert len(replies) == len(native), name
    for original, reply in zip(native, replies):
        assert exact_fact(control_payload(original), control_payload(reply)), name
    for filename, expected in control['sha256'].items():
        assert hashlib.sha256(control['files'][filename].encode()).hexdigest() == expected

events = []
for number in range(2):
    stderr = traces['controls'][f'diagnostic-{number}']['files']['stderr.log']
    events.append([line for line in stderr.splitlines() if line.startswith('[income-probe] ')])
assert len(events[0]) == 132 and events[0] == events[1]
for kind in ['original', 'diagnostic']:
    qualification = qualified[kind]
    assert qualification['passed'] and len(qualification['controls']) == 2
    for control in qualification['controls']:
        assert len(control['requests']) == 307
        assert sum(row['recorded_reads'] for row in control['requests']) == 5336
        assert all(row['native_payload_matches'] and row['read_sequence_matches']
                   and not row['identity_errors'] for row in control['requests'])

cohort = packed('income-eight.json.gz')
continuity = cohort['continuity']['runs']
growth = cohort['growth']['runs']
assert len(continuity) == len(growth) == 8
assert [row['run'] for row in continuity] == [row['run'] for row in growth]
starts = [min((r['turn'] for r in row['requests']), default=None) for row in continuity]
completions = [min(row['first_completed_income_observation'].values(), default=None)
               for row in continuity]
assert starts == [167, 105, None, 118, 139, 91, 111, 95]
assert completions == [None, 125, None, 123, 141, 98, 113, 101]
assert statistics.median(t for t in starts if t is not None) == 111
assert statistics.median(t for t in completions if t is not None) == 118
assert sum(t is not None and t <= 100 for t in completions) == 1

science_ratios, per_citizen_ratios = [], []
smaller = 0
for row in growth:
    rivals = [r for r in row['rivals'] if r.get('science') is not None
              and (r.get('public_stats') or {}).get('population')]
    leader = max(rivals, key=lambda r: r['science'])
    ours = row['public_stats']['population']
    theirs = leader['public_stats']['population']
    smaller += ours < theirs
    science_ratios.append(leader['science'] / row['science'])
    per_citizen_ratios.append((leader['science'] / theirs) / (row['science'] / ours))
assert smaller == 6
assert round(statistics.median(science_ratios), 2) == 2.77
assert round(statistics.median(per_citizen_ratios), 2) == 1.94

witness = qualified['native_witness']
assert witness['input_status'] == 'exact single snapshot'
assert len(witness['native_verification_receipts']) == 1
observed = witness['exact_captured_input_observations'][0]
assert observed['gold'] == 1 and observed['income'] == 3.5
assert observed['capital']['producing'] == 'BUILDING_MARKET'
assert observed['market_quote']['p'] == 2
knights = [order for order in witness['decision']['orders']
           if order.get('kind') == 'produce' and order.get('subject') == 65536]
assert len(knights) == 1 and knights[0]['verb'] == 'UNIT_KNIGHT'
receipt = witness['native_verification_receipts'][0]
assert receipt['turn'] == 102 and receipt['subject'] == 65536
assert receipt['order_kind'] == 'produce' and receipt['verb'] == 'UNIT_KNIGHT'
capital = witness['decision']['decision']['city_ids']['65536']
writers = [row for row in qualified['root_trace']['events']
           if row['event'] == 'produce' and row['turn'] == 102 and row['frame'] == 0
           and row['city'] == capital and row['ok']]
assert writers[-1]['item'] == {'unit': 'knight'}
assert writers[-1]['caller_file'] == 'src/ai/advanced/capital_walls_before_the_surprise.rs'

print('Verified 1,228 complete payloads, four 5,336-read controls, both complete')
print('132-event traces, the exact native Knight witness and all eight census members.')
