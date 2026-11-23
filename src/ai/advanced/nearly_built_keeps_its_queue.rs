//! `nearly-built-keeps-its-queue`: a building or district with production
//! already in it that finishes within [`NEARLY_BUILT_STANDARD_TURNS`] keeps
//! the head of its city's queue against the three claims that displace the
//! most in the opening: the early city floor's Settler
//! (`early-settler-floor`), the holdable-land Settler
//! (`holdable-land-keeps-settling`) and the near rival's deterrent
//! (`near-rival-deterrence`, and its late hold). The claim goes to the next
//! city that qualifies, or waits the two or three turns.
//!
//! **Why (live Tournaments 6 and 7, 2026-11-21/22, 44 Deity and Immortal runs
//! with a why.log, turns 15-80).** Those three claims made 448 of the 659
//! queue displacements the journal records: early-settler-floor 202,
//! near-rival-deterrence 164, holdable-land-keeps-settling 82 (the Walls
//! claims, 158, answer a soldier seen near the city and are left alone).
//! Their victims were Monuments 153 times, Granaries 76, Ancestral Halls 65,
//! Temples 59. Each claim's own test lets a head stay only when it finishes
//! THIS turn (`remaining <= production`), so a Granary two turns from done
//! waited behind a ten-turn Settler: Paris t37, "starts a Settler for the
//! early city floor ... 10.0 turns; granary keeps its progress". Across 65
//! Emperor-Deity runs Pottery came at a median t21, the first city was
//! housing-bound at t25 and the first Granary stood at t47; half of all
//! city-turns from t40 to t99 sat within one citizen of their housing
//! (growth x0.5, or x0.25 at the ceiling).
//!
//! **Why this order is right, not just a preference.** Both items get built
//! either way and production is never lost (the displaced head keeps its
//! progress). The only question is which comes first. Finishing the short job
//! first delays the Settler or the deterrent by two or three turns and brings
//! the Granary's +2 Housing, the Monument's Culture or the Temple's Faith
//! forward by the Settler's whole build, eight to twelve turns. Ordering by
//! shortest remaining work is the rule that minimises the total wait when the
//! items are worth about as much a turn (Smith's rule), and a compounding
//! city building is worth at least what a Settler two turns later costs.
//!
//! Units are not covered: a Settler, Builder or Trader at the head already
//! keeps its queue under these claims, and a soldier is what the deterrent
//! wants anyway. Wonders and Walls already keep theirs. Off, nothing
//! changes: every call returns `false` before reading the board.

use super::*;

/// The longest remaining build, in standard turns, that keeps its queue: 6 is
/// three turns at the live seat's Online speed, four at Quick. Long enough
/// for a half-built Granary or Monument at three to five Production a turn,
/// short enough that the Settler or deterrent waits no more than a few turns.
pub(crate) const NEARLY_BUILT_STANDARD_TURNS: u32 = 6;

impl AdvancedAi {
    /// Whether `cid`'s queue head is a building or district with production
    /// already in it that finishes within [`NEARLY_BUILT_STANDARD_TURNS`], so
    /// the displacing claims leave it alone. Always `false` with the gene off.
    pub(super) fn nearly_built_holds(&self, g: &Game, pid: usize, cid: u32) -> bool {
        if !self.nearly_built_keeps_its_queue {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.owner != pid {
            return false;
        }
        let Some(item) = city.queue.first() else {
            return false;
        };
        if !matches!(item, Item::Building { .. } | Item::District { .. }) {
            return false;
        }
        let invested = g.item_invested_production(cid, item);
        if invested <= 0.0 {
            return false;
        }
        let remaining = g.item_cost_for_city(pid, cid, item) - invested;
        let per_turn = g.city_yields(cid).production * g.item_prod_mult(pid, cid, Some(item));
        if per_turn <= 0.0 {
            return false;
        }
        let limit = f64::from(g.standard_duration(NEARLY_BUILT_STANDARD_TURNS).max(1));
        remaining / per_turn <= limit
    }
}

#[cfg(test)]
mod tests;
