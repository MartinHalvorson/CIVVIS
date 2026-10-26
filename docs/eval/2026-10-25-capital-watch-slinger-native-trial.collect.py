"""Read-only outcome/activation census of the registered prospective Slinger trial.

Upgrade requests are commands, not verified Native upgrades. An ON played
genome is not proof the policy activated. Live cross-file reads are non-atomic;
retain malformed/missing records rather than replace an assigned attempt.
"""
import argparse
import collections
import gzip
import hashlib
import json
import re
from pathlib import Path
import sys

GENE='capital-watch-arms-the-slinger'


def rows(path):
    if not path.exists():
        return [], 0
    result, malformed=[], 0
    for line in path.read_bytes().splitlines():
        try:
            result.append(json.loads(line))
        except ValueError:
            malformed+=1
    return result, malformed


def attempt(root, decider_genome):
    summary, bad_summary = None, 0
    if (root/'summary.json').exists():
        try:
            summary=json.loads((root/'summary.json').read_text())
        except ValueError:
            bad_summary=1
    events,bad_events=rows(root/'events.jsonl')
    decisions,bad_decisions=rows(root/'decisions.jsonl')
    runtime,bad_runtime=rows(root/'runtime_updates.jsonl')
    first={}
    states=[]
    for event in events:
        if event.get('kind')=='state':
            states.append(event)
            first.setdefault(event['turn'],event)
    played=decider_genome(root/'why.log') if (root/'why.log').exists() else None
    tags=played.get('treatments',[]) if played else None
    baseline=sorted(t for t in tags if t!=GENE) if tags is not None else None
    requests=[dict(turn=d['decision']['turn'],frame=d['decision'].get('frame'),action=a)
              for d in decisions for a in d.get('orders',[])
              if a.get('kind')=='unit' and a.get('verb')=='UPGRADE']
    return dict(tag=root.name,terminal_from_summary=summary.get('game_stopped') is True if summary else False,
        summary=summary,upgrade_execution_status="not_observed_by_this_collector",played_genome=played,played_candidate=(GENE in tags if tags is not None else None),
        baseline_without_candidate_sha256=hashlib.sha256(json.dumps(baseline,separators=(',',':')).encode()).hexdigest() if baseline is not None else None,
        runtime_updates=runtime,malformed_rows=dict(events=bad_events,decisions=bad_decisions,runtime=bad_runtime,summary=bad_summary),
        first_native_turn=min(first,default=None),last_native_turn=max(first,default=None),
        maximum_observed_bombers=max((sum(u.get('class')=='PROMOTION_CLASS_AIR_BOMBER' for u in s.get('units',[])) for s in states),default=0),
        feature_journal_lines=[line for line in (root/'why.log').read_text().splitlines()
                               if line.startswith('[why]') and GENE+':' in line] if (root/'why.log').exists() else [],
        issued_upgrade_orders=requests,native_event_counts=dict(collections.Counter(e.get('kind') for e in events)),
        files={p.name:dict(bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest())
               for p in (root/n for n in ['events.jsonl','decisions.jsonl','summary.json','why.log','runtime_updates.jsonl']) if p.exists()})


def validate_assignment(state):
    if state.get('gene') != GENE or state.get('limit') != 16:
        raise ValueError('wrong gene or registered cutoff')
    assigned=state['assigned']
    if len(assigned)>16 or len(set(assigned))!=len(assigned):
        raise ValueError('invalid assigned cohort')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--state',type=Path,required=True)
    parser.add_argument('--control',type=Path,required=True)
    parser.add_argument('--tools',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    args=parser.parse_args()
    if args.out.exists():
        raise ValueError('retain earlier collection; use a new output')
    sys.path.insert(0,str(args.tools))
    from civ6_ladder import decider_genome
    state=json.loads(args.state.read_text())
    validate_assignment(state)
    assigned=[]
    for stem in state['assigned']:
        pattern=re.compile(re.escape(stem)+r'(?:-cont(\d+))?$')
        paths=sorted((p for p in args.control.iterdir() if p.is_dir() and pattern.fullmatch(p.name)),
                     key=lambda p: int(pattern.fullmatch(p.name).group(1) or 0))
        assigned.append(dict(stem=stem,attempts=[attempt(p,decider_genome) for p in paths],missing_attempt_directory=not paths))
    result=dict(scope=__doc__,assignment_state=state,assignments=assigned,
                script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    args.out.write_bytes(gzip.compress(json.dumps(result,sort_keys=True).encode(),mtime=0))
    assert json.loads(gzip.decompress(args.out.read_bytes()))==result
    print(json.dumps([dict(stem=r['stem'],attempts=[dict(tag=a['tag'],terminal=a['terminal_from_summary'],
        played_candidate=a['played_candidate'],maximum_bombers=a['maximum_observed_bombers'],issued_upgrade_requests=len(a['issued_upgrade_orders'])) for a in r['attempts']]) for r in assigned]))


if __name__=='__main__':
    main()
