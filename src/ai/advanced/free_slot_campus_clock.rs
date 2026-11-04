//! Opt-in `free-slot-campus-clock`: let the existing free-slot Campus claim
//! spend the same cost-scaled time as its fifteen-turn Online budget. Native
//! production quotes already contain speed scaling: scale the ceiling once,
//! never the quote. This does not alter other first-district callers, claim
//! holds, rescoring protection, or the final emergency defense pass.

use super::*;

impl AdvancedAi {
    pub(super) fn free_slot_campus_turn_budget(&self, g: &Game) -> f64 {
        let base = BasicAi::FIRST_CAMPUS_MAX_TURNS;
        if self.free_slot_campus_clock {
            base * g.game_speed.cost_percent() as f64
                / crate::setup::GameSpeed::Online.cost_percent() as f64
        } else {
            base
        }
    }
}

#[cfg(test)]
mod tests;
