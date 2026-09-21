use super::*;

impl AdvancedAi {
    /// A Domination army still needs the research that unlocks its next units.
    /// Use the catch-up planner's existing public-yield threshold: otherwise
    /// its fresh Library can be replaced before receiving a single hammer.
    pub(super) fn domination_research_catchup_needed(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !(self.research_building_catchup
                || self.research_building_catchup_2
                || self.research_building_catchup_3)
            || self.base.minor
            || self.base.barb
            || plan.strategy == GrandStrategy::Recovery
            || self.war_plan.is_some()
        {
            return false;
        }
        let science = |seat| {
            g.player_city_ids(seat)
                .into_iter()
                .map(|cid| g.city_yields(cid).science)
                .sum::<f64>()
                + g.observed_yield_adjustments
                    .get(&seat)
                    .map_or(0.0, |y| y.science)
        };
        let best = g
            .players
            .iter()
            .filter(|p| {
                p.id != pid && p.alive && !p.is_minor && !p.is_barbarian && g.has_met(pid, p.id)
            })
            .map(|p| science(p.id))
            .fold(0.0_f64, f64::max);
        science(pid) < 0.7 * best
    }

    /// A placed Campus already owns its tile and district slot. Finishing it
    /// opens the research-building chain without committing a new site.
    pub(super) fn placed_campus_research_item(g: &Game, item: &Item) -> bool {
        let Item::District { district, pos } = item else {
            return false;
        };
        g.district_family(*district) == "campus"
            && g.map.get(*pos).is_some_and(|tile| {
                tile.district_foundation
                    .as_ref()
                    .is_some_and(|foundation| foundation.district == *district)
            })
    }

    pub(super) fn campus_research_building(g: &Game, item: &Item) -> bool {
        let Item::Building { building } = item else {
            // This predicate protects the catch-up pass's fresh reservation
            // before it has invested its first hammer, including completion
            // of the district that makes those buildings possible.
            return Self::placed_campus_research_item(g, item);
        };
        let spec = &g.rules.buildings[building];
        !spec.wonder
            && spec.yields.science > 0.0
            && spec
                .district
                .is_some_and(|district| g.district_family(district) == "campus")
    }
}

#[cfg(test)]
mod tests;
