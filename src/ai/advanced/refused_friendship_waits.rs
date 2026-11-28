//! `refused-friendship-waits` (opt-in): a major whose last friendship ask
//! the host refused is not asked again for [`REFUSED_FRIENDSHIP_WAIT_TURNS`]
//! turns. The other majors are asked meanwhile, as before.
//!
//! **Why (self-play duel `selfplay-duel-20261127T0654Z`, 2026-11-27, two
//! CIVVIS seats on one hotseat table).** Seat p1 (`score-seat-keeps-friends`,
//! which asks every turn) asked seat p0 for a friendship on every turn from
//! 51 to 89, two or three times a turn: 105 journal offers. None landed:
//! the mod refused 32 at once (`friendship_not_offered`) and the order
//! channel read 33 `not_friends` verdicts, on turns 51-53, 63-65, 75-77 and
//! 87-89. The bridge's refused-order rule (three identical failures withhold
//! an order for ten turns) only spaced those bursts, and the seat asked
//! nobody else in between: the refused major stayed its first choice and
//! the withheld order went nowhere. A friendship the host refused does not become likelier the next
//! turn: the host's leaders befriend only from Friendly, a human seat never
//! answers in the turn it is asked, and the mod's own window refuses a
//! repeat for five turns.
//!
//! **The rule.** The live bridge hands the seat, each frame, the turn of each
//! major's latest refused friendship ask (the order channel's own verdicts,
//! `HostOrderRefusals::failed_on`, mapped to this board's seats). While fewer
//! than [`REFUSED_FRIENDSHIP_WAIT_TURNS`] turns have passed since it,
//! `friendship_block` names this gene and the major is no candidate; the
//! rest of the choice (`befriend-the-strongest`, `score-seat-keeps-friends`,
//! `friendship-keeps-the-peace`'s renewal) is unchanged. A verified ask clears
//! the channel's record and so the wait. A native board is handed nothing.
//! Off, nothing is read. Grep the journal for `refused-friendship-waits:`.

use super::*;

/// Turns after a refused friendship ask in which that major is not asked
/// again: the order channel's own cooldown (`ORDER_REFUSAL_COOLDOWN_TURNS`),
/// which keeps the refusal on record exactly as long.
pub(crate) const REFUSED_FRIENDSHIP_WAIT_TURNS: u32 = 10;

impl AdvancedAi {
    /// `refused-friendship-waits`: the turn of each major's latest refused
    /// friendship ask, by this board's seat. The live bridge sets it every
    /// frame; nothing else does.
    pub fn set_refused_friendships(&mut self, refused: BTreeMap<usize, u32>) {
        self.refused_friendships = refused;
    }

    /// Whether `refused-friendship-waits` keeps `other` out of this turn's
    /// friendship ask. False with the gene off.
    pub(super) fn refused_friendship_waits_for(&self, g: &Game, other: usize) -> bool {
        self.refused_friendship_waits
            && self.refused_friendships.get(&other).is_some_and(|refused| {
                *refused <= g.turn && g.turn - *refused < REFUSED_FRIENDSHIP_WAIT_TURNS
            })
    }
}

#[cfg(test)]
mod tests;
