# Architecture

## Layers

```text
data/*.json
    ↓
Rules + map generation
    ↓
Game state machine + Action protocol
    ↓
observations / scripted agents / search experiments
    ↓
CLI / HTTP server / browser / WASM / evaluation tools
```

- **Rules and content** — `src/rules.rs`, `src/validate.rs`, and `data/*.json`.
  Units, technologies, civics, buildings, districts, projects, governments,
  policies, leaders, civilizations, map settings, and other catalogues are data.
  The canonical ruleset is embedded in the binary; tools can load a data
  directory for validation and mod work.
- **World generation** — `src/mapgen.rs`, `src/world.rs`, `src/fractal.rs`, and
  `src/sphere.rs`. Flat maps and geodesic Planet worlds share the same `Pos` and
  adjacency-facing game APIs.
- **Game** — `src/game.rs`, with `src/game/` beside it. `Game` owns
  authoritative state: players, map,
  units, cities, diplomacy, victory state, event log, and the serialized RNG.
  All player-visible mutation goes through `Game::apply(pid, &Action)`.
  Illegal actions return an error without becoming a second mutation path.

  `src/game/` holds the production submodules (`adjacency`, `quests`) and
  **every test module**, one file each. Tests used to be written inline, which
  is why `game.rs` reached 77k lines with 27k of them test code interleaved
  through the production body: a change to a method and a change to a test two
  hundred lines away were edits to the same file, and at this repository's
  merge rate that is a conflict. A new test module belongs in `src/game/` as
  `#[cfg(test)] mod name;` — `super` still resolves to `game`, so nothing about
  writing the test changes.
- **Observation** — `src/obs.rs` produces the fog-filtered JSON view used by a
  seated player and the HTTP API. `src/obs_tensor.rs` produces a fog-honest
  spatial tensor for offline learning. The spectator has an explicitly
  omniscient view; it is not a player observation.
- **Agents** — `src/ai.rs` contains the `Ai` trait, `RandomAi`, and `BasicAi`;
  `src/ai/advanced.rs` contains the production major-civilization controller.
  `src/strategic.rs`, `src/neural.rs`, `src/policy.rs`, and `src/production.rs`
  are search/learning experiments with explicit fallbacks and evaluation
  status. `src/oracle.rs` is a diagnostic wrapper, not a playable strategy.
- **Evaluation and training** — `src/elo.rs`, `src/evolve.rs`, `src/league.rs`,
  `src/action_space.rs`, and the binaries under `src/bin/`. Python in `tools/`
  trains offline models, analyzes evidence, and supervises deployments; it is
  not the game engine.
- **Interfaces** — `src/main.rs` exposes the CLI, while `src/server.rs` serves
  the JSON protocol and the browser client in `web/index.html`. The browser
  renderer stays in `web/assets/app.js`; the setup and Civilization VI bridge
  live in the separately served `web/assets/app_setup.js`. Both are classic
  scripts intentionally: inline controls and renderer callbacks keep their
  existing global contract while setup work no longer conflicts with map
  rendering edits. Native, beta, and WASM delivery all serve the same pair.
  The same Rust engine is compiled for native and WASM use.
- **Civilization VI bridges** — `tools/civ6_strategy.py` exports only the
  economic subset of a league genome to a grounding mod; Firaxis' AI controls
  the remaining real-game behavior. `tools/civ6_control/` is a separate Lua
  controller and does not embed or call the Rust agent.

The former Python `rules.py`, `game.py`, `hexgrid.py`, `CivEnv`, and
`ai/basic_ai.py` architecture no longer exists.

## Action protocol

`Game::legal_actions(pid)` returns the valid `Action` values for the current
state. `Game::apply` consumes the same enum, so internal agents, the HTTP API,
replay tools, and tests do not maintain separate rule paths. Serde gives each
action its tagged JSON representation.

The enum currently has 77 variants across these families:

- movement, attacks, air operations, fortification, pillaging, upgrades, and
  formations;
- founding, improving, harvesting, purchasing plots, and assigning citizens;
- city production and purchases, districts, projects, specialists, and Great
  People;
- research, civics, governments, policies, governors, dedications, religion,
  and Secret Societies;
- diplomacy, deals, alliances, emergencies, World Congress, espionage, trade,
  envoys, and occupied-city decisions;
- turn completion and other mandatory choices.

Do not add a side-channel for a controller. A new player action needs an
`Action` variant, legality enumeration, a `Game::apply` handler, serialization,
and tests.

## JSON boundary versioning

