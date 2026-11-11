#!/usr/bin/env node
// Full natural Online games through the published WebAssembly ABI.
// Includes frame/state/reasoning work; excludes module startup and UI drawing.
// Used by site_wasm_pair.py; importing this file never starts a game.
// Complete browser-engine game, reasoning cursor and every-frame/save parity.
import {readFileSync, writeFileSync} from 'node:fs';
import {performance} from 'node:perf_hooks';
import {createHash} from 'node:crypto';

import {pathToFileURL} from 'node:url';

// Drop build/clock metadata only. All game fields and journal payloads remain.
export function normalizeFrame(state) {
  const stable={...state};
  for (const key of ['server_commit','server_commit_time','server_built_at','server_instance','server_artifact_bytes','turn_compute_ms']) delete stable[key];
  stable.ai_reasoning=canonicalMoveRefusalNotices(stable.ai_reasoning);
  return stable;
}

// Hash the original JSON bytes: parsing a u64 through JavaScript can round it.
export function hashSave(text) {
  return createHash('sha256').update(text).digest('hex');
}

export function validateSettings(settings,roster) {
  const expected={majors:4,cityStates:6,speed:'online',turns:250,map:'tenins_ball',shape:'planet',width:75,height:32,difficulty:'prince',turnStructure:'sequential'};
  if (roster==='six') Object.assign(expected,{majors:6,cityStates:9,map:'continents',width:90,height:38});
  else if (roster!=='default') throw new Error('unknown roster');
  for (const [key,value] of Object.entries(expected)) {
    if (settings[key]!==value) throw new Error(`unexpected game setting ${key}: ${settings[key]}`);
  }
  const victories=['culture','diplomatic','domination','religious','science','score'];
  const conditions=settings.victoryConditions??{};
  if (Object.keys(conditions).length!==victories.length || victories.some(key=>conditions[key]!==true)) throw new Error('victory conditions changed');
}

// judge_move_refusals emits a batch from a HashMap after all per-unit updates.
// Canonicalize only adjacent notices from that one player/turn, retaining the
// complete payload multiset and the exact stream ID sequence. All gameplay
// fields, other thoughts, journal cursors and counts remain unchanged.
export function canonicalMoveRefusalNotices(reasoning) {
  if (!reasoning || !Array.isArray(reasoning.thoughts)) return reasoning;
  const thoughts = reasoning.thoughts.map(thought => ({...thought}));
  const refusal = thought => thought.topic === 'military' && thought.level === 'detail'
    && /^.+ \d+ stops re-asking a move the host keeps refusing$/.test(thought.headline)
    && thought.detail.startsWith('ordered from ')
    && thought.detail.endsWith('so the route must change');
  for (let first = 0; first < thoughts.length;) {
    if (!refusal(thoughts[first])) { first++; continue; }
    let end = first + 1;
    while (end < thoughts.length && refusal(thoughts[end])
      && thoughts[end].player === thoughts[first].player
      && thoughts[end].turn === thoughts[first].turn) end++;
    const batch = thoughts.slice(first, end);
    const ids = batch.map(thought => thought.id);
    if (!ids.every((id, i) => Number.isSafeInteger(id) && (i === 0 || id > ids[i-1]))) {
      throw new Error('Unexpected diagnostic stream ID order');
    }
    batch.sort((a, b) => {
      const key = value => JSON.stringify({...value, id: null});
      const x = key(a), y = key(b);
      return x < y ? -1 : x > y ? 1 : 0;
    });
    for (let i = 0; i < batch.length; i++) thoughts[first+i] = {...batch[i], id: ids[i]};
    first = end;
  }
  return {...reasoning, thoughts};
}

