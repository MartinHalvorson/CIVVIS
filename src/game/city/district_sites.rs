//! District sites: where a city may place a district or a wonder.
//!
//! A city's specialty-district capacity and whether it accepts a new
//! district site, the district types available to it, the sites a district
//! or wonder may go on (`district_sites`, `wonder_sites`), the underbuilt
//! discount and a placement's cost, the sites the live host offers, the
//! excavation sites an Archaeologist may dig, and whether a project's
//! district is active.
//!
//! Moved verbatim out of the `impl Game` block in `game/city.rs`
//! (2026-11-05). Methods the rest of `city` calls are `pub(super)`;
//! methods that were `pub(super)` (all of `game`) are
//! `pub(in crate::game)`, the same reach; the code is otherwise unchanged.

use super::*;

impl Game {
    pub(crate) fn excavation_sites(&self, pid: usize) -> Vec<(Pos, String)> {
        if !self.can_house_additional_great_work(pid, "artifact") {
            return Vec::new();
        }
        self.map
            .tiles
            .keys()
            .copied()
            .filter_map(|position| {
                self.excavation_at(pid, position)
                    .map(|improvement| (position, improvement))
            })
            .collect()
    }

    pub(crate) fn city_specialty_district_capacity(&self, city: &City) -> usize {
        self.host_district_capacity
            .get(&city.id)
            .copied()
            .unwrap_or_else(|| 1 + (city.pop.max(1) - 1) as usize / 3)
    }

    /// The city-level gates a new district site must pass before any tile
    /// is looked at: the family caps, the exclusion list, and the specialty
    /// capacity. Split out of `district_sites` unchanged so a planner can
    /// ask whether owning one more plot could open a site at all — when
    /// these refuse, no plot purchase can help
    /// (`ai/advanced/district_planning.rs`).
    pub(crate) fn city_accepts_new_district_site(&self, city: &City, dname: Name) -> bool {
        let spec = &self.rules.districts[dname];
        let city_family_count = city
            .districts
            .keys()
            .filter(|built| self.district_is_family(*built, dname))
            .count()
            + self.city_foundation_count(city, Some(Name::new(&dname)));
        let empire_family_count = self
            .cities
            .values()
            .filter(|candidate| candidate.owner == city.owner)
            .map(|candidate| {
                candidate
                    .districts
                    .keys()
                    .filter(|built| self.district_is_family(*built, dname))
                    .count()
                    + self.city_foundation_count(candidate, Some(Name::new(&dname)))
            })
            .sum::<usize>();
        let blocked_new_site = !spec.buildable
            || spec
                .max_per_city
                .is_some_and(|limit| city_family_count >= limit)
            || spec
                .max_per_empire
                .is_some_and(|limit| empire_family_count >= limit)
            || spec
                .excludes
                .iter()
                .any(|excluded| self.city_has_district_or_foundation_family(city, excluded));
        let specialty_capacity_full = if spec.specialty {
            let capacity = self.city_specialty_district_capacity(city);
            let built = city
                .districts
                .keys()
                .filter(|name| self.rules.districts[name].specialty)
                .count()
                + city
                    .owned_tiles
                    .iter()
                    .filter_map(|position| self.map.tiles[position].district_foundation.as_ref())
                    .filter(|foundation| self.rules.districts[foundation.district].specialty)
                    .count();
            built >= capacity
        } else {
            false
        };
        !(blocked_new_site || specialty_capacity_full)
    }

