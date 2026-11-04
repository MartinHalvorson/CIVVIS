from pathlib import Path
import collections,gzip,hashlib,json,sys
sys.path.insert(0,'/Users/martbot-mbp-m5-max-128/civvis-main-managed/tools')
from civ6_decision_trace import captured_input
ROOT=Path('/Users/martbot-mbp-m5-max-128/civvis-tactics-results/2026-10-25/defenders-skip-recon')
index=json.loads((ROOT/'native-delivery/observer.json').read_text())
assert len(index['attempts'])==8 and all(x.get('archive_complete') for x in index['attempts'])
report={'scope':'First8nativeattemptstemsfromconfirmeddefenderdelivery, everyrecordretained,no replacements. Observational mixedsource cohort, not a paired causal win-rate comparison. Verification receipts establish recorded host verification, not tactical worth. Quote cases use exactsinglecapturedread whenavailable; changing/missing inputs stay unresolved.','merge_commit':index['merged_commit'],'first_confirmed_delivery':index['first_confirmed_delivery'],'index_sha256':hashlib.sha256((ROOT/'native-delivery/observer.json').read_bytes()).hexdigest(),'runs':[]}

def state_at(raw,turn):
 end=len(raw)
 while end>0:
  pos=max(raw.rfind(b'"kind": "state"',0,end),raw.rfind(b'"kind":"state"',0,end))
  if pos<0:return None
  start=raw.rfind(b'\n',0,pos)+1;stop=raw.find(b'\n',pos);s=json.loads(raw[start:len(raw) if stop<0 else stop])
  if s.get('kind')=='state' and s.get('turn')==turn:return s
  end=start
 return None

