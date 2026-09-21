//! Give a Domination economy its first science/culture governor after the
//! non-founder religious defense has reached Citadel of God, before it can
//! train Apostles that benefit from Patron Saint.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

impl AdvancedAi {
    pub(super) fn domination_needs_economic_governor(&self, g: &Game, pid: usize) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.players[pid]
                .governor_roster
                .get("moksha")
                .is_some_and(|state| state.promotions.contains("citadel_of_god"))
            && !g.player_city_ids(pid).into_iter().any(|cid| {
                let city = &g.cities[&cid];
                city.buildings.iter().any(|building| {
                    g.building_is_family(*building, crate::name!("temple"))
                        && !city.pillaged_buildings.contains(building)
                })
            })
            && !g.players[pid]
                .governor_roster
                .get("pingala")
                .is_some_and(|state| {
                    state.promotions.contains("researcher")
                        && state.promotions.contains("connoisseur")
                })
    }
}

#[cfg(test)]
mod tests;
