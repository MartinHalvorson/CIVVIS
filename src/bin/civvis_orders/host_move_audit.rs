//! The host move audit: what the host did with each move this turn, and
//! whether the engine would have refused it for the same reason.
//!
//! The observed board the audit walks (`ObservedHostMove`,
//! `observed_host_move`), kept current as units advance, die or leave
//! (`advance_observed_host_position`, `remove_observed_local_unit`,
//! `remove_observed_foreign_unit`, `advance_observed_combat_deaths`); the
//! reason the engine's own rules give for refusing a step
//! (`model_move_refusal_reason`); and the verdict for one move and for the
//! turn (`audit_host_move`, `audit_host_moves`, `HostMoveAuditVerdict`).
//!
//! Moved verbatim out of `civvis_orders.rs` (2026-11-05), which re-exports
//! everything here, so every path is unchanged. Items the rest of the binary
//! uses are `pub(super)`; the code is otherwise unchanged.

use super::*;

/// One movement edge the host actually completed.  Unlike an outbound order,
/// this is evidence in the reverse direction: Civilization VI admitted the
/// step, so the reconstructed board must be able to admit it too.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ObservedHostMove {
    pub(super) turn: u32,
    pub(super) unit: i64,
    pub(super) from: (i32, i32),
    pub(super) to: (i32, i32),
}

/// The outcome of asking the reconstructed board about one host-observed step.
/// `Unverifiable` is deliberately distinct from agreement: a move that crossed
/// ground the mirror never received cannot validate either engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HostMoveAuditVerdict {
    ModelAllowed,
    ModelRefused(String),
    Unverifiable(String),
}

pub(super) fn observed_host_move(event: &serde_json::Value) -> Option<ObservedHostMove> {
    if event.get("kind").and_then(|kind| kind.as_str()) != Some("host_move") {
        return None;
    }
    let nonnegative = |key: &str| {
        event
            .get(key)
            .and_then(|value| value.as_i64())
            .filter(|value| *value >= 0 && *value <= i64::from(i32::MAX))
            .map(|value| value as i32)
    };
    let turn = nonnegative("turn")? as u32;
    Some(ObservedHostMove {
        turn,
        unit: event.get("unit")?.as_i64()?,
        from: (nonnegative("from_x")?, nonnegative("from_y")?),
        to: (nonnegative("x")?, nonnegative("y")?),
    })
}

/// Advance the audit's local state shadow after a host-observed movement edge.
/// State exports are periodic while `UnitMoved` can fire several times between
/// them; without this, a unit that has already vacated a tile remains an
/// invented blocker for the next observed edge.
pub(super) fn advance_observed_host_position(
    state: &mut civvis::mirror::StateSnapshot,
    observed: &ObservedHostMove,
) -> bool {
    let Some(unit) = state.units.iter_mut().find(|unit| unit.id == observed.unit) else {
        return false;
    };
    unit.x = observed.to.0;
    unit.y = observed.to.1;
    true
}

/// A `unit_lost` ledger row is explicitly one of our units, so it only affects
/// the local unit list.  Keeping it in the shadow would make a later observed
/// move into its former tile look like a false stacking mismatch.
pub(super) fn remove_observed_local_unit(
    state: &mut civvis::mirror::StateSnapshot,
    unit: i64,
) -> bool {
    let before = state.units.len();
    state.units.retain(|candidate| candidate.id != unit);
    state.units.len() != before
}

/// Remove a known foreign combat casualty without confusing equal per-player
/// Civilization VI unit ids.  The combat ledger gives the owner, while each
/// foreign export stores units under that owner (or gives a hostile its player).
/// An old hostile row without an owner is intentionally retained rather than
/// guessing that an equally numbered casualty was it.
fn remove_observed_foreign_unit(
    state: &mut civvis::mirror::StateSnapshot,
    player: i64,
    unit: i64,
) -> bool {
    let mut removed = false;
    for rival in &mut state.rivals {
        if rival.player as i64 != player {
            continue;
        }
        let before = rival.units.len();
        rival.units.retain(|candidate| candidate.id != unit);
        removed |= rival.units.len() != before;
    }
    for minor in &mut state.minors {
        if minor.player as i64 != player {
            continue;
        }
        let before = minor.units.len();
        minor.units.retain(|candidate| candidate.id != unit);
        removed |= minor.units.len() != before;
    }
    let before = state.hostiles.len();
    state
        .hostiles
        .retain(|candidate| candidate.id != unit || candidate.player != player);
    removed || state.hostiles.len() != before
}

