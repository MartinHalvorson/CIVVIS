//! Districts: what a city's districts are, what they yield, and their
//! adjacency.
//!
//! District families and whether a city has one, a foundation or an active
//! one (`district_family`, `city_has_district_family`,
//! `city_has_active_district_family`), where a family sits, the specialty
//! and defensive district counts, a district's strength from its health
//! and whether it is under siege, the effects and building yields a
//! district carries, its housing and amenity, its yields
//! (`district_yields`), and the adjacency bonus with its sources and the
//! assumed-placement variants (`district_adjacency*`).
//!
//! Moved verbatim out of the `impl Game` block in `game/city.rs`
//! (2026-11-05). Methods the rest of `city` calls are `pub(super)`;
//! methods that were `pub(super)` (all of `game`) are
//! `pub(in crate::game)`, the same reach; the code is otherwise unchanged.

use super::*;

impl Game {
    pub(in crate::game) fn city_defense_district_count(&self, city: &City) -> usize {
        city.districts
            .iter()
            .filter(|(district, position)| {
                self.district_is_active(city, district, **position)
                    && !matches!(
                        self.district_family(**district).as_str(),
                        "preserve" | "aqueduct" | "canal" | "dam"
                    )
            })
            .count()
    }

    pub(in crate::game) fn district_strength_from_health(
        &self,
        cid: u32,
        pos: Pos,
        hp: i32,
        wall_hp: i32,
    ) -> f64 {
        let city = &self.cities[&cid];
        let current_best = self
            .units
            .values()
            .filter(|unit| unit.owner == city.owner)
            .map(|unit| self.rules.units[unit.kind].strength)
            .fold(20.0, f64::max);
        let strongest_built = self.players[city.owner]
            .counters
            .get("strongest_unit_built")
            .map(|value| *value as f64)
            .unwrap_or(current_best);
        let mut strength = (strongest_built - 10.0).max(10.0);
        if wall_hp > 0 {
            strength += 3.0
                * city
                    .buildings
                    .iter()
                    .filter(|building| self.rules.buildings[building].outer_defense > 0)
                    .count() as f64;
        }
        strength += 2.0 * self.city_defense_district_count(city) as f64;
        strength += self.tile_defense_bonus(pos);
        if self.city_has_palace(city) {
            strength += 3.0;
        }
        strength += self.policy_effect(city.owner, "city_defense");
        strength += self.city_state_envoy_strength(city.owner);
        let damaged = (10.0 - hp.clamp(0, 100) as f64 / 10.0).round();
        (strength - damaged).max(0.0)
    }

    pub(in crate::game) fn district_under_siege(&self, owner: usize, position: Pos) -> bool {
        self.map.around(position).into_iter().all(|pos| {
            let Some(tile) = self.map.get(pos) else {
                return true;
            };
            if !self.rules.is_passable(tile) {
                return true;
            }
            self.unit_ids_at(pos).iter().any(|id| {
                let unit = &self.units[id];
                unit.owner != owner
                    && self.is_at_war(owner, unit.owner)
                    && self.rules.units[unit.kind].class == "military"
            }) || self.in_enemy_zoc(owner, pos)
        })
    }

    pub(crate) fn district_family(&self, district: impl AsName) -> Name {
        let mut current = district.as_name();
        // Stock replacements are one level deep. Following the full chain
        // makes modded replacements compose without scattered name lists.
        for _ in 0..self.rules.districts.len() {
            let Some(parent) = self
                .rules
                .districts
                .get_interned(current)
                .and_then(|spec| spec.replaces)
            else {
                break;
            };
            current = parent;
        }
        current
    }

    pub(in crate::game) fn district_is_family(
        &self,
        district: impl AsName,
        family: impl AsName,
    ) -> bool {
        // A district is trivially of its own family, and asking a city whether
        // it holds a Campus mostly means comparing "campus" with "campus".
        let (district, family) = (district.as_name(), family.as_name());
        district == family || self.district_family(district) == self.district_family(family)
    }

