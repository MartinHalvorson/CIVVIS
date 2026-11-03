//! `gun-threat-draws-the-strike`: the hostile that pins our siege guns is the
//! battle planner's priority kill.
//!
//! ## Why
//!
//! `guns-stay-out-of-reach` refuses a siege gun's step onto a tile where one
//! visible hostile's blow, at its upper roll, would destroy it; a ZOC screen
//! of ours clears the tile, and nothing else does. The veto keeps the guns
//! alive, but no part of the controller answers it: the battle planner prices
//! the pinning hostile like any other, so a gun can stand short of its post
//! for the whole siege.
//!
//! - `siege_replay` 224444Z t80 (a city-state siege by a Culture seat,
//!   `city-state-harvest` on): 23 gun steps "the move to X was refused",
//!   every one of them legal for the engine (`Game::apply` accepted it on a
//!   clone); 19 were this veto, at one chokepoint between a natural wonder
//!   and two mountains, for twenty turns, and the siege ran out of patience.
//! - Live 11-02/03: 253 of 1,093 "stops short of its post" lines (23%) were
//!   such refusals.
//!
//! ## What it does
//!
//! When the lethal tiles are marked each frame, every hostile whose one blow
//! makes a tile lethal for one of our guns is recorded in
//! `AdvancedAi::gun_threat_sources`. The battle planner adds
//! [`GUN_THREAT_KILL_BONUS`] to that target's kill value (through
//! [`AdvancedAi::strike_priority_bonus`], the general hook another gene can
//! add its own priority to), so its strikes take the blocker first. The veto
//! itself is untouched: a gun still never steps where it can be destroyed.
//!
//! Off, the set stays empty and the bonus is zero, so the planner prices
//! every target exactly as before.

use super::AdvancedAi;

/// The kill value a gun-pinning hostile is worth on top of its own, on
/// `unit_value`'s scale (a plain unit's kill is worth 190 plus its cost and
/// strength terms): about one more unit's kill.
pub(crate) const GUN_THREAT_KILL_BONUS: f64 = 200.0;

impl AdvancedAi {
    /// The extra kill value of striking `uid` this frame: the shared hook the
    /// battle planner reads. Zero unless a gene names `uid`.
    pub(super) fn strike_priority_bonus(&self, uid: u32) -> f64 {
        let mut bonus = 0.0;
        if self.gun_threat_draws_the_strike && self.gun_threat_sources.contains(&uid) {
            bonus += GUN_THREAT_KILL_BONUS;
        }
        bonus
    }
}

#[cfg(test)]
mod tests;
