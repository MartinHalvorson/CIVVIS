//! `deterrence-trusts-a-friend`: `near-rival-deterrence` does not train a
//! deterrent against a rival we are declared friends with.
//!
//! The deterrent answers a neighbour at peace whose military out-guns ours,
//! because the 10-08 census found the AI's early declarations come at a power
//! ratio below parity. A Declaration of Friendship, while it lasts, bars that
//! declaration outright, and `friendship-keeps-the-peace` offers one to
//! exactly the strongest neighbour at peace: the rival the deterrent then
//! trains against.
//!
//! Live Emperor and King seats from 2026-11-01:
//! - Of 262 deterrent trainings logged across 26 seats, 144 (55%) targeted a
//!   declared friend.
//! - Over 1,232 rival-turns at peace in t30-80 with a rival city within ten
//!   tiles, a declared friend declared war within ten turns on 1.0 per 100
//!   turns (6 of 588). Unfriendly rivals did on 8.0, friendly 10.3, neutral 3.3.
//! - In two of the three King seats that were never attacked by turn 120 yet
//!   built deterrents (051531Z vs the Maya, 054633Z vs Phoenicia), 33 of
//!   their 49 deterrents targeted a declared friend.
//! - Distance does not separate the cases. Phoenicia's soldiers stood a median
//!   four tiles off for 35 turns and never attacked, while the Zulu, a median
//!   seven tiles off, did.
//!
//! With the gene on, a rival we are declared friends with is not a deterrence
//! neighbour. When the friendship lapses or ends, the deterrent answers again
//! on the next turn. Off: unchanged.
use super::*;

impl AdvancedAi {
    /// Whether `rival` is spared the deterrent: the gene is on and we are
    /// declared friends with it.
    pub(super) fn deterrence_spares_a_friend(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.deterrence_trusts_a_friend && g.are_friends(pid, rival)
    }
}

#[cfg(test)]
mod tests;
