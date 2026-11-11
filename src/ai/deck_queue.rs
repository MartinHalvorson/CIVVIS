//! `deck-follows-the-queue`: the policy deck re-reviews on the turn the
//! cities' queues stop matching its production cards.
//!
//! From the operator's heuristic (2026-11-02):
//!
//! > it's often most efficient to produce units with 50% prod boost, to buy
//! > some buildings (as most can't be boosted production). settlers are good
//! > to boost with 50% prod. ships with +100% prod boost. things that can
//! > boost should often be produced with prod.
//!
//! ## What the deck did
//!
//! [`revise_policy_deck`](super::revise_policy_deck) prices every card by the
//! empire reading with and without it, and since 2026-07 that reading includes
//! [`Game::item_prod_mult`] toward each city's queue head, so a production
//! card is worth exactly the production it adds to what is being built. But a
//! full deck is re-examined only every `POLICY_REVIEW_EVERY` (8) turns, and a
//! production card is valuable for the few turns a matching item sits at the
//! head. A card slotted while a Settler was queued stayed after it finished;
//! a Limes that would have doubled the Walls going up never got a review.
//!
//! Live, 69 games 2026-11-01T22:49Z..11-02T23:40Z (13,305 turn states, mostly
//! Emperor and Immortal):
//!
//! - Slotted production cards boosted no queue head on most of their turns:
//!   Feudal Contract 84% of 1,812 slot-turns, Military First 83%, Maritime
//!   Industries 74%, Agoge 72%, Ilkum 59%, Limes 57%, Colonization 45%.
//! - On 8,676 city-turns a city built an item that an unlocked, unslotted
//!   card would have boosted. That is ~89,000 Production, ~1,290 a game:
//!   Limes 262, International Waters 240, Grande Armée 208, Lightning
//!   Warfare 133, Public Works 83, Colonization 82 a game.
//! - Live policy swaps take effect on any turn: 3,099 of 3,185 deck readbacks
//!   matched the requested deck.
//!
//! ## The gene
//!
//! The signature is the set of *queue cards*, held or unlocked, that would
//! add at least [`DECK_QUEUE_MIN_BOOST`] Production a turn to the current
//! queue heads. A queue card is one whose every effect is an item-production
//! bonus `item_prod_mult` reads ([`QUEUE_CARD_EFFECTS`]): its value lives
//! entirely in the queue, so a review the queue does not ask for cannot price
//! it differently. When the signature differs from last turn's, the review
//! runs off its cadence. The review itself, its scoring and its hysteresis are
//! unchanged; only when it runs changes.
//!
//! The signature does not read the deck, so the Advanced strategic layer that
//! runs after the review cannot make it flap: a card it evicts stays out until
//! the queue changes again or the ordinary cadence comes round.
//!
//! The base review runs before this turn's production choices, so a head
//! chosen this turn is first read the next turn. That one-turn lag also covers
//! a resumed head restored by the production reconciliation.
//!
//! With the gene off nothing is computed and the review runs on its cadence.
use super::BasicAi;
use crate::game::Game;
use crate::name::Name;
use std::collections::BTreeSet;

/// The per-item production effects `Game::item_prod_mult` reads. A card all
/// of whose effects are in this list is a queue card: the production it adds
/// to the items being built is the whole of what it does.
pub(super) const QUEUE_CARD_EFFECTS: [&str; 18] = [
    "infantry_production_pct",
    "cavalry_production_pct",
    "naval_production_pct",
    "air_production_pct",
    "support_production_pct",
    "carrier_production_pct",
    "gdr_production_pct",
    "spy_production_pct",
    "builder_production_pct",
    "settler_production_pct",
    "wall_production_pct",
    "military_port_production_pct",
    "wonder_production_pct",
    "classical_wonder_production_pct",
    "renaissance_wonder_production_pct",
    "project_production_pct",
    "space_project_production_pct",
    "nuclear_project_production_pct",
];

