"""Verify proposal activation and every archived read; does not prove native strength."""
import collections
import hashlib
import json
import sys
import tarfile

sha = lambda b: hashlib.sha256(b).hexdigest()
with tarfile.open(sys.argv[1], 'r:*') as archive:
    members = [m for m in archive.getmembers() if m.isfile()]
    assert len({m.name for m in members}) == len(members)
    files = {m.name: archive.extractfile(m).read() for m in members}
j = lambda name: json.loads(files[name])
m = j('manifest.json')
assert set(files) == set(m['files']) | {'manifest.json'}
assert all(sha(files[n]) == h for n, h in m['files'].items())
intent = j('evidence/probe-intent.json')
build = j('evidence/build-process.json')
assert build['exit'] == 0 and build['source_unchanged']
assert m['source_revision'] == intent['source_revision'] == build['source_revision']
assert m['binary_sha256'] == intent['binary_sha256'] == build['binary_sha256']
assert sha(files['evidence/run-probe.py']) == intent['harness_sha256']
assert all(build['source_manifest'][p] == h for p, h in intent['runtime_manifest'].items())
assert intent['arm_order'] == ['off', 'on', 'on', 'off']
results = []
reads = 0
physical_inputs = {}
for meta in j('captures.json'):
    r = j(meta['result'])
    case = next(c for c in intent['cases'] if c['case'] == r['case'])
    assert r['run'] == case['run'] and r['turn'] == case['turn']
    assert r['exit'] == 0 and r['binary_sha256'] == m['binary_sha256']
    fixed = files[meta['input']]
    assert sha(fixed) == case['input_sha256'] == r['input_sha256']
    if case['input_sha256'] not in physical_inputs:
        native = [json.loads(line) for line in fixed.splitlines()]
        seat = [row for row in native if row.get('kind') == 'seat'][-1]
        state = [row for row in native if row.get('kind') == 'state'][-1]
        assert seat['civ'] == 'CIVILIZATION_INDONESIA' and seat['leader'] == 'LEADER_GITARJA'
        assert seat['difficulty'] == 'DIFFICULTY_DEITY' and seat['speed'] == 'GAMESPEED_ONLINE'
        assert seat['size'] == 'MAPSIZE_SMALL' and seat['map'] == 'Continents.lua'
        assert state['turn'] == case['turn'] and state['frame'] == 0
        physical_inputs[case['input_sha256']] = True

    payload = r['payload']
    assert payload['turn'] == payload['decision']['turn'] == case['turn']
    assert payload['decision']['frame'] == 0
    assert payload['decision']['victory_portfolio']['assigned_target'] == 'science'
    cap = payload['input_capture']
    assert cap['complete'] and cap['error'] is None and cap['reads']
    blob = files[meta['raw']]
    rows = [json.loads(line) for line in files[meta['index']].splitlines()]
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
    assert len(actual) == r['actual_reads'] and actual == r['actual_read_sha256']
    reads += len(actual)
    results.append(r)
assert len(results) == m['queries'] == 24 and reads == m['registered_reads'] == 408

def normalized_argv(r):
    a = list(r['argv'])
    for flag in ['--mirror']:
        a[a.index(flag) + 1] = '<fixed-path>'
    if r['arm'] == 'on':
        positions = [i for i in range(len(a) - 1) if a[i:i + 2] == ['--with', m['tag']]]
        assert len(positions) == 1
        del a[positions[0]:positions[0] + 2]
    return a

