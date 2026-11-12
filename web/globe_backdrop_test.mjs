// Pure renderer-cache guards: no browser, WASM, game, or network access.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
const source = fs.readFileSync(new URL('./assets/app.js', import.meta.url), 'utf8');
function fn(name) {
  const start = source.indexOf(`function ${name}(`);
  assert(start >= 0, `missing real ${name}`);
  return source.slice(start, source.indexOf('\n}', start) + 2);
}
function fixture() {
  const commands = [], stack = [];
  const context = {globalAlpha:.6, globalCompositeOperation:'source-over', shadowBlur:7,
    shadowOffsetX:2, shadowOffsetY:3, filter:'blur(1px)', lineWidth:2,
    fillStyle:'#abcdef', strokeStyle:'#123456', font:'12px sans-serif',
    getLineDash:() => [2, 3], setLineDash:d => commands.push(['dash', d]),
    setTransform:(...args) => commands.push(['transform', ...args]),
    save() { stack.push({...this}); commands.push(['save']); },
    restore() { Object.assign(this, stack.pop()); commands.push(['restore']); },
    drawImage:(...args) => commands.push(['paint', ...args])};
  let creates = 0;
  const brush = {drawImage:(...args) => commands.push(['copy', ...args])};
  const ctx = {cam:{x:1,y:2,scale:3,rot:4,tilt:5}, MAPW:600, MAPH:400, DPR:2,
    cv:{width:1200,height:800}, state:{seed:7}, PLANET:{}, cx:context,
    PLANET_HIT_FRAME:{cells:['original'], inkOnly:false},
    document:{createElement(name) {
      assert.equal(name, 'canvas'); creates++;
      return {width:0,height:0,getContext:(kind, options) => {
        assert.equal(kind, '2d'); assert.equal(options.alpha, true); return brush;
      }};
    }}, applyPlanetGroundTilt:y => commands.push(['tilt', y])};
  vm.createContext(ctx);
  const helpers = fs.readFileSync(new URL('./assets/app_backdrop.js', import.meta.url), 'utf8');
  vm.runInContext(helpers, ctx);
  const run = code => vm.runInContext(code, ctx);
  return {ctx, run, commands, stack, context, creates:() => creates};
}
let passed = 0;
function test(name, run) { run(); passed++; console.log(`ok ${passed} - ${name}`); }

test('capture keeps original physical resolution, cells, and hit-test frame', () => {
  const f = fixture();
  f.run('savePlanetBackdrop(planetBackdropView(17, 19), ["cell"]);');
  const held = f.run('planetBackdrop');
  assert.equal(held.canvas.width, 1200); assert.equal(held.canvas.height, 800);
  assert.deepEqual(Array.from(held.cells), ['cell']);
  assert.equal(held.hitFrame, f.ctx.PLANET_HIT_FRAME);
  assert.deepEqual(f.commands[0], ['copy', f.ctx.cv, 0, 0]);
  assert.equal(f.run('reusablePlanetBackdrop(planetBackdropView(17, 19))'), held);
});

test('every camera, viewport, pixel ratio and physical size component invalidates exactly', () => {
  const f = fixture(); f.run('savePlanetBackdrop(planetBackdropView(17, 19), []);');
  const view = Array.from(f.run('planetBackdropView(17, 19)'));
  for (let at = 0; at < view.length; at++) {
    const next = [...view]; next[at] += Number.EPSILON * Math.max(1, Math.abs(next[at]));
    f.ctx.nextView = next;
    assert.equal(f.run('reusablePlanetBackdrop(nextView)'), null, `component ${at}`);
  }
});

test('a different publication or globe never reuses the previous background', () => {
  for (const property of ['state', 'PLANET']) {
    const f = fixture(); f.run('savePlanetBackdrop(planetBackdropView(17, 19), []);');
    f.ctx[property] = {};
    assert.equal(f.run('reusablePlanetBackdrop(planetBackdropView(17, 19))'), null);
  }
});

test('replay copies pixels with identity transform and restores overlay paint state', () => {
  const f = fixture(); f.run('savePlanetBackdrop(planetBackdropView(17, 19), []);');
  f.context.globalAlpha = .2; f.context.fillStyle = '#000000';
  const initialFrame = f.ctx.PLANET_HIT_FRAME; f.ctx.PLANET_HIT_FRAME = null;
  f.run('paintPlanetBackdrop(planetBackdrop, 19)');
  assert.equal(f.context.globalAlpha, .6); assert.equal(f.context.fillStyle, '#abcdef');
  assert.equal(f.context.shadowBlur, 7);
  assert.equal(f.stack.length, 1, 'same ground-tilt save boundary as the original renderer');
  assert.equal(f.run('planetBackdrop.hits'), 1);
  assert.notEqual(f.ctx.PLANET_HIT_FRAME, initialFrame);
  assert.equal(f.ctx.PLANET_HIT_FRAME.cells, initialFrame.cells);
  const paint = f.commands.findIndex(c => c[0] === 'paint');
  assert.deepEqual(f.commands[paint-2], ['transform', 1, 0, 0, 1, 0, 0]);
});

