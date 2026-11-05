import tarfile,json,hashlib,pathlib,sys
path=pathlib.Path(sys.argv[1])
with tarfile.open(path,'r:xz') as tar:
 files={m.name:tar.extractfile(m).read() for m in tar.getmembers() if m.isfile()}
manifest=json.loads(files['manifest.json']);genomes=[];configs=[];states=[];maps=[];orders=[]
for case in manifest['cases']:
 role=case['role'];raw=files[role+'/input.jsonl'];record=json.loads(files[role+'/decision.json']);context=json.loads(files[role+'/observed-context.json'])
 assert len(raw)==case['input_bytes'] and hashlib.sha256(raw).hexdigest()==case['input_sha256']
 unsigned={k:v for k,v in record.items() if k!='sha256'}
 assert hashlib.sha256(json.dumps(unsigned,sort_keys=True,separators=(',',':'),allow_nan=False).encode()).hexdigest()==record['sha256']==case['decision_receipt_sha256']
 assert record['input_capture']['complete'] and record['input_capture']['reads']==[{'count':18,'snapshot_id':1}]
 assert record['binary_on_disk_sha256']==context['binary_sha256']==manifest['native_binary_sha256']
 assert context['launcher_lua_source_revision']==manifest['source_revision']
 genome=context['actual_genome'];assert ('campus-above-the-sea' in genome['treatments'])==role.endswith('on')
 for field in ('treatments','forced'):genome[field]=sorted(g for g in genome[field] if g!='campus-above-the-sea')
 genomes.append(genome);configs.append({k:v for k,v in context['installed_native_lua']['installed_config'].items() if k not in ('RunTag','OrdersDb')})
 events=[json.loads(l) for l in raw.splitlines()];state=[e for e in events if e.get('kind')=='state'][-1]
 assert (state['turn'],state.get('frame',0))==(5,0)
 states.append({k:v for k,v in state.items() if k not in ('run','t','utc')})
 terrain={}
 for e in events:
  if e.get('kind')=='tiles':
   for tile in e['plots']:terrain[(tile['x'],tile['y'])]=tile
 maps.append(terrain);orders.append(record['orders'])
 assert events[-1]['kind']==('state' if role=='01-off' else 'tiles')
assert genomes[0]==genomes[1] and configs[0]==configs[1] and states[0]==states[1]
assert maps[0]!=maps[1] and orders[0]!=orders[1]
print('Verified 36 registered native reads: same city/unit board, different terrain frontier and orders; runtime/config/genome normalization passes')
