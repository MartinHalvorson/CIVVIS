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

use super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::Game;

impl AdvancedAi {
    fn culture_seat_titles_pingala_first(&self, g: &Game) -> bool {
        self.culture_seat_appoints_pingala_first
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
    }

    /// The plan that orders this frame's Governor titles.
    pub(super) fn governor_title_strategy(
        &self,
        g: &Game,
        strategy: GrandStrategy,
    ) -> GrandStrategy {
        if strategy == GrandStrategy::Expansion && self.culture_seat_titles_pingala_first(g) {
            GrandStrategy::Culture
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
        } else {
            strategy
        }
    }
}

#[cfg(test)]
#[path = "culture_pingala_first_tests.rs"]
mod tests;
