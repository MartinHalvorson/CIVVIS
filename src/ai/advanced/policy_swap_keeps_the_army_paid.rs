//! `policy-swap-keeps-the-army-paid` (opt-in): the policy pass does not give
//! up a card whose loss would take income to zero or below.
//!
//! **Why (live Deity Gran Colombia Domination `civvis-20261112T075931Z`,
//! 4-player Tiny Pangaea, Online).** Turn 192 opened a free policy window at
//! +40.8 Gold a turn with 420 Gold and a deck of Levée en Masse, Lightning
//! Warfare, Military First, Ecommerce, Economic Union, Five-Year Plan, Music
//! Censorship and Liberalism. The base deck review (`revise_policy_deck`,
//! forced on a free window) re-seated the whole deck before the strategic
//! pass ran, so Levée en Masse and Economic Union were already gone when
//! `strategic_policies` read the deck, and the pass's own swaps never named
//! them. The host read -29.8 a turn the next frame, 0 Gold from turn 193 and
//! -106 by turn 195; the unit bill went from 108 to 187 and the host
//! disbanded Modern Armor, Rocket Artillery, Spec Ops and a Destroyer.
//! Military power fell from 1,179 (t190) to 579 (t200) while a rival was 15
//! turns from a Science victory. `solvent-or-hold` guards a card only while
//! income is already not positive, so the swap that made it negative passed.
//!
//! **The rule.** Income is projected against the deck the turn began with
//! (`AdvancedAi::turn_start_policies`, the deck the host's income was
//! earned with): the turn's income, less what every card given up since then
//! paid, plus what every card slotted since then pays. A card pays its unit
//! maintenance discount on our paying army (`upkeep_card_value`) plus its
//! estimated Gold (`card_gold_estimate`; Economic Union at the pass's wartime
//! marginal when the pass prices it). Because the projection reads the whole
//! deck, it counts the base review's drops, a government change's pruning
//! and the pass's own earlier swaps together.
//!
//! - Before the swaps, while the projection is at or below zero, the cards
//!   the turn began with that the deck has given up are wanted back, most
//!   valuable first, until it is positive. They take only a free slot or an
//!   unwanted card's, as every wanted card does.
//! - A swap that would give up a card for one paying less keeps the card when
//!   the projected income after it is at or below zero.
//!
//! Journalled as "Keeping X while income is not positive", the detail
//! starting with the tag, for each card still held after the pass. Off,
//! nothing is computed.

use super::*;

/// What each card pays and the deck the turn began with: the projection the
/// policy pass keeps above zero.
pub(super) struct ArmyPay {
    /// The host's income this turn, earned with `turn_start`.
    gold_per_turn: f64,
    /// The deck the turn began with.
    turn_start: BTreeSet<Name>,
    /// Each priced card's upkeep relief and Gold, in Gold a turn.
    paid: BTreeMap<Name, (f64, f64)>,
}

impl ArmyPay {
    /// Gold a turn `card` adds to income: its upkeep relief plus its Gold.
    pub(super) fn income(&self, card: &str) -> f64 {
        self.paid
            .get(&Name::new(card))
            .map_or(0.0, |(upkeep, gold)| upkeep + gold)
    }

    /// Projected income with `deck` slotted: the turn's income, less what
    /// every card given up since the turn began paid, plus what every card
    /// slotted since then pays.
    pub(super) fn projected(&self, deck: &BTreeSet<Name>) -> f64 {
        let lost: f64 = self
            .turn_start
            .iter()
            .filter(|card| !deck.contains(*card))
            .map(|card| self.income(card.as_str()))
            .sum();
        let gained: f64 = deck
            .iter()
            .filter(|card| !self.turn_start.contains(*card))
            .map(|card| self.income(card.as_str()))
            .sum();
        self.gold_per_turn - lost + gained
    }

    /// The projected income if `incoming` took `current`'s slot in `deck`,
    /// when that keeps `current`: `current` pays more than `incoming` and
    /// the projection would be at or below zero.
    pub(super) fn keeps(
        &self,
        deck: &BTreeSet<Name>,
        current: &str,
        incoming: &str,
    ) -> Option<f64> {
        let (lost, gained) = (self.income(current), self.income(incoming));
        if lost <= gained + 1e-9 {
            return None;
        }
        let after = self.projected(deck) - lost + gained;
        (after <= 0.0).then_some(after)
    }

    /// The cards the turn began with that `deck` has given up and
    /// `slottable` allows, most valuable first, while the projection is at or
    /// below zero, each with the projection without it.
    pub(super) fn restores(
        &self,
        deck: &BTreeSet<Name>,
        slottable: impl Fn(&Name) -> bool,
    ) -> Vec<(Name, f64)> {
        let mut projected = self.projected(deck);
        let mut given_up: Vec<(Name, f64)> = self
            .turn_start
            .iter()
            .filter(|card| !deck.contains(*card) && slottable(card))
            .map(|card| (*card, self.income(card.as_str())))
            .filter(|(_, income)| *income > 0.0)
            .collect();
        given_up.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut back = Vec::new();
        for (card, income) in given_up {
            if projected > 0.0 {
                break;
            }
            back.push((card, projected));
            projected += income;
        }
        back
    }

    /// The journal detail for keeping `card` at a projection of `after`;
    /// `incoming` is the card that would have taken its slot, `None` when
    /// the deck review had already given it up.
    pub(super) fn detail(&self, card: &str, after: f64, incoming: Option<&str>) -> String {
        let (upkeep, gold) = self
            .paid
            .get(&Name::new(card))
            .copied()
            .unwrap_or((0.0, 0.0));
        let pays = match (upkeep > 0.0, gold > 0.0) {
            (true, true) => format!("{upkeep:.0} upkeep and {gold:.0} Gold"),
            (true, false) => format!("{upkeep:.0} upkeep"),
            _ => format!("{gold:.0} Gold"),
        };
        let why = match incoming {
            Some(incoming) => format!("{} would have taken its slot", plain(incoming)),
            None => "the deck review had already given its slot away".to_string(),
        };
        format!(
            "policy-swap-keeps-the-army-paid: {:+.1} Gold a turn would fall to {after:+.1} \
             without its {pays}; {why}",
            self.gold_per_turn
        )
    }
}

impl AdvancedAi {
    /// `policy-swap-keeps-the-army-paid`: the pass's projection, pricing the
    /// cards the turn began with, the cards held now and `wanted`.
    /// `wartime_union` is Economic Union's wartime marginal when the pass
    /// prices it. `None` with the gene off.
    pub(super) fn army_pay(
        &self,
        g: &Game,
        pid: usize,
        wanted: &[&str],
        wartime_union: Option<f64>,
    ) -> Option<ArmyPay> {
        if !self.policy_swap_keeps_the_army_paid {
            return None;
        }
        let mut paid = BTreeMap::new();
        let cards = self
            .turn_start_policies
            .iter()
            .chain(g.players[pid].policies.iter())
            .copied()
            .chain(wanted.iter().map(|&card| Name::new(card)));
        for card in cards {
            paid.entry(card).or_insert_with(|| {
                let upkeep = Self::upkeep_card_value(g, pid, card.as_str());
                let gold = match wartime_union {
                    Some(marginal) if card == "economic_union" => marginal,
                    _ => Self::card_gold_estimate(g, pid, card.as_str()),
                };
                (upkeep, gold)
            });
        }
        Some(ArmyPay {
            gold_per_turn: g.players[pid].gold_per_turn,
            turn_start: self.turn_start_policies.clone(),
            paid,
        })
    }
}

#[cfg(test)]
mod tests;
