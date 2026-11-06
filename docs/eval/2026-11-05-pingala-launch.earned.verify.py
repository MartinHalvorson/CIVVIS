"""Audit archived same-program proposals and actual reads, not native outcomes."""
import collections
import hashlib
import json
import sys
import tarfile

sha = lambda b: hashlib.sha256(b).hexdigest()
with tarfile.open(sys.argv[1], 'r:*') as tar:
    members = [m for m in tar.getmembers() if m.isfile()]
    assert len({m.name for m in members}) == len(members)
    files = {m.name: tar.extractfile(m).read() for m in members}
j = lambda name: json.loads(files[name])
m = j('manifest.json')
assert set(files) == set(m['files']) | {'manifest.json'}
assert all(sha(files[n]) == digest for n, digest in m['files'].items())
intent = j('evidence/intent.json')
build = j('evidence/build-process.json')
assert build['exit'] == 0 and build['source_unchanged']
assert build['source_revision'] == m['source_revision'] == intent['provenance']['revision']
assert build['binary_sha256'] == m['binary_sha256'] == intent['binary_sha256'] == intent['provenance']['sha256']
assert sha(files['evidence/run-probe.py']) == intent['harness_sha256']
assert intent['arm_order'] == ['off', 'on', 'on', 'off']
results = []
requests = total = 0
for meta in j('captures.json'):
    r = j(meta['result'])
    assert r['exit'] == 0 and r['binary_sha256'] == build['binary_sha256']
    case = next(c for c in intent['cases'] if c['turn'] == r['turn'])
    samples = r['warmup_results'] + [r]
    declared = case['warmups'] + [case]
    assert len(samples) == len(declared)
    for sample, expected in zip(samples, declared):
        turn = expected['turn']
        assert sample['turn'] == turn
        spec = meta['captures'][str(turn)]
        fixed = files[spec['input']]
        assert sha(fixed) == expected['input_sha256']
        payload = sample['payload']
        assert payload['decision']['turn'] == turn and payload['decision']['frame'] == 0
        assert payload['decision']['victory_portfolio']['assigned_target'] == 'science'
        cap = payload['input_capture']
        assert cap['complete'] is True and cap['error'] is None and cap['reads']
        blob = files[spec['raw']]
        rows = [json.loads(line) for line in files[spec['index']].splitlines()]
        offset = 0
        for number, row in enumerate(rows):
            assert row['id'] == number and row['offset'] == offset
            assert row['parent'] is None or 0 <= row['parent'] < number
            before = 0 if row['parent'] is None else rows[row['parent']]['total_bytes']
            assert row['total_bytes'] == before + row['append_bytes']
            offset += row['append_bytes']
        assert offset == len(blob)
        actual = []
        for read in cap['reads']:
            assert type(read['count']) is int and read['count'] > 0
            ident = read['snapshot_id']
            chain = []
            while ident is not None:
                assert 0 <= ident < len(rows)
                row = rows[ident]
                chain.append(row)
                ident = row['parent']
            data = b''.join(blob[x['offset']:x['offset'] + x['append_bytes']] for x in reversed(chain))
            assert data == fixed
            actual.extend([sha(data)] * read['count'])
        assert actual == sample['actual_read_sha256'] and len(actual) == sample['actual_read_count']
        total += len(actual)
        requests += 1
    results.append(r)
assert len(results) == 12 and requests == 44 and total == m['registered_reads'] == 780
control = lambda sample: {k: sample['payload'][k] for k in ['orders', 'decision']}

def argv(r):
    args = list(r['argv'])
    args[args.index('--mirror') + 1] = '<fixed-input>'
    if r['arm'] == 'on':
        positions = [i for i in range(len(args)-1) if args[i:i+2] == ['--with', m['tag']]]
        assert len(positions) == 1
        del args[positions[0]:positions[0]+2]
    return args

