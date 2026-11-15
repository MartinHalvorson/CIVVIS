//! Hold a Trader's route for a turn when the host says it has no movement left.
//!
//! Civilization VI starts a trade route only for a Trader with movement to
//! spend. A Trader bought with Gold arrives with none, and one that changed its
//! origin city has spent it, so a `TRADE_ROUTE` sent on the same turn is
//! refused whatever the destination. The refusal then did lasting damage:
//!
//! - the mirror retires an origin/destination pair for the rest of the game
//!   once the host has refused it twice, and a replan frame re-sends the order,
//!   so two frames of one turn retired the route the seat ranked first;
//! - three refusals strike the order out for ten turns
//!   (`ORDER_REFUSAL_STRIKES`), which left a Trader idle for that long.
//!
//! Live game `civvis-20261115T071444Z` (Pericles, Deity, both sittings): all
//! six Traders were bought, each was sent on a route in a later frame of the
//! purchase turn, and each was refused; so was every route sent on the turn of
//! an origin change. Of 37 route requests, the 17 that answered a frame
//! showing the Trader with `moves: 0` were all refused, 13 of them for a pair
//! the host itself listed in `route_options`; the 14 the host accepted all
//! answered a frame showing `moves: 2`; the other 6 had movement and asked for
//! a pair the host did not list. On turn 121 the pair (10,15) -> (23,19) was
//! refused with no movement left and accepted on turn 122.
//!
//! The planning board keeps the Trader and its chosen route, so nothing else
//! is planned for it; only the request to the host waits. The next turn's
//! opening frame shows the Trader with its movement and the same route goes
//! out. Only a seat whose export reads `moves` at the start of its own turn
//! (`Seat::moves_at_turn_start`) is trusted to say so; older exports reported
//! `moves: 0` for every Trader, and for those nothing is held.
use super::Order;
use civvis::mirror::StateSnapshot;

/// Remove `TRADE_ROUTE` orders for Traders the host reports with no movement
/// left this turn. Returns how many were held.
pub(super) fn hold_routes(orders: &mut Vec<Order>, state: &StateSnapshot) -> usize {
    if !state.seat.moves_at_turn_start {
        return 0;
    }
    let before = orders.len();
    orders.retain(|order| {
        if order.kind != "unit" || order.verb.as_deref() != Some("TRADE_ROUTE") {
            return true;
        }
        let Some(subject) = order.subject else {
            return true;
        };
        !state
            .units
            .iter()
            .any(|unit| unit.id == subject && unit.kind == "UNIT_TRADER" && unit.moves == 0.0)
    });
    before - orders.len()
}

#[cfg(test)]
#[path = "spent_traders/tests.rs"]
mod tests;
