import csv, json, pathlib, re, statistics
root = pathlib.Path(__file__).resolve().parent
prefix = '2026-10-04-worked-production-technology'
result = {}
for difficulty, suffix, expected in [('emperor', '', 12), ('deity', '-deity', 8)]:
    data = {}
    outcomes = {}
    for arm in ['control', 'candidate']:
        stem = root / (prefix + suffix + '-' + arm)
        with stem.with_suffix('.csv').open() as f:
            data[arm] = {(int(r['seed']), int(r['turn'])): r for r in csv.DictReader(f)}
        matches = re.findall(r'(\d+): turn (\d+), winner (Some\(\d+\)|None), alive (true|false)', stem.with_suffix('.txt').read_text())
        assert len(matches) == expected, (difficulty, arm, len(matches))
        outcomes[arm] = {
            'games': len(matches), 'wins': sum(w == 'Some(0)' for _, _, w, _ in matches),
            'eliminated': sum(a == 'false' for _, _, _, a in matches),
            'raw': [{'seed': int(s), 'turn': int(t), 'winner': w, 'alive': a == 'true'} for s,t,w,a in matches]
        }
    assert {s for s,t in data['control']} == {s for s,t in data['candidate']}
    rows = []
    for turn in [25,50,75,100,125,150]:
        common = sorted(k for k in data['control'].keys() & data['candidate'].keys() if k[1] == turn)
        if not common: continue
        row = {'turn': turn, 'matched_games': len(common)}
        for key in ['production','cumulative_production','science','culture','cities']:
            a = statistics.mean(float(data['control'][k][key]) for k in common)
            b = statistics.mean(float(data['candidate'][k][key]) for k in common)
            row[key] = {'control': a, 'candidate': b, 'change_pct': 100*(b/a-1) if a else None}
        deltas = [float(data['candidate'][k]['production'])-float(data['control'][k]['production']) for k in common]
        row['production_pairs'] = {'higher': sum(d>1e-6 for d in deltas), 'lower': sum(d < -1e-6 for d in deltas), 'equal': sum(abs(d)<=1e-6 for d in deltas)}
        row['production_to_strongest_rival'] = {}
        for arm in ['control','candidate']:
            ratios = [float(data[arm][k]['production'])/float(data[arm][k]['rival_production']) for k in common if float(data[arm][k]['rival_production'])>0]
            row['production_to_strongest_rival'][arm] = {'mean': statistics.mean(ratios) if ratios else None, 'games': len(ratios)}
        rows.append(row)
    result[difficulty] = {'outcomes': outcomes, 'checkpoints': rows}
out = root / (prefix + '-summary.json')
out.write_text(json.dumps(result, indent=2) + '\n')
for d, r in result.items():
    print(d, {a:{k:v for k,v in o.items() if k!='raw'} for a,o in r['outcomes'].items()})
    for row in r['checkpoints']:
        print(row['turn'],row['matched_games'],*[f"{k}={row[k]['control']:.3f}->{row[k]['candidate']:.3f} ({row[k]['change_pct']:+.2f}%)" for k in ['production','cumulative_production','science','culture']], row['production_pairs'])
