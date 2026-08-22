// Play one full headless CIVVIS game and describe it as JSON.
//
// The same driving loop runs against either build, because both answer the
// same requests:
//
//   --arm wasm   the wasm32 module, instantiated in this process. Its protocol
//                is the one `beta/worker.js` speaks — UTF-8 JSON in, UTF-8
//                JSON out, with the answer's byte length as a little-endian
//                u32 in front. It imports nothing.
//   --arm rust   a native `civvis play` server, over HTTP.
//
// Driving both arms through one loop is the point. `POST /new` and `POST
// /step` reach the same `Session` in both builds, so the same seed has to
// produce the same game — and where it does not, the difference is wasm32
// itself (32-bit `usize`, a different allocator) rather than two harnesses
// that asked for different worlds.
//
//   node sim.mjs --arm wasm --wasm target/…/civvis.wasm --seed 7
//   node sim.mjs --arm rust --base http://127.0.0.1:8811 --seed 7

import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";

function flag(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  return at === -1 || at + 1 >= process.argv.length ? fallback : process.argv[at + 1];
}

const arm = flag("arm", "wasm");
const seed = Number(flag("seed", 1));
const players = Number(flag("players", 6));
const speed = flag("speed", "online");
const difficulty = flag("difficulty", "prince");
const maxTurns = Number(flag("max-turns", 0));
const mapScript = flag("map-script", "continents");
const mapTopology = flag("map-topology", "flat");
const mapPoles = flag("map-poles", "poles");
const startEra = flag("start-era", "ancient");
// One request steps this many seats. A turn is one step per seat, so a batch
// well above the roster keeps the call boundary out of the measurement without
// hiding which turn the engine is on.
const batch = Number(flag("batch", 256));

// ---------------------------------------------------------------- transports

let ask; // (method, path, body) -> parsed answer
let armDetail = {};

if (arm === "wasm") {
  const wasmPath = flag("wasm", null);
  if (!wasmPath) throw new Error("--arm wasm needs --wasm <file>");
  const bytes = readFileSync(wasmPath);
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const engine = instance.exports;
  armDetail.wasm_bytes = bytes.length;

  // Read a length-prefixed answer out of the module's memory and free it.
  const take = (ptr) => {
    const length = new DataView(engine.memory.buffer).getUint32(ptr, true);
    const out = new Uint8Array(engine.memory.buffer.slice(ptr + 4, ptr + 4 + length));
    engine.civvis_free(ptr, 4 + length);
    return out;
  };

  // A Rust panic on wasm32 aborts, and all the caller sees is `unreachable`
  // and a list of function indices. The module records the message before it
  // dies and its memory survives the trap, so the diagnosis is still there.
  armDetail.lastPanic = () => {
    try {
      return new TextDecoder().decode(take(engine.civvis_last_panic())) || null;
    } catch {
      return null;
    }
  };
  armDetail.memory = () => engine.memory.buffer.byteLength / 1048576;

  ask = (method, path, body) => {
    const encoded = new TextEncoder().encode(
      JSON.stringify({ method, path, body: body ? JSON.stringify(body) : "", seed }),
    );
    const inPtr = engine.civvis_alloc(encoded.length);
    // Every view is taken *after* the call that might have grown the module's
    // memory: growth detaches the old ArrayBuffer, and a view built before it
    // reads zeroes.
    new Uint8Array(engine.memory.buffer, inPtr, encoded.length).set(encoded);
    return JSON.parse(new TextDecoder().decode(take(engine.civvis_request(inPtr, encoded.length))));
  };
} else if (arm === "rust") {
  const base = flag("base", "http://127.0.0.1:8765");
  armDetail.base = base;
  ask = async (method, path, body) => {
    const response = await fetch(`${base}${path}`, {
      method,
      ...(body ? { headers: { "content-type": "application/json" }, body: JSON.stringify(body) } : {}),
    });
    const text = await response.text();
    if (!response.ok) throw new Error(`${method} ${path} -> ${response.status}: ${text.slice(0, 300)}`);
    return JSON.parse(text);
  };
} else {
  throw new Error(`unknown --arm ${arm}`);
}

// --------------------------------------------------------------------- drive

const report = {
  arm, seed, players, speed, difficulty,
  map_script: mapScript, map_topology: mapTopology, map_poles: mapPoles,
  ok: false, turn: 0, winner: null, requests: 0,
};
if (armDetail.wasm_bytes) report.wasm_bytes = armDetail.wasm_bytes;
if (armDetail.base) report.base = armDetail.base;

