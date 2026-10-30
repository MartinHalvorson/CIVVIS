//! `culture-keeps-every-border-open`: a Culture seat holds an Open Borders
//! grant from every rival it can, every turn, once its tourism is worth
//! multiplying.
//!
//! Tourism pressure is banked against each rival with the multiplier in force
//! on the turn it is generated (`Game::add_passive_tourism`), and a rival's
//! Open Borders grant to us is +25% of everything we generate against it
//! (`Game::international_tourism_multiplier`). The Culture objective's own
//! purchase runs one Open Borders buy every sixth turn, and a grant lasts the
//! standard deal length — so with three rivals the seat held a grant from
//! 0–2 of them at most turn-25 marks of a native Immortal culture census
//! (4p pangaea, three handicapped rival chairs, 16 games): the multiplier is
//! missing on most of the pressure that decides the race.
//!
//! With the gene on, each turn the seat's tourism is at least
//! [`OPEN_BORDERS_TOURISM_FLOOR`], every met rival's Open Borders sale that the
//! ordinary Culture filters allow is bought, the largest domestic-tourist
//! count first — visitors drawn from the bar holder also lower the bar. A
//! purchase never spends into the solvency reserve. Below the floor the grant
//! is a few points of pressure for tens of Gold and the old six-turn cadence
//! is left alone. Off, nothing changes.
use super::AdvancedAi;
use crate::game::{Game, QuickDeal};
use std::collections::BTreeSet;

/// Tourism per turn from which a grant pays for itself: a grant lasts the
/// standard deal length, so at this rate it adds roughly a quarter of a
/// visitor's pressure against that rival for a quote of about 20–40 Gold.
pub(super) const OPEN_BORDERS_TOURISM_FLOOR: f64 = 15.0;

/// Gold kept back when the treasury is falling.
const DEFICIT_RESERVE: f64 = 100.0;

impl AdvancedAi {
    /// Buy every allowed Open Borders grant this turn. Returns how many closed.
    pub(super) fn keep_every_border_open(
        &self,
        g: &mut Game,
        pid: usize,
        excluded_partner: Option<usize>,
        culture_threats: &BTreeSet<usize>,
        science_threats: &BTreeSet<usize>,
    ) -> usize {
        if !self.culture_keeps_every_border_open
            || self.lane_lost
            || g.tourism_per_turn(pid) < OPEN_BORDERS_TOURISM_FLOOR
        {
            return 0;
        }
        let mut closed = 0;
        let mut tried = BTreeSet::new();
        loop {
            let best = g
                .quick_deals(pid)
                .into_iter()
                .filter(|deal| Some(deal.partner) != excluded_partner)
                .filter(|deal| !tried.contains(&deal.partner))
                .filter(|deal| {
                    deal.item == "open_borders"
                        && deal.direction == "buy"
                        && deal.my_value >= 2.0
                        && deal.partner_value >= 2.0
                })
                .filter(|deal| {
                    self.culture_deal_allowed(deal, culture_threats)
                        && self.science_denial_deal_allowed(deal, science_threats)
                })
                .filter(|deal| Self::open_borders_affordable(g, pid, deal))
                .max_by(|left, right| {
                    g.domestic_tourists(left.partner)
                        .cmp(&g.domestic_tourists(right.partner))
                        .then_with(|| right.partner.cmp(&left.partner))
                });
            let Some(deal) = best else {
                break;
            };
            tried.insert(deal.partner);
            if self.base.close_quick_deal(g, pid, deal) {
                closed += 1;
            }
        }
        closed
    }

    /// The quote's lump never spends into the reserve a falling treasury
    /// keeps; its per-turn remainder is already refused by the engine's quote
    /// whenever the payer's income cannot carry it (`Game::quoted_payment`).
    fn open_borders_affordable(g: &Game, pid: usize, deal: &QuickDeal) -> bool {
        let player = &g.players[pid];
        let reserve = if player.gold_per_turn < 0.0 {
            DEFICIT_RESERVE
        } else {
            0.0
        };
        player.gold - deal.offer.gold >= reserve
    }
}

#[cfg(test)]
mod tests;
