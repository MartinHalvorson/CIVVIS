// Does a saved world come back the same world?
//
// Both builds answer `GET /save` and `POST /load`, and nothing has ever
// exercised either. The property tested is that **the world** survives the
// round trip: save at turn N, load it back, and the board, the empires, their
// cities and their scores must be exactly what they were.
//
// ⚠ Deliberately NOT tested: that a reloaded game plays on identically. It
// does not, and that is by design — `Session::from_game` builds a fresh AI
// fleet, and `server.rs` says so outright: "A save carries the world, not what
// anyone was thinking while they played it." Measured on seed 1036, carrying
// straight on ended at turn 202 and resuming from the save ended at 206. A
// check that called that a failure would report one every time it ran, which
// is worse than not checking at all.
//
//   node saveload.mjs --arm wasm --wasm <file> --seed 7 --save-at 60
//   node saveload.mjs --arm rust --base http://127.0.0.1:PORT --seed 7

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
const mapScript = flag("map-script", "continents");
const mapTopology = flag("map-topology", "flat");
const mapPoles = flag("map-poles", "poles");
const startEra = flag("start-era", "ancient");
const saveAt = Number(flag("save-at", 60));
const batch = Number(flag("batch", 64));
// Cross-build save compatibility. One arm writes its save out; the other reads
// it back and rebuilds the world from it. A save that only loads in the build
// that wrote it is not a save format, and this is the one property of these two
// builds that a person would actually hit — the desktop build and civvis.ai
// are meant to be the same program.
const emitSave = flag("emit-save", null);
const loadSave = flag("load-save", null);

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

const SETUP = {
  num_players: players, seed, speed, difficulty,
  base_ruleset: "civ6", start_era: startEra,
  map_script: mapScript, map_topology: mapTopology, map_poles: mapPoles,
  leader_pool: "civ6", civs: [],
  victory_conditions: { science: true, culture: true, religious: true,
                        diplomatic: true, domination: true, score: true },
  spectate: true, paused: true, force: true,
};

const digest = (v) => createHash("sha256").update(JSON.stringify(v ?? null)).digest("hex").slice(0, 16);

// The substance of a world, as digests: what a save is supposed to carry.
// Not the whole `/state` blob, which also holds the reasoning journal and
// other things a reload is entitled to start afresh.
function worldOf(state) {
  return {
    turn: state.turn ?? null,
    map: digest(state.map ?? null),
    empires: digest((state.players ?? []).map((p) => [
      p.id, p.civ, p.score, p.cities, p.gold, p.faith,
      Array.isArray(p.techs) ? p.techs.length : p.techs,
      Array.isArray(p.civics) ? p.civics.length : p.civics,
      p.alive, p.is_minor,
    ])),
    cities: digest(state.cities ?? null),
    units: digest(state.units ?? null),
  };
}

const report = { arm, seed, save_at: saveAt, ok: false };
const started = process.hrtime.bigint();
try {
  let state = await ask("POST", "/new", SETUP);
  if (state.error) throw new Error(`/new refused: ${state.error}`);
  const limit = state.max_turns ?? null;

  // To the save point.
  while ((state.turn ?? 0) < saveAt && state.winner === null) {
    state = await ask("POST", "/step", { count: batch });
    if (state.error) throw new Error(`/step refused: ${state.error}`);
  }
  report.saved_at_turn = state.turn;
  const before = worldOf(state);
  const save = await ask("GET", "/save");
  if (!save || save.error) throw new Error(`/save refused: ${save && save.error}`);
  report.save_digest = digest(save);
  report.save_bytes = JSON.stringify(save).length;

  if (emitSave) {
    const { writeFileSync } = await import("node:fs");
    writeFileSync(emitSave, JSON.stringify({ seed, save_at: saveAt, world: before, game: save }));
    report.emitted = emitSave;
  }

  const reloaded = await ask("POST", "/load", { game: save });
  if (reloaded.error) throw new Error(`/load refused: ${reloaded.error}`);
  const after = worldOf(reloaded);

  report.differing = Object.keys(before).filter((f) => before[f] !== after[f]);
  report.round_trips = report.differing.length === 0;
  if (!report.round_trips) {
    report.before = before;
    report.after = after;
  }

  // The other build's save, if one was left for us. Compared against the world
  // *that build* recorded at the moment it saved, not against this run's own —
  // the two arms play the same game, so the two must agree.
  if (loadSave) {
    const { readFileSync, existsSync } = await import("node:fs");
    if (existsSync(loadSave)) {
      const theirs = JSON.parse(readFileSync(loadSave, "utf8"));
      if (theirs.seed !== seed) {
        report.cross_build = "skipped: that save is from another seed";
      } else {
        const back = await ask("POST", "/load", { game: theirs.game });
        if (back.error) {
          report.cross_build = "refused";
          report.cross_build_error = back.error;
        } else {
          const mine = worldOf(back);
          const differs = Object.keys(theirs.world).filter((f) => theirs.world[f] !== mine[f]);
          report.cross_build = differs.length === 0 ? "ok" : "differs";
          if (differs.length) {
            report.cross_build_differing = differs;
            report.cross_build_theirs = theirs.world;
            report.cross_build_mine = mine;
          }
        }
      }
    }
  }
  report.ok = true;
} catch (error) {
  report.error = String((error && error.message) || error);
}
report.seconds = Number(process.hrtime.bigint() - started) / 1e9;
console.log(JSON.stringify(report, null, 2));
const crossOk = !report.cross_build || report.cross_build === "ok"
  || String(report.cross_build).startsWith("skipped");
process.exit(report.ok && report.round_trips && crossOk ? 0 : 1);
