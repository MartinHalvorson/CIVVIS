//! `culture-core-ends-expansion`: an assigned Culture lane leaves the
//! Expansion half of its plan once it holds a core of
//! [`CULTURE_CORE_CITIES`] cities, instead of waiting for the Culture
//! buildup clock.
//!
//! With an assigned lane the grand strategy reads Expansion ("the first half
//! is reserved for expansion and defense") while the empire holds fewer
//! cities than it wants and a site remains, until `phase_specialization_active`
//! turns on. For Culture that is the first third of the clock, turn 84 of
//! 250. Expansion weighs culture at 0.8 against the Culture plan's 2.8, names
//! the Commercial Hub and the Merchant instead of the Theater Square and the
//! Artist, and takes the expansion policy lists.
//!
//! The Indefinite Benchmarking Tourney's reviewer named it in 7 of 40 games
//! on 2026-11-28 (`culture-lane-parked-for-expansion-until-t86`):
//! - every game logged "best lane culture at 100% progress" from t1 and read
//!   Expansion until t84-t88;
//! - at t100 they had 0 tourists and 0.0% against the rival's 4.35-26.13%,
//!   and culture per turn was 22-39 at t75;
//! - the Expansion half did not buy the cities either: they held 4 cities at
//!   t34-t62 (042707Z, 051020Z and 075840Z at t34-t41, 111318Z at t62).
//!
//! Settling does not depend on the plan reading Expansion: the Settler
//! pipeline reads `plan.desired_cities` and the payback window, and those
//! games kept founding cities after the switch to Culture (042707Z went from
//! 9 to 12 by t120, 111318Z from 5 to 10 by t186). So once the core is held
//! the plan follows its lane, and the city target, the Settler arms and the
//! clock are unchanged. Off, nothing moves.

use super::*;

/// The cities a Culture lane holds before its lane outranks Expansion: the
/// core whose Theater Squares carry the early Great Works, and the count the
/// reviewed games reached at t34-t62 and then sat on in Expansion.
pub(super) const CULTURE_CORE_CITIES: usize = 4;

impl AdvancedAi {
    /// See the module: an assigned Culture lane holding its core of cities.
    /// False with the gene off, for any other lane, or below the core.
    pub(super) fn culture_core_ends_expansion_for(
        &self,
        target: Option<VictoryTarget>,
        cities: usize,
    ) -> bool {
        self.culture_core_ends_expansion
            && target == Some(VictoryTarget::Culture)
            && cities >= CULTURE_CORE_CITIES
    }
}

#[cfg(test)]
mod tests;
