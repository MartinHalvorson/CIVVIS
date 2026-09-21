//! Count usable counter-faith Missionaries when funding religious defense.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

impl AdvancedAi {
    pub(super) fn religious_defense_missionary_count(
        &self,
        g: &Game,
        pid: usize,
        threat: &str,
    ) -> usize {
        let counterfaith_only = self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.victory_conditions.religious
            && g.players[pid].religion.is_none();
        g.units
            .values()
            .filter(|unit| unit.owner == pid && unit.kind == "missionary")
            .filter(|unit| {
                !counterfaith_only
                    || (unit.charges > 0
                        && unit.religion.as_deref().is_some_and(|faith| {
                            faith != threat && Self::safe_adopted_counterfaith(g, pid, faith)
                        }))
            })
            .count()
    }
}

#[cfg(test)]
mod tests;
