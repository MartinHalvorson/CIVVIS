//! Offline inspection of the configured AI's legal improvement scores.

use super::*;
use civvis::ai::{AdvancedAi, GrandStrategy};

fn strategy(args: &[String]) -> Result<GrandStrategy, &'static str> {
    if args.iter().any(|arg| arg == "--serve") {
        return Err("--explain-builders is offline and cannot be combined with --serve");
    }
    match arg_text(args, "--builder-strategy").as_deref() {
        Some("expansion") => Ok(GrandStrategy::Expansion),
        Some("science") => Ok(GrandStrategy::Science),
        Some("culture") => Ok(GrandStrategy::Culture),
        Some("religion") => Ok(GrandStrategy::Religion),
        Some("diplomacy") => Ok(GrandStrategy::Diplomacy),
        Some("conquest") => Ok(GrandStrategy::Conquest),
        Some("recovery") => Ok(GrandStrategy::Recovery),
        _ => Err("--explain-builders requires --builder-strategy expansion|science|culture|religion|diplomacy|conquest|recovery"),
    }
}

pub(super) fn explain(
    args: &[String],
    ai: &AdvancedAi,
    players: usize,
    max_turns: u32,
    frontier: u32,
    load: &impl Fn(Option<u32>) -> Option<(mirror::Snapshot, mirror::StateSnapshot)>,
) -> Result<serde_json::Value, &'static str> {
    let strategy = strategy(args)?;
    let want = arg_text(args, "--turn").and_then(|v| v.parse().ok());
    let (snapshot, state) = load(want).ok_or("no revealed terrain or no state yet")?;
    let (players, turns) = mirror_setup(&state, players, max_turns);
    let live = mirror::LiveMirror::new(&snapshot, &state, players, 1, turns, frontier);
    let builders: Vec<_> = live
        .civ6_of
        .iter()
        .filter_map(|(&uid, &native)| {
            let mut result = ai.builder_job_diagnostics(&live.game, 0, uid, strategy)?;
            result["native_builder"] = native.into();
            Some(result)
        })
        .collect();
    Ok(serde_json::json!({
        "kind": "builder_diagnostics", "schema": 1,
        "turn": state.turn, "frame": state.frame,
        "strategy": strategy.as_str(), "builders": builders,
        "scope": "Fresh reconstructed board and configured CLI genome. No persistent AI memory, historical strategy inference, host-legality claim or orders."
    }))
}

#[cfg(test)]
#[path = "builder_diagnostics/tests.rs"]
mod tests;
