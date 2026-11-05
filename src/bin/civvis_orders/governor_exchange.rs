//! Keep an explicit seat exchange across Firaxis's asynchronous assignments.
//!
//! `defer_governor_followups` still sends one Governor operation per frame.
//! Only a reciprocal pair whose first assignment is that frame's first Governor
//! operation is retained. Appointments/promotions continue to require a re-plan.

use super::Order;
use civvis::mirror::{StateGovernor, StateSnapshot};

const MAX_WAIT_TURNS: u32 = 3;

#[derive(Clone, Copy)]
struct City {
    id: i64,
    owner: i32,
    x: i32,
    y: i32,
}

impl City {
    fn observed(state: &StateSnapshot, governor: &StateGovernor) -> Option<Self> {
        let owner = state.seat.local_player;
        if owner < 0 || governor.city_player != owner || governor.neutralized_turns != 0 {
            return None;
        }
        let city = state.cities.iter().find(|city| {
            city.id == governor.city && city.x == governor.x && city.y == governor.y
        })?;
        Some(Self {
            id: city.id,
            owner,
            x: city.x,
            y: city.y,
        })
    }

    fn present(self, state: &StateSnapshot) -> bool {
        state.seat.local_player == self.owner
            && state
                .cities
                .iter()
                .any(|city| city.id == self.id && city.x == self.x && city.y == self.y)
    }

    fn holds(self, governor: &StateGovernor) -> bool {
        governor.city == self.id
            && governor.city_player == self.owner
            && governor.x == self.x
            && governor.y == self.y
    }

    fn requested_by(self, order: &Order) -> bool {
        order.kind == "governor_assign"
            && order.subject == Some(self.id)
            && order.pos == Some((self.owner, -1))
    }
}

struct Exchange {
    first: String,
    second: String,
    destination: City,
    returning_to: City,
    requested: (u32, u32),
    last_seen: (u32, u32),
    return_requested_turn: Option<u32>,
}

impl Exchange {
    fn capture(state: &StateSnapshot, orders: &[Order]) -> Option<Self> {
        let mut governor_orders = orders.iter().filter(|order| is_governor(order));
        let first = governor_orders.next()?;
        let second = governor_orders.next()?;
        // An earlier title operation may change what can be assigned. Do not
        // retain a plan computed against an unobserved appointment/promotion.
        if first.kind != "governor_assign" || second.kind != "governor_assign" {
            return None;
        }
        let first_name = first.verb.as_deref()?;
        let second_name = second.verb.as_deref()?;
        if first_name == second_name {
            return None;
        }
        let governors = state.governors.as_ref()?;
        let first_post = governors.iter().find(|row| row.kind == first_name)?;
        let second_post = governors.iter().find(|row| row.kind == second_name)?;
        let returning_to = City::observed(state, first_post)?;
        let destination = City::observed(state, second_post)?;
        if destination.id == returning_to.id
            || !destination.requested_by(first)
            || !returning_to.requested_by(second)
        {
            return None;
        }
        Some(Self {
            first: first_name.to_owned(),
            second: second_name.to_owned(),
            destination,
            returning_to,
            requested: (state.turn, state.frame),
            last_seen: (state.turn, state.frame),
            return_requested_turn: None,
        })
    }

    fn return_order(&self) -> Order {
        Order {
            kind: "governor_assign",
            subject: Some(self.returning_to.id),
            verb: Some(self.second.clone()),
            pos: Some((self.returning_to.owner, -1)),
        }
    }
}

fn is_governor(order: &Order) -> bool {
    order.kind.starts_with("governor_")
}

/// Per-decider memory, retained across `--fresh-board` reconstructions.
#[derive(Default)]
pub(super) struct Continuation {
    pending: Option<Exchange>,
}

impl Continuation {
    /// Prepare the next frame; the existing single-operation filter runs after
    /// this. Other orders retain their relative order throughout.
    pub(super) fn prepare(
        &mut self,
        state: &StateSnapshot,
        orders: &mut Vec<Order>,
    ) -> Option<&'static str> {
        let Some(mut exchange) = self.pending.take() else {
            self.pending = Exchange::capture(state, orders);
            return self.pending.as_ref().map(|_| "return_retained");
        };
        let key = (state.turn, state.frame);
        if key < exchange.last_seen
            || state.turn > exchange.requested.0.saturating_add(MAX_WAIT_TURNS)
            || !exchange.destination.present(state)
            || !exchange.returning_to.present(state)
        {
            return Some("cancelled_stale_city_or_frame");
        }
        exchange.last_seen = key;
        let Some(governors) = state.governors.as_ref() else {
            return Some("cancelled_unknown_roster");
        };
        let Some(first) = governors.iter().find(|row| row.kind == exchange.first) else {
            return Some("cancelled_missing_governor");
        };
        let Some(second) = governors.iter().find(|row| row.kind == exchange.second) else {
            return Some("cancelled_missing_governor");
        };
        if first.neutralized_turns != 0 || second.neutralized_turns != 0 {
            return Some("cancelled_neutralized_governor");
        }
        if !exchange.destination.holds(first) {
            if !exchange.returning_to.holds(first) || !exchange.destination.holds(second) {
                return Some("cancelled_changed_post");
            }
            orders.retain(|order| !is_governor(order));
            self.pending = Some(exchange);
            return Some("waiting_for_first_assignment");
        }
        if exchange.returning_to.holds(second) {
            return Some("both_posts_observed");
        }
        if (second.city >= 0 && !exchange.destination.holds(second))
            || governors.iter().any(|governor| {
                governor.kind != exchange.first
                    && governor.kind != exchange.second
                    && exchange.returning_to.holds(governor)
            })
        {
            return Some("cancelled_changed_post");
        }
        let slot = orders.iter().position(is_governor).unwrap_or(0);
        orders.retain(|order| !is_governor(order));
        if key == exchange.requested || exchange.return_requested_turn == Some(state.turn) {
            self.pending = Some(exchange);
            return Some("waiting_for_fresh_return_readback");
        }
        orders.insert(slot.min(orders.len()), exchange.return_order());
        exchange.return_requested_turn = Some(state.turn);
        self.pending = Some(exchange);
        Some("return_released_after_first_readback")
    }
}

#[cfg(test)]
#[path = "governor_exchange/tests.rs"]
mod tests;
