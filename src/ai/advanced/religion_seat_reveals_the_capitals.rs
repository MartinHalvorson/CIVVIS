//! `religion-seat-reveals-the-capitals`: a Religion-target founder uses the
//! existing Embassy reveal policy to locate a rival with no known city.
//!
//! Native Deity Russia `civvis-20261110T055711Z` met Sweden at t85 but first
//! saw one of its cities at t168. At t125 Russia knew Diplomatic Service,
//! held 179 Gold, and Sweden read Declared Friend with no Embassy of ours;
//! at t150 it read Friendly, still unlocated. Russia fielded 29 Apostles at
//! t150 and lost to the Aztecs' Diplomatic victory at t204. A spreader needs
//! a city to head for. The existing fast reveal serves Domination and
//! Science alone; the ordinary mission cadence never sent Sweden an Embassy.
//!
//! The new opt-in admits a founded Religion seat to that same policy, only
//! for an otherwise unlocated rival. Its existing Diplomatic Service,
//! standing, peace, mission, cooldown and treasury checks still apply. A
//! known city, including a remembered one, is enough to leave this purchase
//! to the ordinary cadence. No claim of native execution or victory benefit.

use super::{AdvancedAi, Game, VictoryTarget};

impl AdvancedAi {
    pub(super) fn religion_seat_reveals_the_capitals_active(&self, g: &Game, pid: usize) -> bool {
        self.religion_seat_reveals_the_capitals
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.players[pid].religion.is_some()
    }
}
