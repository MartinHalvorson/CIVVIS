//! `sea-locked-front-moves-on`: a war front the army cannot walk to gives way
//! to an enemy it can.
//!
//! `siege-target-needs-a-road` stands a capture down when half its train holds
//! on land for want of a road, and drops the owner from the campaign ranking
//! while no land road from our cities reaches any of its cities. The one-war
//! front does not read that ranking: the front chosen at the war's start
//! sticks while its war lasts, so the army goes on mustering for the same
//! rival's next city. Live Emperor civvis-20261011T004440Z (a gate passer,
//! lost on Technology at 218) kept the Netherlands as its front from turn 146
//! to 192 and stood ten Dutch sieges down in a row (Amsterdam, Nijmegen,
//! Arnhem, Eindhoven, Groningen, Apeldoorn, Amsterdam, Nijmegen, The Hague,
//! Leiden), 0 units staged at each, "no land road opens within 128 steps",
//! while the Maya (from turn 165, 505 power against our 1,862) and Egypt
//! (from 175, 358) were at war with us on our side of the water. Across the
//! 10-10/11 control runs, the campaign stayed on a stood-down owner for 148
//! turns, 42 of them with another war at 1.5x our power or better running.
//!
//! `siege_road_cuts_off` reads no land road from ANY of our cities, so a single
//! foothold city on the rival's landmass keeps the rival "reachable" while the
//! army stands on another one. In G004440Z every Dutch city read reachable from
//! our cities, yet a replay of the mirror at turns 158-175 found no dry road at
//! all from 13-21 of our land units to Amsterdam, Groningen or Eindhoven (5
//! units had one). The army's own evidence is the stand-downs: two of the
//! front's cities stood down for want of a road at once, held short of water
//! (no closed border named), read the front as sea-locked too.
//!
//! Under the gene, when the front is cut off either way, the front moves to an
//! enemy already at war with us that we outgun [`SEA_LOCKED_SWAP_RATIO`]
//! times, whose cities a land road reaches and which the campaign may target:
//! the holder of an original capital Domination needs first, then the
//! weakest. The war that ends the game (`domination_finish_front`), the
//! Diplomatic Victory contender and the capital hand-offs keep their place
//! ahead of it; the old war stays open. The front, once moved, sticks as any
//! front does.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;
use crate::think;

/// Our military power over the land-reachable enemy's for the front to move
/// to it. The 10-10/11 census counted the turns another war ran at 1.5x or
/// better; a front we do not outgun is no better use of an idle army than
/// the shore.
pub(crate) const SEA_LOCKED_SWAP_RATIO: f64 = 1.5;

/// Road stand-downs of the front's cities, held short of water and still
/// holding, that read the front as sea-locked although a land road from some
/// city of ours reaches the rival. One can be an island city of a mainland
/// rival; G004440Z had two from turn 169 (Amsterdam 165, Nijmegen 169).
pub(crate) const SEA_LOCKED_STAND_DOWNS: usize = 2;

impl AdvancedAi {
    /// `sea-locked-front-moves-on`: whether the army cannot walk to `front`:
    /// cut off from every city of ours (`siege_road_cuts_off`), or with
    /// [`SEA_LOCKED_STAND_DOWNS`] of its cities stood down short of water.
    pub(crate) fn front_sea_locked(&self, g: &Game, pid: usize, front: usize) -> bool {
        self.siege_road_cuts_off(g, pid, front)
            || self.water_road_stand_downs(g, pid, front) >= SEA_LOCKED_STAND_DOWNS
    }

    /// `sea-locked-front-moves-on`: the enemy to move the front to when the
    /// army cannot walk to the current `front` (`front_sea_locked`). An
    /// enemy whose own sieges stand down short of the water is no better
    /// front. `None` with the gene off, outside the
    /// Domination target, while the front can still be walked to, or when no
    /// enemy in `enemies` qualifies. Journals the move.
    pub(crate) fn sea_locked_front_swap(
        &self,
        g: &Game,
        pid: usize,
        front: usize,
        enemies: &[usize],
    ) -> Option<usize> {
        if !self.sea_locked_front_moves_on
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !self.front_sea_locked(g, pid, front)
        {
            return None;
        }
        let power = g.military_power(pid);
        let (_, _, next) = enemies
            .iter()
            .copied()
            .filter(|rival| *rival != front)
            .filter(|rival| {
                g.players
                    .get(*rival)
                    .is_some_and(|player| player.alive && !player.is_minor && !player.is_barbarian)
            })
            .filter(|rival| power >= SEA_LOCKED_SWAP_RATIO * g.military_power(*rival).max(1.0))
            .filter(|rival| self.rival_reachable_by_land(g, pid, *rival))
            .filter(|rival| self.water_road_stand_downs(g, pid, *rival) == 0)
            .filter(|rival| self.campaign_target_legal(g, pid, *rival))
            .map(|rival| {
                let needed = self.domination_capital_target_for(g, pid, Some(rival)).is_some();
                (!needed, g.military_power(rival), rival)
            })
            .min_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)))?;
        think!(self.journal(), Military, Strategy,
            "The front leaves sea-locked {} for land-reachable {}", g.players[front].civ, g.players[next].civ;
            "{} of {}'s sieges stand down short of the water (a land road from our cities reaches it: {}); {:.0} power against {}'s {:.0}, a land road to its cities",
            self.water_road_stand_downs(g, pid, front), g.players[front].civ, self.rival_reachable_by_land(g, pid, front), power, g.players[next].civ, g.military_power(next));
        Some(next)
    }
}

#[cfg(test)]
mod tests;
