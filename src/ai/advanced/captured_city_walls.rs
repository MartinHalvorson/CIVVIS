//! Optional preparation of wall research after a major-city capture.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;
use crate::name::Name;

impl AdvancedAi {
    /// At a free research slot, prepare defenses for foreign land we hold
    /// while its founder remains an active major-war opponent. The existing
    /// visible-threat/recovery wall goal retains priority at the caller.
    pub(super) fn captured_city_walls_research_goal(&self, g: &Game, pid: usize) -> Option<Name> {
        let masonry = crate::name!("masonry");
        if !self.captured_city_wall_research
            || self.base.legacy_movement
            || !self.victory_planning
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || g.players[pid].techs.contains(&masonry)
        {
            return None;
        }
        g.player_city_ids(pid)
            .into_iter()
            .any(|cid| {
                let city = &g.cities[&cid];
                city.owner == pid
                    && city.original_owner != pid
                    && g.city_max_wall_hp(city) == 0
                    && g.players.get(city.original_owner).is_some_and(|original| {
                        original.alive
                            && !original.is_minor
                            && !original.is_barbarian
                            && g.is_at_war(pid, original.id)
                    })
            })
            .then_some(masonry)
    }

    pub fn enable_captured_city_wall_research(&mut self) {
        self.captured_city_wall_research = true;
    }

    pub fn disable_captured_city_wall_research(&mut self) {
        self.captured_city_wall_research = false;
    }
}

#[cfg(test)]
mod tests;
