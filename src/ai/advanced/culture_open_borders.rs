//! `culture-keeps-every-border-open`: a Culture seat holds an Open Borders
//! grant from every rival it can, every turn, once its tourism is worth
//! multiplying.
//!
//! Tourism pressure is banked against each rival with the multiplier in force
//! on the turn it is generated (`Game::add_passive_tourism`), and a rival's
//! Open Borders grant to us is +25% of everything we generate against it
//! (`Game::international_tourism_multiplier`). A native Immortal culture
//! census (4p pangaea, three handicapped rival chairs, 16 games) found the
//! seat holding a grant from 0–2 of three rivals at most turn-25 marks.
//!
//! ★ THE PURCHASE COULD NOT HAPPEN AT ALL. A seat plans on its own player
//! view, and the view copies a met rival's public border enforcement
//! (`Player::borders_enforced`) but not its civics (`player_view.rs`). Every
//! Open Borders rule in the engine — the quick-deal quote, the war-eve quote
//! and both trade validations — asks `tree_effect(partner, "open_borders")`,
//! which reads those civics, so in the view no rival ever quotes passage: a
//! traced Immortal game logged zero Open Borders quotes in 250 turns with 400
//! Gold banked and 44 tourism a turn. The Culture objective's six-turn buy was
//! dead the same way; the grants the census saw came from elsewhere.
//!
//! With the gene on, each turn the seat's tourism is at least
//! [`OPEN_BORDERS_TOURISM_FLOOR`], every met rival whose borders the view
//! shows enforced is read as holding Early Empire — the civic that is this
//! ruleset's only source of enforcement (`data/tree_effects.json`) — for the
//! length of the purchase, and every Open Borders sale the ordinary Culture
//! filters allow is bought, the largest domestic-tourist count first: visitors
//! drawn from the bar holder also lower the bar. Each grant is priced as one
//! lump of Gold at the midpoint of both sides' valuations rather than taken
//! from `Game::quick_deals`, whose quote pays a fraction per turn that a
//! built-up empire's liquidity proxy refuses (see
//! [`AdvancedAi::lump_open_borders_quote`]). The view is restored before
//! anything else reads it, and the authoritative game revalidates every
//! harvested trade against the real civics. A purchase never spends into the
//! solvency reserve. Below the floor nothing changes; off, nothing changes.
use super::AdvancedAi;
use crate::game::{DealItems, Game, QuickDeal};
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
        let revealed = Self::reveal_enforcing_civics(g, pid);
        let mut closed = 0;
        let mut tried = BTreeSet::new();
        loop {
            let best = (0..g.players.len())
                .filter(|partner| Some(*partner) != excluded_partner && !tried.contains(partner))
                .filter_map(|partner| Self::lump_open_borders_quote(g, pid, partner))
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
        for rival in revealed {
            g.players[rival]
                .civics
                .remove(&crate::name!("early_empire"));
        }
        closed
    }

    /// Every met major whose borders the view shows enforced but whose civics
    /// it hides gains Early Empire in the view. Returns the rivals changed, so
    /// the caller can take it back. On an authoritative board the civic is
    /// already there and nothing changes.
    fn reveal_enforcing_civics(g: &mut Game, pid: usize) -> Vec<usize> {
        let early_empire = crate::name!("early_empire");
        let revealed: Vec<usize> = (0..g.players.len())
            .filter(|&rival| {
                let player = &g.players[rival];
                rival != pid
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && g.has_met(pid, rival)
                    && g.enforces_borders(rival)
                    && !player.civics.contains(&early_empire)
            })
            .collect();
        for &rival in &revealed {
            g.players[rival].civics.insert(early_empire);
        }
        revealed
    }

    /// A partner's Open Borders grant priced as one lump of Gold at the
    /// midpoint of the two sides' own valuations (`Game::trade_utilities`),
    /// each side keeping at least two. `Game::quick_deals` quotes the same
    /// midpoint but pays its fraction as 0.1 Gold a turn, and that remainder is
    /// checked against `empire_gold_per_turn` — a liquidity proxy of 4 Gold a
    /// city less every building's upkeep, which a built-up culture empire
    /// drives to zero while its real income is 100+ a turn. Every renewal
    /// after the first grant expired was refused that way in a traced game,
    /// although the same grant for a 25 Gold lump validated every turn.
    fn lump_open_borders_quote(g: &Game, pid: usize, partner: usize) -> Option<QuickDeal> {
        let other = &g.players[partner];
        if partner == pid
            || !other.alive
            || other.is_minor
            || other.is_barbarian
            || !g.has_met(pid, partner)
            || g.is_at_war(pid, partner)
            || g.has_open_borders(pid, partner)
        {
            return None;
        }
        let request = DealItems {
            open_borders: true,
            ..DealItems::default()
        };
        let (worth, negative_cost) =
            g.trade_utilities(pid, partner, &DealItems::default(), &request);
        let cost = -negative_cost;
        let price = ((worth + cost) / 2.0).ceil();
        if !price.is_finite() || worth - price < 2.0 || price - cost < 2.0 {
            return None;
        }
        Some(QuickDeal {
            partner,
            category: "agreement".to_string(),
            item: "open_borders".to_string(),
            direction: "buy".to_string(),
            offer: DealItems {
                gold: price,
                ..DealItems::default()
            },
            request,
            my_value: worth - price,
            partner_value: price - cost,
        })
    }

    /// The lump never spends into the reserve a falling treasury keeps.
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
