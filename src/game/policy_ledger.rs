//! What each slotted policy card is worth to its empire right now.
//!
//! Civilization VI prints a card's text and never its value: "+100% Campus
//! adjacency bonuses" says nothing about whether this empire's Campuses earn
//! two Science or forty from it. Players install a mod for the missing number.
//! Here it is the engine's own arithmetic, read the way the deck chooser in
//! `ai.rs` (`empire_reading`) already reads it: measure the empire with and
//! without cards. Nothing names an effect key, so every card the engine
//! implements is covered and a card it does not implement reads zero instead
//! of quoting its own text back.
//!
//! ★ THE CARDS ADD UP TO THE DECK. Operator 2026-10-15: "math for net policy
//! effect appears wrong maybe. sometimes appear not to sum correctly". Each
//! card used to be priced alone -- the deck with it against the deck without
//! it -- and cards are not independent: a percentage card multiplies a flat
//! one, and amenities move yields only across a mood threshold. Live t84
//! (Civil Prestige + Retainers, both amenity cards) read Faith +0.2 and +0.2
//! against a deck worth +0.7, Production +1.9 and +1.2 against +2.7. So each
//! card is now its SHARE of the deck: its gain averaged over every order the
//! cards could be added in (the Shapley value), which always sums to the
//! deck's whole effect and splits whatever two cards earn together evenly
//! between them.

use super::*;

/// One whole-empire reading, in the units the top bar and city screens print.
///
/// Every field is a per-turn rate except the three city totals (`amenities`,
/// `housing`) and `strength`, the army's mean combat strength — a combat card
/// moves no yield, and the units are the only place it shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EmpireReading {
    pub food: f64,
    /// Production toward what each city is actually building, so the
    /// `*_production_pct` cards (Agoge, Colonization, Ilkum...) count where
    /// they apply and nowhere else.
    pub production: f64,
    /// Net of unit and building upkeep, so Conscription's maintenance
    /// discount reads as the Gold it saves.
    pub gold: f64,
    pub science: f64,
    pub culture: f64,
    pub faith: f64,
    pub amenities: f64,
    pub housing: f64,
    pub loyalty: f64,
    pub great_people: f64,
    pub tourism: f64,
    pub influence: f64,
    pub strength: f64,
}