control = lambda r: {k: r['payload'][k] for k in ['orders', 'decision']}
counter = lambda r: collections.Counter(json.dumps(o, sort_keys=True) for o in r['payload']['orders'])
deltas = []
for case in intent['cases']:
    rs = sorted((r for r in results if r['case'] == case['case']), key=lambda r: r['round'])
    assert [r['arm'] for r in rs] == ['off', 'on', 'on', 'off']
    assert control(rs[0]) == control(rs[3]) and control(rs[1]) == control(rs[2])
    assert len({json.dumps(normalized_argv(r)) for r in rs}) == 1
    a, b = [dict(r['genome']) for r in rs[:2]]
    for field in ['forced', 'treatments']:
        before, after = a.pop(field), b.pop(field)
        assert len(before) == len(set(before)) and len(after) == len(set(after))
        assert m['tag'] not in before and m['tag'] in after and set(after) - {m['tag']} == set(before)
    assert a == b
    off, on = counter(rs[0]), counter(rs[1])
    deltas.append({'case': case['case'], 'run': case['run'], 'turn': case['turn'],
                   'changed': rs[0]['payload']['orders'] != rs[1]['payload']['orders'],
                   'removed': dict(off - on), 'added': dict(on - off)})
assert deltas == m['deltas']
assert [x['case'] for x in deltas if x['changed']] == [5]
for name in m['pairs']:
    pair = j(name)
    case_id = int(name.split('case-')[1].split('-')[0])
    case = next(c for c in intent['cases'] if c['case'] == case_id)
    assert pair['complete'] and pair['request'] == [case['turn'], 0]
    assert pair['repeat'] in [1, 2] and len(pair['reads']) == 1
    read = pair['reads'][0]
    assert read['complete'] and read['sha256'] == case['input_sha256'] and read['request'] == pair['request']
    assert read['bytes'] == len(files['blobs/' + case['input_sha256']])
    rounds = [0, 1] if pair['repeat'] == 1 else [3, 2]
    for arm, n in zip(read['arms'], rounds):
        r = next(r for r in results if r['case'] == case_id and r['round'] == n)
        assert arm == {k: r[k] for k in ['arm', 'exit', 'argv', 'genome', 'payload', 'binary_sha256']}
case = next(c for c in intent['cases'] if c['case'] == 5)
state = [json.loads(line) for line in files['blobs/' + case['input_sha256']].splitlines()
         if json.loads(line).get('kind') == 'state'][-1]
rs = sorted((r for r in results if r['case'] == 5), key=lambda r: r['round'])
for r, cid, quoted in [(rs[0], 720906, 26), (rs[1], 131073, 6)]:
    city = next(c for c in state['cities'] if c['id'] == cid)
    assert next(b for b in city['buildable'] if b['t'] == 'UNIT_SPY')['p'] == quoted
    assert any(o['kind'] == 'produce' and o['subject'] == cid and o['verb'] == 'UNIT_SPY' for o in r['payload']['orders'])
assert {o for o in counter(rs[0]) - counter(rs[1])} == {
    json.dumps(o, sort_keys=True) for o in [
        {'kind': 'produce', 'subject': 131073, 'verb': 'UNIT_FIELD_CANNON', 'x': None, 'y': None},
        {'kind': 'produce', 'subject': 720906, 'verb': 'UNIT_SPY', 'x': None, 'y': None},
        {'kind': 'produce', 'subject': 262147, 'verb': 'BUILDING_ZOO', 'x': None, 'y': None}]}
expected_added = collections.Counter(json.dumps(o, sort_keys=True) for o in [
    {'kind': 'produce', 'subject': 131073, 'verb': 'UNIT_SPY', 'x': None, 'y': None},
    {'kind': 'produce', 'subject': 720906, 'verb': 'UNIT_FIELD_CANNON', 'x': None, 'y': None}])
assert counter(rs[1]) - counter(rs[0]) == expected_added
assert all(n == 1 for n in (counter(rs[0]) - counter(rs[1])).values())
assert collections.Counter(json.dumps(o, sort_keys=True) for o in rs[0]['payload']['orders'] if o['kind'] == 'unit') == collections.Counter(json.dumps(o, sort_keys=True) for o in rs[1]['payload']['orders'] if o['kind'] == 'unit')
print(json.dumps({'queries': len(results), 'reconstructed_reads': reads, 'changed_cases': [5],
                  'scope': 'Repeated offline proposal activation only; coupled production differences retained. No native continuation, actual timing gain or strength result.'}))