/// Reconcile combat casualties that arrive between two state exports.  `combat`
/// names both component type and owner, so this never treats a city/district
/// id as a unit or removes another player's equally numbered unit.
pub(super) fn advance_observed_combat_deaths(
    state: &mut civvis::mirror::StateSnapshot,
    event: &serde_json::Value,
) {
    for (side, killed, local) in [
        ("attacker", "attacker_killed", "ours"),
        ("defender", "defender_killed", "against_us"),
    ] {
        if event.get(killed).and_then(|value| value.as_bool()) != Some(true) {
            continue;
        }
        let Some(participant) = event.get(side) else {
            continue;
        };
        if participant.get("type").and_then(|value| value.as_str()) != Some("unit") {
            continue;
        }
        let (Some(player), Some(unit)) = (
            participant.get("player").and_then(|value| value.as_i64()),
            participant.get("id").and_then(|value| value.as_i64()),
        ) else {
            continue;
        };
        if event.get(local).and_then(|value| value.as_bool()) == Some(true) {
            remove_observed_local_unit(state, unit);
        } else {
            remove_observed_foreign_unit(state, player, unit);
        }
    }
}

/// Give a model refusal the most immediate visible explanation.  This is an
/// audit label, never a replacement for the host's own operation-result
/// reason: the mirror can prove that a blocker is present, but not that it was
/// the first check the host would have made.
fn model_move_refusal_reason(game: &civvis::game::Game, uid: u32, to: civvis::Pos) -> String {
    let Some(mover) = game.units.get(&uid) else {
        return "unit_gone".to_string();
    };
    let occupants = game.unit_ids_at(to).to_vec();
    let own: Vec<u32> = occupants
        .iter()
        .copied()
        .filter(|other| game.units[other].owner == mover.owner)
        .collect();
    if !own.is_empty() {
        return "own_unit".to_string();
    }
    let foreign: Vec<u32> = occupants
        .iter()
        .copied()
        .filter(|other| game.units[other].owner != mover.owner)
        .collect();
    if !foreign.is_empty() {
        return "foreign_unit".to_string();
    }
    let Some(tile) = game.map.get(to) else {
        return "unmapped_destination".to_string();
    };
    if !game.rules.is_passable(tile) {
        return "terrain".to_string();
    }
    if game.closed_borders.contains(&to) {
        return "border".to_string();
    }
    if game.in_enemy_zoc(mover.owner, to) {
        return "zoc".to_string();
    }
    "model_refused".to_string()
}

/// Replay one host-observed adjacent step against the mirror reconstructed from
/// the state that preceded it.  A probe copy of the unit starts at the host's
/// `from` coordinate with ample movement: this tests structural legality
/// (terrain, borders, stacking and foreign units), not whether an earlier host
/// action had already spent its movement allowance.
pub(super) fn audit_host_move(
    snapshot: &civvis::mirror::Snapshot,
    state: &civvis::mirror::StateSnapshot,
    fallback_players: usize,
    fallback_turns: u32,
    frontier: u32,
    observed: &ObservedHostMove,
) -> HostMoveAuditVerdict {
    let from = civvis::hex::offset_to_axial(observed.from.0, observed.from.1);
    let to = civvis::hex::offset_to_axial(observed.to.0, observed.to.1);
    let (players, turns) = mirror_setup(state, fallback_players, fallback_turns);
    let mirror = civvis::mirror::LiveMirror::new(snapshot, state, players, 1, turns, frontier);
    if !mirror.game.map.tiles.contains_key(&from) || !mirror.game.map.tiles.contains_key(&to) {
        return HostMoveAuditVerdict::Unverifiable("unrevealed_ground".to_string());
    }
    if mirror.game.wdist(from, to) != 1 {
        return HostMoveAuditVerdict::Unverifiable("non_adjacent_event".to_string());
    }
    let Some(uid) = mirror.uid_of.get(&observed.unit).copied() else {
        return HostMoveAuditVerdict::Unverifiable("unit_unmapped".to_string());
    };
    match mirror.game.can_move_observed_step(uid, from, to) {
        Some(true) => HostMoveAuditVerdict::ModelAllowed,
        Some(false) => {
            HostMoveAuditVerdict::ModelRefused(model_move_refusal_reason(&mirror.game, uid, to))
        }
        None => HostMoveAuditVerdict::Unverifiable("unit_missing".to_string()),
    }
}

