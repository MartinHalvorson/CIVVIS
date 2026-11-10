//! `religion-seat-takes-the-debater`: on a Religion seat an Apostle offered
//! Debater takes it.
//!
//! **Why (343 live Apostles, 2026-11-09).** A live Apostle takes one promotion
//! from three the host draws, and Debater's +20 is the only one that moves a
//! theological fight. It more than covers the +4 difficulty and +3 or +6 Intel
//! that rival Apostles usually carry. The 47 Apostles offered Debater that took it averaged
//! 0.94 kills and 2.79 spreads each; the 27 that were offered it and took
//! something else averaged 0.07 kills and 1.22 spreads. The chooser picks by
//! situation, so those groups are not randomly assigned, but nothing in them
//! shows Debater costing spreads.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;
use crate::name::Name;

impl AdvancedAi {
    /// Whether this seat's Apostle should take Debater from this menu.
    pub(super) fn religion_seat_takes_debater(&self, g: &Game, menu: &[Name]) -> bool {
        self.religion_seat_takes_the_debater
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && menu.iter().any(|name| *name == crate::name!("debater"))
    }
}

#[cfg(test)]
mod tests;