impl EmpireReading {
    /// The measures in display order, with the key a reader asks them by.
    pub fn fields(&self) -> [(&'static str, f64); 13] {
        [
            ("food", self.food),
            ("production", self.production),
            ("gold", self.gold),
            ("science", self.science),
            ("culture", self.culture),
            ("faith", self.faith),
            ("amenities", self.amenities),
            ("housing", self.housing),
            ("loyalty", self.loyalty),
            ("great_people", self.great_people),
            ("tourism", self.tourism),
            ("influence", self.influence),
            ("strength", self.strength),
        ]
    }

    /// Field by field, so a share can be built from weighted gains.
    fn combine(&self, other: &EmpireReading, f: impl Fn(f64, f64) -> f64) -> EmpireReading {
        EmpireReading {
            food: f(self.food, other.food),
            production: f(self.production, other.production),
            gold: f(self.gold, other.gold),
            science: f(self.science, other.science),
            culture: f(self.culture, other.culture),
            faith: f(self.faith, other.faith),
            amenities: f(self.amenities, other.amenities),
            housing: f(self.housing, other.housing),
            loyalty: f(self.loyalty, other.loyalty),
            great_people: f(self.great_people, other.great_people),
            tourism: f(self.tourism, other.tourism),
            influence: f(self.influence, other.influence),
            strength: f(self.strength, other.strength),
        }
    }

    pub fn minus(&self, other: &EmpireReading) -> EmpireReading {
        EmpireReading {
            food: self.food - other.food,
            production: self.production - other.production,
            gold: self.gold - other.gold,
            science: self.science - other.science,
            culture: self.culture - other.culture,
            faith: self.faith - other.faith,
            amenities: self.amenities - other.amenities,
            housing: self.housing - other.housing,
            loyalty: self.loyalty - other.loyalty,
            great_people: self.great_people - other.great_people,
            tourism: self.tourism - other.tourism,
            influence: self.influence - other.influence,
            strength: self.strength - other.strength,
        }
    }
}

/// One slotted card and its share of what the deck is worth.
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyWorth {
    pub policy: Name,
    pub worth: EmpireReading,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PolicyLedger {
    /// Anarchy suspends every card, so each is worth nothing until the
    /// waiting government takes power.
    pub anarchy: bool,
    /// The empire as it stands, with the whole deck in.
    pub with: EmpireReading,
    /// The whole deck against no deck at all, and the sum of the cards'
    /// shares.
    pub net: EmpireReading,
    pub cards: Vec<PolicyWorth>,
}

impl Game {
    /// The empire `pid` as it stands, read for the policy ledger.
    pub fn empire_reading(&self, pid: usize) -> EmpireReading {
        let mut reading = EmpireReading::default();
        let Some(player) = self.players.get(pid) else {
            return reading;
        };
        if player.is_minor || player.is_barbarian {
            return reading;
        }
        // One memo scope over the sweep, as `ai::empire_reading` keeps: the
        // city derivations share their answers, and the scope is dropped
        // before the caller changes the deck.
        let _memo = self.query_memo();
        for cid in self.player_city_ids(pid) {
            // The model, not the host-corrected figure: on the live mirror a
            // host correction is a constant that would be subtracted from
            // both sides of the counterfactual anyway, and where the host
            // figure stands in for the model the card would read as nothing.
            let yields = self.city_yields_model(cid);
            let city = &self.cities[&cid];
            reading.food += yields.food;
            reading.production +=
                yields.production * self.item_prod_mult(pid, cid, city.queue.first());
            reading.gold += yields.gold;
            reading.science += yields.science;
            reading.culture += yields.culture;
            reading.faith += yields.faith;
            reading.amenities += self.city_amenities(city) as f64;
            reading.housing += self.city_housing(city);
            // The model's rate: the host's own figure, where the mirror has
            // one, replaces it outright and would not move with a card.
            reading.loyalty += self.loyalty_change_for_city(city.owner, cid).delta;
        }
        let extras = self.player_yield_extras(pid);
        reading.food += extras.food;
        reading.production += extras.production;
        reading.gold += extras.gold;
        reading.science += extras.science;
        reading.culture += extras.culture;
        reading.faith += extras.faith;
        reading.gold -= self.modeled_upkeep(pid);
        reading.great_people = self.great_person_points_per_turn(pid).values().sum();
        reading.tourism = self.tourism_per_turn_model(pid);
        // Only a card's share: the government's and the buildings' influence
        // are the same either side of any card and cancel in every difference.
        reading.influence = self.policy_effect(pid, "influence_per_turn");
        let mut fighters = 0.0;
        let mut strength = 0.0;
        for uid in self.player_unit_ids(pid) {
            if let Some(unit) = self.units.get(&uid) {
                let attack = self.unit_strength(unit, false);
                if attack > 0.0 {
                    fighters += 1.0;
                    strength += attack;
                }
            }
        }
        if fighters > 0.0 {
            reading.strength = strength / fighters;
        }
        reading
    }

    /// `pid`'s Gold upkeep as the board prices it, unit by unit and building
    /// by building.
    ///
    /// Not `unit_gold_maintenance`: on the live mirror that is the host
    /// treasury's own total, discounts already taken, so Conscription read as
    /// worth nothing there. Each unit's own host bill is still used where the
    /// host sent one — that figure comes before the discount
    /// (`unit_gold_maintenance_cost`), so the card moves it.
    fn modeled_upkeep(&self, pid: usize) -> f64 {
        let units: f64 = self
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| self.units.get(&uid))
            .map(|unit| self.unit_gold_maintenance_cost(pid, unit))
            .sum();
        let spies = self.spies.values().filter(|spy| spy.owner == pid).count() as f64
            * self.rules.units["spy"].maintenance;
        let infrastructure: f64 = self
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| self.city_infrastructure_gold_maintenance(city))
            .sum();
        units + spies + infrastructure
    }

    /// What each of `pid`'s slotted cards is worth, and the deck as a whole.
    ///
    /// Read on a disposable branch, so the game itself is never touched.
    pub fn policy_ledger(&self, pid: usize) -> PolicyLedger {
        let Some(player) = self.players.get(pid) else {
            return PolicyLedger::default();
        };
        if player.is_minor || player.is_barbarian {
            return PolicyLedger::default();
        }
        self.speculative_clone().into_policy_ledger(pid)
    }

    /// [`Self::policy_ledger`] on a branch the caller has already made — the
    /// server clones under its lock and reads off it.
    ///
    /// Every sub-deck is read once (2^n readings, ~1 ms each on a seven-city
    /// empire, remembered by the server per turn), and each card's share is
    /// its gain over the sub-decks without it, weighted by how many orders of
    /// adding the cards pass through that sub-deck. Past
    /// [`EXACT_SHARE_CARDS`] cards the shares are the gains in deck order
    /// instead: still the deck's whole effect in sum, no longer even-handed.
    pub fn into_policy_ledger(self, pid: usize) -> PolicyLedger {
        self.ledger_sharing_exactly_up_to(pid, EXACT_SHARE_CARDS)
    }

