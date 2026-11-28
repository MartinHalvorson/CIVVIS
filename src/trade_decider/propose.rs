//! What a seat proposes, and to whom: each turn, the one deal with the
//! largest gain for this seat, by its own price ([`super::value`]), that the
//! other side might plausibly take, opened with room to concede.
//!
//! ★ PLAUSIBLE, NOT PRICED FOR THEM. A seat never reads another's valuation.
//! What it can read is what the other side can give (the deal screen's own
//! catalogue on our board: `tradeable_luxuries`, `tradeable_strategics`) and
//! what it can pay (`gold`, `gold_per_turn` on our board). A proposal asks
//! only for what they can give and offers or asks Gold only within what they
//! hold; whether it is worth it to them is theirs to answer, by accepting,
//! countering ([`super::answer`]) or refusing. A refused opening is followed,
//! after the cooldown, by a better one ([`OPENING_SHARES`]); terms refused
//! are never sent again inside [`super::answer::REFUSED_TERMS_TURNS`], and to
//! the person never at all.
//!
//! ★ THE PERSON IS NOT NAGGED. A person is asked at most once every
//! [`PERSON_PROPOSE_GAP_TURNS`] turns by a seat, never twice with the same
//! terms, and only with a deal that gains us at least [`PERSON_MIN_GAIN`].
//! And the table takes turns at it ([`person_slot_open`]): one seat at a
//! time may ask the person, in its own window of
//! [`PERSON_TABLE_WINDOW_TURNS`] turns, so five seats never ask the person
//! in the same turn or more than once a window between them.

use super::answer::{margin, REFUSED_TERMS_TURNS};
use super::board::{BoardItem, DealBoard, Terms};
use super::value::{Position, DEAL_TURNS};
use super::Item;
use crate::mirror::StateRival;
use std::collections::{BTreeMap, BTreeSet};

/// Turns between two proposals to one seat: the mod's own retry window
/// (`DealProposeRetryTurns`, default 5, which refuses a sooner one as
/// `deal_cooldown`).
pub const PROPOSE_COOLDOWN_TURNS: i64 = 5;
/// Turns between two proposals of one seat to the person.
pub const PERSON_PROPOSE_GAP_TURNS: i64 = 10;
/// GUESS. The least a proposal to the person must gain us: a person's
/// attention is worth more than a Gold or two.
pub const PERSON_MIN_GAIN: f64 = 25.0;
/// Turns in one seat's window to ask the person ([`person_slot_open`]): the
/// whole table asks the person at most once in this many turns.
pub const PERSON_TABLE_WINDOW_TURNS: i64 = 3;
/// Seats in the rotation when the board does not say how many players the
/// table has: a hotseat table seats at most six (`civ6_hotseat.py
/// --players`, 2 to 6), so its seats are players 0 to 5.
pub const PERSON_TABLE_SLOTS: i64 = 6;
/// GUESS. What share of an item's worth to us we offer for it, by how many
/// times that seat has already turned down an offer for the same item within
/// [`REFUSED_TERMS_TURNS`]: open low, then move toward the other side.
pub const OPENING_SHARES: [f64; 3] = [0.5, 0.7, 0.85];
/// GUESS. A spare luxury copy is offered for this share of what a first copy
/// of a new type is worth to us (a seat like ours would gain about as much).
pub const SPARE_ASK_SHARE: f64 = 0.8;
/// GUESS. Gold a spare strategic unit is asked at: half of what one we need
/// is worth to us.
pub const STRATEGIC_ASK_GOLD: f64 = 4.0;
/// Strategic units offered or asked for in one proposal.
pub const STRATEGIC_LOT: i64 = 10;

/// ★ WHOSE TURN IT IS TO ASK THE PERSON. Each seat of the table, by its
/// player id, owns one window of [`PERSON_TABLE_WINDOW_TURNS`] turns in a
/// rotation of `players` windows (`players` the table's player count, else
/// [`PERSON_TABLE_SLOTS`]), and asks the person only inside its own. Each
/// seat's decider decides alone, from its own board; the rotation needs
/// nothing but the turn and the seat's own id, so no two seats ever hold the
/// window at once. With [`PERSON_PROPOSE_GAP_TURNS`] longer than a window, a
/// seat asks at most once in it: the person is asked at most once every
/// [`PERSON_TABLE_WINDOW_TURNS`] turns by the whole table, never twice in one
/// turn, and by one seat at most once a rotation (18 turns at six players).
/// The person's own window is never used. An unknown seat (`me` below 0) is
/// never in its window.
pub fn person_slot_open(me: i64, players: i64, turn: i64) -> bool {
    let slots = if players >= 2 {
        players
    } else {
        PERSON_TABLE_SLOTS
    };
    me >= 0
        && turn >= 0
        && (turn / PERSON_TABLE_WINDOW_TURNS).rem_euclid(slots) == me.rem_euclid(slots)
}

