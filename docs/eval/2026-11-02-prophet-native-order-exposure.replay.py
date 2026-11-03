"""Compare host orders until their first divergence on a qualified native prefix.

The variants allow decision metadata to differ, while preserving frame identity
and every ordered input-read byte. They stop before replaying a changed world.
"""
from pathlib import Path
import argparse,hashlib,json,sys,datetime
parser=argparse.ArgumentParser(description=__doc__)
for name in ('repo','qualification','original','before','intent','out'):
 parser.add_argument('--'+name,type=Path,required=True)
parser.add_argument('--delivery',choices=('original','last-snapshot'),default='original',help='Experimental last-snapshot mode retains exact gates but may still hit delivery deadlines')
args=parser.parse_args()
repo=args.repo.resolve()
sys.path.insert(0,str(repo/'tools'))
import civ6_captured_read_replay as replay
from civ6_ladder import decider_genome
assert replay.sha((repo/'tools/civ6_captured_read_replay.py').read_bytes())=='f0cb7f2b84173dca2115514b35e12a55acea7cffe303c3d0ba22f41d6c628e7e', 'Use the archived replay driver or the matching repository revision'
import array,errno,fcntl,os,select,termios,time

class RequestBoundedFifo(replay.FifoReads):
    def _run(self):
        try:
            for number, data in enumerate(self.reads):
                self.read_number = number
                self.phase = "opening"
                self.bytes_sent = 0
                descriptor, observer = None, None
                try:
                    while descriptor is None:
                        self._check()
                        try:
                            descriptor = os.open(self.path, os.O_WRONLY | os.O_NONBLOCK)
                        except OSError as error:
                            if error.errno != errno.ENXIO:
                                raise
                            self.cancelled.wait(0.002)
                    observer = os.open(self.path, os.O_RDONLY | os.O_NONBLOCK)
                    following = self.path.with_name(f".next-read-{number}")
                    if number + 1 == len(self.reads):
                        # Quiescent readers must not open an unserved FIFO.
                        # Any additional captured read still fails the exact gate.
                        following.write_bytes(data)
                    else:
                        os.mkfifo(following)
                    os.replace(following, self.path)
                    self.phase = "writing"
                    remaining = memoryview(data)
                    while remaining:
                        self._check()
                        _, ready, _ = select.select([], [descriptor], [], 0.05)
                        if ready:
                            try:
                                sent = os.write(descriptor, remaining)
                                self.bytes_sent += sent
                                remaining = remaining[sent:]
                            except BlockingIOError:
                                continue
                    self.phase = "draining"
                    queued = array.array("i", [0])
                    while True:
                        self._check()
                        fcntl.ioctl(observer, termios.FIONREAD, queued, True)
                        if queued[0] == 0:
                            break
                        self.cancelled.wait(0.001)
                    self.served += 1
                    self.phase = "closing"
                finally:
                    if observer is not None:
                        os.close(observer)
                    if descriptor is not None:
                        os.close(descriptor)
        except Exception as error:
            self.error = f"{type(error).__name__}: {error}"
        finally:
            self.done.set()

if args.delivery=='last-snapshot':replay.FifoReads=RequestBoundedFifo

q=args.qualification.resolve();proof=json.loads((q/'qualification.json').read_text());assert proof['passed'] and len(proof['controls'])==2
assert all(c['passed'] for c in proof['controls']) and proof['through_turn']==60
raw=(q/'native-decisions.jsonl').read_bytes();assert replay.sha(raw)==proof['native_decisions_sha256'];records=[json.loads(x) for x in raw.splitlines()];by_key={tuple(replay.frame_key(r)):r for r in records}
for record in records:
 assert all(set(order)<= {'kind','subject','verb','x','y'} for order in record['orders']), 'Unknown host-order fields'
context=json.loads((q/'observed-context.json').read_text());base=context['processes'][0]['argv'];new=args.original.resolve();old=args.before.resolve()
assert replay.sha(new.read_bytes())==context['binary_sha256']=='3a26064884da5f21285c279a3e8105b05fa99c6e880376f6849c24fb99514d6d'
assert replay.sha(old.read_bytes())=='b9d62c14b78aedb46d5d8461b1392e2c19b6b3d371c0c4650310187414cba205'
flag='religion-lane-hands-over';index=base.index(flag);assert base[index-1]=='--with' and base.count(flag)==1
out=args.out.resolve();out.mkdir(parents=True,exist_ok=False)
(out/'intent.json').write_bytes(args.intent.read_bytes())
mapping={}
for capture in proof['sealed_captures']:
 local=q/Path(capture['sealed_directory']).name
 directory=local if local.is_dir() else Path(capture['sealed_directory'])
 archive=(directory/'bytes.bin').read_bytes()
 assert len(archive)==capture['archive_bytes'] and replay.sha(archive)==capture['archive_sha256']
 assert replay.sha((directory/'snapshots.jsonl').read_bytes())==capture['index_sha256']
 mapping[capture['source']]=directory
