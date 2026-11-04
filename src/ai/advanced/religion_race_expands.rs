//! `religion-race-keeps-expanding`: a Religion-target seat racing for its
//! Prophet keeps the Expansion grand strategy while it is short of its city
//! target and has somewhere to settle, as every other lane does.
//!
//! **Why (10-31).** `assess` hands a Religion-target seat without a religion
//! the Religion strategy ("the religion lane still needs a religion") before
//! the Expansion branch ("the first half is reserved for expansion and
//! defense") can read the city count. Every other lane takes Expansion until
//! it has its cities. On the 6p Continents Immortal lane since 03:45Z, the
//! Religion seats wanted as many cities at t20-40 as the rest (median 6.5)
//! but 4 of 5 ran Religion there, while every Culture, Domination and
//! Science seat ran Expansion. They held a median 3 cities at t60 (others 4-5),
//! and 4 of 5 were retired by the production-share rule. The prophet race
//! does not need the strategy: `prophet-builds-its-site` and
//! `prophet-race-earns-its-points` key on the victory target, and seats
//! that did found were still retired at t82 with 2-3 cities.
//!
//! With the gene on, such a seat falls through to the Expansion branch while
//! that branch's own condition holds (no specialization, fewer cities than
//! wanted, a site in reach). Once it has its cities, or no site, it races as
//! before; once it founds, it follows the lane. If the race is lost,
//! `religion-lane-hands-over` picks another lane. Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether a Religion seat still racing for its Prophet keeps expanding
    /// first: the gene is on and `room_to_expand` (the Expansion branch's
    /// condition) holds. `false` with the gene off.
    pub(super) fn religion_race_expands_first(&self, room_to_expand: bool) -> bool {
        self.religion_race_keeps_expanding && room_to_expand
    }
}

#[cfg(test)]
mod tests;