/// Audit every `host_move` event in a recorded run.  The mod only emits one
/// after it knows both endpoints, and this command preserves the remaining
/// uncertainty as a count instead of silently calling it agreement.
pub(super) fn audit_host_moves(
    events: &Path,
    fallback_players: usize,
    fallback_turns: u32,
    frontier: u32,
) {
    let Ok(raw) = std::fs::read_to_string(events) else {
        eprintln!("civvis-orders: cannot read {}", events.display());
        return;
    };
    let mut snapshots: std::collections::BTreeMap<u32, civvis::mirror::Snapshot> =
        Default::default();
    let mut current: Option<(civvis::mirror::Snapshot, civvis::mirror::StateSnapshot)> = None;
    let mut observed = 0usize;
    let mut comparable = 0usize;
    let mut model_allowed = 0usize;
    let mut model_refused = 0usize;
    let mut unverifiable: std::collections::BTreeMap<String, usize> = Default::default();
    let mut mismatches: Vec<serde_json::Value> = Vec::new();

    for line in raw.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match event.get("kind").and_then(|kind| kind.as_str()) {
            Some("state") => {
                let Ok(state) = serde_json::from_value::<civvis::mirror::StateSnapshot>(event)
                else {
                    continue;
                };
                let snapshot = snapshots
                    .entry(state.turn)
                    .or_insert_with(|| {
                        civvis::mirror::snapshot_from_events_at(events, Some(state.turn))
                            .unwrap_or_default()
                    })
                    .clone();
                current = Some((snapshot, state));
            }
            Some("host_move") => {
                observed += 1;
                let Some(host_move) = observed_host_move(&event) else {
                    *unverifiable
                        .entry("malformed_event".to_string())
                        .or_default() += 1;
                    continue;
                };
                let Some((snapshot, state)) = current.as_mut() else {
                    *unverifiable
                        .entry("no_prior_state".to_string())
                        .or_default() += 1;
                    continue;
                };
                match audit_host_move(
                    snapshot,
                    state,
                    fallback_players,
                    fallback_turns,
                    frontier,
                    &host_move,
                ) {
                    HostMoveAuditVerdict::ModelAllowed => {
                        comparable += 1;
                        model_allowed += 1;
                    }
                    HostMoveAuditVerdict::ModelRefused(reason) => {
                        comparable += 1;
                        model_refused += 1;
                        mismatches.push(serde_json::json!({
                            "turn": host_move.turn,
                            "unit": host_move.unit,
                            "from": host_move.from,
                            "to": host_move.to,
                            "reason": reason,
                        }));
                    }
                    HostMoveAuditVerdict::Unverifiable(reason) => {
                        *unverifiable.entry(reason).or_default() += 1;
                    }
                }
                // Later host moves in this state interval must see the earlier
                // mover at its observed destination, even if this particular
                // edge was not comparable because the mirror lacked some fact.
                advance_observed_host_position(state, &host_move);
            }
            Some("unit_lost") | Some("unit_captured") => {
                if let (Some((_, state)), Some(unit)) = (
                    current.as_mut(),
                    event.get("unit").and_then(|value| value.as_i64()),
                ) {
                    remove_observed_local_unit(state, unit);
                }
            }
            Some("combat") => {
                if let Some((_, state)) = current.as_mut() {
                    advance_observed_combat_deaths(state, &event);
                }
            }
            _ => {}
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "observed": observed,
            "comparable": comparable,
            "model_allowed": model_allowed,
            "model_refused": model_refused,
            "unverifiable": unverifiable,
            "mismatches": mismatches,
        })
    );
}