    pub fn city_has_district_family(&self, city: &City, family: impl AsName) -> bool {
        let family = family.as_name();
        if family == "city_center" {
            return true;
        }
        city.districts
            .keys()
            .any(|district| self.district_is_family(district, family))
    }

    /// The districts Civilization VI counts as *specialty* — the ones the
    /// Insulae and Medina Quarter housing cards key off, and the ones that
    /// carry the yield buildings. Public for the same reason it was
    /// `pub(crate)`: every caller asks this instead of keeping a second list
    /// of district families that would drift the first time Firaxis moved
    /// one. `gene_screen` is the caller outside the crate, recording
    /// development per seat.
    pub fn city_specialty_district_count(&self, city: &City) -> usize {
        city.districts
            .keys()
            .filter(|district| self.rules.districts[district].specialty)
            .count()
    }

    pub(in crate::game) fn city_foundation_count(
        &self,
        city: &City,
        family: Option<Name>,
    ) -> usize {
        city.owned_tiles
            .iter()
            .filter_map(|position| self.map.tiles[position].district_foundation.as_ref())
            .filter(|foundation| {
                family.is_none_or(|family| self.district_is_family(foundation.district, family))
            })
            .count()
    }

    pub(in crate::game) fn city_has_district_or_foundation_family(
        &self,
        city: &City,
        family: impl AsName,
    ) -> bool {
        let family = family.as_name();
        self.city_has_district_family(city, family)
            || self.city_foundation_count(city, Some(family)) > 0
    }

    pub(in crate::game) fn district_is_active(
        &self,
        city: &City,
        district: impl AsName,
        position: Pos,
    ) -> bool {
        !self.map.tiles[&position].pillaged
            // Resolve the family only when the city-wide encampment flag
            // could disable it. Yield and upkeep queries ask this for every
            // district/building, almost always with the flag clear.
            && !(city.encampment_pillaged
                && self.district_is_family(district, crate::name!("encampment")))
    }

    pub(in crate::game) fn city_has_active_district_family(
        &self,
        city: &City,
        family: impl AsName,
    ) -> bool {
        let family = family.as_name();
        if family == "city_center" {
            return true;
        }
        city.districts.iter().any(|(district, position)| {
            self.district_is_family(district, family)
                && self.district_is_active(city, district, *position)
        })
    }

    pub(in crate::game) fn city_district_effect(&self, city: &City, effect: &str) -> f64 {
        if !self.rules.effect_index.districts(effect) {
            return 0.0;
        }
        city.districts
            .iter()
            .filter(|(district, position)| self.district_is_active(city, district, **position))
            .map(|(district, _)| {
                self.rules.districts[district]
                    .effects
                    .get(effect)
                    .copied()
                    .unwrap_or(0.0)
            })
            .sum()
    }

    pub(in crate::game) fn district_building_yields(&self, city: &City, building: &str) -> Yields {
        let Some(family) = self.rules.buildings[building].district else {
            return Yields::default();
        };
        let Some(position) = self.city_district_family_position(city, family) else {
            return Yields::default();
        };
        let Some((district, _)) = city.districts.iter().find(|(district, candidate)| {
            **candidate == position && self.district_is_family(district, family)
        }) else {
            return Yields::default();
        };
        if !self.district_is_active(city, district, position) {
            return Yields::default();
        }
        let effects = &self.rules.districts[district].effects;
        Yields {
            food: effects.get("building_food").copied().unwrap_or(0.0),
            production: effects.get("building_production").copied().unwrap_or(0.0),
            gold: effects.get("building_gold").copied().unwrap_or(0.0),
            science: effects.get("building_science").copied().unwrap_or(0.0),
            culture: effects.get("building_culture").copied().unwrap_or(0.0),
            faith: effects.get("building_faith").copied().unwrap_or(0.0),
        }
    }

    pub(crate) fn city_district_family_position(
        &self,
        city: &City,
        family: impl AsName,
    ) -> Option<Pos> {
        city.districts.iter().find_map(|(district, position)| {
            self.district_is_family(district, family)
                .then_some(*position)
        })
    }