    pub fn district_sites(&self, cid: u32, dname: impl AsName) -> Vec<Pos> {
        let dname = dname.as_name();
        // A positive answer from the host is stronger than our reconstructed
        // placement model. These coordinates are kept only for the short refusal
        // window and only when their tile is on the mirrored board; if the export
        // did not reveal one, retain the ordinary fallback below.
        if let Some(sites) = self
            .host_district_sites
            .get(&cid)
            .and_then(|by_district| by_district.get(&dname))
        {
            let sites: Vec<Pos> = sites
                .iter()
                .copied()
                .filter(|position| self.map.tiles.contains_key(position))
                .collect();
            if !sites.is_empty() {
                return sites;
            }
        }
        // A district the HOST refused to place IN THIS CITY. Empty in an ordinary
        // game; see `blocked_districts` for why this is per city and not global.
        if self
            .blocked_districts
            .get(&cid)
            .is_some_and(|blocked| blocked.contains(&dname))
        {
            return vec![];
        }
        let city = &self.cities[&cid];
        let spec = &self.rules.districts[dname];
        let mut out: Vec<Pos> = city
            .owned_tiles
            .iter()
            .copied()
            .filter(|position| {
                self.map.tiles[position]
                    .district_foundation
                    .as_ref()
                    .is_some_and(|foundation| foundation.district == dname)
            })
            .collect();
        if !self.city_accepts_new_district_site(city, dname) {
            out.sort();
            return self.host_offered_district_sites(cid, dname, out);
        }
        for pos in &city.owned_tiles {
            if *pos == city.pos || self.wdist(*pos, city.pos) > 3 {
                continue;
            }
            let t = &self.map.tiles[pos];
            if t.flooded
                || t.district.is_some()
                || t.district_foundation.is_some()
                || t.wonder.is_some()
                || t.improvement.as_deref() == Some("national_park")
                || !self.rules.is_passable(t)
            {
                continue;
            }
            if t.feature.as_ref().is_some_and(|feature| {
                self.rules
                    .features
                    .get(feature.as_str())
                    .is_some_and(|feature| feature.natural_wonder || feature.blocks_district)
            }) {
                continue;
            }
            // Undiscovered deposits cannot veto a site the owner observes as
            // resource-free, including through the removal-technology check.
            let visible_resource = t
                .resource
                .as_deref()
                .filter(|resource| self.resource_visible_to(city.owner, resource));
            // Antiquity Sites and Shipwrecks are buried, not deposits: they
            // are invisible until Natural History or Cultural Heritage and
            // never reserve a tile. Building over one destroys it, exactly as
            // a Bonus resource is destroyed.
            if visible_resource.is_some_and(|resource| {
                !matches!(
                    self.rules.resources[resource].class.as_str(),
                    "bonus" | "artifact"
                )
            }) {
                continue;
            }
            let removal_tech = match t.feature.as_deref() {
                Some("forest") => Some("mining"),
                Some("jungle") => Some("bronze_working"),
                Some("marsh") => Some("irrigation"),
                _ => None,
            };
            let vietnam_specialty = self.players[city.owner].civ == "Vietnam" && spec.specialty;
            if vietnam_specialty
                && !matches!(t.feature.as_deref(), Some("forest" | "jungle" | "marsh"))
            {
                continue;
            }
            if !vietnam_specialty
                && removal_tech
                    .is_some_and(|tech| !self.players[city.owner].techs.contains(&Name::new(tech)))
            {
                continue;
            }
            if let Some(resource) = visible_resource {
                let improvement = &self.rules.resources[resource].improvement;
                if self.rules.improvements[improvement]
                    .tech
                    .as_ref()
                    .is_some_and(|tech| !self.players[city.owner].techs.contains(tech))
                {
                    continue;
                }
            }
            let is_water = self.rules.is_water(t);
            let neighbors = self.nbrs(*pos);
            let adjacent_city = neighbors.contains(&city.pos);
            let adjacent_any_city = neighbors
                .iter()
                .any(|neighbor| self.city_at(*neighbor).is_some());
            let adjacent_land = neighbors.iter().any(|neighbor| {
                self.map
                    .get(*neighbor)
                    .is_some_and(|tile| !self.rules.is_water(tile))
            });
            let adjacent_water = |neighbor: Pos| {
                self.map
                    .get(neighbor)
                    .is_some_and(|tile| self.rules.is_water(tile))
            };
            let valid = match spec.placement.as_str() {
                "coast" => {
                    is_water
                        && matches!(t.terrain.as_str(), "coast" | "lake")
                        && t.feature.as_deref() != Some("reef")
                        && adjacent_land
                }
                "water_park" => {
                    is_water
                        && matches!(t.terrain.as_str(), "coast" | "lake")
                        && t.feature.as_deref() != Some("reef")
                        && adjacent_land
                }
                "flat_land" => !is_water && !t.hills,
                "hills" => !is_water && t.hills,
                "hills_adjacent_city" => !is_water && t.hills && adjacent_city,
                "not_adjacent_city" => !is_water && !adjacent_any_city,
                "forest" => !is_water && matches!(t.feature.as_deref(), Some("forest" | "jungle")),
                "vietnam_feature" => {
                    !is_water && matches!(t.feature.as_deref(), Some("forest" | "jungle" | "marsh"))
                }
                "aqueduct" => {
                    let center_edge = self.map.direction_to(*pos, city.pos);
                    let river_source = t
                        .river_edges
                        .iter()
                        .enumerate()
                        .any(|(edge, present)| *present && Some(edge) != center_edge);
                    let water_source = river_source
                        || neighbors.iter().any(|neighbor| {
                            self.map.get(*neighbor).is_some_and(|tile| {
                                tile.terrain == "mountain"
                                    || tile.feature.as_deref() == Some("oasis")
                                    || tile.terrain == "lake"
                            })
                        });
                    !is_water && adjacent_city && water_source
                }
                "dam" => {
                    !is_water
                        && matches!(
                            t.feature.as_deref(),
                            Some("floodplains" | "grassland_floodplains" | "plains_floodplains")
                        )
                        && self.dam_has_available_river(*pos)
                }
                "canal" => {
                    let connections: Vec<usize> = neighbors
                        .iter()
                        .enumerate()
                        .filter(|(_, neighbor)| {
                            **neighbor == city.pos || adjacent_water(**neighbor)
                        })
                        .map(|(index, _)| index)
                        .collect();
                    !is_water
                        && !t.hills
                        && connections.iter().any(|a| {
                            connections.iter().any(|b| {
                                let difference = (*a as i32 - *b as i32).abs();
                                difference.min(6 - difference) >= 2
                            })
                        })
                }
                "land" | "" => !is_water && !spec.water,
                _ => self.rules.is_water(t) == spec.water,
            };
            if !valid {
                continue;
            }
            if self.players[city.owner].civ == "Gaul" && spec.specialty && adjacent_city {
                continue;
            }
            out.push(*pos);
        }
        out.sort();
        self.host_offered_district_sites(cid, dname, out)
    }

