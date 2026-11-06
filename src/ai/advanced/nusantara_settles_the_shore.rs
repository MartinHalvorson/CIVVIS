//! `nusantara-settles-the-shore`: an Indonesian seat's site score prices the
//! Coast.
//!
//! Gitarja's City Center pays +2 Faith next to Coast or Lake, and Great
//! Nusantara pays Campuses, Holy Sites, Industrial Zones and Theater Squares
//! +1 per two adjacent Coast or Lake tiles. `data/civs.json` carries only a
//! placeholder for both, so sites were ranked as if Indonesia were landlocked.
//! `nusantara-reads-the-coast` re-ranks district tiles inside a city, and it
//! cannot help a city founded inland.
//!
//! Live Deity Indonesia, 17 games, 97 foundings from t20: 54 on the shore and
//! 43 inland. 27 of the inland cities had a legal shore plot within two tiles
//! (18 within one). Measured over 101 cities (-79): shore cities reach 3.5
//! Nusantara points in their three best district plots against 2.5 inland, so
//! the edge is the +2 Faith, about +1 yield, and Harbor and ship access.
//!
//! The gene adds [`NUSANTARA_SHORE_SITE_BONUS`] to a seat beside Coast or
//! Lake: a tiebreak for a one or two tile shift, not enough to give up a
//! clearly better inland seat.
//!
//! ⚠ **It does not pull a city off fresh water.** Live non-capital cities on
//! fresh water held 6.8 citizens at t100 and 25% stood near their housing cap,
//! against 6.1 and 66% on the coast (-04, selection-confounded). So a
//! coast-only seat earns the bonus only with no fresh-water plot within
//! [`NUSANTARA_FRESH_WATER_TILES`]; a shore seat with fresh water always earns
//! it, and a Lake gives both. 25 of the 27 cases above survive this rule.
//!
//! The headless game does not model Great Nusantara, so a paired sim read can
//! show harm but not the benefit. Off, and for any other civilization, nothing
//! here runs.

use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;

/// What a shore seat gains: above fresh water's +12 housing term and well
/// under the 100-150 a good seat is worth.
pub(super) const NUSANTARA_SHORE_SITE_BONUS: f64 = 16.0;

/// A fresh-water plot this near stops a coast-only seat earning the bonus.
pub(super) const NUSANTARA_FRESH_WATER_TILES: i32 = 2;

impl AdvancedAi {
    /// The gene's credit for founding at `pos`. Zero with the gene off, for
    /// any civilization but Indonesia, away from Coast and Lake, and on a
    /// coast-only seat with fresh water within reach.
    pub(super) fn nusantara_shore_site_bonus(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        if !self.nusantara_settles_the_shore
            || g.players
                .get(pid)
                .is_none_or(|seat| seat.civ != "Indonesia")
            || !Self::has_harbor_coast(g, pos)
        {
            return 0.0;
        }
        if g.city_site_water(pid, pos).0 {
            return NUSANTARA_SHORE_SITE_BONUS;
        }
        let fresh_near = g
            .wdisk(pos, NUSANTARA_FRESH_WATER_TILES)
            .into_iter()
            .any(|at| {
                g.map
                    .get(at)
                    .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
                    && g.city_site_water(pid, at).0
            });
        if fresh_near {
            0.0
        } else {
            NUSANTARA_SHORE_SITE_BONUS
        }
    }
}

#[cfg(test)]
mod tests;