    fn ledger_sharing_exactly_up_to(mut self, pid: usize, exact_cards: usize) -> PolicyLedger {
        if self
            .players
            .get(pid)
            .is_none_or(|player| player.is_minor || player.is_barbarian)
        {
            return PolicyLedger::default();
        }
        let held = self.players[pid].policies.clone();
        let deck: Vec<Name> = held.iter().copied().collect();
        let n = deck.len();
        let mut shares = vec![EmpireReading::default(); n];
        let with;
        let bare;
        if n <= exact_cards {
            let readings: Vec<EmpireReading> = (0..1usize << n)
                .map(|mask| self.reading_with_cards(pid, &deck, mask))
                .collect();
            // A sub-deck of `s` cards precedes card i in s!(n-1-s)! of the n!
            // orders.
            let factorial = |k: usize| (1..=k).map(|v| v as f64).product::<f64>();
            let weight: Vec<f64> = (0..n)
                .map(|s| factorial(s) * factorial(n - 1 - s) / factorial(n))
                .collect();
            for (i, share) in shares.iter_mut().enumerate() {
                for mask in (0..1usize << n).filter(|mask| mask >> i & 1 == 0) {
                    let w = weight[mask.count_ones() as usize];
                    let gain = readings[mask | 1 << i].minus(&readings[mask]);
                    *share = share.combine(&gain, |a, b| a + w * b);
                }
            }
            with = readings[(1 << n) - 1];
            bare = readings[0];
        } else {
            let mut before = self.reading_with_cards(pid, &deck, 0);
            bare = before;
            for (i, share) in shares.iter_mut().enumerate() {
                let after = self.reading_with_cards(pid, &deck, (1 << (i + 1)) - 1);
                *share = after.minus(&before);
                before = after;
            }
            with = before;
        }
        self.players[pid].policies = held;
        PolicyLedger {
            anarchy: self.in_anarchy(pid),
            with,
            net: with.minus(&bare),
            cards: deck
                .into_iter()
                .zip(shares)
                .map(|(policy, worth)| PolicyWorth { policy, worth })
                .collect(),
        }
    }

    /// The empire read with only the cards of `deck` that `mask` names.
    fn reading_with_cards(&mut self, pid: usize, deck: &[Name], mask: usize) -> EmpireReading {
        self.players[pid].policies = deck
            .iter()
            .enumerate()
            .filter(|(i, _)| mask >> i & 1 == 1)
            .map(|(_, card)| *card)
            .collect();
        self.empire_reading(pid)
    }
}

/// Above this many slotted cards the exact shares (2^n empire readings) give
/// way to gains in deck order. Civilization VI's widest deck is about nine.
pub const EXACT_SHARE_CARDS: usize = 10;

#[cfg(test)]
mod tests {
    use super::*;