source_genome=decider_genome(q/'control-0/stderr.log');assert source_genome and flag in source_genome['treatments']
# Qualify the revised delivery against the actual full native payload first.
full_controls=[]
if args.delivery=='last-snapshot':
 for number in range(2):
  command=base.copy();command[0]=str(new)
  original_variant={'binary_sha256':context['binary_sha256'],'provenance':{'sha256':context['binary_sha256']},'processes':[{'argv':command,'cwd':context['processes'][0]['cwd']}]}
  control=replay.run_control(original_variant,records,mapping,out/f'delivery-control-{number}',60)
  full_controls.append(control)
  proof_report={'delivery_mode':args.delivery,'scope':'Original executable, full native payload, identity and exact ordered read bytes with a regular last snapshot between requests.','passed':len(full_controls)==2 and all(c['passed'] for c in full_controls),'controls':full_controls}
  (out/'delivery-qualification.json').write_text(json.dumps(proof_report,indent=2)+'\n')
  print(json.dumps({'delivery_control':number,'passed':control['passed'],'requests':len(control['requests']),'error':control.get('error')}),flush=True)
  if not control['passed']:raise SystemExit(2)


# This changes ONLY the compared payload for these counterfactual runs. The
# original full-payload qualifications above stay separate and unmodified;
# run_control still enforces exact frame identities and ordered read bytes.
replay.control_payload=lambda record: {'orders':record['orders']}
results=[]
for label,binary,enabled in [('original-on-orders-control',new,True),('old-on-0',old,True),('old-on-1',old,True),('new-off',new,False)]:
 command=base.copy();command[0]=str(binary)
 if not enabled:del command[index-1:index+1]
 digest=replay.sha(binary.read_bytes());variant={'binary_sha256':digest,'provenance':{'sha256':digest,'source':'shadow variant, not a second observed host process'},'processes':[{'argv':command,'cwd':context['processes'][0]['cwd']}]}
 r=replay.run_control(variant,records,mapping,out/label,60)
 for request in r['requests']:request['orders_match']=request.pop('native_payload_matches')
 r['all_orders_match_through_endpoint']=r.pop('passed');summary={'variant':label,'all_orders_match_through_endpoint':r['all_orders_match_through_endpoint'],'requests':len(r['requests']),'stopped_request':r.get('failed_request'),'driver_error':r.get('error'),'comparison_scope':'Host orders only; non-actuation decision metadata may differ.'}
 g=decider_genome(out/label/'stderr.log');expected=sorted(t for t in source_genome['treatments'] if enabled or t!=flag);profile_matches=bool(g) and sorted(g['treatments'])==expected
 summary['realized_profile_matches']=profile_matches
 comparisons=r['requests'];valid_reads=all(not c['identity_errors'] and c['read_sequence_matches'] for c in comparisons)
 expected_stop=bool(r.get('failed_request') and comparisons and not comparisons[-1]['orders_match'] and all(c['orders_match'] for c in comparisons[:-1]) and valid_reads and (r.get('error') or '').startswith('ValueError: original-binary control differs'))
 summary['stopped_at_first_different_order']=expected_stop
 summary['valid_order_prefix_comparison']=profile_matches and valid_reads and (r['all_orders_match_through_endpoint'] or expected_stop)
 if expected_stop:
  key=tuple(r['failed_request']);native=by_key[key];reply=json.loads((out/label/'replies.jsonl').read_bytes().splitlines()[-1]);summary.update({'observed_primary':native['decision']['victory_portfolio']['primary'],'variant_primary':reply['decision']['victory_portfolio']['primary'],'observed_orders':native['orders'],'variant_orders':reply['orders']})
 results.append({'summary':summary,'driver_result':r,'realized_genome':g})
 report={'delivery_mode':args.delivery,'scope':'Qualified captured-world first action divergence, not full-payload equality of variants, native counterfactual execution, saved yields or a strength estimate.','native_full_payload_qualification_sha256':replay.sha((q/'qualification.json').read_bytes()),'native_decisions_sha256':proof['native_decisions_sha256'],'original_context_sha256':replay.sha((q/'observed-context.json').read_bytes()),'comparison_driver_source_sha256':replay.sha((repo/'tools/civ6_captured_read_replay.py').read_bytes()),'results':results,'recorded_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
 (out/'results.json').write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps({k:v for k,v in summary.items() if k not in ['observed_orders','variant_orders']}),flush=True)
 if not summary['valid_order_prefix_comparison']:raise SystemExit(2)
