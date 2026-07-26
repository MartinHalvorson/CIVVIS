# Roadmap

## v0.6 (shipped) — pure Rust

Python reference implementation removed (2026-07-21); the Rust crate is now
the single engine at full v0.5 rules parity, moved to the repo root, with the
GUI server, observation builder, and Elo harness all in Rust (serde-only
deps). External agents use the HTTP JSON protocol; in-process agents use the
`Ai` trait. This release also adds class-specific promotion trees, Corps/
Armies and linked escorts, theological combat and its religious-unit roster,
and independent Encampment defenses and ranged strikes.

## v0.1 (shipped)

Headless engine: hex map + mapgen, cities/growth/borders, districts with
adjacency, buildings/improvements, tech + civic trees, melee/ranged/city
combat, war/peace, three victory types, fog of war, JSON saves, gym-style
env, scripted AIs, CLI, tests.

## v0.2 (shipped)

City-states (pre-founded defensive minors, conquerable, excluded from
victory); `soak` command playing many full AI games across seeds with anomaly
flags — end-to-end games verified at 2-8 players, 100-200 turns.

## v0.3 (shipped) — Rust performance core

`rust/` crate ports the full engine (map/cities/districts/tech/combat/
city-states/AI/CLI) with the same embedded ruleset JSONs and action protocol.
~16x single-core over Python (36k vs 2.3k turns/sec), parallel across cores
with no GIL. Python engine remains the reference spec.

Next for the Rust core:
- PyO3 bindings (maturin) so Python agents/env drive the Rust engine
- Ruleset ID interning + yield caching (est. several-fold further speedup)
- Observation builder + fog in Rust for RL feature extraction

## v0.4 (shipped) — rules depth + browser GUI

Housing/amenities, eurekas & inspirations, unit XP/levels/fortify, city
ranged strikes, barbarian camps & raiders, governments, medieval/renaissance
content (29 techs, 14 civics), and `civvis play` — a zero-dep local web GUI
for human-vs-AI over the JSON action protocol. Rust core still at v0.3 rules;
batch-port these systems next.

## v0.6 rules-completion pass (shipped)

The previously deferred tactical and world systems are active: pillaging and
repairs, coastal raids, cliffs, aircraft basing/combat/interception/anti-air,
named Great People and patronage, complete belief categories and Apostle
promotions, named Governors and promotion trees, barbarian scout alerts,
multi-tile Natural Wonders, Golden/Dark/Heroic Ages and Dedications, bilateral
Quick Deals, grievances, formal wars, friendships, leveled Alliances,
Diplomatic Favor, World Congress voting, and keep/raze/liberate conquest
decisions. Future roadmap work is content expansion or client/tooling work,
not activation of dormant engine systems.

## Browser-local WebAssembly client (planned)

**Status:** approved direction, not started. Preserve the native CLI/server;
the browser client is an additional target, not a replacement.

The goal is a Paperclips-style deployment at `civvis.ai`: GitHub Actions tests
and builds one immutable static release, a CDN serves HTML/JavaScript/art/wasm,
and every game runs on the visitor's CPU inside the browser. The production URL
must always point to the newest commit for which the native suite, WebAssembly
build, deterministic parity checks, and real-browser smoke tests all passed. A
red or cancelled successor leaves the previous green deployment live.

### Feasibility snapshot

Measured on `864f0ce` on 2026-07-25; rerun these measurements before setting
final CI budgets because the game is still moving quickly:

- 20.69 MB tracked repository content; 118,909 raw lines of Rust.
- 11.67 MB of current web files, including 10.60 MB of PNG atlases. Because
  every atlas is currently assigned to an `Image.src` at startup, the present
  static first load is about 10.9 MB after text compression.
- 21.72 MB optimized native executable, 20.40 MB stripped, 15.03 MB gzipped.
  It embeds the UI and all art, so it is a useful upper-scale comparison, not
  the future `.wasm` size.
- A six-player 250-turn game took 9.27 seconds and about 35 MB peak resident
  memory on an M5 Max. Browser and low-power-device performance still require
  measurement; do not present this native result as a browser guarantee.
- A complete save was 1.79 MB raw / 30 KB gzipped. A full spectator observation
  was 1.92 MB raw / 43 KB gzipped. Compression makes HTTP cheap, but repeatedly
  structured-cloning the raw object across a Worker boundary would not be.

The core is a good WebAssembly candidate: `game`, `ai`, `mapgen`, `obs`,
`rules`, `rng`, and `setup` do not depend on networking, files, OS clocks, or
OS threads in their main paths; the rules are embedded; randomness is seeded;
and `Game` already round-trips through serde. Native coupling is concentrated
in `server`, the CLI, league/rating persistence, evolution/self-play output,
parallel multi-game runners, mods, and action logging. There is no existing
wasm target, binding, Worker, browser storage layer, or browser CI yet.

### Target architecture

```text
GitHub main commit
  -> native tests + wasm build + parity/browser/size gates
  -> immutable static artifact stamped with the exact Git SHA
  -> static CDN at civvis.ai
  -> UI/main thread <-> dedicated Worker <-> Rust/wasm Game + AIs
                                      `-> IndexedDB/local export saves
