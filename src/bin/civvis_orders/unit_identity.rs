//! Carry assignments through an observed native upgrade replacement.
//!
//! A missing ID alone is a death, not an alias. Match only an upgrade we
//! actually issued, the host's exact successor, and preserved veteran state.

use std::collections::{BTreeMap, BTreeSet};

use civvis::mirror::StateSnapshot;

use super::PendingOrders;

pub(super) fn replacement_ids(
    pending: &[PendingOrders],
    after: &StateSnapshot,
) -> BTreeMap<i64, i64> {
    let mut matches: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
    for frame in pending {
        if frame.before.seat.local_player != after.seat.local_player
            || after.turn < frame.turn
            || after.turn > frame.turn.saturating_add(1)
            || (after.turn == frame.turn && after.frame <= frame.frame)
        {
            continue;
        }
        for order in &frame.orders {
            if order.kind != "unit" || order.verb.as_deref() != Some("UPGRADE") {
                continue;
            }
            let Some(id) = order.subject else { continue };
            let Some(was) = frame.before.units.iter().find(|u| u.id == id) else {
                continue;
            };
            let Some(successor) = was.upgrade_to.as_deref() else {
                continue;
            };
            if successor.is_empty()
                || was.upgrade_blocked_reason.is_some()
                || was.xp.is_none()
                || was.level.is_none()
                || was.promotions.is_none()
                || was.formation.is_none()
                || after.units.iter().any(|u| u.id == id)
                || after.confirmed_unit_deaths.iter().any(|death| {
                    death.unit == id
                        && death.player as i64 == i64::from(after.seat.local_player)
                        && death.turn >= frame.turn
                })
            {
                continue;
            }
            let old_promotions: BTreeSet<_> = was.promotions.iter().flatten().collect();
            let candidates: Vec<_> = after
                .units
                .iter()
                .filter(|now| {
                    now.id >= 0
                        && now.kind == successor
                        && (now.x, now.y) == (was.x, was.y)
                        && now.player == was.player
                        && now.xp == was.xp
                        && now.level == was.level
                        && now.formation == was.formation
                        && now.promotions.is_some()
                        && now.promotions.iter().flatten().collect::<BTreeSet<_>>()
                            == old_promotions
                        && !frame.before.units.iter().any(|old| old.id == now.id)
                })
                .collect();
            matches
                .entry(id)
                .or_default()
                .extend(candidates.iter().map(|unit| unit.id));
        }
    }
    // Neither an old ID with competing replacements nor two old IDs claiming
    // one replacement may transfer memory to a stranger.
    let unique: Vec<_> = matches
        .iter()
        .filter_map(|(old, new)| {
            if new.len() == 1 {
                new.first().map(|next| (*old, *next))
            } else {
                None
            }
        })
        .collect();
    unique
        .iter()
        .filter(|(_, new)| unique.iter().filter(|(_, other)| other == new).count() == 1)
        .copied()
        .collect()
}

pub(super) fn carried_units(
    previous: &BTreeMap<u32, i64>,
    current: &BTreeMap<i64, u32>,
    replacements: &BTreeMap<i64, i64>,
) -> BTreeMap<u32, u32> {
    previous
        .iter()
        .filter_map(|(old_uid, native)| {
            let next = current.get(native).or_else(|| {
                let replacement = replacements.get(native)?;
                // An existing survivor already owns this identity.
                if previous.values().any(|id| id == replacement) {
                    None
                } else {
                    current.get(replacement)
                }
            })?;
            Some((*old_uid, *next))
        })
        .collect()
}

#[cfg(test)]
#[path = "unit_identity_tests.rs"]
mod tests;
