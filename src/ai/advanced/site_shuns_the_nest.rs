//! `site-shuns-the-nest`: a settle site near a known barbarian camp is priced
//! down by the camp's distance, so the Settler takes a safer site unless the
//! nest's is much better.
//!
//! The site value's safety term (`settlement_safety_penalty`) prices only the
//! hostile units visible when the site is chosen, plus isolation and border
//! pressure. A camp is a standing spawner: no raider need be in sight on the
//! turn the Settler is sent, and one arrives while it walks.
//! - Live Immortal 6p Continents (10-30 to 11-02): 42 settler captures, 26
//!   of them `site-in-barbarian-nest` (a hostile seen within 3 of the site in
//!   the 5 turns before, or a camp within 3); 18 by t100, when a city is
//!   worth most. The seat held a median 4 cities at t60.
//! - Of the 26 dossiers of 11-01/02 that name the site, 9 had a camp within
//!   3 tiles of it (8 of them with a hostile seen there too).
//! - civvis-20261101T053401Z t27: the Settler walked to (66,19) with a camp
//!   within 3 and a Warrior, a Spearman and an Archer around it, guarded by a
//!   24-HP Warrior, and was taken.
//!
//! The penalty is the largest over the known camps (`Game::barb_camps`, the
//! camps the seat has seen on the live mirror): [`NEST_PENALTY_ADJACENT`]
//! within 2 tiles, [`NEST_PENALTY_NEAR`] at 3, [`NEST_PENALTY_EDGE`] at 4.
//! Site values run about 100-200, so a nest site still wins when it is far
//! better than every alternative. Off, it is zero.

use super::*;

/// A site within 2 tiles of a known camp.
pub(crate) const NEST_PENALTY_ADJACENT: f64 = 80.0;
/// A site 3 tiles from a known camp.
pub(crate) const NEST_PENALTY_NEAR: f64 = 50.0;
/// A site 4 tiles from a known camp.
pub(crate) const NEST_PENALTY_EDGE: f64 = 25.0;

impl AdvancedAi {
    /// See the module: the price a known camp near `pos` takes off the site.
    /// Zero with the gene off, for a minor or barbarian seat, or with no known
    /// camp within 4 tiles.
    pub(super) fn nest_penalty(&self, g: &Game, pos: Pos) -> f64 {
        if !self.site_shuns_the_nest || self.base.minor || self.base.barb {
            return 0.0;
        }
        g.barb_camps
            .keys()
            .map(|camp| match g.wdist(*camp, pos) {
                distance if distance <= 2 => NEST_PENALTY_ADJACENT,
                3 => NEST_PENALTY_NEAR,
                4 => NEST_PENALTY_EDGE,
                _ => 0.0,
            })
            .fold(0.0, f64::max)
    }
}

#[cfg(test)]
mod tests;
