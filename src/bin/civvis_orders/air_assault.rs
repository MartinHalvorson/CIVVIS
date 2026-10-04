use super::Order;
use civvis::ai::{AdvancedAi, AirCityAssault};
use civvis::mirror::{Snapshot, StateSnapshot};
use std::collections::{BTreeMap, BTreeSet};

/// A mission the export layer would withhold cannot spend a planned sortie.
pub(super) fn apply_cooldowns(
    g: &mut civvis::game::Game,
    mapped: &BTreeMap<u32, i64>,
    turn: u32,
    refusals: &super::HostOrderRefusals,
) {
    let aircraft: BTreeMap<i64, u32> = mapped
        .iter()
        .filter(|(uid, _)| {
            g.units.get(uid).is_some_and(|unit| {
                unit.owner == 0 && g.rules.units[unit.kind].domain.as_deref() == Some("air")
            })
        })
        .map(|(uid, host)| (*host, *uid))
        .collect();
    let mut blocked = Vec::new();
    // AIR_ATTACK also strikes units and pillages terrain/districts. Iterate
    // exact known orders rather than only city plots or whole aircraft.
    for (kind, verb, subject, pos) in refusals.seen.keys() {
        if kind != "unit" || verb.as_deref() != Some("AIR_ATTACK") {
            continue;
        }
        let (Some(host), Some(offset)) = (subject, pos) else {
            continue;
        };
        let Some(uid) = aircraft.get(host) else {
            continue;
        };
        let target = civvis::hex::offset_to_axial(offset.0, offset.1);
        let request = Order {
            kind: "unit",
            subject: Some(*host),
            verb: verb.clone(),
            pos: Some(*offset),
        };
        if g.map.get(target).is_some() && refusals.withheld(&request, turn).is_some() {
            blocked.push((*uid, target));
        }
    }
    std::sync::Arc::make_mut(&mut g.blocked_strikes).extend(blocked);
}

pub(super) fn observe(ai: &mut AdvancedAi, snapshot: &Snapshot, state: &StateSnapshot) {
    let visible = snapshot
        .revealed_positions()
        .filter_map(|pos| {
            snapshot
                .plot(pos)
                .filter(|plot| plot.vis)
                .map(|_| civvis::hex::offset_to_axial(pos.0, pos.1))
        })
        .collect();
    let left =
        if state.seat.replan_frames && state.seat.tile_delta && state.seat.moves_at_turn_start {
            state
                .seat
                .replan_frame_limit
                .unwrap_or(0)
                .saturating_sub(state.frame)
        } else {
            0
        };
    ai.observe_air_assault_frame(visible, left);
}

/// End the speculative batch at the next required observation. Per-unit
/// queues cannot order one unit's movement against another unit's sortie.
/// Independent later orders are replanned from the same fresh board too;
/// none can accidentally rely on the city having fallen in speculation.
pub(super) fn defer_followups(
    orders: &mut Vec<Order>,
    assault: Option<&AirCityAssault>,
    snapshot: &Snapshot,
    mapped: &BTreeMap<u32, i64>,
) -> usize {
    let Some(assault) = assault.filter(|plan| !plan.aircraft.is_empty()) else {
        return 0;
    };
    let target = civvis::hex::axial_to_offset(assault.target.0, assault.target.1);
    let visible = snapshot.plot(target).is_some_and(|plot| plot.vis);
    let aircraft: BTreeSet<i64> = assault
        .aircraft
        .iter()
        .filter_map(|uid| mapped.get(uid).copied())
        .collect();
    // Every sortie of the maneuver's own aircraft is the volley, including
    // the ones it flies at the defenders on a breached city's approach
    // (`air_city_assault::air_assault_clear_approach`); only orders that are
    // not this wing's wait for the observed board.
    let is_sortie = |order: &Order| {
        order.kind == "unit"
            && order.verb.as_deref() == Some("AIR_ATTACK")
            && order.pos.is_some()
            && order.subject.is_some_and(|id| aircraft.contains(&id))
    };
    let Some(boundary) = orders.iter().position(is_sortie) else {
        return 0;
    };
    let before = orders.len();
    let mut index = 0;
    orders.retain(|order| {
        let keep = index < boundary || (visible && is_sortie(order));
        index += 1;
        keep
    });
    let deferred = before - orders.len();
    // A rejected move or sortie still needs a fresh board. Without this
    // bounded request, no strike/reveal event could strand the cavalry.
    orders.push(Order {
        kind: "observe",
        subject: None,
        verb: Some("AIR_ASSAULT".into()),
        pos: None,
    });
    deferred
}

#[cfg(test)]
#[path = "air_assault/tests.rs"]
mod tests;
