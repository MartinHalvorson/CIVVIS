//! The one integration-test binary.
//!
//! Every `tests/*.rs` file is its own Cargo test target, and every target
//! links the full ~85 MB `libcivvis` rlib into its own binary — so the four
//! small suites that used to live at the top level cost four large link jobs
//! per `cargo test`, and the two that embed `web/assets/app.js` (1.35 MB)
//! compiled it in twice more. One target pays one link and one embed. New
//! integration suites join here as a `mod`, not as a new top-level file;
//! `tests/fixtures/` stays where it is — it is preserved evidence named by
//! `docs/closed/LIVE_GENOME_TRANSFER.md`, not this binary's input.
//!
//! Inside the modules `include_str!` paths climb two levels (`../../data/…`):
//! the paths are relative to each source file, which now sits one directory
//! deeper than a top-level test target.

mod civ6_jerseys;
mod nations_today;
mod planet_map_types;
mod strategic_tile_placement;
