"""Reconstruct every captured read of the six registered earliest blocked frames."""
import argparse
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import sys


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--control',type=Path,required=True)
    parser.add_argument('--census',type=Path,required=True)
    parser.add_argument('--tools',type=Path,required=True)
    parser.add_argument('--capture-map',type=Path,
                        help='JSON map of original to restored absolute capture directories')
    parser.add_argument('--out',type=Path,required=True)
    args=parser.parse_args()
    if args.out.exists():
        raise ValueError('preserve previous evidence')
    args.out.mkdir()
    sys.path.insert(0,str(args.tools))
    from civ6_decision_trace import captured_input
    relocation=json.loads(args.capture_map.read_text()) if args.capture_map else {}
    if not isinstance(relocation,dict) or any(not isinstance(k,str) or not isinstance(v,str)
            or not Path(k).is_absolute() or not Path(v).is_absolute() for k,v in relocation.items()):
        raise ValueError('capture-map must map absolute directory strings')
    source=Path(__file__).with_name('2026-10-26-forward-airfield-preparation.py')
    spec=importlib.util.spec_from_file_location('preparation',source)
    audit=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(audit)
    cases=[]
    registered=json.loads(gzip.decompress(args.census.read_bytes()))
    if len(registered['witness_frames'])!=6:
        raise ValueError('registered six-frame witness set changed')
    for selected in registered['witness_frames']:
        if not (selected['tag'].startswith('civvis-20261025T') and selected['tag'].endswith('Z')):
            raise ValueError('unexpected fixed cohort tag')
        tag=selected['tag'][len('civvis-20261025T'):-1]
        turn,frame=selected['turn'],selected['frame']
        run=args.control/('civvis-20261025T'+tag+'Z')
        events=[json.loads(line) for line in (run/'events.jsonl').read_text().splitlines()]
        decisions=[json.loads(line) for line in (run/'decisions.jsonl').read_text().splitlines()]
        matches=[d for d in decisions if (d['decision']['turn'],d['decision']['frame'])==(turn,frame)]
        if len(matches)!=1:
            raise ValueError('missing or ambiguous actual decision')
        decision=matches[0]
        capture=decision['input_capture']
        if not capture['complete'] or not capture['reads']:
            raise ValueError('incomplete captured input')
        resolved=dict(capture,directory=relocation.get(capture['directory'],capture['directory']))
        states=[(i,e) for i,e in enumerate(events) if e.get('kind')=='state' and (e['turn'],e['frame'])==(turn,frame)]
        if len(states)!=1:
            raise ValueError('missing or ambiguous actual state')
        index,state=states[0]
        reads=[]
        game=next(game for games in registered['cohorts'].values() for game in games if game['tag']==run.name)
        rows=[row for row in game['capital_turns'] if (row['turn'],row['frame'])==(turn,frame)]
        if not rows:
            raise ValueError('registered witness has no retained capital-turn')
        for read in capture['reads']:
            data=captured_input(resolved,read['snapshot_id'])
            observed=[json.loads(line) for line in data.splitlines()]
            actual=[e for e in observed if e.get('kind')=='state'][-1]
            if actual!=state:
                raise ValueError('captured final state differs from actual state')
            width=[e['width'] for e in observed if e.get('kind')=='tiles'][-1]
            for row in rows:
                # Geometry already belongs to the terrain census; compare every
                # budget, prerequisite, queue and actual offer without inventing it.
                derived=audit.describe(actual,dict(row,sites={}),width)
                for city in derived['cities']:
                    city.pop('nearest_candidate_distance')
                expected=json.loads(json.dumps(row))
                for city in expected['cities']:
                    city.pop('nearest_candidate_distance')
                expected['immediate_neutral_site_count']=0
                expected['immediate_owned_site_count']=0
                if derived!=expected:
                    raise ValueError('captured support-chain fields differ from census')
            reads.append(dict(descriptor=read,bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),
                              last_state_matches=True,all_support_chain_fields_match=True))
        following=[(i,e) for i,e in enumerate(events) if i>index and e.get('kind')=='state']
        next_index,next_state=following[0]
        cases.append(dict(run=run.name,turn=turn,frame=frame,decision=decision,capture_reads=reads,
                          before_state=state,after_state=next_state,
                          intervening_events=events[index+1:next_index]))
    result=dict(scope=__doc__,cases=cases,script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                census_sha256=hashlib.sha256(args.census.read_bytes()).hexdigest(),
                audit_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest())
    (args.out/'witnesses.json.gz').write_bytes(gzip.compress(json.dumps(result,sort_keys=True).encode(),mtime=0))
    print(json.dumps([dict(run=c['run'],turn=c['turn'],frame=c['frame'],reads=len(c['capture_reads']),
                           read_calls=sum(r['descriptor']['count'] for r in c['capture_reads'])) for c in cases]))


if __name__=='__main__':
    main()
