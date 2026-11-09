//! `research-weighs-our-own-uniques`: research prices the unlocks that are
//! ours and none that belong to another civilization.
//!
//! **Why (live 6p Continents, random leaders, 2026-10-31).** The live seat
//! now plays whichever leader Civilization VI deals, so a civilization's own
//! designs only help if research reaches the node that unlocks them.
//! - `civic_value` credits the buildings and districts a civic unlocks but no
//!   improvement, so a civic whose prize is our unique improvement is worth
//!   only its policies. Robert the Bruce (civvis-20261031T011801Z, 205 turns)
//!   never adopted Reformed Church, so Scotland never built a Golf Course. Of
//!   the 9 seats dealt a civilization with a unique improvement, only
//!   Sumeria's (no prerequisite) ever built one. Australia, Canada, Egypt,
//!   Ethiopia, France, Gran Colombia, the Inca, Mapuche, the Netherlands,
//!   Persia and Sweden gate theirs on a civic too.
//! - `tech_value` and `civic_value` credit every building, district and
//!   improvement a node unlocks for anyone. Every seat prices Masonry for
//!   China's Great Wall and Irrigation for India's Stepwell, Theology for the
//!   Madrasa, Prasat and Stave Church, and Games and Recreation for the
//!   Tlachtli, Street Carnival and Hippodrome. Units already read
//!   `unique_to` (`unique-unit-preference`).
//!
//! **What the gene does.**
//! - Both functions skip an unlock whose `unique_to` names another
//!   civilization.
//! - They credit our own unique building, district or improvement with
//!   [`OWN_UNIQUE_UNLOCK_CREDIT`], the +55 the unique unit gets. Not while
//!   `civ_blind`.
//! - `civic_value` credits the improvements a civic unlocks with
//!   `tech_value`'s improvement term.
//!
//! Off, both functions are unchanged.

use super::*;

/// The research credit for an unlock that is our own civilization's design:
/// the unique unit's `tech_value` credit.
pub(super) const OWN_UNIQUE_UNLOCK_CREDIT: f64 = 55.0;

impl AdvancedAi {
    /// Whether research prices an unlock that `unique_to` owns for `pid`.
    /// Always true with the gene off.
    pub(super) fn research_counts_unlock(
        &self,
        g: &Game,
        pid: usize,
        unique_to: Option<&str>,
    ) -> bool {
        !self.research_weighs_our_own_uniques
            || unique_to.is_none_or(|owner| owner == g.players[pid].civ.as_str())
    }

    /// The extra research credit for an unlock that is `pid`'s own unique.
    /// Zero with the gene off, or civ-blind.
    pub(super) fn own_unique_unlock_credit(
        &self,
        g: &Game,
        pid: usize,
        unique_to: Option<&str>,
    ) -> f64 {
        if self.research_weighs_our_own_uniques
            && !self.civ_blind
            && unique_to == Some(g.players[pid].civ.as_str())
        {
            OWN_UNIQUE_UNLOCK_CREDIT
        } else {
            0.0
        }
    }

    /// `civic_value`'s improvement term, `tech_value`'s for a civic: each
    /// improvement the civic unlocks for us. Zero with the gene off.
    pub(super) fn civic_improvement_value(
        &self,
        g: &Game,
        pid: usize,
        civic: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        if !self.research_weighs_our_own_uniques {
            return 0.0;
        }
        g.rules
            .improvements
            .values()
            .filter(|improvement| improvement.civic.as_deref() == Some(civic))
            .filter(|improvement| {
                self.research_counts_unlock(g, pid, improvement.unique_to.as_deref())
            })
            .map(|improvement| {
                self.yield_value(improvement.yields, strategy) * 10.0
                    + 18.0
                    + self.own_unique_unlock_credit(g, pid, improvement.unique_to.as_deref())
            })
            .sum()
    }
}

#[cfg(test)]
mod tests;