/// The Production a turn a queue card must add across the empire's queue
/// heads to enter the signature. Below it a city of three or four Production
/// building a Warrior under Agoge would flip the signature, and a review,
/// every time its queue rolled over.
pub(super) const DECK_QUEUE_MIN_BOOST: f64 = 1.0;

/// Whether every effect of `card` is an item-production bonus.
pub(super) fn is_queue_card(g: &Game, card: Name) -> bool {
    g.rules.policies.get_interned(card).is_some_and(|spec| {
        !spec.dark_age
            && !spec.effects.is_empty()
            && spec
                .effects
                .keys()
                .all(|effect| QUEUE_CARD_EFFECTS.contains(&effect.as_str()))
    })
}

/// The queue cards, held or unlocked, that add at least
/// [`DECK_QUEUE_MIN_BOOST`] Production a turn to `pid`'s current queue heads.
///
/// Each card is toggled on the seat's deck and put back exactly as it was,
/// the same counterfactual the deck review itself runs.
pub(super) fn queue_card_signature(g: &mut Game, pid: usize) -> BTreeSet<Name> {
    let held: Vec<Name> = g.players[pid].policies.iter().copied().collect();
    let mut cards: Vec<Name> = g.available_policies(pid);
    cards.extend(held.iter().copied());
    cards.sort();
    cards.dedup();
    cards.retain(|card| is_queue_card(g, *card));
    if cards.is_empty() {
        return BTreeSet::new();
    }
    let cities: Vec<u32> = g
        .player_city_ids(pid)
        .into_iter()
        .filter(|cid| g.cities.get(cid).is_some_and(|city| !city.queue.is_empty()))
        .collect();
    let multipliers = |g: &Game| -> Vec<f64> {
        cities
            .iter()
            .map(|cid| g.item_prod_mult(pid, *cid, g.cities[cid].queue.first()))
            .collect()
    };
    let base = multipliers(g);
    let mut production: Vec<Option<f64>> = vec![None; cities.len()];
    let mut signature = BTreeSet::new();
    for card in cards {
        let incumbent = g.players[pid].policies.contains(&card);
        // `board_written` on both sides of each what-if, as in
        // `policy_deck::policy_card_score`: nothing read under the rearranged
        // deck may answer for the real one.
        let (with, without) = if incumbent {
            g.players[pid].policies.remove(&card);
            g.board_written();
            let without = multipliers(g);
            g.players[pid].policies.insert(card);
            g.board_written();
            (base.clone(), without)
        } else {
            g.players[pid].policies.insert(card);
            g.board_written();
            let with = multipliers(g);
            g.players[pid].policies.remove(&card);
            g.board_written();
            (with, base.clone())
        };
        let mut boost = 0.0;
        for (index, cid) in cities.iter().enumerate() {
            let gain = with[index] - without[index];
            if gain <= 0.0 {
                continue;
            }
            let yield_production =
                *production[index].get_or_insert_with(|| g.city_yields(*cid).production.max(0.0));
            boost += gain * yield_production;
        }
        if boost + f64::EPSILON >= DECK_QUEUE_MIN_BOOST {
            signature.insert(card);
        }
    }
    signature
}

impl BasicAi {
    /// `deck-follows-the-queue`: whether the deck review should run off its
    /// cadence this turn because the queue cards worth slotting changed.
    /// Records this turn's signature. Always false, and computes nothing,
    /// while the gene is off.
    pub(super) fn deck_queue_review_due(&self, g: &mut Game, pid: usize) -> bool {
        if !self.deck_follows_the_queue || self.minor || self.barb {
            return false;
        }
        let now = queue_card_signature(g, pid);
        let previous = self
            .deck_queue_signature
            .borrow_mut()
            .insert(pid, now.clone());
        match previous {
            Some(previous) => previous != now,
            None => !now.is_empty(),
        }
    }
}
