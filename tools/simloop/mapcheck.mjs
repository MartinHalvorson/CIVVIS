// Does the world differ before anybody plays it?
//
// A game that diverges could diverge anywhere — mapgen, the AI, combat. This
// asks only the first question: hand both builds the same setup, generate the
// world, and hash it before a single turn is taken. If the hashes differ, the
// difference is in the map and nothing downstream needs looking at yet.
//
//   node mapcheck.mjs --arm wasm --wasm <file> --seed 4242 --map-topology planet
//   node mapcheck.mjs --arm rust --base http://127.0.0.1:PORT --seed 4242 …

import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";

function flag(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  return at === -1 || at + 1 >= process.argv.length ? fallback : process.argv[at + 1];
}

const arm = flag("arm", "wasm");
const seed = Number(flag("seed", 1));
const players = Number(flag("players", 6));
const mapTopology = flag("map-topology", "flat");
const mapScript = flag("map-script", "continents");
const startEra = flag("start-era", "ancient");

let ask;
if (arm === "wasm") {
  const bytes = readFileSync(flag("wasm", null));
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const e = instance.exports;
  const take = (ptr) => {
    const len = new DataView(e.memory.buffer).getUint32(ptr, true);
    const out = new Uint8Array(e.memory.buffer.slice(ptr + 4, ptr + 4 + len));
    e.civvis_free(ptr, 4 + len);
    return out;
  };
  ask = (method, path, body) => {
    const enc = new TextEncoder().encode(
      JSON.stringify({ method, path, body: body ? JSON.stringify(body) : "", seed }),
    );
    const p = e.civvis_alloc(enc.length);
    new Uint8Array(e.memory.buffer, p, enc.length).set(enc);
    return JSON.parse(new TextDecoder().decode(take(e.civvis_request(p, enc.length))));
  };
} else {
  const base = flag("base", "http://127.0.0.1:8765");
  ask = async (method, path, body) => {
    const r = await fetch(`${base}${path}`, {
      method,
      ...(body ? { headers: { "content-type": "application/json" }, body: JSON.stringify(body) } : {}),
    });
    return JSON.parse(await r.text());
  };
}

const opened = await ask("POST", "/new", {
  num_players: players, seed, speed: "online", difficulty: "prince",
  base_ruleset: "civ6", start_era: startEra,
  map_script: mapScript, map_topology: mapTopology, map_poles: "poles",
  leader_pool: "civ6", civs: [],
  victory_conditions: { science: true, culture: true, religious: true,
                        diplomatic: true, domination: true, score: true },
  spectate: true, paused: true, force: true,
});
if (opened.error) throw new Error(opened.error);

const map = opened.map ?? {};
const digest = (value) => createHash("sha256").update(JSON.stringify(value ?? null)).digest("hex").slice(0, 16);

// Hashed field by field, so a difference names which part of the world moved
// rather than only that one did.
const out = {
  arm, seed, topology: mapTopology,
  width: map.width, height: map.height,
  whole_map: digest(map),
};
for (const field of ["tiles", "terrain", "elevation", "rivers", "resources", "features", "planet"]) {
  if (map[field] !== undefined) out[`map.${field}`] = digest(map[field]);
}
// Where the six empires were put is the most sensitive readout of all: start
// siting reads the generated world and a single reclassified tile moves it.
out.starts = digest(
  (opened.players ?? []).filter((p) => !p.is_minor).map((p) => [p.civ, p.capital, p.cities]),
);

// `--dump <file>` writes the world itself, so two builds can be compared tile
// by tile rather than only hash against hash. "The maps differ" is true of one
// reclassified tile and of a completely different continent, and those are not
// the same bug to fix.
const dump = flag("dump", null);
if (dump) {
  const { writeFileSync } = await import("node:fs");
  writeFileSync(dump, JSON.stringify({ map, players: opened.players ?? [] }));
  out.dumped = dump;
}

console.log(JSON.stringify(out, null, 2));
