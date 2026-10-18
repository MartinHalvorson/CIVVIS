//! `foothold-faces-the-capital`: under a Domination conquest, a rival town
//! within [`FOOTHOLD_REACH`] of a rival's original capital, while no city of
//! ours stands that close to it, is worth [`FOOTHOLD_VALUE`] more to the
//! campaign. Taking it gives the Bombers a base in range of the capital and
//! the land army a place to heal and muster beside it.
//!
//! **Why (live Emperor, the 11 pin-era games of 2026-10-18 that passed the
//! turn-150 gate).** On 723 turns from turn 120 we were at war with the
//! owner of a rival capital. On 408 of them (56%), no city of ours stood
//! within Bomber range (10 tiles) of that capital, while a rival town did in
//! every one. The campaign's first objective was that town on 154 turns,
//! the capital on 29, and a town more than 10 tiles from the capital on 225
//! (median 12 tiles). Bombers struck a capital on 6 of the 723 turns, though
//! we held two or more on 554. Where a foothold existed they worked: from
//! Ostia, 4 tiles out, they took Rome's walls from 400 to 256 in five turns
//! (civvis-20261018T075143Z). The other campaign terms charge 7 a tile from
//! our core and 5 from our army, so a town a dozen tiles nearer home wins by
//! about 90; [`FOOTHOLD_VALUE`] flips that.
//!
//! Off, nothing changes.

use super::*;
use crate::game::City;

/// A Bomber's range: the farthest a city of ours may stand from a rival
/// capital and still base the strikes on its walls.
pub(super) const FOOTHOLD_REACH: i32 = 10;

/// The campaign value of a town that would be our first base within
/// [`FOOTHOLD_REACH`] of a rival capital. See the module.
pub(super) const FOOTHOLD_VALUE: f64 = 90.0;

impl AdvancedAi {
    /// [`FOOTHOLD_VALUE`] for a major rival's town within [`FOOTHOLD_REACH`]
    /// of a rival original capital no city of ours reaches yet, under a
    /// Domination conquest; else 0. Always 0 with the gene off.
    pub(super) fn foothold_value(
        &self,
        g: &Game,
        pid: usize,
        city: &City,
        strategy: GrandStrategy,
    ) -> f64 {
        if !self.foothold_faces_the_capital
            || strategy != GrandStrategy::Conquest
            || city.owner == pid
            || city.is_capital
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return 0.0;
        }
        let Some(owner) = g.players.get(city.owner) else {
            return 0.0;
        };
        if owner.is_minor || owner.is_barbarian {
            return 0.0;
        }
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        let faces_an_unreached_capital = g.cities.values().any(|capital| {
            capital.is_capital
                && capital.owner != pid
                && g.players
                    .get(capital.owner)
                    .is_some_and(|holder| !holder.is_minor && !holder.is_barbarian)
                && g.wdist(capital.pos, city.pos) <= FOOTHOLD_REACH
                && ours
                    .iter()
                    .all(|pos| g.wdist(*pos, capital.pos) > FOOTHOLD_REACH)
        });
        if faces_an_unreached_capital {
            FOOTHOLD_VALUE
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests;
