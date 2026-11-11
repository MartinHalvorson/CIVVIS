//! City helpers: what a city is worth, what it may hold and what it costs.
//!
//! Carved verbatim out of the `city helpers` section of the single
//! `impl Game` in `game.rs`.  Yields (`city_yields`, `city_yields_inner`,
//! `player_tile_yields`, the citizen plan), housing, amenities, appeal,
//! districts and their adjacency, national parks, rivers and dams, the
//! production catalogue and its prices, and the native competitions.
//!
//! This is the same inherent `impl Game`, in a child module: nothing about
//! the rules moved, only the text.  See `docs/VERSION_CONTROL.md`.

use super::*;

/// The citizen plan: the jobs a city's citizens work. See
/// `game/city/citizens.rs`.
mod citizens;
/// Native competitions: opening, closing and scoring them. See
/// `game/city/competitions.rs`.
mod competitions;
/// District sites: where a city may place a district or a wonder. See
/// `game/city/district_sites.rs`.
mod district_sites;
/// Districts: families, yields, housing and amenity, and adjacency. See
/// `game/city/districts.rs`.
mod districts;
/// Appeal and national parks. See `game/city/parks.rs`.
mod parks;
/// The production catalogue: what a city may produce, its costs, and what
/// blocks it. See `game/city/production.rs`.
mod production;
/// Rivers: river edges and components, dams, and the river Holy Site. See
/// `game/city/rivers.rs`.
mod rivers;
/// The two longest sources of a city's yields: its buildings and its trade
/// routes. See `game/city/yield_sources.rs`.
mod yield_sources;

impl Game {
    // -------------------------------------------------------- city helpers

    pub fn can_found_city(&self, uid: u32) -> bool {
        let u = &self.units[&uid];
        if self.players[u.owner].is_minor {
            return false;
        }
        // A site the host engine has already refused. Empty unless CIVVIS is driving
        // one; see `blocked_city_sites`.
        if self.blocked_city_sites.contains(&u.pos) {
            return false;
        }
        let t = &self.map.tiles[&u.pos];
        // Founding clears the centre tile's feature, so a Settler standing on
        // a natural wonder would erase it. Oasis likewise ships
        // `Settlement=false`; Civ VI blocks both sites outright.
        if self.rules.is_water(t)
            || !self.rules.is_passable(t)
            || self.tile_is_natural_wonder(t)
            || t.feature.as_deref() == Some("oasis")
        {
            return false;
        }
        for c in self.cities.values() {
            if self.wdist(c.pos, u.pos) < 4 {
                return false;
            }
        }
        // A tile can still name a city that has since been razed or removed
        // (the live gene screen of 2026-10-19 panicked here, seed 99001+ on
        // `prey-is-declared-on`'s wars); a city that is gone owns nothing.
        if let Some(owner) = t
            .owner_city
            .and_then(|oc| self.cities.get(&oc))
            .map(|city| city.owner)
        {
            if owner != u.owner {
                return false;
            }
        }
        true
    }

    /// Gathering Storm grants 100 HP of Outer Defenses per wall level.
    pub fn city_max_wall_hp(&self, city: &City) -> i32 {
        if let Some(max) = self.observed_city_max_wall_hp.get(&city.id) {
            return (*max).max(0);
        }
        let built: i32 = city
            .buildings
            .iter()
            .map(|building| self.rules.buildings[building].outer_defense)
            .sum();
        if self.tree_effect(city.owner, "urban_defenses") > 0.0 {
            built.max(400)
        } else {
            built
        }
    }

    /// Prefer a visible native city ranged strength. Otherwise use the owner
    /// history/model, with the base-three fallback when no ranged unit is known.
    pub fn city_ranged_strength(&self, cid: u32) -> f64 {
        if let Some(strength) = self.observed_city_ranged_strength.get(&cid) {
            return *strength;
        }
        let owner = self.cities[&cid].owner;
        let current = self
            .units
            .values()
            .filter(|u| u.owner == owner)
            .map(|u| self.rules.units[u.kind].ranged_strength)
            .fold(3.0, f64::max);
        let base = self.players[owner]
            .counters
            .get("strongest_ranged_built")
            .map(|v| *v as f64)
            .unwrap_or(current)
            .max(3.0);
        base + self.policy_effect(owner, "city_ranged")
    }

    pub fn city_strength(&self, cid: u32) -> f64 {
        if let Some(strength) = self.observed_city_strength.get(&cid) {
            return strength.max(0.0);
        }
        let city = &self.cities[&cid];
        let city_state_bonus = self.city_state_envoy_strength(city.owner);
        let current_best = self
            .units
            .values()
            .filter(|u| u.owner == city.owner)
            .map(|u| self.rules.units[u.kind].strength)
            .fold(20.0, f64::max);
        let strongest_built = self.players[city.owner]
            .counters
            .get("strongest_unit_built")
            .map(|v| *v as f64)
            .unwrap_or(current_best);
        let garrison = self
            .unit_ids_at(city.pos)
            .iter()
            .filter_map(|id| {
                let u = &self.units[id];
                (u.owner == city.owner && self.rules.units[u.kind].class == "military")
                    .then(|| self.unit_unembarked_strength(u) - city_state_bonus)
            })
            .fold(0.0, f64::max);
        let mut s = (strongest_built - 10.0).max(garrison).max(10.0);
        if city.wall_hp > 0 {
            // +3 combat strength per standing wall level (Civ 6)
            s += 3.0
                * city
                    .buildings
                    .iter()
                    .filter(|building| self.rules.buildings[building].outer_defense > 0)
                    .count() as f64;
        }
        s += 2.0 * self.city_defense_district_count(city) as f64;
        s += self.tile_defense_bonus(city.pos);
        if self.city_has_palace(city) {
            s += 3.0;
        }
        s += self.policy_effect(city.owner, "city_defense");
        s += self.governor_effect(city.owner, city.id, "garrison_strength");
        s += city_state_bonus;
        let damaged_penalty = (10.0 - city.hp.clamp(0, 200) as f64 / 20.0).round();
        (s - damaged_penalty).max(0.0)
    }

    pub fn encampment_strength(&self, cid: u32) -> f64 {
        let city = &self.cities[&cid];
        let pos = self
            .city_district_family_position(city, crate::name!("encampment"))
            .unwrap_or(city.pos);
        self.district_strength_from_health(cid, pos, city.encampment_hp, city.encampment_wall_hp)
    }

    /// A city cannot heal when every adjacent passable tile is occupied by a
    /// hostile combat unit or covered by hostile ZOC. Off-map and impassable
    /// neighbors count as sealed sides of the siege ring.
    pub(super) fn city_under_siege(&self, cid: u32) -> bool {
        let city = &self.cities[&cid];
        self.governor_effect(city.owner, cid, "city_siege_immunity") <= 0.0
            && self.district_under_siege(city.owner, city.pos)
    }

    pub(super) fn home_continent(&self, pid: usize) -> Option<usize> {
        self.observed_city_palaces
            .iter()
            .filter(|(_, current)| **current)
            .filter_map(|(id, _)| self.cities.get(id))
            .find(|city| city.owner == pid)
            .or_else(|| {
                self.cities.values().find(|city| {
                    !self.observed_city_palaces.contains_key(&city.id)
                        && city.is_capital
                        && city.original_owner == pid
                })
            })
            .and_then(|city| self.map.get(city.pos))
            .and_then(|tile| tile.continent)
    }

    /// Whether two plots share a continent (both known and equal); water and
    /// unknown ground are on no continent and never match.
    pub(super) fn same_continent(&self, a: Pos, b: Pos) -> bool {
        match (
            self.map.get(a).and_then(|tile| tile.continent),
            self.map.get(b).and_then(|tile| tile.continent),
        ) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    }

    pub(super) fn on_foreign_continent(&self, pid: usize, position: Pos) -> bool {
        self.home_continent(pid).is_some_and(|home| {
            self.map
                .get(position)
                .and_then(|tile| tile.continent)
                .is_some_and(|continent| continent != home)
        })
    }

    pub(super) fn city_has_building_family(&self, city: &City, family: impl AsName) -> bool {
        city.buildings
            .iter()
            .any(|building| self.building_is_family(building, family))
    }

    pub(crate) fn building_is_family(&self, building: impl AsName, family: impl AsName) -> bool {
        let (building, family) = (building.as_name(), family.as_name());
        building == family
            || self
                .rules
                .buildings
                .get_interned(building)
                .is_some_and(|spec| spec.replaces == Some(family))
    }

    pub(super) fn city_has_active_building_family(&self, city: &City, family: impl AsName) -> bool {
        city.buildings.iter().any(|building| {
            !city.pillaged_buildings.contains(building)
                && self.building_district_is_active(city, building)
                && self.building_is_family(building, family)
        })
    }

    /// Ground that pays nothing at all whoever holds it: a flooded lowland
    /// waiting on a Flood Barrier, a plot inside a nuclear accident's fallout,
    /// and a district that is placed but not yet built.
    pub(super) fn tile_pays_nothing(&self, tile: &crate::world::Tile) -> bool {
        tile.flooded || tile.fallout_until > self.turn || tile.district_foundation.is_some()
    }

    /// ★★★★★ WHAT THE GROUND ITSELF PAYS, before any owner's techs and civics.
    ///
    /// The ruleset's worked yield, a neighbouring feature's adjacency (Vesuvius,
    /// Yosemite), the permanent fertility a disaster left behind, and drought.
    /// None of it depends on who holds the plot — and every bit of it was
    /// invisible on a plot nobody held.
    ///
    /// [`Self::modeled_tile_yields`] used to hand an unowned tile straight to
    /// `Rules::worked_tile_yields`, which knows the catalogue and nothing else.
    /// A tile an eruption had enriched therefore read at its bare figure for
    /// every settle score, every Builder choice and every draw of the board
    /// until somebody's border finally reached it — and Volcanic Soil has no
    /// yields of its own, so the enrichment was the whole of what that plot was
    /// worth.
    pub(super) fn ground_tile_yields(&self, pos: Pos, tile: &crate::world::Tile) -> Yields {
        if self.tile_pays_nothing(tile) {
            return Yields::default();
        }
        let mut yields = self.rules.worked_tile_yields(tile);
        for neighbor in self.nbrs(pos) {
            if let Some(spec) = self.map.tiles[&neighbor]
                .feature
                .as_deref()
                .and_then(|feature| self.rules.features.get(feature))
            {
                yields.add(spec.adjacent_yields);
            }
        }
        yields.faith += tile.disaster_faith;
        // What a storm's silt left behind. Gathering Storm's disasters are a
        // trade, not a pure loss: the ground they wreck comes back richer.
        yields.food += tile.disaster_food;
        yields.production += tile.disaster_production;
        let drought_food_protected = tile
            .owner_city
            .and_then(|city| self.cities.get(&city))
            .is_some_and(|city| self.city_disaster_protected(city, "drought_protection"))
            || (!tile.pillaged
                && tile.improvement.as_deref().is_some_and(|improvement| {
                    self.rules.improvements[improvement]
                        .effects
                        .get("drought_food_protection")
                        .copied()
                        .unwrap_or(0.0)
                        > 0.0
                }));
        if tile.drought && !drought_food_protected {
            yields.food = (yields.food - 1.0).max(0.0);
        }
        yields
    }

