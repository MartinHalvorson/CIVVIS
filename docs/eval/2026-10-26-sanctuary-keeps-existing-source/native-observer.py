"""Observe delivery and the first eight native starts; never control a game."""
import argparse, datetime, gzip, hashlib, json, subprocess, time
from pathlib import Path

parser=argparse.ArgumentParser()
parser.add_argument('--merged-commit',required=True)
parser.add_argument('--merged-at',required=True)
parser.add_argument('--repo',type=Path,default=Path('/Users/martbot-mbp-m5-max-128/civvis-main-managed'))
parser.add_argument('--runs-root',type=Path,default=Path('/Users/martbot-mbp-m5-max-128/civvis-civ6-runs'))
parser.add_argument('--out',type=Path,required=True)
args=parser.parse_args()
REPO=args.repo
ROOT=args.runs_root
OUT=args.out
OUT.mkdir(parents=True,exist_ok=False)
state={'overlap_note':'Attempt membership may overlap earlier income-selector, industrial-guard, Amenity-recovery and Aqueduct-recovery cohorts. Retain source revisions and never count overlapping games as independent replication or paired treatment.','protocol':'First eight native attempt stems starting at first confirmed merged-correction delivery. Include failures, continuations, missing outcomes and source changes; no replacements, game input or setup changes. Observational evidence, not a causal native win-rate trial.','merged_commit':args.merged_commit,'merged_at':args.merged_at,'started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'first_confirmed_delivery':None,'pre_delivery_attempts':[],'attempts':[]}
receipt=OUT/'observer.json'
cutoff='civvis-'+datetime.datetime.fromisoformat(args.merged_at.replace('Z','+00:00')).strftime('%Y%m%dT%H%M%SZ')
source_cache={}

def digest(data):return hashlib.sha256(data).hexdigest()
def save():
    tmp=receipt.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(receipt)

def inspect_source(run):
    p=ROOT/'decider-snapshots'/run/'civvis_orders.provenance.json'
    binary=p.with_name('civvis_orders')
    if not p.exists() or not binary.exists():return {'status':'missing frozen binary/provenance; retain attempt'}
    data=p.read_bytes();prov=json.loads(data);rev=prov.get('revision','')
    if rev not in source_cache:
        ancestor=subprocess.run(['git','merge-base','--is-ancestor',args.merged_commit,rev],cwd=REPO,capture_output=True)
        if ancestor.returncode not in (0,1):return {'status':'unresolved revision; await ordinary source refresh','provenance':prov}
        source_cache[rev]=ancestor.returncode==0
    actual=digest(binary.read_bytes());matched=actual==prov.get('sha256')
    result={'status':'verified' if matched else 'binary/provenance hash mismatch','provenance':prov,'provenance_sha256':digest(data),'actual_frozen_binary_sha256':actual,'binary_hash_matches':matched,'contains_merged_commit':source_cache[rev]}
    if source_cache[rev] and matched:
        module='src/ai/advanced/adopted_faith_sanctuary.rs'
        actual_function=subprocess.check_output(['git','show',rev+':'+module],cwd=REPO)
        expected_function=subprocess.check_output(['git','show',args.merged_commit+':'+module],cwd=REPO)
        decision_path=ROOT/'control'/run/'decisions.jsonl'
        first=None
        if decision_path.exists():
            with decision_path.open() as stream:
                line=stream.readline()
                if line.strip():first=json.loads(line)
        executing=bool(first and first.get('binary_on_disk_sha256')==actual and first.get('binary')==str(binary))
        result.update(sanctuary_module_sha256=digest(actual_function),matches_merged_sanctuary_module=actual_function==expected_function,first_decision_sha256=(first or {}).get('sha256'),first_executing_decision_matches=executing)
        result['correction_readback']=actual_function==expected_function and executing
    return result


def archive_terminal(run,row):
    summary=ROOT/'control'/run/'summary.json'
    if not summary.exists():return
    if row.get('archive_complete'):return
    directory=OUT/run;directory.mkdir(exist_ok=True)
    captured={}
    for name in ['summary.json','events.jsonl','decisions.jsonl','why.log','runtime_updates.jsonl','brain.log','stdout.log']:
        src=summary.with_name(name)
        if not src.exists():captured[name]={'status':'missing'};continue
        before=src.stat();data=src.read_bytes();after=src.stat()
        if (before.st_size,before.st_mtime_ns)!=(after.st_size,after.st_mtime_ns):
            row['archive_status']='source changing after summary; retry same attempt, no replacement';return
        dest=directory/(name+'.gz')
        with gzip.GzipFile(filename=str(dest),mode='wb',mtime=0) as f:f.write(data)
        assert gzip.decompress(dest.read_bytes())==data
        captured[name]={'bytes':len(data),'sha256':digest(data),'archive_sha256':digest(dest.read_bytes())}
    row.update(summary=json.loads(summary.read_text()),files=captured,archive_complete=True,archive_status='complete exact byte snapshots retained')

save()
print('Read-only native observer starts; waiting for corrected frozen binary.',flush=True)
while True:
    runs=sorted(p.name for p in (ROOT/'control').glob('civvis-*') if p.is_dir() and p.name>=cutoff)
    if state['first_confirmed_delivery'] is None:
        state['pre_delivery_attempts']=[]
        for run in runs:
            source=inspect_source(run)
            if source.get('correction_readback'):
                state['first_confirmed_delivery']={'run':run,'observed_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'source':source}
                print('First corrected-source attempt',run,flush=True);break
            state['pre_delivery_attempts'].append({'run':run,'source':source})
    anchor=state['first_confirmed_delivery']
    if anchor:
        selected=[run for run in runs if run>=anchor['run']][:8]
        previous={row['run']:row for row in state['attempts']}
        assert all(row['run'] in selected for row in state['attempts']), 'Never replace an observed cohort member'
        rows=[]
        for run in selected:
            row=previous.get(run,{'run':run,'first_observed_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()})
            if row.get('source',{}).get('status')!='verified':row['source']=inspect_source(run)
            archive_terminal(run,row);rows.append(row)
        state['attempts']=rows
        state['selected_attempts']=len(rows)
        state['archived_terminal_segments']=sum(bool(row.get('archive_complete')) for row in rows)
        if len(rows)==8 and all(row.get('archive_complete') for row in rows):
            state['status']='all first eight attempt segments retained; inspect true outcomes and source/settings before interpretation';save();print(state['status'],flush=True);break
    save();time.sleep(15)
