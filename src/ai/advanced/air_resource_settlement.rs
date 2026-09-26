//! Consider a defended resource colony for the committed bomber wing.

use super::{AdvancedAi, VictoryTarget, CITY_STATE_SETTLEMENT_BUFFER};
use crate::game::Game;
use crate::Pos;
use std::collections::BTreeSet;

impl AdvancedAi {
    /// The ordinary city-state buffer protects an unsupported colony from a
    /// future hostile Suzerain. A needed city-center resource may instead be
    /// considered when every nearby minor answers to us, a healthy land
    /// defender is close, and the colony can hold its Loyalty without help.
    /// This changes candidate eligibility only; it never orders a Settler.
    pub(super) fn air_resource_settlement_sites(
        &self,
        g: &Game,
        pid: usize,
        excluded: &BTreeSet<Pos>,
    ) -> BTreeSet<Pos> {
        if !self.air_surge_enabled()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return BTreeSet::new();
        }
        let Some(resource) = self.air_resource_shortfall(g, pid) else {
            return BTreeSet::new();
        };
        if self.threatened_city(g, pid).is_some() {
            return BTreeSet::new();
        }
        let visible = g.player_vision_frame(pid);
        excluded
            .iter()
            .copied()
            .filter(|site| {
                let Some(tile) = g.map.get(*site) else {
                    return false;
                };
                if tile.resource != Some(resource)
                    || tile.owner_city.is_some()
                    || tile.flooded
                    || tile.submerged
                    || !g.sees(&visible, *site)
                    || g.rules.is_water(tile)
                    || !g.rules.is_passable(tile)
                    || g.tile_is_natural_wonder(tile)
                    || g.blocked_city_sites.contains(site)
                    || g.cities.values().any(|city| g.wdist(city.pos, *site) < 4)
                {
                    return false;
                }
                let protected = g
                    .cities
                    .values()
                    .filter(|city| {
                        g.players[city.owner].alive
                            && g.players[city.owner].is_minor
                            && g.players[pid].explored.contains(&city.pos)
                            && g.wdist(city.pos, *site) <= CITY_STATE_SETTLEMENT_BUFFER
                    })
                    .all(|city| {
                        g.suzerain_of(city.owner) == Some(pid) && !g.is_at_war(pid, city.owner)
                    });
                let defender = g.units.values().any(|unit| {
                    let spec = &g.rules.units[unit.kind];
                    unit.owner == pid
                        && unit.hp >= 75
                        && g.wdist(unit.pos, *site) <= 4
                        && spec.class == "military"
                        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                        && !spec.has_ranged_attack()
                });
                if !protected
                    || !defender
                    || self.settlement_tile_risk_with_support(g, pid, None, *site, &visible, false)
                        > 0.0
                    || self
                        .settle_site_frontier_loyalty_verdict(g, pid, *site)
                        .is_some()
                {
                    return false;
                }
                // The generic rate alarm can be withheld by the live genome.
                // This exception always pays the full forecast and admits no
                // negative rate, even a slow loss below the generic alarm.
                let mut forecast = g.speculative_clone();
                let city = forecast.found_city_for(pid, *site, None);
                forecast.city_loyalty_per_turn(&forecast.cities[&city]) >= 0.0
            })
            .collect()
    }

    /// Recheck the exception at arrival and at stalled founding. A cached
    /// march must not retain permission after the supporting facts change.
    pub(super) fn air_resource_colony_refused(&self, g: &Game, pid: usize, site: Pos) -> bool {
        if !self.settlement_safety
            || !self.air_surge_enabled()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !Self::air_surge_bomber(g, pid).is_some_and(|bomber| {
                g.rules.units[bomber]
                    .requires_resource
                    .is_some_and(|resource| {
                        g.map
                            .get(site)
                            .is_some_and(|tile| tile.resource == Some(resource))
                    })
            })
            || !g.cities.values().any(|city| {
                g.players[city.owner].alive
                    && g.players[city.owner].is_minor
                    && g.players[pid].explored.contains(&city.pos)
                    && g.wdist(city.pos, site) <= CITY_STATE_SETTLEMENT_BUFFER
            })
        {
            return false;
        }
        !self
            .air_resource_settlement_sites(g, pid, &BTreeSet::from([site]))
            .contains(&site)
    }
}

#[cfg(test)]
mod tests;
