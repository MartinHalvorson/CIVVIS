//! Emergency patronage must actually reach the age threshold while it matters.

use super::{Action, AdvancedAi, Game};

const AGE_CLOSER_STANDARD_TURNS: u32 = 5;

impl AdvancedAi {
    pub fn enable_age_closer_2(&mut self) {
        self.age_closer = false;
        self.age_closer_2 = true;
    }

    pub fn disable_age_closer_2(&mut self) {
        self.age_closer_2 = false;
    }

    pub(super) fn age_closing_deadline(&self, g: &Game, pid: usize) -> Option<u32> {
        if !self.age_closer_2 || g.players[pid].era_score >= g.players[pid].normal_age_threshold {
            return None;
        }
        let end = g.world_era_countdown_end?;
        let remaining = end.checked_sub(g.turn)?;
        (end < g.max_turns && remaining <= g.standard_duration(AGE_CLOSER_STANDARD_TURNS))
            .then_some(end)
    }

    pub(super) fn purchase_reaches_normal_age(g: &Game, pid: usize, action: &Action) -> bool {
        let threshold = g.players[pid].normal_age_threshold;
        if g.players[pid].era_score >= threshold {
            return false;
        }
        // The engine selects the one- or three-point patronage Moment, then
        // applies Taj Mahal, Dedications and the person's actual activation.
        // Reusing that action also refuses an illegal or unavailable offer.
        let mut after = g.speculative_clone();
        after.apply(pid, action).is_ok() && after.players[pid].era_score >= threshold
    }
}

#[cfg(test)]
mod tests;
