//! Retain an observed MOVE_TO endpoint when a later replan moves the unit away.
//!
//! A native state is a positive postcondition, not an operation acknowledgement
//! or proof of which queued request caused it. This extends the verifier's
//! existing final-position evidence to intermediate frames of the same turn.

use super::{own_unit, IssuedOrder, PendingOrders};
use civvis::mirror::StateSnapshot;

pub(super) fn observed_endpoint(
    pending: &PendingOrders,
    order: &IssuedOrder,
    states: &[StateSnapshot],
) -> bool {
    if order.kind != "unit" || order.verb.as_deref() != Some("MOVE_TO") {
        return false;
    }
    let (Some(id), Some(want)) = (order.subject, order.pos) else {
        return false;
    };
    if own_unit(&pending.before, id).is_none() {
        return false;
    }
    let seat = &pending.before.seat;
    states.iter().any(|state| {
        state.turn == pending.turn
            && state.frame > pending.frame
            // Native unit ids are player-local. Readers attach each state's
            // preceding seat event; never borrow a matching id from a new seat.
            && state.seat.local_player == seat.local_player
            && state.seat.players == seat.players
            && state.seat.civ == seat.civ
            && state.seat.leader == seat.leader
            && own_unit(state, id).is_some_and(|unit| (unit.x, unit.y) == want)
    })
}

#[cfg(test)]
#[path = "host_move_postconditions/tests.rs"]
mod tests;
