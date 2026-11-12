//! `culture-seat-appoints-pingala-first`: a Culture target spends its first
//! Governor titles on Pingala and her Connoisseur promotion.
//!
//! ## What the live games say
//!
//! Connoisseur is +1 Culture per citizen in Pingala's city. On the live host
//! it reads as `+14.2 from Population` for an eleven-citizen city, which was
//! 20% of the whole empire's Culture at t100 in 20261110T171451Z.
//!
//! In 13 Deity Online Kristina games of 2026-11-10 the first title arrived at
//! t24-t35 and went to Magnus every time: the plan still reads Expansion that
//! early, and Expansion's roster starts with him. The rule that finishes the
//! earliest incumbent's two promotions then gave him the next two titles
//! too. Pingala was appointed at t59-t84, and Connoisseur came at t75-t117.
//! Science-first title order made it later still, because Pingala's list only
//! puts Connoisseur first while the plan reads Culture.
//!
//! Over 60 finished Pericles games of the census, Culture per turn at t100 had
//! a rank correlation of 0.83 with the final foreign-tourist share of the
//! victory bar, the strongest of the early readings.
//!
//! ## What the gene does
//!
//! While the seat's victory target is Culture:
//! - a plan that reads Expansion orders titles as Culture does, so Pingala is
//!   the first appointee, and
//! - Pingala's own promotions follow the Culture order under any plan, so
//!   Connoisseur comes before Researcher even during a Recovery.
//!
//! Her seat is unchanged: `best_governor_city` already prefers the
//! largest-population city. Other plans, including Recovery's Victor-first
//! roster when the seat is under attack, keep their appointment order. Off, or
//! for any other victory target, nothing changes.
//!
//! The cost is that Magnus arrives later, after Pingala's two tier-one
//! promotions. His Provision title spares a citizen per Settler, so the gene
//! is judged on cities at t60 as well as on Culture at t60 and t100.
//!
//! ## `culture-keeps-pingala-home`
//!
//! The base Loyalty pass (`reassign_governor_for_loyalty`) moves any seated
//! Governor from a city at 90+ Loyalty to the city that flips soonest. With
//! Pingala the only Governor, it pulled her out of the capital within 25
//! turns of her appointment in four of the first five live Kristina games
//! carrying `culture-seat-appoints-pingala-first` (195741Z t34, 201305Z t31
//! and t57, 201752Z t50, 202929Z t32), each time to a two-citizen border city
//! at 22-69 Loyalty; in the fifth, 193814Z, it took her from a 13-citizen
//! capital to a five-citizen city at 68 Loyalty at t100. Over all 20 Pingala moves of the
//! day, 9 went to a city three or more citizens smaller. With the gene on, a
//! Culture target's seated Pingala answers the emergency only when the city
//! would flip within `PINGALA_LOYALTY_RUNWAY_STANDARD` turns; any other
//! Governor, and an unassigned one, still goes as before. The runway rides on
//! `BasicAi::pingala_loyalty_runway`, set at the top of each turn, because
//! the Loyalty pass runs twice: from `AdvancedAi`'s governor block and from
//! the base ancillary pass after the titles.

use super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::Game;

/// `culture-keeps-pingala-home`: standard turns of Loyalty runway inside
/// which a seated Pingala still answers the emergency (about eight Online
/// turns). A Governor needs five turns to establish, so a longer runway can
/// wait for another Governor's title.
const PINGALA_LOYALTY_RUNWAY_STANDARD: u32 = 12;

impl AdvancedAi {
    fn culture_seat_titles_pingala_first(&self, g: &Game) -> bool {
        self.culture_seat_appoints_pingala_first
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
    }

    /// `culture-keeps-pingala-home`: the Governor the Loyalty pass spares and
    /// the runway inside which it still goes. `None` with the gene off or for
    /// another target.
    pub(super) fn culture_pingala_stays_home(&self, g: &Game) -> Option<(&'static str, f64)> {
        // `domination-seat-appoints-pingala-first` keeps its Pingala seated
        // the same way. See `advanced/domination_governors.rs`.
        ((self.culture_keeps_pingala_home
            && self.active_victory_target(g) == Some(VictoryTarget::Culture))
            || self.domination_seat_titles_pingala_first(g))
        .then(|| {
            (
                "pingala",
                g.standard_duration(PINGALA_LOYALTY_RUNWAY_STANDARD) as f64,
            )
        })
    }

    /// The plan that orders this frame's Governor titles.
    pub(super) fn governor_title_strategy(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
    ) -> GrandStrategy {
        if strategy == GrandStrategy::Expansion && self.culture_seat_titles_pingala_first(g) {
            GrandStrategy::Culture
        } else if self.domination_titles_follow_science(g, pid, strategy) {
            // `domination-seat-appoints-pingala-first`: see
            // `advanced/domination_governors.rs`.
            GrandStrategy::Science
        } else {
            strategy
        }
    }

    /// The plan that orders one governor's promotions: Pingala always takes
    /// the Culture order on a Culture seat.
    pub(super) fn pingala_title_strategy(
        &self,
        g: &Game,
        strategy: GrandStrategy,
        governor: &str,
    ) -> GrandStrategy {
        if governor == "pingala" && self.culture_seat_titles_pingala_first(g) {
            GrandStrategy::Culture
        } else if governor == "pingala"
            && strategy == GrandStrategy::Culture
            && self.domination_seat_titles_pingala_first(g)
        {
            // `domination-seat-appoints-pingala-first`: Researcher before
            // Connoisseur under any plan.
            GrandStrategy::Science
        } else {
            strategy
        }
    }
}

#[cfg(test)]
#[path = "culture_pingala_first_tests.rs"]
mod tests;
