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
//! One Museum is enough to open that line: once it stands, the
//! `natural_history` civic goal (`culture_museum_unlock_goal`) and the
//! Archaeologist valuation (`excavation_sites` > 0) already run. Every later
//! Museum stays an Art Museum.

use super::{AdvancedAi, VictoryTarget};
use crate::game::{Game, Item};
use crate::name::Name;

/// Enough to break the Museums' exact tie, and to move the Museum ahead of an
/// ordinary building in the same city, without outbidding a district.
pub(super) const DIG_MUSEUM_BONUS: f64 = 300.0;

impl AdvancedAi {
    /// The bonus `production_value` adds to an Archaeological Museum while the
    /// Culture seat owns none, built or queued.
    pub(super) fn dig_museum_bonus(&self, g: &Game, pid: usize, building: Name) -> f64 {
        if !self.culture_digs_one_museum
            || self.lane_lost
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || !g.building_is_family(building, crate::name!("archaeological_museum"))
        {
            return 0.0;
        }
        let owns_one = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .any(|city| {
                city.buildings.iter().any(|built| {
                    g.building_is_family(*built, crate::name!("archaeological_museum"))
                }) || city.queue.iter().any(|item| {
                    matches!(item, Item::Building { building }
                        if g.building_is_family(*building, crate::name!("archaeological_museum")))
                })
            });
        if owns_one {
            0.0
        } else {
            DIG_MUSEUM_BONUS
        }
    }
}

#[cfg(test)]
mod tests;
