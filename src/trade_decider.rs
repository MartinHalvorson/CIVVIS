//! The trade decider: what each CIVVIS seat would pay for, or take for, every
//! thing it can trade, priced from its own board alone.
//!
//! ★ WHY A SEAT NEEDS ITS OWN PRICE. Against Firaxis's leaders the live seat
//! never had to know what a deal was worth to itself: the mod put the items in
//! a working deal and asked the leader's own deal AI (`DealProposalAction.
//! EQUALIZE`), and the decider only carried a floor or a ceiling
//! (`sale_floor`, `border_buy_ceiling` in `civvis_orders`). At a hotseat table
//! every major is a human player — the person, or another CIVVIS seat — and
//! there is nobody to ask: on 2026-11-27 (`hotseat-20261128T031456Z`) a
//! Great Work purchase from another CIVVIS seat ended `no_deal_response` and
//! every delegation `delegation_not_offered`. So each seat prices a deal by
//! its own position, accepts a deal only when it gains by that price, and
//! finds the other side's price by offer and counter-offer. No seat reads
//! another's valuation: the engine's own `Game::trade_utilities` computes
//! both sides at once, which a real table cannot.
//!
//! [`value`] is the valuation: one common unit, Gold now, for every item, from
//! this seat's board at this moment. [`board`] reads the hotseat deal board
//! and writes the `deal` order's items; [`answer`] accepts, counters or
//! rejects a deal proposed to the seat; [`propose`] finds the deal the seat
//! gains most from that another seat might take.

pub mod answer;
pub mod board;
pub mod propose;
pub mod value;

#[cfg(test)]
mod fixtures;

/// One thing that can change hands in a deal, as one side sees it.
///
/// Whether it is given or received is the side of the [`Deal`] it sits on.
/// The host's deal screen carries every one of these
/// (`Base/Assets/UI/DiplomacyDealView.lua`).
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// Lump Gold, paid when the deal closes.
    Gold(i64),
    /// Gold every turn for [`value::DEAL_TURNS`] turns.
    GoldPerTurn(i64),
    /// One copy of a luxury, by host type name (`RESOURCE_SILK`), for
    /// [`value::DEAL_TURNS`] turns.
    Luxury(String),
    /// A lump of a strategic stockpile: host type name and amount.
    Strategic(String, i64),
    /// Lump Diplomatic Favor.
    Favor(i64),
    /// The giver's Open Borders to the receiver, for [`value::DEAL_TURNS`]
    /// turns.
    OpenBorders,
    /// One Great Work, by its host object class (`GREATWORKOBJECT_WRITING`).
    GreatWork(String),
    /// A Diplomatic Delegation. Given: we host the other side's delegation.
    /// Received: ours is accepted at their court (we pay the host's fee).
    Delegation,
    /// A Resident Embassy, the same way round as [`Item::Delegation`].
    Embassy,
    /// A Declaration of Friendship. It binds both sides alike, so it means the
    /// same on either side of a deal.
    Friendship,
}

/// A deal from one seat's side: what it hands over and what it receives.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Deal {
    pub give: Vec<Item>,
    pub get: Vec<Item>,
}

impl Deal {
    /// The same deal from the other side of the table.
    pub fn mirrored(&self) -> Deal {
        Deal {
            give: self.get.clone(),
            get: self.give.clone(),
        }
    }

    /// Whether nothing changes hands.
    pub fn is_empty(&self) -> bool {
        self.give.is_empty() && self.get.is_empty()
    }
}
