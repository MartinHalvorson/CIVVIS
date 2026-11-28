#!/usr/bin/env node
// The first world a visit draws, played ahead of time.
//
// On a first visit the engine is ~3.2 MB of the ~4 MB the browser fetches,
// and nothing could be drawn until it had arrived, compiled and answered:
// the page sat on its loading card for the length of the download. Yet the
// answers the page needs for its first frame are fixed by the build and the
// world's seed — the engine imports nothing and is deterministic per seed —
// so this script asks the lane's own `civvis.wasm` for them now, for a small
// bank of seeds, exactly as the page would. `shim.js` picks one of these
// seeds for a first visit, draws its world from the baked answers while the
// engine downloads, and hands over to the engine, which has been asked the
// very same requests in the very same order, the moment it is ready.
//
// What is baked, per seed, is one gzip file of three answers:
//   state         GET /state?painted=&viewer=…&think=0&planet=1 (the opening read;
//                 the viewer id does not change the answer)
//   rules         GET /rules (the Future Era tree and the seat strategy vary by world)
//   gene_program  GET /gene-program
// asked after the requests the page makes before them (`/runtime`, the shim's
// opening `/pace`, `/machine-metrics`), so the engine is in the state it will
// be in for the visitor. The only settings the opening `/pace` carries are
// the pace and the finale hold; the answer is baked at the published site's
// defaults, and the shim uses the bank only when the visit would send those.
//
// usage: node bake_openings.mjs <lane dir> <salt> [count]
//
// The lane is changed in place: `assets/openings/<seed>.json.gz` is written,
// and the `OPENING_BANK = null` placeholder in `shim.js` becomes the bank
// (seeds and the content hash each file is asked for by). A lane whose shim
// has no placeholder — a pinned revision from before this — is left alone.
// Gzip rather than brotli because the file is decoded by the page itself
// (`DecompressionStream`), so it reads the same from any static host:
// Cloudflare, the desktop app's local server, the publish check's.

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";

const [lane, salt, countArg] = process.argv.slice(2);
if (!lane || !salt) {
  console.error("usage: node bake_openings.mjs <lane dir> <salt> [count]");
  process.exit(2);
}
const count = Math.max(1, Math.min(32, Number(countArg || 8) || 8));
const shimPath = path.join(lane, "shim.js");
const wasmPath = path.join(lane, "civvis.wasm");
const PLACEHOLDER = "const OPENING_BANK = null;";

const shim = fs.readFileSync(shimPath, "utf8");
if (!shim.includes(PLACEHOLDER)) {
  console.log("   this revision's shim takes no baked openings; none written");
  process.exit(0);
}
// The opening settings the published site sends when the visitor has chosen
// none. Read from the shim itself so the two cannot drift apart.
const constant = (name) => {
  const found = new RegExp(`const ${name} = (\\d+);`).exec(shim);
  if (!found) throw new Error(`shim.js no longer defines ${name}`);
  return Number(found[1]);
};
const settings = {
  ms: constant("PUBLISHED_SITE_PACE_MS"),
  between_game_countdown_ms: constant("DEFAULT_FINALE_MS"),
};

const module = await WebAssembly.compile(fs.readFileSync(wasmPath));

async function engine() {
  const exports = (await WebAssembly.instantiate(module, {})).exports;
  return (method, target, body, seed) => {
    const asked = new TextEncoder().encode(JSON.stringify({ method, path: target, body: body || "", seed }));
    const at = exports.civvis_alloc(asked.length);
    new Uint8Array(exports.memory.buffer, at, asked.length).set(asked);
    const answer = exports.civvis_request(at, asked.length);
    const length = new DataView(exports.memory.buffer).getUint32(answer, true);
    const bytes = Buffer.from(exports.memory.buffer.slice(answer + 4, answer + 4 + length));
    exports.civvis_free(answer, 4 + length);
    return bytes;
  };
}

// The bank's seeds come from the build, so every build shows new worlds and
// rebuilding one revision bakes the same ones.
function seedFor(index) {
  const digest = crypto.createHash("sha256").update(`${salt}:opening:${index}`).digest();
  return (digest.readUInt32LE(0) % 0xffffffff) + 1;
}

const folder = path.join(lane, "assets", "openings");
fs.rmSync(folder, { recursive: true, force: true });
fs.mkdirSync(folder, { recursive: true });
const seeds = {};
let largest = 0;
const began = Date.now();
for (let index = 0; index < count; index++) {
  const seed = seedFor(index);
  if (Object.hasOwn(seeds, seed)) continue;
  const ask = await engine();
  ask("GET", "/runtime", "", seed);
  ask("POST", "/pace", JSON.stringify(settings), seed);
  ask("GET", "/machine-metrics", "", seed);
  const geneProgram = ask("GET", "/gene-program", "", seed);
  const state = ask("GET", "/state?painted=&viewer=baked&think=0&planet=1", "", seed);
  const rules = ask("GET", "/rules", "", seed);
  for (const [name, bytes] of [["state", state], ["rules", rules], ["gene_program", geneProgram]]) {
    const parsed = JSON.parse(bytes.toString("utf8"));
    if (!parsed || typeof parsed !== "object" || parsed.error) {
      throw new Error(`seed ${seed}: the engine's ${name} answer is not a world (${bytes.toString("utf8").slice(0, 120)})`);
    }
  }
  const whole = Buffer.concat([
    Buffer.from('{"state":'), state, Buffer.from(',"rules":'), rules,
    Buffer.from(',"gene_program":'), geneProgram, Buffer.from("}"),
  ]);
  const packed = zlib.gzipSync(whole, { level: 9 });
  fs.writeFileSync(path.join(folder, `${seed}.json.gz`), packed);
  seeds[seed] = crypto.createHash("sha256").update(packed).digest("hex").slice(0, 16);
  largest = Math.max(largest, packed.length);
}

// The pace the engine keeps when the shim names none — what a visit off the
// published site, which sends only the finale hold, opens at.
const fresh = await engine();
const firstSeed = Number(Object.keys(seeds)[0]);
fresh("GET", "/runtime", "", firstSeed);
const enginePace = JSON.parse(fresh("GET", "/state?painted=&viewer=baked&think=0&planet=1", "", firstSeed).toString("utf8")).pace;
if (typeof enginePace !== "number") throw new Error("the engine's state no longer says its pace");

const bank = { settings, engine_pace: enginePace, seeds };
fs.writeFileSync(shimPath, shim.replace(PLACEHOLDER, `const OPENING_BANK = ${JSON.stringify(bank)};`));
console.log(`   ${Object.keys(seeds).length} openings baked in ${((Date.now() - began) / 1000).toFixed(1)} s, ` +
  `largest ${largest.toLocaleString("en-US")} bytes`);
