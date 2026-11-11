//! `first-pad-in-the-forge`: while no Spaceport of ours stands, is placed or
//! is queued, a city may start the empire's first one only if its Production
//! is at least [`FIRST_PAD_FORGE_SHARE`] of the best producer among the
//! cities that can place one. A placed district cannot move, and every
//! launch project then runs at that city's Production.
//!
//! **Why (live King, 11-01/02, 23 games with a first pad).**
//! - Under a Science plan every city prices the first pad at the 3,000-point
//!   rung (`one-launch-pad`, which narrows it to `launch_pad_site`, is not
//!   live), so the first idle city the generic sweep reaches takes it.
//!   civvis-20261102T091117Z placed it in Gori, "about 200 turns at 4.5 a
//!   turn", while Telavi (54 Production), Tbilisi (34) and Poti (32) could
//!   place one; civvis-20261101T150036Z in Panamá, "about 225 turns at 4.0".
//!   Four more started at 26-39 turns.
//! - Over the 23 games the first pad stood in a city under 80% of the best
//!   placing producer in 9 (Production share 0.09 to 0.75), and those pads
//!   took 24-38 turns to build against 11-17 for the rest.
//! - The idle-Great-Person activation path reserves "the fastest idle city"
//!   for a missing Spaceport, which is the city with an empty queue, not the
//!   producer: civvis-20261102T105550Z started it in Chicago at 18 Production
//!   against the best placing producer's 78 and finished it 36 turns later.
//! - `launch-pad-holds-its-queue` then holds the first pad wherever it was
//!   placed.
//!
//! Three places start the first pad, and the gene vetoes each: the strategic
//! scorer (`AdvancedAi::production_value`), the idle-Great-Person activation
//! path, and `BasicAi::pick_item`'s first-pad step, which hands the pad to
//! whichever city it is filling once Rocketry is known. Live
//! civvis-20261102T142440Z (gene armed) placed it through that step in
//! Liverpool at 48 Production while Birmingham stood at 65.
//!
//! Off, every call answers `true`.

use super::BasicAi;
use crate::game::{Game, Item};

/// The share of the best placing producer's Production a city needs to
/// start the empire's first Spaceport.
pub(crate) const FIRST_PAD_FORGE_SHARE: f64 = 0.8;

impl BasicAi {
    /// Whether `cid` holds a Spaceport, has one queued, or has one placed on
    /// its tiles.
    pub(crate) fn city_spaceport_committed(g: &Game, cid: u32) -> bool {
        let city = &g.cities[&cid];
        city.districts
            .keys()
            .any(|district| g.district_family(*district) == "spaceport")
            || city.queue.iter().any(|item| {
                matches!(item, Item::District { district, .. }
                    if g.district_family(*district) == "spaceport")
            })
            || city.owned_tiles.iter().any(|pos| {
                g.map
                    .get(*pos)
                    .and_then(|tile| tile.district_foundation.as_ref())
                    .is_some_and(|foundation| g.district_family(foundation.district) == "spaceport")
            })
    }

    /// Whether `cid` can place a Spaceport on one of its tiles.
    fn city_can_place_a_pad(g: &Game, pid: usize, cid: u32) -> bool {
        let pad = crate::name!("spaceport");
        g.district_sites(cid, pad)
            .into_iter()
            .any(|pos| g.can_produce(pid, cid, &Item::District { district: pad, pos }))
    }

    /// See the module: whether `cid` may start the empire's first Spaceport.
    /// `true` with the gene off, once any pad is committed, or when no city
    /// can place one.
    pub(crate) fn first_pad_city_allowed(&self, g: &Game, pid: usize, cid: u32) -> bool {
        // One memo for the whole sweep; see `Game::query_memo`.
        let _memo = g.query_memo();
        if !self.first_pad_in_the_forge || self.minor || self.barb {
            return true;
        }
        let cities = g.player_city_ids(pid);
        if cities
            .iter()
            .any(|city| Self::city_spaceport_committed(g, *city))
        {
            return true;
        }
        let best = cities
            .iter()
            .filter(|city| Self::city_can_place_a_pad(g, pid, **city))
            .map(|city| g.city_yields(*city).production)
            .fold(None, |best: Option<f64>, production| {
                Some(best.map_or(production, |seen| seen.max(production)))
            });
        let Some(best) = best else {
            return true;
        };
        g.city_yields(cid).production + 1e-9 >= FIRST_PAD_FORGE_SHARE * best
    }

    /// [`Self::first_pad_city_allowed`] for a queue item: anything but a
    /// Spaceport district is allowed.
    pub(crate) fn first_pad_item_allowed(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> bool {
        !matches!(item, Item::District { district, .. }
            if g.district_family(*district) == "spaceport")
            || self.first_pad_city_allowed(g, pid, cid)
    }

    pub(crate) fn enable_first_pad_in_the_forge(&mut self) {
        self.first_pad_in_the_forge = true;
    }

    pub(crate) fn disable_first_pad_in_the_forge(&mut self) {
        self.first_pad_in_the_forge = false;
    }
}

#[cfg(test)]
mod tests;
