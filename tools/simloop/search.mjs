// What does a search on the Sim debugger cost, and does it hold together?
//
// A one-ply search, on nothing but what the debugger already answers: play a
// game to a turn, then for each Civ AI that could hold the seat play the next
// few turns under it and compare how the seat stands. It is run twice, the
// two ways a search can be arranged:
//
// * on forks: the engine's memory copied into one fresh instance per Civ AI,
//   each on a thread of its own (what `POST /forks` does in the page, see
//   `beta/worker.js`), all playing at once;
// * on one engine: play a Civ AI's turns, walk back to the turn the search
//   started from (`jump`), play the next.
//
// Every step is asked for `lean` (`docs/SIM_DEBUGGER.md`): the answer is the
// turn and every seat's score, and no state document. The two arrangements
// must agree on every score, which is the check that a fork is the engine
// and that a walk back is the world it left; the times are what a search
// pays for each.
//
//   node search.mjs --wasm <civvis.wasm> [--seed 7] [--from 60] [--turns 10]
//                   [--agents jack,siege,...] [--rivals wheels]
//
// `--rivals` seats one Civ AI in every rival chair (`rival_strategy` on
// `/new`); against `wheels` a turn costs about half what it does against the
// house fleet. Exit status 1 if the two arrangements disagree.

import { readFileSync } from "node:fs";
import { Worker, isMainThread, parentPort, workerData } from "node:worker_threads";

function flag(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  return at === -1 || at + 1 >= process.argv.length ? fallback : process.argv[at + 1];
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

// One engine: a module instance and the one call its ABI has.
function engine(module, seed) {
  const exports = new WebAssembly.Instance(module, {}).exports;
  const ask = (method, path, body) => {
    const request = encoder.encode(JSON.stringify({
      method, path, body: body === undefined ? "" : JSON.stringify(body), seed,
    }));
    const at = exports.civvis_alloc(request.length);
    new Uint8Array(exports.memory.buffer, at, request.length).set(request);
    const out = exports.civvis_request(at, request.length);
    const length = new DataView(exports.memory.buffer).getUint32(out, true);
    const text = decoder.decode(new Uint8Array(exports.memory.buffer, out + 4, length));
    exports.civvis_free(out, length + 4);
    return JSON.parse(text);
  };
  return { exports, ask };
}

// A fork: a fresh instance given a copy of another's memory. The module
// imports nothing and keeps all it knows there, so that is the same engine.
function fork(module, memory, seed) {
  const made = engine(module, seed);
  const short = memory.byteLength - made.exports.memory.buffer.byteLength;
  if (short > 0) made.exports.memory.grow(short / 65536);
  new Uint8Array(made.exports.memory.buffer).set(new Uint8Array(memory));
  return made;
}

// `turns` turns with `agent` in the seat, and how the seat stands after them.
function rollout(played, agent, turns) {
  const started = performance.now();
  let last = null;
  for (let turn = 0; turn < turns; turn++) {
    last = played.ask("POST", "/debug", { op: "step", strategy: agent, lean: true });
    if (last.error) break;
  }
  const seat = (last.scores || []).find((entry) => entry.id === 0) || {};
  const rivals = (last.scores || []).filter((entry) => entry.id !== 0).map((entry) => entry.score);
  return {
    agent,
    ms: Math.round(performance.now() - started),
    turn: last.turn,
    error: last.error || null,
    score: seat.score,
    lead: seat.score - Math.max(...rivals),
  };
}

if (!isMainThread) {
  const { module, memory, seed, agent, turns } = workerData;
  parentPort.postMessage(rollout(fork(module, memory, seed), agent, turns));
} else {
  const wasm = flag("wasm", null);
  if (!wasm) {
    console.error("usage: node search.mjs --wasm <civvis.wasm> [--seed N] [--from T] [--turns H] [--agents a,b] [--rivals name]");
    process.exit(2);
  }
  const seed = Number(flag("seed", 7));
  const from = Number(flag("from", 60));
  const turns = Number(flag("turns", 10));
  const agents = flag("agents", "jack,siege,moonshot,tourist,holy,swing,forward,stock").split(",");
  const rivals = flag("rivals", "");

  const module = new WebAssembly.Module(readFileSync(wasm));
  const main = engine(module, seed);
  const made = main.ask("POST", "/new", {
    num_players: 6, spectate: false, seed, map_script: "continents", force: true,
    ...(rivals ? { rival_strategy: rivals } : {}),
  });
  if (made.error) {
    console.error(`the game was not made: ${made.error}`);
    process.exit(2);
  }
  let started = performance.now();
  let turn = 1;
  while (turn < from) {
    const next = main.ask("POST", "/debug", { op: "step", lean: true });
    if (next.error || next.turn === turn) break;
    turn = next.turn;
  }
  const opening = Math.round(performance.now() - started);

  // On forks, all at once. The memory is copied once into a buffer every
  // thread reads, and once more by each fork into its own instance.
  started = performance.now();
  const shared = new SharedArrayBuffer(main.exports.memory.buffer.byteLength);
  new Uint8Array(shared).set(new Uint8Array(main.exports.memory.buffer));
  const onForks = await Promise.all(agents.map((agent) => new Promise((resolve, reject) => {
    const worker = new Worker(new URL(import.meta.url), {
      workerData: { module, memory: shared, seed, agent, turns },
    });
    worker.once("message", resolve);
    worker.once("error", reject);
  })));
  const forksMs = Math.round(performance.now() - started);

  // On the one engine, walking back between them.
  started = performance.now();
  const onOne = [];
  for (const agent of agents) {
    onOne.push(rollout(main, agent, turns));
    const back = main.ask("POST", "/debug", { op: "jump", turn, lean: true });
    if (back.error) {
      console.error(`could not walk back to turn ${turn}: ${back.error}`);
      process.exit(1);
    }
  }
  const oneMs = Math.round(performance.now() - started);

  const agree = onForks.every((result, index) =>
    result.score === onOne[index].score && result.turn === onOne[index].turn
      && result.error === onOne[index].error);
  console.log(JSON.stringify({
    seed, rivals: rivals || "the house fleet", from: turn, turns, agents: agents.length,
    opening_ms: opening,
    engine_mb: main.exports.memory.buffer.byteLength >> 20,
    on_forks_ms: forksMs,
    on_one_engine_ms: oneMs,
    rollout_turns_a_second_on_forks: Math.round(agents.length * turns * 1000 / forksMs),
    agree,
    ranked: onForks.slice().sort((a, b) => b.lead - a.lead)
      .map((result) => `${result.agent} ${result.score} (${result.lead >= 0 ? "+" : ""}${result.lead})`),
  }, null, 1));
  if (!agree) {
    console.error("the forks and the one engine disagree:", JSON.stringify({ onForks, onOne }));
    process.exit(1);
  }
}
