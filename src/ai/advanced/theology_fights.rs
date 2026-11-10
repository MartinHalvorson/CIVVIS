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
//!
//! `theology-fights-count-the-host-edge`: the reading above still left out
//! two terms of the live fight. On 2026-11-09 the host priced 75 of our
//! religious strikes, and 40 of the 64 it could be matched to a fight were
//! losing exchanges, which dealt 786 and took 1,338 with no kill. Rival
//! Apostles carry +4 for difficulty and usually +3 or +6 Intel, so an Apostle
//! duel the model reads as an exact tie, which passes `dealt >= received`,
//! really runs 7 or more points against us. The gene reads the fight with
//! those two terms; the rule itself does not change.
//!
//! The model also gave an attacker Holy Ground, which the host gives to the
//! defender only (79 defenders and no attacker in 346 previews), so the
//! gene's reading leaves it off the attacker.

use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;

impl AdvancedAi {
    /// Whether a `TheologicalAttack` from `uid` on `target` is worth starting:
    /// the mean blow kills the defender, or it is at least the blow received
    /// and ours survives that blow. An unreadable exchange is not taken.
    pub(super) fn theology_fight_is_winnable(&self, g: &Game, uid: u32, target: Pos) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let reading = if self.theology_fights_count_the_host_edge {
            g.theological_exchange_with_host_edge(uid, target)
        } else {
            g.theological_exchange_preview(uid, target)
        };
        let Some((dealt, received, defender_hp)) = reading else {
            return false;
        };
        // `corps-comes-home-alive`: see `advanced/apostle_army.rs`.
        if self.corps_declines_the_fight(g, uid, (dealt, received, defender_hp)) {
            return false;
        }
        dealt >= f64::from(defender_hp) || (dealt >= received && f64::from(unit.hp) > received)
    }
}

#[cfg(test)]
mod tests;