```

- Keep rendering and DOM work on the main thread. Run the game and AI in a
  dedicated Worker so a long AI turn cannot freeze input or animation.
- Preserve the JSON action/state contract. Replace the page's single
  `fetchJSON` implementation with a transport interface: HTTP for the native
  `civvis play` server and Worker messages for the hosted client. Do not fork
  the 20k-line UI or couple it directly to engine internals.
- Extract or build a browser-sized session layer around `Game`; do not port the
  TCP listener, supervisor handoffs, host health endpoints, filesystem league
  recording, or native multi-game worker fleet.
- Preserve the existing tile-baseline/patch protocol. Bound simulation batches
  so pause/cancel messages are serviced, transfer encoded buffers where that is
  measurably better, and publish fewer render snapshots during flat-out
  headless runs. Never copy a full ~2 MB spectator observation every turn by
  default.
- Store autosaves and named saves in IndexedDB and retain JSON import/export.
  Stamp every save with the engine version and build SHA. A running tab stays
  on the code it loaded; announce a newer green build and reload only after the
  player has saved.
- Keep assets same-origin. Content-hash immutable wasm/JS/art; revalidate the
  small HTML/version manifest. Lazy-load cinematic and wonder atlases so a
  strategic-map player does not pay the entire art download at startup.

### Implementation sequence

1. **Platform boundary.** Add `native`/`web` Cargo features or target gates.
   Keep the portable engine available without compiling `server`, CLI, file
   persistence, league runners, or native parallelism. Add a wasm compile check
   without weakening the native build.
2. **Browser session crate.** Add a small `cdylib` wrapper using
   `wasm-bindgen`, exporting new game, state, action, route, step, autoplay,
   save/load, rules, and pedia operations. Use the same engine methods and
   serde formats as native CIVVIS.
3. **Deterministic parity harness.** Feed identical setup/actions to native and
   wasm builds and compare normalized serialized state at checkpoints across
   multiple seeds. Any platform-only nondeterminism blocks deployment.
4. **Worker transport.** Implement request IDs and typed success/error replies,
   then route the existing UI endpoint calls through HTTP or Worker transport.
   Keep server and hosted modes behaviorally aligned where the feature exists;
   return explicit capability information for native-only features.
5. **Scheduling and persistence.** Port single-game AI stepping and map patches,
   add IndexedDB autosaves/import/export, and handle build-update prompts. One
   game may use one Worker initially; parallel browser tournaments can use
   multiple Workers only after memory and throttling tests.
6. **Performance and delivery.** Measure Chromium, Firefox, and WebKit on desktop
   and at least one constrained/mobile device. Optimize release wasm, lazy-load
   art, and set CI budgets from measurements. Planning targets are a `.wasm`
   asset below 25 MiB and a roughly 15-20 MB or smaller first load, not promises.
7. **Green-only deployment.** On pull requests, build and smoke-test without
   production credentials. On a `main` push, deploy the exact tested artifact
   only after every required job succeeds. Include `version.json` and expose
   the abbreviated SHA in the UI. Keep the prior deployment available for
   rollback.

### Production acceptance gates

- Existing native CLI, local server, JSON clients, saves, and deterministic
  outcomes remain supported and pass the locked release/CI suite.
- A browser can start, play, spectate, autoplay, save, reload, export, and
  import a representative game without a simulation API call to the host.
- Native/wasm parity passes across fixed seeds and action traces; browser tests
  cover at least Chromium, Firefox, and WebKit.
- A standard six-player browser game has documented load size, peak memory,
  turns/second, long-task behavior, and state-transfer volume. Mobile limits
  are explicit rather than silently hanging or exhausting memory.
- Production is static-only by default. Multiplayer, trusted tournament
  rankings, shared accounts/saves, and simulations that continue after a tab
  closes require an authoritative backend and are separate projects. Results
  uploaded from a local browser are untrusted.
- Static hosting should use a CDN without GitHub Pages' traffic ceiling;
  GitHub remains the source/build authority. Cloudflare Pages is the current
  candidate, but the green-artifact contract must remain host-independent.

## v0.3 — systems breadth

- Religion (pantheons, beliefs, religious combat)
- Great people; trade routes; city-states + envoys
- Expand diplomacy beyond the shipped economic/relationship deals
- Per-civ unique abilities/units (data-driven, like everything else)
- Era score / golden ages

## v0.4 — clients

- Web client (canvas hex renderer) speaking the JSON action protocol to a
  local server wrapper around `Game`
- Terminal TUI client
- Multiplayer via the same protocol (engine is already lockstep-friendly)

## v0.5 — mod ecosystem

- Ruleset validation + mod loader (multiple data dirs, overrides)
- Full Civ 6 base-game content pass in `data/`

## AI track (parallel)

- PettingZoo-style multi-agent wrapper
- Action-masked observation tensors for RL
- MCTS baseline using dict-state cloning
- Seeded tournament harness + Elo for agent evaluation
