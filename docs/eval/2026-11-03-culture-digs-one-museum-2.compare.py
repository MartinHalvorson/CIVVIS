"""Compare disposable fresh-board proposals on exact captured native reads."""
import argparse,datetime,hashlib,json,subprocess,sys
from pathlib import Path

def sha(raw):return hashlib.sha256(raw).hexdigest()
def main():
 parser=argparse.ArgumentParser(description=__doc__)
 for name in ['repo','root','before','candidate','out']:parser.add_argument('--'+name,type=Path,required=True)
 args=parser.parse_args();args.out.mkdir(exist_ok=False,parents=True);sys.path.insert(0,str(args.repo/'tools'))
 from civ6_decision_trace import captured_input
 from civ6_ladder import decider_genome
 run=args.root/'civvis-20261103T002240Z';source_genome=decider_genome(run/'why.log');profile=sorted(source_genome['forced']);tag='culture-digs-one-museum-2';original='culture-digs-one-museum';assert tag not in profile and original not in profile
 summary=json.loads((run/'summary.json').read_text());decisions=[json.loads(x) for x in (run/'decisions.jsonl').read_bytes().splitlines()];results=[]
 evidence={'scope':'Fresh-board proposal comparisons only. No persistent native replay, host execution, actual artifact/Leakey activation or win-rate estimate.','source_run':run.name,'source_outcome':summary['outcome'],'created_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'binary_sha256':{name:sha(path.read_bytes()) for name,path in [('before',args.before),('candidate',args.candidate)]},'cases':results}
 for turn in [150,153,160,175]:
  row=next(r for r in decisions if (r['decision']['turn'],r['decision']['frame'])==(turn,0));capture=row['input_capture'];assert capture['complete'];selected=None
  for read in capture['reads']:
   raw=captured_input(capture,read['snapshot_id']);states=[json.loads(x) for x in raw.splitlines() if json.loads(x).get('kind')=='state']
   if states and (states[-1]['turn'],states[-1].get('frame',0))==(turn,0):selected=(raw,read['snapshot_id']);break
  assert selected;raw,snapshot=selected;case_dir=args.out/f't{turn}';case_dir.mkdir();(case_dir/'captured-events.jsonl').write_bytes(raw)
  case={'turn':turn,'complete':False,'snapshot_id':snapshot,'source_decision_sha256':row['sha256'],'reads':[{'complete':True,'sha256':sha(raw),'bytes':len(raw),'arms':[]}],'controls':[]};results.append(case);by_label={}
  for label,binary,flag in [('before-off',args.before,None),('before-v1',args.before,original),('candidate-off',args.candidate,None),('candidate-v1',args.candidate,original),('candidate-on',args.candidate,tag),('candidate-repeat',args.candidate,tag)]:
   directory=case_dir/label;directory.mkdir();(directory/'events.jsonl').write_bytes(raw);argv=[str(binary.resolve()),'--mirror',str(directory.resolve()),'--turn',str(turn),'--victory','culture','--civ',summary['seat']['civ'],'--fresh-board','--explain']
   for gene in sorted(profile+([flag] if flag else [])):argv+=['--with',gene]
   process=subprocess.run(argv,cwd=args.repo,capture_output=True,timeout=60);(directory/'stdout').write_bytes(process.stdout);(directory/'stderr').write_bytes(process.stderr)
   response={'label':label,'argv':argv,'exit':process.returncode,'genome':decider_genome(directory/'stderr')};case['controls'].append(response);assert process.returncode==0,(turn,label,process.stderr[-1000:]);response['payload']=json.loads(process.stdout);observed=response['genome'];assert observed and (tag in observed['forced'])==(flag==tag) and (original in observed['forced'])==(flag==original);assert sorted(observed['forced'])==sorted(profile+([flag] if flag else []));assert sorted(observed['treatments'])==sorted(source_genome['treatments']+([flag] if flag else []));by_label[label]=response;(directory/'events.jsonl').unlink()
   (args.out/'results.json').write_text(json.dumps(evidence,indent=2)+'\n')
  for arm,label in [('off','candidate-off'),('on','candidate-on')]:case['reads'][0]['arms'].append({'arm':arm,**{k:v for k,v in by_label[label].items() if k!='label'}})
  for first,second in [('before-off','candidate-off'),('before-v1','candidate-v1'),('candidate-on','candidate-repeat')]:assert by_label[first]['payload']['orders']==by_label[second]['payload']['orders'],(turn,first,second,'orders differ')
  before=by_label['candidate-off']['payload']['orders'];after=by_label['candidate-on']['payload']['orders'];case['removed_orders']=[o for o in before if o not in after];case['added_orders']=[o for o in after if o not in before];case['changed']=before!=after;case['complete']=True
  (case_dir/'result.json').write_text(json.dumps(case,indent=2)+'\n');(args.out/'results.json').write_text(json.dumps(evidence,indent=2)+'\n');print(json.dumps({k:case[k] for k in ['turn','changed','added_orders','removed_orders']}),flush=True)
 assert any(c['changed'] for c in results),'candidate has no demonstrated activation'
if __name__=='__main__':main()
