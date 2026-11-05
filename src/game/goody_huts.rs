//! Goody huts: tribal villages and meteor sites, and what a unit finds in
//! one.
//!
//! Entering a hut (`maybe_goody_hut`), rolling the reward category and the
//! reward within it (`roll_goody_reward`, `roll_category_reward`), and
//! granting it (`grant_goody_reward`). The Apocalypse mode's meteor showers
//! (`process_meteors`) strike open terrain and leave a meteor goody site
//! for the first unit in.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Tribal village rewards on entering a goody-hut tile: the shipped
    /// seven-category table (data/goody_huts.json), rolled the way the game
    /// rolls it — a uniform category (every shipped category weighs 100),
    /// then a weighted reward among the ones currently eligible.
    pub(super) fn maybe_goody_hut(&mut self, uid: u32) {
        let (pos, owner) = match self.units.get(&uid) {
            Some(u) => (u.pos, u.owner),
            None => return,
        };
        if self.players[owner].is_barbarian {
            return;
        }
        let hut = self.map.get(pos).and_then(|t| t.improvement);
        match hut.as_deref() {
            Some("goody_hut") => {
                self.map.tiles.get_mut(&pos).unwrap().improvement = None;
                // The historic-moment counter cannot serve as the claim
                // tally: the Moment is era-gated to the Ancient world era,
                // so a village popped later would vanish from any census.
                *self.players[owner]
                    .counters
                    .entry("goody_huts_claimed".to_string())
                    .or_insert(0) += 1;
                self.roll_goody_reward(owner, uid, pos);
                self.add_historic_moment(owner, "MOMENT_GOODY_HUT_TRIGGERED");
            }
            Some("meteor_goody") => {
                // The crashed meteor is its own goody type with its own
                // one-entry table (METEOR_GOODIES).
                self.map.tiles.get_mut(&pos).unwrap().improvement = None;
                *self.players[owner]
                    .counters
                    .entry("meteor_goodies_claimed".to_string())
                    .or_insert(0) += 1;
                self.roll_category_reward(owner, uid, pos, "meteor_goodies");
            }
            _ => {}
        }
    }

    pub(super) fn roll_goody_reward(&mut self, owner: usize, uid: u32, pos: Pos) {
        // The village spins over the tribal categories only; the meteor's
        // table is a separate goody type popped by its own site.
        let categories: Vec<Name> = self
            .rules
            .goody_huts
            .keys()
            .filter(|category| category.as_str() != "meteor_goodies")
            .cloned()
            .collect();
        let mut order: Vec<usize> = (0..categories.len()).collect();
        let first = self.rng.below(order.len());
        order.rotate_left(first);
        for index in order {
            if self.roll_category_reward(owner, uid, pos, &categories[index]) {
                return;
            }
        }
    }

    /// One weighted roll inside a single goody category; false when
    /// nothing is currently eligible (a city-gated category before the
    /// first city).
    fn roll_category_reward(&mut self, owner: usize, uid: u32, pos: Pos, category: &str) -> bool {
        let has_city = self.cities.values().any(|c| c.owner == owner);
        let Some(rewards) = self.rules.goody_huts.get(category) else {
            return false;
        };
        let eligible: Vec<(String, i64)> = rewards
            .iter()
            .filter(|(_, reward)| {
                reward.weight > 0
                    && self.turn >= reward.min_turn
                    && (!reward.requires_city || has_city)
            })
            .map(|(name, reward)| (name.clone(), reward.weight))
            .collect();
        let total: i64 = eligible.iter().map(|(_, weight)| weight).sum();
        if total == 0 {
            return false;
        }
        let mut roll = self.rng.below(total as usize) as i64;
        for (name, weight) in eligible {
            roll -= weight;
            if roll < 0 {
                let reward = self.rules.goody_huts[category][&name].reward.clone();
                self.grant_goody_reward(owner, uid, pos, &reward);
                return true;
            }
        }
        false
    }

    fn grant_goody_reward(
        &mut self,
        owner: usize,
        uid: u32,
        pos: Pos,
        reward: &BTreeMap<String, f64>,
    ) {
        for (kind, amount) in reward {
            match kind.as_str() {
                "gold" => self.players[owner].gold += amount,
                "faith" => self.players[owner].faith += amount,
                "civic_boosts" => self.grant_random_boosts(owner, *amount as usize, false),
                "tech_boosts" => self.grant_random_boosts(owner, *amount as usize, true),
                "techs" => self.complete_random_nodes(owner, *amount as usize, true),
                "relic" => {
                    for _ in 0..*amount as usize {
                        let era = self.world_era;
                        self.grant_great_work(owner, "relic", era, "tribal_village");
                    }
                }
                "experience" => self.award_xp(uid, *amount),
                "heal" => {
                    if let Some(unit) = self.units.get_mut(&uid) {
                        unit.hp = (unit.hp + *amount as i32).min(100);
                    }
                }
                "population" => {
                    if let Some(city) = self
                        .cities
                        .values()
                        .filter(|c| c.owner == owner)
                        .min_by_key(|c| self.wdist(c.pos, pos))
                        .map(|c| c.id)
                    {
                        self.increase_city_population(city, 1);
                    }
                }
                "governor_title" => {
                    *self.players[owner]
                        .counters
                        .entry("district_governor_titles".to_string())
                        .or_insert(0) += *amount as i64;
                }
                "envoy" => self.players[owner].envoys_free += *amount as i64,
                "favor" => self.players[owner].diplomatic_favor += amount,
                "strategic_resources" => {
                    // The most advanced strategic the player has revealed.
                    let best = self
                        .rules
                        .resources
                        .iter()
                        .filter(|(_, spec)| spec.class == "strategic")
                        .filter(|(name, _)| self.resource_visible_to(owner, name))
                        .max_by_key(|(name, spec)| {
                            (
                                spec.tech
                                    .as_deref()
                                    .and_then(|t| self.rules.techs.get(t))
                                    .map(|t| t.era)
                                    .unwrap_or(0),
                                *name,
                            )
                        })
                        .map(|(name, _)| *name);
                    if let Some(resource) = best {
                        let capacity = self.strategic_stockpile_capacity(owner);
                        let stock = self.players[owner]
                            .strategic_resources
                            .entry(resource)
                            .or_insert(0.0);
                        *stock = (*stock + amount).min(capacity);
                    }
                }
                "advanced_heavy_cavalry" => {
                    // MODIFIER_PLAYER_GRANT_ADVANCED_UNIT_OF_CLASS_IN_
                    // NEAREST_OWNER_CITY: the best Heavy Cavalry the finder
                    // can field arrives in the closest owned city, and the
                    // refund modifier keeps it free of resource upkeep.
                    let best = self
                        .rules
                        .units
                        .iter()
                        .filter(|(_, spec)| {
                            spec.promotion_class == "heavy_cavalry"
                                && spec.buildable
                                && spec.unique_to.is_none()
                                && spec.civic.is_none()
                                && spec.tech.as_deref().is_none_or(|tech| {
                                    self.players[owner].techs.contains(&Name::new(tech))
                                })
                        })
                        .max_by_key(|(name, spec)| {
                            let era = spec
                                .tech
                                .as_deref()
                                .and_then(|tech| self.rules.techs.get(tech))
                                .map(|tech| tech.era)
                                .unwrap_or(0);
                            (era, spec.cost as i64, *name)
                        })
                        .map(|(name, _)| *name);
                    let home = self
                        .cities
                        .values()
                        .filter(|city| city.owner == owner)
                        .min_by_key(|city| (self.wdist(city.pos, pos), city.id))
                        .map(|city| city.pos);
                    if let (Some(kind), Some(home)) = (best, home) {
                        for _ in 0..*amount as usize {
                            if let Some(id) = self.place_new_unit(&kind, owner, home) {
                                self.units.get_mut(&id).unwrap().free_upkeep = true;
                            }
                        }
                    }
                }
                unit if unit.starts_with("unit:") => {
                    let kind = &unit["unit:".len()..];
                    for _ in 0..*amount as usize {
                        self.spawn_unit(kind, owner, pos);
                    }
                }
                _ => {}
            }
        }
    }

    /// The Apocalypse pack's meteor showers: about six strikes per game
    /// (the shipped Moderate frequency; Minimal 3 / Light 4 / Heavy 10 /
    /// Hyperreal 15), each hitting one open hex of the shipped strike
    /// terrains outside anyone's territory (Severity 1, Hexes 1,
    /// AvoidTerritory) and leaving the meteor goody site for the first
    /// unit in. Strikes draw from a turn-keyed stream so they never
    /// perturb the main RNG sequence, and any shortfall is forced in
    /// before the turn limit.
    pub(super) fn process_meteors(&mut self) {
        const PER_GAME: u32 = 6;
        // Meteor showers arrived with the New Frontier Pass' Apocalypse mode,
        // not with Gathering Storm. A stock lobby — and every tournament one,
        // which disables all game modes — sees none.
        if !self.game_mode("apocalypse") || self.meteor_strikes >= PER_GAME || self.max_turns == 0 {
            return;
        }
        let mut rng = Rng::new(
            self.seed
                ^ 0x4D45_5445_4F52u64
                    .wrapping_add((self.turn as u64) << 8)
                    .wrapping_add(self.meteor_strikes as u64),
        );
        let remaining = (PER_GAME - self.meteor_strikes) as u64;
        let due = u64::from(self.max_turns.saturating_sub(self.turn)) <= remaining
            || (rng.below(self.max_turns as usize) as u32) < PER_GAME;
        if !due {
            return;
        }
        // Open land on the shipped strike list, kept clear of anything
        // already standing so camp and hut bookkeeping stays whole.
        let candidates: Vec<Pos> = self
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| {
                matches!(
                    tile.terrain.as_str(),
                    "plains" | "grassland" | "snow" | "desert"
                ) && tile.owner_city.is_none()
                    && tile.district.is_none()
                    && tile.wonder.is_none()
                    && tile.improvement.is_none()
                    && tile
                        .feature
                        .as_deref()
                        .is_none_or(|feature| matches!(feature, "forest" | "jungle" | "marsh"))
            })
            .map(|(position, _)| *position)
            .collect();
        if candidates.is_empty() {
            return;
        }
        let position = candidates[rng.below(candidates.len())];
        self.map.tiles.get_mut(&position).unwrap().improvement = Some(crate::name!("meteor_goody"));
        self.meteor_strikes += 1;
    }
}
