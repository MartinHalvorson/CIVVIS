//! `fresh-water-prices-the-site`: a fresh-water seat is worth more than the
//! site score says.
//!
//! The score pays `(housing - 2) * 4`: fresh water +12 and coast +4. With the
//! armed `coastal-city-sites` base of +6, a coast-only seat collects +10, so
//! fresh water leads it by 2 on seats worth 100-150. The game does not agree.
//!
//! Live Deity games, 11-05, non-capital cities at least 30 turns old at t100:
//!
//! | water | housing | n | pop | within one of housing |
//! |---|---:|---:|---:|---:|
//! | dry | 2 | 23 | 5.7 | 91% |
//! | coast | 3 | 67 | 6.1 | 66% |
//! | fresh | 5 | 64 | 6.8 | 25% |
//!
//! An Aqueduct is not buildable in 55-67% of capped cities at any stage, so a
//! capped seat tends to stay capped. Of 239 dry or coast-only cities founded
//! across 79 live games, 67 had a legal fresh-water plot within two tiles at
//! founding (42 within one): the seat was a choice, not the map's.
//!
//! The gene adds [`FRESH_WATER_SITE_BONUS`] to a fresh-water seat, doubling
//! its housing term, in both the site score and the shortlist prefilter. It
//! decides close calls: a richer dry seat still wins. Off, both read exactly
//! what they read before.

use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;

/// What a fresh-water seat gains: as much again as its `(5 - 2) * 4` housing
/// term. The lead over a coast-only seat goes from 2 to 14.
pub(super) const FRESH_WATER_SITE_BONUS: f64 = 12.0;

impl AdvancedAi {
    /// The gene's credit for founding at `pos`. Zero with the gene off and
    /// on a seat without fresh water.
    pub(super) fn fresh_water_site_bonus(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        if self.fresh_water_prices_the_site && g.city_site_water(pid, pos).0 {
            FRESH_WATER_SITE_BONUS
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests;