changed = {}
for case in intent['cases']:
    turn = case['turn']
    rs = sorted((r for r in results if r['turn'] == turn), key=lambda r: r['round'])
    assert [r['arm'] for r in rs] == ['off', 'on', 'on', 'off']
    for a, b in [(rs[0], rs[3]), (rs[1], rs[2])]:
        assert [control(x) for x in a['warmup_results']+[a]] == [control(x) for x in b['warmup_results']+[b]]
    assert len({json.dumps(argv(r)) for r in rs}) == 1
    a, b = [dict(r['genome']) for r in rs[:2]]
    for field in ['forced', 'treatments']:
        before, after = a.pop(field), b.pop(field)
        assert len(before) == len(set(before)) and len(after) == len(set(after))
        assert m['tag'] not in before and set(after)-{m['tag']} == set(before) and m['tag'] in after
    assert a == b
    assert [w['payload']['orders'] for w in rs[0]['warmup_results']] == [w['payload']['orders'] for w in rs[1]['warmup_results']]
    if turn in [142, 153]:
        expected = 'PROJECT_LAUNCH_EARTH_SATELLITE' if turn == 142 else 'PROJECT_LAUNCH_MOON_LANDING'
        for r in rs:
            assert any(o['kind'] == 'produce' and o['subject'] == 131073 and o['verb'] == expected for o in r['warmup_results'][-1]['payload']['orders'])
    changed[str(turn)] = rs[0]['payload']['orders'] != rs[1]['payload']['orders']
    if turn == 153:
        extra = {'kind':'governor_assign','subject':131073,'verb':'GOVERNOR_THE_EDUCATOR','x':0,'y':-1}
        for off, on in [(rs[0], rs[1]), (rs[3], rs[2])]:
            counter = lambda r: collections.Counter(json.dumps(o, sort_keys=True) for o in r['payload']['orders'])
            assert counter(on)-counter(off) == collections.Counter([json.dumps(extra, sort_keys=True)])
            assert not counter(off)-counter(on)
            assert 'governor_exchange=return_retained' in on['payload']['note']
assert changed == m['changed_cases'] == {'134':False, '142':False, '153':True}
for name in m['pairs']:
    pair = j(name)
    assert pair['complete'] and pair['repeat'] in [1, 2]
    turn = pair['request'][0]
    case = next(c for c in intent['cases'] if c['turn'] == turn)
    rs = [next(r for r in results if r['turn'] == turn and r['round'] == n) for n in ([0,1] if pair['repeat'] == 1 else [3,2])]
    assert len(pair['reads']) == len(case['warmups'])+1
    for index, (read, expected) in enumerate(zip(pair['reads'], case['warmups']+[case])):
        assert read['complete'] and read['request'] == [expected['turn'],0] and read['sha256'] == expected['input_sha256']
        assert read['bytes'] == len(files['blobs/'+expected['input_sha256']])
        for arm, r in zip(read['arms'], rs):
            wanted = {k:r[k] for k in ['arm','exit','argv','genome','payload','binary_sha256']}
            wanted['payload'] = (r['warmup_results']+[r])[index]['payload']
            assert arm == wanted
# Read the physical native state independently of proposal summaries.
case = next(c for c in intent['cases'] if c['turn'] == 153)
states = []
seat = None
for line in files['blobs/'+case['input_sha256']].splitlines():
    row = json.loads(line)
    if row.get('kind') == 'seat':
        seat = row
    if row.get('kind') == 'state':
        states.append(row)
assert seat['civ'] == 'CIVILIZATION_INDONESIA' and seat['leader'] == 'LEADER_GITARJA'
assert seat['difficulty'] == 'DIFFICULTY_DEITY' and seat['speed'] == 'GAMESPEED_ONLINE'
state = states[-1]
assert state['turn'] == 153 and state['frame'] == 0
city = next(c for c in state['cities'] if c['id'] == 131073)
assert city['name'] == 'Palembang' and city['producing'] == 'PROJECT_LAUNCH_MOON_LANDING'
assert city['production_turns'] == 16
assert any(d['type'] == 'DISTRICT_SPACEPORT' and d['complete'] and not d['pillaged'] for d in city['districts'])
assert any(g['type'] == 'GOVERNOR_THE_EDUCATOR' and g['city'] == 65536 and g['established'] for g in state['governors'])
assert any(g['type'] == 'GOVERNOR_THE_RESOURCE_MANAGER' and g['city'] == 131073 and g['established'] for g in state['governors'])
seen = set()
for state in states:
    if 105 <= state['turn'] <= 153:
        seen.add(state['turn'])
        for g in state['governors']:
            expected = {'GOVERNOR_THE_EDUCATOR':65536, 'GOVERNOR_THE_RESOURCE_MANAGER':131073}.get(g['type'])
            if expected is not None:
                assert g['city'] == expected and g['established'] and g['neutralized_turns'] == 0
assert seen == set(range(105,154))
print(json.dumps({'target_queries':12,'warmup_queries':32,'verified_registered_reads':total,'changed_cases':changed,'scope':'Proposal activation only. No original private history, native execution, return-leg acknowledgment, actual completion gain, or strength claim.'}))
