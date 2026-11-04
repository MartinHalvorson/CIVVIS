"""Independently reconstruct every raw reply and recompute all CPU comparisons."""
import argparse
import collections
import gzip
import hashlib
import json
from pathlib import Path
import statistics


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run',type=Path,required=True)
    parser.add_argument('--selection',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    args = parser.parse_args()
    assert not args.out.exists()
    selection = json.loads(args.selection.read_text())
    registration = json.loads((args.run/'registration.json').read_text())
    assert sha(args.selection.read_bytes()) == registration['selection_sha256']
    assert sha(Path(selection['genome_file']).read_bytes()) == registration['genome_sha256']
    results = json.loads((args.run/'results.json').read_text())
    available = [i for i,c in enumerate(selection['cases']) if c.get('input_file')]
    assert len(selection['cases'])==48 and len(available)==42
    expected = 2*len(available)*(1+selection['control_rounds']+selection['comparison_rounds'])
    assert len(results)==expected and all(r['complete'] and r['exit']==0 for r in results)
    canonical = {i:gzip.decompress((args.run/f'case-{i:04d}.semantic.json.gz').read_bytes()) for i in available}
    for raw in canonical.values():
        assert set(json.loads(raw))=={'game','new_actions','plan','reasoning'}
    paths = {r['argv'][0] for r in results}
    binary_hashes = {p:sha(Path(p).read_bytes()) for p in paths}
    keys = set()
    for r in results:
        key = (r['phase'],r['round'],r['index'],r['label'])
        assert key not in keys
        keys.add(key)
        directory = args.run/f'{r["phase"]}-{r["round"]:02d}-{r["index"]:04d}-{r["label"]}'
        assert json.loads((directory/'result.json').read_text())==r
        expected_arm = 'baseline' if r['phase']=='control' else r['label']
        assert r['binary_arm']==expected_arm
        assert binary_hashes[r['argv'][0]]==registration['binaries'][expected_arm]
        assert sha((directory/'stderr').read_bytes())==r['stderr_sha256']
        semantic = canonical[r['index']]
        assert sha(semantic)==r['semantic_sha256']
        prefix,suffix = r['stdout_prefix'].encode(),r['stdout_suffix'].encode()
        digest = hashlib.sha256()
        for part in [prefix,semantic,suffix]:
            digest.update(part)
        assert digest.hexdigest()==r['stdout_sha256']
        assert len(prefix)+len(semantic)+len(suffix)==r['stdout_bytes']
        # Parsing only the exact nonsemantic bytes verifies original timing
        # metadata without repeatedly materializing the identical full world.
        metadata = json.loads(prefix+b'null'+suffix)
        case = selection['cases'][r['index']]
        assert metadata['case']==r['index'] and metadata['pid']==case['pid'] and metadata['turn']==case['turn']
        assert metadata['samples']==r['samples']
        assert len(r['samples'])==(1 if r['phase']=='qualification' else selection['repetitions'])
    for index in available:
        assert ('qualification',0,index,'baseline') in keys and ('qualification',0,index,'candidate') in keys
    rounds = {}
    by_case = {i:dict(index=i,turn=selection['cases'][i]['turn'],pid=selection['cases'][i]['pid'],readings={}) for i in available}
    for phase,count in [('control',selection['control_rounds']),('comparison',selection['comparison_rounds'])]:
        rows = []
        for number in range(count):
            group = [r for r in results if r['phase']==phase and r['round']==number]
            assert len(group)==2*len(available)
            totals = {a:sum(s['process_cpu_ticks'] for r in group if r['label']==a for s in r['samples']) for a in ['baseline','candidate']}
            assert totals['baseline']>0
            rows.append(dict(round=number,**totals,change_pct=100*(totals['candidate']/totals['baseline']-1)))
            for index in available:
                pair = {r['label']:sum(s['process_cpu_ticks'] for s in r['samples']) for r in group if r['index']==index}
                assert set(pair)=={'baseline','candidate'} and pair['baseline']>0
                by_case[index]['readings'].setdefault(phase,[]).append(dict(round=number,**pair,ratio=pair['candidate']/pair['baseline']))
        rounds[phase] = dict(readings=rows,median_change_pct=statistics.median(r['change_pct'] for r in rows))
    original = json.loads((args.run/'summary.json').read_text())
    assert original['complete'] and original['qualified'] and not original['errors']
    assert rounds==original['rounds']
    adjusted = []
    if selection.get('interleave_phases'):
        assert selection['control_rounds']==selection['comparison_rounds']
        for control,comparison in zip(rounds['control']['readings'],rounds['comparison']['readings']):
            q=control['candidate']/control['baseline']
            r=comparison['candidate']/comparison['baseline']
            adjusted.append(dict(round=control['round'],change_pct=100*(r/q-1)))
        for case in by_case.values():
            case['control_adjusted_changes_pct']=[100*(b['ratio']/a['ratio']-1) for a,b in zip(case['readings']['control'],case['readings']['comparison'])]
    for case in by_case.values():
        case['comparison_median_change_pct']=statistics.median(100*(r['ratio']-1) for r in case['readings']['comparison'])
    out = dict(complete=True,verified_raw_replies=len(results),verified_canonical_worlds=len(canonical),
               source_registration=registration,selection_sha256=sha(args.selection.read_bytes()),
               rounds=rounds,control_adjusted_rounds=adjusted,
               control_adjusted_median_change_pct=statistics.median(r['change_pct'] for r in adjusted) if adjusted else None,
               cases=[by_case.get(i,dict(index=i,unavailable=True)) for i in range(len(selection['cases']))],
               limits='Fixed synthetic model workload; raw controls and comparisons retained. Control adjustment is secondary. No Native, whole-game or strength estimate.')
    args.out.write_text(json.dumps(out,indent=2,allow_nan=False)+'\n')
    print(json.dumps({k:v for k,v in out.items() if k in ['complete','verified_raw_replies','verified_canonical_worlds','rounds','control_adjusted_rounds','control_adjusted_median_change_pct']},indent=2))


if __name__=='__main__':
    main()
