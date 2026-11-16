//! `spared-soldiers-come-to-the-capital`: at our original capital, the
//! Anvil's one-blow rewrite (`anvil-spares-the-one-blow`) never leaves a
//! posted soldier standing more than [`CAPITAL_STAND_OFF`] tiles out. That
//! rewrite trades a post where one enemy blow at the top of its roll removes
//! the soldier for the least dangerous open tile within two of the city, or
//! for the tile the soldier stands on. On a tie it keeps the soldier's own
//! tile, and a tile far from the enemy reads no danger, so a soldier six
//! tiles out stays six tiles out. Here, a soldier standing farther out than
//! [`CAPITAL_STAND_OFF`] is not offered its own tile: it takes the least
//! dangerous open tile within two of the capital, or keeps its ring post
//! when no such tile is safer than the post.
//!
//! **Why (live 6p Continents Online, 11-15 00Z to 11-16 04Z: 27 games lost
//! the original capital).** Read from events.jsonl and why.log
//! (`~/civvis-score-tools/capdef-20261116/`):
//! - In 21 of the 27, the rewrite at the capital, during the war that took
//!   it, gave a posted soldier a tile four or more out: 261 such rewrites
//!   against 128 within three. Up to eight a turn
//!   (civvis-20261115T095713Z, -142213Z).
//! - At the first blow on the capital, a median of 25% of our land soldiers
//!   stood within three tiles of it (0% in three games).
//! - civvis-20261115T222126Z, Immortal Rome: Nubia declared at t110 and took
//!   Rome at t117 with two Cuirassiers, a Bombard and a Great General.
//!   Nubia's military was 1.0 times ours at the declaration and 0.7-0.8
//!   times ours from t112. One Crossbowman held the city. Ten to eleven of
//!   our soldiers stood four to six tiles out, most of them the Defend Rome
//!   force (12% ready). The Anvil posted them around Rome, and the rewrite
//!   sent each back to its own tile, up to six a turn. They held there or
//!   fortified for seven turns.
//! - civvis-20261116T033254Z, Immortal Rome: Phoenicia declared at t133 and
//!   took Rome at t144. Phoenicia's military was 1.3 times ours at the
//!   declaration and 0.7-0.9 times ours from t140. A Field Cannon held the
//!   city. The Defend force's Cuirassier, Man-at-Arms and Field Cannons were
//!   posted at Rome's ring and kept, turn after turn, at tiles four to eight
//!   out.
//!
//! **Scope.** Only the one-blow rewrite, only at a city of ours that is our
//! original capital, and only for a soldier standing farther than
//! [`CAPITAL_STAND_OFF`] from it. A soldier the posts left alone, a soldier
//! within two, and every other city are unchanged. With
//! `anvil-spares-the-one-blow` off there is no rewrite, so nothing changes.
//! One journal line per soldier called in.

use super::*;

/// How far from our original capital the one-blow rewrite may leave a
/// posted soldier standing: the radius of the tiles the rewrite offers.
pub(super) const CAPITAL_STAND_OFF: i32 = 2;

impl AdvancedAi {
    /// The stand-off limit for the Anvil's one-blow rewrite at city `cid`:
    /// [`CAPITAL_STAND_OFF`] with the gene on and `cid` our original capital,
    /// else `None` (the rewrite as it was).
    pub(super) fn capital_stand_off(&self, g: &Game, pid: usize, cid: u32) -> Option<i32> {
        if !self.spared_soldiers_come_to_the_capital {
            return None;
        }
        let city = g.cities.get(&cid)?;
        (city.is_capital && city.original_owner == pid && city.owner == pid)
            .then_some(CAPITAL_STAND_OFF)
    }
}

#[cfg(test)]
mod tests;
