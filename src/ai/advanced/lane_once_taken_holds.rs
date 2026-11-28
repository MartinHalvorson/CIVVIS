//! `lane-once-taken-holds`: an assigned victory lane that has once followed
//! its lane does not hand the plan back to Expansion afterwards.
//!
//! With an assigned lane, `assess` reads Expansion ("the first half is
//! reserved for expansion and defense") while the empire holds fewer cities
//! than it wants and a site remains. Both of those move after the lane is
//! taken: `desired_cities` grows with the empire (map room, width, rival
//! pace), and a site that was out of reach comes back into it. So a seat that
//! met its target and turned to its lane is sent back to Expansion by the
//! next city it founds.
//!
//! The Indefinite Benchmarking Tourney's reviewer named the parked Science
//! lane in 11 of 40 games on 2026-11-28
//! (`science-lane-parked-for-expansion-until-t85`). Two of them show this
//! relapse in their own why.log, every line "best lane science at 100%
//! progress":
//! - civvis-t7-20261128T023758Z (Lv 8): science at t77 with "10 cities of 10
//!   wanted", expansion at t85 ("10 of 11"), science t92 ("11 of 11"),
//!   expansion t99 ("11 of 12"), science t105; 29 techs to the rival's 42 at
//!   t100;
//! - civvis-t7-20261128T041325Z (Lv 8): science at t49 (5 of 10, no site in
//!   reach), expansion at t54 when a site opened, and after a recovery at
//!   t74 expansion again at t84; science only from t117.
//!
//! Expansion weighs a beaker at 1.2 against the Science lane's 4.2 and takes
//! the expansion districts, Great People and policies; settling does not
//! need it. The Settler pipeline reads `plan.desired_cities` and its payback
//! window, which this gene leaves alone, so the cities the target still wants
//! are founded under the lane. Recovery, denial and every other branch above
//! the lane still answer first; once they let go the plan returns to the
//! lane, not to Expansion.
//!
//! A Domination lane is left out: its hand-over to Conquest carries its own
//! army check (`domination-lane-hands-over`), which a latch would skip. A
//! change of assigned lane starts over. Off, nothing moves.

use super::*;

impl AdvancedAi {
    /// See the module: this seat has already followed `target` as its lane.
    /// False with the gene off, for Domination, or before the lane was taken.
    pub(super) fn lane_once_taken_holds_for(&self, target: VictoryTarget) -> bool {
        self.lane_once_taken_holds
            && target != VictoryTarget::Domination
            && self.lane_taken.get() == Some(target)
    }

    /// `assess` followed the assigned lane `target`. Remembered only with the
    /// gene on, so the stock agent carries no state from it.
    pub(super) fn note_lane_taken(&self, target: VictoryTarget) {
        if self.lane_once_taken_holds {
            self.lane_taken.set(Some(target));
        }
    }
}

#[cfg(test)]
mod tests;
