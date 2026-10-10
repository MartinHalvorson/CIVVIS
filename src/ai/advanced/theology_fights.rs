//! `theology-fights-only-to-win`: an Apostle or Inquisitor of ours starts a
//! theological fight only when the exchange, both rolls at their mean, kills
//! the defender or at least trades evenly while ours survives the blow back.
//!
//! **Why (10-09/10 founder losses).** In G215527Z (Buddhism, lost t147) our
//! Inquisitors started 9 fights at strength 75 against Orthodox Apostles at
//! 100, took 277 damage for 258 dealt, killed one, and all five Inquisitors
//! died; only 3 of their 15 Remove Heresy charges were spent, and every death
//! hands the winner's faith 250 pressure where it falls, beside our own
//! cities. G094619Z (Buddhism, lost t130): 6 fights, 166 taken for 109, all
//! four Inquisitors lost. The step took any adjacent fight while it had 55 hp
//! or the defender was down 45, with no reading of the two strengths.

use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;

impl AdvancedAi {
    /// Whether a `TheologicalAttack` from `uid` on `target` is worth starting:
    /// the mean blow kills the defender, or it is at least the blow received
    /// and ours survives that blow. An unreadable exchange is not taken.
    pub(super) fn theology_fight_is_winnable(g: &Game, uid: u32, target: Pos) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let Some((dealt, received, defender_hp)) = g.theological_exchange_preview(uid, target)
        else {
            return false;
        };
        dealt >= f64::from(defender_hp)
            || (dealt >= received && f64::from(unit.hp) > received)
    }
}

#[cfg(test)]
mod tests;
