//! `religion-seat-spends-the-bank`: a Religion-target founder on the offensive
//! keeps buying spreaders past its corps caps while the bank is deep.
//!
//! **Why (live Deity Russia, 2026-11-10).** The corps is capped at six
//! Apostles (`religion-seat-fields-apostles`) and six Missionaries
//! (`religious_spending_with_reserve`), sized for a seat that only defends its
//! own cities. Run civvis-20261110T012017Z, the first to hold four of five
//! majors (t167), fielded 13-14 religious units from t100 on and banked the
//! rest: 527 Faith at t100, 1,563 at t120, 2,495 at t150, 3,077 at t170 and
//! 3,343 at t180, at 150-250 Faith a turn with an Apostle near 450. It lost to
//! a rival's Science at t185 with the fifth major at 1 of 12 cities and two of
//! the four it had held slipping back. 005727Z ended with 1,775 banked and
//! 001935Z with 1,312. Faith a Religion seat has not spent when the game ends
//! bought nothing.
//!
//! **What the gene does.** For a seat aiming at Religion that has founded and
//! is on the offensive, while the bank holds at least
//! [`DEEP_BANK_APOSTLES`] Apostle prices
//! ([`AdvancedAi::religion_seat_bank_is_deep`]):
//! - the Apostle and Missionary caps do not stop a purchase;
//! - the Apostle is asked for first, then the Missionary, then the Guru.
//!
//! The pass still buys one unit a turn, still buys only in a city following
//! our faith, and the Inquisition's corps still comes first when a city of
//! ours is under pressure. Below the threshold, off, or for a seat aiming
//! elsewhere, nothing changes.

use super::*;

/// The bank, in Apostle prices, at which the corps caps stop binding. Two: one
/// is what `religion-seat-fields-apostles` saves toward, so a second is Faith
/// the capped corps has no use for.
pub(super) const DEEP_BANK_APOSTLES: f64 = 2.0;

impl AdvancedAi {
    /// Whether the gene lifts the corps caps this turn. See the module.
    pub(super) fn religion_seat_bank_is_deep(
        &self,
        g: &Game,
        pid: usize,
        offensive: bool,
        religion: &str,
    ) -> bool {
        self.religion_seat_spends_the_bank
            && offensive
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.players[pid].religion.as_deref() == Some(religion)
            && self
                .religion_seat_apostle_price(g, pid, religion)
                .is_some_and(|price| g.players[pid].faith >= DEEP_BANK_APOSTLES * price)
    }
}

#[cfg(test)]
mod tests;
