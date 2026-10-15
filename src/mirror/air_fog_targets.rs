//! Aircraft must not turn reconstructed sight into a native target offer.

use super::Snapshot;
use crate::game::{Game, MIRRORED_SEAT};
use crate::hex::axial_to_offset;
use std::collections::BTreeSet;
use std::sync::Arc;

/// A native sight frame is authoritative for aircraft targets until the next
/// export. Reconstructed sight (notably a Spy's) can overstate what Firaxis
/// offers. Keep that derivation for simulation and movement, but block sorties
/// into known native fog using the same per-unit gate as a refused strike.
/// Foreign units positively observed by the host remain valid spot sightings.
/// Without any current plot sight, retain older archives' existing behavior.
pub(super) fn apply(game: &mut Game, snapshot: &Snapshot) {
    if !snapshot.revealed.values().any(|plot| plot.vis) {
        return;
    }
    let mut fogged = BTreeSet::new();
    for unit in game.units.values().filter(|unit| {
        unit.owner == MIRRORED_SEAT && game.rules.units[unit.kind].domain.as_deref() == Some("air")
    }) {
        for target in game.wdisk(
            game.air_operation_origin(unit.id),
            game.unit_attack_range(unit.id),
        ) {
            if !game.host_observed.contains(&target)
                && snapshot
                    .plot(axial_to_offset(target.0, target.1))
                    .is_some_and(|plot| !plot.vis)
            {
                fogged.insert((unit.id, target));
            }
        }
    }
    Arc::make_mut(&mut game.blocked_strikes).extend(fogged);
}

#[cfg(test)]
mod tests;
