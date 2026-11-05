//! A bounded Science foundation before optional advanced military research.
//!
//! Native 002152 legacy had six Universities but no labs at t150. At t133
//! Chemistry needed only Sanitation; the chooser instead began the tank path.
//! This opt-in opens usable labs, then nearby Rocketry. Recovery, threatened
//! cities and pre-Modern army upgrades retain their research. The match in
//! `research.rs` also retains walls, committed breakthroughs, siege packages,
//! barbarian emergencies, fuel and named production technology.

use super::*;

impl AdvancedAi {
    pub(super) fn science_foundation_research_goal(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<&'static str> {
        let known = &g.players[pid].techs;
        if !self.science_foundation_before_backlog
            || self.space_race_excluded()
            || !g.victory_conditions.science
            || self.raced_target() != Some(VictoryTarget::Science)
            || self.decision_objective(plan.strategy) != GrandStrategy::Science
            || plan.strategy == GrandStrategy::Recovery
            || plan.threatened_city.is_some()
            || plan.rush
            || known.contains(&crate::name!("rocketry"))
        {
            return None;
        }
        let chemistry = g.rules.techs.get(&crate::name!("chemistry"))?;
        // Preserve the operator's Warrior/Archer modernization requirement,
        // including a minority of old units hidden by a larger modern stack.
        if g.units
            .values()
            .filter(|unit| unit.owner == pid)
            .any(|unit| {
                let Some(held) = g.rules.units.get(&unit.kind) else {
                    return false;
                };
                if held.class != "military"
                    || held.promotion_class == "recon"
                    || matches!(held.domain.as_deref(), Some("sea" | "air"))
                {
                    return false;
                }
                let Some(next) = held.upgrade_to else {
                    return false;
                };
                let next = &g.rules.units[g.player_unit_replacement(pid, next)];
                next.buildable
                    && next.civic.is_none()
                    && next.tech.is_some_and(|tech| {
                        !known.contains(&tech)
                            && g.rules
                                .techs
                                .get(&tech)
                                .is_some_and(|s| s.era < chemistry.era)
                    })
            })
        {
            return None;
        }
        let goal = if !known.contains(&crate::name!("chemistry")) {
            let lab = g.rules.buildings.get(&crate::name!("research_lab"))?;
            let ready = g.cities.values().any(|city| {
                city.owner == pid
                    && city
                        .districts
                        .keys()
                        .any(|d| g.district_family(*d) == crate::name!("campus"))
                    && lab.requires.iter().all(|b| city.buildings.contains(b))
            });
            if !ready {
                return None;
            }
            "chemistry"
        } else {
            "rocketry"
        };
        // No long new beeline: every missing parent must itself be legal now.
        // This closes again if the empire is still several generations away.
        let available = g.available_techs(pid);
        g.rules
            .techs
            .get(&Name::new(goal))?
            .requires
            .iter()
            .all(|parent| known.contains(parent) || available.contains(parent))
            .then_some(goal)
    }
}

#[cfg(test)]
mod tests;
