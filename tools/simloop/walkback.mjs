// Is a line a search walked back to the line the page plays straight?
//
// The Sim debugger's Autosearch (`docs/SIM_DEBUGGER.md`) plays many lines of
// one world on copies of the engine. A line is who holds the seat when: one
// Civ AI from the turn the search begins on, and handovers after it. A copy
// does not play each line from that turn: it walks back to the turn where the
// line parts from the one it played last (`jump`, with the line's Civ AI for
// that turn named) and plays on from there (`play`). The page then plays the
// line that was found straight through: a walk to the first turn, and each
// stretch seated (`status`) and stepped.
//
// The two must end on the same world, or what a search found is not what
// playing it gives. This plays six lines both ways, the copy's in an order
// that walks back to the middle of a line, to its start, and back onto a
// line it left, and compares the engine's print of the last world and the
// reading of the seat there. It does so twice: on the game itself, whose
// turns keep their agents, and on a half of a split, which keeps none until
// its copy is asked to (`remember`).
//
//   node walkback.mjs --wasm <civvis.wasm> [--seed 7] [--from 20] [--turns 12]
//
// Exit status 1 if a walked line and the line played straight disagree.

import { readFileSync } from "node:fs";

function flag(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  return at === -1 || at + 1 >= process.argv.length ? fallback : process.argv[at + 1];
}

const wasm = flag("wasm", null);
if (!wasm) {
  console.error("usage: node walkback.mjs --wasm <civvis.wasm> [--seed N] [--from T] [--turns H]");
  process.exit(2);
}
const seed = Number(flag("seed", 7));
const from = Number(flag("from", 20));
const turns = Number(flag("turns", 12));
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const module = new WebAssembly.Module(readFileSync(wasm));

// One engine: a module instance and the one call its ABI has.
function engine() {
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

// A fork: a fresh instance given a copy of another's memory.
function fork(of) {
  const made = engine();
  const memory = of.exports.memory.buffer;
  const short = memory.byteLength - made.exports.memory.buffer.byteLength;
  if (short > 0) made.exports.memory.grow(short / 65536);
  new Uint8Array(made.exports.memory.buffer).set(new Uint8Array(memory));
  return made;
}

const must = (answer, what) => {
  if (answer.error) throw new Error(`${what}: ${answer.error}`);
  return answer;
};
// Who holds the seat on a turn of a line.
const who = (line, turn) => line.filter(([start]) => start <= turn).pop()[1];
// Where a line ended: the print of that world, and how the seat stands in it.
function reading(answer) {
  const debug = answer.debug || {};
  const print = (debug.worlds || [])[(debug.turns || []).indexOf(answer.turn)];
  return `${answer.turn} ${print} ${JSON.stringify(answer.seat)}`;
}

// As the page plays a line that was found (`debugPlayPlan`).
function paged(on, path, line, to) {
  let last = must(on.ask("POST", path, { op: "jump", turn: from, strategy: who(line, from), lean: true }), "walk");
  for (let turn = from; turn < to; turn++) {
    if (turn > from && who(line, turn) !== who(line, turn - 1)) {
      must(on.ask("POST", path, { op: "status", strategy: who(line, turn), lean: true }), "seat");
    }
    last = must(on.ask("POST", path, { op: "step", strategy: who(line, turn), lean: true }), "step");
  }
  return reading(last);
}

// As a copy plays it, from the turn `at` where it parts from the copy's last.
function searched(on, path, line, at, to) {
  let last = must(on.ask("POST", path, { op: "jump", turn: at, strategy: who(line, at), lean: true }), "walk back");
  if (last.turn !== at) throw new Error(`a walk back to turn ${at} landed on ${last.turn}`);
  let turn = at;
  while (turn < to) {
    let stop = to;
    for (const [start] of line) if (start > turn) { stop = Math.min(stop, start); break; }
    last = must(on.ask("POST", path, {
      op: "play", turns: Math.min(5, stop - turn), strategy: who(line, turn), lean: true,
    }), "play");
    turn += last.readings.length;
  }
  return reading(last);
}

function world() {
  const made = engine();
  must(made.ask("POST", "/new", {
    num_players: 6, spectate: false, seed, map_script: "continents", force: true,
  }), "the game");
  return made;
}
function played(on, path, to) {
  let turn = on.ask("POST", path, { op: "status", lean: true }).turn;
  while (turn < to) turn = must(on.ask("POST", path, { op: "step", strategy: "jack", lean: true }), "opening").turn;
}

const end = from + turns;
const third = from + Math.floor(turns / 3);
const later = from + Math.floor(2 * turns / 3);
const lines = {
  one: [[from, "jack"]],
  two: [[from, "jack"], [third, "moonshot"]],
  three: [[from, "jack"], [third, "moonshot"], [later, "jack"]],
  four: [[from, "moonshot"]],
  five: [[from, "siege"], [later, "jack"]],
  six: [[from, "siege"], [third, "holy"], [later, "jack"]],
};
// The copy's order: each line, and the turn it is walked back to.
const walks = [
  ["one", from], ["two", third], ["three", later], ["one", third], ["four", from],
  ["five", from], ["six", third], ["five", third], ["three", from],
];

const report = { seed, from, to: end };
let agree = true;
for (const where of ["the game itself", "a half of a split"]) {
  const root = world();
  let path = "/debug";
  if (where === "the game itself") played(root, path, from);
  else {
    // The half plays its last four turns itself: those turns keep no agents.
    played(root, path, from - 4);
    path = "/debug?pane=a-walkback";
    played(root, path, from);
  }
  const straight = {};
  for (const [name, line] of Object.entries(lines)) straight[name] = paged(fork(root), path, line, end);
  const copy = fork(root);
  must(copy.ask("POST", path, { op: "breakpoints", lean: true }), "breakpoints");
  must(copy.ask("POST", path, { op: "remember", lean: true }), "remember");
  const differ = [];
  for (const [name, at] of walks) {
    if (searched(copy, path, lines[name], at, end) !== straight[name]) differ.push(`${name} from turn ${at}`);
  }
  report[where] = {
    walks: walks.length,
    worlds: new Set(Object.values(straight)).size,
    agree: differ.length === 0,
    ...(differ.length ? { differ } : {}),
  };
  agree = agree && differ.length === 0;
}
console.log(JSON.stringify(report, null, 1));
if (!agree) {
  console.error("a line walked back to is not the line played straight");
  process.exit(1);
}