const started = process.hrtime.bigint();
const cpuStarted = process.cpuUsage();
try {
  // Every setting is named, none inherited. `new_game_params` starts from
  // whatever the process is already playing, and the two arms boot differently
  // — the module opens on its own exhibition, the native server on its command
  // line — so anything left unsaid is a difference between the arms rather
  // than between the builds. Naming them all is what makes the same seed mean
  // the same game.
  const opened = await ask("POST", "/new", {
    num_players: players,
    seed,
    speed,
    difficulty,
    base_ruleset: "civ6",
    start_era: startEra,
    map_script: mapScript,
    map_topology: mapTopology,
    map_poles: mapPoles,
    leader_pool: "civ6",
    civs: [],
    victory_conditions: {
      science: true,
      culture: true,
      religious: true,
      diplomatic: true,
      domination: true,
      score: true,
    },
    spectate: true,
    // The driver owns the clock. A native server runs its own stepper thread
    // for any spectated game; pausing parks it so both arms are advanced by
    // exactly these `/step` calls and nothing else. `/step` itself does not
    // consult the pause on either build.
    paused: true,
    // A spectated game in progress is not replaceable by accident — an old
    // page whose result timer survived a handoff must not reset a healthy
    // world. A harness that exists to replace it says so.
    force: true,
    ...(maxTurns > 0 ? { max_turns: maxTurns } : {}),
  });
  if (opened.error) throw new Error(`/new refused: ${opened.error}`);
  report.turn_limit = opened.max_turns ?? null;
  report.map = opened.map ? { width: opened.map.width, height: opened.map.height } : null;
  // The world before anybody plays it. When two arms disagree this is the
  // question worth answering first, and answering it later costs a rerun: a
  // divergence whose maps already differ is mapgen, and one whose maps match
  // is everything downstream of it. Hashing a megabyte is milliseconds against
  // a forty-second game.
  // Which agent actually sat down. #1094 gave the wasm module a shipped league
  // roster to seat from, and the native binary has none unless `--league` says
  // so — so the two builds can now play the same world with *different AI*.
  // That is a difference between the arms, not between the builds, and pairing
  // across it would report the engine diverging when the players differ.
  try {
    const rules = await ask("GET", "/rules");
    report.seat_strategy = rules.seat_strategy ?? null;
  } catch {
    report.seat_strategy = null;
  }
  report.map_digest = createHash("sha256")
    .update(JSON.stringify(opened.map ?? null))
    .digest("hex")
    .slice(0, 16);
  report.starts_digest = createHash("sha256")
    .update(JSON.stringify((opened.players ?? []).filter((p) => !p.is_minor).map((p) => [p.civ, p.capital])))
    .digest("hex")
    .slice(0, 16);

  // Not a policy, only a promise that a game which somehow never finishes ends
  // this process rather than running until the tab closes.
  const cap = 200_000;
  let last = opened;
  let stalled = 0;
  for (let i = 0; i < cap; i += 1) {
    const before = report.turn;
    last = await ask("POST", "/step", { count: batch });
    report.requests += 1;
    if (last.error) throw new Error(`/step refused: ${last.error}`);
    report.turn = last.turn ?? report.turn;
    if (last.winner !== null && last.winner !== undefined) break;
    if (report.turn_limit && report.turn >= report.turn_limit) break;
    // A batch of 256 seat-steps that did not advance the turn once is a wedged
    // engine, not a slow one. Say so rather than spinning to the cap.
    stalled = report.turn === before ? stalled + 1 : 0;
    if (stalled >= 8) throw new Error(`the engine stopped advancing at turn ${report.turn}`);
  }

  const status = await ask("GET", "/status");
  report.turn = status.turn ?? report.turn;
  report.winner = status.winner ?? null;
  report.commit = status.commit ?? null;
  report.victory = last.victory_type ?? null;
  // The standings are what make two arms comparable: same seed, same scores.
  report.scores = (last.players ?? [])
    .filter((p) => !p.is_minor && !p.is_barbarian)
    .map((p) => ({
      id: p.id ?? null,
      civ: p.civ ?? null,
      score: p.score ?? null,
      cities: p.cities ?? null,
      techs: Array.isArray(p.techs) ? p.techs.length : (p.techs ?? null),
      eliminated: p.alive === false,
    }));
  report.ok = true;
} catch (error) {
  report.error = String((error && error.message) || error);
  const panic = armDetail.lastPanic?.();
  if (panic) report.panic = panic;
}
report.seconds = Number(process.hrtime.bigint() - started) / 1e9;
report.turns_per_second = report.turn > 0 ? Number((report.turn / report.seconds).toFixed(3)) : 0;
// Wall-clock cannot tell "the build got slower" from "the box got busier", and
// this box runs two live Civ 6 games and several agents besides — the same
// seed, producing the identical game, measured 4.294 turns/s at load 6 and
// 2.972 at load 24. CPU time is what the engine actually spent.
//
// For the wasm arm the engine *is* this process, so its CPU is the engine's.
// The rust arm's engine is the server, and its CPU is read from `ps` by the
// caller, which is why nothing is claimed here for it.
if (arm === "wasm") {
  const spent = process.cpuUsage(cpuStarted);
  report.cpu_seconds = Number(((spent.user + spent.system) / 1e6).toFixed(3));
}
if (armDetail.memory) report.peak_wasm_mib = Number(armDetail.memory().toFixed(1));

console.log(JSON.stringify(report, null, 2));
process.exit(report.ok ? 0 : 1);
