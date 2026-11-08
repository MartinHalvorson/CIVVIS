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
    /// Native origin relocation for an idle Trader whose proposed route would
    /// repeat a market. Positive host route options are the authority for the
    /// other origin; the bridge still asks Firaxis to permit the transfer.
    pub fn native_culture_trade_origin(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        proposed_destination: u32,
        covered: &std::collections::BTreeSet<usize>,
    ) -> Option<(u32, usize)> {
        if !self.culture_routes_reach_every_rival
            || self.lane_lost
            || self.active_victory_target(g) != Some(super::VictoryTarget::Culture)
            || g.tourism_per_turn(pid) < CULTURE_ROUTE_TOURISM_FLOOR
            || g.host_unit_facts.get(&uid)?.civ6_id.is_none()
        {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.owner != pid || unit.kind != "trader" {
            return None;
        }
        let current = g.city_at(unit.pos)?;
        if g.cities[&current].owner != pid {
            return None;
        }
        let destination = g.cities.get(&proposed_destination)?;
        let owner = &g.players[destination.owner];
        if !owner.is_minor
            && !owner.is_barbarian
            && destination.owner != pid
            && !covered.contains(&destination.owner)
        {
            return None; // The existing plan already opens a new market.
        }
        let missing_market = |destination: u32| {
            g.cities.get(&destination).is_some_and(|city| {
                let rival = &g.players[city.owner];
                city.owner != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !g.same_team(pid, city.owner)
                    && !g.is_at_war(pid, city.owner)
                    && !covered.contains(&city.owner)
            })
        };
        if g.observed_route_options
            .keys()
            .any(|&(origin, destination)| {
                origin == current
                    && missing_market(destination)
                    && g.can_establish_trade_route(pid, origin, destination)
            })
        {
            return None; // An origin change is unnecessary here.
        }
        g.observed_route_options
            .keys()
            .filter_map(|&(origin, destination)| {
                let city = g.cities.get(&origin)?;
                let target = g.cities.get(&destination)?;
                let rival = &g.players[target.owner];
                if origin == current
                    || city.owner != pid
                    || city.hp < 200
                    || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                    || target.owner == pid
                    || !rival.alive
                    || rival.is_minor
                    || rival.is_barbarian
                    || g.same_team(pid, target.owner)
                    || g.is_at_war(pid, target.owner)
                    || covered.contains(&target.owner)
                    || !g.can_establish_trade_route(pid, origin, destination)
                {
                    return None;
                }
                let value = self.trade_route_destination_value_from(
                    g,
                    pid,
                    Some(origin),
                    target,
                    GrandStrategy::Culture,
                );
                value.is_finite().then_some((
                    g.domestic_tourists(target.owner),
                    value,
                    origin,
                    target.owner,
                ))
            })
            .max_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| a.1.total_cmp(&b.1))
                    .then_with(|| b.2.cmp(&a.2))
                    .then_with(|| b.3.cmp(&a.3))
            })
            .map(|(_, _, origin, owner)| (origin, owner))
    }

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
