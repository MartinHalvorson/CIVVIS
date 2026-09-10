//! The planning board contains observations, never authoritative hidden state.
//! Keep this inside Game so derived indexes can be rebuilt after redaction.
use super::*;

impl Game {
    /// A disposable decision board. Hidden terrain uses the same explicit
    /// unknown/prior representation as the Civilization VI mirror. Last-seen
    /// cities and terrain come from the observer's memory, not today's world.
    pub fn player_decision_view(&self, pid: usize) -> Self {
        let visible = self.player_visibility(pid);
        let viewers = self.visibility_viewers(pid);
        let mut tiles = BTreeMap::new();
        let mut remembered = BTreeMap::new();
        for viewer in viewers {
            for (pos, tile) in self.players[viewer].remembered_tiles.iter() {
                let stamp = self.players[viewer].remembered_tiles.seen_turn(pos);
                if tiles.get(pos).is_none_or(|(old, _)| stamp >= *old) {
                    tiles.insert(*pos, (stamp, tile.clone()));
                }
            }
            for (id, city) in &self.players[viewer].remembered_cities {
                if remembered
                    .get(id)
                    .is_none_or(|old: &RememberedCity| city.seen_turn >= old.seen_turn)
                {
                    remembered.insert(*id, city.clone());
                }
            }
        }
        for city in self.cities.values().filter(|c| visible.contains(&c.pos)) {
            remembered.insert(city.id, self.remember_city(city));
        }
        remembered
            .retain(|id, city| !visible.contains(&city.pos) || self.city_at(city.pos) == Some(*id));
        let mut view = self.clone();
        Arc::make_mut(&mut view.rules).enable_unknown_terrain();
        for tile in view.map.tiles.values_mut() {
            let pos = tile.pos;
            if visible.contains(&pos) {
                // Strategic resources below our discovery technology must not
                // become visible merely because the model knows their type.
                if let Some(resource) = tile.resource {
                    if !self.resource_visible_to(pid, resource.as_str()) {
                        tile.resource = None;
                    }
                }
            } else if let Some((_, known)) = tiles.get(&pos) {
                *tile = known.tile.clone();
            } else {
                *tile = Tile::new(pos);
                tile.terrain = crate::name!("unknown");
                // A probe is an assumption, never knowledge of hidden terrain.
            }
        }
        view.units.retain(|id, unit| {
            unit.owner == pid || (visible.contains(&unit.pos) && self.unit_visible_to(*id, pid))
        });
        let own_cities: BTreeSet<u32> = self
            .cities
            .values()
            .filter(|c| c.owner == pid)
            .map(|c| c.id)
            .collect();
        view.cities.clear();
        for id in &own_cities {
            view.cities.insert(*id, self.cities[id].clone());
        }
        for city in remembered.values().filter(|c| c.owner != pid) {
            view.cities.insert(city.id, public_city(city));
            Arc::make_mut(&mut view.observed_city_max_wall_hp).insert(city.id, city.wall_max);
            if visible.contains(&city.pos) {
                Arc::make_mut(&mut view.observed_city_strength)
                    .insert(city.id, self.city_strength(city.id));
            }
        }
        // A visible border does not disclose its city's location. Preserve
        // the passage/loyalty constraint without a dangling city reference.
        for tile in view.map.tiles.values_mut() {
            if tile
                .owner_city
                .is_some_and(|id| !view.cities.contains_key(&id))
            {
                let owner = if visible.contains(&tile.pos) {
                    tile.owner_city
                        .and_then(|id| self.cities.get(&id))
                        .map(|c| c.owner)
                } else {
                    tiles
                        .get(&tile.pos)
                        .and_then(|(_, remembered)| remembered.owner)
                };
                tile.owner_city = None;
                if let Some(owner) = owner.filter(|owner| *owner != pid) {
                    if !self.players[owner].is_minor && !self.players[owner].is_barbarian {
                        view.unseen_major_borders.insert(tile.pos);
                    }
                    if !self.is_at_war(pid, owner) && !self.has_open_borders(pid, owner) {
                        view.closed_borders.insert(tile.pos);
                    }
                }
            }
        }
        for other in 0..view.players.len() {
            if other == pid {
                continue;
            }
            let source = &self.players[other];
            let known = self.has_met(pid, other) || source.is_barbarian;
            let mut player = Player::new(
                other,
                if known { &source.civ } else { CIV_NAMES[0] },
                source.is_minor,
            );
            player.is_barbarian = source.is_barbarian;
            player.is_free_city = source.is_free_city;
            if known {
                player.alive = source.alive;
                player.team = source.team;
                player.government = source.government.clone();
                player.borders_enforced = Some(self.enforces_borders(other));
                player.gold = source.gold;
                player.gold_per_turn = source.gold_per_turn;
                player.age = source.age.clone();
                player.pantheon = source.pantheon.clone();
                player.religion = source.religion.clone();
                player.religion_beliefs = source.religion_beliefs.clone();
                player.envoys = source
                    .envoys
                    .iter()
                    .filter(|(minor, _)| self.has_met(pid, *minor))
                    .copied()
                    .collect();
                player.dvp = source.dvp;
                player.diplomatic_favor = source.diplomatic_favor;
                // The victory panel exposes tourist counts, not the exact
                // hidden accumulators behind their next threshold. Counts
                // are provided through observed_public_empire_stats below.
                player.science_projects = source.science_projects.clone();
                player.exoplanet_distance = source.exoplanet_distance;
                // These are bilateral facts known by the observing party.
                player.friends_until = source
                    .friends_until
                    .iter()
                    .filter(|(p, _)| **p == pid)
                    .map(|(p, v)| (*p, *v))
                    .collect();
                player.open_borders_until = source
                    .open_borders_until
                    .iter()
                    .filter(|(p, _)| **p == pid)
                    .map(|(p, v)| (*p, *v))
                    .collect();
                player.alliances = source
                    .alliances
                    .iter()
                    .filter(|(p, _)| **p == pid)
                    .map(|(p, v)| (*p, v.clone()))
                    .collect();
            }
            view.players[other] = player;
            if known {
                Arc::make_mut(&mut view.observed_military_power)
                    .insert(other, self.military_power(other));
                Arc::make_mut(&mut view.observed_score).insert(other, self.score(other));
                Arc::make_mut(&mut view.observed_public_empire_stats).insert(
                    other,
                    ObservedPublicEmpireStats {
                        city_count: Some(self.player_city_ids(other).len()),
                        techs: Some(source.techs.len()),
                        civics: Some(source.civics.len()),
                        foreign_tourists: Some(self.foreign_tourists(other).max(0) as usize),
                        domestic_tourists: Some(self.domestic_tourists(other).max(0) as usize),
                        science_victory_points: Some(self.science_victory_points(other)),
                        science_victory_points_needed: Some(
                            self.science_victory_points_needed(other),
                        ),
                        science_victory_points_per_turn: Some(self.exoplanet_speed(other)),
                        ..Default::default()
                    },
                );
            }
        }
        view.spies.retain(|_, spy| spy.owner == pid);
        view.routes.retain(|route| route.owner == pid);
        view.pending_deals
            .retain(|deal| deal.from == pid || deal.to == pid);
        view.active_trade_deals
            .retain(|deal| deal.from == pid || deal.to == pid);
        view.at_war.retain(|(a, b)| {
            *a == pid || *b == pid || (self.has_met(pid, *a) && self.has_met(pid, *b))
        });
        view.wars.clear();
        view.concluded_wars.clear();
        view.siege = Default::default();
        view.unit_move_trails.clear();
        view.nuclear_strikes.clear();
        view.barb_camps.retain(|pos, _| visible.contains(pos));
        for cooldown in view.barb_camps.values_mut() {
            // A visible camp does not disclose its next spawn countdown.
            *cooldown = 0;
        }
        view.barb_naval_camps.retain(|pos| visible.contains(pos));
        view.barb_camp_guards.clear();
        view.barb_scout_homes.clear();
        view.barb_raider_homes.clear();
        view.barb_scout_targets.clear();
        view.barb_camp_targets.clear();
        view.barb_alerted_until.clear();
        view.peace_treaties
            .retain(|(a, b), _| *a == pid || *b == pid);
        if let Some(congress) = view.congress.as_mut() {
            for resolution in &mut congress.resolutions {
                resolution.ballots.retain(|seat, _| *seat == pid);
            }
        }
        view.next_deal_id = view
            .pending_deals
            .iter()
            .map(|d| d.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        view.storms.clear();
        view.droughts.clear();
        view.log = Default::default();
        view.events = Vec::new().into();
        // A simulator's future random stream is not observable in the game.
        view.seed = (self.turn as u64).wrapping_mul(0x9e3779b9) ^ pid as u64;
        view.rng = Rng::new(view.seed);
        // IDs are opaque execution handles. Preserve the allocator so actions
        // referring to a city founded earlier in this plan remain replayable.
        view.occ.clear();
        for unit in view.units.values() {
            view.occ.entry(unit.pos).or_default().push(unit.id);
        }
        view.city_by_pos = view.cities.values().map(|c| (c.pos, c.id)).collect();
        // Unrevealed resources are hidden even on remembered terrain.
        for tile in view.map.tiles.values_mut() {
            if tile
                .resource
                .is_some_and(|r| !self.resource_visible_to(pid, r.as_str()))
            {
                tile.resource = None;
            }
        }
        for tile in view.players[pid].remembered_tiles.values_mut() {
            if tile
                .tile
                .resource
                .is_some_and(|r| !self.resource_visible_to(pid, r.as_str()))
            {
                tile.tile.resource = None;
            }
        }
        view.grow_player_frontier(6);
        // The player can read actual yields in its city panels even when a
        // modifier depends on off-screen infrastructure. Preserve that reading
        // as a correction, just as the live mirror does, without revealing the
        // hidden infrastructure that caused it.
        for id in &own_cities {
            let observed = self.city_yields(*id);
            let modeled = view.city_yields(*id);
            let correction = Arc::make_mut(&mut view.observed_city_yield_adjustments)
                .entry(*id)
                .or_default();
            correction.food += observed.food - modeled.food;
            correction.production += observed.production - modeled.production;
            correction.science += observed.science - modeled.science;
            correction.culture += observed.culture - modeled.culture;
            correction.gold += observed.gold - modeled.gold;
            correction.faith += observed.faith - modeled.faith;
        }
        for other in 0..view.players.len() {
            if other == pid || !self.has_met(pid, other) {
                continue;
            }
            let mut observed = self.player_yield_extras(other);
            for id in self.player_city_ids(other) {
                observed.add(self.city_yields(id));
            }
            let mut modeled = view.player_yield_extras(other);
            for id in view.player_city_ids(other) {
                modeled.add(view.city_yields(id));
            }
            let correction = Arc::make_mut(&mut view.observed_yield_adjustments)
                .entry(other)
                .or_default();
            correction.science += observed.science - modeled.science;
            correction.culture += observed.culture - modeled.culture;
            if let Some(religion) = self.majority_religion_of(other) {
                Arc::make_mut(&mut view.observed_majority_religion)
                    .insert(other, religion.to_string());
            }
        }
        view.set_fog_memory(false);
        view
    }

    /// Domain-specific exploration priors, derived only from known terrain.
    pub fn grow_player_frontier(&mut self, depth: u32) {
        for tile in self
            .map
            .tiles
            .values_mut()
            .filter(|t| t.terrain == "unknown")
        {
            tile.assumed_traversable = false;
            tile.assumed_navigable = false;
        }
        for water in [false, true] {
            let mut seen: BTreeSet<Pos> = self
                .map
                .tiles
                .values()
                .filter(|t| {
                    !self.rules.is_unknown(t)
                        && self.rules.is_water(t) == water
                        && (!water || self.rules.is_passable(t))
                })
                .map(|t| t.pos)
                .collect();
            let mut edge: Vec<Pos> = seen.iter().copied().collect();
            for _ in 0..depth {
                let mut next = Vec::new();
                for pos in edge {
                    for neighbour in self.map.neighbors(pos) {
                        if seen.contains(&neighbour) {
                            continue;
                        }
                        if let Some(tile) = self.map.tiles.get_mut(&neighbour) {
                            if tile.terrain != "unknown" {
                                continue;
                            }
                            if water {
                                tile.assumed_navigable = true;
                            } else {
                                tile.assumed_traversable = true;
                            }
                            seen.insert(neighbour);
                            next.push(neighbour);
                        }
                    }
                }
                edge = next;
                if edge.is_empty() {
                    break;
                }
            }
        }
    }
}

fn public_city(c: &RememberedCity) -> City {
    City {
        id: c.id,
        name: c.name.clone(),
        owner: c.owner,
        pos: c.pos,
        pop: c.pop,
        food: 0.0,
        production: 0.0,
        production_progress: Default::default(),
        strategic_resource_commitments: Default::default(),
        border_culture: 0.0,
        hp: c.hp,
        buildings: Vec::new(),
        products: Vec::new(),
        building_eras: Default::default(),
        pillaged_buildings: Default::default(),
        districts: Default::default(),
        wonders: Default::default(),
        owned_tiles: vec![c.pos],
        queue: Vec::new(),
        original_owner: c.original_owner,
        is_capital: c.is_capital,
        struck: false,
        extra_strikes_used: 0,
        wall_hp: c.wall_hp,
        encampment_hp: c.encampment_hp,
        encampment_wall_hp: c.encampment_wall_hp,
        encampment_struck: false,
        encampment_extra_strikes_used: 0,
        encampment_last_attacked: 0,
        encampment_pillaged: c.encampment_pillaged,
        last_attacked: 0,
        pressure: c.religion.iter().map(|r| (r.clone(), 100.0)).collect(),
        atheist_pressure: 0.0,
        loyalty: 100.0,
        free_city_pressure: Default::default(),
        captured_from: c.captured_from,
        occupied_from: c.occupied_from,
        occupation_grievance: None,
        reactor_age: 0,
        great_person_foreign_route_gold: 0.0,
    }
}

#[cfg(test)]
mod tests;
