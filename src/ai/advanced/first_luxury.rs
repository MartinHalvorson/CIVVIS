//! `first-luxury-first`: the Builder improves the empire's FIRST copy of a
//! luxury ahead of an ordinary tile, and prices that first copy by the
//! Amenities the empire is actually short.
//!
//! **The finding (11 King games of 2026-08-29, 7,071 city-turns).** 46 % of
//! city-turns are Amenity-short — Displeased, and `amenity_yield_mult_for`
//! multiplies EVERY yield the city makes by 0.9 for it — and the deficit is
//! shallow: 2,429 of the 3,258 short city-turns are short by exactly one.
//! After setting aside the turns the shortfall is a treasury bankruptcy
//! penalty, **93.4 % of the deficit could have been covered by improving a
//! luxury the empire already OWNED and never improved**. 47 % of owned
//! luxury tiles were never improved at all, and the ones that were waited a
//! median 21 turns for the charge. The Builder orders of those games say the
//! same thing from the other side: FARM 147, MINE 117, LUMBER_MILL 75,
//! ROMAN_FORT 35, PASTURE 24, QUARRY 23, ALCAZAR 21, CAMP 17, PLANTATION 15.
//! The empire built ten farms for every plantation while half its cities
//! took a 10 % yield cut for want of one Amenity.
//!
//! **The mechanism.** `AdvancedAi::improvement_value_with_appeal` pays a
//! **flat +14.0** for any improvement that works a luxury:
//!
//! ```ignore
//! "luxury" => 14.0 * worked as i32 as f64,
//! ```
//!
//! That number is blind to both halves of what a luxury is actually worth.
//! It is blind to whether the empire already holds a copy — a fifth Wine
//! ties the first, though only the first buys Amenities
//! (`Game::luxury_amenity_allocations` supplies each DISTINCT luxury to four
//! cities and a duplicate copy supplies nothing unless a Congress
//! `luxury_policy` is live) — and it is blind to whether anybody is
//! Displeased.
//!
//! It is a CONSTANT, and that is why the tile waits. The Builder ranks every
//! owned tile of the empire by `improvement_value` less `0.7` a tile of
//! distance and takes the head; a first Plantation on Silk prices 15.8 under
//! Expansion, against 32.2 for a Mine on an Iron hill (the strategic premium
//! is 30) and more for anything with real yields, so the luxury sits behind a
//! queue that refills as fast as the empire grows. Nothing about waiting ever
//! makes the tile score higher, and nothing about three Displeased cities
//! does either — which is exactly how 47 % of them are still unimproved at
//! the end of the game.
//!
//! **The gene.** Under `first_luxury_first`, an improvement that works a
//! luxury the empire holds NO improved copy of earns
//! [`FIRST_COPY_BASE`] + [`FIRST_COPY_PER_AMENITY`] × the empire's Amenity
//! deficit, capped at [`FIRST_COPY_DEFICIT_CAP`] Amenities — 26 with every
//! city content and 106 at the cap, on top of the shipped +14. At the three
//! Amenities the studied seat was typically short that is 86, which carries
//! the Plantation's 15.8 past the Iron Mine's 32.2 and past most of the
//! distance term; content, the 26 clears ordinary ground and nothing more. A
//! duplicate copy keeps the flat +14 exactly: the gene never says "improve
//! more luxuries", it says "improve the ones nobody has opened yet, first".
//!
//! "Holds a copy" is [`Game::resource_access_count`], which is the engine's
//! own answer: connected copies of our own and our suzerains', plus the
//! balance of live trades, plus an Amani's share. So a Wine that arrived in
//! a deal, or from a city-state, counts as held and the tile drops back to
//! +14 — and an owned but UNIMPROVED tile does not count, which is the whole
//! premise (`connected_resource_census` requires
//! `Game::tile_connects_resource`).
//!
//! **Cost.** The Amenity deficit walks every city and reads
//! `Game::city_amenities`, whose luxury allocation is an empire-wide
//! decision; it is computed once a seat-turn and cached in
//! [`AmenityDeficitFrame`], the same shape `yield_floors.rs` uses. The
//! held-copy question is asked live per candidate rather than cached, so a
//! Builder that opens the first Wine this turn does not leave a second
//! Builder still believing the empire has none.
//!
//! Off by default, and byte-identical while off: [`FIRST_COPY_BASE`] is only
//! ever reached through `if self.first_luxury_first`.

use super::AdvancedAi;
use crate::game::Game;
use crate::name::Name;
use crate::reasoning::plain;
use crate::think;
use crate::Pos;

/// What the empire's first copy of a luxury is worth with every city
/// content. The flat luxury premium it sits beside is 14.0 and a Farm on
/// plain grassland prices 2.0 under Expansion, so 26 puts an unopened luxury
/// clear of ordinary ground — and just clear of the +30 a strategic deposit
/// earns — without reordering anything a duplicate copy competes with.
pub(crate) const FIRST_COPY_BASE: f64 = 26.0;
/// Per Amenity the empire is short. One Amenity is a satisfaction band and a
/// band is a multiplier on every yield a city makes (`-1` → 0.9), so the
/// slope is deliberately steeper than the base: at the measured King deficit
/// of one to four short Amenities this pays 46 to 106 on top of the shipped
/// +14, which is what it takes to clear an unopened Iron deposit's 32.2 and
/// the distance to a tile a few rings out.
pub(crate) const FIRST_COPY_PER_AMENITY: f64 = 20.0;
/// The deficit is capped here. A single luxury supplies at most four cities
/// (`Game::luxury_amenity_allocations`; six for the Aztecs), so an empire
/// eight Amenities short is not twice as well served by this one plantation
/// as one that is four short — and an uncapped deficit would let a
/// Revolt-band empire pay hundreds for a tile that cannot fix it.
pub(crate) const FIRST_COPY_DEFICIT_CAP: f64 = 4.0;