test('repeated ordinary publications reuse one backing allocation', () => {
  const f = fixture();
  for (let i=0; i<100; i++) f.run('planetBackdrop = null; savePlanetBackdrop(planetBackdropView(17, 19), []);');
  assert.equal(f.creates(), 1);
});

test('zero and oversized canvases do not allocate a backdrop', () => {
  for (const [width,height] of [[0,800],[20000,20000]]) {
    const f = fixture(); f.ctx.cv.width=width; f.ctx.cv.height=height;
    f.run('savePlanetBackdrop(planetBackdropView(17, 19), []);');
    assert.equal(f.creates(), 0); assert.equal(f.run('planetBackdrop'), null);
  }
});

test('real ordinary draw invalidates while animation draw requests reuse', () => {
  const f = fixture();
  const calls = [];
  Object.assign(f.ctx, {syncSkyNav:()=>{}, drawPlanetMap:reuse=>{calls.push(reuse); return true;},
    performance:{now:()=>10}, recordPerformanceRender:()=>{}});
  vm.runInContext(fn('drawScene')+'\n'+fn('draw'), f.ctx);
  f.run('savePlanetBackdrop(planetBackdropView(17, 19), []); draw(true);');
  assert(f.run('planetBackdrop')); assert.equal(calls.at(-1), true);
  f.run('draw();'); assert.equal(f.run('planetBackdrop'), null); assert.equal(calls.at(-1), false);
  f.run('savePlanetBackdrop(planetBackdropView(17, 19), []); draw({type:"click"});');
  assert.equal(f.run('planetBackdrop'), null); assert.equal(calls.at(-1), false);
});

function sceneFixture() {
  const f = fixture(), ctx = f.ctx, visits = {ground:0, cells:0, overlays:[]};
  // Execute the real complete map function. Synthetic painters isolate its
  // control flow; native copy/compositing is covered by --native-canvas.
  const map = fn('drawPlanetMap');
  const builtins = new Set(['if','for','while','switch','Map','Set','Object','Number']);
  for (const [,name] of map.matchAll(/(?<![.\w])([A-Za-z_$][\w$]*)\(/g))
    if (!builtins.has(name) && !(name in ctx)) ctx[name] = () => {};
  Object.assign(ctx, {SPEC:true, cam:{x:1,y:2,scale:1,rot:0,tilt:0},
    state:{view_player:null,map:{tiles:[]},cities:[],units:[]}, PLANET_GPU_MAP:false,
    PLANET_GPU:null, SKY_MARKERS:100, PLANET_STRATEGIC_DETAIL_SCALE:.3,
    mapLens:null,sel:null,selCity:null,mapSearchQuery:'',tacks:[],
    PLANET_HIT_CELLS:null, performance:{now:()=>100},
    planetMap:()=>true,planetReady:()=>true,planetCamera:()=>({radius:40,chart:false}),
    planetEarthCenter:()=>[60,40],tileSight:()=>({visible:new Set(),turnVisible:new Set()}),
    skyOnCanvas:()=>true,drawPlanetGpuSurface:()=>false,skyDepth:()=>0,
    planetCells:()=>{visits.cells++;return [];},
    drawPlanetGround:()=>{visits.ground++;},
    drawPlanetBlasts:(_cells,now)=>visits.overlays.push(now)});
  Object.assign(f.context, {createRadialGradient:()=>({addColorStop:()=>{}}),
    beginPath:()=>{},arc:()=>{},fill:()=>{},stroke:()=>{},fillRect:()=>{},clearRect:()=>{}});
  vm.runInContext(map,ctx);
  return {...f,visits};
}

test('actual map replay skips only backdrop painters and keeps live overlays', () => {
  const f=sceneFixture();
  f.run('drawPlanetMap(false)');
  assert.equal(f.visits.ground,1); assert.equal(f.visits.cells,1);
  f.ctx.performance.now=()=>200;
  f.run('drawPlanetMap(true)');
  assert.equal(f.visits.ground,1); assert.equal(f.visits.cells,1);
  assert.deepEqual(f.visits.overlays,[100,200]);
  assert.equal(f.run('planetBackdrop.hits'),1);
  assert.equal(f.stack.length,0,'both complete frames balance their drawing-state stacks');
});

test('actual map camera movement redraws without warming unusable animation copies', () => {
  const f=sceneFixture(); f.run('drawPlanetMap(false)');
  f.ctx.cam.x+=.01; f.run('drawPlanetMap(true)');
  assert.equal(f.visits.ground,2); assert.equal(f.visits.cells,2);
  assert.equal(f.run('planetBackdrop.hits'),0); assert.equal(f.creates(),1);
  f.run('drawPlanetMap(false);drawPlanetMap(true)');
  assert.equal(f.visits.ground,3); assert.equal(f.run('planetBackdrop.hits'),1);
});

test('actual interactive lens bypasses a previously valid backdrop', () => {
  const f=sceneFixture();f.run('drawPlanetMap(false)');
  f.ctx.mapLens='empire';f.run('drawPlanetMap(true)');
  assert.equal(f.visits.ground,2);assert.equal(f.run('planetBackdrop.hits'),0);
});
console.log(`${passed} pure backdrop tests passed`);