    /// One city on plain grassland, nothing else on the board: whatever a
    /// card is worth here is the card.
    fn founded_game() -> (Game, u32) {
        let mut game = Game::new_full(2, 40, 24, 388200, 300, 0, false);
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(uid);
        }
        game.barb_camps.clear();
        game.barb_naval_camps.clear();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
        }
        let cid = game.found_city_for(0, (6, 12), None);
        (game, cid)
    }

    #[test]
    fn a_flat_production_card_reads_as_its_production() {
        let (mut game, _) = founded_game();
        game.players[0].government = Some("chiefdom".to_string());
        game.players[0].policies.clear();
        game.players[0].policies.insert(Name::new("urban_planning"));
        let ledger = game.policy_ledger(0);
        assert_eq!(ledger.cards.len(), 1);
        let worth = ledger.cards[0].worth;
        assert!(
            (worth.production - 1.0).abs() < 1e-6,
            "Urban Planning is +1 Production in one city, read {worth:?}"
        );
        assert_eq!(worth.science, 0.0);
        assert_eq!(ledger.net, worth, "a one-card deck is worth that card");
        // Read on a branch: the deck in play is untouched.
        assert!(game.players[0]
            .policies
            .contains(&Name::new("urban_planning")));
    }

    fn assert_shares_sum_to_net(ledger: &PolicyLedger) {
        let mut sum = EmpireReading::default();
        for card in &ledger.cards {
            sum = sum.combine(&card.worth, |a, b| a + b);
        }
        for ((key, total), (_, net)) in sum.fields().into_iter().zip(ledger.net.fields()) {
            assert!(
                (total - net).abs() < 1e-9,
                "{key}: the cards add to {total}, the deck is worth {net}"
            );
        }
    }

    /// Urban Planning's +1 Production is itself sped by Colonization's +50%
    /// toward the Settler, so the two earn half a Production together. Each
    /// card's share carries half of that, and the shares add up to the deck.
    #[test]
    fn two_cards_that_multiply_split_what_they_earn_together() {
        let (mut game, cid) = founded_game();
        game.players[0].government = Some("chiefdom".to_string());
        game.players[0].policies.clear();
        game.players[0].policies.insert(Name::new("urban_planning"));
        game.players[0].policies.insert(Name::new("colonization"));
        let city = game.cities.get_mut(&cid).unwrap();
        city.queue.clear();
        city.queue.push(Item::Unit {
            unit: Name::new("settler"),
        });
        let mut bare = game.clone();
        bare.players[0].policies.clear();
        let base = bare.city_yields_model(cid).production;
        let ledger = game.policy_ledger(0);
        let worth = |card: &str| {
            ledger
                .cards
                .iter()
                .find(|worth| worth.policy == Name::new(card))
                .unwrap()
                .worth
                .production
        };
        assert!(
            (worth("urban_planning") - 1.25).abs() < 1e-6,
            "+1, and half of the +0.5 Colonization adds to it, read {}",
            worth("urban_planning")
        );
        assert!(
            (worth("colonization") - (0.5 * base + 0.25)).abs() < 1e-6,
            "half of {base}, and half of the +0.5 it adds to Urban Planning, read {}",
            worth("colonization")
        );
        assert!((ledger.net.production - (0.5 * base + 1.5)).abs() < 1e-6);
        assert_shares_sum_to_net(&ledger);
        // Past the exact limit the shares follow the deck's order instead:
        // the later card takes what the two earn together, the sum holds.
        let ordered = game.speculative_clone().ledger_sharing_exactly_up_to(0, 0);
        assert_eq!(ordered.net, ledger.net);
        assert_shares_sum_to_net(&ordered);
        assert_eq!(
            ordered.cards.iter().map(|card| card.policy).collect::<Vec<_>>(),
            ledger.cards.iter().map(|card| card.policy).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_card_that_moves_nothing_here_reads_zero() {
        let (mut game, _) = founded_game();
        game.players[0].government = Some("chiefdom".to_string());
        game.players[0].policies.clear();
        // +50% toward Settlers, with nothing being built: worth nothing now.
        game.players[0].policies.insert(Name::new("colonization"));
        for city in game.cities.values_mut() {
            city.queue.clear();
        }
        let ledger = game.policy_ledger(0);
        assert_eq!(ledger.cards[0].worth, EmpireReading::default());
    }

    #[test]
    fn a_maintenance_discount_reads_as_the_gold_it_saves() {
        let (mut game, _) = founded_game();
        game.players[0].government = Some("chiefdom".to_string());
        game.players[0].policies.clear();
        game.players[0].policies.insert(Name::new("conscription"));
        for _ in 0..3 {
            game.spawn_test_unit("swordsman", 0, (6, 12));
        }
        let saved: f64 = game
            .player_unit_ids(0)
            .into_iter()
            .map(|uid| game.rules.units[game.units[&uid].kind].maintenance.min(1.0))
            .sum();
        assert!(
            saved > 0.0,
            "the fixture's army must cost something to keep"
        );
        let worth = game.policy_ledger(0).cards[0].worth;
        assert!(
            (worth.gold - saved).abs() < 1e-6,
            "Conscription saves 1 Gold a unit ({saved}), read {worth:?}"
        );
    }

    #[test]
    fn a_production_percentage_counts_toward_what_it_speeds() {
        let (mut game, cid) = founded_game();
        game.players[0].government = Some("chiefdom".to_string());
        game.players[0].policies.clear();
        game.players[0].policies.insert(Name::new("colonization"));
        let city = game.cities.get_mut(&cid).unwrap();
        city.queue.clear();
        city.queue.push(Item::Unit {
            unit: Name::new("settler"),
        });
        let base = game.city_yields_model(cid).production;
        let ledger = game.policy_ledger(0);
        let worth = ledger.cards[0].worth.production;
        assert!(
            (worth - base * 0.5).abs() < 1e-6,
            "Colonization is +50% of {base} toward a Settler, read {worth}"
        );
    }
}
