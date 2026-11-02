//! Keep the first launch's technology path through a temporary posture change.
//!
//! Native Emperor `civvis-20261102T161612Z` owned Rocketry at t171,
//! researched Ballistics and Siege Tactics for barbarians, Electricity under
//! Recovery, and then the Advanced Ballistics chain for its standing army.
//! Satellites was first observed owned at t196; a rival won at t208.
//! Advanced Flight is a required prerequisite and remains on this path.
//!
//! This is an opt-in hypothesis, not a default promotion. It protects only
//! the first launch's research after Rocketry, retaining the chooser's earlier
//! immediate defenses and committed military breakthroughs. Later launch
//! technologies keep the existing endgame rules.

use super::*;

impl AdvancedAi {
    pub(super) fn first_launch_research_goal(&self, g: &Game, pid: usize) -> Option<&'static str> {
        (self.first_launch_keeps_research
            && !self.space_race_excluded()
            && g.victory_conditions.science
            && self.raced_target() == Some(VictoryTarget::Science)
            && g.players[pid].techs.contains(&crate::name!("rocketry"))
            && !g.players[pid].techs.contains(&crate::name!("satellites")))
        .then_some("satellites")
    }
}

#[cfg(test)]
mod tests;
