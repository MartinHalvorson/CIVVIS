"""Reconstruct every captured read for four selected explanatory frames."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import sys


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--control',type=Path,required=True)
    parser.add_argument('--tools',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    args=parser.parse_args()
    if args.out.exists():
        raise ValueError('preserve previous evidence')
    args.out.mkdir()
    sys.path.insert(0,str(args.tools))
    from civ6_decision_trace import captured_input
    cases=[]
    for tag,turn,frame in [('134959',156,3),('134959',157,0),('134959',160,0),('124842',195,0)]:
        run=args.control/('civvis-20261025T'+tag+'Z')
        events=[json.loads(line) for line in (run/'events.jsonl').read_text().splitlines()]
        decisions=[json.loads(line) for line in (run/'decisions.jsonl').read_text().splitlines()]
        matches=[d for d in decisions if (d['decision']['turn'],d['decision']['frame'])==(turn,frame)]
        assert len(matches)==1
        decision=matches[0]
        capture=decision['input_capture']
        assert capture['complete'] and capture['reads']
        states=[(i,e) for i,e in enumerate(events) if e.get('kind')=='state' and (e['turn'],e['frame'])==(turn,frame)]
        assert len(states)==1
        index,state=states[0]
        reads=[]
        for read in capture['reads']:
            data=captured_input(capture,read['snapshot_id'])
            observed=[json.loads(line) for line in data.splitlines()]
            actual=[e for e in observed if e.get('kind')=='state'][-1]
            assert actual==state
            name=f'{tag}-t{turn}-f{frame}-read{read["snapshot_id"]}.jsonl.gz'
            (args.out/name).write_bytes(gzip.compress(data,mtime=0))
            reads.append(dict(descriptor=read,bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),file=name))
        following=[(i,e) for i,e in enumerate(events) if i>index and e.get('kind')=='state']
        next_index,next_state=following[0]
        cases.append(dict(run=run.name,turn=turn,frame=frame,decision=decision,capture_reads=reads,
                          before_state=state,after_state=next_state,
                          intervening_events=events[index+1:next_index]))
    result=dict(scope=__doc__,cases=cases,script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    (args.out/'witnesses.json.gz').write_bytes(gzip.compress(json.dumps(result,sort_keys=True).encode(),mtime=0))
    print(json.dumps([dict(run=c['run'],turn=c['turn'],frame=c['frame'],reads=len(c['capture_reads']),
                           read_calls=sum(r['descriptor']['count'] for r in c['capture_reads'])) for c in cases]))


if __name__=='__main__':
    main()
