//! Trade routes: establishing, pricing, running and cancelling them.
//!
//! Whether a route can be established (`validate_trade_route_destination`,
//! `can_establish_trade_route`), the `do_trade_route` order, a route's
//! duration, yields and district/Trading Post/Trading Dome gold, the cities
//! it passes, the turn's `process_routes`, and recalling or cancelling
//! routes.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Trading capacity: 1 with Foreign Trade, plus capacity granted by
    /// capacity-bearing buildings and explicit modifiers. Commercial Hubs and
    /// Harbors unlock their respective trade building, but do not themselves
    /// add a route slot in the shipped Civ VI rules.
    pub fn trade_capacity(&self, pid: usize) -> i64 {
        if let Some(capacity) = self.observed_trade_capacity.get(&pid) {
            return *capacity;
        }
        let p = &self.players[pid];
        let tree_capacity = self.tree_effect(pid, "trade_route_capacity") as i64;
        if tree_capacity <= 0 {
            return 0;
        }
        let mut cap = tree_capacity;
        for city in self.cities.values().filter(|city| city.owner == pid) {
            // Market and Lighthouse are the same either/or capacity tier.
            // This follows the installed rules database: both attach
            // MODIFIER_PLAYER_ADJUST_TRADE_ROUTE_CAPACITY, while the base
            // Commercial Hub and Harbor district rows attach no such modifier.
            if self.city_has_active_building_family(city, crate::name!("market"))
                || self.city_has_active_building_family(city, crate::name!("lighthouse"))
            {
                cap += 1;
            }
            // The Owls of Minerva Gilded Vault is the explicit exception: a
            // Harbor in the same city grants one additional route.
            if city.buildings.iter().any(|building| {
                building == "gilded_vault"
                    && !city.pillaged_buildings.contains(building)
                    && self.building_district_is_active(city, building)
            }) && self.city_has_district_family(city, crate::name!("harbor"))
            {
                cap += self.rules.buildings["gilded_vault"]
                    .effects
                    .get("harbor_trade_route_capacity")
                    .copied()
                    .unwrap_or(0.0) as i64;
            }
        }
        cap += self.empire_wonder_effect(pid, "trade_route_capacity") as i64;
        cap += self.policy_effect(pid, "policy_trade_route_capacity") as i64;
        cap += self.gov_effects(pid).trade_route_capacity as i64;
        cap += p
            .counters
            .get("great_person_trade_capacity")
            .copied()
            .unwrap_or(0);
        if self.congress_effect_active("trade_policy", "A", &pid.to_string()) {
            cap += 1;
        }
        if self.grants_city_state_unique_bonus(pid, "Carthage") {
            cap += self
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .filter(|city| {
                    self.city_has_active_district_family(city, crate::name!("encampment"))
                })
                .count() as i64;
        }
        cap
    }

    pub fn active_routes(&self, pid: usize) -> i64 {
        self.routes.iter().filter(|r| r.owner == pid).count() as i64
    }

    /// Standard-speed Gathering Storm route duration. A Trader must complete
    /// whole round trips and may finish only after the era-scaled minimum end
    /// turn: 20 turns through Classical, then +10 in Medieval/Renaissance,
    /// +20 in Industrial through Atomic, and +30 in Information/Future.
    pub(crate) fn trade_route_duration(one_way_distance: u32, world_era: usize) -> u32 {
        let minimum_end_turn = 20
            + match world_era {
                0 | 1 => 0,
                2 | 3 => 10,
                4..=6 => 20,
                _ => 30,
            };
        let round_trip = one_way_distance.max(1).saturating_mul(2);
        round_trip.saturating_mul(minimum_end_turn / round_trip + 1)
    }

    /// Origin-city yields of a route by destination districts (Civ 6 vanilla
    /// table): domestic food/production, international gold/science/etc.
    pub fn route_yields(&self, dest: u32, domestic: bool) -> Yields {
        let city = &self.cities[&dest];
        let mut ys = Yields::default();
        if domestic {
            ys.food += 1.0;
            ys.production += 1.0; // city center
        } else {
            ys.gold += 3.0;
        }
        // Every completed destination district pays its row, PILLAGED OR NOT:
        // Rome's pillaged Holy Site and Campus kept paying Cumae's domestic
        // route "+6 from Outgoing Trade Routes" for t81-95 of run
        // civvis-20260816T200454Z (the model, skipping them, read +4/+5), and
        // the ledger fell back into step exactly as each was repaired.
        for (d, _) in city.districts.iter() {
            match (self.district_family(*d).as_str(), domestic) {
                ("campus", true)
                | ("holy_site", true)
                | ("theater_square", true)
                | ("entertainment_complex", true) => ys.food += 1.0,
                ("encampment", _)
                | ("industrial_zone", _)
                | ("commercial_hub", true)
                | ("harbor", true) => ys.production += 1.0,
                ("commercial_hub", false) | ("harbor", false) => ys.gold += 3.0,
                ("campus", false) => ys.science += 1.0,
                ("holy_site", false) => ys.faith += 1.0,
                ("theater_square", false) => ys.culture += 1.0,
                ("entertainment_complex", false) => ys.food += 1.0,
                ("government_plaza", true) | ("diplomatic_quarter", true) => {
                    ys.food += 1.0;
                    ys.production += 1.0;
                }
                ("government_plaza", false) => ys.gold += 2.0,
                ("diplomatic_quarter", false) => ys.culture += 1.0,
                _ => {}
            }
        }
        ys
    }

    /// Origin yields that depend on both the route owner and destination.
    /// This keeps Great Person modifiers visible to city processing, AI route
    /// valuation, and any future preview/UI consumer through one calculation.
    pub fn trade_route_yields(&self, owner: usize, dest: u32) -> Yields {
        let city = &self.cities[&dest];
        let domestic = city.owner == owner;
        let mut yields = self.route_yields(dest, domestic);
        if !domestic {
            // Gathering Storm ships city-state `SEND_TRADE_ROUTE_BONUS` rows,
            // but the host does not apply those rows to origin route yields.
            // Keep this absent for host parity; Sovereignty A consequently has
            // no route-yield change in the installed game either.
            yields.gold += city.great_person_foreign_route_gold;
            // A Feitoria belongs to the foreign destination rather than the
            // route's origin. Every Portuguese international route arriving
            // at that city receives its production-and-gold contract bonus.
            if self.players[owner].civ == "Portugal" {
                let feitorias = city
                    .owned_tiles
                    .iter()
                    .filter(|position| {
                        let tile = &self.map.tiles[position];
                        tile.improvement.as_deref() == Some("feitoria") && !tile.pillaged
                    })
                    .count() as f64;
                let effects = &self.rules.improvements["feitoria"].effects;
                yields.gold +=
                    feitorias * effects.get("foreign_route_gold").copied().unwrap_or(0.0);
                yields.production += feitorias
                    * effects
                        .get("foreign_route_production")
                        .copied()
                        .unwrap_or(0.0);
            }
        }
        yields.gold += self.players[owner]
            .counters
            .get("great_person:strategic_destination_trade_gold")
            .copied()
            .unwrap_or(0) as f64
            * self.city_connected_strategic_resources(city) as f64;
        yields
    }

    /// Samarkand: each unpillaged Trading Dome in the origin city adds one
    /// Gold to that city's outgoing international routes.
    pub(super) fn trading_dome_origin_route_gold(&self, origin: u32) -> f64 {
        let city = &self.cities[&origin];
        city.owned_tiles
            .iter()
            .filter(|position| {
                let tile = &self.map.tiles[position];
                tile.improvement.as_deref() == Some("trading_dome") && !tile.pillaged
            })
            .count() as f64
            * self.rules.improvements["trading_dome"]
                .effects
                .get("international_trade_gold_at_origin")
                .copied()
                .unwrap_or(0.0)
    }

    /// Improved strategic-resource copies controlled by one city. Great
    /// Person trade effects count copies at the destination, not empire-wide
    /// access or resources imported through diplomacy.
    pub(super) fn city_connected_strategic_resources(&self, city: &City) -> usize {
        city.owned_tiles
            .iter()
            .filter(|position| {
                let tile = &self.map.tiles[position];
                let Some(resource) = tile.resource else {
                    return false;
                };
                let Some(spec) = self.rules.resources.get_interned(resource) else {
                    return false;
                };
                spec.class == "strategic"
                    && self.tile_connects_resource(tile, resource, **position == city.pos)
            })
            .count()
    }

    fn validate_trade_route_destination(
        &self,
        pid: usize,
        origin: u32,
        dest: u32,
    ) -> Result<(), String> {
        let origin_city = self
            .cities
            .get(&origin)
            .filter(|city| city.owner == pid)
            .ok_or_else(|| "route must originate in one of your cities".to_string())?;
        let dc = self
            .cities
            .get(&dest)
            .ok_or_else(|| "no such city".to_string())?;
        if dest == origin {
            return Err("destination is the origin".into());
        }
        if self.is_at_war(pid, dc.owner) {
            return Err("cannot trade with an enemy".into());
        }
        if self
            .blocked_trade_routes
            .contains(&(origin_city.pos, dc.pos))
        {
            return Err("host rejected that trade route".into());
        }
        if origin_city.owner != dc.owner
            && (self.congress_effect_active("trade_policy", "B", &pid.to_string())
                || self.congress_effect_active("trade_policy", "B", &dc.owner.to_string()))
        {
            return Err("World Congress embargoes that trade route".into());
        }
        // The native chooser's CanStartRoute offers account for extended
        // range and trading posts. A current offer for this exact pair is
        // stronger evidence than the simulator's fixed distance limit.
        if self.wdist(origin_city.pos, dc.pos) > 15
            && !self.observed_route_options.contains_key(&(origin, dest))
        {
            return Err("destination out of range".into());
        }
        if self
            .routes
            .iter()
            .any(|route| route.owner == pid && route.dest == dest)
        {
            return Err("route already active".into());
        }
        Ok(())
    }

    /// Whether this empire could legally assign an available Trader from the
    /// given origin to the destination. Capacity and the unit itself are
    /// intentionally excluded so AI planning can count concrete route
    /// opportunities before deciding how many Traders to build.
    pub(crate) fn can_establish_trade_route(&self, pid: usize, origin: u32, dest: u32) -> bool {
        self.validate_trade_route_destination(pid, origin, dest)
            .is_ok()
    }

    pub(super) fn do_trade_route(&mut self, pid: usize, uid: u32, dest: u32) -> Result<(), String> {
        let u = self.own_unit(pid, uid)?;
        if u.kind != "trader" {
            return Err("only traders run routes".into());
        }
        let origin = self
            .city_at(u.pos)
            .filter(|cid| self.cities[cid].owner == pid)
            .ok_or_else(|| "trader must be in one of your cities".to_string())?;
        self.validate_trade_route_destination(pid, origin, dest)?;
        if self.active_routes(pid) >= self.trade_capacity(pid) {
            return Err("no trading capacity".into());
        }
        let level = self.road_level_for(pid);
        self.build_road(self.cities[&origin].pos, self.cities[&dest].pos, level);
        let distance = self
            .wdist(self.cities[&origin].pos, self.cities[&dest].pos)
            .max(1) as u32;
        let ends = self.turn
            + self.standard_duration(Self::trade_route_duration(distance, self.world_era));
        self.routes.push(TradeRoute {
            origin,
            dest,
            owner: pid,
            ends,
        });
        self.remove_unit(uid); // the trader services the route until it ends
        Ok(())
    }

    /// The best route the shipped Routes table lets this civilization's
    /// traders lay: Medieval from the Classical era, Industrial and Modern
    /// from their namesake eras (each route's PrereqEra). Railroads are
    /// never trader-laid - they are engineer-built only.
    pub(super) fn road_level_for(&self, pid: usize) -> u8 {
        match self.player_era(pid) {
            era if era >= 5 => 4,
            era if era >= 4 => 3,
            era if era >= 1 => 2,
            _ => 1,
        }
    }

    /// Lay a road of the given route level along a greedy shortest walk
    /// between two cities (traders build roads as they go in Civ 6, and a
    /// later-era route upgrades whatever it passes over, never downgrades).
    fn build_road(&mut self, from: Pos, to: Pos, level: u8) {
        let mut cur = from;
        for _ in 0..40 {
            if let Some(t) = self.map.tiles.get_mut(&cur) {
                if !self.rules.terrains[t.terrain].water {
                    t.road = t.road.max(level);
                }
            }
            if cur == to {
                break;
            }
            let next = self
                .nbrs(cur)
                .into_iter()
                .filter(|n| self.rules.is_passable(&self.map.tiles[n]))
                .min_by_key(|n| (self.wdist(*n, to), *n));
            match next {
                Some(n) if self.wdist(n, to) < self.wdist(cur, to) => cur = n,
                _ => break,
            }
        }
    }

    /// Reconstruct the deterministic path used when a route lays its road and
    /// return the strongest Panama/Golden Gate district-Gold multiplier it
    /// crosses. TradeRoute remains compact in saves while wonder behavior is
    /// still derived from the actual route geometry.
    pub(super) fn route_district_gold_multiplier(&self, from: Pos, to: Pos) -> f64 {
        let mut current = from;
        let mut multiplier: f64 = 1.0;
        for _ in 0..40 {
            if let Some(wonder) = self.map.tiles[&current].wonder.as_deref() {
                multiplier = multiplier.max(
                    self.rules.wonders[wonder]
                        .effects
                        .get("route_district_gold_multiplier")
                        .copied()
                        .unwrap_or(1.0),
                );
            }
            if current == to {
                break;
            }
            let Some(next) = self
                .nbrs(current)
                .into_iter()
                .filter(|position| self.rules.is_passable(&self.map.tiles[position]))
                .min_by_key(|position| (self.wdist(*position, to), *position))
            else {
                break;
            };
            current = next;
        }
        multiplier
    }

    /// The cities a route passes through on its way from `from` to `to`, in
    /// walking order, ending with the destination and never including the
    /// origin. The same greedy straight walk `route_district_gold_multiplier`
    /// uses: CIVVIS has no trader pathfinder, so this is the route as the crow
    /// flies over passable ground, which is what a Firaxis trader mostly does
    /// on land within range.
    fn route_path_cities(&self, from: Pos, to: Pos) -> Vec<u32> {
        let mut current = from;
        let mut cities = Vec::new();
        for _ in 0..40 {
            if current == to {
                break;
            }
            let Some(next) = self
                .nbrs(current)
                .into_iter()
                .filter(|position| self.rules.is_passable(&self.map.tiles[position]))
                .min_by_key(|position| (self.wdist(*position, to), *position))
            else {
                break;
            };
            current = next;
            if let Some(cid) = self.city_at(current) {
                if !cities.contains(&cid) {
                    cities.push(cid);
                }
            }
        }
        if let Some(dest) = self.city_at(to) {
            if !cities.contains(&dest) {
                cities.push(dest);
            }
        }
        cities
    }

    /// Gold a route earns from the Trading Posts it passes through, as the
    /// shipped rules pay it.
    ///
    /// Two host parameters and one trait, all read from the game database:
    /// `TRADING_POST_GOLD_IN_FOREIGN_CITY = 1` pays one Gold per foreign city
    /// on the route where the owner already holds a Trading Post — CIVVIS
    /// records those as `trading_post_city:<id>` when a route completes;
    /// `TRADING_POST_GOLD_IN_OWN_CITY = 0` pays nothing at home; and Rome's
    /// `TRAIT_GOLD_FROM_DOMESTIC_TRADING_POSTS`
    /// (`MODIFIER_PLAYER_ADJUST_TRADE_ROUTE_YIELD_PER_POST_IN_OWN_CITY`, 1 Gold)
    /// pays one Gold per OWN city on the route, every Roman city holding a
    /// post from founding (`TRAIT_FREE_TRADING_POSTS`). Both are
    /// `own_trading_post_route_gold` / `free_trading_posts` in `civs.json`.
    ///
    /// Measured before it was modelled, on live Rome/Trajan run
    /// civvis-20260816T011314Z: a domestic route Antium -> Rome read **+1 Gold**
    /// in the host and 0 in the model from the turn it opened (t33) to the turn
    /// it closed (t47), and Antium -> Arpinum through Ostia read **+2** — one per
    /// Roman city on the way. Nothing else in the route ledger moved.
    pub(super) fn trading_post_route_gold(&self, owner: usize, origin: u32, dest: u32) -> f64 {
        let (Some(origin_city), Some(dest_city)) =
            (self.cities.get(&origin), self.cities.get(&dest))
        else {
            return 0.0;
        };
        // `TRADING_POST_GOLD_IN_FOREIGN_CITY`, a shipped GlobalParameter.
        let foreign_post_gold = 1.0;
        let own_post_gold = self.civ_effect(owner, "own_trading_post_route_gold");
        // Where the host has told the mirror which posts its own path
        // crosses, that answer stands in for the walk below: the pathfinder
        // follows roads (Ostia -> Aquileia by way of Cumae, run
        // civvis-20260816T200454Z t144-154) and a straight line does not.
        if let Some(&(own, foreign)) = self.observed_route_posts.get(&(origin, dest)) {
            return own as f64 * own_post_gold + foreign as f64 * foreign_post_gold;
        }
        let free_own_posts = self.civ_effect(owner, "free_trading_posts") > 0.0;
        let mut gold = 0.0;
        for cid in self.route_path_cities(origin_city.pos, dest_city.pos) {
            let Some(city) = self.cities.get(&cid) else {
                continue;
            };
            if city.owner == owner {
                // A post at home pays only where the trait says so, and the
                // trait's cities all hold one from founding.
                if free_own_posts {
                    gold += own_post_gold;
                }
            } else if self.players[owner]
                .counters
                .contains_key(&format!("trading_post_city:{cid}"))
            {
                gold += foreign_post_gold;
            }
        }
        gold
    }

    /// Route upkeep at the owner's turn start: expire finished routes and
    /// hand the trader back to its origin city.
    pub(super) fn process_routes(&mut self, pid: usize) {
        let turn = self.turn;
        let expired: Vec<TradeRoute> = self
            .routes
            .iter()
            .filter(|r| r.owner == pid && turn >= r.ends)
            .cloned()
            .collect();
        // Only a route that ran its course counts as completed; the other
        // paths into `recall_trade_routes` are war, embargo and city transfer.
        let completed = expired.len() as i64;
        for route in &expired {
            self.note_completed_trade_route_moments(pid, route);
        }
        self.recall_trade_routes(expired);
        self.dedication_trigger(pid, "trade_route", completed);
    }

    pub(super) fn note_completed_trade_route_moments(&mut self, pid: usize, route: &TradeRoute) {
        let Some(destination_owner) = self.cities.get(&route.dest).map(|city| city.owner) else {
            return;
        };
        if destination_owner == pid
            || self.players[destination_owner].is_minor
            || self.players[destination_owner].is_barbarian
        {
            return;
        }
        self.players[pid]
            .counters
            .entry(format!("trading_post_city:{}", route.dest))
            .or_insert(1);
        let post_key = format!("trading_post_in:{destination_owner}");
        let added = match self.players[pid].counters.entry(post_key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(1);
                true
            }
            std::collections::btree_map::Entry::Occupied(_) => false,
        };
        if added {
            self.add_historic_moment(pid, "MOMENT_TRADING_POST_CONSTRUCTED_IN_OTHER_CIV");
        }
        let every_other_major = self
            .players
            .iter()
            .filter(|player| {
                player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
            })
            .all(|player| {
                self.players[pid]
                    .counters
                    .contains_key(&format!("trading_post_in:{}", player.id))
            });
        if every_other_major {
            self.first_historic_moment(
                pid,
                "trading_posts_in_every_civilization",
                Some("MOMENT_TRADING_POST_CONSTRUCTED_IN_EVERY_CIV"),
                Some("MOMENT_TRADING_POST_CONSTRUCTED_IN_EVERY_CIV_FIRST_IN_WORLD"),
            );
        }
    }

    /// Cancel routes without destroying their Traders. Normal completion,
    /// war, an embargo, and a city ownership change all return each Trader to
    /// its origin when possible, or another city still owned by that player.
    pub(super) fn recall_trade_routes(&mut self, recalled: Vec<TradeRoute>) {
        self.routes.retain(|route| !recalled.contains(route));
        for route in recalled {
            let city = self
                .cities
                .get(&route.origin)
                .filter(|city| city.owner == route.owner)
                .map(|city| city.id)
                .or_else(|| self.player_city_ids(route.owner).first().copied());
            if let Some(city) = city {
                self.place_new_unit("trader", route.owner, self.cities[&city].pos);
            }
        }
    }

    pub(super) fn cancel_routes_with(&mut self, a: usize, b: usize) {
        let recalled = self
            .routes
            .iter()
            .filter(|r| {
                let downer = self.cities.get(&r.dest).map(|c| c.owner);
                (r.owner == a && downer == Some(b))
                    || (r.owner == b && downer == Some(a))
                    || !self.cities.contains_key(&r.origin)
                    || downer.is_none()
            })
            .cloned()
            .collect();
        self.recall_trade_routes(recalled);
    }

    // -------------------------------------------------- religion

    pub(super) const RELIGION_NAMES: [&'static str; 51] = [
        "Buddhism",
        "Christianity",
        "Confucianism",
        "Hinduism",
        "Islam",
        "Judaism",
        "Protestantism",
        "Shinto",
        "Taoism",
        "Zoroastrianism",
        "Jainism",
        "Sikhism",
        "Bon",
        "Tengrism",
        "Manichaeism",
        "Mithraism",
        "Druidism",
        "Asatru",
        "Vodun",
        "Candomble",
        "Santeria",
        "Bahai",
        "Yazidism",
        "Mandaeism",
        "Samaritanism",
        "Rastafari",
        "Cao Dai",
        "Tenrikyo",
        "Hermeticism",
        "Gnosticism",
        "Arianism",
        "Nestorianism",
        "Catharism",
        "Bogomilism",
        "Sufism",
        "Ismailism",
        "Alevism",
        "Druzism",
        "Orthodoxy",
        "Catholicism",
        "Lutheranism",
        "Calvinism",
        "Anglicanism",
        "Methodism",
        "Quakerism",
        "Unitarianism",
        "Theravada",
        "Mahayana",
        "Vajrayana",
        "Animism",
        "Shaivism",
    ];
}

#[cfg(test)]
mod tests;
