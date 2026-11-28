//! `science-seat-writes-first`: a Science-target seat that knows
//! [`SCIENCE_WRITES_FIRST_TECHS`] technologies and not Writing makes Writing
//! its forced research goal, so its next free research slot walks toward the
//! Campus and Library unlock instead of the lane's long beeline.
//!
//! **Why (hotseat `hotseat-20261128T031456Z`, seat p3 "moonshot", Cree,
//! Science target, 2026-11-27).** The seat researched Animal Husbandry (t1),
//! Mining (t6), Pottery (t10) and Sailing (t12, still in progress at t16),
//! each journalled "the most valuable step toward rocketry, which science
//! needs": the Science lane's forced goal is Rocketry from turn one
//! (`science_victory_tech_goal`), and `goal_pick` takes a step toward it,
//! by value with `beeline-orders-by-value` (in the deployment genome, hence
//! "most valuable"), by printed cost with ties by name without it. Writing
//! (50, after Pottery) leads to Rocketry too and lost to Sailing (50), so a
//! Science seat had no Writing, no Campus and five Science a turn at turn 16,
//! with one city.
//! `science-writing-before-the-luxury` takes the same slot only from the
//! second city (Korea from Pottery); p3 had no second city by turn 16.
//!
//! **The rule.** In `advanced_research`, in that gene's arm of the forced-goal
//! ladder (behind every defence, barbarian, war, launch and named-production
//! arm, ahead of Astrology, the luxury connection, the bargains and the lane
//! beeline): once the seat knows two technologies, Writing is the goal. The
//! ordinary prerequisite walk applies, so a seat without Pottery takes Pottery
//! first. A technology already in progress finishes as before (the research
//! pick never replaces an in-flight path for this arm). Off, with Writing
//! known, before the second technology or on any other target, nothing
//! changes. Grep the journal for `science-seat-writes-first:`.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

/// How many technologies the seat knows before Writing becomes its goal: the
/// opening's first two (p3 knew Animal Husbandry and Mining at turn 10).
pub(crate) const SCIENCE_WRITES_FIRST_TECHS: usize = 2;

impl AdvancedAi {
    /// The forced research goal: `Some("writing")` on a Science-target seat
    /// that knows [`SCIENCE_WRITES_FIRST_TECHS`] technologies and not
    /// Writing; `None` with the gene off or otherwise.
    pub(super) fn science_seat_writes_first_goal(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        let techs = &g.players[pid].techs;
        (self.science_seat_writes_first
            && self.active_victory_target(g) == Some(VictoryTarget::Science)
            && techs.len() >= SCIENCE_WRITES_FIRST_TECHS
            && !techs.contains(&crate::name!("writing")))
        .then_some("writing")
    }
}

#[cfg(test)]
mod tests;
