//! Continue one observed native maneuver, not a speculative action tail.
use super::{AdvancedAi, GrandStrategy};
use crate::game::Game;
use crate::Pos;

impl AdvancedAi {
    /// Recompute the bounded local assault after the adapter's observation
    /// barrier. The caller owns same-turn custody and native ownership checks.
    /// This does not change the strategic target or replay deferred actions.
    pub fn resume_observed_air_city_assault(
        &mut self,
        g: &mut Game,
        pid: usize,
        target: Pos,
    ) -> bool {
        self.air_city_assault = None;
        if g.current != pid || g.winner.is_some() || !self.air_surge_enabled() {
            return false;
        }
        let Some(mut plan) = self.plan.clone() else {
            return false;
        };
        if plan.strategy != GrandStrategy::Conquest || self.threatened_city(g, pid).is_some() {
            return false;
        }
        let Some(cid) = g.city_at(target) else {
            return false;
        };
        let owner = g.cities[&cid].owner;
        if owner == pid || !g.is_at_war(pid, owner) {
            return false;
        }
        plan.target_player = Some(owner);
        plan.target_city = Some(cid);
        self.journal().begin_turn(g.turn, pid);
        !self.plan_air_city_assault(g, pid, &plan).is_empty()
    }
}

#[cfg(test)]
mod tests;
