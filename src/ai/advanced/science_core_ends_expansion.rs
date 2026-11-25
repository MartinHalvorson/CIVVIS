//! `science-core-ends-expansion`: an assigned Science lane leaves the
//! Expansion half of its plan once it holds a core of
//! [`SCIENCE_CORE_CITIES`] cities, instead of waiting for the specialization
//! clock.
//!
//! With an assigned lane the grand strategy reads Expansion ("the first half
//! is reserved for expansion and defense") while the empire holds fewer
//! cities than it wants and a site remains, until `phase_specialization_active`
//! turns on. Even with `science-specializes-earlier` that is turn 84 of 250.
//! Expansion weighs a beaker at 1.2 against the Science lane's 4.2, names the
//! Commercial Hub and the Merchant, and takes the expansion policy lists.
//!
//! The Indefinite Benchmarking Tourney's reviewer named it in 11 of 40 games
//! on 2026-11-24 (`science-lane-parked-for-expansion-until-t85`):
//! - every game logged "best lane science at 100% progress" from t1 and read
//!   Expansion until t84-t88;
//! - science at t50 was 8.1 vs the rival's 32.9 (191902Z, Lv 7), 14.1 vs
//!   81.9 (221444Z, Lv 8), 11 vs 51.4 (224903Z, Lv 7), 15.5 vs 22.4
//!   (212350Z, Lv 4), with 0-2 Campuses;
//! - the Expansion half did not buy the cities either: 4 cities at t50 in
//!   most of them, and 212350Z held 4 from t44 to t77.
//!
//! Settling does not depend on the plan reading Expansion: the Settler
//! pipeline reads `plan.desired_cities` and the payback window, and every one
//! of those games kept founding cities after it switched to Science (212350Z:
//! t84, 91, 114, 123, 136). So once the core is held the plan follows its
//! lane, and the city target, the Settler arms and the clock are unchanged.
//! Off, nothing moves.

use super::*;

/// The cities a Science lane holds before its lane outranks Expansion: the
/// core whose Campuses the lane scales, and the count the reviewed games
/// reached at t35-t45 and then sat on.
pub(super) const SCIENCE_CORE_CITIES: usize = 4;

impl AdvancedAi {
    /// See the module: an assigned Science lane holding its core of cities.
    /// False with the gene off, for any other lane, or below the core.
    pub(super) fn science_core_ends_expansion_for(
        &self,
        target: Option<VictoryTarget>,
        cities: usize,
    ) -> bool {
        self.science_core_ends_expansion
            && target == Some(VictoryTarget::Science)
            && cities >= SCIENCE_CORE_CITIES
    }
}

#[cfg(test)]
mod tests;
