//! A bounded Science foundation before optional advanced military research.
//!
//! Native 002152 legacy had six Universities but no labs at t150. At t133
//! Chemistry needed only Sanitation; the chooser instead began the tank path.
//! This opt-in opens usable labs, then nearby Rocketry. Recovery, threatened
//! cities and pre-Modern army upgrades retain their research. The match in
//! `research.rs` also retains walls, committed breakthroughs, siege packages,
//! barbarian emergencies, fuel and named production technology.
//!
//! `chemistry-follows-the-university` opens the same goal for Chemistry alone
//! and without the last-step test, under the same gates, except that only
//! Warriors and Archers hold it back.

use super::*;

impl AdvancedAi {
    pub(super) fn science_foundation_research_goal(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<&'static str> {
        let known = &g.players[pid].techs;
        if !(self.science_foundation_before_backlog || self.chemistry_follows_the_university)
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
        //
        // `chemistry-follows-the-university` reads that requirement as the
        // operator gave it (11-02: "...that we can't upgrade archers or
        // warriors"): only those two lines and their uniques hold Chemistry
        // back. Any older land unit held it on 84% of live boards with a
        // University standing (291 of 347), mostly a Horseman waiting on
        // Castles or a Musketman on Military Science.
        let warriors_and_archers_only =
            self.chemistry_follows_the_university && !self.science_foundation_before_backlog;
        if g.units
            .values()
            .filter(|unit| unit.owner == pid)
            .any(|unit| {
                let Some(held) = g.rules.units.get(&unit.kind) else {
                    return false;
                };
                if warriors_and_archers_only {
                    let line = held.replaces.unwrap_or(unit.kind);
                    if line != "warrior" && line != "archer" {
                        return false;
                    }
                }
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
            // `chemistry-follows-the-university`: a standing University opens
            // the whole path to the Research Lab, however many parents are
            // left. Live Deity took Chemistry at t128-146 with Universities
            // already standing, Scientific Theory 10-28 turns behind it; the
            // parent test below kept this goal closed on those boards.
            //
            // A coastal seat waits for Cartography, which it reaches at a
            // median t91: Scientific city-states earn 13% of Science from
            // t100, late contact costs 5%, and hulls need Cartography to
            // cross. The walk took it from t91 to t125 on a journal seed.
            if self.chemistry_follows_the_university {
                let waits_for_the_sea = BasicAi::empire_is_coastal(g, pid)
                    && !known.contains(&crate::name!("cartography"));
                return (!waits_for_the_sea).then_some("chemistry");
            }
            "chemistry"
        } else if self.science_foundation_before_backlog {
            "rocketry"
        } else {
            return None;
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
