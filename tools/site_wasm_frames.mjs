#!/usr/bin/env node
// Time the published browser engine the way the page drives it: one frame per
// round at Lightning pace, each frame a turn played and a state document
// written. `civvis site-bench` prices the same frame loop on a native build;
// this prices the module a visitor actually downloads.
//
//   curl -o /tmp/civvis.wasm https://www.civvis.ai/test/civvis.wasm
//   node tools/site_wasm_frames.mjs /tmp/civvis.wasm --turns 100 --seed 7
//   node tools/site_wasm_frames.mjs /tmp/civvis.wasm --turns 200 --split
//
// By hand only: it needs a built or downloaded module, and a CI runner's clock
// is not a laptop's. Compare two modules with the same seed, in both orders,
// and take the geometric mean of the two ratios: V8 shares compiled code
// between identical modules, and a module listed second has read 16% faster
// than the same bytes listed first.
//
// Four things that have each cost a wrong reading:
//
// * A frame is requested by naming the frame the page holds (`have=`). Without
//   it `/state` re-reads the world and nothing is played.
// * `think=0` on every request re-sends the whole reasoning journal, and the
//   document reads 1.5-2 MB instead of 110-440 KB. Track the cursor.
// * A bare module starts at pace 500, which is one frame per seat. The site
//   runs Lightning, so `/pace {"ms": 0}` comes first.
// * The first turns run on cold code and cost about 2.75x; a second instance
//   of the same compiled module (`--warm`) shows the steady state.
//
// For where the time goes by function, build a module with names and run this
// under `node --cpu-prof`; `tools/site_wasm_profile.py` reads the result.
//
// `--split` also prices the document alone: it pauses the world, asks for the
// frame it already holds, and reads that as the document's share. The rest of
// the frame is the agents and the rules.

import fs from "node:fs";

const args = process.argv.slice(2);
const file = args.find((arg) => !arg.startsWith("--"));
const flag = (name, fallback) => {
  const at = args.indexOf(name);
  return at >= 0 && args[at + 1] !== undefined ? Number(args[at + 1]) : fallback;
};
if (!file) {
  console.error("usage: site_wasm_frames.mjs civvis.wasm [--turns N] [--seed N] [--split] [--warm]");
  process.exit(2);
}
const TURNS = flag("--turns", 100);
const SEED = flag("--seed", 7);
const SPLIT = args.includes("--split");
const WARM = args.includes("--warm");
const BANDS = [[1, 10], [11, 25], [26, 50], [51, 75], [76, 100], [101, 150], [151, 200], [201, Infinity]];

const module_ = new WebAssembly.Module(fs.readFileSync(file));

function session() {
  // The module imports nothing: no clock, no entropy, no host calls.
  const { exports } = new WebAssembly.Instance(module_, {});
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  return (method, path, body) => {
    const request = encoder.encode(JSON.stringify({ method, path, body: body ?? "", seed: SEED }));
    const at = exports.civvis_alloc(request.length);
    new Uint8Array(exports.memory.buffer, at, request.length).set(request);
    const answer = exports.civvis_request(at, request.length);
    const length = new DataView(exports.memory.buffer).getUint32(answer, true);
    const text = decoder.decode(new Uint8Array(exports.memory.buffer, answer + 4, length));
    exports.civvis_free(answer, length + 4);
    return text;
  };
}

function play(label) {
  const ask = session();
  ask("POST", "/pace", JSON.stringify({ ms: 0, paused: false }));
  let state = JSON.parse(ask("GET", "/state?painted=&viewer=h&think=0"));
  const seed = state.seed;
  let cursor = state.ai_reasoning?.cursor ?? 0;
  const held = (st) => `${seed}:${st.turn}:${st.finished ? 1 : 0}:${st.frame_sequence}`;
  const bands = BANDS.map(() => ({ frames: 0, frame: 0, document: 0, bytes: 0, units: 0, cities: 0 }));
  const started = performance.now();
  const cpu = process.cpuUsage();
  while (state.turn <= TURNS && !state.finished) {
    const band = bands[BANDS.findIndex(([first, last]) => state.turn >= first && state.turn <= last)];
    const before = performance.now();
    const text = ask("GET", `/state?painted=&viewer=h&have=${held(state)}&think=${cursor}`);
    band.frame += performance.now() - before;
    const next = JSON.parse(text);
    if (next.turn === state.turn && next.frame_sequence === state.frame_sequence) {
      throw new Error(`the world did not advance past turn ${state.turn}`);
    }
    cursor = next.ai_reasoning?.cursor ?? cursor;
    band.frames += 1;
    band.bytes += text.length;
    band.units += (next.units ?? []).length;
    band.cities += (next.cities ?? []).length;
    if (SPLIT) {
      ask("POST", "/pace", JSON.stringify({ ms: 0, paused: true }));
      const again = performance.now();
      ask("GET", `/state?painted=&viewer=h&have=${held(next)}&think=${cursor}`);
      band.document += performance.now() - again;
      ask("POST", "/pace", JSON.stringify({ ms: 0, paused: false }));
    }
    state = next;
  }
  const spent = process.cpuUsage(cpu);
  const frames = bands.reduce((sum, band) => sum + band.frame, 0);
  const documents = bands.reduce((sum, band) => sum + band.document, 0);
  console.log(
    `${label}: turn ${state.turn}${state.finished ? " (finished)" : ""}, frames ${(frames / 1000).toFixed(2)} s` +
      (SPLIT ? `, of which documents ${(documents / 1000).toFixed(2)} s (${((100 * documents) / frames).toFixed(0)}%)` : "") +
      `; wall ${((performance.now() - started) / 1000).toFixed(2)} s, process CPU ${(spent.user / 1e6).toFixed(2)} s`,
  );
  BANDS.forEach(([first, last], at) => {
    const band = bands[at];
    if (!band.frames) return;
    const per = (value) => (value / band.frames).toFixed(0);
    console.log(
      `  turns ${first}-${last === Infinity ? "on" : last}: ${per(band.frame)} ms a frame` +
        (SPLIT ? ` (document ${per(band.document)} ms, play ${per(band.frame - band.document)} ms)` : "") +
        `, ${per(band.bytes / 1024)} KB, ${per(band.units)} units, ${per(band.cities)} cities, ${band.frames} frames`,
    );
  });
}

play("cold");
if (WARM) play("warm");
