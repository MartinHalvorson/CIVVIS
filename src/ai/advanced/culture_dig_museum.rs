//! `culture-digs-one-museum`: a Culture seat makes one Museum an
//! Archaeological Museum.
//!
//! The two Museums cost and yield the same and value the same in
//! `production_value` (Great Work slots × 180 for a Culture plan), and the
//! cost tie in the base AI now breaks toward the Art Museum. So a Culture seat
//! never owns an Artifact slot: live King culture seat civvis-20261101T091422Z
//! produced 66 Art Museum orders, no Archaeological Museum and no
//! Archaeologist. Its map listed Antiquity Sites, and it lost to a rival
//! Science finish at t237, 82 visitors short of the bar. The map generator
//! buries six Antiquity Sites and two Shipwrecks per civilization, and an
//! Artifact pays Tourism from the turn it is dug to the end of the game.
//!
//! Two paths pick a Museum, and the gene reaches both: the strategic scorer
//! (`production_value`, through [`AdvancedAi::dig_museum_bonus`]) and the
//! delegated baseline governor that `lane-delegates-production-2` hands every
//! city of a lane seat for the whole game, whose cost-tie sort in `ai.rs`
//! counts an Artifact slot at half an Art slot. For that call the seat lends
//! `BasicAi::digs_one_museum`, which counts it at double instead while the
//! empire owns none.
//!
//! One Museum is enough to open that line: once it stands, the
//! `natural_history` civic goal (`culture_museum_unlock_goal`) and the
//! Archaeologist valuation (`excavation_sites` > 0) already run. Every later
//! Museum stays an Art Museum.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;
use crate::name::Name;

/// Enough to break the Museums' exact tie, and to move the Museum ahead of an
/// ordinary building in the same city, without outbidding a district.
pub(super) const DIG_MUSEUM_BONUS: f64 = 300.0;

impl AdvancedAi {
    /// Whether the gene acts for this seat now: on, an active Culture target,
    /// and the lane still open.
    pub(super) fn dig_museum_lane_active(&self, g: &Game) -> bool {
        self.culture_digs_one_museum
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
    }

    /// The bonus `production_value` adds to an Archaeological Museum while the
    /// Culture seat owns none, built or queued.
    pub(super) fn dig_museum_bonus(&self, g: &Game, pid: usize, building: Name) -> f64 {
        // `culture-digs-every-site` carries this gene: the bonus lasts until
        // the Museums its reachable sites want stand. See
        // `advanced/culture_digs_every_site.rs`.
        if self.dig_every_site_active(g)
            && g.building_is_family(building, crate::name!("archaeological_museum"))
        {
            let wanted = self.dig_museums_wanted(g, pid);
            return if crate::ai::BasicAi::dig_museums_owned(g, pid) < wanted {
                DIG_MUSEUM_BONUS
            } else {
                0.0
            };
        }
        if !self.dig_museum_lane_active(g)
            || !g.building_is_family(building, crate::name!("archaeological_museum"))
            || crate::ai::BasicAi::owns_dig_museum(g, pid)
        {
            return 0.0;
        }
        DIG_MUSEUM_BONUS
    }
}

#[cfg(test)]
mod tests;
