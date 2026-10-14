//! What each slotted policy card is worth to its empire right now.
//!
//! Civilization VI prints a card's text and never its value: "+100% Campus
//! adjacency bonuses" says nothing about whether this empire's Campuses earn
//! two Science or forty from it. Players install a mod for the missing number.
//! Here it is the engine's own arithmetic, read the way the deck chooser in
//! `ai.rs` (`empire_reading`) already reads it: measure the empire, take the
//! card out, measure again. Nothing names an effect key, so every card the
//! engine implements is covered and a card it does not implement reads zero
//! instead of quoting its own text back.

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

/// One slotted card and what taking it out would cost the empire.
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
    /// The whole deck against no deck at all. Not the sum of the cards: two
    /// cards can multiply each other, and this is what the deck actually does.
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
    pub fn into_policy_ledger(mut self, pid: usize) -> PolicyLedger {
        if self
            .players
            .get(pid)
            .is_none_or(|player| player.is_minor || player.is_barbarian)
        {
            return PolicyLedger::default();
        }
        let deck = self.players[pid].policies.clone();
        let with = self.empire_reading(pid);
        let mut cards = Vec::with_capacity(deck.len());
        for card in &deck {
            self.players[pid].policies.remove(card);
            let without = self.empire_reading(pid);
            self.players[pid].policies.insert(*card);
            cards.push(PolicyWorth {
                policy: *card,
                worth: with.minus(&without),
            });
        }
        self.players[pid].policies.clear();
        let bare = self.empire_reading(pid);
        self.players[pid].policies = deck;
        PolicyLedger {
            anarchy: self.in_anarchy(pid),
            with,
            net: with.minus(&bare),
            cards,
        }
    }
}

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
