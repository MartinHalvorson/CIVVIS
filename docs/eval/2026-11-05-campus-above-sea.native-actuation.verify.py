import pathlib,json,tarfile,hashlib,sys
path=pathlib.Path(sys.argv[1])
with tarfile.open(path,'r:xz') as tar:files={m.name:tar.extractfile(m).read() for m in tar.getmembers() if m.isfile()}
m=json.loads(files['manifest.json']);record=json.loads(files['native-decision.json']);context=json.loads(files['observed-context.json']);preflight=json.loads(files['preflight.json'])
unsigned={k:v for k,v in record.items() if k!='sha256'}
assert hashlib.sha256(json.dumps(unsigned,sort_keys=True,separators=(',',':'),allow_nan=False).encode()).hexdigest()==record['sha256']
assert record['binary_on_disk_sha256']==context['binary_sha256']=='27d98df06b4b6193f93d6dee6f62a38a292e95007f939a79515ccfdf9400751b'
assert context['launcher_lua_source_revision']==m['source_revision']=='d66b7cf2dd20abb7fea2e9ebc5de58a0e25b6118'
assert context['actual_genome']==preflight['actual_genome'] and context['campus_above_the_sea'] is True
assert 'campus-above-the-sea' in context['actual_genome']['treatments']
assert context['installed_native_lua']['installed_config']==preflight['configuration']
dec=record['decision'];assert [dec['turn'],dec['frame']]==[117,0] and dec['victory_portfolio']['assigned_target']=='science'
assert record['input_capture']['complete'] and record['input_capture']['error'] is None
expected=[{'snapshot_id':f['snapshot_id'],'count':f['count']} for f in m['frontiers']]
assert expected==record['input_capture']['reads'] and sum(f['count'] for f in expected)==18
reconstructed={}
for f in m['frontiers']:
 base=b'' if f['parent'] is None else reconstructed[f['parent']]
 append=files[f['append_path']];assert len(append)==f['append_bytes'];raw=base+append
 assert len(raw)==f['raw_bytes'] and hashlib.sha256(raw).hexdigest()==f['raw_sha256'];reconstructed[f['snapshot_id']]=raw
 state=None;tiles={}
 for line in raw.splitlines():
  e=json.loads(line)
  if e.get('kind')=='state':state=e
  if e.get('kind')=='tiles':
   for t in e['plots']:tiles[(t['x'],t['y'])]=t
 assert [state['turn'],state.get('frame',0)]==[117,0]
 c=next(c for c in state['cities'] if c['id']==m['city'])
 assert not any(d['type']=='DISTRICT_CAMPUS' for d in c.get('districts',[]))
 offered=next(b for b in c['buildable'] if b['t']=='DISTRICT_CAMPUS')['s']
 assert all({'x':x,'y':y} in offered for x,y in (m['planned'],m['selected']))
 assert tiles[tuple(m['planned'])]['cl']==2 and tiles[tuple(m['selected'])]['cl']==-1
 assert not tiles[tuple(m['selected'])]['flooded'] and not tiles[tuple(m['selected'])]['submerged']
model=dec['city_ids'][str(m['city'])]
assert any(a.get('type')=='produce' and a.get('city')==model and a.get('item',{}).get('district')=='campus' and [a['item']['pos'][0]+a['item']['pos'][1]//2,a['item']['pos'][1]]==m['planned'] for a in dec['native_actions'])
assert any(o['kind']=='produce' and o['subject']==m['city'] and o['verb']=='DISTRICT_CAMPUS' and [o['x'],o['y']]==m['selected'] for o in record['orders'])
witnesses={}
for item in m['host_witnesses']:
 raw=files[item['path']];assert hashlib.sha256(raw).hexdigest()==item['sha256'];state=json.loads(raw)
 assert [state['turn'],state.get('frame',0)]==[item['turn'],item['frame']]
 c=next(c for c in state['cities'] if c['id']==m['city']);campus=[d for d in c['districts'] if d['type']=='DISTRICT_CAMPUS']
 assert len(campus)==1 and [campus[0]['x'],campus[0]['y']]==m['selected'] and not campus[0]['pillaged']
 witnesses[(item['turn'],item['frame'])]=(c,campus[0])
assert witnesses[(117,1)][1]['complete'] is False and witnesses[(130,0)][1]['complete'] is True
for key,building in [((133,0),'BUILDING_LIBRARY'),((139,0),'BUILDING_UNIVERSITY'),((150,0),'BUILDING_RESEARCH_LAB')]:assert building in witnesses[key][0]['buildings']
final=witnesses[(166,2)][0]
assert all(b in final['buildings'] and b not in final.get('pillaged_buildings',[]) for b in ('BUILDING_LIBRARY','BUILDING_UNIVERSITY','BUILDING_RESEARCH_LAB'))
print('Verified 18 actual reads, host-legal lowland-to-safe retarget, native foundation117, completion130, library133/university139/lab150, unpillaged through166; no strength gain established')