`src/protocol.rs` owns the contract shared by the native HTTP server and the
WASM router. Public JSON objects carry `protocol: "civvis-json"` and
`protocol_version: 1`; the existing fields stay at the top level so a client
can adopt the marker without rewriting its decoder. Requests may omit the
marker for compatibility with older clients, but a future version is refused
before an action or save is decoded.

Durable saves use the envelope
`{format: "civvis.save", save_format_version: 1, game: {...}}`. The loader
accepts that envelope and the raw `Game` JSON written by older builds, making
the migration one-way and explicit. Save files are written atomically, and
the same decoder is used by named saves, autosaves, uploads, and the browser
build so the two runtimes cannot silently develop different formats.

## Turn lifecycle

Players act sequentially. At a turn boundary the engine refreshes units and
visibility, applies healing and ongoing effects, settles city yields, growth,
production, borders, trade, research, civics, income, maintenance, diplomacy,
climate/disasters, era systems, and victory progress. The game then exposes the
next player's legal actions.

Victory checks use the represented Civilization VI routes: Science, Culture,
Religion, Diplomacy, Domination, and turn-limit Score. The same checks are used
by human play, scripted play, rollouts, tournaments, and exact victory tests.

## AI architecture

### Production controllers

`BasicAi` is a deterministic, full-state heuristic. It is deliberately cheap
and is used for city-states and barbarians. `AdvancedAi` wraps a persistent
hierarchy around the same legal-action interface for major civilizations:

```text
victory pressure and empire assessment
    ↓
persistent strategic plan and city roles
    ↓
campaigns, settlements, builders, trade, diplomacy, and spending
    ↓
domain-specific force groups and tactical candidates
    ↓
Game::apply(Action)
```

Combat units are clustered by movement domain and command distance into
`ForceGroup`s with a shared objective, anchor, focus target, readiness, local
strength, and posture. Orders are recomputed between unit actions, so a kill,
retreat, opened route, or local-power change can alter the rest of the turn.

Candidate attacks use a static exchange score and a bounded forcing-reply
extension on a clone. Only forcing combat responses are expanded. This is real
local search inside `AdvancedAi`; macro rollout search is not the only search in
the codebase.

### Experimental controllers

`StrategicAi` wraps `AdvancedAi` and periodically projects an adaptive branch
plus enabled victory-lane commitments. Decided branches return the outcome;
unresolved branches use score share blended with an optional 25-feature value
net. Priors may answer before a rollout, and `ReviewCensus` reports which path
actually decided each review.

`NeuralAi` applies a similar optional scalar value net to scripted peace/war
rollouts on top of `BasicAi`. `PolicyAi` applies candidate tactical actions to
clones and greedily scores their resulting state before handing routine work to
`AdvancedAi`. `ProductionSearchAi` projects candidate city builds. None is the
default major controller, and the latter two are retained in part because their
controlled evaluations were negative.

The repository embeds an evolved scripted genome but no value net. Agent
factories therefore resolve missing learned artifacts to documented scripted or
score-share behavior. `builtin_provenance` uses the same loaders as the agent
factory so an evaluator can report what will actually play.

### League and live seating

`src/league.rs` stores named builtin or parameterized `AdvancedAi` strategies
with Glicko-2 state. The supervised exhibition starts with a committed snapshot,
records into a gitignored runtime copy, and rank-weights each
leader/civilization's top three live-eligible strategies while avoiding repeats
where possible. `league_only` entries participate in offline rating but are
excluded from exhibition and auto-play seating.

Without a league, `Session::ai_fleet` constructs stock `AdvancedAi` for major
civilizations and `BasicAi` for minors/barbarians. A human seat is never silently
credited to a roster strategy.

## Learning surfaces

The runtime and training representations are intentionally distinct:

- `evolve::features` — 25 full-state empire aggregates used by the scalar MLP;
- `decision_features` — 34 action-sensitive scalar terms for offline policy
  experiments;
- `obs_tensor` — 25 fog-honest spatial planes plus public/global values;
- `action_space` — stable action-kind IDs and contextual/destination features.

`selfplay` and counterfactual/Q exporters write training corpora. Python tools
train scalar or spatial models. A spatial checkpoint currently has no Rust
consumer, and no learned model file is committed, so these are development
surfaces rather than claims about the live agent.

## Determinism and serialization

One serialized `Rng` drives engine randomness. Scripted agents use seeded local
RNGs only where needed. The same initial state and action sequence reproduce the
same game, and JSON saves round-trip the state required to continue it.

Search clones the same state machine and applies real actions; it does not use a
simplified shadow rules engine. Evaluation relies on that property for paired
maps, replay, counterfactual labels, and provenance checks.