    pub(in crate::game) fn city_active_district_family_position(
        &self,
        city: &City,
        family: impl AsName,
    ) -> Option<Pos> {
        city.districts.iter().find_map(|(district, position)| {
            (self.district_is_family(district, family)
                && self.district_is_active(city, district, *position))
            .then_some(*position)
        })
    }

    pub(crate) fn district_housing(&self, district: &str, position: Pos) -> f64 {
        let river_goddess =
            self.pantheon_river_holy_site(district, position, "river_holy_site_housing");
        let spec = &self.rules.districts[district];
        let Some(maximum) = spec.effects.get("appeal_housing_max").copied() else {
            return spec.housing + river_goddess;
        };
        let appeal = self.tile_appeal(position);
        let dynamic: f64 = if maximum >= 6.0 {
            match appeal {
                4.. => 6.0,
                2..=3 => 5.0,
                0..=1 => 4.0,
                -2..=-1 => 3.0,
                _ => 2.0,
            }
        } else {
            match appeal {
                4.. => 3.0,
                2..=3 => 2.0,
                0..=1 => 1.0,
                _ => 0.0,
            }
        };
        spec.housing + dynamic.min(maximum) + river_goddess
    }

    pub(crate) fn district_amenity(&self, district: &str, position: Pos) -> f64 {
        let river_goddess =
            self.pantheon_river_holy_site(district, position, "river_holy_site_amenities");
        let spec = &self.rules.districts[district];
        let geothermal = spec
            .effects
            .get("geothermal_amenity")
            .copied()
            .unwrap_or(0.0);
        spec.amenity
            + river_goddess
            + if geothermal > 0.0
                && self.nbrs(position).into_iter().any(|neighbor| {
                    self.map
                        .get(neighbor)
                        .is_some_and(|tile| tile.feature.as_deref() == Some("geothermal_fissure"))
                })
            {
                geothermal
            } else {
                0.0
            }
    }

    pub fn district_yields(&self, dname: impl AsName, dpos: Pos) -> Yields {
        let dname = dname.as_name();
        if let Some(memo) = self.query_memo.district_yields.borrow().as_ref() {
            if let Some(yields) = memo.get(&(dname, dpos)) {
                return *yields;
            }
        }
        let yields = self.district_yields_uncached(dname, dpos);
        if let Some(memo) = self.query_memo.district_yields.borrow_mut().as_mut() {
            memo.insert((dname, dpos), yields);
        }
        yields
    }

    fn district_yields_uncached(&self, dname: Name, dpos: Pos) -> Yields {
        let spec = &self.rules.districts[dname];
        let mut ys = spec.yields;
        ys.add(self.district_adjacency(dname, dpos, None));
        if let Some(owner) = self
            .map
            .get(dpos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city_id| self.cities.get(&city_id))
            .map(|city| city.owner)
        {
            if self.on_foreign_continent(owner, dpos) {
                ys.gold += spec
                    .effects
                    .get("foreign_continent_gold")
                    .copied()
                    .unwrap_or(0.0);
            }
            let coast_or_lake = matches!(self.map.tiles[&dpos].terrain.as_str(), "coast" | "lake")
                || self.nbrs(dpos).iter().any(|neighbor| {
                    matches!(self.map.tiles[neighbor].terrain.as_str(), "coast" | "lake")
                });
            if coast_or_lake && self.grants_city_state_unique_bonus(owner, "Nan Madol") {
                ys.culture += 2.0;
            }
        }
        ys
    }

    /// Every line of a district's adjacency ledger, in ruleset order, so the
    /// interface can show a player where each point came from instead of only
    /// the total.  Modifiers (Gaul's mines, a doubling policy card) are lines
    /// of their own.
    pub fn district_adjacency_sources(
        &self,
        dname: impl AsName,
        dpos: Pos,
    ) -> Vec<AdjacencySource> {
        let mut detail = Vec::new();
        self.district_adjacency(dname, dpos, Some(&mut detail));
        detail
    }

