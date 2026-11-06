#!/usr/bin/env python3
"""Independently verify the archived fixed-input proposal evidence.

No game or controller is executed. This verifies recorded proposals and actual
captured reads; it does not attest host application or a played counterfactual.
"""
from pathlib import Path
import argparse,hashlib,json,tarfile

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('archive',type=Path)
args=parser.parse_args()
sha=lambda value:hashlib.sha256(value).hexdigest()
with tarfile.open(args.archive,'r:*') as tar:
 def data(name,seen=None):
  seen=set() if seen is None else seen
  assert name not in seen;seen.add(name)
  member=tar.getmember(name)
  if member.islnk():return data(member.linkname,seen)
  assert member.isfile()
  return tar.extractfile(member).read()
 def read(name):return json.loads(data(name))
 protocol=read('protocol.json');source=read('source.json')
 blob=data('input/final-read.jsonl')
 assert sha(blob)==protocol['input_sha256']
 assert len(blob)==protocol['input_bytes']
 assert sha(data('probe.py'))==source['probe_sha256']
 assert sha(data('protocol.json'))==source['protocol_sha256']
 states=[e for e in map(json.loads,blob.splitlines()) if e.get('kind')=='state']
 assert (states[-1]['turn'],states[-1].get('frame',0))==(40,1)
 roles=['OFF-0','OFF-1','ON-0','ON-1'];payloads={};genomes={};argvs={};counts={}
 for role in roles:
  reply=read(role+'/reply.json');invocation=read(role+'/invocation.json')
  assert read(role+'/process-footer.json')=={'complete':True,'exit':0}
  assert (reply['decision']['turn'],reply['decision']['frame'])==(40,1)
  cap=reply['input_capture'];assert cap['complete'] is True and cap['error'] is None
  assert cap['schema']==1 and cap['reads']
  archive=data(role+'/recaptured/bytes.bin');assert archive==blob
  indices=[json.loads(line) for line in data(role+'/recaptured/snapshots.jsonl').splitlines()]
  assert len(indices)==1
  assert indices[0]=={'id':0,'parent':None,'offset':0,'append_bytes':len(blob),'total_bytes':len(blob)}
  assert all(x['snapshot_id']==0 and isinstance(x['count'],int) and x['count']>0 for x in cap['reads'])
  count=sum(x['count'] for x in cap['reads']);counts[role]=count
  check=read(role+'/read-verification.json')
  assert check['complete'] and check['actual_reads']==count
  assert check['each_read_sha256']==sha(blob) and check['each_read_bytes']==len(blob)
  assert check['recorded_capture_directory']==cap['directory']
  payloads[role]={'decision':reply['decision'],'orders':reply['orders']}
  genomes[role]=read(role+'/genome.json');argvs[role]=invocation['argv']
  raw_genomes=[]
  for line in data(role+'/stderr.log').decode().splitlines()[:20]:
   if line.startswith('{'):
    record=json.loads(line)
    if record.get('kind')=='genome':raw_genomes.append(record)
  assert len(raw_genomes)==1
  assert {key:raw_genomes[0].get(key) for key in genomes[role]}==genomes[role]
  assert invocation['cwd']==source['worktree']
  assert '--serve' in argvs[role] and '--fresh-board' in argvs[role]
  assert argvs[role][argvs[role].index('--victory')+1]=='science'
  assert '--strategy' not in argvs[role]
  assert argvs[role][argvs[role].index('--civ')+1]=='CIVILIZATION_INDONESIA'
 for cold,role in [('cold-off','OFF-0'),('cold-on','ON-0')]:
  metadata=read(cold+'/metadata.json')
  assert metadata['exit']==0 and data(cold+'/stdout.log')==b''
  assert metadata['genome']==genomes[role]
  records=[json.loads(line) for line in data(cold+'/stderr.log').decode().splitlines()[:20] if line.startswith('{')]
  raw=next(x for x in records if x.get('kind')=='genome')
  assert {key:raw.get(key) for key in metadata['genome']}==metadata['genome']
 assert payloads['OFF-0']==payloads['OFF-1'] and payloads['ON-0']==payloads['ON-1']
 assert len(set(counts.values()))==1
 for n in [0,1]:
  pair=read('pair-'+str(n)+'.json');assert pair['complete'] and len(pair['reads'])==1
  capture=pair['reads'][0]
  assert capture['complete'] and capture['sha256']==sha(blob) and capture['bytes']==len(blob)
  arms={a['arm']:a for a in capture['arms']};assert set(arms)=={'off','on'}
  normalized=[];metadata=[]
  for arm,role in [('off','OFF-'+str(n)),('on','ON-'+str(n))]:
   a=arms[arm];assert a['exit']==0 and a['payload']==payloads[role]
   assert a['genome']==genomes[role] and a['argv']==argvs[role]
   words=list(a['argv']);words[words.index('--mirror')+1]='<mirror>'
   if arm=='on':
    i=next(i for i in range(len(words)-1) if words[i]=='--with' and words[i+1]==protocol['tag']);del words[i:i+2]
   else:assert protocol['tag'] not in words
   for tag in protocol['shared_forced_tags']:
    assert any(words[i:i+2]==['--with',tag] for i in range(len(words)-1))
   normalized.append(words)
   g=dict(a['genome'])
   for field in ['treatments','forced']:
    values=g.pop(field)
    if arm=='on':assert protocol['tag'] in values;values=[x for x in values if x!=protocol['tag']]
    else:assert protocol['tag'] not in values
    g[field]=sorted(values)
   metadata.append(g)
  assert normalized[0]==normalized[1] and metadata[0]==metadata[1]
 result=read('result.json')
 assert result['complete'] and result['repeated_arms_identical']
 changed=payloads['OFF-0']['orders']!=payloads['ON-0']['orders']
 assert changed==result['orders_differ']
 for arm,role in [('off','OFF-0'),('on','ON-0')]:
  assert result[arm+'_capital_production']==[o for o in payloads[role]['orders'] if o.get('kind') in ['produce','produce_next'] and o.get('subject')==65536]
 print(json.dumps({'verified':True,'archive_sha256':sha(args.archive.read_bytes()),'source':source['head'],'actual_reads_per_query':counts,'orders_differ':changed,'off_capital_production':result['off_capital_production'],'on_capital_production':result['on_capital_production'],'scope':protocol['scope']},indent=2))
