"""Read-only first-input/profile controls before the separate Native lease.

Use the actual delivered adapter, complete first request and exact driver flags.
All outputs go to a fresh disposable directory; never open the real orders DB.
This command does not arm a trial or execute any Native action.
"""
import argparse,hashlib,importlib.util,json,pathlib,subprocess,sys

def validate_genomes(actual, off, on, repeated, gene):
    """Preserve every existing list position; only one candidate may be added."""
    if off != actual or repeated != actual:
        raise ValueError('OFF or OFF-repeat genome differs from the actual played genome')
    normalized=json.loads(json.dumps(on))
    for field in ['treatments','forced']:
        base=actual.get(field); enabled=on.get(field)
        if not isinstance(base,list) or not isinstance(enabled,list) or gene in base or enabled.count(gene)!=1:
            raise ValueError('candidate addition is not unique in '+field)
        normalized[field]=[tag for tag in enabled if tag!=gene]
    if normalized != actual:
        changed=sorted(key for key in set(normalized)|set(actual) if normalized.get(key)!=actual.get(key))
        raise ValueError('ON changed fields beyond the candidate: '+', '.join(changed))

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--repo',type=pathlib.Path,required=True)
    p.add_argument('--repair',required=True)
    p.add_argument('--qualification-run',required=True)
    p.add_argument('--out',type=pathlib.Path,required=True)
    a=p.parse_args();a.out.mkdir(parents=True,exist_ok=False)
    observer_path=pathlib.Path(__file__).with_name('2026-10-26-forward-airfield-native.observer.py')
    spec=importlib.util.spec_from_file_location('observer',observer_path)
    observer=importlib.util.module_from_spec(spec);spec.loader.exec_module(observer)
    root=pathlib.Path.home()/'civvis-civ6-runs'
    q=observer.qualify(a.repo.resolve(),root,a.qualification_run,a.repair)
    (a.out/'qualification.json').write_text(json.dumps(q,indent=2)+'\n')
    sys.path.insert(0,str(a.repo/'tools'))
    from civ6_decision_trace import captured_input
    from civ6_ladder import decider_genome
    first=q['first_decision'];capture=first['input_capture'];words=q['driver']['argv']
    assert '--civvis-victory' in words,'actual driver has no explicit lane'
    lane=words[words.index('--civvis-victory')+1]
    args=[]
    for i,word in enumerate(words):
        if word in ['--civvis-with','--civvis-without']:
            args.extend(['--with' if word=='--civvis-with' else '--without',words[i+1]])
    report={'scope':__doc__,'qualification_run':a.qualification_run,'reads':[],'complete':False}
    (a.out/'profile-controls.json').write_text(json.dumps(report,indent=2)+'\n')
    for descriptor in capture['reads']:
        sid=descriptor['snapshot_id']
        if any(r['snapshot_id']==sid for r in report['reads']):continue
        data=captured_input(capture,sid)
        row={'snapshot_id':sid,'descriptor':descriptor,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'arms':[]}
        report['reads'].append(row)
        states=[json.loads(line) for line in data.splitlines() if line and json.loads(line).get('kind')=='state']
        assert states and (states[-1]['turn'],states[-1].get('frame'))==(first['decision']['turn'],first['decision'].get('frame'))
        for arm in ['off','on','off-repeat']:
            d=a.out/f'read-{sid}'/arm;d.mkdir(parents=True);(d/'events.jsonl').write_bytes(data)
            turn=first['decision']['turn']
            cmd=[q['runtime']['binary'],'--explain','--serve','--fresh-board','--mirror',str(d),'--victory',lane,'--turn',str(turn),*args]
            if arm=='on':cmd+=['--with',observer.GENE]
            done=subprocess.run(cmd,input=str(turn)+'\n',capture_output=True,text=True,timeout=300)
            (d/'stdout').write_text(done.stdout);(d/'stderr').write_text(done.stderr)
            played=decider_genome(d/'stderr')
            value={'arm':arm,'argv':cmd,'exit':done.returncode,'genome':played}
            row['arms'].append(value)
            if done.returncode==0:value['payload']=json.loads(done.stdout)
            (a.out/'profile-controls.json').write_text(json.dumps(report,indent=2)+'\n')
            assert done.returncode==0 and played is not None,value
        off,on,repeat=row['arms']
        validate_genomes(q['played_genome'],off['genome'],on['genome'],repeat['genome'],observer.GENE)
        assert all(v['payload']['orders']==first['orders'] for v in row['arms'])
        row['complete']=True
    assert report['reads']
    report['complete']=True
    (a.out/'profile-controls.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'complete':True,'distinct_reads':len(report['reads']),'calls':len(capture['reads']),'binary_sha256':q['runtime']['binary_sha256']}))

if __name__=='__main__':main()
