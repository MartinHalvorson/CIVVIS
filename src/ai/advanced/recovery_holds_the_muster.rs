//! `recovery-holds-the-muster`: while the plan reads Recovery ("at war and
//! losing ground at home"), the Objective Board raises no Siege row for a
//! city our siege train has not yet invested, and the train musters for no
//! such city. The army the muster held stays free for the Defend and
//! Relieve rows at home.
//!
//! **The finding (Indefinite Benchmarking Tourney, 2026-11-26).** The
//! reviewer named the same mistake in 8 of 40 Lv 8 games. In
//! `civvis-t8-20261126T122815Z` "Task force will muster for Siege Brisbane"
//! ran 87 times from t41 to t81, while "Defend Cairo" ran t30-t80, "Defend
//! Baghdad" t35-t117 and the plan kept reading "recovery | at war and losing
//! ground at home" at 219 power against 519. The siege itself read "nothing
//! to open the walls, so the train holds outside the city's reach; muster
//! holds" and "6 guns cannot breach 300 walls"; the peace gate in the same
//! turns said "no siege of ours against them is feasible". Cairo fell at
//! t81. In `civvis-t8-20261126T154251Z` "muster for Siege Novgorod" ran 367
//! times (t85-t187) beside Defend rows for six of our cities; Novgorod was
//! never taken, and Alamata (t130) and the capital (t186) fell.
//!
//! The existing stand-downs did not catch it: `siege-at-the-walls-dies-first`
//! holds Siege rows only on the turns a gun or a taker stands at our walls,
//! so the row came back between assaults; `outclassed-siege-stands-down`
//! reads a one-blow era gap the stagers did not face; and Recovery itself
//! changes production and research weights, not the board.
//!
//! **The gene.** An assessment under Recovery notes its turn. For
//! [`HOLD_TURNS`] turns from that note (the plan flips Recovery and the
//! lane strategy from one assessment to the next while the war lasts), a
//! Siege row stands down unless the train has invested the city (stage
//! Invest, Reduce or Take) or the city is falling (walls gone, at most half
//! its health). A stood-down city still covers its neighbourhood against
//! Destroy rows, as the other stand-downs do. An arena is never held.
//! `Kind::OptIn`; with the gene off nothing is read or remembered.
use super::siege_train::SiegeStage;
use super::{AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::Game;

/// Turns a Recovery reading holds the muster: the reading's own turn and
/// the next two, so a plan that flips to its lane for a turn or two between
/// Recovery readings does not send the army back out.
pub(crate) const HOLD_TURNS: u32 = 3;

/// A falling city's health ceiling: walls gone and at most half of its 200.
const FALLING_CITY_HP: i32 = 100;

impl AdvancedAi {
    /// `recovery-holds-the-muster`: note this assessment's plan and return
    /// whether the muster is held this turn. False with the gene off and on
    /// an arena.
    pub(super) fn recovery_muster_held(&mut self, g: &Game, plan: &StrategicPlan) -> bool {
        if !self.recovery_holds_the_muster || g.is_arena() {
            return false;
        }
        if plan.strategy == GrandStrategy::Recovery {
            self.recovery_muster_held_turn = Some(g.turn);
        }
        self.recovery_muster_holds(g)
    }

    /// Whether a Recovery reading within [`HOLD_TURNS`] holds the muster.
    fn recovery_muster_holds(&self, g: &Game) -> bool {
        self.recovery_holds_the_muster
            && !g.is_arena()
            && self
                .recovery_muster_held_turn
                .is_some_and(|turn| g.turn >= turn && g.turn - turn < HOLD_TURNS)
    }

    /// Whether the siege of `cid` is held this turn: the muster is held and
    /// the train has not invested the city, which is not falling. False with
    /// the gene off.
    pub(super) fn recovery_holds_the_siege(&self, g: &Game, cid: u32) -> bool {
        if !self.recovery_muster_holds(g) {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.wall_hp <= 0 && city.hp <= FALLING_CITY_HP {
            return false;
        }
        !matches!(
            self.sieges.get(&cid).map(|siege| siege.stage),
            Some(SiegeStage::Invest | SiegeStage::Reduce | SiegeStage::Take)
        )
    }
}

#[cfg(test)]
mod tests;