    pub fn wonder_sites(&self, cid: u32, wname: &str) -> Vec<Pos> {
        let city = &self.cities[&cid];
        let spec = &self.rules.wonders[wname];
        if self.host_unavailable_wonders.contains(&Name::new(wname)) {
            return Vec::new();
        }
        // A positive host answer is stronger than our reconstructed placement
        // model. Keep only fresh mirrored tiles; an incomplete map must retain
        // the ordinary fallback rather than fabricate a wonder site.
        if let Some(sites) = self
            .host_wonder_sites
            .get(&cid)
            .and_then(|by_wonder| by_wonder.get(&Name::new(wname)))
        {
            let sites: Vec<Pos> = sites
                .iter()
                .copied()
                .filter(|position| self.map.tiles.contains_key(position))
                .collect();
            if !sites.is_empty() {
                return sites;
            }
        }
        // A host that has already said it has no ground for this wonder HERE is
        // answering about the city, not about one tile, so there is nothing left to
        // offer and no point re-deriving a site next turn. See `blocked_wonders`.
        if self
            .blocked_wonders
            .get(&cid)
            .is_some_and(|blocked| blocked.contains(&Name::new(wname)))
        {
            return Vec::new();
        }
        if !self.unlocked(city.owner, &spec.tech, &spec.civic)
            || spec
                .requires_buildings
                .iter()
                .any(|required| !self.city_has_building_family(city, *required))
            || (!spec.requires_any_buildings.is_empty()
                && !spec
                    .requires_any_buildings
                    .iter()
                    .any(|required| self.city_has_building_family(city, *required)))
            || (spec.founded_religion && self.players[city.owner].religion.is_none())
            || (spec
                .effects
                .get("free_warrior_monks")
                .copied()
                .unwrap_or(0.0)
                > 0.0
                && self.players[city.owner].religion.is_none()
                && self.city_religion(city).is_none())
            || self.wonder_built(wname)
        {
            return Vec::new();
        }
        let mut out = Vec::new();
        for pos in &city.owned_tiles {
            if *pos == city.pos || self.wdist(*pos, city.pos) > 3 {
                continue;
            }
            let tile = &self.map.tiles[pos];
            if tile.flooded
                || tile.district.is_some()
                || tile.district_foundation.is_some()
                || tile.wonder.is_some()
                || tile.improvement.as_deref() == Some("national_park")
                || (!self.rules.is_passable(tile) && spec.placement != "mountain")
                || tile.feature.as_ref().is_some_and(|feature| {
                    let feature = &self.rules.features[feature];
                    feature.natural_wonder || feature.impassable
                })
                || tile.resource.as_ref().is_some_and(|resource| {
                    // A buried Artifact reserves nothing — see `district_sites`.
                    // As with districts, an undiscovered deposit cannot veto
                    // a site its owner observes as resource-free. Discovery
                    // also cannot invalidate a wonder already placed here,
                    // including one paused in favor of another queue head.
                    self.resource_visible_to(city.owner, resource)
                        && !matches!(
                            self.rules.resources[resource].class.as_str(),
                            "bonus" | "artifact"
                        )
                        && !city.queue.first().is_some_and(|item| {
                            matches!(item, Item::Wonder { wonder, pos: placed }
                                if wonder == wname && placed == pos)
                        })
                        && !city
                            .production_progress
                            .contains_key(&Self::item_progress_key(&Item::Wonder {
                                wonder: Name::new(wname),
                                pos: *pos,
                            }))
                })
            {
                continue;
            }
            let is_water = self.rules.is_water(tile);
            if is_water != spec.water
                || (!spec.terrain.is_empty() && !spec.terrain.contains(&tile.terrain))
                || (!spec.feature.is_empty()
                    && !tile
                        .feature
                        .as_ref()
                        .is_some_and(|feature| spec.feature.contains(feature)))
                || spec.hills.is_some_and(|hills| tile.hills != hills)
                || (spec.river && !tile.has_river())
            {
                continue;
            }
            let neighbors = self.nbrs(*pos);
            if spec.coast {
                let valid_coast = if spec.water {
                    tile.terrain == "coast"
                        && neighbors.iter().any(|neighbor| {
                            self.map
                                .get(*neighbor)
                                .is_some_and(|candidate| !self.rules.is_water(candidate))
                        })
                } else {
                    neighbors.iter().any(|neighbor| {
                        self.map
                            .get(*neighbor)
                            .is_some_and(|candidate| candidate.terrain == "coast")
                    })
                };
                if !valid_coast {
                    continue;
                }
            }
            if spec.adjacent_mountain
                && !neighbors.iter().any(|neighbor| {
                    self.map
                        .get(*neighbor)
                        .is_some_and(|candidate| candidate.terrain == "mountain")
                })
            {
                continue;
            }
            if spec.adjacent_district.as_ref().is_some_and(|required| {
                !neighbors.iter().any(|neighbor| {
                    if required == "city_center" {
                        return self.city_at(*neighbor).is_some();
                    }
                    self.map.get(*neighbor).is_some_and(|candidate| {
                        candidate.district.is_some_and(|district| {
                            self.district_is_family(district, Name::new(required))
                        })
                    })
                })
            }) {
                continue;
            }
            if spec.adjacent_resource.as_ref().is_some_and(|required| {
                !neighbors.iter().any(|neighbor| {
                    self.map.get(*neighbor).is_some_and(|candidate| {
                        candidate.resource.as_deref() == Some(required.as_str())
                    })
                })
            }) {
                continue;
            }
            if spec.adjacent_improvement.as_ref().is_some_and(|required| {
                !neighbors.iter().any(|neighbor| {
                    self.map.get(*neighbor).is_some_and(|candidate| {
                        candidate.improvement.as_deref() == Some(required.as_str())
                    })
                })
            }) {
                continue;
            }
            let special_valid = match spec.placement.as_str() {
                "adjacent_capital" => neighbors.iter().any(|neighbor| {
                    self.city_at(*neighbor).is_some_and(|candidate| {
                        self.cities[&candidate].owner == city.owner
                            && self.city_capital_flag(&self.cities[&candidate])
                    })
                }),
                "panama_canal" => {
                    let connections: Vec<usize> = neighbors
                        .iter()
                        .enumerate()
                        .filter(|(_, neighbor)| {
                            **neighbor == city.pos
                                || self.map.get(**neighbor).is_some_and(|candidate| {
                                    self.rules.is_water(candidate)
                                        || candidate.district.is_some_and(|district| {
                                            self.district_is_family(district, crate::name!("canal"))
                                        })
                                })
                        })
                        .map(|(index, _)| index)
                        .collect();
                    !tile.hills
                        && connections.iter().any(|a| {
                            connections.iter().any(|b| {
                                let difference = (*a as i32 - *b as i32).abs();
                                difference.min(6 - difference) >= 2
                            })
                        })
                }
                "golden_gate_bridge" => {
                    let land_sides: Vec<usize> = neighbors
                        .iter()
                        .enumerate()
                        .filter(|(_, neighbor)| {
                            self.map
                                .get(**neighbor)
                                .is_some_and(|candidate| !self.rules.is_water(candidate))
                        })
                        .map(|(index, _)| index)
                        .collect();
                    land_sides.iter().any(|a| {
                        land_sides
                            .iter()
                            .any(|b| (*a as i32 - *b as i32).abs() == 3)
                    })
                }
                "lake_adjacent_land" => neighbors.iter().any(|neighbor| {
                    self.map
                        .get(*neighbor)
                        .is_some_and(|candidate| !self.rules.is_water(candidate))
                }),
                _ => true,
            };
            if special_valid {
                out.push(*pos);
            }
        }
        out.sort();
        out
    }

