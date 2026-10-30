//! `culture-routes-reach-every-rival`: a Culture seat's Trader takes a route
//! to a rival that has none of ours before it takes any other route.
//!
//! An international route of ours is +25% on all tourism pressure we bank
//! against its destination's owner (`Game::has_tourism_trade_route`,
//! `Game::international_tourism_multiplier`), and pressure is banked with the
//! multiplier in force when it is generated. A native Immortal culture census
//! (4p pangaea, three handicapped rival chairs, 16 games) found routes to
//! about half the rivals at turns 100–125 of a tourism ramp: the existing
//! premium (`culture_route_bonus`, 12 + a quarter of our tourism, doubled at
//! most for the bar holder) loses to a domestic route's Food and Production.
//! The same multiplier bought through Open Borders
//! (`culture-keeps-every-border-open`) finished culture wins 7 turns sooner.
//!
//! With the gene on, while a Culture seat's tourism is at least
//! [`CULTURE_ROUTE_TOURISM_FLOOR`], a Trader standing in one of our cities that
//! can reach a city of a met rival at peace with us and without a route of
//! ours goes there — to the rival with the most domestic tourists first
//! (visitors drawn from the bar holder also lower the bar), then the most
//! valuable of its cities. Only the route a Trader commits to changes; the
//! value that prices building a Trader does not. Off, nothing changes.
use super::{AdvancedAi, GrandStrategy};
use crate::game::Game;

/// Tourism per turn from which the multiplier outweighs a domestic route's
/// yields; the same floor as `culture-keeps-every-border-open`.
pub(super) const CULTURE_ROUTE_TOURISM_FLOOR: f64 = 15.0;

impl AdvancedAi {
    /// The rival city a Trader in `origin` should open a route to, with that
    /// route's value, or `None` when the gene does not claim this Trader.
    pub(super) fn culture_route_claim(
        &self,
        g: &Game,
        pid: usize,
        origin: u32,
        strategy: GrandStrategy,
    ) -> Option<(f64, u32)> {
        if !self.culture_routes_reach_every_rival
            || self.lane_lost
            || self.decision_objective(strategy) != GrandStrategy::Culture
            || g.tourism_per_turn(pid) < CULTURE_ROUTE_TOURISM_FLOOR
        {
            return None;
        }
        g.cities
            .values()
            .filter(|city| {
                let owner = city.owner;
                let rival = &g.players[owner];
                owner != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !g.same_team(pid, owner)
                    && !g.is_at_war(pid, owner)
                    && !g.has_tourism_trade_route(pid, owner)
            })
            .filter(|city| g.can_establish_trade_route(pid, origin, city.id))
            .map(|city| {
                (
                    g.domestic_tourists(city.owner),
                    self.trade_route_destination_value_from(g, pid, Some(origin), city, strategy),
                    city.id,
                )
            })
            .max_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then_with(|| left.1.total_cmp(&right.1))
                    .then_with(|| right.2.cmp(&left.2))
            })
            .map(|(_, value, city)| (value, city))
    }
}

#[cfg(test)]
mod tests;
