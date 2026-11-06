//! `wounded-ship-sails-on`: a wounded sea scout with no healing water in reach
//! keeps charting instead of fortifying where it can never recover.
//!
//! **Why (live Deity, 2026-11-05).** A ship heals 20 a turn in friendly water
//! and nothing in neutral water (`Game::unit_heal_rate_at`). `rotate_wounded`
//! ranks the tiles in reach by that rate, but when every one reads 0 it still
//! picks a tile, usually the one the ship stands on, fortifies it and remembers
//! it as recovering, so the ship waits for hit points that cannot come and the
//! turn is spent. Over 28 games, 80% of the turns a hull stood still after
//! Cartography (524 of 654) were wounded ships in unowned water with their
//! health frozen: 172506Z's Galley stood 95 turns at 6 hp, 121129Z's 93 at 39,
//! 111305Z's 80 at 53, 093803Z's 78, 095229Z's 42 at 72, journalling "holds
//! position to heal". Such a ship also still counts as the empire's naval eye
//! (`naval_recon_is_the_missing_arm`), so nothing replaces it, and the map
//! stayed 17.5% revealed at t100 and 32.5% at t150, with 4 of 9 city-states met
//! by t100.
//!
//! **What the gene does.** A wounded ship stops being a recovery case when it
//! is out of danger, no tile it can reach this turn heals it, and it is on the
//! sea-scout roster (`naval_explorer`). It drops out of the recovering set and
//! takes its explorer's turn, which steers around known threats. Friendly water
//! in reach still wins, so a ship near home heals as before; a ship in danger
//! or with nothing left to chart is treated as before. If the ship is lost the
//! arm reopens and a new hull can be built, which is better than a parked one.
//! Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether `uid` should chart rather than recover: a sea-scout hull with no
    /// healing water in reach. The caller has already read it as wounded and
    /// out of danger. See the module.
    pub(super) fn wounded_ship_sails_on(&self, g: &Game, pid: usize, uid: u32) -> bool {
        if !self.wounded_ship_sails_on {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        g.rules.units[unit.kind].domain.as_deref() == Some("sea")
            && std::iter::once(unit.pos)
                .chain(g.reachable(uid))
                .all(|tile| g.unit_heal_rate_at(uid, tile) <= 0)
            && self.naval_explorer(g, pid).contains(&uid)
    }
}

#[cfg(test)]
mod tests;
