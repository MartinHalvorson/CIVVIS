//! `capital-walls-at-any-ratio`: at Immortal and Deity, the capital's walls
//! (`capital-walls-before-the-surprise`) arm on a watched major's soldier at
//! our door whatever its power over ours, instead of only at
//! [`CAPITAL_WALLS_RATIO`] times it.
//!
//! **Why (live Immortal, 2026-10-30/31, -8b's census of 74 seats).** 29 of
//! 80 seats lost a city before t100, 29 of the 38 losses to a major's war,
//! and 8 of the 18 located losses were the CAPITAL, a median 11 tiles from
//! the attacker's. The ratio barely tells the attackers apart: the taker
//! stood a median 2.69 times our power in the ten turns before declaring
//! (7 of 10 at 2.0 or more), but neighbours that never declared stood a
//! median 3.0 (14 of 20 at 2.0 or more), and the declarations that took the
//! capital came early (four at t21-26, the capital gone by t25-28). Live
//! civvis-20261030T201608Z: Egypt, 1.41 times our power at t50, declared at
//! t54 with Athens unwalled; the walls were started at t60 and broken by t78,
//! and Athens fell at t80. Walls are 40 Production and cannot be bought; the
//! capital is the game.
//!
//! **What the gene does.** `capital_watched_major_power` reads a ratio of 1.0
//! as the arming bar at Immortal and Deity, so the existing watch (a soldier
//! within eight tiles of the original capital in the last ten live turns)
//! arms the Masonry research and the capital's Ancient Walls whenever it
//! sees one. Every other leg of the gene (the Settler that finishes first,
//! the walls that started finishing, the bought defender at the door) is
//! unchanged. Off, or below Immortal, the bar stays 2.0.

use super::capital_walls_before_the_surprise::CAPITAL_WALLS_RATIO;
use super::*;

/// The arming bar at Immortal and Deity: any watched major stronger than us.
pub(super) const CAPITAL_WALLS_HIGH_TIER_RATIO: f64 = 1.0;

impl AdvancedAi {
    /// The power ratio over ours at which a watched major arms the capital's
    /// walls on this board.
    pub(super) fn capital_walls_ratio(&self, g: &Game) -> f64 {
        if self.capital_walls_at_any_ratio && matches!(g.difficulty.as_str(), "immortal" | "deity")
        {
            CAPITAL_WALLS_HIGH_TIER_RATIO
        } else {
            CAPITAL_WALLS_RATIO
        }
    }
}

#[cfg(test)]
mod tests;
