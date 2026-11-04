//! `domination-targets-land-first`: a Domination seat aims at the rivals it
//! can march to before the ones across water.
//!
//! The Domination lane picks its war target by straight-line reach and value:
//! the frontier pick, the capital pick and the hand-over bar's "weakest rival
//! in reach" all ignore water. On 6p Continents the army cannot be staged at
//! peace on an overseas target: `stage-march-keeps-to-land` and
//! `come-ashore` keep it on land, and the amphibious train forms only at war.
//! So a plan aimed overseas never declares. Live Immortal 041027Z (Korea,
//! 10-31) aimed at overseas Russia (Novgorod) from t78 to t85 and was retired
//! at t99 with no city taken. In a census of the 10 long 6p Continents games,
//! 22 of 45 rival slots were overseas, while every seat had at least one
//! rival on its own landmass.
//!
//! With the gene on, a Domination seat ranks rivals reachable by land
//! (`rival_reachable_by_land`, the `denial-needs-a-road` search) ahead of the
//! rest in the frontier and capital picks. The hand-over bar reads only those
//! rivals as "the weakest in reach". With no land rival the picks fall back
//! to the existing order. Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether the gene reorders this seat's Domination picks.
    pub(super) fn domination_land_first_active(&self, g: &Game) -> bool {
        self.domination_targets_land_first
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
    }

    /// The pick's leading key: 0 for a rival reachable by land (or with the
    /// gene inactive), 1 for one across water.
    pub(super) fn domination_land_rank(&self, g: &Game, pid: usize, rival: usize) -> u8 {
        u8::from(
            self.domination_land_first_active(g) && !self.rival_reachable_by_land(g, pid, rival),
        )
    }
}

#[cfg(test)]
mod tests;
