"""Verify a self-contained offline proposal matrix; establish no native effect."""
from pathlib import Path
import sys,json,hashlib,tarfile
sha=lambda b:hashlib.sha256(b).hexdigest()
with tarfile.open(sys.argv[1],'r:*') as tar:
 files={m.name:tar.extractfile(m).read() for m in tar.getmembers() if m.isfile()}
j=lambda name:json.loads(files[name])
manifest=j('manifest.json')
assert set(files)==set(manifest['files'])|{'manifest.json'}
assert all(sha(files[name])==digest for name,digest in manifest['files'].items())
intent=j('evidence/intent.json');build=j('evidence/build-process.json');tag=manifest['tag']
assert build['exit']==0 and build['source_unchanged']
assert intent['binary_sha256']==build['binary_sha256']
assert intent['provenance']['revision']==build['source_revision']
assert intent['provenance']['sha256']==build['binary_sha256']
assert len(intent['cases'])==3 and intent['arm_order']==['off','on','on','off']
metas=j('capture-archives.json');assert len(metas)==12
results=[];total=0
for meta in metas:
 result=j(meta['result']);payload=result['payload'];cap=payload['input_capture']
 assert result['exit']==0 and result['binary_sha256']==build['binary_sha256']
 assert payload['decision']['turn']==result['turn'] and payload['decision']['frame']==0
 assert payload['decision']['victory_portfolio']['assigned_target']=='science'
 assert cap['complete'] is True and cap['reads'] and cap['error'] is None
 blob=files[meta['blob']];assert sha(blob)==result['shared_input_sha256']
 assert len(blob)==result['shared_input_bytes'] and sha(blob)==meta['blob_sha256']
 index=files[meta['index']];assert sha(index)==meta['index_sha256']
 rows=[json.loads(line) for line in index.splitlines()];offset=0
 for number,row in enumerate(rows):
  assert row['id']==number and row['offset']==offset
  assert row['parent'] is None or 0<=row['parent']<number
  before=0 if row['parent'] is None else rows[row['parent']]['total_bytes']
  assert row['total_bytes']==before+row['append_bytes']
  offset+=row['append_bytes']
 assert offset==len(blob)
 actual=[]
 for read in cap['reads']:
  assert type(read['count']) is int and read['count']>0
  ident=read['snapshot_id'];assert 0<=ident<len(rows)
  chain=[]
  while ident is not None:
   row=rows[ident];chain.append(row);ident=row['parent']
  data=b''.join(blob[r['offset']:r['offset']+r['append_bytes']] for r in reversed(chain))
  assert data==files[meta['input']]
  actual.extend([sha(data)]*read['count'])
 assert len(actual)==result['actual_read_count'] and actual==result['actual_read_sha256']
 total+=len(actual);results.append(result)
def control(result):return {k:result['payload'][k] for k in ['orders','decision']}
def argv(result):
 args=list(result['argv']);pos=args.index('--mirror');args[pos+1]='<fixed-input>'
 if result['arm']=='on':
  at=[i for i in range(len(args)-1) if args[i:i+2]==['--with',tag]];assert len(at)==1
  del args[at[0]:at[0]+2]
 return args
changed={}
for case in intent['cases']:
 turn=case['turn'];rs=[r for r in results if r['turn']==turn];assert len(rs)==4
 off=[r for r in rs if r['arm']=='off'];on=[r for r in rs if r['arm']=='on']
 assert len(off)==len(on)==2
 assert control(off[0])==control(off[1]) and control(on[0])==control(on[1])
 assert argv(off[0])==argv(on[0])
 a,b=[dict(r['genome']) for r in [off[0],on[0]]]
 for field in ['treatments','forced']:
  before,after=a.pop(field),b.pop(field)
  assert len(before)==len(set(before)) and len(after)==len(set(after))
  assert tag not in before and tag in after and set(after)-{tag}==set(before)
 assert a==b
 changed[str(turn)]=off[0]['payload']['orders']!=on[0]['payload']['orders']
for name in manifest['pairs']:
 pair=j(name);turn=pair['request'][0];assert pair['complete'] and pair['request']==[turn,0]
 assert pair['repeat'] in [1,2] and len(pair['reads'])==1
 read=pair['reads'][0];case=next(c for c in intent['cases'] if c['turn']==turn)
 assert read['complete'] and read['sha256']==case['input_sha256'] and read['bytes']==case['input_bytes']
 assert len(read['arms'])==2
 rounds={'off':0,'on':1} if pair['repeat']==1 else {'off':3,'on':2}
 for arm in read['arms']:
  result=next(r for r in results if r['turn']==turn and r['round']==rounds[arm['arm']])
  assert arm=={key:result[key] for key in ['arm','exit','argv','genome','payload','binary_sha256']}
assert changed==manifest['changed_cases']
assert total==manifest['registered_reads']
print(json.dumps({'queries':len(results),'verified_registered_reads':total,'changed_cases':changed,'scope':'Same-input proposal firing only; no original private history, native actuation, city survival or strength claim.'}))
