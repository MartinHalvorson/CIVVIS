//! `economy-clock`: windows tuned at Online speed keep their economy time at
//! every other speed.
//!
//! ## Why
//!
//! Almost every gene in the registry was tuned at Online speed, both in the
//! screen (`gene_screen`'s one shape is Online to its 250-turn clock) and on
//! the live board, and its turn windows were written either as raw Online
//! turns or as `g.standard_duration(n)`. Neither keeps its meaning when the
//! game slows down. Civilization VI scales costs by
//! [`GameSpeed::cost_percent`] (Online 50, Quick 67, Standard 100, Epic 150,
//! Marathon 300) but movement, combat, healing and per-turn yields not at
//! all, and `standard_duration` follows the separate durations table
//! (`GameSpeed::scale_turns`, Online 66% of Standard, Marathon 200%). So a
//! window that means "the opening", "a build that finishes soon enough" or
//! "after the expansion phase" ends early in economy time at any speed
//! slower than the one it was tuned at:
//!
//! | written as             | Online | Standard | Epic  | Marathon |
//! |------------------------|--------|----------|-------|----------|
//! | raw Online turns       | 1.00x  | 0.50x    | 0.33x | 0.17x    |
//! | `standard_duration(n)` | 1.00x  | 0.76x    | 0.63x | 0.50x    |
//!
//! (economy time relative to what the window held at Online). The live
//! ladder moved to Standard on 2026-11-03, and a census of `src/ai` found 171
//! economy-time sites mis-scaled that way, among them the opening Settler
//! floor, the unescorted-Settler window, the early-conquest naming deadline
//! and its preparation time, and the 15-36-turn build caps on the queue
//! claims (a Granary that "finishes soon enough" at Online takes twice the
//! turns at Standard and falls outside its cap). Fixed gold reserves are the
//! same defect in another currency: purchase prices follow `cost_percent`
//! while gold income per turn does not, so a reserve tuned at Online covers
//! half the purchases at Standard.
//!
//! ## What the gene does
//!
//! With the gene on, each converted site reads its window on the economy
//! clock: an Online-tuned count `n` lasts `n x cost_percent / 50` turns at
//! this speed ([`economy_turns`]), an Online-tuned `standard_duration(n)`
//! lasts `Online.scale_turns(n) x cost_percent / 50` ([`AdvancedAi::econ_window`]),
//! and a fixed gold amount scales with purchase prices
//! ([`AdvancedAi::econ_gold`]). At Online every conversion returns exactly
//! what the site read before, so an Online game is byte-identical with the
//! gene on; the gene only changes play at other speeds. With the gene off
//! every site reads what it always did.
//!
//! What is deliberately NOT converted: tactical and movement counts (a march,
//! a siege's patience, an attack memory), which do not scale with speed at
//! all, and the windows that open a DEFENCE, because a rival's army arrives
//! in movement time: stretching `near-rival-deterrence`'s start from turn 45
//! to 60 at Standard would arm later against the same rush.

use super::*;
use crate::setup::GameSpeed;

/// Turns at `speed` that hold the economy time `online_turns` Online turns
/// held: costs scale by `cost_percent`, Online's is 50.
pub(crate) fn economy_turns(speed: GameSpeed, online_turns: u32) -> u32 {
    let online = u64::from(GameSpeed::Online.cost_percent());
    (u64::from(online_turns) * u64::from(speed.cost_percent())).div_ceil(online) as u32
}

/// An amount priced at Online, at `speed`'s prices.
pub(crate) fn economy_amount(speed: GameSpeed, online_amount: f64) -> f64 {
    online_amount * f64::from(speed.cost_percent()) / f64::from(GameSpeed::Online.cost_percent())
}

impl AdvancedAi {
    /// A window the site wrote as `g.standard_duration(standard)` and tuned
    /// at Online: on the economy clock with the gene on, unchanged off. At
    /// Online both read the same.
    pub(super) fn econ_window(&self, g: &Game, standard: u32) -> u32 {
        if self.economy_clock {
            economy_turns(g.game_speed, GameSpeed::Online.scale_turns(standard))
        } else {
            g.standard_duration(standard)
        }
    }

    /// A raw turn count tuned at Online: on the economy clock with the gene
    /// on, unchanged off.
    pub(super) fn econ_turns(&self, g: &Game, online_turns: u32) -> u32 {
        if self.economy_clock {
            economy_turns(g.game_speed, online_turns)
        } else {
            online_turns
        }
    }

    /// A fixed gold amount tuned against Online prices: at this speed's
    /// prices with the gene on, unchanged off.
    pub(super) fn econ_gold(&self, g: &Game, online_amount: f64) -> f64 {
        if self.economy_clock {
            economy_amount(g.game_speed, online_amount)
        } else {
            online_amount
        }
    }
}

#[cfg(test)]
mod tests;
