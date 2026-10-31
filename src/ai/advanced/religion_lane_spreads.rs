//! `religion-lane-spreads-from-founding`: a Religion-target seat keeps its
//! faith spreading once it has founded.
//!
//! The Religion target maps to `GrandStrategy::Religion` only while the seat
//! has no religion ("the religion lane still needs a religion"). Once it has
//! founded, the shared expansion branch puts every lane in Expansion until
//! half the clock or the city target. Three switches read the plan, not the
//! target:
//! - `religious_offensive_posture` (buying spreaders and moving them on
//!   foreign cities);
//! - the committed 80-Faith reserve, against 1,200 off-plan;
//! - `religious_production` (Shrine and Temple in idle Holy Site cities).
//!
//! So a founder sat on its Faith from its founding (median t44-54 in the 6p
//! Immortal live config) until about t125. A 6-game native Religion screen
//! (calibrated rivals, 10-31) founded in 5, won 0, and ended to rival culture
//! or science at t145-176: the corps woke 20-50 turns before the end.
//!
//! With the gene on, a seat whose active target is Religion and that holds a
//! religion takes all three as though its plan were Religion. The plan
//! itself, and so the Expansion plan's settling, is unchanged.
//! `religious_production` only fills idle queues. The defensive purchases in
//! `religious_spending` still run first. Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// See the module. `false` with the gene off, on any other target, or
    /// before the seat holds a religion.
    pub(super) fn religion_lane_spreads(&self, g: &Game, pid: usize) -> bool {
        self.religion_lane_spreads_from_founding
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.players[pid].religion.is_some()
    }
}

#[cfg(test)]
mod tests;