    /// The adjacency half of [`Game::district_yields`].  `detail` is `None` on
    /// every hot path, so recording the breakdown costs one branch per source
    /// and nothing else.
    pub(in crate::game) fn district_adjacency(
        &self,
        dname: impl AsName,
        dpos: Pos,
        detail: Option<&mut Vec<AdjacencySource>>,
    ) -> Yields {
        self.district_adjacency_assuming(dname, dpos, None, detail)
    }

    /// [`Game::district_adjacency`] with planning assumptions layered in — an
    /// assumed city center (settlement planning) and foundations counted as
    /// the districts they will become (build-order planning).  Every live
    /// yield path passes `None` above and is unchanged; only the adjacency
    /// calculator (`game::adjacency`) passes assumptions.
    pub(crate) fn district_adjacency_assuming(
        &self,
        dname: impl AsName,
        dpos: Pos,
        assume: Option<&crate::game::adjacency::PlanAssumption>,
        detail: Option<&mut Vec<AdjacencySource>>,
    ) -> Yields {
        let dname = dname.as_name();
        let spec = &self.rules.districts[dname];
        if spec.adjacency.is_empty() {
            return Yields::default();
        }
        let family = self.district_family(dname);
        self.district_adjacency_assuming_with_family(dname, dpos, assume, detail, family)
    }

    pub(crate) fn district_adjacency_assuming_with_family(
        &self,
        dname: Name,
        dpos: Pos,
        assume: Option<&crate::game::adjacency::PlanAssumption>,
        detail: Option<&mut Vec<AdjacencySource>>,
        family: Name,
    ) -> Yields {
        let mut neighbors = [None; 6];
        for (index, pos) in self.nbrs(dpos).into_iter().enumerate() {
            neighbors[index] = self.map.get(pos);
        }
        self.district_adjacency_assuming_with_family_and_neighbors(
            dname, dpos, assume, detail, family, &neighbors,
        )
    }

