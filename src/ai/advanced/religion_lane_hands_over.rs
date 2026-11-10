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
//!
//! The native host can report the Prophet class exhausted before every
//! rival founds. In `civvis-20261102T203602Z` that public signal arrived at
//! t39, while the fourth religion appeared at t57. Count that same terminal
//! signal used by `prophet_class_exhausted`, with the existing exception for
//! a Prophet already pending for our seat. Missing telemetry stays unknown.
//!
//! The reading must still hold on a later turn. The host can report the class
//! exhausted for one frame as our own Prophet arrives, before it marks that
//! Prophet pending. `civvis-20261110T081626Z` handed over at t19 with 0 of 4
//! religions founded, had its Prophet pending at t20 and founded that turn. The
//! seat then played Science: it founded on Tithe, never ordered a Shrine, held
//! 722 Faith at t68 and lost its religion. All four Russia games founded on
//! Tithe that day went the same way.

use super::*;

impl AdvancedAi {
    /// Whether no religion of ours can still be founded: none held, no
    /// Prophet of ours pending, and the founded religions plus the Prophets
    /// other majors hold fill every slot, or the host reports none left.
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
        claimed >= g.max_religions() || Self::prophet_class_exhausted(g, pid)
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
        if !self.religion_lane_hands_over || self.victory_target != Some(VictoryTarget::Religion) {
            return;
        }
        if !Self::religion_race_lost(g, pid) {
            self.religion_race_lost_since = None;
            return;
        }
        // The retarget is permanent, so one frame's reading cannot make it.
        let first_read = *self.religion_race_lost_since.get_or_insert(g.turn);
        if g.turn <= first_read {
            return;
        }
        let Some(lane) = Self::religion_handover_lane(g) else {
            return;
        };
        think!(self.journal(), Strategy, Strategy,
               "Handing the Religion lane to {}", lane.as_str();
               "religion-lane-hands-over: {} of {} religions founded, host Prophet class exhausted={}; none is ours and no Prophet is pending",
               g.religions_founded(), g.max_religions(), Self::prophet_class_exhausted(g, pid));
        self.retarget(lane);
    }
}

#[cfg(test)]
mod tests;
