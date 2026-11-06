//! The selected Science beeline may cross the rolling research window.
//!
//! The ordinary window keeps useful old branches in view. An explicitly
//! assigned Science seat already bypasses it for a legal victory milestone;
//! this opt-in also admits its legal prerequisite steps. Goal precedence and
//! the existing prerequisite scorer continue to decide what is researched.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;
use crate::name::Name;

impl AdvancedAi {
    pub(super) fn admit_science_chain_steps(
        &self,
        g: &Game,
        pid: usize,
        victory_goal: Option<&str>,
        selected_goal: Option<&str>,
        available: &mut Vec<Name>,
    ) {
        let Some(goal) = victory_goal else { return };
        if !self.science_chain_crosses_window
            || self.victory_target != Some(VictoryTarget::Science)
            || !g.victory_conditions.science
            || selected_goal != Some(goal)
        {
            return;
        }
        for tech in g.available_techs(pid) {
            if self.tech_leads_to(g, &tech, goal) && !available.contains(&tech) {
                available.push(tech);
            }
        }
    }
}

#[cfg(test)]
mod tests;
