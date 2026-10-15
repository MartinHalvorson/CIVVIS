use super::{line_ranges_containing, Snapshot, StateSnapshot};
use crate::game::{Game, NativeSettlementPressure};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Observation {
    turn: u32,
    #[serde(default)]
    frame: u32,
    #[serde(default)]
    available: bool,
    #[serde(default)]
    plots: Vec<Warning>,
}

#[derive(Clone, Debug, Deserialize)]
struct Warning {
    x: i32,
    y: i32,
    loyalty: f64,
}

pub(super) fn read(
    raw: &str,
    state_start: Option<usize>,
    board: Option<(u32, u32)>,
) -> Option<Observation> {
    let state_start = state_start?;
    let board = board?;
    for (start, end) in line_ranges_containing(raw, "\"settlement_loyalty\"")
        .into_iter()
        .rev()
    {
        if start >= state_start {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw[start..end]) else {
            continue;
        };
        if value.get("kind").and_then(|kind| kind.as_str()) != Some("settlement_loyalty") {
            continue;
        }
        // The newest event is authoritative even when unavailable, malformed,
        // or stamped for another frame. Never revive an older warning table.
        let observation: Observation = serde_json::from_value(value).ok()?;
        return (observation.available && (observation.turn, observation.frame) == board)
            .then_some(observation);
    }
    None
}

pub(super) fn apply(game: &mut Game, snapshot: &Snapshot, state: &StateSnapshot) {
    game.observed_settlement_loyalty = snapshot
        .settlement_loyalty
        .as_ref()
        .filter(|reading| {
            reading.available && (reading.turn, reading.frame) == (state.turn, state.frame)
        })
        .map(|reading| {
            Arc::new(NativeSettlementPressure {
                player: 0,
                turn: state.turn,
                plots: reading
                    .plots
                    .iter()
                    .filter(|row| {
                        snapshot.is_revealed((row.x, row.y))
                            && row.loyalty.is_finite()
                            && (-20.0..=20.0).contains(&row.loyalty)
                    })
                    .map(|row| (crate::hex::offset_to_axial(row.x, row.y), row.loyalty))
                    .collect(),
            })
        });
}

#[cfg(test)]
mod tests;
