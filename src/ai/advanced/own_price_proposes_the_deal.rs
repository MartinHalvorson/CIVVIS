//! `own-price-proposes-the-deal` (opt-in): a CIVVIS seat at a hotseat table
//! proposes, each turn, the one deal it gains most from by its own price, and
//! asks a human seat no session it cannot answer.
//!
//! **Why (hotseat 2026-11-27/28).** At a hotseat table every major is a human
//! seat. The live seat's trade lanes (`sell`, `buy`, the luxury swap) ask a
//! Firaxis leader's deal AI for its price (EQUALIZE), which a human seat
//! never gives (`selfplay-duel-20261127T0654Z-p0`: asked turn 54, expired
//! turn 60), so no CIVVIS seat ever proposed a deal another seat could
//! answer. Its friendship, delegation and embassy asks fared no better: each
//! is a session of its own that opens and closes on the asker's turn, before
//! the other seat plays (`hotseat-20261128T045523Z` DiplomacyManager.csv;
//! 33 `order_failed friendship not_friends` in `selfplay-duel-20261127T0654Z-p1`).
//!
//! **The rule.** On the turn's opening frame of a board that carries the
//! hotseat `deals` record, the live bridge drops every friendship, delegation,
//! embassy and alliance ask toward a `person` or `civvis` counterpart, and
//! sends at most one `deal` PROPOSE (`trade_decider::propose`): among the
//! seats at peace with nothing pending either way and outside the cooldown
//! (5 turns; the person 10), the deal with the largest gain for this seat by
//! its own valuation that clears the margin (the person also 25 Gold),
//! asking only what the other side's deal screen offers and paying or asking
//! Gold only within what our board says each side holds. It opens with half
//! of an item's worth to us and moves toward the other side after a refusal;
//! refused terms are never sent again (to the person, for the whole game).
//! Off, or on a board with no `deals`, nothing changes. Grep the journal for
//! `own_price_propose=` and `own_price_human_sessions_held=`.

use super::*;

impl AdvancedAi {
    /// Whether `own-price-proposes-the-deal` is on; the live bridge asks
    /// before it proposes (`hotseat_deals` in `civvis_orders`).
    pub fn own_price_proposes_the_deal_enabled(&self) -> bool {
        self.own_price_proposes_the_deal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_price_proposes_the_deal_is_opt_in() {
        super::super::test_support::opt_in_off_in_both_controllers(
            "own-price-proposes-the-deal",
            |ai| ai.own_price_proposes_the_deal,
        );
    }

    #[test]
    fn the_bridge_reads_the_flag_it_sets() {
        let mut ai = AdvancedAi::new();
        assert!(!ai.own_price_proposes_the_deal_enabled());
        ai.enable_own_price_proposes_the_deal();
        assert!(ai.own_price_proposes_the_deal_enabled());
        ai.disable_own_price_proposes_the_deal();
        assert!(!ai.own_price_proposes_the_deal_enabled());
    }
}
