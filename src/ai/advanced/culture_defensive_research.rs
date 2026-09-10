use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::Game;
use crate::name::Name;

impl AdvancedAi {
    pub(super) fn culture_defensive_walls_goal(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<Name> {
        let masonry = crate::name!("masonry");
        if !self.victory_planning
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || plan.strategy != GrandStrategy::Recovery
            || g.players[pid].techs.contains(&masonry)
            || !g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
        {
            return None;
        }
        // Recovery can begin from a wartime power deficit before a single
        // city crosses the tactical emergency threshold. Waiting for that
        // threshold leaves no time to research and then build the walls.
        g.player_city_ids(pid)
            .into_iter()
            .any(|cid| g.city_max_wall_hp(&g.cities[&cid]) == 0)
            .then_some(masonry)
    }
}

#[cfg(test)]
mod tests;
