//! How a seat answers a deal proposed to it: accept when the deal is worth it
//! by its own price ([`super::value`]), otherwise counter with the smallest
//! change that makes it worth it, otherwise reject.
//!
//! ★ THE OTHER SIDE'S PRICE IS NEVER READ. A counter asks for what would make
//! the deal worth it to US — more Gold (lump, or a turn when their treasury
//! cannot carry it), less of ours, or one of our items dropped — bounded by
//! what our board says the other side holds. Whether that is worth it to them
//! is theirs to answer on their own turn. Two CIVVIS seats running this code
//! from their own boards therefore close a deal exactly when both gain, and
//! stop after [`MAX_COUNTERS`] counters each, never sending the same refused
//! terms twice.

use super::board::{IncomingDeal, Terms};
use super::value::Position;
use super::Deal;
use crate::mirror::StateRival;
use std::collections::{BTreeMap, BTreeSet};

/// GUESS. The least a deal must gain us to be worth closing: a deal worth a
/// Gold or two is not worth a session, and the valuation is not that exact.
pub const ACCEPT_MARGIN_GOLD: f64 = 5.0;
/// GUESS. And a share of what changes hands, for the same reason on a large
/// deal.
pub const ACCEPT_MARGIN_SHARE: f64 = 0.05;
/// Counters a seat sends in one exchange before it rejects: an offer, two
/// counters each way and an answer bound any exchange to six deals.
pub const MAX_COUNTERS: u32 = 2;
/// Turns in which terms the other side refused are not sent to it again.
pub const REFUSED_TERMS_TURNS: i64 = 10;
/// A counter's Gold is rounded up to this many Gold.
pub const COUNTER_GOLD_STEP: i64 = 5;

/// What a seat does with a deal proposed to it.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Accept,
    /// These terms instead (this seat's side).
    Counter(Terms),
    /// No terms we could ask would work; the reason, for the journal.
    Reject(&'static str),
}

/// The least a deal must gain us: [`ACCEPT_MARGIN_GOLD`], or
/// [`ACCEPT_MARGIN_SHARE`] of the larger of what we receive and what we give.
pub fn margin(position: &Position, deal: &Deal, partner: Option<&StateRival>) -> f64 {
    let received = position.net(
        &Deal {
            give: Vec::new(),
            get: deal.get.clone(),
        },
        partner,
    );
    let given = -position.net(
        &Deal {
            give: deal.give.clone(),
            get: Vec::new(),
        },
        partner,
    );
    let moved = received.abs().max(given.abs());
    if moved.is_finite() {
        ACCEPT_MARGIN_GOLD.max(ACCEPT_MARGIN_SHARE * moved)
    } else {
        ACCEPT_MARGIN_GOLD
    }
}

/// What this seat knows about the exchange a deal belongs to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Exchange {
    /// Counters we have already sent in it.
    pub counters_sent: u32,
    /// Terms (by [`Terms::key`]) the other side refused from us within
    /// [`REFUSED_TERMS_TURNS`].
    pub refused: BTreeSet<String>,
}

/// Round a Gold amount up to [`COUNTER_GOLD_STEP`].
fn round_up(gold: f64) -> i64 {
    let steps = (gold / COUNTER_GOLD_STEP as f64).ceil().max(1.0);
    steps as i64 * COUNTER_GOLD_STEP
}

