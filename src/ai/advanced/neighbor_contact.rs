//! Version two of the early contact window prices the first major contact
//! as well as city-state contacts. It reads our contact ledger and explored
//! frontier, never another player's hidden city or starting position.

use super::{AdvancedAi, EARLY_CONTACT_UNMET_VALUE};
use crate::game::Game;

const NEIGHBOR_FRONTIER_RANGE: i32 = 12;

impl AdvancedAi {
    /// Raw production value, before the shared per-Scout discount. Only the
    /// first two Scouts receive this incentive, and only until one living
    /// rival major has been met. City-state incentives remain independent.
    pub(super) fn neighbor_contact_value(&self, g: &Game, pid: usize, scouts: usize) -> f64 {
        if scouts >= 2 {
            return 0.0;
        }
        let mut unmet_major = false;
        for other in &g.players {
            if other.id == pid
                || !other.alive
                || other.is_minor
                || other.is_barbarian
                || other.is_free_city
                || g.same_team(pid, other.id)
            {
                continue;
            }
            if g.has_met(pid, other.id) {
                return 0.0;
            }
            unmet_major = true;
        }
        if !unmet_major {
            return 0.0;
        }
        let cities = g.player_city_ids(pid);
        let explored = &g.players[pid].explored;
        let near_frontier = explored.iter().any(|pos| {
            cities
                .iter()
                .any(|cid| g.wdist(g.cities[cid].pos, *pos) <= NEIGHBOR_FRONTIER_RANGE)
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
                && g.nbrs(*pos)
                    .into_iter()
                    .any(|next| !explored.contains(&next))
        });
        if near_frontier {
            // One neighbor is worth two city-state leads. This is a fixed
            // first-contact incentive, not a subsidy for every distant civ.
            2.0 * EARLY_CONTACT_UNMET_VALUE
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests;
