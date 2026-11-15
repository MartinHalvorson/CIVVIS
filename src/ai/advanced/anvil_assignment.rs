//! `anvil-follows-the-defense`: an eligible Anvil follows its Defend city.
//!
//! Preserve the original global warning and nearest-force eligibility. Once
//! that formation would act, its board Defend city can replace the unrelated
//! global threat city. Other missions keep the original fallback.
use super::objective_board::ObjectiveKey;
use super::*;

impl AdvancedAi {
    pub(super) fn anvil_defense_city(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        group: &ForceGroup,
    ) -> Option<u32> {
        // Preserve the original eligibility, including a global warning. A
        // board Defend row alone also occurs during ordinary early scouting.
        let original = plan.threatened_city?;
        let city = g.cities.get(&original).filter(|city| city.owner == pid)?;
        let nearest = self
            .force_groups
            .iter()
            .filter(|other| other.domain == ForceDomain::Land)
            .min_by_key(|other| (g.wdist(other.anchor, city.pos), other.id))
            .map(|other| other.id)?;
        if group.domain != ForceDomain::Land
            || group.id != nearest
            || g.wdist(group.anchor, city.pos) > THREAT_RELIEF_RADIUS
        {
            return None;
        }
        let assigned = if self.anvil_follows_the_defense && self.objective_board {
            self.objective_board_state
                .forces
                .iter()
                .find(|force| force.id == group.id && force.domain == ForceDomain::Land)
                .and_then(|force| match force.objective_key {
                    ObjectiveKey::Defend(cid) => Some(cid),
                    _ => None,
                })
        } else {
            None
        };
        let cid = assigned.unwrap_or(original);
        // `defend-force-holds-the-city`: a held city's posts stand the Anvil
        // down. `false` with the gene off.
        if self.defend_hold_holds(cid) {
            return None;
        }
        let city = g.cities.get(&cid).filter(|city| city.owner == pid)?;
        (g.wdist(group.anchor, city.pos) <= THREAT_RELIEF_RADIUS).then_some(cid)
    }
}

#[cfg(test)]
mod tests;
