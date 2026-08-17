# Roadmap

Where the project actually is, and what it is doing next. History below is
kept for orientation; the current-state section is the part to trust, and
`docs/AI_GAPS.md` is the always-current assessment of the AI specifically.

## Where the project is (2026-08-17)

Everything the old roadmap called planned has shipped and then some:

- **The engine** is a single pure-Rust crate (serde-only deps) at full rules
  depth — religion, governors, ages, World Congress, aircraft, alliances,
  unique units, the lot. The rules-completion pass closed 2026-07; remaining
  engine work is fidelity against real Civilization VI (`docs/FIDELITY.md`),
  not activation of dormant systems.
- **civvis.ai is live**: the WebAssembly client shipped, with a `/test` lane
  redeployed from head half-hourly, a stable front page moved by operator
  judgment (`docs/SPECTATOR_DEPLOY.md`), native/wasm build-parity gates, and
  a home page selling two products — full-game simulations and Tactics
  battles (historical scenarios on real terrain, an era rolled per battle).
- **The AI is scripted and measured**: `AdvancedAi` plus a league of bred
  genome variants, rated by a Glicko-2 selection league, priced by a paired
  evaluator at the deployment shape, and published in batch by `civvis arena`
  (anchored Elo; standardized table size). No learned policy ships; search
  wins offline but is not live-eligible. `docs/AI_GAPS.md` ranks the gaps.
- **The live bridge plays real Civilization VI**: a Lua control mod + macOS
  harness drives full Settler-difficulty games end to end, self-records every
  attempt on the difficulty ladder (`docs/CIV6_LADDER.md`), and carries its
  bridge health (orders-applied rate, ~97%) on the ledger. **Rung 1, Settler,
  was claimed on 2026-08-16** by run `civvis-20260816T054344Z` — a victory
  event naming our own team at turn 251 of a configured 250-turn game, score
  1021. A second Settler win followed the same day (`civvis-20260816T223457Z`,
  1121 against a best rival's 1031). Winning is not yet reliable: of the 119
  attempts since 2026-08-10, 70 reached the turn cap and two won. This is the
  project's front line.

## Active objectives (ranked 2026-08-17)

1. **Make Settler repeatable, then take Chieftain.** The rung is claimed; two
   wins in 119 attempts is a result, not a capability. The next milestone is
   a win rate that survives a batch, and the rung above it.
2. **Close the actuation gap.** Applied-rate floored on the ledger; envoy
   spending and the built-in production ladder's ~27% share are the open
   holes.
3. **Price the shipped live-seat bundle by withholding.** `live_without_*`
   arms exist for every withholdable treatment; run the unpriced ones
   through the paired evaluator before the next `city_target_floor` hides
   in a composite (`docs/EVAL.md` is the ledger).
4. **A tactics-grade controller for the arena.** Bounded search on the
   20×20 battlefield, measured on the skirmish benchmark — the one live
   surface where search's cost objection collapses.
5. **Split the three conflict hotspots** (`src/game.rs`, `src/ai/advanced.rs`,
   `web/assets/app.js`) along existing seams; they tax every concurrent PR.
6. **Delete measured-null code.** ✅ The 2026-08-17 cleanup removes the
   confirmed-null `bounded_recovery` and `envoy_infrastructure` arms from
   production while retaining explicit evaluator/live-bridge controls and
   their negative records. The remaining off-flags and netless experiment arms
   are still queued for their own evidence-backed cleanup.
7. **wasm/native viewer parity.** Panels that read native-only state are
   silently dead on civvis.ai; implement or hide, and gate the contract.
8. **Headless empire actuation repairs** (housing/loyalty cards, eureka
   asks) — screen first, then gate at the deployment shape.
9. **Drain the stranded-work queue** (`tools/stranded_work_report.py`).
10. **Keep the paper trail true** — this file, retired docs to
    `docs/closed/`, generated ledgers current.

The measurement doctrine behind that ordering, in one line each: actuation
repairs pay and valuation tunes do not; a composite gate licenses the
composite, never its parts; gate on the deployment shape; one seed is never
a result; `audit` detects defects but does not estimate value.

## History (shipped)

### v0.1 — headless engine
Hex map + mapgen, cities/growth/borders, districts with adjacency,
buildings/improvements, tech + civic trees, melee/ranged/city combat,
war/peace, three victory types, fog of war, JSON saves, gym-style env,
scripted AIs, CLI, tests.

### v0.2 — soak
City-states (pre-founded defensive minors, conquerable, excluded from
victory); `soak` playing many full AI games across seeds with anomaly flags.

### v0.3 — Rust performance core
Ported the Python engine to Rust: ~16x single-core (36k vs 2.3k turns/sec),
parallel across cores. (The once-planned PyO3 bindings eventually became
unnecessary: the Python engine was removed instead.)

### v0.4 — rules depth + browser GUI
Housing/amenities, eurekas & inspirations, unit XP/fortify, city strikes,
barbarians, governments, medieval/renaissance content, and `civvis play` —
a zero-dep local web GUI over the JSON action protocol.

### v0.5 — content and systems breadth
Religion, Great People, trade routes, envoys, expanded diplomacy, per-civ
uniques, era score; ruleset data pass.

### v0.6 — pure Rust, rules completion
Python reference implementation removed (2026-07-21); the crate moved to the
repo root with GUI server, observation builder, and Elo harness all in Rust.
The completion pass activated every deferred tactical and world system:
pillaging/repairs, coastal raids, cliffs, aircraft, named Great People and
Governors, belief categories and Apostle promotions, Ages and Dedications,
Quick Deals, grievances, formal wars, alliances, Diplomatic Favor, World
Congress, conquest decisions.

### The browser client (shipped)
What an earlier revision of this file scoped as "planned" is the live site:
a `wasm32-unknown-unknown` build of the engine behind a Worker shim
(`beta/shim.js`), deterministic native/wasm parity checks in CI
(`docs/FLOAT_DETERMINISM.md`), immutable content-hashed static artifacts,
and Cloudflare Pages serving `/` (stable tag) and `/test` (head).
`docs/SPECTATOR_DEPLOY.md` owns the deploy contract. The acceptance gates
that section demanded — parity across seeds, green-only deployment, explicit
lane provenance (`build.json`) — are the shipped `published-build` +
`to-test-auto-30` machinery.

### The live Civilization VI bridge (shipped, climbing)
`tools/civ6_control` (a Lua mod + macOS input/vision harness) configures and
plays real Civ 6 games unattended: `docs/CIV6_COMPUTER_CONTROL.md` is the
contract, `docs/CIV6_LADDER.md` the record, and a supervisor loop plays one
game per fresh build of head.
