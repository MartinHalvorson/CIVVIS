//! `culture-recovery-spends-the-bank`: a named Culture seat in Recovery
//! still buys its Rock Bands and Naturalists once the Faith bank holds twice
//! the cheapest of them.
//!
//! `culture_lane_spends` keeps Recovery's Faith in reserve unless every
//! Faith-purchasable unit on every city's menu is a Naturalist or a Rock
//! Band (`recovery_faith_has_only_culture_units`). Any Missionary, Apostle or
//! Faith-bought soldier on any menu fails that test, so a Culture seat in
//! Recovery stops buying the only units that turn Faith into Tourism, and the
//! bank grows. Live Immortal culture seats of 2026-10-30 logged 38-134
//! "recovery plan" journal lines a game; on 22 turns of civvis-20261030T132518Z
//! a Band (550) or a Naturalist (350) was for sale with at least twice its
//! price banked, the bank stood at 1,840 Faith at turn 200 and 4,384 at 210,
//! and none was bought (153459Z: 12 such turns, peak 1,753; 165127Z: 13,
//! peak 1,050; 141223Z: 6, peak 949).
//!
//! With the gene on, Recovery's reserve gives way for a named Culture seat
//! whose bank holds at least [`CULTURE_RECOVERY_BANK_MULTIPLE`] times the
//! cheapest Rock Band or Naturalist any of its cities sells. The purchase
//! itself is unchanged (`culture_spending`: one a turn, its own band cap).
use super::AdvancedAi;
use crate::game::Game;

/// The bank, as a multiple of the cheapest Band or Naturalist on sale, past
/// which Recovery's reserve gives way.
pub(crate) const CULTURE_RECOVERY_BANK_MULTIPLE: f64 = 2.0;

/// The units `culture_spending` buys with Faith.
const CULTURE_FAITH_UNITS: [&str; 2] = ["rock_band", "naturalist"];

impl AdvancedAi {
    /// Whether the Faith bank overflows Recovery's reserve: the gene is on and
    /// the bank holds [`CULTURE_RECOVERY_BANK_MULTIPLE`] times the cheapest
    /// Rock Band or Naturalist a city of ours sells. `false` when none is for
    /// sale.
    pub(super) fn culture_recovery_bank_overflows(&self, g: &Game, pid: usize) -> bool {
        if !self.culture_recovery_spends_the_bank {
            return false;
        }
        let cheapest = g
            .player_city_ids(pid)
            .into_iter()
            .flat_map(|cid| {
                CULTURE_FAITH_UNITS
                    .iter()
                    .filter_map(move |unit| g.unit_purchase_cost(pid, cid, unit, "faith"))
            })
            .fold(f64::INFINITY, f64::min);
        cheapest.is_finite()
            && g.players[pid].faith + f64::EPSILON >= CULTURE_RECOVERY_BANK_MULTIPLE * cheapest
    }
}

#[cfg(test)]
mod tests;