    pub(in crate::game) fn project_has_active_district(
        &self,
        city: &City,
        project: &crate::rules::ProjectSpec,
    ) -> bool {
        project.district.is_none_or(|district| {
            std::iter::once(district)
                .chain(
                    project
                        .alternate_districts
                        .iter()
                        .map(|name| Name::new(name.as_str())),
                )
                .any(|family| self.city_has_active_district_family(city, family))
        })
    }

    pub(in crate::game) fn district_type_available(&self, pid: usize, district: &str) -> bool {
        let spec = &self.rules.districts[district];
        spec.buildable
            && spec.specialty
            && district != "thanh"
            && self.unlocked(pid, &spec.tech, &spec.civic)
            && spec
                .unique_to
                .as_deref()
                .is_none_or(|civilization| self.owns_civ_unique(pid, civilization))
            && !self.rules.districts.values().any(|candidate| {
                candidate.replaces == Some(Name::new(district))
                    && candidate
                        .unique_to
                        .as_deref()
                        .is_some_and(|owner| self.owns_civ_unique(pid, owner))
            })
    }

    /// Gathering Storm's district discount compares the number of completed
    /// specialty districts (B) with the number of unlocked district families
    /// (A), then discounts an underbuilt family T when C(T) < B / A.
    pub(in crate::game) fn district_underbuilt_discount(
        &self,
        pid: usize,
        district: &str,
        purchased: bool,
    ) -> f64 {
        let spec = &self.rules.districts[district];
        if !spec.specialty || matches!(district, "preserve" | "thanh") {
            return 0.0;
        }
        let available: BTreeSet<String> = self
            .rules
            .districts
            .keys()
            .filter(|candidate| self.district_type_available(pid, candidate))
            .map(|candidate| self.district_family(*candidate).to_string())
            .collect();
        let a = available.len();
        if a == 0 {
            return 0.0;
        }
        let mut completed = 0usize;
        let mut target_placed = 0usize;
        let target_family = self.district_family(Name::new(district));
        for city in self.cities.values().filter(|city| city.owner == pid) {
            for built in city.districts.keys() {
                let family = self.district_family(*built);
                if built != "thanh" && available.contains(family.as_str()) {
                    completed += 1;
                }
                if family == target_family {
                    target_placed += 1;
                }
            }
            target_placed +=
                self.city_foundation_count(city, Some(Name::new(target_family.as_str())));
        }
        if completed < a
            || target_placed as f64 + f64::EPSILON >= completed as f64 / a as f64
            || (purchased && target_placed == 0)
        {
            return 0.0;
        }
        if matches!(
            target_family.as_str(),
            "government_plaza" | "diplomatic_quarter"
        ) {
            0.25
        } else {
            0.40
        }
    }

