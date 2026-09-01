//! `settler-target-floor`: a Settler is never sent to a site that is not
//! worth the walk.
//!
//! ## The live evidence (Emperor, run `civvis-20260901T182050Z`, Rome)
//!
//! Between t55 and t105 the seat's Settlers changed target on 61 of 100
//! marching turns. With `rapid-city-expansion` on, `best_settler_target`
//! exhausts the four-tile ring and then takes the global best with no travel
//! premium and no floor, so once the near ring was empty the walker sent
//! Settlers 14–18 tiles to sites worth **−35.0**, **−4.1**, **−4.7**, 17.7
//! and 27.9 (`why.log`: *"marching to (59, 8) | 14 tiles away, the site is
//! worth -35.0"*), while the seven sites it did found were worth 96–140.
//! Two Settlers were lost on those roads (t97, t103); none of the far marches
//! ended in a city.
//!
//! ## What the gene does
//!
//! Every candidate a Settler may be sent to is charged for the walk beyond
//! the ordinary eight-tile ring at [`SETTLER_EXTRA_TRAVEL_PRICE`] a tile, and
//! a candidate whose worth after travel is under [`SETTLER_TARGET_WORTH_FLOOR`]
//! is not a target — in the ranked search and in `settler-never-idles`'
//! exhaustion search alike. A near site worth 14 still passes (a city beats
//! no city); a far site worth 18, or any site worth nothing, does not. What
//! the Settler does when nothing passes is unchanged: the exhaustion ladder,
//! then the stranded hold or a founding where it stands.
//!
//! Off (the default) the predicate is `true` for every site.
use super::{AdvancedAi, SETTLER_EXTRA_TRAVEL_PRICE};
use crate::game::Game;
use crate::Pos;

/// The least a site may be worth, after travel, to be a Settler's target.
pub(super) const SETTLER_TARGET_WORTH_FLOOR: f64 = 10.0;

/// Tiles the walker may cover before travel is charged — the ordinary local
/// search radius, so a site inside it is judged on its worth alone.
pub(super) const SETTLER_TARGET_FREE_TILES: i32 = 8;

impl AdvancedAi {
    /// A site's worth net of the walk beyond the free ring.
    pub(super) fn settler_target_worth_after_travel(
        g: &Game,
        from: Pos,
        site: Pos,
        worth: f64,
    ) -> f64 {
        let extra = (g.wdist(from, site) - SETTLER_TARGET_FREE_TILES).max(0) as f64;
        worth - extra * SETTLER_EXTRA_TRAVEL_PRICE
    }

    /// Whether the gene lets a Settler at `from` take `site` as a target.
    pub(super) fn settler_target_clears_floor(
        &self,
        g: &Game,
        from: Pos,
        site: Pos,
        worth: f64,
    ) -> bool {
        !self.settler_target_floor
            || Self::settler_target_worth_after_travel(g, from, site, worth)
                >= SETTLER_TARGET_WORTH_FLOOR
    }
}