for row in index['attempts']:
 directory=ROOT/'native-delivery'/row['run'];data={}
 for name,entry in row['files'].items():
  if entry.get('status')=='missing':continue
  path=directory/(name+'.gz');compressed=path.read_bytes();assert hashlib.sha256(compressed).hexdigest()==entry['archive_sha256'];raw=gzip.decompress(compressed);assert hashlib.sha256(raw).hexdigest()==entry['sha256'];data[name]=raw.decode()
 summary=json.loads(data['summary.json']);outcome=summary.get('outcome');kind=outcome.get('kind') if isinstance(outcome,dict) else None
 result={'run':row['run'],'source':row['source'],'summary':summary,'outcome':outcome,'result':'win' if kind=='victory' and outcome.get('won') is True else 'loss' if (kind=='victory' and outcome.get('won') is False) or (kind=='defeat' and outcome.get('ours') is True) else 'missing or unclassified outcome','verified_gold_unit_purchases':[],'verified_upgrade_receipts':[],'requested_gold_units':[],'requested_upgrades':[],'milestones':{}}
 events=[json.loads(x) for x in data['events.jsonl'].splitlines()];frames={};receipts=[]
 for e in events:
  if e.get('kind')=='state':frames.setdefault(e['turn'],e)
  if e.get('kind')=='order_verified':
   receipts.append(e)
   if e.get('order_kind')=='purchase' and e.get('verb','').startswith('UNIT_'):result['verified_gold_unit_purchases'].append(e)
   if e.get('order_kind')=='unit' and 'UPGRADE' in e.get('verb','').upper():result['verified_upgrade_receipts'].append(e)
 for turn in [40,60,80,100,120,150,200]:
  if turn not in frames:continue
  s=frames[turn];cities=s['cities'];buildings=collections.Counter(b for c in cities for b in c.get('buildings',[]));districts=collections.Counter(d['type'] for c in cities for d in c.get('districts',[]) if d.get('complete',True))
  result['milestones'][str(turn)]={'turn':turn,'frame':s.get('frame',0),'gold':s['gold'],'income':s['gold_per_turn'],'science':s['science'],'culture':s['culture'],'military':s['military'],'cities':len(cities),'production':s.get('public_stats',{}).get('production'),'unit_maintenance':s.get('unit_maintenance_total'),'building_maintenance':s.get('building_maintenance_total'),'district_maintenance':s.get('district_maintenance_total'),'routes':len(s.get('trade_routes',[])),'trade_capacity':s.get('trade_capacity'),'buildings':dict(buildings),'completed_districts':dict(districts),'observed_rivals':[{'civ':r['civ'],'science':r.get('science'),'military':r.get('military'),'income':r.get('gold_per_turn'),'production':r.get('public_stats',{}).get('production')} for r in s.get('rivals',[])]}
 decisions=[json.loads(x) for x in data['decisions.jsonl'].splitlines()]
 result['recorded_binary_hashes']=sorted({q.get('binary_on_disk_sha256') for q in decisions});result['recorded_hashes_match_frozen_binary']=all(x==row['source'].get('actual_frozen_binary_sha256') for x in result['recorded_binary_hashes'])
 for ordinal,q in enumerate(decisions,1):
  wanted=[a for a in q['decision'].get('native_actions',[]) if (a.get('type')=='buy' and a.get('currency')=='gold') or a.get('type')=='upgrade_unit']
  if not wanted:continue
  s=None;status=None;prefix_sha=None
  try:
   c=q['input_capture'];assert c['complete'],'incomplete capture';assert all('snapshot_id' in r for r in c['reads']),'read errors';ids={r['snapshot_id'] for r in c['reads']};assert len(ids)==1,'changing input reads'
   raw=captured_input(c,next(iter(ids)));prefix_sha=hashlib.sha256(raw).hexdigest();s=state_at(raw,q['decision']['turn']);assert s is not None and s.get('frame',0)==q['decision']['frame'],'state identity mismatch';status='exact single captured read'
  except Exception as e:status=str(e)
  for a in wanted:
   case={'decision_line':ordinal,'decision_sha256':q['sha256'],'turn':q['decision']['turn'],'frame':q['decision']['frame'],'action':a,'input_status':status,'input_prefix_sha256':prefix_sha,'input_capture':q.get('input_capture')}
   if a['type']=='buy':
    native=next((int(n) for n,m in q['decision'].get('city_ids',{}).items() if m==a['city']),None);orders=[o for o in q['orders'] if o.get('kind')=='purchase' and o.get('subject')==native];case['native_orders']=orders
    if s is not None:
     city=next((c for c in s['cities'] if c['id']==native),None);case['native_city_id']=native;case['treasury_before']=s['gold'];case['income_before']=s['gold_per_turn'];case['native_purchase_menu']=city.get('purchasable',[]) if city else None
    case['matching_same_turn_city_verification']=[e for e in result['verified_gold_unit_purchases'] if e.get('turn')==case['turn'] and e.get('subject')==native and any(e.get('verb')==o['verb'] for o in orders)]
    result['requested_gold_units'].append(case)
   else:
    native=q['decision'].get('unit_ids',{}).get(str(a['unit']));orders=[o for o in q['orders'] if o.get('kind')=='unit' and 'UPGRADE' in o.get('verb','').upper() and o.get('subject')==native];case['native_orders']=orders;case['native_unit_id']=native
    if s is not None:case['native_unit_and_upgrade_quote']=next((u for u in s['units'] if u['id']==native),None);case['treasury_before']=s['gold'];case['income_before']=s['gold_per_turn']
    case['matching_same_turn_unit_verification']=[e for e in result['verified_upgrade_receipts'] if e.get('turn')==case['turn'] and e.get('subject')==native]
    result['requested_upgrades'].append(case)
 result['verified_gold_unit_counts']=dict(collections.Counter(e['verb'] for e in result['verified_gold_unit_purchases']));result['verified_gold_scout_receipts']=result['verified_gold_unit_counts'].get('UNIT_SCOUT',0)
 result['requested_gold_scouts']=sum(x['action']['unit']=='scout' for x in result['requested_gold_units']);result['exact_gold_quote_cases']=sum(x['input_status']=='exact single captured read' for x in result['requested_gold_units']);result['exact_upgrade_quote_cases']=sum(x['input_status']=='exact single captured read' for x in result['requested_upgrades'])
 report['runs'].append(result);print(row['run'],result['result'],(outcome or {}).get('victory_type',kind),(outcome or {}).get('turn'),'Gold',result['verified_gold_unit_counts'],'upgrade verified',len(result['verified_upgrade_receipts']),'cities taken',summary.get('combat',{}).get('cities_taken'),flush=True)
report['wins']=sum(x['result']=='win' for x in report['runs']);report['losses']=sum(x['result']=='loss' for x in report['runs']);report['missing_or_unclassified']=sum(x['result']=='missing or unclassified outcome' for x in report['runs']);report['all_recorded_hashes_match_frozen_binary']=all(x['recorded_hashes_match_frozen_binary'] for x in report['runs']);report['verified_gold_scout_receipts']=sum(x['verified_gold_scout_receipts'] for x in report['runs']);report['requested_gold_scouts']=sum(x['requested_gold_scouts'] for x in report['runs']);report['source_revisions']=dict(collections.Counter(x['source']['provenance']['revision'] for x in report['runs']))
out=ROOT/'native-delivery-analysis.json.gz'
with gzip.GzipFile(filename=str(out),mode='wb',mtime=0) as f:f.write(json.dumps(report,sort_keys=True,separators=(',',':')).encode())
assert json.loads(gzip.decompress(out.read_bytes()))==report
(ROOT/'native-delivery-analysis.py').write_bytes(Path(__file__).read_bytes());print(json.dumps({k:report[k] for k in ['wins','losses','missing_or_unclassified','all_recorded_hashes_match_frozen_binary','verified_gold_scout_receipts','requested_gold_scouts','source_revisions']},indent=2),flush=True)
