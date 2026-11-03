//! `anvil-follows-the-defense`: a Defend force forms around its own city.
//!
//! The board can assign Defend Constantinople while the independent strategic
//! plan names Trebizond. The original Anvil reads the latter and takes the
//! nearest land force away from its board assignment. This candidate reads the
//! stable task-force key, never the group's tactical contact tile. Other board
//! missions retain their ordinary movement. Board disabled or gene withheld:
//! preserve the original nearest-force/global-threat routing.
use super::*;

impl AdvancedAi {
    pub(super) fn anvil_defense_city(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        group: &ForceGroup,
    ) -> Option<u32> {
        let cid = plan.threatened_city?;
        let city = g.cities.get(&cid).filter(|city| city.owner == pid)?;
        let nearest = self.force_groups.iter()
            .filter(|other| other.domain == ForceDomain::Land)
            .min_by_key(|other| (g.wdist(other.anchor, city.pos), other.id))
            .map(|other| other.id)?;
        (group.id == nearest && g.wdist(group.anchor, city.pos) <= THREAT_RELIEF_RADIUS)
            .then_some(cid)
    }
}

#[cfg(test)]
mod tests;
