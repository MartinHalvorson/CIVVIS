//! `own-price-answers-the-deal` (opt-in): a CIVVIS seat at a hotseat table
//! answers every deal another seat proposed to it, by its own price.
//!
//! **Why (hotseat 2026-11-27/28).** At a hotseat table every major is a human
//! seat: the person, or another CIVVIS seat. Nobody evaluates a deal for a
//! human (the shipped deal screen auto-proposes only to a Firaxis leader), so
//! a deal proposed to a CIVVIS seat waited until its proposer withdrew it: the
//! owner proposed to seat 4 on turn 12 of `hotseat-20261128T045523Z`, looked
//! again on turn 17, cancelled on turn 27 and declared war twelve seconds
//! later; seat 4's records show nothing in between
//! (`~/civvis-civ6-runs/hotseat/findings/trade-deals-design.md`, F2). The
//! Firaxis price ask the live seat uses everywhere else (EQUALIZE) gets no
//! answer from a human (F6).
//!
//! **The rule.** The live bridge reads the hotseat deal board (`state.deals`,
//! contract "deals v1") on the turn's opening frame and, for each deal waiting
//! for this seat, sends one `deal` order: ACCEPT when the deal gains the seat
//! at least the margin by its own valuation (`trade_decider::value`), COUNTER
//! with the smallest change that makes it worth it (more Gold that our board
//! says they hold, less of ours, one of our items dropped; at most two
//! counters an exchange, never terms they refused), else REJECT
//! (`trade_decider::answer`). A deal holding an item the valuation cannot
//! price (a city, a war, an alliance) is left as it stands. Nothing reads the
//! other seat's valuation. Off, or on a board with no `deals` (every
//! single-player game), nothing changes. Grep the journal for
//! `own_price_deal=`.

use super::*;

impl AdvancedAi {
    /// Whether `own-price-answers-the-deal` is on; the live bridge asks
    /// before it answers the deals on the board (`hotseat_deals` in
    /// `civvis_orders`).
    pub fn own_price_answers_the_deal_enabled(&self) -> bool {
        self.own_price_answers_the_deal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_price_answers_the_deal_is_opt_in() {
        super::super::test_support::opt_in_off_in_both_controllers(
            "own-price-answers-the-deal",
            |ai| ai.own_price_answers_the_deal,
        );
    }

    #[test]
    fn the_bridge_reads_the_flag_it_sets() {
        let mut ai = AdvancedAi::new();
        assert!(!ai.own_price_answers_the_deal_enabled());
        ai.enable_own_price_answers_the_deal();
        assert!(ai.own_price_answers_the_deal_enabled());
        ai.disable_own_price_answers_the_deal();
        assert!(!ai.own_price_answers_the_deal_enabled());
    }
}
