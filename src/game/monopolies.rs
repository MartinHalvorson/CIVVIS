//! Monopolies and Corporations: resources an empire controls, the
//! industries and corporations built on them, products, and monopolies.
//!
//! Counting resources: connected and controlled, including through
//! suzerained city-states, and the world's totals. Industries and
//! corporations: a city's economic improvement and the effects of its
//! resource industry, founding a corporation by retiring a Great Merchant
//! (`can_found_corporation`, `do_found_corporation`), and its owner.
//! Products: capacity, the slot a product sits in, moving one
//! (`do_move_product`) and the yields it gives a city. Monopolies: the
//! bonuses a controlling share earns (`monopoly_bonuses`,
//! `monopoly_bonuses_with`, the context they read) and the first-monopoly
//! historic moments.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! and `pub(crate)` methods are unchanged; the code is otherwise unchanged.

use super::*;

impl Game {
    fn corporation_owner(&self, resource: &str) -> Option<usize> {
        self.cities.values().find_map(|city| {
            city.owned_tiles.iter().find_map(|position| {
                let tile = &self.map.tiles[position];
                (tile.improvement.as_deref() == Some("corporation")
                    && tile.resource.as_deref() == Some(resource))
                .then_some(city.owner)
            })
        })
    }

    pub fn can_found_corporation(&self, pid: usize, pos: Pos) -> bool {
        let Some(tile) = self.map.get(pos) else {
            return false;
        };
        let Some(city_id) = tile.owner_city else {
            return false;
        };
        let Some(resource) = tile.resource.as_deref() else {
            return false;
        };
        if self.cities[&city_id].owner != pid
            || tile.improvement.as_deref() != Some("industry")
            || tile.pillaged
            || !self.players[pid].techs.contains(&crate::name!("economics"))
            || self.controlled_resource_count(pid, resource) < 3
            || !self.great_person_class_offered_now(pid, "merchant")
            || self.current_great_person("merchant").is_none_or(|_| {
                self.players[pid]
                    .gpp
                    .get("merchant")
                    .copied()
                    .unwrap_or(0.0)
                    + f64::EPSILON
                    < self.gp_cost(pid, "merchant")
            })
        {
            return false;
        }
        self.corporation_owner(resource).is_none_or(|owner| {
            owner != pid
                && self.controlled_resource_count(pid, resource)
                    > self.controlled_resource_count(owner, resource)
        })
    }

    fn retire_merchant_for_corporation(&mut self, pid: usize) -> Result<(), String> {
        if !self.great_person_class_offered_now(pid, "merchant") {
            return Err("Great Merchant class is not currently offered by the live game".into());
        }
        let (id, era) = self
            .current_great_person("merchant")
            .map(|(id, spec)| (id.to_string(), spec.era))
            .ok_or_else(|| "no Great Merchant is currently available".to_string())?;
        let cost = self.gp_cost(pid, "merchant");
        let points = self.players[pid]
            .gpp
            .get("merchant")
            .copied()
            .unwrap_or(0.0);
        if points + f64::EPSILON < cost {
            return Err("not enough Great Merchant points".into());
        }
        self.players[pid].gpp.insert("merchant".to_string(), 0.0);
        self.retired_great_people.insert(id.clone());
        self.players[pid].great_people.push(id);
        *self.players[pid]
            .gp_claimed
            .entry("merchant".to_string())
            .or_insert(0) += 1;
        self.add_historic_moment(
            pid,
            if era < self.world_era {
                "MOMENT_GREAT_PERSON_CREATED_PAST_ERA"
            } else {
                "MOMENT_GREAT_PERSON_CREATED_GAME_ERA"
            },
        );
        self.dedication_trigger(pid, "great_person", 1);
        bump(&mut self.players[pid], "great_people");
        self.apply_great_person_district_effects(pid);
        self.refresh_great_person_offers();
        Ok(())
    }

