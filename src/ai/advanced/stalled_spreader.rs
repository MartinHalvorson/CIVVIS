//! `stalled-spreader-braves-the-march`: on a Religion seat, a spreader that
//! `spreader-steps-out-of-the-apostle` has held for three turns running stops
//! yielding to a rival Apostle that would only out-trade it, and keeps
//! yielding only to a blow that would kill it.
//!
//! **Why (10-31).** The evasion counts a rival as a threat when its mean blow
//! would kill our unit *or* deal more than it takes back. On a Religion seat
//! the spreaders end up in a pocket: their own tile is safe, but every step
//! toward every target lands in a winning Apostle's reach, so the march
//! holds (and the target loop tries each target and holds again). An Apostle
//! that camps there pins the corps for as long as it stays. Live Immortal
//! 063431Z (Australia, Religion, founded t38, 10 cities) logged 821 "holds
//! outside a rival Apostle's reach" lines on 43 turns: missionary 76 was held
//! t118-177, 75 t118-138, 64 t110-159. The seat spread 5 times all game and
//! lost to Kongo's Science victory at t179. In the 10-10 Emperor replays the
//! evasion was built on, 0-2 units were held a turn on domination seats, so
//! this is a different regime.
//!
//! **What it does.** The march's hold records the unit's streak of
//! consecutive held turns. Once a streak reaches [`STALL_TURNS`], for
//! [`BRAVE_TURNS`] after its last hold, both the flee and the march read a
//! rival as a threat only when its blow would kill the unit outright. A kill
//! still hands the winner 250 pressure, and that is still avoided; an
//! exchange the unit survives is accepted, so it walks on and spreads. A
//! wounded unit's next blow is lethal again, so it still retreats. Only on a
//! seat whose victory target is Religion, where spreading is the victory.
//! Off, nothing changes.

use super::*;

/// Consecutive held turns before the unit braves the march.
pub(crate) const STALL_TURNS: u32 = 3;

/// Turns after its last hold that a stalled unit keeps braving the march.
pub(crate) const BRAVE_TURNS: u32 = 6;

impl AdvancedAi {
    /// Record that the march held `uid` this turn. Several holds in one turn
    /// (one per target tried) count once. Nothing with the gene off.
    pub(super) fn record_spreader_hold(&self, g: &Game, uid: u32) {
        if !self.stalled_spreader_braves_the_march {
            return;
        }
        let mut streaks = self.spreader_hold_streaks.borrow_mut();
        let streak = streaks.entry(uid).or_insert((g.turn, g.turn));
        if streak.1 + 1 >= g.turn {
            streak.1 = g.turn;
        } else {
            *streak = (g.turn, g.turn);
        }
    }

    /// Whether `uid`, on a Religion seat, has been held [`STALL_TURNS`] turns
    /// running and is still within [`BRAVE_TURNS`] of its last hold. `false`
    /// with the gene off.
    pub(super) fn spreader_braves_the_march(&self, g: &Game, uid: u32) -> bool {
        if !self.stalled_spreader_braves_the_march
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
        {
            return false;
        }
        self.spreader_hold_streaks
            .borrow()
            .get(&uid)
            .is_some_and(|(first, last)| {
                last + 1 - first >= STALL_TURNS && g.turn <= last + BRAVE_TURNS
            })
    }
}

#[cfg(test)]
mod tests;
