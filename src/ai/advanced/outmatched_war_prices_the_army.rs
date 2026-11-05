//! `outmatched-war-prices-the-army`: while a much stronger major is at war
//! with us and a home city is threatened, a land unit the army still lacks
//! outweighs the economy in every city's queue.
//!
//! **The finding (live Deity Korea science, 2026-11-05).** The strategic
//! scorer (`production_value`) divides each item's raw value by `7 + turns`.
//! A land unit's raw value is `power × 4 + 58 × force_gap`, plus a role term,
//! and the only large war term (`+210`) goes to the one city the plan names
//! as threatened. Every other city prices the war like peacetime.
//!
//! A replay of `civvis-20261105T033244Z` t168 with `--resumed`, run with
//! `war-defense-spends-the-runway` and `threatened-lane-lends-the-army` on,
//! reads: "Gyeongju starts stock exchange | worth 104 to the recovery plan".
//! At that point the Aztecs fielded 3377 power against our 577. An AT Crew's
//! raw value of about 570, divided by 7 + 4 turns, scores near 52, so the
//! capital kept building its economy. The seat was eliminated at t203, and played-out
//! Deity games have lost 2.9 cities each (29% eliminated).
//!
//! **The gene.** While a living major at war with us fields at least
//! [`OUTMATCHED_RATIO`] times our military power, a home city is threatened
//! (`home_city_threatened`), and the land army is short of its war target
//! (`force_gap > 0`), a land military unit's raw value gains
//! [`OUTMATCHED_UNIT_PREMIUM`] times that ratio, capped at
//! [`OUTMATCHED_RATIO_CAP`]: 525 to 1050. That lifts a typical defender from
//! about 570 to 1100-1600 raw, above the economic buildings the Recovery plan
//! otherwise picks. The army target is itself enemy-weighted and capped
//! (`enemy_weighted_army_target`), so the premium switches off once the gap
//! is filled. Naval and air units, peace, an unthreatened war and a war at
//! parity are unchanged. With the gene off, nothing changes.

use super::war_defense_spends_the_runway::OUTMATCHED_RATIO;
use super::*;

/// Raw value per unit of enemy-to-our power ratio that a missing land unit
/// gains in an outmatched, threatened war.
pub(super) const OUTMATCHED_UNIT_PREMIUM: f64 = 350.0;

/// The ratio stops adding premium here: past three to one, more of the same
/// unit is still the only answer, but the economy is not worth less.
pub(super) const OUTMATCHED_RATIO_CAP: f64 = 3.0;

impl AdvancedAi {
    /// The power ratio of the strongest living major at war with `pid` over
    /// ours, when it is at least [`OUTMATCHED_RATIO`]; `None` otherwise.
    pub(super) fn outmatched_war_ratio(&self, g: &Game, pid: usize) -> Option<f64> {
        let ours = g.military_power(pid).max(1.0);
        let strongest = g
            .players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && g.is_at_war(pid, rival.id)
            })
            .map(|rival| g.military_power(rival.id))
            .fold(0.0_f64, f64::max);
        let ratio = strongest / ours;
        (ratio >= OUTMATCHED_RATIO).then_some(ratio)
    }

    /// `outmatched-war-prices-the-army`: the raw value a land military unit
    /// gains this call. Zero with the gene off, for naval or air units, while
    /// the land army is at its war target, or unless an outmatched war
    /// threatens a home city.
    pub(super) fn outmatched_war_unit_premium(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        land: bool,
        force_gap: f64,
    ) -> f64 {
        if !self.outmatched_war_prices_the_army || !land || force_gap <= 0.0 {
            return 0.0;
        }
        let Some(ratio) = self.outmatched_war_ratio(g, pid) else {
            return 0.0;
        };
        if !Self::home_city_threatened(g, pid, plan) {
            return 0.0;
        }
        OUTMATCHED_UNIT_PREMIUM * ratio.min(OUTMATCHED_RATIO_CAP)
    }
}

#[cfg(test)]
mod tests;
