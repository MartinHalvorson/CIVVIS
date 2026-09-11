use super::*;

impl AdvancedAi {
    /// A roster full of field units can still lack the ability to break walls.
    /// Counts include queued units, so only the first siege order gets this
    /// composition exception to the ordinary army ceiling.
    pub(super) fn missing_domination_siege(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
        spec: &crate::rules::UnitSpec,
    ) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && plan.strategy == GrandStrategy::Conquest
            && counts.siege == 0
            && spec.siege
            && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            && plan
                .target_city
                .and_then(|id| g.cities.get(&id))
                .is_some_and(|city| {
                    Some(city.owner) == plan.target_player
                        && city.wall_hp > 0
                        && g.is_at_war(pid, city.owner)
                })
    }
}

#[cfg(test)]
mod tests;