/// One deal this seat proposes: to whom, the terms from our side, and our
/// own gain from them.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub to: usize,
    pub terms: Terms,
    pub gain: f64,
}

fn luxury(name: &str) -> BoardItem {
    BoardItem {
        kind: "RESOURCE".to_string(),
        name: Some(name.to_string()),
        amount: 1,
        duration: i64::from(DEAL_TURNS),
        class: Some("luxury".to_string()),
        ..BoardItem::default()
    }
}

fn strategic(name: &str, amount: i64) -> BoardItem {
    BoardItem {
        kind: "RESOURCE".to_string(),
        name: Some(name.to_string()),
        amount,
        class: Some("strategic".to_string()),
        ..BoardItem::default()
    }
}

/// Gold worth about `price` to the payer: lump when `lump_cap` carries it,
/// else the fewest Gold a turn (within `income_cap`) with the rest in lump;
/// `None` when the payer cannot carry it.
fn payment(price: f64, annuity: f64, lump_cap: i64, income_cap: i64) -> Option<Vec<BoardItem>> {
    if !price.is_finite() || price < 1.0 {
        return None;
    }
    for per_turn in 0..=income_cap.clamp(0, 1_000) {
        let lump = (price - per_turn as f64 * annuity).ceil().max(0.0) as i64;
        if lump <= lump_cap.max(0) {
            let mut items = Vec::new();
            if per_turn > 0 {
                items.push(BoardItem::gold_per_turn(per_turn));
            }
            if lump > 0 {
                items.push(BoardItem::gold(lump));
            }
            return Some(items);
        }
    }
    None
}

/// What a seat knows of its past proposals to one other seat.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct History {
    /// Turn of our last proposal to that seat (proposed, countered or not).
    pub last_proposed: Option<i64>,
    /// Terms (by [`Terms::key`]) that seat refused from us.
    pub refused: BTreeSet<String>,
    /// How many times that seat turned down an offer for each item we asked
    /// of it, by the item's name.
    pub turned_down: BTreeMap<String, usize>,
}

