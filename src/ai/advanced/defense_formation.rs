//! A Defend assignment has a city to protect, even when an easier contact
//! offers the best exchange elsewhere. Native Indonesia 214426Z assigned all
//! four soldiers to Defend Majapahit at turn 29, then formed toward a scout
//! five tiles away; its archer left the capital's reach. The city fell at 31.
//! This is evidence of a lost defensive remit, not proof of an avoidable loss.
//!
//! The independent opt-in keeps healthy combat formation slots within the
//! existing wall-defender radius and ranks contacts threatening that city.
//! Healing placements retain their ordinary safe retreat search. It changes
//! neither the kill search nor immediate legal strikes, Relieve assignments,
//! offensive forces, or arena formations. No native strength gain is claimed.
use super::objective_board::ObjectiveKey;
use super::{AdvancedAi, ForceDomain, ForceGroup};
use crate::game::Game;
use crate::Pos;

impl AdvancedAi {
    pub(super) fn defensive_formation_anchor(
        &self,
        g: &Game,
        pid: usize,
        group: &ForceGroup,
    ) -> Option<Pos> {
        if !self.defenders_keep_the_city_in_reach
            || !self.objective_board
            || g.is_arena()
            || group.domain != ForceDomain::Land
        {
            return None;
        }
        let task = self
            .objective_board_state
            .forces
            .iter()
            .find(|task| task.id == group.id)?;
        let ObjectiveKey::Defend(cid) = task.objective_key else {
            return None;
        };
        let city = g.cities.get(&cid)?;
        (city.owner == pid && self.city_under_fire_for_the_hold(g, pid, cid)).then_some(city.pos)
    }
}

#[cfg(test)]
mod tests;