    pub(crate) fn district_adjacency_assuming_with_family_and_neighbors(
        &self,
        dname: Name,
        dpos: Pos,
        assume: Option<&crate::game::adjacency::PlanAssumption>,
        detail: Option<&mut Vec<AdjacencySource>>,
        family: Name,
        neighbors: &[Option<&crate::world::Tile>; 6],
    ) -> Yields {
        self.district_adjacency_assuming_with_family_and_neighbors_cached(
            dname, dpos, assume, detail, family, neighbors, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn district_adjacency_assuming_with_family_and_neighbors_and_district_count(
        &self,
        dname: Name,
        dpos: Pos,
        assume: Option<&crate::game::adjacency::PlanAssumption>,
        detail: Option<&mut Vec<AdjacencySource>>,
        family: Name,
        neighbors: &[Option<&crate::world::Tile>; 6],
        district_count: usize,
    ) -> Yields {
        self.district_adjacency_assuming_with_family_and_neighbors_cached(
            dname,
            dpos,
            assume,
            detail,
            family,
            neighbors,
            Some(district_count),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::game) fn district_adjacency_assuming_with_family_and_neighbors_cached(
        &self,
        dname: Name,
        dpos: Pos,
        assume: Option<&crate::game::adjacency::PlanAssumption>,
        mut detail: Option<&mut Vec<AdjacencySource>>,
        family: Name,
        neighbors: &[Option<&crate::world::Tile>; 6],
        cached_district_count: Option<usize>,
    ) -> Yields {
        let spec = &self.rules.districts[dname];
        let mut adj = Yields::default();
        if !spec.adjacency.is_empty() {
            let tile = &self.map.tiles[&dpos];
            let owner_city = self
                .map
                .get(dpos)
                .and_then(|tile| tile.owner_city)
                .and_then(|city_id| self.cities.get(&city_id));
            let owner = owner_city.map(|city| city.owner);
            let gaul = owner.is_some_and(|pid| self.players[pid].civ == "Gaul");
            let count_uncached = |key: &str, key_family: Option<Name>| -> usize {
                match key {
                    "self" => 1,
                    "river" => usize::from(tile.has_river()),
                    "mountain" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| t.terrain == crate::name!("mountain"))
                        .count(),
                    "forest" | "woods" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| t.feature == Some(crate::name!("forest")))
                        .count(),
                    "rainforest" | "jungle" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| t.feature == Some(crate::name!("jungle")))
                        .count(),
                    "natural_wonder" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| {
                            t.feature.as_ref().is_some_and(|feature| {
                                self.rules
                                    .features
                                    .get(feature.as_str())
                                    .is_some_and(|spec| spec.natural_wonder)
                            })
                        })
                        .count(),
                    "reef" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| {
                            matches!(t.feature.as_deref(), Some("reef" | "great_barrier_reef"))
                        })
                        .count(),
                    "great_barrier_reef" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| t.feature.as_deref() == Some("great_barrier_reef"))
                        .count(),
                    "geothermal_fissure" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| t.feature.as_deref() == Some("geothermal_fissure"))
                        .count(),
                    "pamukkale" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| t.feature.as_deref() == Some("pamukkale"))
                        .count(),
                    // City Centers are districts but are represented by the
                    // city index instead of `Tile::district`.
                    // A PILLAGED district is not adjacent to anything: live
                    // Rome's Campus (run civvis-20260816T200454Z) read "+8
                    // from Campus" beside a Government Plaza and a Holy Site,
                    // "+6" from the turn after the Holy Site was pillaged
                    // (t82) until it was repaired (t96), Natural Philosophy
                    // doubling a base that had lost its district pair.
                    "district" => cached_district_count.unwrap_or_else(|| {
                        neighbors
                            .iter()
                            .flatten()
                            .filter(|t| {
                                !gaul
                                    && ((t.district.is_some() && !t.pillaged)
                                        || self.city_at(t.pos).is_some()
                                        || assume.is_some_and(|plan| {
                                            plan.treats_as_city_center(t.pos)
                                                || (plan.foundations
                                                    && t.district_foundation.is_some())
                                        }))
                            })
                            .count()
                    }),
                    "city_center" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| {
                            t.owner_city
                                .and_then(|cid| self.cities.get(&cid))
                                .is_some_and(|city| city.pos == t.pos)
                                || assume.is_some_and(|plan| plan.treats_as_city_center(t.pos))
                        })
                        .count(),
                    "wonder" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| t.wonder.is_some())
                        .count(),
                    "coast_resource" | "sea_resource" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| self.rules.is_water(t) && t.resource.is_some())
                        .count(),
                    "strategic_resource" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| {
                            t.resource.as_ref().is_some_and(|resource| {
                                self.rules
                                    .resources
                                    .get(resource.as_str())
                                    .is_some_and(|spec| spec.class == "strategic")
                            })
                        })
                        .count(),
                    "luxury_resource" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| {
                            t.resource.as_ref().is_some_and(|resource| {
                                self.rules
                                    .resources
                                    .get(resource.as_str())
                                    .is_some_and(|spec| spec.class == "luxury")
                            })
                        })
                        .count(),
                    "resource" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| t.resource.is_some())
                        .count(),
                    "mine" | "quarry" | "lumber_mill" | "plantation" | "farm" => neighbors
                        .iter()
                        .flatten()
                        .filter(|t| t.improvement.as_deref() == Some(key))
                        .count(),
                    district_family if self.rules.districts.contains_key(district_family) => {
                        // The family this key counts, resolved when the ruleset
                        // loaded. Interning it here instead cost an `RwLock`
                        // read and a hash once per plot per district per key —
                        // about twenty million times in a six-player game, and
                        // the largest single leaf in the settlement scorer at
                        // 2.7% of busy CPU. The fallback keeps the old route
                        // for any caller reaching this arm without a table.
                        let wanted = key_family
                            .unwrap_or_else(|| self.district_family(Name::new(district_family)));
                        neighbors
                            .iter()
                            .flatten()
                            .filter(|t| {
                                (!t.pillaged
                                    && t.district.is_some_and(|district| {
                                        self.district_family(district) == wanted
                                    }))
                                    || assume.is_some_and(|plan| {
                                        plan.foundations
                                            && t.district_foundation.as_ref().is_some_and(
                                                |foundation| {
                                                    self.district_family(foundation.district)
                                                        == wanted
                                                },
                                            )
                                    })
                            })
                            .count()
                    }
                    _ => 0,
                }
            };
            let count = count_uncached;
            let key_families = self.rules.district_adjacency_families.get_interned(dname);
            debug_assert!(
                key_families.is_none_or(|families| families.len() == spec.adjacency.len()),
                "the adjacency family table is built in `spec.adjacency`'s own key order"
            );
            for (index, (key, bonus)) in spec.adjacency.iter().enumerate() {
                let tiles = count(
                    key,
                    key_families.and_then(|families| families.get(index).copied().flatten()),
                );
                let n = tiles as f64;
                // Every source has its own TilesRequired bucket in Civ VI.
                // Fractions from different sources therefore never combine.
                let paid = Yields {
                    food: (n * bonus.food).trunc(),
                    production: (n * bonus.production).trunc(),
                    gold: (n * bonus.gold).trunc(),
                    science: (n * bonus.science).trunc(),
                    culture: (n * bonus.culture).trunc(),
                    faith: (n * bonus.faith).trunc(),
                };
                adj.add(paid);
                if let Some(detail) = detail.as_mut() {
                    // A source counting no tiles is still worth listing when
                    // it is the reason a site was chosen — but an empty line
                    // for every unmet source would bury the ones that pay.
                    if tiles > 0 {
                        detail.push(AdjacencySource {
                            source: key.clone(),
                            count: tiles,
                            percent: 0.0,
                            yields: paid,
                            // Half-point sources bank their remainder, so show
                            // the unrounded figure too: a Campus beside three
                            // districts has 1.5 science and the next one pays.
                            raw: Yields {
                                food: n * bonus.food,
                                production: n * bonus.production,
                                gold: n * bonus.gold,
                                science: n * bonus.science,
                                culture: n * bonus.culture,
                                faith: n * bonus.faith,
                            },
                        });
                    }
                }
            }
            if gaul && spec.specialty {
                let mines = count("mine", None);
                let minor = (mines as f64 * 0.5).trunc();
                let mut paid = Yields::default();
                match family.as_str() {
                    "campus" => paid.science = minor,
                    "holy_site" => paid.faith = minor,
                    "commercial_hub" | "harbor" => paid.gold = minor,
                    "theater_square" => paid.culture = minor,
                    "industrial_zone" => paid.production = minor,
                    _ => {}
                }
                adj.add(paid);
                if let Some(detail) = detail.as_deref_mut() {
                    if mines > 0 && paid != Yields::default() {
                        detail.push(AdjacencySource {
                            source: "gaul_mine".to_string(),
                            count: mines,
                            percent: 0.0,
                            yields: paid,
                            raw: paid,
                        });
                    }
                }
            }
            if owner.is_some_and(|pid| {
                self.empire_wonder_effect(pid, "mountain_commercial_industrial_theater_adjacency")
                    > 0.0
            }) {
                let mountains = count("mountain", None);
                let mut paid = Yields::default();
                match family.as_str() {
                    "commercial_hub" => paid.gold = mountains as f64,
                    "industrial_zone" => paid.production = mountains as f64,
                    "theater_square" => paid.culture = mountains as f64,
                    _ => {}
                }
                adj.add(paid);
                if let Some(detail) = detail.as_deref_mut() {
                    if mountains > 0 && paid != Yields::default() {
                        detail.push(AdjacencySource {
                            source: "wonder_mountain".to_string(),
                            count: mountains,
                            percent: 0.0,
                            yields: paid,
                            raw: paid,
                        });
                    }
                }
            }
            if family == crate::name!("holy_site") {
                // ★ Desert Folklore, Dance of the Aurora and Sacred Path are
                // ONE modifier over three plot tests, so this is one predicate
                // rather than three special cases:
                // `MODIFIER_ALL_CITIES_TERRAIN_ADJACENCY` for the first two and
                // `MODIFIER_ALL_CITIES_FEATURE_ADJACENCY` for the third, each
                // DistrictType `DISTRICT_HOLY_SITE`, YieldType `YIELD_FAITH`,
                // Amount 1, subject `CITY_FOLLOWS_PANTHEON_REQUIREMENTS`.
                // A fourth row of this shape is data, not code.
                //
                // ⚠ Desert Folklore and Dance of the Aurora each ship TWO rows
                // — `..._FAITHDESERTADJACENCY` and `..._FAITHDESERTHILLSADJACENCY`
                // over `TERRAIN_DESERT` and `TERRAIN_DESERT_HILLS` — which is
                // one terrain here because CIVVIS carries hills as a flag on
                // the plot rather than as a terrain of its own. No id in this
                // family appears in `Expansion2_RemoveData.xml`.
                if let Some(pid) = owner {
                    for (effect, plot) in PANTHEON_HOLY_SITE_ADJACENCY {
                        let amount = self.pantheon_effect(pid, effect);
                        if amount == 0.0 {
                            continue;
                        }
                        let tiles = neighbors
                            .iter()
                            .flatten()
                            .filter(|t| t.terrain == plot || t.feature.as_deref() == Some(plot))
                            .count();
                        let paid = Yields {
                            faith: tiles as f64 * amount,
                            ..Yields::default()
                        };
                        adj.add(paid);
                        if let Some(detail) = detail.as_deref_mut() {
                            if tiles > 0 {
                                detail.push(AdjacencySource {
                                    source: format!("pantheon_{plot}"),
                                    count: tiles,
                                    percent: 0.0,
                                    yields: paid,
                                    raw: paid,
                                });
                            }
                        }
                    }
                }
                if let Some(city) = self
                    .map
                    .get(dpos)
                    .and_then(|tile| tile.owner_city)
                    .and_then(|city_id| self.cities.get(&city_id))
                {
                    let woods = count("forest", None);
                    let paid = Yields {
                        faith: woods as f64
                            * self.city_building_effect(city, "holy_site_woods_adjacency"),
                        ..Yields::default()
                    };
                    adj.add(paid);
                    if let Some(detail) = detail.as_deref_mut() {
                        if paid.faith != 0.0 {
                            detail.push(AdjacencySource {
                                source: "building_woods".to_string(),
                                count: woods,
                                percent: 0.0,
                                yields: paid,
                                raw: paid,
                            });
                        }
                    }
                }
            }
            // The six adjacency-card families include unique replacements.
            if let Some(pid) = owner {
                let mut percent = match family.as_str() {
                    "campus" => self.policy_effect(pid, "campus_adjacency_pct"),
                    "holy_site" => self.policy_effect(pid, "holy_site_adjacency_pct"),
                    "commercial_hub" => self.policy_effect(pid, "commercial_hub_adjacency_pct"),
                    "harbor" => self.policy_effect(pid, "harbor_adjacency_pct"),
                    "theater_square" => self.policy_effect(pid, "theater_square_adjacency_pct"),
                    "industrial_zone" => self.policy_effect(pid, "industrial_zone_adjacency_pct"),
                    _ => 0.0,
                };
                if matches!(family.as_str(), "commercial_hub" | "harbor") {
                    if let Some(city) = owner_city {
                        percent +=
                            self.governor_effect(pid, city.id, "commercial_harbor_adjacency_pct");
                    }
                }
                if family == crate::name!("theater_square")
                    && self.grants_city_state_unique_bonus(pid, "Vilnius")
                {
                    percent += 50.0 * self.highest_active_alliance_level(pid) as f64;
                }
                let scale = percent / 100.0;
                let bonus = Yields {
                    food: adj.food * scale,
                    production: adj.production * scale,
                    gold: adj.gold * scale,
                    science: adj.science * scale,
                    culture: adj.culture * scale,
                    faith: adj.faith * scale,
                };
                adj.add(bonus);
                if let Some(detail) = detail.as_mut() {
                    if bonus != Yields::default() {
                        detail.push(AdjacencySource {
                            source: "adjacency_bonus".to_string(),
                            count: 0,
                            percent,
                            yields: bonus,
                            raw: bonus,
                        });
                    }
                }
            }
        }
        adj
    }
}
