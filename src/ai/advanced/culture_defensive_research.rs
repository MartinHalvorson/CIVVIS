use super::{
    AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget, IMMINENT_ATTACK_STRENGTH_RATIO,
};
use crate::game::Game;
use crate::name::Name;

impl AdvancedAi {
    pub(super) fn defensive_walls_research_goal(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<Name> {
        let masonry = crate::name!("masonry");
        let target = self.active_victory_target(g);
        if !self.victory_planning
            || !matches!(
                target,
                Some(VictoryTarget::Culture | VictoryTarget::Domination)
            )
            || g.players[pid].techs.contains(&masonry)
        {
            return None;
        }
        let unwalled: Vec<_> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| g.city_max_wall_hp(&g.cities[cid]) == 0)
            .collect();
        if unwalled.is_empty() {
            return None;
        }
        // Preserve the Culture recovery warning and give a recovering
        // Domination empire the same chance to unlock its first defenses.
        if plan.strategy == GrandStrategy::Recovery
            && g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
        {
            return Some(masonry);
        }
        // Research and construction need lead time. A visible land attacker
        // that overmatches an unwalled center and has a short route to its
        // approach is already a reason to unlock walls, even when terrain or
        // screening units keep it out of this turn's attack envelope.
        if target == Some(VictoryTarget::Domination) {
            let visible = g.player_vision_frame(pid);
            if unwalled.into_iter().any(|cid| {
                let city = &g.cities[&cid];
                let defense = g.city_strength(cid).max(1.0);
                g.units.values().any(|unit| {
                    let spec = &g.rules.units[unit.kind];
                    unit.owner != pid
                        && g.is_at_war(pid, unit.owner)
                        && spec.class == "military"
                        && spec.is_melee_capable()
                        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                        && g.map
                            .get(unit.pos)
                            .is_some_and(|tile| !g.rules.is_water(tile))
                        && g.wdist(city.pos, unit.pos) <= 4
                        && g.sees(&visible, unit.pos)
                        && g.unit_visible_to(unit.id, pid)
                        && crate::game::effective_strength(g.unit_strength(unit, false), unit.hp)
                            >= defense * IMMINENT_ATTACK_STRENGTH_RATIO
                        && g.route_distance(unit.id, city.pos, 1)
                            .is_some_and(|steps| steps <= 4)
                })
            }) {
                return Some(masonry);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
