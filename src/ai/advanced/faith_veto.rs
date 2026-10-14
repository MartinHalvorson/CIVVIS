//! `found-against-a-rival-faith`: a Domination seat with no religion enters
//! the Prophet race once a rival's faith reaches the religious early-warning
//! bar, while a religion slot is still open.
//!
//! A city that follows no religion resists none. Live King games on
//! 2026-10-04 lost to Religion three times running. In game 71 our ten cities
//! went from two Catholic to five between turns 105 and 144, and Hungary won.
//! No hostile missionary or apostle stood within six tiles of a city of ours
//! at any frame: passive pressure from neighbouring Catholic cities did it,
//! which no condemnation can stop. In game 70 only one of three religions
//! was founded through turn 195, so two slots stayed open while we held 195
//! Faith. A home religion gives every city we own something to resist with,
//! and our cities cannot then be a rival faith's last holdout.
//!
//! The race itself is `enter-the-prophet-race-2`'s package: Astrology, the
//! first Holy Site, Prophet patronage, and `pursue_religion`. This gene opens
//! the same admission gate (`prophet_race_enterable_for`) under its own
//! condition, so the slot, viability and last-call checks all still apply.

use super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::Game;

impl AdvancedAi {
    /// Whether a rival's founded faith stands at the religious early-warning
    /// bar while this Domination seat holds no religion (the same bar
    /// `domination_faithless_conversion_counter` reads).
    pub(super) fn faith_veto_due(&self, g: &Game, pid: usize) -> bool {
        if !self.found_against_a_rival_faith
            || g.players[pid].religion.is_some()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !Self::victory_strategy_enabled(g, GrandStrategy::Religion)
        {
            return false;
        }
        let living = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
            .count() as i32;
        if living <= 2 {
            return false;
        }
        let match_point = 100 * (living - 1) / living;
        let early_warning = (100 * (living - 2) / living).max(50).min(match_point);
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && rival.religion.is_some()
            })
            .any(|rival| self.lane_progress_table(g, rival.id)[2] >= early_warning)
    }
}
