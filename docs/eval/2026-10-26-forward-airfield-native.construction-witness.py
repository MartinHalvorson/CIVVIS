"""Verify the observed candidate support-stage requests against exact Native states."""
import argparse
import hashlib
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    events = [json.loads(line) for line in (args.run/'events.jsonl').read_text().splitlines()]
    decisions = [json.loads(line) for line in (args.run/'decisions.jsonl').read_text().splitlines()]
    journal = (args.run/'why.log').read_text().splitlines()
    witnesses = []
    for turn, verb, complete_turn in [(138,'DISTRICT_ENCAMPMENT',140),(140,'BUILDING_BARRACKS',141)]:
        matching = [d for d in decisions if d['decision']['turn']==turn and d['decision'].get('frame')==0]
        assert len(matching)==1
        decision = matching[0]
        orders = [a for a in decision['orders'] if a.get('kind')=='produce' and a.get('subject')==655369 and a.get('verb')==verb]
        assert len(orders)==1
        states = [e for e in events if e.get('kind')=='state' and e.get('turn')==turn and e.get('frame')==0]
        assert len(states)==1
        pre = [city for city in states[0]['cities'] if city.get('id')==655369 and (city.get('x'),city.get('y'))==(40,30)]
        assert len(pre)==1
        def completed(city):
            if verb=='DISTRICT_ENCAMPMENT':
                return any(d.get('type')==verb and d.get('complete') is True and (d.get('x'),d.get('y'))==(38,32) for d in city.get('districts',[]))
            return verb in city.get('buildings',[])
        assert not completed(pre[0])
        later = []
        for event in events:
            if event.get('kind')!='state' or event.get('turn',-1)<turn:
                continue
            for city in event.get('cities',[]):
                if (city.get('x'),city.get('y'))==(40,30) and completed(city):
                    later.append((event,city))
        assert later and later[0][0]['turn']==complete_turn
        post_event, post_city = later[0]
        receipts = [e for e in events if e.get('kind')=='order_verified' and e.get('order_kind')=='produce' and e.get('turn')==turn and e.get('subject')==655369 and e.get('verb')==verb]
        assert len(receipts)==1
        why = [line for line in journal if line.startswith(f'[why] t{turn} ') and 'Beijing prepares a forward airfield' in line and 'forward-airfield-project:' in line]
        assert why
        witnesses.append(dict(turn=turn,frame=0,order=orders[0],journal=why,pre_city=pre[0],
                              verified_order=receipts[0],first_completed_state={k:post_event.get(k) for k in ['turn','frame','utc']},post_city=post_city,
                              input_capture=decision['input_capture'],binary_sha256=decision.get('binary_on_disk_sha256')))
    out = dict(run=args.run.name,witnesses=witnesses,
               scope='Observed support-stage requests and later host completion; no Engineer, Airstrip or incremental strength attribution.',
               files={name:dict(bytes=(args.run/name).stat().st_size,sha256=hashlib.sha256((args.run/name).read_bytes()).hexdigest()) for name in ['events.jsonl','decisions.jsonl','why.log','summary.json','runtime_updates.jsonl']})
    args.out.write_text(json.dumps(out,indent=2)+'\n')
    print('Verified',len(witnesses),'Native stage completions')


if __name__=='__main__':
    main()