    pub(super) fn player_tile_yields(
        &self,
        pid: usize,
        pos: Pos,
        tile: &crate::world::Tile,
    ) -> Yields {
        if self.tile_pays_nothing(tile) {
            return Yields::default();
        }
        // The ground first; everything below this line is the owner's own —
        // their techs, their civics, their improvements.
        let mut yields = self.ground_tile_yields(pos, tile);
        match tile.improvement.as_deref() {
            Some("mine") => yields.production += self.tree_effect(pid, "mine_production"),
            Some("pasture") => {
                yields.food += self.tree_effect(pid, "pasture_food");
                yields.production += self.tree_effect(pid, "pasture_production");
            }
            Some("quarry") => yields.production += self.tree_effect(pid, "quarry_production"),
            Some("plantation") => {
                yields.food += self.tree_effect(pid, "plantation_food");
                yields.gold += self.tree_effect(pid, "plantation_gold");
                let hacienda = &self.rules.improvements["hacienda"].effects;
                let adjacent_haciendas = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("hacienda")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                yields.production += if self.players[pid]
                    .civics
                    .contains(&crate::name!("rapid_deployment"))
                {
                    adjacent_haciendas
                        * hacienda
                            .get("adjacent_hacienda_production_after_rapid_deployment")
                            .copied()
                            .unwrap_or(0.0)
                } else {
                    (adjacent_haciendas / 2.0).floor()
                        * hacienda
                            .get("adjacent_hacienda_pair_production")
                            .copied()
                            .unwrap_or(0.0)
                };
            }
            Some("camp") => {
                yields.food += self.tree_effect(pid, "camp_food");
                yields.production += self.tree_effect(pid, "camp_production");
                yields.gold += self.tree_effect(pid, "camp_gold");
            }
            Some("fishing_boats") => {
                yields.food += self.tree_effect(pid, "fishing_boats_food");
                yields.gold += self.tree_effect(pid, "fishing_boats_gold");
                // Colonialism's +1 Production (Improvement_BonusYieldChanges,
                // the row a duplicate Id hid from the audit — see
                // civ6_fidelity.py TABLE_KEYS).
                yields.production += self.tree_effect(pid, "fishing_boats_production");
                // Each adjacent Seastead lifts its neighbouring Fishing Boats.
                yields.production += self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("seastead")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64
                    * self.rules.improvements["seastead"]
                        .effects
                        .get("adjacent_fishing_boats_production")
                        .copied()
                        .unwrap_or(0.0);
            }
            Some("seastead") => {
                let effects = &self.rules.improvements["seastead"].effects;
                let adjacent_boats = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("fishing_boats")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                yields.production += adjacent_boats
                    * effects
                        .get("adjacent_fishing_boats_production")
                        .copied()
                        .unwrap_or(0.0);
                let adjacent_reefs = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| self.map.tiles[neighbor].feature.as_deref() == Some("reef"))
                    .count() as f64;
                yields.culture +=
                    adjacent_reefs * effects.get("adjacent_reef_culture").copied().unwrap_or(0.0);
            }
            Some("lumber_mill") => {
                yields.production += self.tree_effect(pid, "lumber_mill_production");
            }
            Some("oil_well" | "offshore_oil_rig") => {
                yields.production += self.tree_effect(pid, "oil_improvement_production");
            }
            Some("farm") => {
                let adjacent_farms = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("farm")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                // Farms_MedievalAdjacency pays Feudalism +1 Food per TWO
                // adjacent Farms and carries ObsoleteTech
                // TECH_REPLACEABLE_PARTS; Farms_MechanizedAdjacency then pays
                // +1 per Farm. The second replaces the first rather than
                // stacking with it, exactly as Stirrups does to the Kurgan.
                let mechanized = self.tree_effect(pid, "farm_adjacency_food");
                if mechanized > 0.0 {
                    yields.food += adjacent_farms * mechanized;
                } else {
                    yields.food += (adjacent_farms / 2.0).floor()
                        * self.tree_effect(pid, "farm_pair_adjacency_food");
                }
            }
            Some("pairidaeza") => {
                let effects = &self.rules.improvements["pairidaeza"].effects;
                for neighbor in self.nbrs(pos) {
                    let adjacent = &self.map.tiles[&neighbor];
                    if self.city_at(neighbor).is_some() {
                        yields.gold += effects
                            .get("adjacent_city_center_gold")
                            .copied()
                            .unwrap_or(0.0);
                    }
                    if let Some(district) = adjacent.district {
                        if self.district_is_family(district, crate::name!("commercial_hub")) {
                            yields.gold += effects
                                .get("adjacent_commercial_hub_gold")
                                .copied()
                                .unwrap_or(0.0);
                        } else if self.district_is_family(district, crate::name!("holy_site")) {
                            yields.culture += effects
                                .get("adjacent_holy_site_culture")
                                .copied()
                                .unwrap_or(0.0);
                        } else if self.district_is_family(district, crate::name!("theater_square"))
                        {
                            yields.culture += effects
                                .get("adjacent_theater_square_culture")
                                .copied()
                                .unwrap_or(0.0);
                        }
                    }
                }
                if self.players[pid]
                    .civics
                    .contains(&crate::name!("diplomatic_service"))
                {
                    yields.culture += effects
                        .get("culture_after_diplomatic_service")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
            Some("sphinx") => {
                let effects = &self.rules.improvements["sphinx"].effects;
                yields.culture += self.tree_effect(pid, "sphinx_culture");
                if matches!(
                    tile.feature.as_deref(),
                    Some("floodplains" | "grassland_floodplains" | "plains_floodplains")
                ) {
                    yields.culture += effects.get("floodplains_culture").copied().unwrap_or(0.0);
                }
                if self
                    .nbrs(pos)
                    .iter()
                    .any(|neighbor| self.map.tiles[neighbor].wonder.is_some())
                {
                    yields.faith += effects.get("adjacent_wonder_faith").copied().unwrap_or(0.0);
                }
            }
            Some("kurgan") => {
                // +1 Faith per adjacent Pasture, which Stirrups obsoletes in
                // favour of a +2 rule rather than stacking with it.
                let effects = &self.rules.improvements["kurgan"].effects;
                let per_pasture = if self.players[pid].techs.contains(&crate::name!("stirrups")) {
                    effects
                        .get("adjacent_pasture_faith_after_stirrups")
                        .copied()
                        .unwrap_or(0.0)
                } else {
                    effects
                        .get("adjacent_pasture_faith")
                        .copied()
                        .unwrap_or(0.0)
                };
                yields.faith += per_pasture
                    * self
                        .nbrs(pos)
                        .iter()
                        .filter(|neighbor| {
                            self.map.tiles[neighbor].improvement.as_deref() == Some("pasture")
                                && !self.map.tiles[neighbor].pillaged
                        })
                        .count() as f64;
            }
            Some("mound") => {
                let effects = &self.rules.improvements["mound"].effects;
                let adjacent_districts = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| self.map.tiles[neighbor].district.is_some())
                    .count() as f64;
                if self.players[pid]
                    .techs
                    .contains(&crate::name!("replaceable_parts"))
                {
                    yields.food += adjacent_districts
                        * effects
                            .get("adjacent_district_food_after_replaceable_parts")
                            .copied()
                            .unwrap_or(0.0);
                } else if self.players[pid]
                    .civics
                    .contains(&crate::name!("feudalism"))
                {
                    yields.food += (adjacent_districts / 2.0).floor()
                        * effects
                            .get("adjacent_district_pair_food_after_feudalism")
                            .copied()
                            .unwrap_or(0.0);
                }
            }
            Some("monastery") => {
                let effects = &self.rules.improvements["monastery"].effects;
                let adjacent_districts = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].district.is_some()
                            || self.city_at(**neighbor).is_some()
                    })
                    .count() as f64;
                yields.faith += (adjacent_districts / 2.0).floor()
                    * effects
                        .get("adjacent_district_pair_faith")
                        .copied()
                        .unwrap_or(0.0);
            }
            Some("colossal_head") => {
                let effects = &self.rules.improvements["colossal_head"].effects;
                let adjacent_woods_rainforest = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        matches!(
                            self.map.tiles[neighbor].feature.as_deref(),
                            Some("forest" | "jungle")
                        )
                    })
                    .count() as f64;
                yields.faith += if self.players[pid].civics.contains(&crate::name!("humanism")) {
                    adjacent_woods_rainforest
                        * effects
                            .get("adjacent_woods_rainforest_faith_after_humanism")
                            .copied()
                            .unwrap_or(0.0)
                } else {
                    (adjacent_woods_rainforest / 2.0).floor()
                        * effects
                            .get("adjacent_woods_rainforest_pair_faith")
                            .copied()
                            .unwrap_or(0.0)
                };
            }
            Some("mahavihara") => {
                let effects = &self.rules.improvements["mahavihara"].effects;
                let campus_count = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].district.is_some_and(|district| {
                            self.district_is_family(district, crate::name!("campus"))
                        })
                    })
                    .count() as f64;
                let holy_site_count = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].district.is_some_and(|district| {
                            self.district_is_family(district, crate::name!("holy_site"))
                        })
                    })
                    .count() as f64;
                let campus_key = if self.players[pid]
                    .techs
                    .contains(&crate::name!("scientific_theory"))
                {
                    "adjacent_campus_science_after_scientific_theory"
                } else {
                    "adjacent_campus_science"
                };
                yields.science += campus_count * effects.get(campus_key).copied().unwrap_or(0.0);
                yields.faith += holy_site_count
                    * effects
                        .get("adjacent_holy_site_faith")
                        .copied()
                        .unwrap_or(0.0);
            }
            Some("moai") => {
                let effects = &self.rules.improvements["moai"].effects;
                let adjacent_moai = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("moai")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                yields.culture += if self.players[pid]
                    .civics
                    .contains(&crate::name!("medieval_faires"))
                {
                    adjacent_moai
                        * effects
                            .get("adjacent_moai_culture_after_medieval_faires")
                            .copied()
                            .unwrap_or(0.0)
                } else {
                    (adjacent_moai / 2.0).floor()
                        * effects
                            .get("adjacent_moai_pair_culture")
                            .copied()
                            .unwrap_or(0.0)
                };
                if tile.feature.as_deref() == Some("volcanic_soil")
                    || self.nbrs(pos).iter().any(|neighbor| {
                        self.map.tiles[neighbor].feature.as_deref() == Some("volcanic_soil")
                    })
                {
                    yields.culture += effects
                        .get("volcanic_adjacent_or_on_culture")
                        .copied()
                        .unwrap_or(0.0);
                }
                if self.nbrs(pos).iter().any(|neighbor| {
                    matches!(self.map.tiles[neighbor].terrain.as_str(), "coast" | "lake")
                }) {
                    yields.culture += effects
                        .get("adjacent_coast_lake_culture")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
            Some("mekewap") => {
                let effects = &self.rules.improvements["mekewap"].effects;
                let adjacent_resource_count = |class: &str| {
                    self.nbrs(pos)
                        .iter()
                        .filter(|neighbor| {
                            self.map.tiles[neighbor]
                                .resource
                                .as_ref()
                                .is_some_and(|resource| {
                                    self.rules
                                        .resources
                                        .get(resource.as_str())
                                        .is_some_and(|spec| spec.class == class)
                                })
                        })
                        .count() as f64
                };
                let bonus = adjacent_resource_count("bonus");
                let luxury = adjacent_resource_count("luxury");
                if self.players[pid]
                    .civics
                    .contains(&crate::name!("conservation"))
                {
                    yields.food += bonus
                        * effects
                            .get("adjacent_bonus_food_after_conservation")
                            .copied()
                            .unwrap_or(0.0);
                } else {
                    yields.food += (bonus / 2.0).floor()
                        * effects
                            .get("adjacent_bonus_pair_food")
                            .copied()
                            .unwrap_or(0.0);
                }
                if luxury > 0.0 {
                    yields.gold += effects.get("adjacent_luxury_gold").copied().unwrap_or(0.0);
                }
                if self.players[pid]
                    .techs
                    .contains(&crate::name!("cartography"))
                {
                    yields.gold += luxury
                        * effects
                            .get("adjacent_luxury_gold_after_cartography")
                            .copied()
                            .unwrap_or(0.0);
                }
                if self.players[pid]
                    .civics
                    .contains(&crate::name!("civil_service"))
                {
                    yields.production += effects
                        .get("production_after_civil_service")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
            Some("trading_dome") => {
                let adjacent_luxuries = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor]
                            .resource
                            .as_ref()
                            .is_some_and(|resource| {
                                self.rules.resources[resource].class == "luxury"
                            })
                    })
                    .count() as f64;
                yields.gold += adjacent_luxuries
                    * self.rules.improvements["trading_dome"]
                        .effects
                        .get("adjacent_luxury_gold")
                        .copied()
                        .unwrap_or(0.0);
            }
            Some("batey") => {
                let effects = &self.rules.improvements["batey"].effects;
                let late = self.players[pid]
                    .civics
                    .contains(&crate::name!("exploration"));
                let bonus_resources = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor]
                            .resource
                            .as_ref()
                            .filter(|resource| self.resource_visible_to(pid, resource))
                            .and_then(|resource| self.rules.resources.get(resource.as_str()))
                            .is_some_and(|resource| resource.class == "bonus")
                    })
                    .count() as f64;
                let entertainment_complexes = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].district.is_some_and(|district| {
                            self.district_is_family(district, crate::name!("entertainment_complex"))
                        })
                    })
                    .count() as f64;
                let bonus_key = if late {
                    "adjacent_bonus_culture_after_exploration"
                } else {
                    "adjacent_bonus_culture"
                };
                let district_key = if late {
                    "adjacent_entertainment_complex_culture_after_exploration"
                } else {
                    "adjacent_entertainment_complex_culture"
                };
                yields.culture += bonus_resources * effects.get(bonus_key).copied().unwrap_or(0.0)
                    + entertainment_complexes * effects.get(district_key).copied().unwrap_or(0.0);
            }
            Some("rock_hewn_church") => {
                let effects = &self.rules.improvements["rock_hewn_church"].effects;
                yields.faith += self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        let adjacent = &self.map.tiles[neighbor];
                        adjacent.hills || adjacent.terrain == "mountain"
                    })
                    .count() as f64
                    * effects
                        .get("adjacent_hill_mountain_faith")
                        .copied()
                        .unwrap_or(0.0);
            }
            Some("ziggurat") => {
                yields.culture += self.tree_effect(pid, "ziggurat_culture");
                if tile.has_river() {
                    yields.culture += self.rules.improvements["ziggurat"]
                        .effects
                        .get("river_culture")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
            Some("great_wall") => {
                let adjacent = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("great_wall")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                yields.culture += adjacent * self.tree_effect(pid, "great_wall_culture_adjacency");
                yields.gold += adjacent
                    * self.rules.improvements["great_wall"]
                        .effects
                        .get("adjacent_great_wall_gold")
                        .copied()
                        .unwrap_or(0.0);
            }
            Some("chateau") => {
                let effects = &self.rules.improvements["chateau"].effects;
                let wonders = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| self.map.tiles[neighbor].wonder.is_some())
                    .count() as f64;
                let wonder_key = if self.players[pid].techs.contains(&crate::name!("flight")) {
                    "adjacent_wonder_culture_after_flight"
                } else {
                    "adjacent_wonder_culture"
                };
                yields.culture += wonders * effects.get(wonder_key).copied().unwrap_or(0.0);
                if tile.has_river() {
                    yields.gold += effects.get("river_gold").copied().unwrap_or(0.0);
                }
            }
            Some("golf_course") => {
                let effects = &self.rules.improvements["golf_course"].effects;
                for neighbor in self.nbrs(pos) {
                    let adjacent = &self.map.tiles[&neighbor];
                    if self.city_at(neighbor).is_some() {
                        yields.culture += effects
                            .get("adjacent_city_center_culture")
                            .copied()
                            .unwrap_or(0.0);
                    }
                    if adjacent.district.is_some_and(|district| {
                        self.district_is_family(district, crate::name!("entertainment_complex"))
                    }) {
                        yields.culture += effects
                            .get("adjacent_entertainment_complex_culture")
                            .copied()
                            .unwrap_or(0.0);
                    }
                }
            }
            Some("hacienda") => {
                let effects = &self.rules.improvements["hacienda"].effects;
                let adjacent_plantations = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("plantation")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                yields.food += if self.players[pid]
                    .techs
                    .contains(&crate::name!("replaceable_parts"))
                {
                    adjacent_plantations
                        * effects
                            .get("adjacent_plantation_food_after_replaceable_parts")
                            .copied()
                            .unwrap_or(0.0)
                } else {
                    (adjacent_plantations / 2.0).floor()
                        * effects
                            .get("adjacent_plantation_pair_food")
                            .copied()
                            .unwrap_or(0.0)
                };
                let adjacent_haciendas = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("hacienda")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                yields.production += if self.players[pid]
                    .civics
                    .contains(&crate::name!("rapid_deployment"))
                {
                    adjacent_haciendas
                        * effects
                            .get("adjacent_hacienda_production_after_rapid_deployment")
                            .copied()
                            .unwrap_or(0.0)
                } else {
                    (adjacent_haciendas / 2.0).floor()
                        * effects
                            .get("adjacent_hacienda_pair_production")
                            .copied()
                            .unwrap_or(0.0)
                };
            }
            Some("ice_hockey_rink") => {
                let effects = &self.rules.improvements["ice_hockey_rink"].effects;
                let adjacent_tundra_snow = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        matches!(self.map.tiles[neighbor].terrain.as_str(), "tundra" | "snow")
                    })
                    .count() as f64;
                yields.culture += adjacent_tundra_snow
                    * effects
                        .get("adjacent_tundra_snow_culture")
                        .copied()
                        .unwrap_or(0.0);
                let stadiums = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        let adjacent = &self.map.tiles[neighbor];
                        adjacent.district.is_some_and(|district| {
                            self.district_is_family(district, crate::name!("entertainment_complex"))
                        }) && adjacent
                            .owner_city
                            .and_then(|city_id| self.cities.get(&city_id))
                            .is_some_and(|city| {
                                self.city_has_active_building_family(city, crate::name!("stadium"))
                            })
                    })
                    .count() as f64;
                yields.culture += stadiums
                    * effects
                        .get("adjacent_stadium_culture")
                        .copied()
                        .unwrap_or(0.0);
                if self.players[pid]
                    .civics
                    .contains(&crate::name!("professional_sports"))
                {
                    yields.food += effects
                        .get("food_after_professional_sports")
                        .copied()
                        .unwrap_or(0.0);
                    yields.production += effects
                        .get("production_after_professional_sports")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
            Some("kampung") => {
                let effects = &self.rules.improvements["kampung"].effects;
                let boats = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("fishing_boats")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                yields.food += boats
                    * effects
                        .get("adjacent_fishing_boats_food")
                        .copied()
                        .unwrap_or(0.0);
                if self.players[pid]
                    .civics
                    .contains(&crate::name!("civil_engineering"))
                {
                    yields.production += effects
                        .get("production_after_civil_engineering")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
            Some("mission") => {
                let effects = &self.rules.improvements["mission"].effects;
                if self.on_foreign_continent(pid, pos) {
                    yields.faith += effects
                        .get("foreign_continent_faith")
                        .copied()
                        .unwrap_or(0.0);
                    yields.food += effects
                        .get("foreign_continent_food")
                        .copied()
                        .unwrap_or(0.0);
                    yields.production += effects
                        .get("foreign_continent_production")
                        .copied()
                        .unwrap_or(0.0);
                }
                let adjacent_religious_science = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].district.is_some_and(|district| {
                            self.district_is_family(district, crate::name!("campus"))
                                || self.district_is_family(district, crate::name!("holy_site"))
                        })
                    })
                    .count() as f64;
                yields.science += adjacent_religious_science
                    * effects
                        .get("adjacent_campus_holy_site_science")
                        .copied()
                        .unwrap_or(0.0);
                if self.players[pid]
                    .civics
                    .contains(&crate::name!("cultural_heritage"))
                {
                    yields.science += effects
                        .get("science_after_cultural_heritage")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
            Some("open_air_museum") => {
                let effects = &self.rules.improvements["open_air_museum"].effects;
                let terrain_count = tile
                    .owner_city
                    .and_then(|city_id| self.cities.get(&city_id))
                    .map(|city| {
                        city.owned_tiles
                            .iter()
                            .map(|position| self.map.tiles[position].terrain)
                            .collect::<BTreeSet<_>>()
                            .len() as f64
                    })
                    .unwrap_or(0.0);
                yields.culture += terrain_count
                    * effects
                        .get("city_distinct_terrain_culture")
                        .copied()
                        .unwrap_or(0.0);
            }
            Some("outback_station") => {
                let effects = &self.rules.improvements["outback_station"].effects;
                let adjacent_pastures = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("pasture")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                yields.food += adjacent_pastures
                    * effects.get("adjacent_pasture_food").copied().unwrap_or(0.0);
                let adjacent_outbacks = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("outback_station")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                if self.players[pid]
                    .techs
                    .contains(&crate::name!("steam_power"))
                {
                    yields.production += (adjacent_outbacks / 2.0).floor()
                        * effects
                            .get("adjacent_outback_pair_production_after_steam_power")
                            .copied()
                            .unwrap_or(0.0);
                }
                if self.players[pid]
                    .civics
                    .contains(&crate::name!("rapid_deployment"))
                {
                    yields.food += (adjacent_outbacks / 2.0).floor()
                        * effects
                            .get("adjacent_outback_pair_food_after_rapid_deployment")
                            .copied()
                            .unwrap_or(0.0);
                }
            }
            Some("polder") => {
                let effects = &self.rules.improvements["polder"].effects;
                let adjacent_polders = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("polder")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                if self.players[pid]
                    .techs
                    .contains(&crate::name!("replaceable_parts"))
                {
                    yields.food += adjacent_polders
                        * effects
                            .get("adjacent_polder_food_after_replaceable_parts")
                            .copied()
                            .unwrap_or(0.0);
                    yields.production += adjacent_polders
                        * effects
                            .get("adjacent_polder_production_after_replaceable_parts")
                            .copied()
                            .unwrap_or(0.0);
                } else {
                    yields.food += adjacent_polders
                        * effects.get("adjacent_polder_food").copied().unwrap_or(0.0);
                }
                if self.players[pid]
                    .civics
                    .contains(&crate::name!("civil_engineering"))
                {
                    yields.gold += effects
                        .get("gold_after_civil_engineering")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
            Some("stepwell") => {
                let effects = &self.rules.improvements["stepwell"].effects;
                for neighbor in self.nbrs(pos) {
                    let adjacent = &self.map.tiles[&neighbor];
                    if adjacent.district.is_some_and(|district| {
                        self.district_is_family(district, crate::name!("holy_site"))
                    }) {
                        yields.faith += effects
                            .get("adjacent_holy_site_faith")
                            .copied()
                            .unwrap_or(0.0);
                    }
                    if adjacent.improvement.as_deref() == Some("farm") && !adjacent.pillaged {
                        yields.food += effects.get("adjacent_farm_food").copied().unwrap_or(0.0);
                    }
                }
                if self.players[pid]
                    .civics
                    .contains(&crate::name!("feudalism"))
                {
                    yields.faith += effects.get("faith_after_feudalism").copied().unwrap_or(0.0);
                }
                if self.players[pid]
                    .civics
                    .contains(&crate::name!("professional_sports"))
                {
                    yields.food += effects
                        .get("food_after_professional_sports")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
            Some("terrace_farm") => {
                let effects = &self.rules.improvements["terrace_farm"].effects;
                let adjacent_mountains = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| self.map.tiles[neighbor].terrain == "mountain")
                    .count() as f64;
                yields.food += adjacent_mountains
                    * effects
                        .get("adjacent_mountain_food")
                        .copied()
                        .unwrap_or(0.0);
                if self.nbrs(pos).iter().any(|neighbor| {
                    self.map.tiles[neighbor].district.is_some_and(|district| {
                        self.district_is_family(district, crate::name!("aqueduct"))
                    })
                }) {
                    yields.production += effects
                        .get("adjacent_aqueduct_production")
                        .copied()
                        .unwrap_or(0.0);
                } else if tile.has_river()
                    || self.nbrs(pos).iter().any(|neighbor| {
                        matches!(self.map.tiles[neighbor].terrain.as_str(), "lake")
                            || self.map.tiles[neighbor].feature.as_deref() == Some("oasis")
                    })
                {
                    yields.production += effects
                        .get("fresh_water_production")
                        .copied()
                        .unwrap_or(0.0);
                }
                let adjacent_farms = self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.tiles[neighbor].improvement.as_deref() == Some("farm")
                            && !self.map.tiles[neighbor].pillaged
                    })
                    .count() as f64;
                yields.food += if self.players[pid]
                    .techs
                    .contains(&crate::name!("replaceable_parts"))
                {
                    adjacent_farms
                        * effects
                            .get("adjacent_farm_food_after_replaceable_parts")
                            .copied()
                            .unwrap_or(0.0)
                } else {
                    (adjacent_farms / 2.0).floor()
                        * effects
                            .get("adjacent_farm_pair_food")
                            .copied()
                            .unwrap_or(0.0)
                };
            }
            _ => {}
        }
        // Nazca Lines deliberately have no workable yield of their own. Their
        // value is the pattern they create around them, so apply every active
        // neighbouring line to this tile rather than teaching the governor to
        // work an unworkable geoglyph.
        if !tile.pillaged {
            for neighbor in self.nbrs(pos) {
                let adjacent = &self.map.tiles[&neighbor];
                if adjacent.improvement.as_deref() != Some("nazca_line")
                    || adjacent.pillaged
                    || !adjacent
                        .owner_city
                        .and_then(|city_id| self.cities.get(&city_id))
                        .is_some_and(|city| city.owner == pid)
                {
                    continue;
                }
                let effects = &self.rules.improvements["nazca_line"].effects;
                yields.faith += effects.get("adjacent_tile_faith").copied().unwrap_or(0.0);
                if tile.resource.is_some() {
                    yields.faith += effects
                        .get("adjacent_resource_faith")
                        .copied()
                        .unwrap_or(0.0);
                }
                if tile.terrain == "desert"
                    && self.players[pid]
                        .civics
                        .contains(&crate::name!("civil_service"))
                {
                    yields.food += effects
                        .get("adjacent_desert_food_after_civil_service")
                        .copied()
                        .unwrap_or(0.0);
                }
                if !self.rules.is_water(tile)
                    && !tile.hills
                    && self.players[pid]
                        .techs
                        .contains(&crate::name!("mass_production"))
                {
                    yields.production += effects
                        .get("adjacent_flat_production_after_mass_production")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
        }
        if !tile.pillaged {
            if let Some(improvement) = tile.improvement.as_deref() {
                yields.science += self.rules.improvements[improvement]
                    .effects
                    .get("appeal_science_percent")
                    .copied()
                    .unwrap_or(0.0)
                    / 100.0
                    * self.tile_appeal(pos).max(0) as f64;
                yields.culture += self.rules.improvements[improvement]
                    .effects
                    .get("appeal_culture_percent")
                    .copied()
                    .unwrap_or(0.0)
                    / 100.0
                    * self.tile_appeal(pos).max(0) as f64;
                yields.gold += self.rules.improvements[improvement]
                    .effects
                    .get("appeal_gold")
                    .copied()
                    .unwrap_or(0.0)
                    * self.tile_appeal(pos).max(0) as f64;
                yields.faith += self.rules.improvements[improvement]
                    .effects
                    .get("appeal_faith")
                    .copied()
                    .unwrap_or(0.0)
                    * self.tile_appeal(pos).max(0) as f64;
            }
        }

        let Some(city) = tile
            .owner_city
            .and_then(|city_id| self.cities.get(&city_id))
            .filter(|city| city.owner == pid)
        else {
            return yields;
        };
        // ★ Lady of the Reeds and Marshes, Goddess of Fire and Earth Goddess
        // are ONE modifier over three plot tests — every one of them is
        // `MODIFIER_CITY_PLOT_YIELDS_ADJUST_PLOT_YIELD` with a
        // `REQUIREMENTSET_TEST_ANY` subject and a `CITY_FOLLOWS_PANTHEON`
        // owner — so the engine asks the plot once and the belief supplies
        // the amount. Read from the Gathering Storm install
        // (`DLC/Expansion2/Data/Expansion2_Beliefs.xml`) with every id checked
        // against `Expansion2_RemoveData.xml`, because two of these three are
        // rows the expansion deletes and replaces.
        let pantheon_plot = |effect: &str, matched: bool| -> f64 {
            if matched {
                self.pantheon_effect(pid, effect)
            } else {
                0.0
            }
        };
        // ⚠ `LADY_OF_THE_REEDS_PRODUCTION` (+1) is deleted by the expansion and
        // replaced by `LADY_OF_THE_REEDS_PRODUCTION2` (+2) over the same
        // `PLOT_HAS_REEDS_REQUIREMENTS`. That set names `FEATURE_FLOODPLAINS`
        // and NOT the expansion's own `FEATURE_FLOODPLAINS_GRASSLAND` or
        // `..._PLAINS`, and the shipped text agrees: "Marsh, Oasis, and DESERT
        // Floodplains". A grassland floodplain pays nothing.
        yields.production += pantheon_plot(
            "reeds_production",
            matches!(
                tile.feature.as_deref(),
                Some("marsh" | "oasis" | "floodplains")
            ),
        );
        // Goddess of Fire is a Gathering Storm belief with no base-game row at
        // all: `GODDESS_OF_FIRE_FEATURES_FAITH_MODIFIER`, +2 Faith over
        // `FEATURE_GEOTHERMAL_FISSURE` or `FEATURE_VOLCANIC_SOIL`.
        yields.faith += pantheon_plot(
            "volcanic_geothermal_faith",
            matches!(
                tile.feature.as_deref(),
                Some("geothermal_fissure" | "volcanic_soil")
            ),
        );
        // ⚠⚠ Earth Goddess is the third case where a base-game row states the
        // OPPOSITE of the shipped rule. `Expansion2_RemoveData.xml` deletes
        // `EARTH_GODDESS_APPEAL_FAITH{,_MODIFIER}` and Gathering Storm re-adds
        // them against `PLOT_BREATHTAKING_APPEAL` (`MinimumAppeal 4`) where the
        // base game used `PLOT_CHARMING_APPEAL` (`MinimumAppeal 2`) — the
        // shipped text moves from "Charming or better" to "Breathtaking" with
        // it. Modelling the cache's requirement set alone would have paid this
        // on twice the map.
        yields.faith += pantheon_plot("breathtaking_appeal_faith", self.tile_appeal(pos) >= 4);

        let building_effect = |effect: &str| self.city_building_effect(city, effect);
        let is_coast_or_lake = matches!(tile.terrain.as_str(), "coast" | "lake");
        let is_floodplain = matches!(
            tile.feature.as_deref(),
            Some("floodplains" | "grassland_floodplains" | "plains_floodplains")
        );
        let fresh_water = tile.has_river()
            || self.nbrs(pos).iter().any(|neighbor| {
                self.map.get(*neighbor).is_some_and(|candidate| {
                    candidate.terrain == "lake" || candidate.feature.as_deref() == Some("oasis")
                })
            });

        if is_coast_or_lake && self.grants_city_state_unique_bonus(pid, "Auckland") {
            // `REQUIRES_PLAYER_IS_INDUSTRIAL_ERA` is
            // `REQUIREMENT_PLAYER_ERA_AT_LEAST`: the suzerain's own era.
            yields.production += if self.player_era(pid) >= 4 { 2.0 } else { 1.0 };
        }

        if !tile.pillaged && tile.improvement.is_none() && tile.feature.is_some() {
            yields.gold += self.governor_effect(pid, city.id, "unimproved_feature_gold");
        }
        if !tile.pillaged
            && matches!(
                tile.improvement.as_deref(),
                Some("wind_farm" | "solar_farm" | "offshore_wind_farm" | "geothermal_plant")
            )
        {
            yields.gold += self.governor_effect(pid, city.id, "renewable_gold");
        }

        if !tile.pillaged {
            match tile.improvement.as_deref() {
                Some("fishery") => {
                    let effects = &self.rules.improvements["fishery"].effects;
                    let sea_resources = self
                        .nbrs(pos)
                        .iter()
                        .filter(|neighbor| {
                            let neighbor = &self.map.tiles[neighbor];
                            self.rules.is_water(neighbor) && neighbor.resource.is_some()
                        })
                        .count() as f64;
                    yields.food += sea_resources
                        * effects
                            .get("adjacent_sea_resource_food")
                            .copied()
                            .unwrap_or(0.0);
                    if self.governor_effect(pid, city.id, "fishery") > 0.0 {
                        yields.production +=
                            effects.get("liang_production").copied().unwrap_or(0.0);
                    }
                }
                Some("city_park") if self.governor_effect(pid, city.id, "city_park") > 0.0 => {
                    let spec = &self.rules.improvements["city_park"];
                    let total = spec
                        .effects
                        .get("liang_total_culture")
                        .copied()
                        .unwrap_or(spec.yields.culture);
                    yields.culture += (total - spec.yields.culture).max(0.0);
                }
                _ => {}
            }
        }

        // The Water Mill names three resources and asks nothing of the tile
        // beyond carrying one: WATERMILL_* ship a RESOURCE_TYPE_MATCHES each
        // for Maize, Rice and Wheat, with no improvement requirement. CIVVIS
        // demanded a Farm and then paid *any* Bonus resource, which is both
        // too strict and too broad at once.
        if matches!(tile.resource.as_deref(), Some("maize" | "rice" | "wheat")) {
            yields.food += building_effect("cereal_resource_food");
        }
        if fresh_water {
            yields.food += building_effect("fresh_water_tile_food");
        }
        if is_coast_or_lake {
            yields.food += building_effect("coast_tile_food");
            yields.gold += building_effect("coast_tile_gold");
            if tile.improvement.is_none() {
                yields.production += building_effect("unimproved_coast_production");
            }
        }
        if self.rules.is_water(tile) && tile.resource.is_some() {
            yields.production += building_effect("coastal_resource_production");
        }
        if matches!(tile.feature.as_deref(), Some("jungle" | "marsh")) {
            yields.science += building_effect("rainforest_marsh_science");
        }
        // The Aquarium pays a Reef, and separately a Coast tile that has a
        // visible resource - AQUARIUM_REEF_SCIENCE against
        // AQUARIUM_SEARESOURCE_SCIENCE, whose requirement set is
        // PLOT_RESOURCE_VISIBLE and TERRAIN_COAST together. CIVVIS paid every
        // coast tile whether or not anything was on it.
        if matches!(tile.feature.as_deref(), Some("reef" | "great_barrier_reef")) {
            yields.science += building_effect("reef_science");
        }
        if is_coast_or_lake && tile.resource.is_some() {
            yields.science += building_effect("coast_resource_science");
        }
        if tile.feature.is_some() && self.rules.is_passable(tile) {
            yields.culture += building_effect("passable_feature_culture");
            yields.faith += building_effect("passable_feature_faith");
        }

        let adjacent_preserve = self.nbrs(pos).iter().any(|neighbor| {
            self.map.get(*neighbor).is_some_and(|candidate| {
                candidate.district.is_some_and(|district| {
                    self.district_is_family(district, crate::name!("preserve"))
                        && candidate.owner_city == tile.owner_city
                        && !candidate.pillaged
                })
            })
        });
        if adjacent_preserve
            && tile.improvement.is_none()
            && tile.district.is_none()
            && tile.wonder.is_none()
        {
            let prefix = if self.tile_appeal(pos) >= 4 {
                Some("breathtaking_adjacent_")
            } else if self.tile_appeal(pos) >= 2 {
                Some("charming_adjacent_")
            } else {
                None
            };
            if let Some(prefix) = prefix {
                yields.food += building_effect(&format!("{prefix}food"));
                yields.production += building_effect(&format!("{prefix}production"));
                yields.gold += building_effect(&format!("{prefix}gold"));
                yields.science += building_effect(&format!("{prefix}science"));
                yields.culture += building_effect(&format!("{prefix}culture"));
                yields.faith += building_effect(&format!("{prefix}faith"));
            }
        }

        // Tile-changing wonder effects. City-scoped keys apply only in the
        // constructing city; empire keys apply to every owned worked tile.
        if matches!(tile.feature.as_deref(), Some("marsh")) {
            yields.science += self.empire_wonder_effect(pid, "empire_marsh_science");
            yields.production += self.empire_wonder_effect(pid, "empire_marsh_production");
        }
        if tile.terrain == "lake" {
            yields.food += self.empire_wonder_effect(pid, "empire_lake_food");
            yields.production += self.empire_wonder_effect(pid, "empire_lake_production");
        }
        let city_wonder_effect = |effect: &str| {
            city.wonders
                .keys()
                .map(|wonder| {
                    self.rules.wonders[wonder]
                        .effects
                        .get(effect)
                        .copied()
                        .unwrap_or(0.0)
                })
                .sum::<f64>()
        };
        if is_floodplain {
            yields.science += city_wonder_effect("city_floodplain_science");
            yields.production += city_wonder_effect("city_floodplain_production");
        }
        if is_coast_or_lake {
            yields.science += city_wonder_effect("city_coast_science");
            yields.culture += city_wonder_effect("city_coast_culture");
            yields.faith += city_wonder_effect("city_coast_faith");
        }
        if tile.terrain == "desert" && !is_floodplain {
            yields.food += city_wonder_effect("city_desert_food");
            yields.production += city_wonder_effect("city_desert_production");
            yields.gold += city_wonder_effect("city_desert_gold");
        }
        if tile.feature.as_deref() == Some("jungle") {
            yields.culture += city_wonder_effect("city_rainforest_culture");
            yields.production += city_wonder_effect("city_rainforest_production");
        }
        if tile.terrain == "tundra" {
            yields.food += city_wonder_effect("city_tundra_food");
            yields.production += city_wonder_effect("city_tundra_production");
            yields.culture += city_wonder_effect("city_tundra_culture");
        }
        if matches!(tile.improvement.as_deref(), Some("mine" | "quarry")) {
            yields.production += city_wonder_effect("city_mine_quarry_production");
        }
        yields
    }

    /// What a citizen working this plot actually collects: CIVVIS's own tile
    /// model plus whatever the host says it owes on top. Terrain, Hills,
    /// feature, resource and improvement only — a district's or a wonder's
    /// yields are the city's, not the ground's, and are added by their own
    /// readers.
    ///
    /// Public so the observation the spectator board is drawn from can publish
    /// this exact number rather than re-deriving one beside it (`obs::tile_json`),
    /// and so `civvis-orders --dump-mirror` can print it per plot beside the
    /// host's own reading.
    pub fn workable_tile_yields(&self, pos: Pos) -> Yields {
        let mut yields = self.modeled_tile_yields(pos);
        // A mirrored board pays the tile what the host pays it; see
        // `observed_tile_yield_adjustments`. Empty on a native game.
        if let Some(adjustment) = self.observed_tile_yield_adjustments.get(&pos) {
            yields.add(*adjustment);
        }
        yields
    }

    /// Local yield change from placing an improvement, including researched
    /// bonuses and the loss of any feature the operation removes. Compare
    /// modeled yields on both sides so host corrections cancel rather than
    /// becoming an invented improvement bonus. This is a read-only forecast;
    /// callers still check legality and resource/adjacency effects separately.
    pub(crate) fn improvement_yield_change(
        &self,
        pid: usize,
        pos: Pos,
        improvement: Name,
    ) -> Yields {
        let tile = &self.map.tiles[&pos];
        let mut before = tile.clone();
        if before.pillaged {
            before.improvement = None;
        }
        let mut after = tile.clone();
        after.improvement = Some(improvement);
        after.pillaged = false;
        if self.rules.improvements[improvement].removes_feature {
            after.feature = None;
        }
        let mut gain = self.player_tile_yields(pid, pos, &after);
        gain.add_scaled(self.player_tile_yields(pid, pos, &before), -1.0);
        gain
    }

    /// CIVVIS's own tile model, before any host correction: what
    /// [`Self::workable_tile_yields`] pays on a native game, and the number the
    /// mirror measures the host's per-plot export against.
    pub fn modeled_tile_yields(&self, pos: Pos) -> Yields {
        if let Some(memo) = self.query_memo.tile_yields.borrow().as_ref() {
            if let Some(yields) = memo.get(&pos) {
                return *yields;
            }
        }
        let yields = self.modeled_tile_yields_uncached(pos);
        if let Some(memo) = self.query_memo.tile_yields.borrow_mut().as_mut() {
            memo.insert(pos, yields);
        }
        yields
    }

    fn modeled_tile_yields_uncached(&self, pos: Pos) -> Yields {
        let tile = &self.map.tiles[&pos];
        let owner = tile
            .owner_city
            .and_then(|city| self.cities.get(&city))
            .map(|city| city.owner);
        if !tile.pillaged || tile.improvement.is_none() {
            return owner
                .map(|pid| self.player_tile_yields(pid, pos, tile))
                .unwrap_or_else(|| self.ground_tile_yields(pos, tile));
        }
        let mut unworked = tile.clone();
        unworked.improvement = None;
        owner
            .map(|pid| self.player_tile_yields(pid, pos, &unworked))
            .unwrap_or_else(|| self.ground_tile_yields(pos, &unworked))
    }

    /// Open a memo scope for the expensive read-only queries.
    ///
    /// A candidate loop that scores dozens of purchases asks the same handful
    /// of cities for their yields over and over, and a route search asks the
    /// same unit how it moves once per tile it considers. Both answers cost
    /// tens of microseconds to derive. While the returned guard is alive the
    /// game cannot be mutated, so both can simply be reused.
    pub fn query_memo(&self) -> QueryMemo<'_> {
        let outermost = self.query_memo.yields.borrow().is_none();
        if outermost {
            *self.query_memo.yields.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.appeal.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.tile_yields.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.district_yields.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.traversal.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.air_patrols.borrow_mut() = None;
            *self.query_memo.passage_improvements.borrow_mut() = None;
            *self.query_memo.movement.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.amenities.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.purchase_price.borrow_mut() = Some(PurchasePriceMemo::new());
            *self.query_memo.unit_ids.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.worked_backlog.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.unit_territory_access.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.city_ids.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.lux_alloc.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.lux_names.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.housed_works.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.housed_pieces.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.suzerain.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.gw_slots.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.gw_housing.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.regional.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.powered.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.national_parks.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.regional_slots.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.wonder_effects.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.tourism.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.domestic_tourists.borrow_mut() = Some(BTreeMap::new());
        }
        QueryMemo {
            game: self,
            outermost,
        }
    }

    pub fn city_yields(&self, cid: u32) -> Yields {
        if let Some(memo) = self.query_memo.yields.borrow().as_ref() {
            if let Some(yields) = memo.get(&cid) {
                return *yields;
            }
        }
        let kept = self.kept_yields_for(cid);
        if let Some(kept) = kept {
            if let Some(yields) = self.kept_city_yields(kept, cid) {
                return yields;
            }
        }
        // One derivation prices every owned plot to plan the citizens and
        // the worked plots again to sum them, and asks the city's amenities
        // more than once. A caller with no memo open still shares those
        // answers within this one derivation; a caller with one open nests.
        let _memo = self.query_memo();
        let yields = self.city_yields_uncached(cid);
        if let Some(memo) = self.query_memo.yields.borrow_mut().as_mut() {
            memo.insert(cid, yields);
        }
        if let Some(kept) = kept {
            kept.borrow_mut().insert(cid, yields);
        }
        yields
    }

    /// Keep the city yields of every seat but the acting one from one
    /// read-only pass to the next, until the board changes.
    ///
    /// ★★★★ TEN PASSES A TURN EACH PRICED EVERY CITY IN THE WORLD. A seat's
    /// turn asks what its rivals' cities yield over and over — the culture
    /// forecast, the lane table, the culture-defence floor, the catch-up
    /// investments, the spaceport race — and each pass derives all of them
    /// again, because a `QueryMemo` lasts one pass and a mutable borrow
    /// separates one pass from the next. On the world civvis.ai plays that
    /// was 38% of the stepping thread (`docs/SIMULATOR_PERFORMANCE.md`,
    /// 2026-11-11). Most of those passes change nothing: a city with work
    /// queued is not given more, so the board the tenth pass reads is the
    /// board the first one read.
    ///
    /// What is kept is dropped:
    ///
    /// * **By every action,** at both ends of `apply`, successful or refused,
    ///   and it is never read or filled while one is in flight: the rules
    ///   read yields half-way through changing the board. The one exception
    ///   is `Action::Produce`, which names what one of the acting seat's own
    ///   cities builds next and reaches nothing of another seat's; `apply`
    ///   carries the argument.
    /// * **By every tile write,** through the tile grid's epoch, which every
    ///   `get_mut` moves whoever calls it.
    /// * **By [`Game::board_written`],** which is what a caller that writes
    ///   anything else about the board without an action must call.
    ///
    /// And it never answers while `trace_policy_reads` is recording, because
    /// that caller reads which questions a derivation asked, and a kept
    /// answer asks none.
    ///
    /// A clone keeps nothing, and it is off unless a driver turns it on. The
    /// spectator session does, for the length of one seat's turn; the live
    /// Civilization VI seat, the gene screen and `simulate` do not, and play
    /// exactly as they did.
    ///
    /// [`Game::audit_rival_yields`] checks the answers: with it on, every
    /// reused yield is derived again and compared. It cannot check for a
    /// caller like the policy trace, which is why `site_bench`'s tests also
    /// play the same seed with and without and compare the saves.
    pub fn keep_rival_yields(&mut self, on: bool) {
        self.query_memo.keeps_rival_yields.set(on);
        self.board_written();
    }

    /// Keep the acting seat's own city yields as well.
    ///
    /// Separate from the rivals' because more can change them without an
    /// action. An agent weighs its own empire by rearranging it in place: the
    /// policy scorer seats a card, reads the empire and unseats it, and the
    /// governor passes write the food bias and the city directives. So its
    /// own cities are kept on stricter terms:
    ///
    /// * every action drops them, `Produce` included — a city's yields read
    ///   its own queue (the citizen plan, a running project, the envoy bonus);
    /// * the seat's policy deck, food bias and directive count are folded
    ///   into a stamp that is read back on every lookup
    ///   (`own_settings_stamp`), so a what-if against the deck is noticed
    ///   whether or not anybody reported it;
    /// * and the writers report anyway, through [`Game::board_written`].
    ///
    /// Has no effect unless [`Game::keep_rival_yields`] is on.
    pub fn keep_own_yields(&mut self, on: bool) {
        self.query_memo.keeps_own_yields.set(on);
        self.board_written();
    }

    /// Something about the board was written without an action and without
    /// touching a tile. Drops what [`Game::keep_rival_yields`] holds.
    pub fn board_written(&mut self) {
        self.query_memo.rival_yields.borrow_mut().clear();
        self.query_memo.own_yields.borrow_mut().clear();
    }

    /// Derive every reused yield again and panic if the two differ. An
    /// instrument for tests and `civvis site-bench --audit-kept-yields`: it
    /// costs more than keeping nothing would.
    pub fn audit_rival_yields(&mut self, on: bool) {
        self.query_memo.audits_rival_yields.set(on);
    }

    /// How many rival city yields were answered from what was kept, and how
    /// many were derived while keeping was on.
    pub fn rival_yields_reused(&self) -> (u64, u64) {
        (
            self.query_memo.rival_yields_reused.get(),
            self.query_memo.rival_yields_derived.get(),
        )
    }

    /// Which kept map answers for this city right now, if either does.
    fn kept_yields_for(&self, cid: u32) -> Option<&std::cell::RefCell<BTreeMap<u32, Yields>>> {
        let memo = &self.query_memo;
        if !memo.keeps_rival_yields.get()
            || memo.acting.get() != 0
            || memo.auditing_rival_yields.get()
            // ⚠ A kept answer skips the derivation, and one caller reads the
            // derivation's *questions*, not its answer: `trace_policy_reads`
            // records every policy effect a sweep asks so the policy scorer
            // can call a card inert. A sweep answered from what was kept asks
            // nothing, every card reads as inert, and the seat stops changing
            // its deck. Nothing is reused while a trace is recording.
            || memo.policy_reads_on.get()
        {
            return None;
        }
        if self.cities.get(&cid)?.owner != self.current {
            Some(&memo.rival_yields)
        } else if memo.keeps_own_yields.get() {
            // The two settings an agent writes on its own seat most, read
            // back rather than trusted to be reported.
            let stamp = self.own_settings_stamp();
            if memo.own_yields_stamp.get() != stamp {
                memo.own_yields.borrow_mut().clear();
                memo.own_yields_stamp.set(stamp);
            }
            Some(&memo.own_yields)
        } else {
            None
        }
    }

    /// The acting seat's policy deck and food bias, folded to one number.
    ///
    /// These are what an agent rearranges in place to ask what-if: the policy
    /// scorer seats a card, reads the empire and unseats it. Both writers
    /// call `board_written`; this is the second lock, so a what-if written
    /// tomorrow against the deck cannot read today's kept yields because
    /// somebody forgot to. FNV-1a over the interned card ids and the bias.
    fn own_settings_stamp(&self) -> u64 {
        let Some(seat) = self.players.get(self.current) else {
            return 0;
        };
        let mut stamp = 0xcbf2_9ce4_8422_2325_u64;
        let mut fold = |value: u64| {
            stamp = (stamp ^ value).wrapping_mul(0x0000_0100_0000_01b3);
        };
        for card in &seat.policies {
            fold(u64::from(card.id()));
        }
        fold(seat.policies.len() as u64);
        fold(seat.citizen_food_bias.to_bits());
        fold(seat.city_directives.len() as u64);
        fold(
            seat.citizen_locks
                .values()
                .map(|held| held.len() as u64)
                .sum(),
        );
        stamp
    }

    fn kept_city_yields(
        &self,
        held: &std::cell::RefCell<BTreeMap<u32, Yields>>,
        cid: u32,
    ) -> Option<Yields> {
        let memo = &self.query_memo;
        let epoch = self.map.tiles.epoch();
        if memo.rival_yields_epoch.get() != epoch {
            memo.rival_yields.borrow_mut().clear();
            memo.own_yields.borrow_mut().clear();
            memo.rival_yields_epoch.set(epoch);
        }
        let kept = held.borrow().get(&cid).copied();
        let Some(kept) = kept else {
            memo.rival_yields_derived
                .set(memo.rival_yields_derived.get() + 1);
            return None;
        };
        memo.rival_yields_reused
            .set(memo.rival_yields_reused.get() + 1);
        if memo.audits_rival_yields.get() {
            // Nothing kept may answer inside the comparison, or a stale
            // neighbour would vouch for a stale city.
            memo.auditing_rival_yields.set(true);
            let fresh = {
                let _memo = self.query_memo();
                self.city_yields_uncached(cid)
            };
            memo.auditing_rival_yields.set(false);
            assert!(
                fresh == kept,
                "a kept city yield went stale: city {cid} of seat {} on turn {} while seat {} \
                 acts. Kept {kept:?}, the board now gives {fresh:?}. Something wrote the board \
                 without an action, a tile write or `Game::board_written`.",
                self.cities[&cid].owner,
                self.turn,
                self.current,
            );
        }
        Some(kept)
    }

    /// Yields this city would produce if its citizens were assigned under
    /// `weights` instead of its own appetites.
    ///
    /// `None` is exactly what the city actually produces, so this is additive
    /// and changes no behaviour; it is never on the cached path. It exists so
    /// an instrument can bound the food available to a capital without the
    /// engine adopting a different governor — see `docs/OPENINGS.md`.
    pub fn city_yields_weighted(&self, cid: u32, weights: Yields) -> Yields {
        self.city_yields_inner(cid, Some(weights), true)
    }

    pub(super) fn city_yields_uncached(&self, cid: u32) -> Yields {
        self.city_yields_inner(cid, None, true)
    }

    /// The modeled yield before a live-host correction. Mirror reconciliation
    /// uses this to derive the additive correction without reading itself.
    pub fn city_yields_model(&self, cid: u32) -> Yields {
        self.city_yields_inner(cid, None, false)
    }

    /// The tile-level ledger behind [`Self::city_yields_model`]: the centre as
    /// CIVVIS floors it (2 Food / 1 Production minimum), then every worked
    /// tile with the yields it pays this city, then every assigned specialist
    /// with the yields its slot pays, then every standing district's own
    /// yields. Whatever the model total holds beyond the sum of these is
    /// buildings, routes, policies and the rest.
    ///
    /// Diagnostic only — nothing decides on it. It exists so the yield-fidelity
    /// instrument (`tools/civ6_yield_drift.py`) can diff CIVVIS's tile model
    /// against the host's own per-plot export and name the tile, specialist or
    /// district a gap lives on, rather than only its size per city.
    pub fn city_yield_ledger(&self, cid: u32) -> CityYieldLedger {
        let city = &self.cities[&cid];
        let _memo = self.query_memo();
        let mut center = self.modeled_tile_yields(city.pos);
        center.food = center.food.max(2.0);
        center.production = center.production.max(1.0);
        let plan = self.city_citizen_plan(cid);
        let tiles = plan
            .worked_tiles
            .iter()
            .map(|pos| (*pos, self.modeled_tile_yields(*pos)))
            .collect();
        let tile_adjustments = std::iter::once(city.pos)
            .chain(plan.worked_tiles.iter().copied())
            .filter_map(|pos| {
                self.observed_tile_yield_adjustments
                    .get(&pos)
                    .map(|adjustment| (pos, *adjustment))
            })
            .collect();
        let jobs = self.city_specialist_jobs(city);
        let specialists = plan
            .specialists
            .iter()
            .map(|family| {
                let yields = jobs
                    .iter()
                    .find(|(district, _)| district == family)
                    .map(|(_, yields)| *yields)
                    .unwrap_or_default();
                (family.clone(), yields)
            })
            .collect();
        let districts = city
            .districts
            .iter()
            .filter(|(dname, dpos)| {
                let (dname, dpos): (&Name, &Pos) = (dname, dpos);
                !(self.map.tiles[dpos].pillaged
                    || (self.district_is_family(dname, crate::name!("encampment"))
                        && city.encampment_pillaged))
            })
            .map(|(dname, dpos)| {
                let (dname, dpos): (&Name, &Pos) = (dname, dpos);
                (dname.to_string(), *dpos, self.district_yields(dname, *dpos))
            })
            .collect();
        CityYieldLedger {
            center,
            tiles,
            tile_adjustments,
            specialists,
            districts,
        }
    }

    pub(super) fn city_yields_inner(
        &self,
        cid: u32,
        weights: Option<Yields>,
        apply_observation: bool,
    ) -> Yields {
        // An arena's city is a unit tap, not a settlement. It collects the
        // one flat Production figure both sides are granted and nothing else:
        // no Food, so it never grows and the two sides cannot drift apart in
        // size; no Gold or Science, which the arena grants per side in
        // `begin_turn` so that a side without a city still gets them. This
        // sits in the shared inner routine deliberately — the AI's weighted
        // and model views read the same truth as the turn does, so nothing
        // plans against tile yields the arena will never pay out.
        if self.is_arena() {
            return Yields {
                production: f64::from(self.tactics.production),
                ..Yields::default()
            };
        }
        // A city's yields reach for two empire-wide derivations — its Amenity
        // band and its housed Great Works. Opening a scope here means they are
        // taken once per city rather than once per lookup, and nests harmlessly
        // inside a caller that already holds one.
        let _amenity_memo = self.query_memo();
        let city = &self.cities[&cid];
        let mut ys = Yields::default();
        let mut center = self.workable_tile_yields(city.pos);
        center.food = center.food.max(2.0);
        center.production = center.production.max(1.0);
        ys.add(center);
        let citizen_plan = self.city_citizen_plan_weighted(cid, weights);
        let employed = citizen_plan.worked_tiles.len() + citizen_plan.specialists.len();
        // Collectivism's COLLECTIVISM_FARM_FOOD_MODIFIER is a plot yield on
        // Farm plots, so it is paid where a citizen works one.
        let farm_food = self.policy_effect(city.owner, "farm_food");
        for pos in citizen_plan.worked_tiles {
            ys.add(self.workable_tile_yields(pos));
            if farm_food != 0.0 {
                let tile = &self.map.tiles[&pos];
                if !tile.pillaged && tile.improvement.as_deref() == Some("farm") {
                    ys.food += farm_food;
                }
            }
        }
        let specialist_jobs = self.city_specialist_jobs(city);
        for family in citizen_plan.specialists {
            if let Some((_, specialist_yields)) = specialist_jobs
                .iter()
                .find(|(district, _)| district == &family)
            {
                ys.add(*specialist_yields);
            }
        }
        // A citizen with no tile and no slot still pays half a Gold. Measured
        // on live Rome (run civvis-20260816T200454Z): with every workable plot
        // taken and no specialist slot, the host's Gold ledger read "+0.5 from
        // Population" for one unemployed citizen (t81-96), "+1" for two
        // (t97-106) and nothing once new plots were worked (t107) — the only
        // per-citizen Gold in the game, and nothing in the other five ledgers.
        ys.gold += 0.5 * (city.pop as f64 - employed as f64).max(0.0);
        for (dname, dpos) in &city.districts {
            if self.map.tiles[dpos].pillaged
                || (self.district_is_family(dname, crate::name!("encampment"))
                    && city.encampment_pillaged)
            {
                continue;
            }
            ys.add(self.district_yields(dname, *dpos));
        }
        // Nan Madol's `MODIFIER_PLAYER_DISTRICTS_ADJUST_YIELD_CHANGE` reaches
        // every district plot on or beside Coast — the City Center and each
        // wonder's plot included, which `district_yields` above never sees.
        // The host's culture ledger on live run civvis-20260816T155856Z read
        // "+2 from City Center" in Rome and "+2 from Wonder" in Mediolanum
        // beside the specialty districts the model already paid, two Culture
        // short in every coastal city for a hundred turns.
        if self.grants_city_state_unique_bonus(city.owner, "Nan Madol") {
            let coastal = |pos: Pos| {
                matches!(self.map.tiles[&pos].terrain.as_str(), "coast" | "lake")
                    || self.nbrs(pos).iter().any(|neighbor| {
                        matches!(self.map.tiles[neighbor].terrain.as_str(), "coast" | "lake")
                    })
            };
            if coastal(city.pos) {
                ys.culture += 2.0;
            }
            for wonder_pos in city.wonders.values() {
                if coastal(*wonder_pos) {
                    ys.culture += 2.0;
                }
            }
        }
        self.add_building_yields(city, &mut ys);
        ys.add(self.regional_building_effects(city).0);
        for wonder in city.wonders.keys() {
            let spec = &self.rules.wonders[wonder];
            if spec.regional_range <= 0 {
                ys.add(spec.yields);
            }
        }
        ys.add(self.regional_wonder_effects(city).0);
        // Products are economic Great Works: only Products backed by active
        // Stock Exchange/Seaport slots yield or confer their Industry effect.
        ys.add(self.city_product_yields(city));
        let relic_faith = if self.grants_city_state_unique_bonus(city.owner, "Kandy") {
            6.0
        } else {
            4.0
        };
        // Reliquaries: `MODIFIER_SINGLE_CITY_ADJUST_GREATWORK_YIELD` on Relics,
        // ScalingFactor 300 — triple Faith in a city following the religion.
        let relic_faith =
            relic_faith * (1.0 + self.city_religion_belief_effect(city, "relic_faith_pct") / 100.0);
        if let Some(works) = self.housed_great_works(city.owner).get(&cid) {
            for (kind, count) in works {
                let count = *count as f64;
                match kind.as_str() {
                    "writing" => ys.culture += 2.0 * count,
                    "art" | "religious_art" | "artifact" => ys.culture += 3.0 * count,
                    "music" | "any" => ys.culture += 4.0 * count,
                    "relic" => ys.faith += relic_faith * count,
                    _ => {}
                }
                if self.grants_city_state_unique_bonus(city.owner, "Anshan") {
                    match kind.as_str() {
                        "writing" => ys.science += 2.0 * count,
                        "artifact" | "relic" => ys.science += count,
                        _ => {}
                    }
                }
            }
            let pieces = self.housed_great_work_pieces(city.owner);
            let (_, theming_culture, _) = self.city_theming(
                city.owner,
                cid,
                pieces.get(&cid).map(Vec::as_slice).unwrap_or(&[]),
            );
            ys.culture += theming_culture;
        }
        ys.science += 0.5 * city.pop as f64;
        ys.culture += 0.3 * city.pop as f64;
        if self.city_has_palace(city) {
            ys.add(self.rules.buildings["palace"].yields);
            ys.gold += self.policy_effect(city.owner, "capital_gold");
            ys.faith += self.policy_effect(city.owner, "capital_faith");
        }
        ys.production += self.policy_effect(city.owner, "city_production");
        // A civilization whose signature ability is a flat yield in every city
        // pays it here, once per city, alongside the policy that does the same.
        ys.food += self.civ_effect(city.owner, "city_food");
        ys.production += self.civ_effect(city.owner, "city_production");
        ys.gold += self.civ_effect(city.owner, "city_gold");
        ys.science += self.civ_effect(city.owner, "city_science");
        ys.culture += self.civ_effect(city.owner, "city_culture");
        ys.faith += self.civ_effect(city.owner, "city_faith");
        if self.grants_city_state_unique_bonus(city.owner, "Johannesburg") {
            let improved_resources: BTreeSet<Name> = city
                .owned_tiles
                .iter()
                .filter_map(|position| {
                    let tile = self.map.get(*position)?;
                    let resource = tile.resource.as_ref()?;
                    self.tile_connects_resource(tile, *resource, *position == city.pos)
                        .then_some(*resource)
                })
                .collect();
            let per_resource = if self.players[city.owner]
                .techs
                .contains(&crate::name!("industrialization"))
            {
                2.0
            } else {
                1.0
            };
            ys.production += per_resource * improved_resources.len() as f64;
        }
        if self.grants_city_state_unique_bonus(city.owner, "Singapore") {
            let partners: BTreeSet<usize> = self
                .routes
                .iter()
                .filter(|route| route.origin == cid)
                .filter_map(|route| {
                    self.cities
                        .get(&route.dest)
                        .map(|destination| destination.owner)
                })
                .filter(|owner| {
                    *owner != city.owner
                        && !self.players[*owner].is_minor
                        && !self.players[*owner].is_barbarian
                })
                .collect();
            ys.production += 2.0 * partners.len() as f64;
        }
        // Public Transport pays per Neighborhood, not once per city, and its
        // Food and Production are banded by that district's own tile Appeal:
        // PUBLICTRANSPORT_NEIGHBORHOOD_GOLD is unconditional, the Charming
        // rows need PLOT_IS_APPEAL_BETWEEN MinimumAppeal 2, and the
        // Breathtaking rows another at 4, which stack.
        for (district, position) in &city.districts {
            if !self.district_is_family(district, crate::name!("neighborhood"))
                || !self.district_is_active(city, district, *position)
            {
                continue;
            }
            ys.gold += self.policy_effect(city.owner, "neighborhood_gold");
            let appeal = self.tile_appeal(*position);
            if appeal >= 2 {
                ys.food += self.policy_effect(city.owner, "neighborhood_food");
                ys.production += self.policy_effect(city.owner, "neighborhood_production");
            }
            if appeal >= 4 {
                ys.food += self.policy_effect(city.owner, "neighborhood_breathtaking_food");
                ys.production +=
                    self.policy_effect(city.owner, "neighborhood_breathtaking_production");
            }
        }
        // Merchant Confederation's Gold per Envoy, an Emergency's Gold per
        // Envoy and Raj's yields per tributary are PLAYER-level income
        // (`MODIFIER_PLAYER_ADJUST_YIELD_CHANGE_PER_USED_INFLUENCE_TOKEN`,
        // `..._PER_TRIBUTARY`), collected on the top bar beside the city sum —
        // never a line of any city's ledger. They used to be paid into the
        // Palace city here; measured on live run civvis-20260816T155856Z the
        // capital read 44 Gold modelled against 17 reported for thirty turns,
        // the whole of it 27 placed Envoys under Merchant Confederation, while
        // the host's own capital ledger carried no such line. They now sit in
        // `player_policy_yields`, read by every consumer of the per-turn
        // figure alongside founder-belief income.
        self.add_trade_route_yields(cid, city, &mut ys);
        let derived_incoming_routes =
            self.routes.iter().filter(|route| route.dest == cid).count() as f64;
        let derived_incoming_foreign_routes = self
            .routes
            .iter()
            .filter(|route| route.dest == cid && route.owner != city.owner)
            .count() as f64;
        // A mirrored host can report that a foreign route enters this city
        // while its origin city remains outside our visibility. Such a route
        // cannot be materialized in `self.routes`, so prefer the host's
        // aggregate `(foreign, domestic)` count whenever one is available.
        // Native games leave the observation empty and keep the derived path.
        let (incoming_routes, incoming_foreign_routes) = self
            .observed_incoming_route_deltas
            .get(&cid)
            .map(|(foreign_delta, domestic_delta)| {
                let foreign = (derived_incoming_foreign_routes + *foreign_delta as f64).max(0.0);
                let domestic = (derived_incoming_routes - derived_incoming_foreign_routes
                    + *domestic_delta as f64)
                    .max(0.0);
                (foreign + domestic, foreign)
            })
            .unwrap_or((derived_incoming_routes, derived_incoming_foreign_routes));
        // What this city earns as a destination from incoming routes.
        let mut iys = Yields::default();
        for route in self.routes.iter().filter(|route| route.dest == cid) {
            let government = self.gov_effects(route.owner);
            if self.government_trade_partner(route.owner, city.owner) {
                iys.food += government.allied_suzerain_trade_food;
                iys.production += government.allied_suzerain_trade_production;
                // ...and the destination half, paid to the city receiving it.
                iys.food += self.policy_effect(route.owner, "allied_suzerain_trade_food");
                iys.production +=
                    self.policy_effect(route.owner, "allied_suzerain_trade_production");
            }
            if let Some(alliance) = self.alliance_with(city.owner, route.owner) {
                match alliance.kind.as_str() {
                    "research" => iys.science += 1.0,
                    "cultural" => iys.culture += 1.0,
                    "economic" => iys.gold += 2.0,
                    "religious" => iys.faith += 1.0,
                    _ => {}
                }
            }
            if route.owner != city.owner {
                iys.gold += city.great_person_foreign_route_gold;
            }
        }
        iys.gold += incoming_foreign_routes
            * self.governor_effect(city.owner, city.id, "incoming_foreign_trade_gold");
        // World Congress Trade Policy A. The resolution's text says the SENDER
        // earns +4 Gold; what Firaxis ships is `INCREASES_TRADE_TO_GOLD`, an
        // `EFFECT_ADJUST_TRADE_ROUTE_YIELD_FROM_OTHERS` (Amount 4) attached to
        // the chosen player — the same destination-side effect Zhang Qian's
        // "+2 Gold from incoming foreign routes" and Cleopatra's Egypt use —
        // and the host's own ledger agrees: the chosen player's city reads
        // "+4 from Incoming Trade Routes" per foreign route (Cumae, run
        // civvis-20260816T200454Z t87-101) while a domestic incoming route
        // pays nothing and the sender's origin nothing extra. The +1 route
        // capacity half of the resolution is in `trade_capacity`.
        if incoming_foreign_routes > 0.0
            && self.congress_effect_active("trade_policy", "A", &city.owner.to_string())
        {
            iys.gold += 4.0 * incoming_foreign_routes;
        }
        ys.add(iys);
        if incoming_routes > 0.0
            && city
                .wonders
                .contains_key(&crate::name!("university_of_sankore"))
        {
            ys.science += incoming_routes
                * self.rules.wonders["university_of_sankore"]
                    .effects
                    .get("incoming_route_city_science")
                    .copied()
                    .unwrap_or(0.0);
        }
        if !self.players[city.owner].is_minor {
            ys.add(self.envoy_yields(city.owner, city));
        }
        if self.city_religion(city).is_some() {
            let choral_music = self.city_religion_belief_effect(city, "holy_building_culture");
            let shrine_food = self.city_religion_belief_effect(city, "shrine_food");
            let temple_food = self.city_religion_belief_effect(city, "temple_food");
            for building in city.buildings.iter().filter(|building| {
                !city.pillaged_buildings.contains(*building)
                    && self.building_district_is_active(city, building)
            }) {
                if self.building_is_family(building, crate::name!("shrine")) {
                    ys.culture += self.rules.buildings[building].yields.faith * choral_music;
                    ys.food += shrine_food;
                } else if self.building_is_family(building, crate::name!("temple")) {
                    ys.culture += self.rules.buildings[building].yields.faith * choral_music;
                    ys.food += temple_food;
                }
            }
            // Divine Inspiration: `MODIFIER_SINGLE_CITY_ADJUST_WONDER_YIELD_CHANGE`
            // +4 Faith per Wonder standing in a city that follows the religion —
            // whoever founded it. Rome under Catholicism read 35 Faith in the
            // host and 23 in the model until this line: three Wonders, twelve
            // Faith (run civvis-20260816T123936Z t231-239), and Ostia's
            // Mausoleum the same four more.
            let wonder_faith = self.city_religion_belief_effect(city, "wonder_faith");
            if wonder_faith > 0.0 {
                ys.faith += wonder_faith * city.wonders.len() as f64;
            }
            let work_ethic =
                self.city_religion_belief_effect(city, "holy_site_production_from_adjacency");
            if work_ethic > 0.0 {
                ys.production += city
                    .districts
                    .iter()
                    .filter(|(district, position)| {
                        self.district_is_family(district, crate::name!("holy_site"))
                            && self.district_is_active(city, district, **position)
                    })
                    .map(|(district, position)| self.district_yields(district, *position).faith)
                    .sum::<f64>()
                    * work_ethic;
            }
        }
        if let Some(partner) = self.alliance_partner(city.owner, "religious", 3) {
            if let Some(religion) = self.players[partner].religion.as_deref() {
                ys.faith += self.religious_followers_in_city(city, religion);
            }
        }
        if self.has_ability(city.owner, "platos_republic") {
            let suz = self
                .players
                .iter()
                .filter(|m| m.is_minor && !m.is_barbarian && m.alive)
                .filter(|m| self.suzerain_of(m.id) == Some(city.owner))
                .count() as f64;
            ys.culture *= 1.0 + 0.05 * suz; // Surrounded by Glory
        }
        let active_improvements = |improvement: &str| {
            city.owned_tiles
                .iter()
                .filter(|position| {
                    let tile = &self.map.tiles[position];
                    !tile.pillaged && tile.improvement.as_deref() == Some(improvement)
                })
                .count() as f64
        };
        // Pantheons that pay per improved tile. Every amount and predicate here
        // is read from the Gathering Storm install's own modifier rows
        // (`Expansion*/Data/*.xml`, checked against `Expansion2_RemoveData.xml`
        // so a base-game row the expansion deletes is never modelled) rather
        // than from the compiled cache, which holds whatever ruleset the game
        // last ran — see `docs/FIDELITY.md`.
        let improved_resource_tiles = |improvement: Option<&str>, classes: &[&str]| -> f64 {
            city.owned_tiles
                .iter()
                .filter(|position| {
                    let tile = &self.map.tiles[position];
                    if tile.pillaged {
                        return false;
                    }
                    match improvement {
                        Some(wanted) => tile.improvement.as_deref() == Some(wanted),
                        // God of Craftsmen asks only that the resource be
                        // worked, whichever improvement works it.
                        None => tile.improvement.is_some(),
                    }
                    .then_some(tile.resource.as_deref())
                    .flatten()
                    .and_then(|resource| self.rules.resources.get(resource))
                    .is_some_and(|spec| classes.contains(&spec.class.as_str()))
                })
                .count() as f64
        };
        ys.culture +=
            self.pantheon_effect(city.owner, "pasture_culture") * active_improvements("pasture");
        ys.production += self.pantheon_effect(city.owner, "fishing_boats_production")
            * active_improvements("fishing_boats");
        // Goddess of the Hunt: GODDESS_OF_THE_HUNT_CAMP_{FOOD,PRODUCTION}_MODIFIER,
        // +1 each on PLOT_HAS_CAMP_REQUIREMENTS.
        ys.food += self.pantheon_effect(city.owner, "camp_food") * active_improvements("camp");
        ys.production +=
            self.pantheon_effect(city.owner, "camp_production") * active_improvements("camp");
        // Stone Circles: STONE_CIRCLES_QUARRY_FAITH_MODIFIER, +2 on a Quarry.
        ys.faith +=
            self.pantheon_effect(city.owner, "quarry_faith") * active_improvements("quarry");
        // Goddess of Festivals: the expansion deletes the base game's
        // PLANTATION_TAG_FOOD row and grants Culture instead.
        ys.culture += self.pantheon_effect(city.owner, "plantation_culture")
            * active_improvements("plantation");
        // Religious Idols: two modifiers, BONUS_MINE and LUXURY_MINE, +2 Faith
        // each — one effect here because a plot carries one resource.
        ys.faith += self.pantheon_effect(city.owner, "resource_mine_faith")
            * improved_resource_tiles(Some("mine"), &["bonus", "luxury"]);
        // God of Craftsmen: the expansion deletes STRATEGIC_MINE_PRODUCTION and
        // pays on any improved Strategic resource, Production and Faith alike.
        let improved_strategics = improved_resource_tiles(None, &["strategic"]);
        ys.production +=
            self.pantheon_effect(city.owner, "strategic_improved_production") * improved_strategics;
        ys.faith +=
            self.pantheon_effect(city.owner, "strategic_improved_faith") * improved_strategics;
        if self.dedication_active(city.owner, "pen_brush_and_voice") {
            ys.culture += city
                .districts
                .iter()
                .filter(|(district, position)| self.district_is_active(city, district, **position))
                .count() as f64;
        }
        if self.dedication_active(city.owner, "free_inquiry") {
            for (district, position) in &city.districts {
                if !self.district_is_active(city, district, *position)
                    || !matches!(
                        self.district_family(*district).as_str(),
                        "commercial_hub" | "harbor"
                    )
                {
                    continue;
                }
                let total = self.district_yields(district, *position);
                let base = self.rules.districts[district].yields;
                ys.science += (total.gold - base.gold).max(0.0);
            }
        }
        // Heartbeat of Steam's Golden Age half is
        // `COMMEMORATION_INUDSTRIAL_GA_CAMPUS_MODIFIER`: every Campus grants
        // Production equal to its Science adjacency (the wonder +10% lives with
        // production). It was paying +1 Science per Industrial Zone building,
        // which no row grants. Measured on live run civvis-20260816T132247Z:
        // the host's ledgers read "+10 from Campus" under PRODUCTION in Cumae
        // and "+8 from Campus" in Arretium for the whole Golden Age, the largest
        // persistent gaps of the game.
        if self.dedication_active(city.owner, "heartbeat_of_steam") {
            for (district, position) in &city.districts {
                if !self.district_is_active(city, district, *position)
                    || self.district_family(*district) != "campus"
                {
                    continue;
                }
                let total = self.district_yields(district, *position);
                let base = self.rules.districts[district].yields;
                ys.production += (total.science - base.science).max(0.0);
            }
        }
        let eff = self.gov_effects(city.owner);
        ys.production += eff.production_per_pop * city.pop as f64;
        if self.city_governor_active(city.owner, city.id) {
            ys.production += eff.governor_production_per_pop * city.pop as f64;
        }
        ys.faith += eff.faith_per_pop * city.pop as f64;
        if self.city_governor_active(city.owner, city.id) {
            ys.faith += eff.governor_faith_per_pop * city.pop as f64;
        }
        ys.culture += eff.culture_per_district * self.city_specialty_district_count(city) as f64;
        ys.science +=
            self.governor_effect(city.owner, city.id, "science_per_pop") * city.pop as f64;
        ys.culture +=
            self.governor_effect(city.owner, city.id, "culture_per_pop") * city.pop as f64;
        ys.gold += self.governor_effect(city.owner, city.id, "gold_per_pop") * city.pop as f64;
        let active_specialty_districts = city
            .districts
            .iter()
            .filter(|(district, position)| {
                self.rules.districts[district].specialty
                    && self.district_is_active(city, district, **position)
            })
            .count() as f64;
        ys.faith += self.governor_effect(city.owner, city.id, "specialty_district_faith")
            * active_specialty_districts;
        if city.buildings.iter().any(|building| {
            matches!(
                building.as_str(),
                "coal_power_plant" | "oil_power_plant" | "nuclear_power_plant"
            ) && !city.pillaged_buildings.contains(building)
                && self.building_district_is_active(city, building)
        }) {
            ys.production += self.governor_effect(city.owner, city.id, "power_plant_production");
        }
        if self.city_has_active_building_family(city, crate::name!("hydroelectric_dam")) {
            ys.gold += self.governor_effect(city.owner, city.id, "renewable_gold");
        }
        if self.city_has_palace(city) {
            ys.add(eff.capital_yields);
        }
        let government_buildings = city
            .buildings
            .iter()
            .filter(|building| {
                !city.pillaged_buildings.contains(*building)
                    && self.building_district_is_active(city, building)
                    && self.rules.buildings[building]
                        .district
                        .is_some_and(|district| {
                            matches!(
                                self.district_family(district).as_str(),
                                "government_plaza" | "diplomatic_quarter"
                            )
                        })
            })
            .count()
            + usize::from(self.city_has_palace(city));
        for _ in 0..government_buildings {
            ys.add(eff.government_building_yields);
        }
        let district_production_pct = if self
            .city_has_active_district_family(city, crate::name!("commercial_hub"))
            || self.city_has_active_district_family(city, crate::name!("encampment"))
        {
            eff.commercial_encampment_production_pct
        } else {
            0.0
        };
        // ★★★★ PERCENTAGE MODIFIERS SUM; THEY DO NOT CHAIN. Every
        // `EFFECT_ADJUST_CITY_YIELD_MODIFIER` on a city — government, policy,
        // wonder, Governor, suzerain bonus, the Amenity band, Loyalty — is one
        // term of a single sum applied once to the base. Live Rome (run
        // civvis-20260816T200454Z, t146) read "+25 (+4.5) from Modifiers" on
        // a base of 18: Merchant Republic's 10 (`CITY_HAS_GOVERNOR`) plus
        // Kilwa's 15 = 25 → 22.5, where the model's ×1.10 × ×1.15 read 22.77;
        // and at t150 "-10 (-2) from Amenities | +10 (+2) from Modifiers" on
        // 21 read exactly 21.0, which ×0.9 × ×1.1 would have made 20.79.
        // `pct` collects them; they land at the end of this function.
        let mut pct = Yields::default();
        pct.production += eff.production_pct + district_production_pct;
        pct.science += eff.science_pct;
        pct.gold += eff.gold_pct;
        if self.city_has_established_governor(city.owner, city.id) {
            pct.gold += eff.governor_gold_pct;
        }
        pct.science += self.governor_effect(city.owner, city.id, "science_pct");
        pct.culture += self.governor_effect(city.owner, city.id, "culture_pct");
        // Dark Age cards buy their strength with an empire-wide penalty.
        pct.science += self.policy_effect(city.owner, "city_science_pct");
        pct.culture += self.policy_effect(city.owner, "city_culture_pct");
        // Monasticism (shipped GS `PolicyModifiers`): MONASTICISM_HOLYSITE_SCIENCE
        // is +75% Science under CITY_HAS_HOLY_SITE; MONASTICISM_CULTURE_MODIFIER
        // is -25% Culture with NO requirement set — every city, Holy Site or
        // not — and so rides the plain `city_culture_pct` term above.
        if self.city_has_active_district_family(city, crate::name!("holy_site")) {
            pct.science += self.policy_effect(city.owner, "holy_site_city_science_pct");
        }
        // Robber Barons, as the shipped rows read: ROBBERBARONS_STOCKEXCHANGE_GOLD
        // (+50% Gold) requires BUILDING_IS_STOCK_EXCHANGE and
        // ROBBERBARONS_FACTORY_PRODUCTION (+25% Production) requires
        // BUILDING_IS_FACTORY. There is no Bank-or-Shipyard set anywhere in
        // the Expansion data; the note this code used to carry was wrong.
        if self.city_has_active_building_family(city, crate::name!("stock_exchange")) {
            pct.gold += self.policy_effect(city.owner, "stock_exchange_city_gold_pct");
        }
        if self.city_has_active_building_family(city, crate::name!("factory")) {
            pct.production += self.policy_effect(city.owner, "factory_city_production_pct");
        }
        let local_wonder_effect = |effect: &str| {
            city.wonders
                .keys()
                .map(|wonder| {
                    self.rules.wonders[wonder]
                        .effects
                        .get(effect)
                        .copied()
                        .unwrap_or(0.0)
                })
                .sum::<f64>()
        };
        pct.production += local_wonder_effect("city_production_pct");
        pct.science += local_wonder_effect("city_science_pct");
        pct.culture += local_wonder_effect("city_culture_pct");
        pct.faith += local_wonder_effect("city_faith_pct");
        if self.city_governor_active(city.owner, city.id)
            && self.on_foreign_continent(city.owner, city.pos)
        {
            let abroad =
                self.empire_wonder_effect(city.owner, "governor_foreign_continent_yields_pct");
            pct.production += abroad;
            pct.gold += abroad;
            pct.faith += abroad;
        }
        let mut science_pct = self.empire_wonder_effect(city.owner, "empire_science_pct");
        let mut production_pct = self.empire_wonder_effect(city.owner, "empire_production_pct");
        if let Some(source) = self
            .cities
            .values()
            .filter(|source| source.owner == city.owner)
            .find(|source| {
                source
                    .wonders
                    .contains_key(&crate::name!("amundsen_scott_research_station"))
            })
        {
            let required = self.rules.wonders["amundsen_scott_research_station"]
                .effects
                .get("snow_condition_double")
                .copied()
                .unwrap_or(0.0) as usize;
            let snow = self
                .wdisk(source.pos, 3)
                .into_iter()
                .filter(|position| {
                    self.map.get(*position).is_some_and(|tile| {
                        tile.terrain == "snow"
                            && tile
                                .owner_city
                                .and_then(|owner_city| self.cities.get(&owner_city))
                                .is_some_and(|owner_city| owner_city.owner == city.owner)
                    })
                })
                .count();
            if required > 0 && snow >= required {
                science_pct *= 2.0;
                production_pct *= 2.0;
            }
        }
        pct.science += science_pct;
        pct.production += production_pct;
        if self.players[city.owner]
            .counters
            .get("warlords_throne_until")
            .is_some_and(|until| *until >= self.turn as i64)
        {
            let capture_pct = self
                .cities
                .values()
                .filter(|source| source.owner == city.owner)
                .map(|source| self.city_building_effect(source, "capture_city_production_pct"))
                .sum::<f64>();
            pct.production += capture_pct;
        }
        if self.on_foreign_continent(city.owner, city.pos) {
            pct.gold += self.policy_effect(city.owner, "foreign_continent_gold_pct");
            pct.production += self.policy_effect(city.owner, "foreign_continent_production_pct");
        }
        let suzerains = self
            .players
            .iter()
            .filter(|minor| {
                minor.is_minor
                    && !minor.is_barbarian
                    && self.suzerain_of(minor.id) == Some(city.owner)
            })
            .count() as f64;
        pct.science += suzerains * self.policy_effect(city.owner, "science_pct_per_suzerain");
        pct.culture += suzerains * self.policy_effect(city.owner, "culture_pct_per_suzerain");
        let economic_yields = self.city_resource_industry_effects(city).city_yield_pct;
        pct.add(economic_yields);
        // No age multiplies yields. Gathering Storm's `Modifiers` carry no
        // yield term keyed on PLAYER_HAS_GOLDEN_AGE or a Dark Age beyond the
        // named commemorations (and Suleiman's trait, Tsikhe); the ×1.10 /
        // ×0.95 this line used to apply, and Sky and Stars' ×1.10 in
        // Aerodrome/Spaceport cities (`COMMEMORATION_AERONAUTICAL_GA_*` grants
        // tech boosts, air-unit XP and Aluminum, no yield), were CIVVIS's own.
        let band = (self.amenity_yield_mult(city) - 1.0) * 100.0;
        // Scottish Enlightenment is separate from the ordinary amenity band:
        // its four live modifiers add +5% Science and Production at Happy,
        // then +10% at Ecstatic. Like every other city-yield modifier, these
        // are terms in Civ VI's one additive percentage sum, not a chained
        // multiplier over the ordinary +10% / +20% Happiness bonus.
        let scottish_happiness = self.scottish_enlightenment_happiness_scale(city);
        pct.science += scottish_happiness * self.civ_effect(city.owner, "happy_science_pct");
        pct.production += scottish_happiness * self.civ_effect(city.owner, "happy_production_pct");
        // Loyalty bands the non-Food yields only. `LoyaltyLevels.YieldChange`
        // (-25% / -50% / -100%) never reaches Food; the level's `GrowthChange`
        // (0.75 / 0.25 / 0) is what a disloyal city pays, and
        // `loyalty_growth_mult` applies it where food becomes citizens. Measured
        // on the host's own per-yield ledgers (runs civvis-20260816T040537Z and
        // T045316Z, 59 city-turns in a loyalty band): "from Disloyal" /
        // "from Wavering Loyalty" appears on culture, faith, gold, production
        // and science every time and on food never — a city in Unrest read
        // "+7 from Worked Tiles", total 7, while the model paid it **0**.
        let band = band + (Self::loyalty_yield_mult(city.loyalty) - 1.0) * 100.0;
        pct.production += band;
        pct.gold += band;
        pct.science += band;
        pct.culture += band;
        pct.faith += band;
        // `MINOR_CIV_PRODUCTION_PENALTY`: a city-state's cities carry -50%
        // Production in the same additive sum.
        if self.players[city.owner].is_minor && !self.players[city.owner].is_barbarian {
            pct.production -= 50.0;
        }
        if self.grants_city_state_unique_bonus(city.owner, "Geneva")
            && !self.at_war_with_any_civilization(city.owner)
        {
            pct.science += 15.0;
        }
        if self.grants_city_state_unique_bonus(city.owner, "Antananarivo") {
            let great_people = self.players[city.owner].great_people.len().min(15) as f64;
            pct.culture += 2.0 * great_people;
        }
        if self.grants_city_state_unique_bonus(city.owner, "Taruga") {
            // +5% Science per *different* improved Strategic resource the
            // city has, so two Iron mines are one resource, not two.
            let kinds: BTreeSet<&str> =
                city.owned_tiles
                    .iter()
                    .filter_map(|position| self.map.get(*position))
                    .filter(|tile| !tile.pillaged)
                    .filter_map(|tile| {
                        let resource = tile.resource.as_deref()?;
                        let spec = self.rules.resources.get(resource)?;
                        let improved =
                            tile.improvement.as_deref().is_some_and(|improvement| {
                                spec.improvement == improvement
                                    || self.rules.improvements.get(improvement).is_some_and(
                                        |have| {
                                            have.resources.iter().any(|listed| listed == resource)
                                        },
                                    )
                            });
                        (spec.class == "strategic" && improved).then_some(resource)
                    })
                    .collect();
            pct.science += 5.0 * kinds.len() as f64;
        }
        pct.science += self.kilwa_type_bonus_pct(city.owner, city, "scientific");
        pct.culture += self.kilwa_type_bonus_pct(city.owner, city, "cultural");
        pct.gold += self.kilwa_type_bonus_pct(city.owner, city, "trade");
        pct.faith += self.kilwa_type_bonus_pct(city.owner, city, "religious");
        // The difficulty handicap is one more term of the same sum.
        let handicap = self.handicap_yield_pct(city.owner);
        pct.production += handicap.production;
        pct.gold += handicap.gold;
        pct.science += handicap.science;
        pct.culture += handicap.culture;
        pct.faith += handicap.faith;
        // The one application. A sum below -100 (Unrest on top of a Dark
        // Age card) pays nothing rather than a negative yield.
        let apply = |value: f64, percent: f64| value * (1.0 + percent / 100.0).max(0.0);
        ys.production = apply(ys.production, pct.production);
        // A running district project converts a share of the city's
        // PRODUCTION RATE into its yield every turn — Firaxis
        // `Project_YieldConversions.PercentOfProductionRate` (Commercial Hub
        // Investment 30% Gold; Campus/Theater/Holy Site 15%; Encampment and
        // Harbor 15% Gold) — and the host's own ledger files it as a base
        // line ("+7.7 from Commercial Hub Investment" on 26 Production, Rome,
        // run civvis-20260816T223457Z t112) that the Amenity band then scales
        // with the rest. It used to be paid only at turn processing, so the
        // city's yield read short of the host for every project turn.
        if let Some(Item::Project { project }) = city.queue.first() {
            if let Some(spec) = self.rules.projects.get(project) {
                if self.project_has_active_district(city, spec) {
                    for (kind, percent) in &spec.ongoing_yields {
                        let amount = ys.production * percent / 100.0;
                        match kind.as_str() {
                            "science" => ys.science += amount,
                            "culture" => ys.culture += amount,
                            "gold" => ys.gold += amount,
                            "faith" => ys.faith += amount,
                            "food" => ys.food += amount,
                            "production" => ys.production += amount,
                            _ => {}
                        }
                    }
                }
            }
        }
        ys.food = apply(ys.food, pct.food);
        ys.gold = apply(ys.gold, pct.gold);
        ys.science = apply(ys.science, pct.science);
        ys.culture = apply(ys.culture, pct.culture);
        ys.faith = apply(ys.faith, pct.faith);
        if apply_observation {
            if let Some(adjustment) = self.observed_city_yield_adjustments.get(&cid) {
                ys.add(*adjustment);
            }
        }
        ys
    }

    pub(super) fn forget_observed_palaces(&mut self, old: usize, new: usize) {
        Arc::make_mut(&mut self.observed_city_palaces).retain(|id, _| {
            self.cities
                .get(id)
                .is_some_and(|city| city.owner != old && city.owner != new)
        });
    }

    /// The host's current-capital flag for location-based rules. Without an
    /// observation, preserve the engine's existing capital-flag behavior.
    /// Domination and original-capital rules read `City::is_capital` directly.
    pub(super) fn city_capital_flag(&self, city: &City) -> bool {
        self.observed_city_palaces
            .get(&city.id)
            .copied()
            .unwrap_or(city.is_capital)
    }

    /// The Palace occupies the original capital while it is controlled;
    /// after that city is captured it moves to another owned city. City-states
    /// likewise have a Palace even though their city is not an original
    /// capital for Domination Victory purposes.
    pub(crate) fn city_has_palace(&self, city: &City) -> bool {
        if let Some(&has_palace) = self.observed_city_palaces.get(&city.id) {
            return has_palace;
        }
        let owns_original_capital = self.cities.values().any(|candidate| {
            candidate.owner == city.owner
                && candidate.original_owner == city.owner
                && candidate.is_capital
        });
        if owns_original_capital {
            city.is_capital && city.original_owner == city.owner
        } else {
            self.player_city_ids(city.owner).into_iter().min() == Some(city.id)
        }
    }

    pub(super) fn can_establish_industry(&self, pid: usize, pos: Pos) -> bool {
        let Some(tile) = self.map.get(pos) else {
            return false;
        };
        let Some(city_id) = tile.owner_city else {
            return false;
        };
        let city = &self.cities[&city_id];
        let Some(resource) = tile.resource.as_deref() else {
            return false;
        };
        let Some(spec) = self.rules.resources.get(resource) else {
            return false;
        };
        if city.owner != pid
            || spec.class != "luxury"
            || !self.rules.improvements["industry"]
                .resources
                .iter()
                .any(|candidate| candidate == resource)
            || self.rules.is_water(tile)
            || !self.players[pid].techs.contains(&crate::name!("currency"))
            || tile.improvement.as_deref() != Some(spec.improvement.as_str())
            || tile.pillaged
            || tile.flooded
            || tile.submerged
            || tile.district.is_some()
            || tile.district_foundation.is_some()
            || tile.wonder.is_some()
            || self.city_at(pos).is_some()
            || self.city_economic_improvement(city).is_some()
            || self.empire_has_economic_improvement(pid, resource)
        {
            return false;
        }
        // Both qualifying copies must be connected, and the Builder replaces
        // the existing resource improvement with the Industry.
        self.controlled_resource_count(pid, resource) >= 2
    }

    pub(super) fn builder_may_improve_territory(&self, pid: usize, territory_owner: usize) -> bool {
        territory_owner == pid
            || (self
                .players
                .get(territory_owner)
                .is_some_and(|owner| owner.is_minor && !owner.is_barbarian)
                && self.suzerain_of(territory_owner) == Some(pid))
    }

    /// Whether a tile carries one of the map's natural wonders. Districts,
    /// world wonders and city sites all have to leave those tiles alone, and
    /// so does every Builder improvement bar the National Park.
    pub fn tile_is_natural_wonder(&self, tile: &crate::world::Tile) -> bool {
        tile.feature.as_ref().is_some_and(|feature| {
            self.rules
                .features
                .get(feature.as_str())
                .is_some_and(|spec| spec.natural_wonder)
        })
    }

    pub fn valid_improvements(&self, pid: usize, pos: Pos) -> Vec<Name> {
        // Ground the host engine has already refused to improve. Empty unless CIVVIS
        // is driving one; see `blocked_improvement_sites`. Gated here because this is
        // the single chokepoint every improvement decision passes through, so one
        // check routes the planner around the tile everywhere at once.
        if self.blocked_improvement_sites.contains(&pos) {
            return vec![];
        }
        // A Great Person is standing there. The host will not let a builder end
        // its move on that plot, so an improvement it "could" build there is one
        // it walks toward forever; see `great_person_plots`.
        if self.great_person_plots.contains_key(&pos) {
            return vec![];
        }
        let t = match self.map.get(pos) {
            Some(t) => t,
            None => return vec![],
        };
        if t.flooded
            || t.district.is_some()
            || t.district_foundation.is_some()
            || t.wonder.is_some()
            || t.improvement.as_deref() == Some("national_park")
            || self.city_at(pos).is_some()
        {
            return vec![];
        }
        // A natural wonder is permanent terrain in Civ VI: nothing a Builder
        // can do removes or covers one. Without this the map quietly loses
        // wonders as the game runs, because a Farm sites itself on the
        // Pantanal's grassland or Uluru's desert like any other tile and
        // clears the feature on the way in.
        let natural_wonder = self.tile_is_natural_wonder(t);
        let refused = self.blocked_improvements.get(&pos);
        let host_refused = |name: &Name| refused.is_some_and(|names| names.contains(name));
        if !natural_wonder {
            if let Some(excavation) = self.valid_excavation(pid, pos) {
                let excavation = Name::new(&excavation);
                if host_refused(&excavation) {
                    return vec![];
                }
                return vec![excavation];
            }
        }
        // A remembered or disposable planning tile can outlive its city.
        // Without the city, ownership is unknown and no Builder permission
        // can be inferred from the stale handle.
        let (oc, territory_owner) = match t.owner_city {
            Some(oc) => {
                let Some(owner_city) = self.cities.get(&oc) else {
                    return vec![];
                };
                (Some(oc), Some(owner_city.owner))
            }
            None => (None, None),
        };
        let visible_resource = t
            .resource
            .as_deref()
            .filter(|resource| self.resource_visible_to(pid, resource));
        let water = self.rules.is_water(t);
        // Whether a strategic deposit here would be connected by `candidate`
        // and is not by whatever stands on it now: the one replacement the
        // agent can carry out (`improvement_replacement_started`).
        let connects_strategic = |candidate: &str| {
            visible_resource.is_some_and(|resource| {
                let deposit = &self.rules.resources[resource];
                let connects = |improvement: &str| {
                    deposit.improvement == improvement
                        || self
                            .rules
                            .improvements
                            .get(improvement)
                            .is_some_and(|spec| spec.resources.iter().any(|r| r == resource))
                };
                deposit.class == "strategic"
                    && connects(candidate)
                    && !t.improvement.as_deref().is_some_and(connects)
            })
        };
        // Whether another Artifact has a slot anywhere in the empire — an
        // empire-wide Great Work assignment, so it is asked once for this
        // tile rather than once per excavation improvement on it.
        let mut artifact_home: Option<bool> = None;
        let mut out = Vec::new();
        for (name, spec) in &self.rules.improvements {
            if host_refused(name) {
                continue;
            }
            if name == "national_park" {
                if self.tree_effect(pid, "national_parks") > 0.0
                    && self.national_park_site_at(pid, pos).is_some()
                {
                    out.push(*name);
                }
                continue;
            }
            // A National Park may enclose a natural wonder — and leaves its
            // feature standing — so it is checked above this line. Nothing
            // else may.
            if natural_wonder {
                continue;
            }
            if name == "industry" {
                if self.can_establish_industry(pid, pos) {
                    out.push(*name);
                }
                continue;
            }
            // A Builder works on bare ground. The host refuses to build over
            // another improvement: across 101 live logs (2026-11-10), 98 such
            // orders failed and 7 worked. Removing one first is a separate
            // operation, which the agent performs only for a strategic deposit.
            if t.improvement.is_some() && !connects_strategic(name.as_str()) {
                continue;
            }
            if matches!(name.as_str(), "fishery" | "city_park")
                && oc.is_none_or(|city| self.governor_effect(pid, city, name) <= 0.0)
            {
                continue;
            }
            if matches!(name.as_str(), "archaeological_dig" | "shipwreck_excavation")
                && !*artifact_home
                    .get_or_insert_with(|| self.can_house_additional_great_work(pid, "artifact"))
            {
                continue;
            }
            // Every disqualifier below is a pure read of this tile, this
            // improvement and the empire, so the order they are asked in
            // cannot change the answer, only the work. The tile-local siting
            // tests refuse most (tile, improvement) pairs outright and run
            // first; the empire lookups (the unlock state, the city-state
            // unique-bonus walk) and the six-neighbour scans run only for a
            // pair that could stand here at all. Before this staging the
            // neighbour scans ran for every improvement on every tile, and
            // `grants_city_state_unique_bonus` for every unique improvement
            // on every tile a Builder considered.
            let seaside_volcanic =
                name == "seaside_resort" && t.feature.as_deref() == Some("volcanic_soil");
            // Civ 6 sites an improvement through any one of three routes —
            // a valid terrain, a valid feature, or a valid resource. Farms
            // stand on grassland OR on desert floodplains OR on wheat;
            // Lumber Mills list no terrain at all, so only their feature
            // route exists. An improvement listing none of the three is
            // unrestricted (governor and appeal specials gate elsewhere).
            let feature_route = t.feature.as_ref().is_some_and(|feature| {
                spec.feature.contains(feature)
                    || spec
                        .feature_after_civic
                        .get(feature.as_str())
                        .is_some_and(|civic| self.players[pid].civics.contains(&Name::new(civic)))
            });
            let resource_route = visible_resource.is_some_and(|resource| {
                spec.resources.iter().any(|candidate| candidate == resource)
                    || self.rules.resources[resource].improvement == *name
            });
            let unrestricted =
                spec.terrain.is_empty() && spec.feature.is_empty() && spec.resources.is_empty();
            let sited = unrestricted
                || spec.terrain.contains(&t.terrain)
                || feature_route
                || resource_route;
            // Firaxis evaluates a featured plot through Improvement_ValidFeatures
            // (or a compatible resource), not through the terrain hidden below it.
            // Treating the underlying Hills as sufficient offered Mines on Woods;
            // the live engine refused both attempts in the turn-150 Poland trace.
            let incompatible_feature = t.feature.is_some() && !feature_route && !resource_route;
            if spec.unbuildable
                || water != spec.water
                || t.improvement.as_deref() == Some(name)
                || (spec.requires_hills && !t.hills)
                || (spec.hills_or_resource && !t.hills && visible_resource.is_none())
                || (spec.hills_or_resource_or_feature
                    && !t.hills
                    && visible_resource.is_none()
                    && !feature_route)
                || (spec.hills_or_feature && !t.hills && !feature_route)
                || incompatible_feature
                || (!sited && !seaside_volcanic)
            {
                continue;
            }
            if !self.unlocked(pid, &spec.tech, &spec.civic)
                || spec.unique_to.as_deref().is_some_and(|owner| {
                    !self.owns_civ_unique(pid, owner)
                        && !self.grants_city_state_unique_bonus(pid, owner)
                })
                || (spec.requires_flat
                    && t.hills
                    && !seaside_volcanic
                    && !(name == "farm" && self.tree_effect(pid, "hill_farms") > 0.0))
                || (spec.removes_feature
                    && !feature_route
                    && t.feature
                        .as_deref()
                        .is_some_and(|feature| !self.feature_removal_unlocked(pid, feature)))
            {
                continue;
            }
            let seaside_invalid = name == "seaside_resort"
                && (self.tile_appeal(pos) < 4
                    || !self.nbrs(pos).iter().any(|neighbor| {
                        self.map
                            .get(*neighbor)
                            .is_some_and(|tile| self.rules.is_water(tile))
                    }));
            let same_adjacent_invalid = !spec.same_adjacent_valid
                && self.nbrs(pos).iter().any(|neighbor| {
                    self.map.tiles[neighbor].improvement.as_deref() == Some(name.as_str())
                });
            let adjacent_resource_class_invalid =
                !spec.requires_adjacent_resource_classes.is_empty()
                    && !self.nbrs(pos).iter().any(|neighbor| {
                        self.map.tiles[neighbor]
                            .resource
                            .as_ref()
                            .filter(|resource| self.resource_visible_to(pid, resource))
                            .and_then(|resource| self.rules.resources.get(resource.as_str()))
                            .is_some_and(|resource| {
                                spec.requires_adjacent_resource_classes
                                    .iter()
                                    .any(|class| class == &resource.class)
                            })
                    });
            let adjacent_passable_land_invalid = spec.requires_adjacent_passable_land > 0
                && self
                    .nbrs(pos)
                    .iter()
                    .filter(|neighbor| {
                        self.map.get(**neighbor).is_some_and(|tile| {
                            !self.rules.is_water(tile) && self.rules.is_passable(tile)
                        })
                    })
                    .count()
                    < spec.requires_adjacent_passable_land;
            let adjacent_water_resource_invalid = spec.requires_adjacent_water_resource
                && !self.nbrs(pos).iter().any(|neighbor| {
                    self.map.get(*neighbor).is_some_and(|tile| {
                        self.rules.is_water(tile)
                            && tile
                                .resource
                                .as_deref()
                                .is_some_and(|resource| self.resource_visible_to(pid, resource))
                    })
                });
            let forbidden_adjacent_feature = !spec.forbids_adjacent_features.is_empty()
                && self.nbrs(pos).iter().any(|neighbor| {
                    self.map
                        .get(*neighbor)
                        .and_then(|tile| tile.feature)
                        .is_some_and(|feature| spec.forbids_adjacent_features.contains(&feature))
                });
            let insufficient_appeal = spec
                .min_appeal
                .is_some_and(|minimum| self.tile_appeal(pos) < minimum);
            let city_limit_reached = spec.one_per_city
                && oc.is_none_or(|city| {
                    self.cities[&city].owned_tiles.iter().any(|position| {
                        self.map.tiles[position].improvement.as_deref() == Some(name.as_str())
                    })
                });
            let territory_invalid = if spec.requires_foreign_territory {
                territory_owner.is_none_or(|owner| {
                    owner == pid
                        || self.is_at_war(pid, owner)
                        || (spec.requires_open_borders && !self.has_open_borders(pid, owner))
                })
            } else {
                territory_owner.map_or_else(
                    || {
                        !spec.can_build_outside_territory
                            // The native mirror puts unattributed held ground
                            // here too: a missing owning city is not proof of
                            // neutral territory. Keep those unknown sites shut.
                            || self.blocked_city_sites.contains(&pos)
                    },
                    |owner| !self.builder_may_improve_territory(pid, owner),
                )
            };
            if seaside_invalid
                || same_adjacent_invalid
                || adjacent_resource_class_invalid
                || adjacent_passable_land_invalid
                || adjacent_water_resource_invalid
                || forbidden_adjacent_feature
                || insufficient_appeal
                || city_limit_reached
                || territory_invalid
            {
                continue;
            }
            match visible_resource {
                Some(resource) => {
                    let stock_improvement = &self.rules.resources[resource].improvement;
                    if !spec.resources.iter().any(|candidate| candidate == resource)
                        && stock_improvement != name
                    {
                        continue;
                    }
                }
                None if spec.resource_only => continue,
                // An undiscovered deposit cannot veto an otherwise legal
                // improvement: the player's observed board has no resource
                // here and must agree with execution until it is revealed.
                None => {}
            }
            // Unique replacements suppress their base improvement for that civ.
            if self.rules.improvements.values().any(|candidate| {
                candidate.replaces == Some(*name)
                    && candidate
                        .unique_to
                        .as_deref()
                        .is_some_and(|owner| self.owns_civ_unique(pid, owner))
            }) {
                continue;
            }
            out.push(*name);
        }
        out.sort();
        out
    }

    pub(super) fn valid_excavation(&self, pid: usize, pos: Pos) -> Option<String> {
        self.excavation_at(pid, pos)
            .filter(|_| self.can_house_additional_great_work(pid, "artifact"))
    }

    /// The excavation this hex offers, leaving aside whether the empire has
    /// anywhere to put another Artifact. That last question is about the
    /// empire, not the hex — it assigns every Great Work the player owns
    /// across every slot in every city, twice — so a caller sweeping the map
    /// asks it once rather than at every tile.
    pub(super) fn excavation_at(&self, pid: usize, pos: Pos) -> Option<String> {
        let tile = self.map.get(pos)?;
        if tile.flooded
            || tile.submerged
            || tile.improvement.as_deref() == Some("national_park")
            || tile.district.is_some()
            || tile.district_foundation.is_some()
            || tile.wonder.is_some()
            || self.city_at(pos).is_some()
        {
            return None;
        }
        let improvement = match tile.resource.as_deref()? {
            "antiquity_site" => "archaeological_dig",
            "shipwreck" => "shipwreck_excavation",
            _ => return None,
        };
        let spec = &self.rules.improvements[improvement];
        if !self.resource_visible_to(pid, tile.resource.as_deref().unwrap())
            || !self.unlocked(pid, &spec.tech, &spec.civic)
            || self.rules.is_water(tile) != spec.water
        {
            return None;
        }
        let territory_owner = tile
            .owner_city
            .and_then(|city| self.cities.get(&city))
            .map(|city| city.owner);
        if territory_owner.is_some_and(|owner| {
            owner != pid
                && (self.is_at_war(pid, owner)
                    || (!self.has_open_borders(pid, owner)
                        && self.empire_wonder_effect(pid, "archaeologist_open_borders") <= 0.0))
        }) {
            return None;
        }
        Some(improvement.to_string())
    }

    /// Exact completion awards for a district project in this city. Keeping
    /// this calculation shared lets AI search value the same whole-percent
    /// progression, Governor, government, and Congress modifiers that the
    /// authoritative completion path will eventually apply.
    pub(crate) fn project_completion_gpp_awards(
        &self,
        pid: usize,
        cid: u32,
        project: &str,
    ) -> BTreeMap<String, f64> {
        let Some(spec) = self.rules.projects.get(project) else {
            return BTreeMap::new();
        };
        let progress = self.game_progress_ratio(pid);
        let city_multiplier = 1.0 + self.governor_effect(pid, cid, "great_people_pct") / 100.0;
        let empire_multiplier = 1.0
            + (self.gov_effects(pid).great_people_pct
                + self.policy_effect(pid, "great_people_pct"))
                / 100.0;
        spec.completion_gpp
            .iter()
            .map(|(kind, base_points)| {
                // PointProgressionParam1=800: interpolate from the base
                // award to eight times that award, then floor before local
                // and empire modifiers.
                let points = (base_points * (1.0 + 7.0 * progress)).floor();
                let congress_multiplier = if self.congress_effect_active("patronage", "A", kind) {
                    2.0
                } else if self.congress_effect_active("patronage", "B", kind) {
                    0.0
                } else {
                    1.0
                };
                (
                    kind.clone(),
                    points * city_multiplier * empire_multiplier * congress_multiplier,
                )
            })
            .collect()
    }

    pub(super) fn city_active_project_effect(&self, city: &City, effect: &str) -> f64 {
        let Some(Item::Project { project }) = city.queue.first() else {
            return 0.0;
        };
        self.rules.projects.get(project).map_or(0.0, |spec| {
            if self.project_has_active_district(city, spec) {
                spec.effects.get(effect).copied().unwrap_or(0.0)
            } else {
                0.0
            }
        })
    }

    /// COST_PROGRESSION_GAME_PROGRESS and district scaling both use the
    /// civilization's farther-advanced tree, truncated to a whole percent.
    pub(super) fn game_progress_ratio(&self, pid: usize) -> f64 {
        let researched_techs = self.players[pid]
            .techs
            .iter()
            .filter(|technology| self.rules.techs.contains_key(technology))
            .count();
        let researched_civics = self.players[pid]
            .civics
            .iter()
            .filter(|civic| self.rules.civics.contains_key(civic))
            .count();
        let progress = (researched_techs as f64 / self.rules.techs.len().max(1) as f64)
            .max(researched_civics as f64 / self.rules.civics.len().max(1) as f64);
        (100.0 * progress).floor() / 100.0
    }

    pub(super) fn converted_power_plant(project: &str) -> Option<&'static str> {
        match project {
            "convert_reactor_to_coal" => Some("coal_power_plant"),
            "convert_reactor_to_oil" => Some("oil_power_plant"),
            "convert_reactor_to_uranium" => Some("nuclear_power_plant"),
            _ => None,
        }
    }

    pub(crate) fn item_progress_key(item: &Item) -> String {
        match item {
            Item::Formation { unit, formation } => format!("formation:{unit}:{formation}"),
            Item::Unit { unit } => format!("unit:{unit}"),
            Item::Building { building } => format!("building:{building}"),
            Item::District { district, pos } => {
                format!("district:{district}:{},{}", pos.0, pos.1)
            }
            Item::Wonder { wonder, pos } => format!("wonder:{wonder}:{},{}", pos.0, pos.1),
            Item::Repair { repair, pos } => {
                format!("repair:{repair}:{},{}", pos.0, pos.1)
            }
            Item::Project { project } => format!("project:{project}"),
            Item::Product { product } => format!("product:{product}"),
        }
    }

    pub(super) fn unit_resource_is_committed(&self, cid: u32, item: &Item) -> bool {
        let cost = self.unit_resource_cost(cid, item);
        cost > 0.0
            && self.cities[&cid]
                .strategic_resource_commitments
                .get(&Self::item_progress_key(item))
                .is_some_and(|amount| *amount >= cost)
    }

    /// Commit a unit's one-time strategic material at construction start.
    /// Pausing and later resuming the same unit retains that commitment.
    pub(super) fn commit_unit_resource(&mut self, pid: usize, cid: u32, item: &Item) -> bool {
        let unit = match item {
            Item::Unit { unit } | Item::Formation { unit, .. } => unit,
            _ => return true,
        };
        let spec = &self.rules.units[unit];
        let Some(resource) = spec.requires_resource else {
            return true;
        };
        let cost = self.unit_resource_cost(cid, item);
        if cost <= 0.0 || self.unit_resource_is_committed(cid, item) {
            return true;
        }
        let stock = self.strategic_stockpile(pid, resource);
        if stock + f64::EPSILON < cost {
            return false;
        }
        self.players[pid]
            .strategic_resources
            .insert(Name::new(&resource), stock - cost);
        self.cities
            .get_mut(&cid)
            .unwrap()
            .strategic_resource_commitments
            .insert(Self::item_progress_key(item), cost);
        true
    }

    /// Resolve a generic upgrade target to this civilization's unique
    /// replacement. For example, Nubian Slingers advance to Pitati Archers
    /// instead of creating Archers that Nubia is not allowed to train.
    pub fn player_unit_replacement(&self, pid: usize, unit: impl AsName) -> Name {
        let unit = unit.as_name();
        // On an arena with unique units switched off, no civilization has a
        // replacement — which has to be said here and not only at the build
        // menu, because this function is also what *suppresses* a stock unit
        // for the civ that replaces it. Refusing the Hoplite alone would
        // leave Greece unable to field a Spearman either.
        if self.is_arena() && !self.tactics.unique_units {
            return unit;
        }
        self.rules
            .units
            .iter()
            .find(|(_, spec)| {
                spec.replaces == Some(unit)
                    && spec
                        .unique_to
                        .as_deref()
                        .is_some_and(|owner| self.owns_civ_unique(pid, owner))
            })
            .map(|(name, _)| *name)
            .unwrap_or(unit)
    }

    /// Direct, currently unlocked successor for one unit kind. Upgrade
    /// actions deliberately advance one link per turn, matching Civ VI and
    /// preserving the production-cost basis of every intermediate step.
    pub fn unit_upgrade_target(&self, pid: usize, unit: impl AsName) -> Option<Name> {
        let base = self.rules.units[unit.as_name()].upgrade_to?;
        let target = self.player_unit_replacement(pid, base);
        let spec = self.rules.units.get(&target)?;
        (spec.buildable && self.unlocked(pid, &spec.tech, &spec.civic)).then_some(target)
    }

    /// Whether a game *opening* in `era` fields this unit: the knowledge
    /// [`Self::open_in_start_era`] grants for that era — everything of the
    /// eras before it — covers the unit's unlocking technology and civic. A
    /// unit with no gate is every era's. This is what caps each rung of an
    /// era-pool army at its own age.
    pub(super) fn unit_within_era(&self, unit: Name, era: usize) -> bool {
        let Some(spec) = self.rules.units.get(&unit) else {
            return false;
        };
        let known = |gate: &Option<Name>, tree: &SpecMap<crate::rules::TechSpec>| {
            gate.as_ref()
                .and_then(|name| tree.get(name))
                .is_none_or(|entry| entry.era < era)
        };
        known(&spec.tech, &self.rules.techs) && known(&spec.civic, &self.rules.civics)
    }

    /// Seat Gathering Storm's targeted aid request at its native trigger.
    /// Unlike congress competitions the affected empire receives aid and
    /// cannot score in its own request.
    pub(super) fn open_native_aid_request(
        &mut self,
        target: usize,
        trigger: NativeCompetitionTrigger,
    ) {
        if !self.native_competitions || self.competition.is_some() || !self.victory_eligible(target)
        {
            return;
        }
        let Some(competition) = Self::NATIVE_COMPETITIONS.iter().find(|competition| {
            competition.trigger == trigger
                // No aid request carries a civilization requirement today, so
                // this changes nothing now. It is here because the gate belongs
                // to the spec rather than to the congress path: a trait-gated
                // competition added on another trigger must not slip past it.
                && self.game_meets_competition_trait(competition)
                && self
                    .competition_lockout_until
                    .get(competition.kind)
                    .is_none_or(|until| self.turn >= *until)
        }) else {
            return;
        };
        let any_member_can_score = self
            .players
            .iter()
            .filter(|player| player.id != target && self.victory_eligible(player.id))
            .any(|player| self.can_score_competition(player.id, competition.kind));
        if !any_member_can_score {
            return;
        }
        self.competition = Some(Competition {
            kind: competition.kind.to_string(),
            ends: self.turn + self.standard_duration(competition.duration),
            target: Some(target),
            scores: BTreeMap::new(),
        });
        self.query_memo.producible.borrow_mut().clear();
    }

    /// Replace the host's menus (see [`Game::host_buildable`]). Mirror input
    /// like `replace_blocked_production`, and for the same reason it clears a
    /// production menu memoised before the export arrived.
    pub(crate) fn replace_host_menus(
        &mut self,
        buildable: BTreeMap<u32, BTreeMap<String, HostMenuEntry>>,
        purchasable: BTreeMap<u32, BTreeMap<String, HostPurchaseEntry>>,
        district_plots: BTreeMap<u32, BTreeMap<Name, BTreeSet<Pos>>>,
    ) {
        self.host_buildable = Arc::new(buildable);
        self.host_purchasable = Arc::new(purchasable);
        self.host_district_plots = Arc::new(district_plots);
        self.query_memo.producible.borrow_mut().clear();
    }

    /// Replace the host's saved work on paused items (see
    /// [`Game::host_saved_work`]). Mirror input, refreshed with the menus.
    pub(crate) fn replace_host_saved_work(&mut self, saved: BTreeMap<u32, BTreeMap<String, f64>>) {
        self.host_saved_work = Arc::new(saved);
    }
}

/// The tie-break between two equally valued citizen jobs. It was the text
/// `tile:{x:+06}:{y:+06}` or `specialist:{district}:{index:03}`, formatted for
/// every workable plot of every plan and cloned for every pair each swap pass
/// compared. The derived order is that text's order: `specialist` sorts before
/// `tile`, and past a coordinate's sign (`+` before `-`) the zero-padded digits
/// rise with the magnitude, so a coordinate compares as `(negative, |value|)`.
/// Specialists are few and keep their text.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CitizenJobKey {
    Specialist(String),
    Tile((bool, u32), (bool, u32)),
}

impl CitizenJobKey {
    fn tile(pos: Pos) -> Self {
        let part = |value: i32| (value < 0, value.unsigned_abs());
        CitizenJobKey::Tile(part(pos.0), part(pos.1))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod hidden_resource_improvement_tests;

#[cfg(test)]
mod hidden_resource_district_tests;

#[cfg(test)]
mod hidden_resource_wonder_tests;

#[cfg(test)]
mod builder_ownership_tests;
