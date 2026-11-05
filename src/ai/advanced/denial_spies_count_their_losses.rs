//! `denial-spies-count-their-losses`: once our Spies keep dying, stop
//! training new ones until the losses age out, and keep that Production for
//! our own research.
//!
//! ## Why
//!
//! Live Lv 8 Deity Korea Online (science seat, 2026-11-04/05): every game past
//! turn 100 fed Spies to the rivals' counter-espionage. Distinct Spy units
//! from the first Spy to the end: 23 in `civvis-20261104T232613Z` (lifetimes
//! 1-16 turns, many one), 13 in `20261105T002152Z`, 17 in `20261104T222818Z`,
//! 15 in `20261104T213449Z`; each costs more than the last (112, then 150 to
//! 262 Production), about 5.4-9.7% of the empire's whole Production after the
//! first. The network still held 0-2 of 2-5 slots, because the stock
//! valuation prices a Spy at 1,500 plus 850 for a Science plan whenever a slot
//! is open (11 of the 19 Spy starts in 232613Z), and `science-denial-trains-
//! spies` claims a queue for the rest. No winner's launch visibly slowed:
//! Spain's projects landed at t168 and t178, India's third at t163 before its
//! win at t182, both science losses of the lane.
//!
//! ## What the gene does
//!
//! The bridge reads our Spy losses from the run's log
//! ([`AdvancedAi::observe_spy_losses`]). While [`SPY_LOSS_PAUSE_COUNT`] or
//! more fall within [`SPY_LOSS_WINDOW_STANDARD`] standard turns, no new Spy is
//! valued or claimed: the stock valuation scores it out, the denial claim
//! stands down, and the base production fallback (`BasicAi::pick_item`,
//! where the live replays' Spies came from) offers none. Spies already afield keep their missions; training resumes
//! once the losses age out of the window. `Kind::OptIn`; off, nothing changes.
//!
//! ## Version two
//!
//! `denial-spies-count-their-losses-2` counts only the Spies lost within
//! three turns of running Disrupt Rocketry (the bridge remembers each Spy's
//! last `spy_operation` from the agent state records). Session -71's
//! census of the Deity games since 11-04 21Z: of Spies still present 6-8
//! turns after their order, Disrupt kept 7 of 48 (85% lost), Listening Post
//! 31 of 40 and Gain Sources 28 of 37, while Steal Tech Boost (`spies-steal-
//! the-lead`) trades a ~25% loss for 250-431 Science a Eureka. A stealing
//! network's losses are a good trade and no longer pause training.

use super::*;

/// Losses inside this many standard turns count.
pub(crate) const SPY_LOSS_WINDOW_STANDARD: u32 = 30;
/// This many recent losses pause Spy training.
pub(crate) const SPY_LOSS_PAUSE_COUNT: usize = 3;

impl AdvancedAi {
    /// Whether `denial-spies-count-their-losses` is on.
    pub fn denial_spies_count_their_losses_enabled(&self) -> bool {
        self.denial_spies_count_their_losses
    }

    /// Whether `denial-spies-count-their-losses-2` is on: the bridge then
    /// passes only the losses of Spies last seen running Disrupt Rocketry.
    pub fn denial_spies_count_their_losses_2_enabled(&self) -> bool {
        self.denial_spies_count_their_losses_2
    }

    /// The live bridge: the turns our Spies were lost on, as the host
    /// reported them. Replaces what was there.
    pub fn observe_spy_losses(&mut self, turns: Vec<u32>) {
        self.spy_loss_turns = turns;
    }

    /// How many of our Spies were lost inside the window ending this turn.
    pub(crate) fn recent_spy_losses(&self, g: &Game) -> usize {
        let window = g.standard_duration(SPY_LOSS_WINDOW_STANDARD);
        self.spy_loss_turns
            .iter()
            .filter(|lost| **lost <= g.turn && g.turn - **lost < window)
            .count()
    }

    /// Whether Spy training stands down this turn. Always false with the
    /// gene off.
    pub(crate) fn spy_losses_pause_training(&self, g: &Game) -> bool {
        (self.denial_spies_count_their_losses || self.denial_spies_count_their_losses_2)
            && self.recent_spy_losses(g) >= SPY_LOSS_PAUSE_COUNT
    }
}

#[cfg(test)]
mod tests;