    pub(in crate::game) fn district_cost_for_placement(
        &self,
        pid: usize,
        district: &str,
        purchased: bool,
    ) -> f64 {
        let spec = &self.rules.districts[district];
        if self.district_is_family(Name::new(district), crate::name!("spaceport")) {
            return spec.cost;
        }
        // COST_PROGRESSION_NUM_UNDER_AVG_PLUS_TECH truncates the leading
        // tree ratio to a whole percentage point before applying the 1x-10x
        // multiplier (for example, 5/77 = 6%, not 6.49%).
        let progress = self.game_progress_ratio(pid);
        let scaled = (spec.cost * (1.0 + 9.0 * progress)).floor();
        (scaled * (1.0 - self.district_underbuilt_discount(pid, district, purchased))).floor()
    }

    /// Keep only the sites the host's own placement offer includes, when the
    /// export carried the complete offer for this district in this city.
    pub(in crate::game) fn host_offered_district_sites(
        &self,
        cid: u32,
        dname: Name,
        mut sites: Vec<Pos>,
    ) -> Vec<Pos> {
        if let Some(offered) = self
            .host_district_plots
            .get(&cid)
            .and_then(|plots| plots.get(&dname))
        {
            sites.retain(|pos| offered.contains(pos));
        }
        sites
    }
}
