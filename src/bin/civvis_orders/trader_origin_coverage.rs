//! Serialize the AI's native Trader origin choice without inventing a route.
use super::Order;
use civvis::{
    ai::AdvancedAi,
    game::Game,
    hex::{axial_to_offset, offset_to_axial},
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn apply(
    ai: &AdvancedAi,
    game: &Game,
    native_units: &BTreeMap<i64, u32>,
    busy: &BTreeSet<i64>,
    orders: &mut [Order],
) -> usize {
    let mut covered: BTreeSet<_> = (0..game.players.len())
        .filter(|owner| game.has_tourism_trade_route(0, *owner))
        .collect();
    let mut transfers = 0;
    for order in orders {
        if order.kind != "unit" || order.verb.as_deref() != Some("TRADE_ROUTE") {
            continue;
        }
        let Some((native, (x, y))) = order.subject.zip(order.pos) else {
            continue;
        };
        if busy.contains(&native) {
            continue;
        }
        let Some(&unit) = native_units.get(&native) else {
            continue;
        };
        let Some(destination) = game.city_at(offset_to_axial(x, y)) else {
            continue;
        };
        if let Some((origin, owner)) =
            ai.native_culture_trade_origin(game, 0, unit, destination, &covered)
        {
            order.verb = Some("TELEPORT_TO_CITY".into());
            order.pos = Some(axial_to_offset(game.cities[&origin].pos));
            covered.insert(owner); // Do not send two idle Traders to the same missing market.
            transfers += 1;
        } else {
            covered.insert(game.cities[&destination].owner);
        }
    }
    transfers
}

#[cfg(test)]
mod tests;
