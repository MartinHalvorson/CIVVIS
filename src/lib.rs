//! Martin Halvorson's Civilization VIS — the engine: one pure-Rust crate
//! carrying the `data/` ruleset JSON, the action protocol, the mechanics, the
//! agents, and the GUI server. The Python reference implementation it was
//! ported from was removed on 2026-07-21 (`docs/ROADMAP.md`, v0.6), so there is
//! no second implementation to agree with; this one is deterministic per seed.
#![recursion_limit = "512"]
// ⭐ THIS CRATE DOCUMENTS ITS PRIVATE ITEMS ON PURPOSE. The reasoning lives in
// doc comments on the functions that carry it, and a public `enable_*` toggle
// links the private field it sets so a reader — or an IDE — can jump straight
// there. `rustdoc` warns because such a link cannot be followed in *published*
// docs; nothing here is published, and rewriting 78 of them into dead code
// spans would cost the navigation to buy nothing. A link that names something
// no longer in the crate is a different matter and still warns:
// `unresolved_intra_doc_links` stays on, and is what caught the envelope
// cache's argument for a mechanism deleted in #2163 (#2556).
#![allow(rustdoc::private_intra_doc_links)]

// ⭐ EVERY NATIVE BINARY ALLOCATES THROUGH mimalloc, NOT THE PLATFORM.
//
// The simulator clones whole boards to search, so allocation is not incidental
// work here — it is the inner loop's tax. `docs/SIMULATOR_PERFORMANCE.md` puts
// allocator and libc memory primitives at a **16.84%** roll-up, with macOS's own
// `_xzm_free` the largest single leaf in the profile at 3.66% self.
//
// One line in the library covers all twelve binaries, which is the point:
// `#[global_allocator]` is chosen by the final artifact, so stating it per
// binary means twelve chances for the next `[[bin]]` to be added without it and
// measure the platform allocator while every document says otherwise. Setting
// it in a library is normally rude because it overrides the choice of whoever
// links you — nobody links this one. The crate docs above say so in their first
// sentence: the Python implementation was removed, this is the only one, and
// nothing here is published.
//
// ⚠ Gated to native. `mimalloc` vendors a C library with no wasm32 target, and
// `Cargo.toml` gates the dependency itself to match; both halves are needed,
// because a `cfg`-ed out `static` still leaves an unbuildable crate in the
// dependency graph for `cargo check --target wasm32-unknown-unknown`.
#[cfg(not(target_arch = "wasm32"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// Each browser worker owns its own non-shared WebAssembly memory. Use the
// dynamically growing heap; an atomics build retains the standard allocator.
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
#[global_allocator]
static WASM_GLOBAL: talc::wasm::WasmDynamicTalc = talc::wasm::new_wasm_dynamic_allocator();

// The action encoder's consumers are all in `experiments/closed/`; the module
// compiles only `kind_name` without the `closed-experiments` feature.
pub mod action_space;
pub mod actionlog;
pub mod ai;
pub mod belief;
pub mod city_states;
pub mod civ6;
pub mod doctrine;
pub mod elo;
pub mod evolve;
pub mod fractal;
pub mod game;
pub mod hex;
pub mod historical_scenarios;
pub mod historical_terrain;
/// Rival chairs that fight like the live host: the harness switch
/// `--rival-host-tactics` and the census that reads any opponent's tactics.
pub mod host_tactics;
pub mod idmap;
pub mod leader_roster;
pub mod mapgen;
pub mod mirror;
pub mod name;
pub mod obs;
pub mod odds;
pub mod protocol;
pub mod routes;
// The oracle's only consumer is `experiments/closed/ablate.rs`, so it rides
// the same opt-in feature: normal builds and CI skip its ~1,300 lines and 12
// tests, and `--features closed-experiments` brings the whole ablation
// harness back exactly as recorded.
#[cfg(feature = "closed-experiments")]
pub mod oracle;
pub mod parallel;
#[rustfmt::skip]
pub mod obs_tensor;
// The training-data lane (`selfplay` exporter and its 34-wide feature
// vector) rides the `closed-experiments` feature with the action encoder and
// oracle: the value-estimator studies it fed are closed, so normal builds
// skip it and `--features closed-experiments` brings the exporter back
// exactly as recorded. `obs_tensor` stays unconditional — the strategic
// search's counterfactual sampling reads it directly.
#[cfg(feature = "closed-experiments")]
pub mod decision_features;
pub mod mods;
pub mod pedia;
pub mod reasoning;
pub mod rng;
pub mod rules;
#[cfg(feature = "closed-experiments")]
pub mod selfplay;
pub mod server;
pub mod setup;
pub mod simultaneous;
pub mod skirmish;
pub mod specmap;
pub mod sphere;
pub mod strategic;
pub mod strategies;
pub mod trafalgar;
pub mod validate;
pub mod valuenet;
pub mod world;

pub type Pos = (i32, i32);

#[cfg(test)]
mod tests;
