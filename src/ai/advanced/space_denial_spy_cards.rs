//! `space-denial-slots-the-spy-cards` (opt-in): a Culture or Domination seat
//! whose spy network is answering a space race slots the two spy cards.
//!
//! Live Deity Kristina, 11-10 (10 rival Science losses, t168-198): the denial
//! genes fire on the culture lane (`racer-keeps-the-spies`,
//! `science-denial-trains-spies`, `science-denial-every-pad`), but the
//! network is thin and its Disrupts mostly fail. One to three Spies held at
//! t150 against a capacity of two to five; training ran at 15-50 Production
//! a turn for a 225-Production Spy; 8 of 32 Disrupt Rocketry runs against
//! the eventual winner pillaged the pad (host odds 0.25-0.37 at level one).
//! Machiavellianism (Diplomatic Service, Diplomatic slot: +50% Production
//! toward Spies, offensive operations 25% shorter) was on the menu on every
//! turn from t120 in all nine games read and slotted on none; Cryptography
//! (Cold War: our offensive Spies +1 level, enemy Spies -2 in our lands)
//! was slotted on 0-14 of 49-79 turns. The lane decks (`policies.rs`) do not
//! name Machiavellianism at all and rank Cryptography near their tail.
//!
//! While [`AdvancedAi::denial_spy_racers`] names a rival (a decisive space
//! racer, or under `space-denial-spies-from-rocketry` any rival holding
//! Rocketry) and the seat aims at Culture or Domination, this puts
//! Machiavellianism then Cryptography in the portfolio just ahead of the
//! first ordinary Diplomatic card, behind every emergency, defence, loyalty
//! and Nobel card and every card that pays Culture or Tourism (Collective
//! Activism), and lets them take a Diplomatic slot from an ordinary wanted
//! card ranked after them (Gunboat Diplomacy, Merchant Confederation). The
//! Science and Religion seats keep their decks unchanged. Exact no-op while
//! the gene is off.

use super::*;

/// The spy cards, in the order the portfolio wants them.
pub(super) const DENIAL_SPY_CARDS: [&str; 2] = ["machiavellianism", "cryptography"];

impl AdvancedAi {
    /// The spy cards this seat's denial wants slotted now, in order: those
    /// the host offers or that are already slotted. Empty with the gene off,
    /// off a Culture or Domination aim, with the Science Victory off, or
    /// while no rival is a racer the spy network answers.
    pub(super) fn denial_spy_cards(&self, g: &Game, pid: usize) -> Vec<&'static str> {
        if !self.space_denial_slots_the_spy_cards
            || !g.victory_conditions.science
            || !matches!(
                self.active_victory_target(g),
                Some(VictoryTarget::Culture | VictoryTarget::Domination)
            )
            || self.denial_spy_racers(g, pid).is_empty()
        {
            return Vec::new();
        }
        let available = g.available_policies(pid);
        DENIAL_SPY_CARDS
            .into_iter()
            .filter(|card| {
                let name = Name::new(*card);
                g.rules.policies.contains_key(&name)
                    && (g.players[pid].policies.contains(&name) || available.contains(&name))
            })
            .collect()
    }

    /// Whether `card` pays Culture or Tourism: a lane card the spy cards
    /// never pass (Collective Activism, +5% Culture per Suzerain).
    pub(super) fn denial_spy_card_spares(g: &Game, card: &str) -> bool {
        g.rules.policies.get(card).is_some_and(|spec| {
            spec.effects
                .keys()
                .any(|key| key.contains("culture") || key.contains("tourism"))
        })
    }

    /// Puts `cards` into `desired` just ahead of the first Diplomatic card
    /// neither in `protected` nor paying Culture or Tourism, or at the end
    /// when there is none.
    pub(super) fn place_denial_spy_cards<'a>(
        g: &Game,
        desired: &mut Vec<&'a str>,
        cards: &[&'a str],
        protected: &dyn Fn(&str) -> bool,
    ) {
        if cards.is_empty() {
            return;
        }
        desired.retain(|card| !cards.contains(card));
        let at = desired
            .iter()
            .position(|card| {
                !protected(card)
                    && !Self::denial_spy_card_spares(g, card)
                    && g.rules
                        .policies
                        .get(*card)
                        .is_some_and(|spec| spec.slot == "diplomatic")
            })
            .unwrap_or(desired.len());
        desired.splice(at..at, cards.iter().copied());
    }

    /// Whether a denial spy card `card`, at `rank` in `desired`, may take
    /// `current`'s slot: a Diplomatic card the portfolio ranks after it (or
    /// does not want), never a protected one, one paying Culture or Tourism,
    /// or another spy card.
    pub(super) fn denial_spy_card_may_displace(
        g: &Game,
        desired: &[&str],
        rank: usize,
        card: &str,
        current: &str,
        protected: &dyn Fn(&str) -> bool,
    ) -> bool {
        DENIAL_SPY_CARDS.contains(&card)
            && !DENIAL_SPY_CARDS.contains(&current)
            && !protected(current)
            && !Self::denial_spy_card_spares(g, current)
            && g.rules
                .policies
                .get(current)
                .is_some_and(|spec| spec.slot == "diplomatic")
            && desired
                .iter()
                .position(|wanted| *wanted == current)
                .is_none_or(|theirs| theirs > rank)
    }
}

#[cfg(test)]
mod tests;