    pub(super) fn do_found_corporation(&mut self, pid: usize, pos: Pos) -> Result<(), String> {
        if !self.can_found_corporation(pid, pos) {
            return Err("cannot found a Corporation here".into());
        }
        let resource = self.map.tiles[&pos].resource.unwrap();
        self.retire_merchant_for_corporation(pid)?;
        if let Some(previous) = self.corporation_owner(&resource) {
            if previous != pid {
                let existing = self
                    .map
                    .tiles
                    .iter()
                    .find(|(_, tile)| {
                        tile.improvement.as_deref() == Some("corporation")
                            && tile.resource.as_deref() == Some(resource.as_str())
                    })
                    .map(|(position, _)| *position);
                if let Some(position) = existing {
                    self.map.tiles.get_mut(&position).unwrap().improvement =
                        Some(crate::name!("industry"));
                }
            }
        }
        self.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("corporation"));
        self.map.tiles.get_mut(&pos).unwrap().pillaged = false;
        bump(&mut self.players[pid], "corporations");
        self.first_historic_moment(
            pid,
            "first_corporation",
            Some("MOMENT_FIRST_CORPORATION"),
            Some("MOMENT_FIRST_CORPORATION_IN_WORLD"),
        );
        Ok(())
    }

    pub(super) fn do_move_product(
        &mut self,
        pid: usize,
        from: u32,
        to: u32,
        product: &str,
    ) -> Result<(), String> {
        if from == to {
            return Err("Product is already in that city".into());
        }
        let source = self
            .cities
            .get(&from)
            .filter(|city| city.owner == pid)
            .ok_or_else(|| "not your source city".to_string())?;
        let index = source
            .products
            .iter()
            .position(|candidate| candidate == product)
            .ok_or_else(|| "source city does not house that Product".to_string())?;
        let target = self
            .cities
            .get(&to)
            .filter(|city| city.owner == pid)
            .ok_or_else(|| "not your target city".to_string())?;
        if target.products.len() >= self.product_capacity(target) {
            return Err("target city has no open Product slot".into());
        }
        self.cities.get_mut(&from).unwrap().products.remove(index);
        self.cities
            .get_mut(&to)
            .unwrap()
            .products
            .push(product.to_string());
        Ok(())
    }

    /// Connected native copies before temporary deal imports and exports.
    pub fn connected_resource_count(&self, pid: usize, res: &str) -> i32 {
        if !self.resource_visible_to(pid, res) {
            return 0;
        }
        self.connected_resource_count_unchecked(pid, res)
    }

    /// The interned-name form used by the retained monopoly context. Keeping
    /// this separate preserves the borrowed census API used by luxury and
    /// strategic-resource callers while letting the long-lived cache own its
    /// keys safely.
    fn connected_resource_census_owned(&self, pid: usize) -> BTreeMap<Name, i32> {
        let mut counts: BTreeMap<Name, i32> = BTreeMap::new();
        for city in self.cities.values().filter(|city| city.owner == pid) {
            for position in &city.owned_tiles {
                let tile = &self.map.tiles[position];
                let Some(resource) = tile.resource else {
                    continue;
                };
                if self.tile_connects_resource(tile, resource, *position == city.pos) {
                    *counts.entry(resource).or_insert(0) += 1;
                }
            }
        }
        counts
    }

    pub(super) fn connected_resource_count_unchecked(&self, pid: usize, res: &str) -> i32 {
        if self.rules.resources.get(res).is_none() {
            return 0;
        }
        let resource = Name::new(res);
        self.cities
            .values()
            .filter(|city| city.owner == pid)
            .flat_map(|city| {
                city.owned_tiles
                    .iter()
                    .map(move |position| (city, position))
            })
            .filter(|(city, position)| {
                let tile = &self.map.tiles[position];
                self.tile_connects_resource(tile, resource, **position == city.pos)
            })
            .count() as i32
    }

    /// Improved map copies controlled for the Monopolies & Corporations mode.
    /// Unlike ordinary resource access this includes suzerain city-states, but
    /// deliberately excludes diplomatic imports and synthetic luxury grants.
    pub fn controlled_resource_count(&self, pid: usize, res: &str) -> i32 {
        let own = self.connected_resource_count(pid, res);
        if !self.resource_visible_to(pid, res) {
            return own;
        }
        self.controlled_resource_count_via(pid, res, &self.suzerained_minors(pid))
    }

    pub(super) fn controlled_resource_count_via(
        &self,
        pid: usize,
        res: &str,
        minors: &[usize],
    ) -> i32 {
        let own = self.connected_resource_count(pid, res);
        if !self.resource_visible_to(pid, res) {
            return own;
        }
        own + minors
            .iter()
            .map(|minor| self.connected_resource_count_unchecked(*minor, res))
            .sum::<i32>()
    }

    /// How many copies of every resource the world holds, in one pass.
    fn world_resource_counts(&self) -> BTreeMap<Name, i32> {
        let mut counts: BTreeMap<Name, i32> = BTreeMap::new();
        for tile in self.map.tiles.values() {
            if tile.submerged {
                continue;
            }
            if let Some(resource) = tile.resource {
                *counts.entry(resource).or_default() += 1;
            }
        }
        counts
    }

    pub(super) fn city_economic_improvement(&self, city: &City) -> Option<(Name, bool)> {
        city.owned_tiles.iter().find_map(|position| {
            let tile = &self.map.tiles[position];
            let corporation = match tile.improvement.as_deref() {
                Some("industry") => false,
                Some("corporation") => true,
                _ => return None,
            };
            tile.resource
                .as_ref()
                .map(|resource| (*resource, corporation))
        })
    }

    pub(super) fn city_active_economic_improvement(&self, city: &City) -> Option<(Name, bool)> {
        self.city_economic_improvement(city).filter(|_| {
            city.owned_tiles.iter().any(|position| {
                let tile = &self.map.tiles[position];
                !tile.pillaged
                    && matches!(
                        tile.improvement.as_deref(),
                        Some("industry" | "corporation")
                    )
            })
        })
    }

    /// Complete city effect of Industries, Corporations and housed Products.
    /// The improvement is one Industry bundle (two for a Corporation); each
    /// active Product contributes its own shipped Product modifier bundle.
    pub(super) fn city_resource_industry_effects(&self, city: &City) -> ResourceIndustryEffects {
        let mut effects = ResourceIndustryEffects::default();
        for product in city.products.iter().take(self.product_capacity(city)) {
            if let Some(resource) = self.rules.resources.get(product) {
                effects.add_scaled(resource.product_effects, 1.0);
            }
        }
        if let Some((resource, corporation)) = self.city_active_economic_improvement(city) {
            if let Some(resource) = self.rules.resources.get(&resource) {
                effects.add_scaled(
                    resource.industry_effects,
                    if corporation { 2.0 } else { 1.0 },
                );
            }
        }
        effects
    }

    pub(super) fn city_product_yields(&self, city: &City) -> Yields {
        let mut yields = Yields::default();
        for product in city.products.iter().take(self.product_capacity(city)) {
            if let Some(resource) = self.rules.resources.get(product) {
                yields.add(resource.product_yields);
            }
        }
        yields
    }

    pub(super) fn empire_has_economic_improvement(&self, pid: usize, resource: &str) -> bool {
        self.cities.values().any(|city| {
            city.owner == pid
                && self
                    .city_economic_improvement(city)
                    .is_some_and(|(candidate, _)| candidate == resource)
        })
    }

    pub fn product_capacity(&self, city: &City) -> usize {
        city.buildings
            .iter()
            .filter(|building| {
                !city.pillaged_buildings.contains(*building)
                    && self.building_district_is_active(city, building)
            })
            .map(|building| {
                self.rules.buildings[building]
                    .effects
                    .get("product_slots")
                    .copied()
                    .unwrap_or(0.0)
                    .max(0.0) as usize
            })
            .sum()
    }

    pub(super) fn world_product_count(&self, resource: &str) -> usize {
        self.cities
            .values()
            .flat_map(|city| city.products.iter())
            .filter(|product| product.as_str() == resource)
            .count()
    }

    pub(super) fn product_slot_city(&self, pid: usize, preferred: u32) -> Option<u32> {
        self.cities
            .get(&preferred)
            .filter(|city| city.owner == pid && city.products.len() < self.product_capacity(city))
            .map(|city| city.id)
            .or_else(|| {
                self.cities
                    .values()
                    .filter(|city| {
                        city.owner == pid && city.products.len() < self.product_capacity(city)
                    })
                    .map(|city| city.id)
                    .min()
            })
    }

    /// Per-turn Gold and outgoing Tourism modifier from live luxury
    /// Monopolies. The percentage follows Civ VI's 1% per controlled copy and
    /// foreign non-controller, or 3% after establishing an Industry/Corporation.
    pub fn monopoly_bonuses(&self, pid: usize) -> (f64, f64) {
        // A standalone query must remain a faithful snapshot even for
        // callers that assemble a test/scenario board by editing fields
        // directly. The retained form is populated by the action boundary;
        // without one, build a one-shot context and do not retain it.
        let cached = self
            .query_memo
            .monopoly_context
            .borrow()
            .as_ref()
            .filter(|context| {
                context.connected.len() == self.players.len()
                    && context.minors.len() == self.players.len()
            })
            .cloned();
        let context = cached.unwrap_or_else(|| Arc::new(self.build_monopoly_context()));
        self.monopoly_bonuses_with(&context, pid)
    }

    /// The three world-wide derivations a monopoly answer reads.
    ///
    /// None of them depends on which player is asking, and together they are
    /// nearly all of the cost: the world census walks every tile on the map,
    /// and the connected census walks every owned tile of every city in the
    /// game. Separating them lets a caller that asks about several players
    /// pay once — `note_first_monopoly_moments` asks about every seat after
    /// most actions, which made that scan quadratic in the number of seats.
    pub(super) fn monopoly_context(&self) -> Arc<MonopolyContext> {
        if let Some(context) =
            self.query_memo
                .monopoly_context
                .borrow()
                .as_ref()
                .filter(|context| {
                    context.connected.len() == self.players.len()
                        && context.minors.len() == self.players.len()
                })
        {
            return context.clone();
        }
        let context = Arc::new(self.build_monopoly_context());
        *self.query_memo.monopoly_context.borrow_mut() = Some(context.clone());
        context
    }

    fn build_monopoly_context(&self) -> MonopolyContext {
        let mut minors: Vec<Vec<usize>> = vec![Vec::new(); self.players.len()];
        // The suzerain memo is guard-scoped, while this context is retained
        // across calls. Open one scope for the complete build so every
        // city-state patron is resolved once even on a cache miss.
        let _memo = self.query_memo();
        for minor in self
            .players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian)
        {
            if let Some(patron) = self.suzerain_of(minor.id) {
                if let Some(clients) = minors.get_mut(patron) {
                    clients.push(minor.id);
                }
            }
        }
        // Resource control is queried for every luxury below. Build each
        // player's connected holdings once rather than walking all of their
        // city tiles again for every resource and every foreign player.
        let connected: Vec<BTreeMap<Name, i32>> = (0..self.players.len())
            .map(|player| self.connected_resource_census_owned(player))
            .collect();
        MonopolyContext {
            world_counts: self.world_resource_counts(),
            minors,
            connected,
        }
    }

    fn monopoly_bonuses_with(&self, context: &MonopolyContext, pid: usize) -> (f64, f64) {
        let mut gold = 0.0;
        let mut tourism_percent = 0.0;
        let mut monopolies = 0usize;
        let world_counts = &context.world_counts;
        let minors = &context.minors;
        let connected = &context.connected;
        let controlled = |player: usize, resource: Name| {
            if !self.resource_visible_to(player, resource.as_str()) {
                return 0;
            }
            connected[player]
                .get(&resource)
                .copied()
                .unwrap_or_default()
                + minors[player]
                    .iter()
                    .map(|minor| {
                        connected[*minor]
                            .get(&resource)
                            .copied()
                            .unwrap_or_default()
                    })
                    .sum::<i32>()
        };
        for (resource, spec) in &self.rules.resources {
            if spec.class != "luxury" {
                continue;
            }
            let total = world_counts.get(resource).copied().unwrap_or_default();
            let player_controlled = controlled(pid, *resource);
            if total <= 0 || player_controlled * 100 < total * 60 {
                continue;
            }
            monopolies += 1;
            gold += if player_controlled >= total {
                25.0
            } else if player_controlled * 100 >= total * 75 {
                10.0
            } else {
                5.0
            };
            let foreign_noncontrollers = self
                .players
                .iter()
                .filter(|player| {
                    player.id != pid
                        && player.alive
                        && !player.is_minor
                        && !player.is_barbarian
                        && controlled(player.id, *resource) == 0
                })
                .count() as f64;
            let developed = self.cities.values().any(|city| {
                city.owner == pid
                    && city.owned_tiles.iter().any(|position| {
                        let tile = &self.map.tiles[position];
                        tile.resource == Some(*resource)
                            && matches!(
                                tile.improvement.as_deref(),
                                Some("industry" | "corporation")
                            )
                    })
            });
            tourism_percent += player_controlled as f64
                * foreign_noncontrollers
                * if developed { 3.0 } else { 1.0 };
        }
        if monopolies > 0 {
            let partners = self
                .routes
                .iter()
                .filter(|route| route.owner == pid && route.ends > self.turn)
                .filter_map(|route| self.cities.get(&route.dest).map(|city| city.owner))
                .filter(|owner| {
                    *owner != pid
                        && !self.players[*owner].is_minor
                        && !self.players[*owner].is_barbarian
                })
                .collect::<BTreeSet<_>>()
                .len();
            gold += 5.0 * partners as f64;
        }
        (gold, tourism_percent)
    }

    /// Monopoly control can change through improvements, repair, city
    /// transfer, trade, or Suzerain changes. Running this one transition
    /// detector after each successful action covers every such mutation.
    pub(super) fn note_first_monopoly_moments(&mut self) {
        if !self.players.iter().any(|player| {
            player.alive
                && !player.is_minor
                && !player.is_barbarian
                && !player.is_free_city
                && !player
                    .counters
                    .contains_key("historic_moment:first_luxury_monopoly")
        }) {
            return;
        }
        // One context for the whole sweep. Asking each seat separately made
        // this walk the map and every city's tiles once per seat.
        let context = self.monopoly_context();
        let new_monopolists: Vec<usize> = self
            .players
            .iter()
            .filter(|player| {
                player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && !player.is_free_city
                    && !player
                        .counters
                        .contains_key("historic_moment:first_luxury_monopoly")
                    && self.monopoly_bonuses_with(&context, player.id).0 > 0.0
            })
            .map(|player| player.id)
            .collect();
        for pid in new_monopolists {
            self.first_historic_moment(
                pid,
                "first_luxury_monopoly",
                Some("MOMENT_FIRST_LUXURY_RESOURCE_MONOPOLY"),
                Some("MOMENT_FIRST_LUXURY_RESOURCE_MONOPOLY_IN_WORLD"),
            );
        }
    }
}