/// The terms worth sending instead of `terms`, smallest change first: every
/// candidate is worth at least the margin to us, and the one sent is the one
/// that gains us least, so it leaves the other side the most.
fn counter_terms(
    position: &Position,
    terms: &Terms,
    partner: Option<&StateRival>,
    own_works: &BTreeMap<String, String>,
    exchange: &Exchange,
) -> Option<Terms> {
    let annuity = position.annuity(super::value::DEAL_TURNS).max(1.0);
    // What our board says they hold; unknown reads as no limit, and the
    // host's own validation refuses what they cannot pay.
    let their_gold = partner
        .map(|rival| rival.gold)
        .filter(|gold| gold.is_finite())
        .map_or(i64::MAX, |gold| gold.floor() as i64);
    let their_income = partner
        .map(|rival| rival.gold_per_turn)
        .filter(|gold| gold.is_finite())
        .map_or(i64::MAX, |gold| gold.floor() as i64);
    let worth = |candidate: &Terms| -> Option<f64> {
        let deal = candidate.deal(own_works)?;
        let net = position.net(&deal, partner);
        (net.is_finite() && net >= margin(position, &deal, partner)).then_some(net)
    };
    // Each base: the terms as they are, and with one of our items dropped
    // while we still give something (a counter that gives nothing is a
    // demand, not a trade).
    let mut bases = vec![terms.clone()];
    for index in 0..terms.give.len() {
        let mut dropped = terms.clone();
        dropped.give.remove(index);
        if !dropped.give.is_empty() {
            bases.push(dropped);
        }
    }
    let mut candidates: Vec<Terms> = Vec::new();
    for base in bases {
        let Some(deal) = base.deal(own_works) else {
            continue;
        };
        let net = position.net(&deal, partner);
        let short = margin(position, &deal, partner) - net;
        if base != *terms {
            candidates.push(base.clone());
        }
        if !net.is_finite() || short <= 0.0 {
            continue;
        }
        // Less of our Gold.
        let ours = base.lump_gold_from_us();
        if ours > 0 {
            candidates.push(base.with_less_gold_from_us(round_up(short).min(ours)));
        }
        // More of theirs, lump if their treasury carries it, else a turn.
        let lump = round_up(short);
        if base.gold_from_them(false).saturating_add(lump) <= their_gold {
            candidates.push(base.with_more_gold_from_them(lump, false));
        }
        let per_turn = (short / annuity).ceil().max(1.0) as i64;
        if base.gold_from_them(true).saturating_add(per_turn) <= their_income {
            candidates.push(base.with_more_gold_from_them(per_turn, true));
        }
    }
    candidates
        .into_iter()
        .filter(|candidate| candidate != terms)
        // Gold for Gold is a transfer, never a trade both sides gain from.
        .filter(|candidate| {
            candidate
                .give
                .iter()
                .chain(&candidate.get)
                .any(|item| item.gold_kind().is_none())
        })
        .filter(|candidate| {
            candidate
                .key()
                .is_some_and(|key| !exchange.refused.contains(&key))
        })
        .filter_map(|candidate| worth(&candidate).map(|net| (net, candidate)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, candidate)| candidate)
}

/// ★ THE ANSWER to a deal proposed to this seat. `None` when the deal holds
/// an item the valuation cannot price (a city, a war, an alliance): it is
/// left as it stands, as before. Otherwise: a gift is accepted; a deal worth at least [`margin`] is accepted; else the smallest
/// counter worth it ([`counter_terms`]) while fewer than [`MAX_COUNTERS`] have
/// been sent; else rejected.
pub fn answer(
    position: &Position,
    deal: &IncomingDeal,
    partner: Option<&StateRival>,
    own_works: &BTreeMap<String, String>,
    exchange: &Exchange,
) -> Option<Answer> {
    let priced = deal.terms.deal(own_works)?;
    let net = position.net(&priced, partner);
    // A gift costs nothing to take.
    if deal.terms.give.is_empty() && net >= 0.0 {
        return Some(Answer::Accept);
    }
    if net.is_finite() && net >= margin(position, &priced, partner) {
        return Some(Answer::Accept);
    }
    if exchange.counters_sent >= MAX_COUNTERS {
        return Some(Answer::Reject("counters_spent"));
    }
    Some(
        match counter_terms(position, &deal.terms, partner, own_works, exchange) {
            Some(terms) => Answer::Counter(terms),
            None => Answer::Reject("no_terms_worth_it"),
        },
    )
}

/// The counters already sent in the exchange `deal` belongs to: from the
/// board's round count when it carries one (deals crossed both ways, from 1:
/// ours are every second one after the first), else the outcomes of our
/// proposals to that seat that it countered within [`REFUSED_TERMS_TURNS`].
pub fn counters_sent(deal: &IncomingDeal, outcomes: &[super::board::DealOutcome], turn: i64) -> u32 {
    if let Some(round) = deal.round {
        return u32::try_from((round - 1).max(0) / 2).unwrap_or(u32::MAX);
    }
    outcomes
        .iter()
        .filter(|outcome| {
            outcome.with == deal.from
                && outcome.outcome == "countered"
                && turn - outcome.turn < REFUSED_TERMS_TURNS
        })
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

/// The terms seat `with` refused from us within [`REFUSED_TERMS_TURNS`] of
/// `turn` (rejected, countered or let expire: anything but accepted), as
/// [`Terms::key`]s; outcomes the board carries without terms are not counted.
pub fn refused_terms(
    outcomes: &[super::board::DealOutcome],
    with: usize,
    turn: i64,
) -> BTreeSet<String> {
    outcomes
        .iter()
        .filter(|outcome| {
            outcome.with == with
                && matches!(
                    outcome.outcome.as_str(),
                    "rejected" | "countered" | "expired"
                )
                && turn - outcome.turn < REFUSED_TERMS_TURNS
                && !(outcome.terms.give.is_empty() && outcome.terms.get.is_empty())
        })
        .filter_map(|outcome| outcome.terms.key())
        .collect()
}

#[cfg(test)]
mod tests;