export async function playGame(path, seedArg='7', roster='default', report='/tmp/civvis-browser-game.json') {
if (!['default','six'].includes(roster)) throw new Error('unknown roster');
const seed=Number(seedArg);
if (!Number.isSafeInteger(seed) || seed < 0) throw new Error('invalid seed');
const moduleBytes=readFileSync(path), moduleSha256=hashSave(moduleBytes);
const {instance,module}=await WebAssembly.instantiate(moduleBytes,{});
if (WebAssembly.Module.imports(module).length) throw new Error('engine must not import host state');
const engine=instance.exports, encoder=new TextEncoder(), decoder=new TextDecoder();
let requestWallMs=0, traceWallMs=0;
function ask(method,path,body,raw=false) {
  const before=performance.now();
  const request=encoder.encode(JSON.stringify({method,path,body:body===undefined?'':JSON.stringify(body),seed}));
  const at=engine.civvis_alloc(request.length);
  new Uint8Array(engine.memory.buffer,at,request.length).set(request);
  const answer=engine.civvis_request(at,request.length);
  const length=new DataView(engine.memory.buffer).getUint32(answer,true);
  const text=decoder.decode(new Uint8Array(engine.memory.buffer,answer+4,length));
  engine.civvis_free(answer,length+4);
  const value=JSON.parse(text);
  if(value.error) throw new Error(value.error);
  requestWallMs+=performance.now()-before;
  return raw ? {value,text} : value;
}
const trace=[],digest=createHash('sha256');
function record(state) {
  const before=performance.now(), stable=normalizeFrame(state);
  const hash=createHash('sha256').update(JSON.stringify(stable)).digest('hex');
  digest.update(hash+'\n');
  trace.push(JSON.stringify({turn:state.turn,sequence:state.frame_sequence,finished:state.finished,hash}));
  traceWallMs+=performance.now()-before;
}
const cpuStart=process.cpuUsage(),before=performance.now();
if(roster==='six') ask('POST','/new',{seed,num_players:6,num_city_states:9,width:74,height:46,map_script:'continents',map_topology:'planet',game_speed:'online',max_turns:250,spectate:true,paused:false,force:true});
ask('POST','/pace',{ms:0,paused:false});
let state=ask('GET','/state?painted=&viewer=h&think=0');
const majors=state.players.filter(p=>!p.is_minor&&!p.is_barbarian).length;
const cityStates=state.players.filter(p=>p.is_minor&&!p.is_barbarian).length;
if(majors!==(roster==='six'?6:4)||state.game_speed!=='online'||state.max_turns!==250||state.map.shape!=='planet') throw new Error('unexpected game settings');
if(roster==='six' && (cityStates!==9||state.map.script!=='continents')) throw new Error('unexpected six-major map');
const settings={majors,cityStates,speed:state.game_speed,turns:state.max_turns,map:state.map.script,shape:state.map.shape,width:state.map.width,height:state.map.height,difficulty:state.difficulty,turnStructure:state.turn_structure,victoryConditions:state.victory_conditions};
validateSettings(settings,roster);
let cursor=state.ai_reasoning?.cursor??0,count=0;
record(state);
console.log(JSON.stringify({harnessVersion:7,path,seed,roster,settings}));
while(!state.finished) {
  const held=`${state.seed}:${state.turn}:${state.finished?1:0}:${state.frame_sequence}`;
  const next=ask('GET',`/state?painted=&viewer=h&have=${held}&think=${cursor}`);
  if(next.turn===state.turn&&next.frame_sequence===state.frame_sequence&&!next.finished) throw new Error('world did not advance');
  cursor=next.ai_reasoning?.cursor??cursor;
  state=next;count++;record(state);
  if(count%25===0) console.log(JSON.stringify({count,turn:state.turn,wallMs:performance.now()-before}));
  if(count>5000||state.turn>251) throw new Error('game exceeded normal clock');
}
const wallMs=performance.now()-before, cpu=process.cpuUsage(cpuStart), requests=requestWallMs;
const {value:save,text:saveText}=ask('GET','/save',undefined,true);
if(save.game.game_speed!=='online'||save.game.max_turns!==250||save.game.players.filter(p=>!p.is_minor&&!p.is_barbarian).length!==majors) throw new Error('saved setup changed');
writeFileSync(report,saveText);
writeFileSync(report+'.trace.jsonl',trace.join('\n')+'\n');
console.log(JSON.stringify({harnessVersion:7,path,seed,roster,settings,wallMs,requestWallMs:requests,traceWallMs,cpuMs:(cpu.user+cpu.system)/1000,frames:count,turn:state.turn,finished:state.finished,winner:state.winner,victoryType:state.victory_type,traceHash:digest.digest('hex'),saveHash:hashSave(saveText),moduleSha256,memoryBytes:engine.memory.buffer.byteLength}));

}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await playGame(...process.argv.slice(2));
}
