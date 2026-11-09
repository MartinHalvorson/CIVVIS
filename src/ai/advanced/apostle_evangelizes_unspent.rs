//! `apostle-evangelizes-unspent`: an Apostle that has spent a charge stops
//! trying to Evangelize a Belief.
//!
//! **Why (live Deity Russia, 2026-11-09).** Across the day's Russia games the
//! bridge asked for `EVANGELIZE_BELIEF` on 99 Apostle-turns. With 3 or more
//! charges it landed on all 17 turns where the belief could be credited to
//! one Apostle. With 2 charges it landed on none of 82, and the host answered
//! `cannot_evangelize_*` each time. Launching the Inquisition costs a charge,
//! so the Apostle that does it cannot add a belief afterwards.
//! `civvis-20261109T174632Z` launched at t73 and then stood on its Lavra from
//! t74 to t93 re-asking, first for Holy Order and then for Mosque, until
//! another Apostle added Mosque at t93. Other games lost 29, 22 and 7 turns
//! the same way.
//!
//! The refusal is read from those games, not from a shipped script: the
//! operation's row in `UnitOperations.xml` states no requirement, and the
//! host gives no reason (`CanStartOperation` is simply false). Charges below
//! the unit's ruleset count is the narrowest rule that fits every game.
//!
//! **What the gene does.** An Apostle below the ruleset's charge count skips
//! Evangelize and goes on to its other work this turn. Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether this Apostle may ask to Evangelize. Always true with the gene
    /// off.
    pub(super) fn apostle_may_evangelize(&self, g: &Game, uid: u32) -> bool {
        if !self.apostle_evangelizes_unspent {
            return true;
        }
        let unit = &g.units[&uid];
        unit.charges >= g.rules.units[unit.kind].charges
    }
}

#[cfg(test)]
mod tests;
