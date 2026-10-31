//! `religion-lane-hands-over`: a Religion-target seat that has lost the
//! Prophet race plays Science.
//!
//! The Religion target maps to `GrandStrategy::Religion` while the seat has
//! no religion ("the religion lane still needs a religion"). If the last
//! religion slot goes to a rival, that branch still reads true, so the seat
//! plays a Religion plan, with no religion, for the rest of the game. The
//! homeland religious defence is also skipped for a Religion plan off
//! Domination.
//!
//! Live 10-30/31: in the 6p Immortal config the fourth religion closed the
//! race at a median t47.5 (t34-72), and our seat founded in only 5 of 17.
//! Native, 1 of 6 Religion-target seats missed (Phoenicia, 2 cities and 0-1
//! Faith a turn to t50) and ended with 9 of 10 cities on a foreign faith.
//!
//! With the gene on, once no religion is ours, no Prophet of ours is pending,
//! and every slot is claimed (founded religions plus pending Prophets), the
//! seat retargets to Science, the lane that came nearest at Immortal (census
//! of 965 seats, 10-31). If Science is off it takes Culture, then
//! Domination. The retarget is permanent: no slot reopens. Off, nothing
//! changes.

use super::*;

impl AdvancedAi {
    /// Whether no religion of ours can still be founded: none held, no
    /// Prophet of ours pending, and the founded religions plus the Prophets
    /// other majors hold fill every slot.
    pub(super) fn religion_race_lost(g: &Game, pid: usize) -> bool {
        let player = &g.players[pid];
        if player.religion.is_some() || player.prophet_pending {
            return false;
        }
        let claimed = g.religions_founded()
            + g.players
                .iter()
                .filter(|candidate| candidate.prophet_pending)
                .count();
        claimed >= g.max_religions()
    }

    /// The lane a lost Religion seat takes: the first of Science, Culture and
    /// Domination the game enables.
    fn religion_handover_lane(g: &Game) -> Option<VictoryTarget> {
        [
            VictoryTarget::Science,
            VictoryTarget::Culture,
            VictoryTarget::Domination,
        ]
        .into_iter()
        .find(|lane| g.victory_conditions.is_enabled(lane.as_str()))
    }

    /// See the module. Runs once a turn, before the turn reads its target.
    /// A no-op with the gene off, on any other target, or while the race is
    /// open.
    pub(super) fn hand_over_a_lost_religion_lane(&mut self, g: &Game, pid: usize) {
        if !self.religion_lane_hands_over
            || self.victory_target != Some(VictoryTarget::Religion)
            || !Self::religion_race_lost(g, pid)
        {
            return;
        }
        let Some(lane) = Self::religion_handover_lane(g) else {
            return;
        };
        think!(self.journal(), Strategy, Strategy,
               "Handing the Religion lane to {}", lane.as_str();
               "religion-lane-hands-over: all {} religions are claimed and none is ours, so no Prophet we recruit can found one",
               g.max_religions());
        self.retarget(lane);
    }
}

#[cfg(test)]
mod tests;
