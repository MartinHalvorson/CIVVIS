use super::*;

/// The first wall breaker equips an otherwise incomplete campaign. Use the
/// opening conquest reservation scale; ordinary production-time pricing still
/// prefers a faster city and existing/queued weapons close this reservation.
pub(super) const FIRST_WEAPON_RESERVATION: f64 = 400.0;

impl AdvancedAi {
    /// A roster full of field units can still lack the ability to break walls.
    /// Counts include queued units, so only the first siege order gets this
    /// composition exception to the ordinary army ceiling. An active siege
    /// keeps this requirement when the empire replans for expansion or recovery.
    pub(super) fn missing_domination_siege(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
        spec: &crate::rules::UnitSpec,
    ) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
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

#[cfg(test)]
mod war_strategy_tests;
