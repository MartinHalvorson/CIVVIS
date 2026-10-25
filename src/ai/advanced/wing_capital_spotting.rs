//! Put one spare idle spy where the existing Bomber wing needs sight.
//!
//! Native census: `docs/eval/2026-10-25-capital-assault-audit.md`. The host's
//! `Base/Assets/Gameplay/Data/Units.xml:769` gives UNIT_SPY sight 3 and
//! TeamVisibility. Posting is preparation, not an observed native attack.

use super::*;

pub(super) enum WingSpyOrder {
    Act(Action),
    Hold,
}

impl AdvancedAi {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn wing_capital_spy_action(
        &self,
        g: &Game,
        pid: usize,
        spy_id: u32,
        plan: &StrategicPlan,
        legal: &[Action],
        racer_leader: Option<usize>,
        culture_targets: &BTreeSet<usize>,
    ) -> Option<WingSpyOrder> {
        if !self.wing_capital_spotting
            || plan.strategy != GrandStrategy::Conquest
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || self.forced_target_player.is_some()
            || plan.threatened_city.is_some()
            || self.threatened_city(g, pid).is_some()
            || legal.is_empty()
        {
            return None;
        }
        let spy = g.spies.get(&spy_id)?;
        if spy.owner != pid
            || spy.captured_by.is_some()
            || spy.mission.is_some()
            || spy.ready_turn > g.turn
        {
            return None;
        }
        if spy
            .city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| {
                city.owner == pid
                    && (city.districts.contains_key(crate::name!("spaceport"))
                        || g.spies.values().any(|other| {
                            other.owner != pid
                                && other.captured_by.is_none()
                                && other.city == Some(city.id)
                        }))
            })
        {
            return None;
        }
        // Exact denial opportunities keep priority even with another agent
        // already covering the same rival. Earlier caller hooks own postings.
        if legal.iter().any(|action| {
            matches!(action,
            Action::SpyMission { mission, .. }
                if mission == "disrupt_rocketry" || mission == "great_work_heist")
        }) {
            return None;
        }
        let covered_by_another = |owner| {
            g.spies.values().any(|other| {
                other.id != spy_id
                    && other.owner == pid
                    && other.captured_by.is_none()
                    && other.ready_turn <= g.turn
                    && other
                        .city
                        .and_then(|cid| g.cities.get(&cid))
                        .is_some_and(|city| city.owner == owner)
            })
        };
        if racer_leader.is_some_and(|owner| !covered_by_another(owner))
            || culture_targets
                .iter()
                .any(|owner| !covered_by_another(*owner))
        {
            return None;
        }
        let target = g
            .cities
            .values()
            .filter(|city| {
                city.is_capital
                    && city.owner != pid
                    && plan.target_player == Some(city.owner)
                    && g.players
                        .get(city.original_owner)
                        .is_some_and(|owner| !owner.is_minor && !owner.is_barbarian)
                    && g.players
                        .get(city.owner)
                        .is_some_and(|owner| !owner.is_minor && !owner.is_barbarian)
                    && g.is_at_war(pid, city.owner)
                    && !g.same_team(pid, city.owner)
                    && (spy.city == Some(city.id)
                        || !self.air_assault_target_visible(g, pid, city.pos))
                    && g.units
                        .values()
                        .filter(|unit| {
                            unit.owner == pid
                                && unit.hp >= 50
                                && g.rules.units[unit.kind].domain.as_deref() == Some("air")
                                && g.rules.units[unit.kind].promotion_class == "air_bomber"
                                && g.wdist(unit.pos, city.pos) <= g.unit_attack_range(unit.id)
                        })
                        .take(2)
                        .count()
                        == 2
                    && !g.spies.values().any(|other| {
                        other.id != spy_id
                            && other.owner == pid
                            && other.captured_by.is_none()
                            && other.city == Some(city.id)
                    })
            })
            .min_by_key(|city| (Some(city.id) != plan.target_city, city.id))?;
        if spy.city == Some(target.id) {
            // A safe operation may be unavailable while its benefit still
            // runs. Hold rather than gambling this sight source on unrest.
            return Some(
                ["listening_post", "gain_sources"]
                    .into_iter()
                    .find_map(|wanted| {
                        legal
                            .iter()
                            .find(|action| {
                                matches!(action,
                    Action::SpyMission { mission, .. } if mission == wanted)
                            })
                            .cloned()
                    })
                    .map_or(WingSpyOrder::Hold, WingSpyOrder::Act),
            );
        }
        legal
            .iter()
            .find(|action| {
                matches!(action,
            Action::AssignSpy { spy, city } if *spy == spy_id && *city == target.id)
            })
            .cloned()
            .map(WingSpyOrder::Act)
    }
}

#[cfg(test)]
mod tests;