/// The empire's Amenity deficit for one seat-turn.
///
/// `improvement_value_with_appeal` is called once per candidate improvement
/// per candidate tile, and a Builder sweep scores every tile the empire
/// owns; the deficit walks every city and reads the empire-wide luxury
/// allocation, so it is computed once and read many times. A city founded or
/// lost within the turn re-reads it.
#[derive(Clone, Default)]
pub(crate) struct AmenityDeficitFrame {
    turn: Option<u32>,
    pid: usize,
    cities: usize,
    deficit: f64,
}

/// The luxury this tile carries, if it carries one. Every question this gene
/// asks starts here, so the "is it a luxury" rule is written once.
fn luxury_at(g: &Game, pos: Pos) -> Option<Name> {
    let resource = g.map.get(pos)?.resource?;
    let spec = g.rules.resources.get(resource.as_str())?;
    (spec.class == "luxury").then_some(resource)
}

/// Whether this improvement is one of the ones that WORKS the resource — the
/// same test `improvement_value_with_appeal` applies before paying its flat
/// premium. A Farm on a Silk forest connects nothing.
fn works_resource(g: &Game, improvement: &str, resource: Name) -> bool {
    g.rules
        .improvements
        .get(improvement)
        .is_some_and(|spec| spec.resources.iter().any(|entry| entry == &resource))
}

impl AdvancedAi {
    /// Σ over the seat's cities of `max(0, required − supplied)`.
    ///
    /// Deliberately `Game::city_amenities_required` against
    /// `Game::city_amenities` rather than `Game::city_amenity_surplus`: the
    /// surplus also subtracts `bankruptcy_amenity_penalty`, and a treasury
    /// bankruptcy is not a shortfall a plantation can repair. The census this
    /// gene is built on set those turns aside for the same reason.
    pub(super) fn empire_amenity_deficit(&self, g: &Game, pid: usize) -> f64 {
        if self.base.minor || self.base.barb {
            return 0.0;
        }
        let cities = g.player_city_ids(pid);
        {
            let frame = self.first_luxury_frame.borrow();
            if frame.turn == Some(g.turn) && frame.pid == pid && frame.cities == cities.len() {
                return frame.deficit;
            }
        }
        let deficit = cities
            .iter()
            .filter_map(|cid| g.cities.get(cid))
            .map(|city| {
                (Game::city_amenities_required(city) - g.city_amenities(city)).max(0) as f64
            })
            .sum();
        *self.first_luxury_frame.borrow_mut() = AmenityDeficitFrame {
            turn: Some(g.turn),
            pid,
            cities: cities.len(),
            deficit,
        };
        deficit
    }

    /// The extra this improvement on this tile is worth because it opens a
    /// luxury the empire has no copy of. Zero while the gene is off, zero for
    /// an improvement that does not work the tile's resource, and zero for a
    /// resource the empire already holds — which keeps the shipped flat +14
    /// as the whole answer for a duplicate copy.
    pub(super) fn first_luxury_premium(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        improvement: &str,
    ) -> f64 {
        if !self.first_luxury_first {
            return 0.0;
        }
        let first_copy = luxury_at(g, pos).is_some_and(|resource| {
            works_resource(g, improvement, resource)
                // The engine's own account of access: connected copies of ours
                // and our suzerains', the balance of live trades, an Amani's
                // share. An owned but UNIMPROVED tile is not one of them.
                && g.resource_access_count(pid, resource.as_str()) == 0
        });
        if !first_copy {
            return 0.0;
        }
        let deficit = self
            .empire_amenity_deficit(g, pid)
            .min(FIRST_COPY_DEFICIT_CAP);
        FIRST_COPY_BASE + FIRST_COPY_PER_AMENITY * deficit
    }

    /// `improvement_value` plus the first-copy premium. With the gene off the
    /// premium is zero and this is `improvement_value` exactly, which is why
    /// the Builder's ranking sites can all read through here.
    pub(super) fn improvement_value_for(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        improvement: &str,
        strategy: super::GrandStrategy,
    ) -> f64 {
        self.improvement_value(g, pos, improvement, strategy)
            + self.first_luxury_premium(g, pid, pos, improvement)
    }

    /// Journal an improvement the premium decided, once, after the engine has
    /// accepted the order. Nothing while the gene is off or the tile was not
    /// the empire's first copy.
    pub(super) fn note_first_luxury_opened(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        improvement: &str,
    ) {
        if !self.first_luxury_first {
            return;
        }
        // Asked AFTER the order landed, so the tile now connects and
        // `first_luxury_premium` would answer zero. The premium the decision
        // was taken on is the one the deficit still describes.
        let Some(resource) = luxury_at(g, pos).filter(|resource| {
            works_resource(g, improvement, *resource)
                // One: the copy this order just connected, and no other.
                && g.resource_access_count(pid, resource.as_str()) <= 1
        }) else {
            return;
        };
        let deficit = self.empire_amenity_deficit(g, pid);
        think!(self.journal(), Cities, Decision,
               "A {} improves the empire's first {}",
               plain(improvement), plain(resource.as_str());
               "amenity deficit {deficit:.0} across the empire, and a duplicate copy \
                of a luxury buys no Amenities"; pos);
    }
}
