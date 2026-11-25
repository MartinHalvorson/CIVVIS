//! `religion-core-ends-expansion`: an assigned Religion lane that has founded
//! its religion leaves the Expansion half of its plan once it holds a core of
//! [`RELIGION_CORE_CITIES`] cities, instead of waiting for the city target or
//! the specialization clock.
//!
//! The Religion target reads Religion only while the seat has no religion
//! ("the religion lane still needs a religion"). Once it founds, `assess`
//! drops into the assigned-lane Expansion branch ("the first half is reserved
//! for expansion and defense") while the empire holds fewer cities than it
//! wants and a site remains. Under Expansion a Faith weighs 0.5 against the
//! Religion plan's 4.5, Theology loses its +120, the religious policy lists
//! and the Holy Site aims go, and `rock_convert_city` and the faith asks
//! price at the Expansion rate. `religion-lane-spreads-from-founding` keeps
//! the spenders on, but not the economy that feeds them.
//!
//! The Indefinite Benchmarking Tourney's reviewer named it in 9 of 40 games
//! on 2026-11-24 (`religion-lane-parked-for-expansion-midgame`):
//! - founded at t17-t27, Expansion from the next turn until t106-t136, every
//!   line reading "best lane religion at 100% progress";
//! - 2-12 spreads in t51-100 against 26-97 in t101-150 once the lane came
//!   back, and `converted` 0 at t100 in every Deity game;
//! - the Expansion half did not buy the cities: 194620Z and 020949Z held 2
//!   cities from t31 to t57.
//!
//! Settling does not depend on the plan reading Expansion: the Settler
//! pipeline reads `plan.desired_cities` and the payback window (see
//! `science_core_ends_expansion`, where the same switch kept founding). So a
//! founded Religion lane holding its core follows its lane, and the city
//! target, the Settler arms and the clock are unchanged. Before founding the
//! Prophet race reads as before. Off, nothing moves.

use super::*;

/// The cities a founded Religion lane holds before its lane outranks
/// Expansion: the Holy City and the two sites the opening settles before the
/// Prophet (`EXPANSION_BEFORE_PROPHET_CITIES`), the count the reviewed games
/// reached at t31-t60.
pub(super) const RELIGION_CORE_CITIES: usize = 3;

impl AdvancedAi {
    /// See the module: an assigned Religion lane that holds a religion and its
    /// core of cities. False with the gene off, for any other lane, before
    /// founding, or below the core.
    pub(super) fn religion_core_ends_expansion_for(
        &self,
        target: Option<VictoryTarget>,
        founded: bool,
        cities: usize,
    ) -> bool {
        self.religion_core_ends_expansion
            && target == Some(VictoryTarget::Religion)
            && founded
            && cities >= RELIGION_CORE_CITIES
    }
}

#[cfg(test)]
mod tests;
