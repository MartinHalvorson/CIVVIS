//! `russia-settles-the-tundra`: a Russian seat's site score reads what Mother
//! Russia pays on frozen ground.
//!
//! The shipped rows, `Base/Assets/Gameplay/Data/Civilizations.xml:480-497`:
//! `TRAIT_INCREASED_TUNDRA_FAITH` and `TRAIT_INCREASED_TUNDRA_PRODUCTION` are
//! `MODIFIER_PLAYER_ADJUST_PLOT_YIELD` over `PLOT_HAS_TUNDRA_REQUIREMENTS`,
//! Amount 1 each, and the `_HILLS_` pair does the same over
//! `PLOT_HAS_TUNDRA_HILLS_REQUIREMENTS`. So every Tundra plot pays +1 Faith
//! and +1 Production, hills or not.
//!
//! The site forecast prices a plot from `Rules::tile_yields`, which has no
//! owner, so a Russian seat read plain Tundra as 1 Food. Plains pay 2 yields
//! and Grassland 2, and Tundra pays Russia 3. The mirror corrects a tile once
//! the seat owns it (`observed_tile_yield_adjustments`), which is after the
//! Settler has gone somewhere else, so the correction cannot reach this
//! decision. `capital-leaves-the-frost` already leaves Russia alone, on the
//! same reasoning.
//!
//! The gene adds the two yields to each workable Tundra plot in the forecast,
//! and to the plot's neighbours in the prefilter so a top-N scan keeps the
//! seat the full score would prefer. The founding plot itself is left out,
//! because what the City Center keeps of this modifier has not been read from
//! the host.
//!
//! `data/civs.json` still models Mother Russia as a flat +1 Faith and +1
//! Production per city, so a headless read of a Russian seat can show this
//! gene's harm but not its benefit. Off, and for any other civilization,
//! nothing here runs.

use super::AdvancedAi;
use crate::game::Game;
use crate::rules::Yields;
use crate::world::Tile;
use crate::Pos;

/// What Mother Russia adds to a Tundra plot, flat or hills.
const MOTHER_RUSSIA_TUNDRA_FAITH: f64 = 1.0;
const MOTHER_RUSSIA_TUNDRA_PRODUCTION: f64 = 1.0;

impl AdvancedAi {
    /// What this seat's Mother Russia pays on `tile`. Zero with the gene off,
    /// for any civilization but Russia, and off Tundra.
    pub(super) fn mother_russia_tile_yields(&self, g: &Game, pid: usize, tile: &Tile) -> Yields {
        if !self.russia_settles_the_tundra
            || tile.terrain != "tundra"
            || g.players.get(pid).is_none_or(|seat| seat.civ != "Russia")
        {
            return Yields::default();
        }
        Yields {
            faith: MOTHER_RUSSIA_TUNDRA_FAITH,
            production: MOTHER_RUSSIA_TUNDRA_PRODUCTION,
            ..Yields::default()
        }
    }

    /// The prefilter's share of the same credit: each neighbour at the
    /// prefilter's own weight of 1, so the top-N cut cannot drop a site the
    /// forecast would rank first. Zero whenever the tile credit is.
    pub(super) fn mother_russia_prefilter_credit(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        g.nbrs(pos)
            .into_iter()
            .filter_map(|neighbour| g.map.get(neighbour))
            .map(|tile| Self::settlement_yield_value(self.mother_russia_tile_yields(g, pid, tile)))
            .sum()
    }
}

#[cfg(test)]
mod tests;
