//! `anvil-follows-the-defense`: a Defend force forms around its own city.
//!
//! Read the stable task-force key, never the group's tactical contact tile.
//! Other board missions retain ordinary movement. With either the board or
//! this opt-in gene disabled, preserve the original global-threat routing.
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
        let cid = if self.anvil_follows_the_defense && self.objective_board {
            // Defend has one task force per domain. Relieve, offensive and
            // unassigned forces keep their own mission instead of being borrowed
            // by the global threat city's formation.
            let force = self
                .objective_board_state
                .forces
                .iter()
                .find(|force| force.id == group.id && force.domain == ForceDomain::Land)?;
            match force.objective_key {
                ObjectiveKey::Defend(cid) => cid,
                _ => return None,
            }
        } else {
            let cid = plan.threatened_city?;
            let city = g.cities.get(&cid).filter(|city| city.owner == pid)?;
            let nearest = self
                .force_groups
                .iter()
                .filter(|other| other.domain == ForceDomain::Land)
                .min_by_key(|other| (g.wdist(other.anchor, city.pos), other.id))
                .map(|other| other.id)?;
            if group.id != nearest {
                return None;
            }
            cid
        };
        let city = g.cities.get(&cid).filter(|city| city.owner == pid)?;
        (group.domain == ForceDomain::Land
            && g.wdist(group.anchor, city.pos) <= THREAT_RELIEF_RADIUS)
            .then_some(cid)
    }
}

#[cfg(test)]
mod tests;