impl History {
    /// This seat's history with `with`, from the board's outcomes (and, for
    /// the person, every refusal of the game).
    pub fn from_board(board: &DealBoard, with: usize, turn: i64) -> History {
        let mut history = History::default();
        let person = board.is_person(with);
        for outcome in board.outcomes.iter().filter(|outcome| outcome.with == with) {
            history.last_proposed = history.last_proposed.max(Some(outcome.turn));
            let refused = outcome.outcome != "accepted";
            let recent = turn - outcome.turn < REFUSED_TERMS_TURNS;
            if refused && (recent || person) {
                if let Some(key) = outcome.terms.key() {
                    history.refused.insert(key);
                }
                for item in &outcome.terms.get {
                    if let Some(name) = &item.name {
                        *history.turned_down.entry(name.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
        if person {
            history.refused.extend(
                board
                    .refused_by_person
                    .iter()
                    .filter_map(|(_, _, terms)| terms.key()),
            );
        }
        for outgoing in board.outgoing.iter().filter(|deal| deal.to == with) {
            history.last_proposed = history.last_proposed.max(Some(outgoing.proposed_turn));
        }
        history
    }

    fn share(&self, item: &str) -> f64 {
        let refusals = self.turned_down.get(item).copied().unwrap_or(0);
        OPENING_SHARES[refusals.min(OPENING_SHARES.len() - 1)]
    }
}

/// Every deal this seat might open with `partner`, by its own price, each
/// asking only what `partner` can give and paying or asking Gold only within
/// what `partner` and we hold.
pub fn candidates(position: &Position, partner: &StateRival, history: &History) -> Vec<Terms> {
    let annuity = position.annuity(DEAL_TURNS).max(1.0);
    let our_gold = position.gold.floor() as i64;
    let our_income = position.gold_per_turn.floor().max(0.0) as i64;
    let their_gold = if partner.gold.is_finite() {
        partner.gold.floor() as i64
    } else {
        0
    };
    let their_income = if partner.gold_per_turn.is_finite() {
        partner.gold_per_turn.floor().max(0.0) as i64
    } else {
        0
    };
    let mut out = Vec::new();
    let spares: Vec<&String> = position
        .luxuries
        .iter()
        .filter(|(_, copies)| **copies >= 2)
        .map(|(name, _)| name)
        .collect();
    // Their luxuries we lack: swapped for a spare of ours, else bought.
    for name in partner.tradeable_luxuries.iter().flatten() {
        let wanted = luxury(name);
        let worth = position.value_of_getting(&Item::Luxury(name.clone()), Some(partner));
        if worth <= 0.0 {
            continue;
        }
        for spare in &spares {
            out.push(Terms {
                give: vec![luxury(spare)],
                get: vec![wanted.clone()],
            });
        }
        let offer = worth * history.share(name);
        if let Some(pay) = payment(offer, annuity, our_gold, our_income) {
            out.push(Terms {
                give: pay,
                get: vec![wanted.clone()],
            });
        }
    }
    // Our spare luxuries, sold for what a new type is worth to a seat like
    // ours.
    let new_type = position.luxury_types_value(1, 0);
    for spare in &spares {
        if let Some(ask) = payment(
            new_type * SPARE_ASK_SHARE,
            annuity,
            their_gold,
            their_income,
        ) {
            out.push(Terms {
                give: vec![luxury(spare)],
                get: ask,
            });
        }
    }
    // Strategics: our surplus sold, theirs bought where we need it.
    for name in position.strategics.keys() {
        let lot = strategic(name, STRATEGIC_LOT);
        let cost =
            position.cost_of_giving(&Item::Strategic(name.clone(), STRATEGIC_LOT), Some(partner));
        if cost.is_finite() && cost < STRATEGIC_ASK_GOLD * STRATEGIC_LOT as f64 {
            let ask = STRATEGIC_ASK_GOLD * STRATEGIC_LOT as f64;
            if let Some(pay) = payment(ask, annuity, their_gold, their_income) {
                out.push(Terms {
                    give: vec![lot],
                    get: pay,
                });
            }
        }
    }
    for (name, offered) in partner.tradeable_strategics.iter().flatten() {
        let amount = (*offered).min(STRATEGIC_LOT);
        if amount <= 0 {
            continue;
        }
        let worth =
            position.value_of_getting(&Item::Strategic(name.clone(), amount), Some(partner));
        if let Some(pay) = payment(worth * history.share(name), annuity, our_gold, our_income) {
            out.push(Terms {
                give: pay,
                get: vec![strategic(name, amount)],
            });
        }
    }
    out
}

/// ★ THIS TURN'S PROPOSAL: among the other seats at this table we may ask
/// (met, at peace, a `person` or `civvis` counterpart, nothing pending either
/// way, outside the cooldown), the candidate with the largest gain for us
/// that clears the margin (and [`PERSON_MIN_GAIN`] for the person, whose
/// candidates also rank that much lower, so a CIVVIS seat is asked first) and
/// was not refused before. The person is a candidate only when
/// `person_window` says this turn is this seat's to ask them
/// ([`person_slot_open`]). `None` when there is none: most turns.
pub fn propose(
    position: &Position,
    rivals: &[StateRival],
    board: &DealBoard,
    own_works: &BTreeMap<String, String>,
    person_window: bool,
) -> Option<Proposal> {
    let turn = i64::from(position.turn);
    let mut best: Option<(Proposal, f64)> = None;
    for partner in rivals {
        let kind = board.counterparts.get(&partner.player).map(String::as_str);
        if !matches!(kind, Some("person") | Some("civvis")) || partner.at_war {
            continue;
        }
        if board
            .incoming
            .iter()
            .any(|deal| deal.from == partner.player)
            || board.outgoing.iter().any(|deal| deal.to == partner.player)
        {
            continue;
        }
        let person = kind == Some("person");
        if person && !person_window {
            continue;
        }
        let history = History::from_board(board, partner.player, turn);
        let gap = if person {
            PERSON_PROPOSE_GAP_TURNS
        } else {
            PROPOSE_COOLDOWN_TURNS
        };
        if history
            .last_proposed
            .is_some_and(|last| last >= 0 && turn - last < gap)
        {
            continue;
        }
        for terms in candidates(position, partner, &history) {
            let Some(key) = terms.key() else { continue };
            if history.refused.contains(&key) {
                continue;
            }
            let Some(deal) = terms.deal(own_works) else {
                continue;
            };
            let gain = position.net(&deal, Some(partner));
            let floor = margin(position, &deal, Some(partner)).max(if person {
                PERSON_MIN_GAIN
            } else {
                0.0
            });
            if !gain.is_finite() || gain < floor {
                continue;
            }
            // The person is asked only when a seat's deal is clearly better:
            // ranked by the gain less what asking the person costs us.
            let rank = if person { gain - PERSON_MIN_GAIN } else { gain };
            if best.as_ref().is_none_or(|(_, current)| rank > *current) {
                best = Some((
                    Proposal {
                        to: partner.player,
                        terms,
                        gain,
                    },
                    rank,
                ));
            }
        }
    }
    best.map(|(proposal, _)| proposal)
}

#[cfg(test)]
mod tests;
