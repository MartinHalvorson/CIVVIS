//! `diplo-match-point-beelines-the-points`: a Diplomatic seat near its
//! victory researches Seasteads and adopts Global Warming Mitigation first.
//!
//! **Why.** Each is one Diplomatic Victory Point (`tech_seasteads_diplovp`,
//! `civic_mitigation_grant_diplovp`), and near the bar Congress stops paying:
//! live Emperor civvis-20261030T111154Z stood on 18 of 20 from t180 to its
//! loss to Culture at t213. At t200 every rival voted B on us, 19 votes, out
//! of the Favor's reach, and Global Warming Mitigation's point at t198 is what
//! held the 18. Seasteads never landed. The lane's research goal names it only
//! under a Diplomacy plan (`phase_specialization_active`); from t197 the plan
//! read Recovery and research went to Satellites, Electricity and Predictive
//! Systems.
//!
//! **What the gene does.** For an assigned Diplomatic seat that has not lost
//! its lane, on [`DIPLO_MATCH_POINT_DVP`] points or more: the research goal is
//! Seasteads until it is known, and the civic goal is Global Warming
//! Mitigation until it is adopted, whatever the plan says. Both slot in ahead
//! of the plan's own lane arms and behind the wartime, barbarian, endgame and
//! science-race goals.
//!
//! Off, below the bar, or for a seat not assigned Diplomacy, nothing changes.

use super::*;

/// Diplomatic Victory Points from which the seat buys its last points in
/// the trees. Four short: two Congress sessions' worth on a good day, and the
/// point at which rivals start to vote B on the leader.
pub(super) const DIPLO_MATCH_POINT_DVP: i64 = 16;

impl AdvancedAi {
    fn diplo_match_point(&self, g: &Game, pid: usize) -> bool {
        self.diplo_match_point_beelines
            && self.victory_target == Some(VictoryTarget::Diplomacy)
            && !self.lane_lost
            && g.players[pid].dvp >= DIPLO_MATCH_POINT_DVP
    }

    /// The research goal at a Diplomatic match point: Seasteads until known.
    pub(super) fn diplo_match_point_tech(&self, g: &Game, pid: usize) -> Option<&'static str> {
        (self.diplo_match_point(g, pid)
            && !g.players[pid].techs.contains(&crate::name!("seasteads")))
        .then_some("seasteads")
    }

    /// The civic goal at a Diplomatic match point: Global Warming Mitigation
    /// until adopted.
    pub(super) fn diplo_match_point_civic(&self, g: &Game, pid: usize) -> Option<&'static str> {
        (self.diplo_match_point(g, pid)
            && !g.players[pid]
                .civics
                .contains(&crate::name!("global_warming_mitigation")))
        .then_some("global_warming_mitigation")
    }
}

#[cfg(test)]
mod tests;
