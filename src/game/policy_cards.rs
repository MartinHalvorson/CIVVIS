//! Policy cards: the slots a government offers and the cards that may fill
//! them.
//!
//! A government's slots by kind (`gov_slots`), whether a deck fits them
//! (`policies_fit`), pruning the cards a changed government can no longer
//! hold (`prune_policies_to_government`), and the cards a player may slot
//! (`available_policies`). Reading what a slotted card does is in
//! `game/modifiers.rs`.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub fn gov_slots(&self, pid: usize) -> crate::rules::PolicySlots {
        // Anarchy has no slots of its own, and none a wonder can lend it.
        if self.in_anarchy(pid) {
            return Default::default();
        }
        let mut slots = match &self.players[pid].government {
            Some(g) => self
                .rules
                .governments
                .get(g)
                .map(|s| s.slots)
                .unwrap_or_default(),
            None => Default::default(),
        };
        let any = slots.military + slots.economic + slots.diplomatic + slots.wildcard;
        if any > 0 && self.has_ability(pid, "platos_republic") {
            slots.wildcard += 1; // Greece: Plato's Republic
        }
        slots.military += self.empire_wonder_effect(pid, "military_policy_slots") as i64;
        slots.economic += self.empire_wonder_effect(pid, "economic_policy_slots") as i64;
        slots.diplomatic += self.empire_wonder_effect(pid, "diplomatic_policy_slots") as i64;
        slots.wildcard += self.empire_wonder_effect(pid, "wildcard_policy_slots") as i64;
        if let Some(government) = self.players[pid].government.as_deref() {
            if self.congress_effect_active("world_ideology", "A", government) {
                slots.wildcard += 1;
            } else if self.congress_effect_active("world_ideology", "B", government) {
                slots.wildcard = slots.wildcard.saturating_sub(1);
            }
        }
        slots
    }

    /// Can this set of cards be seated in the player's slots? Typed cards
    /// fill their own slot type first; overflow and wildcard cards need
    /// wildcard slots (Civ 6 rule).
    /// Drop slotted cards until the deck fits `pid`'s CURRENT government.
    ///
    /// ⚠⚠ A GOVERNMENT CHANGE INVALIDATES A DECK AND NOTHING RE-CHECKED IT.
    /// `policies_fit` is enforced when a card is SLOTTED and never again, so a
    /// deck legal under the old constitution survived into the new one. On the
    /// live bridge the mirror sets the government from the host export, and
    /// CIVVIS then re-sent that stale deck EVERY TURN and the host refused it
    /// every turn: run `civvis-20260805T192809Z` switched Theocracy ->
    /// Merchant Republic on t155 and was still asking for the Theocracy shape
    /// (ECONOMIC x3, MILITARY x2, DIPLOMATIC x1) at t160-163, while the host sat
    /// at FIVE cards in SIX slots.
    ///
    /// Cards are dropped in reverse slot order so the deterministic `BTreeSet`
    /// iteration cannot depend on insertion history.
    pub(crate) fn prune_policies_to_government(&mut self, pid: usize) -> usize {
        let mut dropped = 0;
        loop {
            let cards = self.players[pid].policies.clone();
            if cards.is_empty() || self.policies_fit(pid, &cards) {
                return dropped;
            }
            let Some(victim) = cards.iter().next_back().copied() else {
                return dropped;
            };
            self.players[pid].policies.remove(&victim);
            dropped += 1;
        }
    }

    pub(super) fn policies_fit(&self, pid: usize, cards: &BTreeSet<Name>) -> bool {
        let slots = self.gov_slots(pid);
        let (mut m, mut e, mut d, mut w) = (0i64, 0i64, 0i64, 0i64);
        for c in cards {
            match self.rules.policies.get(c).map(|p| p.slot.as_str()) {
                Some("military") => m += 1,
                Some("economic") => e += 1,
                Some("diplomatic") => d += 1,
                _ => w += 1,
            }
        }
        let overflow = (m - slots.military).max(0)
            + (e - slots.economic).max(0)
            + (d - slots.diplomatic).max(0);
        overflow + w <= slots.wildcard
    }

    /// Cards the player has unlocked and may slot (not yet slotted, civic
    /// met, not obsoleted by an unlocked successor card).
    pub fn available_policies(&self, pid: usize) -> Vec<Name> {
        let p = &self.players[pid];
        if p.is_minor {
            return vec![];
        }
        // A card is retired by ANY unlocked successor, and one successor can retire
        // several — Public Works kills both Bastions and Serfdom — so this flattens
        // rather than taking one name per card.
        let obsolete: BTreeSet<Name> = self
            .rules
            .policies
            .values()
            .filter(|s| {
                s.civic
                    .as_ref()
                    .map(|c| p.civics.contains(c))
                    .unwrap_or(true)
            })
            .flat_map(|s| s.replaces.iter().copied())
            .collect();
        self.rules
            .policies
            .iter()
            .filter(|(name, s)| {
                !p.policies.contains(*name)
                    && !obsolete.contains(*name)
                    // A card the HOST ruleset has retired, learned from its own
                    // refusals. Empty in an ordinary game; see `blocked_policies`.
                    && !self.blocked_policies.contains(*name)
                    && self.host_policy_choices.get(&pid).is_none_or(|choices| choices.contains(*name))
                    && s.offered(&p.age, self.world_era)
                    && s.civic
                        .as_ref()
                        .map(|c| p.civics.contains(c))
                        .unwrap_or(true)
            })
            .map(|(name, _)| *name)
            .collect()
    }
}
