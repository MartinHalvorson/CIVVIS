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
        if let Some(oc) = t.owner_city {
            if self.cities[&oc].owner != u.owner {
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

    /// City ranged strike strength: the strongest ranged unit the owner
    /// fields, or 3 if none (Civ 6 rule).
    pub fn city_ranged_strength(&self, cid: u32) -> f64 {
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

    pub(super) fn city_defense_district_count(&self, city: &City) -> usize {
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
        if city.encampment_wall_hp > 0 {
            strength += 3.0
                * city
                    .buildings
                    .iter()
                    .filter(|building| self.rules.buildings[building].outer_defense > 0)
                    .count() as f64;
        }
        strength += 2.0 * self.city_defense_district_count(city) as f64;
        if let Some(position) = self.city_district_family_position(city, crate::name!("encampment"))
        {
            strength += self.tile_defense_bonus(position);
        }
        if self.city_has_palace(city) {
            strength += 3.0;
        }
        strength += self.policy_effect(city.owner, "city_defense");
        strength += self.city_state_envoy_strength(city.owner);
        let damaged = (10.0 - city.encampment_hp.clamp(0, 100) as f64 / 10.0).round();
        (strength - damaged).max(0.0)
    }

    pub(super) fn encampment_take_damage(
        &mut self,
        _attacker: usize,
        cid: u32,
        damage: i32,
        wall_mult: f64,
        bypass_walls: bool,
    ) {
        let (wall, max) = {
            let city = &self.cities[&cid];
            (city.encampment_wall_hp, self.city_max_wall_hp(city))
        };
        let city = self.cities.get_mut(&cid).unwrap();
        city.encampment_last_attacked = self.turn;
        if wall > 0 && max > 0 {
            let fraction = wall as f64 / max as f64;
            let through = if bypass_walls {
                damage
            } else if fraction >= 0.8 {
                1
            } else if fraction >= 0.2 {
                damage / 2
            } else {
                damage
            };
            city.encampment_wall_hp =
                (wall - ((damage as f64 * wall_mult).round() as i32).max(1)).max(0);
            city.encampment_hp -= through.max(1);
        } else {
            city.encampment_hp -= damage;
        }
    }

    pub(super) fn district_under_siege(&self, owner: usize, position: Pos) -> bool {
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

    /// A city cannot heal when every adjacent passable tile is occupied by a
    /// hostile combat unit or covered by hostile ZOC. Off-map and impassable
    /// neighbors count as sealed sides of the siege ring.
    pub(super) fn city_under_siege(&self, cid: u32) -> bool {
        let city = &self.cities[&cid];
        self.governor_effect(city.owner, cid, "city_siege_immunity") <= 0.0
            && self.district_under_siege(city.owner, city.pos)
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

    pub(super) fn district_is_family(&self, district: impl AsName, family: impl AsName) -> bool {
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
    /// Insulae and Medina Quarter housing cards key off. `pub(crate)` so the
    /// policy chooser asks this instead of keeping a second list of district
    /// families that would drift the first time Firaxis moved one.
    pub(crate) fn city_specialty_district_count(&self, city: &City) -> usize {
        city.districts
            .keys()
            .filter(|district| self.rules.districts[district].specialty)
            .count()
    }

    pub(super) fn city_foundation_count(&self, city: &City, family: Option<Name>) -> usize {
        city.owned_tiles
            .iter()
            .filter_map(|position| self.map.tiles[position].district_foundation.as_ref())
            .filter(|foundation| {
                family.is_none_or(|family| self.district_is_family(foundation.district, family))
            })
            .count()
    }

    pub(super) fn city_has_district_or_foundation_family(
        &self,
        city: &City,
        family: impl AsName,
    ) -> bool {
        let family = family.as_name();
        self.city_has_district_family(city, family)
            || self.city_foundation_count(city, Some(family)) > 0
    }

    pub(super) fn district_is_active(
        &self,
        city: &City,
        district: impl AsName,
        position: Pos,
    ) -> bool {
        !self.map.tiles[&position].pillaged
            && !(self.district_is_family(district, crate::name!("encampment"))
                && city.encampment_pillaged)
    }

    pub(super) fn city_has_active_district_family(&self, city: &City, family: impl AsName) -> bool {
        let family = family.as_name();
        if family == "city_center" {
            return true;
        }
        city.districts.iter().any(|(district, position)| {
            self.district_is_family(district, family)
                && self.district_is_active(city, district, *position)
        })
    }

    pub(super) fn city_district_effect(&self, city: &City, effect: &str) -> f64 {
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

    pub(super) fn district_building_yields(&self, city: &City, building: &str) -> Yields {
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

    pub(super) fn city_active_district_family_position(
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

    pub(super) fn home_continent(&self, pid: usize) -> Option<usize> {
        self.cities
            .values()
            .find(|city| city.is_capital && city.original_owner == pid)
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

    /// Gathering Storm appeal of a tile from adjacent terrain, features,
    /// improvements, wonders, and districts. River appeal belongs to the
    /// evaluated tile itself; the other modifiers come from its neighbors.
    /// Appeal of a tile, memoized for the life of a [`Game::query_memo`] scope.
    ///
    /// The answer is a pure function of the tile, its six neighbours, and the
    /// owning city's wonders and Governor -- none of which a read-only scope
    /// changes, which is the same contract [`Game::city_yields`] is cached
    /// under. Outside a scope this computes as it always did, so a caller that
    /// is mutating between reads cannot see a stale figure.
    pub fn tile_appeal(&self, position: Pos) -> i32 {
        if let Some(memo) = self.query_memo.appeal.borrow().as_ref() {
            if let Some(appeal) = memo.get(&position) {
                return *appeal;
            }
        }
        let appeal = self.tile_appeal_uncached(position);
        if let Some(memo) = self.query_memo.appeal.borrow_mut().as_mut() {
            memo.insert(position, appeal);
        }
        appeal
    }

    pub(super) fn tile_appeal_uncached(&self, position: Pos) -> i32 {
        // The host's own count where the export carried one. The derivation
        // below reads the six neighbours the board can see; the host's
        // reads the ones it has, fog included, plus every modifier the model
        // does not carry.
        if let Some(appeal) = self.observed_appeal.get(&position) {
            return *appeal;
        }
        let Some(tile) = self.map.get(position) else {
            return 0;
        };
        let owner_city_id = tile.owner_city;
        let owner = owner_city_id
            .and_then(|city_id| self.cities.get(&city_id))
            .map(|city| city.owner);
        let biosphere = owner
            .map(|pid| self.empire_wonder_effect(pid, "rainforest_marsh_adjacent_appeal"))
            .unwrap_or(0.0) as i32;
        let mut appeal = i32::from(tile.has_river());
        for neighbor in self.nbrs(position) {
            let Some(adjacent) = self.map.get(neighbor) else {
                continue;
            };
            if matches!(adjacent.terrain.as_str(), "mountain" | "coast" | "lake") {
                appeal += 1;
            }
            // Features.Appeal, read rather than listed: Woods and an Oasis are
            // +1, Rainforest, Marsh and Floodplains -1, and a natural wonder is
            // +2 -- except the Cliffs of Dover and Uluru, which are +4.
            appeal += adjacent
                .feature
                .as_deref()
                .and_then(|feature| self.rules.features.get(feature))
                .map_or(0, |spec| spec.appeal.round() as i32);
            if adjacent.owner_city == owner_city_id
                && adjacent.improvement.is_none()
                && adjacent.feature.is_some()
            {
                if let Some(city_id) = owner_city_id {
                    if let Some(city) = self.cities.get(&city_id) {
                        appeal += self.governor_effect(city.owner, city.id, "appeal") as i32;
                    }
                }
            }
            if biosphere != 0 && matches!(adjacent.feature.as_deref(), Some("jungle" | "marsh")) {
                appeal += biosphere;
            }

            if adjacent.wonder.is_some() {
                appeal += 1;
            }
            if adjacent.pillaged {
                appeal -= 1;
            }
            if let Some(district) = adjacent.district {
                appeal += self
                    .rules
                    .districts
                    .get(district.as_str())
                    .map(|spec| spec.appeal.round() as i32)
                    .unwrap_or(0);
            }

            if !adjacent.pillaged {
                appeal += adjacent
                    .improvement
                    .as_deref()
                    .and_then(|improvement| self.rules.improvements.get(improvement))
                    .and_then(|improvement| improvement.effects.get("adjacent_appeal"))
                    .copied()
                    .unwrap_or(0.0) as i32;
            }
        }
        if let Some(owner) = owner {
            appeal += self.empire_wonder_effect(owner, "empire_appeal") as i32;
            appeal += self.empire_wonder_effect(owner, "empire_tile_appeal") as i32;
        }
        if let Some(city) = tile
            .owner_city
            .and_then(|city_id| self.cities.get(&city_id))
        {
            appeal += city
                .wonders
                .keys()
                .map(|wonder| {
                    self.rules.wonders[wonder]
                        .effects
                        .get("city_appeal")
                        .copied()
                        .unwrap_or(0.0) as i32
                })
                .sum::<i32>();
        }
        appeal
    }

    /// The four tiles of a park on a globe: a tile, two of its neighbours that
    /// also touch each other, and the tile opposite. The pair is taken at a
    /// fixed place in the tile's own ring so that one tile names one park.
    pub(super) fn park_rhombus(&self, top: Pos) -> Option<[Pos; 4]> {
        let ring = self.nbrs(top);
        if ring.len() < 6 {
            return None;
        }
        let (left, right) = (ring[4], ring[5]);
        let far = self
            .nbrs(left)
            .into_iter()
            .find(|pos| *pos != top && self.nbrs(right).contains(pos))?;
        Some([top, left, right, far])
    }

    pub(super) fn national_park_diamond(&self, top: Pos) -> Option<[Pos; 4]> {
        // A park is a rhombus: a tile, two neighbours that touch each other,
        // and the tile those two share on the far side. On a flat map that is
        // a fixed set of coordinate offsets; on a globe the same shape is
        // built from the tile's own ring of neighbours, which is the only
        // definition that survives a world with no fixed compass.
        let positions = match self.map.sphere() {
            Some(_) => self.park_rhombus(top)?,
            None => [
                top,
                (top.0 - 1, top.1 + 1),
                (top.0, top.1 + 1),
                (top.0 - 1, top.1 + 2),
            ]
            .map(|position| hex::canon(position, self.map.width)),
        };
        let mut sorted = positions;
        sorted.sort_unstable();
        let distinct = sorted.windows(2).all(|pair| pair[0] != pair[1]);
        (distinct
            && positions
                .iter()
                .all(|position| self.map.tiles.contains_key(position)))
        .then_some(positions)
    }

    pub(super) fn valid_national_park_site(&self, pid: usize, positions: &[Pos; 4]) -> bool {
        let Some(city_id) = self.map.tiles[&positions[0]].owner_city else {
            return false;
        };
        self.cities
            .get(&city_id)
            .is_some_and(|city| city.owner == pid)
            && positions.iter().all(|position| {
                let tile = &self.map.tiles[position];
                let natural_wonder = tile.feature.as_ref().is_some_and(|feature| {
                    self.rules
                        .features
                        .get(feature.as_str())
                        .is_some_and(|feature| feature.natural_wonder)
                });
                tile.owner_city == Some(city_id)
                    && !tile.flooded
                    && !tile.submerged
                    && tile.improvement.is_none()
                    && tile.district.is_none()
                    && tile.district_foundation.is_none()
                    && tile.wonder.is_none()
                    && self.city_at(*position).is_none()
                    && (tile.terrain == "mountain"
                        || natural_wonder
                        || self.tile_appeal(*position) >= 2)
            })
            && positions
                .iter()
                .any(|position| self.rules.is_passable(&self.map.tiles[position]))
    }

    pub fn national_park_sites(&self, pid: usize) -> Vec<[Pos; 4]> {
        if self.tree_effect(pid, "national_parks") <= 0.0 {
            return Vec::new();
        }
        let mut sites: Vec<[Pos; 4]> = self
            .map
            .tiles
            .keys()
            .copied()
            .filter_map(|top| self.national_park_diamond(top))
            .filter(|positions| self.valid_national_park_site(pid, positions))
            .collect();
        sites.sort();
        sites.dedup();
        sites
    }

    pub(super) fn national_park_site_at(&self, pid: usize, position: Pos) -> Option<[Pos; 4]> {
        let possible_tops = [
            position,
            (position.0 + 1, position.1 - 1),
            (position.0, position.1 - 1),
            (position.0 + 1, position.1 - 2),
        ];
        possible_tops
            .into_iter()
            .filter_map(|top| self.national_park_diamond(top))
            .filter(|positions| self.valid_national_park_site(pid, positions))
            .max_by_key(|positions| {
                (
                    positions
                        .iter()
                        .map(|position| self.tile_appeal(*position).max(0))
                        .sum::<i32>(),
                    std::cmp::Reverse(*positions),
                )
            })
    }

    pub(super) fn established_national_parks(&self, pid: usize) -> Vec<(u32, [Pos; 4])> {
        let mut parks = Vec::new();
        let mut used = BTreeSet::new();
        // Every established park has four owned tiles carrying the
        // `national_park` improvement. Derive the small set of possible tops
        // from those tiles instead of probing every tile on the map whenever
        // a city asks for its Amenities.
        let candidate_tops: BTreeSet<Pos> = self
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .flat_map(|city| city.owned_tiles.iter().copied())
            .filter(|position| {
                self.map.tiles[position].improvement.as_deref() == Some("national_park")
            })
            .flat_map(|position| -> Vec<Pos> {
                match self.map.sphere() {
                    // Any tile of a rhombus lies within two steps of its top.
                    Some(_) => self.wdisk(position, 2),
                    None => [
                        position,
                        (position.0 + 1, position.1 - 1),
                        (position.0, position.1 - 1),
                        (position.0 + 1, position.1 - 2),
                    ]
                    .map(|top| hex::canon(top, self.map.width))
                    .to_vec(),
                }
            })
            .collect();
        for top in candidate_tops {
            let Some(positions) = self.national_park_diamond(top) else {
                continue;
            };
            let Some(city_id) = self.map.tiles[&positions[0]].owner_city else {
                continue;
            };
            if self
                .cities
                .get(&city_id)
                .is_some_and(|city| city.owner == pid)
                && positions.iter().all(|position| {
                    let tile = &self.map.tiles[position];
                    tile.owner_city == Some(city_id)
                        && tile.improvement.as_deref() == Some("national_park")
                })
                && positions.iter().all(|position| !used.contains(position))
            {
                used.extend(positions);
                parks.push((city_id, positions));
            }
        }
        parks.sort();
        parks.dedup();
        parks
    }

    /// What River Goddess pays a district standing where the belief asks.
    ///
    /// ★ One predicate for both halves of the belief: Gathering Storm ships
    /// `RIVER_GODDESS_HOLY_SITE_AMENITIES` and `RIVER_GODDESS_HOLY_SITE_HOUSING`
    /// as two modifiers with the SAME subject requirement set,
    /// `PLOT_HAS_HOLY_SITE_RIVER_REQUIREMENTS` (`REQUIRES_PLOT_HAS_HOLY_SITE`
    /// and `REQUIRES_PLOT_ADJACENT_TO_RIVER`, tested ALL), so the plot test
    /// lives here once and the caller names which yield it is collecting.
    ///
    /// ⚠ `Expansion2_RemoveData.xml` DELETES the base game's
    /// `RIVER_GODDESS_HOLY_SITE_AMENITY` — a different id, +1 Amenity through
    /// `MODIFIER_CITY_DISTRICTS_ADJUST_CITY_AMENITIES_FROM_RELIGION` and no
    /// Housing at all. Reading the base row would underpay the Amenity and
    /// miss the Housing outright.
    pub(super) fn pantheon_river_holy_site(
        &self,
        district: &str,
        position: Pos,
        effect: &str,
    ) -> f64 {
        // ⚠ River first, and the district family last. This is asked once per
        // district per city on every Housing and Amenity read, and
        // `Name::new` takes a read lock on the global name registry — the
        // plot's own six river edges settle it for almost every caller
        // without interning anything.
        let Some(tile) = self.map.get(position) else {
            return 0.0;
        };
        if !tile.has_river() {
            return 0.0;
        }
        let Some(amount) = tile
            .owner_city
            .and_then(|city_id| self.cities.get(&city_id))
            .map(|city| self.pantheon_effect(city.owner, effect))
            .filter(|amount| *amount != 0.0)
        else {
            return 0.0;
        };
        if !self.district_is_family(Name::new(district), crate::name!("holy_site")) {
            return 0.0;
        }
        amount
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
    pub(super) fn district_adjacency(
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
    pub(super) fn district_adjacency_assuming_with_family_and_neighbors_cached(
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

    /// Build the automatic citizen governor's priorities from three layers:
    /// survival/growth, this city's current role and production, and the
    /// civilization's strengths.  Re-evaluating it from current state means a
    /// city changes jobs immediately when it starts a wonder, goes to war,
    /// reaches its housing cap, or develops a specialty district.
    pub fn citizen_strategy(&self, cid: u32) -> CitizenStrategy {
        let city = &self.cities[&cid];
        let player = &self.players[city.owner];
        let mut weights = Yields {
            food: 1.25,
            production: 1.55,
            gold: 0.85,
            science: 1.30,
            culture: 1.20,
            faith: 0.90,
        };
        weights.food += player.citizen_food_bias;
        let mut focus = "balanced".to_string();

        // Existing districts make cities lean into their established role.
        // This is intentionally based on the district's actual ruleset yields
        // so modded specialty districts inherit sensible behavior.
        let mut specialty = Yields::default();
        for (name, pos) in &city.districts {
            specialty.add(self.district_yields(name, *pos));
        }
        for name in &city.buildings {
            specialty.add(self.rules.buildings[name].yields);
        }
        weights.production += specialty.production * 0.12;
        weights.gold += specialty.gold * 0.12;
        weights.science += specialty.science * 0.18;
        weights.culture += specialty.culture * 0.18;
        weights.faith += specialty.faith * 0.18;
        let specialties = [
            (specialty.production, "production"),
            (specialty.gold, "commerce"),
            (specialty.science, "science"),
            (specialty.culture, "culture"),
            (specialty.faith, "faith"),
        ];
        if let Some((amount, name)) = specialties
            .into_iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| a.1.cmp(b.1)))
        {
            if amount > 0.0 {
                focus = name.to_string();
            }
        }

        // Production is the immediate city-level plan. Yield-bearing items
        // also reinforce the specialization they are being built to support.
        if let Some(item) = city.queue.first() {
            match item {
                Item::Unit { unit } if unit == "settler" => {
                    focus = "expansion".to_string();
                    weights.food += 0.55;
                    weights.production += 1.15;
                }
                Item::Unit { unit } | Item::Formation { unit, .. } => {
                    focus = if self.rules.units[unit].class == "military" {
                        "military".to_string()
                    } else {
                        "development".to_string()
                    };
                    weights.production += if focus == "military" { 1.35 } else { 0.85 };
                }
                Item::Building { building } => {
                    let spec = &self.rules.buildings[building];
                    focus = if spec.wonder {
                        "wonder"
                    } else {
                        "infrastructure"
                    }
                    .to_string();
                    weights.production += if spec.wonder { 1.75 } else { 0.80 };
                    weights.food += spec.yields.food * 0.10;
                    weights.gold += spec.yields.gold * 0.15;
                    weights.science += spec.yields.science * 0.22;
                    weights.culture += spec.yields.culture * 0.22;
                    weights.faith += spec.yields.faith * 0.22;
                }
                Item::District { district, pos } => {
                    focus = district.replace('_', " ");
                    weights.production += 1.00;
                    let dy = self.district_yields(district, *pos);
                    weights.gold += dy.gold * 0.16;
                    weights.science += dy.science * 0.22;
                    weights.culture += dy.culture * 0.22;
                    weights.faith += dy.faith * 0.22;
                }
                Item::Wonder { wonder, .. } => {
                    let spec = &self.rules.wonders[wonder];
                    focus = "wonder".to_string();
                    weights.production += 1.75;
                    weights.gold += spec.yields.gold * 0.15;
                    weights.science += spec.yields.science * 0.22;
                    weights.culture += spec.yields.culture * 0.22;
                    weights.faith += spec.yields.faith * 0.22;
                }
                Item::Repair { .. } => {
                    focus = "repair".to_string();
                    weights.production += 1.25;
                }
                Item::Project { .. } => {
                    focus = "space race".to_string();
                    weights.production += 1.75;
                    weights.science += 0.60;
                }
                Item::Product { product } => {
                    focus = format!("{product} product");
                    weights.production += 1.25;
                    weights.gold += 0.40;
                    weights.culture += 0.25;
                }
            }
        }

        // Civilization plans use ability keys rather than seat numbers, so a
        // custom ruleset may reorder civilizations without changing behavior.
        // The roster boundary matters here too: historical identities keep a
        // neutral plan even where a legacy rules row shares their name.
        if self.uses_civ6_content(city.owner) {
            match self.rules.civs.get(&player.civ).map(|c| c.ability.as_str()) {
                Some("trajans_column") => {
                    weights.production += 0.30;
                    weights.culture += 0.55;
                }
                Some("iteru") => {
                    weights.production += if self.map.tiles[&city.pos].has_river() {
                        1.00
                    } else {
                        0.55
                    };
                    weights.gold += 0.35;
                }
                Some("platos_republic") => weights.culture += 1.35,
                Some("dynastic_cycle") => {
                    weights.production += 0.30;
                    weights.science += 0.75;
                    weights.culture += 0.75;
                }
                Some("epic_quest") => {
                    weights.production += 0.65;
                    weights.gold += 0.45;
                }
                Some("gifts_for_the_tlatoani") => {
                    weights.food += 0.20;
                    weights.production += 0.85;
                    weights.gold += 0.40;
                }
                Some("ta_seti") => {
                    weights.food += 0.20;
                    weights.production += 1.25;
                }
                Some("killer_of_cyrus") => {
                    weights.food += 0.35;
                    weights.production += 1.05;
                }
                _ => {}
            }
        }

        let at_war = self.players.iter().any(|other| {
            other.id != city.owner
                && other.alive
                && !other.is_barbarian
                && self.is_at_war(city.owner, other.id)
        });
        if at_war {
            weights.production += 1.00;
            if city.queue.is_empty() {
                focus = "wartime".to_string();
            }
        }

        // Everything above is local: the districts standing here, the item in
        // the queue, the civilization, and one empire-wide `at_war` boolean.
        // None of it can see what the empire is trying to *win*, so a seat
        // racing to Mars and a seat feeding a war work the same tiles. A
        // directive is the channel that carries the plan down to the tile, and
        // it is additive on purpose — the local evidence keeps its full weight
        // and the plan tilts the margin.
        let mut growth_ceiling = f64::INFINITY;
        if let Some(directive) = player.city_directives.get(&cid) {
            weights.food += directive.emphasis.food;
            weights.production += directive.emphasis.production;
            weights.gold += directive.emphasis.gold;
            weights.science += directive.emphasis.science;
            weights.culture += directive.emphasis.culture;
            weights.faith += directive.emphasis.faith;

            match directive.role {
                CityRole::Balanced => {}
                CityRole::Forge => weights.production += 0.70,
                CityRole::Breadbasket => weights.food += 0.70,
                // Double down on whatever this city already earns most of,
                // reusing the specialty vector measured above rather than
                // guessing a second time.
                CityRole::Specialist => match focus.as_str() {
                    "production" => weights.production += 0.55,
                    "commerce" => weights.gold += 0.55,
                    "science" => weights.science += 0.55,
                    "culture" => weights.culture += 0.55,
                    "faith" => weights.faith += 0.55,
                    _ => {}
                },
                CityRole::Bastion => {
                    weights.production += 0.90;
                    if city.queue.is_empty() {
                        focus = "besieged".to_string();
                    }
                }
            }

            // Military awareness, on its own axis from the role: how hard the
            // city is being pressed scales the hammer appetite whatever it is
            // otherwise for, so a Forge two tiles from an enemy army starts
            // paying for its own defense before the alarm names it a Bastion.
            // Capped so a hopeless ratio cannot drive the appetite arbitrarily
            // high.
            if directive.pressure > 0.0 {
                weights.production += directive.pressure.min(2.0) * 0.80;
            }
            if directive.halt_growth {
                weights.food *= 0.70;
                growth_ceiling = 0.0;
            }
            if directive.role != CityRole::Balanced && focus == "balanced" {
                focus = directive.role.as_str().to_string();
            }
        }

        let housing_headroom = self.city_housing(city) - city.pop as f64;
        let amenities = self.city_amenity_surplus(city);
        let growth_surplus = if housing_headroom > 1.0 && amenities >= -2 {
            (0.75 + housing_headroom * 0.25).min(2.0)
        } else {
            // Do not sacrifice useful production/science to grow into a hard
            // housing cap; the food constraint below still prevents starvation.
            weights.food *= 0.55;
            0.0
        };
        let food_target = 2.0 * city.pop as f64 + growth_surplus.min(growth_ceiling);
        CitizenStrategy {
            focus,
            weights,
            food_target,
        }
    }

    pub(super) fn citizen_value(ys: Yields, weights: Yields) -> f64 {
        ys.food * weights.food
            + ys.production * weights.production
            + ys.gold * weights.gold
            + ys.science * weights.science
            + ys.culture * weights.culture
            + ys.faith * weights.faith
    }

    /// Return one entry for every specialist slot in an active specialty
    /// district. The district supplies the base specialist yield; tier-three
    /// buildings improve every citizen in that district, and every worship
    /// building adds the standard +1 Faith specialist bonus.
    pub(super) fn city_specialist_jobs(&self, city: &City) -> Vec<(String, Yields)> {
        let mut districts = std::collections::BTreeMap::<String, Yields>::new();
        for (district, position) in &city.districts {
            if self.map.tiles[position].pillaged
                || (self.district_is_family(district, crate::name!("encampment"))
                    && city.encampment_pillaged)
            {
                continue;
            }
            let family = self.district_family(*district).to_string();
            let base = self
                .rules
                .districts
                .get(&family)
                .or_else(|| self.rules.districts.get(district.as_str()))
                .map(|spec| spec.citizen_yields)
                .unwrap_or_default();
            if base != Yields::default() {
                districts.entry(family).or_insert(base);
            }
        }

        let mut jobs = Vec::new();
        for (family, mut yields) in districts {
            let mut slots = 0;
            for building_name in &city.buildings {
                if city.pillaged_buildings.contains(building_name)
                    || (city.encampment_pillaged
                        && self.rules.buildings[building_name]
                            .district
                            .is_some_and(|district| self.district_family(district) == "encampment"))
                {
                    continue;
                }
                let building = &self.rules.buildings[building_name];
                if building
                    .district
                    .is_none_or(|district| self.district_family(district) != family)
                {
                    continue;
                }
                slots += building.citizen_slots.max(0) as usize;
                yields.food += building.effects.get("citizen_food").copied().unwrap_or(0.0);
                yields.production += building
                    .effects
                    .get("citizen_production")
                    .copied()
                    .unwrap_or(0.0);
                yields.gold += building.effects.get("citizen_gold").copied().unwrap_or(0.0);
                yields.science += building
                    .effects
                    .get("citizen_science")
                    .copied()
                    .unwrap_or(0.0);
                yields.culture += building
                    .effects
                    .get("citizen_culture")
                    .copied()
                    .unwrap_or(0.0);
                yields.faith += building
                    .effects
                    .get("citizen_faith")
                    .copied()
                    .unwrap_or(0.0);
                if building.worship_belief.is_some() {
                    yields.faith += 1.0;
                }
            }
            jobs.extend((0..slots).map(|_| (family.clone(), yields)));
        }
        jobs
    }

    /// Choose the actual population-worked tiles. It exhausts food-bearing and
    /// other usable tiles before falling back to specialists, while choosing
    /// by strategic value within each tier. It then performs the least-cost
    /// swaps needed to hit the food target. A final local improvement pass
    /// recovers strategic value without violating nutrition. This keeps the
    /// hot turn loop fast while preventing a production-focused governor from
    /// starving a city.
    pub fn city_citizen_plan(&self, cid: u32) -> CitizenPlan {
        self.city_citizen_plan_weighted(cid, None)
    }

    /// The same citizen assignment under a substituted weight vector.
    ///
    /// `None` is exactly [`Self::city_citizen_plan`] — this is additive and
    /// changes no behaviour. It exists so an instrument can ask what a city
    /// *would* work under different appetites without the engine adopting
    /// them, which is how `docs/OPENINGS.md` bounds the food ceiling on the
    /// capital: expansion is gated by capital growth, and the shipped weights
    /// value production at 1.55 against food at 1.25.
    pub fn city_citizen_plan_weighted(&self, cid: u32, weights: Option<Yields>) -> CitizenPlan {
        let city = &self.cities[&cid];
        let mut strategy = self.citizen_strategy(cid);
        if let Some(weights) = weights {
            strategy.weights = weights;
        }
        // A live Firaxis mirror has already made its citizen assignment. Use that
        // exact baseline for ordinary planning and yields; weighted probes remain
        // counterfactual and deliberately run CIVVIS's governor below.
        if weights.is_none() {
            if let Some(worked_tiles) = self.observed_city_worked_tiles.get(&cid) {
                return CitizenPlan {
                    strategy,
                    worked_tiles: worked_tiles.clone(),
                    specialists: self
                        .observed_city_specialists
                        .get(&cid)
                        .cloned()
                        .unwrap_or_default(),
                };
            }
        }
        let mut center = self.workable_tile_yields(city.pos);
        center.food = center.food.max(2.0);
        center.production = center.production.max(1.0);

        const FOOD_TILE_TIER: u8 = 0;
        const USABLE_TILE_TIER: u8 = 1;
        const SPECIALIST_TIER: u8 = 2;
        const BARREN_TILE_TIER: u8 = 3;

        #[derive(Clone)]
        struct Job {
            key: String,
            pos: Option<Pos>,
            specialist: Option<String>,
            yields: Yields,
            value: f64,
            // Lower tiers are exhausted first. A specialist should not pull
            // an early citizen off usable ground just because its district
            // yield matches the city's current focus; barren plots remain a
            // lower fallback so a city with no usable tiles can still employ
            // its available specialist slots.
            fallback_tier: u8,
        }

        let growth_supported = strategy.food_target > 2.0 * city.pop.max(0) as f64 + 1e-9;

        let mut cands: Vec<Job> = city
            .owned_tiles
            .iter()
            .filter(|pos| **pos != city.pos)
            .filter_map(|pos| {
                let tile = &self.map.tiles[pos];
                if tile.district.is_some()
                    || tile.district_foundation.is_some()
                    || tile.wonder.is_some()
                {
                    return None;
                }
                if tile.improvement.as_deref().is_some_and(|improvement| {
                    self.rules.improvements[improvement]
                        .effects
                        .get("unworkable")
                        .copied()
                        .unwrap_or(0.0)
                        > 0.0
                }) {
                    return None;
                }
                let ys = self.workable_tile_yields(*pos);
                let fallback_tier = if growth_supported && ys.food > 0.0 {
                    FOOD_TILE_TIER
                } else if ys.total() > 0.0 {
                    USABLE_TILE_TIER
                } else {
                    BARREN_TILE_TIER
                };
                Some(Job {
                    key: format!("tile:{:+06}:{:+06}", pos.0, pos.1),
                    pos: Some(*pos),
                    specialist: None,
                    yields: ys,
                    value: Self::citizen_value(ys, strategy.weights),
                    fallback_tier,
                })
            })
            .collect();
        for (index, (district, yields)) in self.city_specialist_jobs(city).into_iter().enumerate() {
            cands.push(Job {
                key: format!("specialist:{district}:{index:03}"),
                pos: None,
                specialist: Some(district),
                yields,
                value: Self::citizen_value(yields, strategy.weights),
                fallback_tier: SPECIALIST_TIER,
            });
        }
        cands.sort_by(|a, b| {
            a.fallback_tier
                .cmp(&b.fallback_tier)
                .then_with(|| b.value.partial_cmp(&a.value).unwrap())
                .then(a.key.cmp(&b.key))
        });
        let workers = (city.pop.max(0) as usize).min(cands.len());
        let mut selected = vec![false; cands.len()];
        for slot in selected.iter_mut().take(workers) {
            *slot = true;
        }
        // Fixed food (buildings, districts, routes, envoys, beliefs) satisfies
        // nutrition before citizens are pulled off more valuable jobs. This is
        // important for granary/harbor cities: food infrastructure should let
        // their population work production or specialty yields.
        let mut food = center.food;
        for (name, pos) in &city.districts {
            food += self.district_yields(name, *pos).food;
        }
        for name in &city.buildings {
            food += self.rules.buildings[name].yields.food;
        }
        for route in self.routes.iter().filter(|r| r.origin == cid) {
            if let Some(dest) = self.cities.get(&route.dest) {
                food += self.route_yields(route.dest, dest.owner == city.owner).food;
            }
        }
        if !self.players[city.owner].is_minor {
            food += self.envoy_yields(city.owner, city).food;
        }
        if self.city_religion(city).is_some() {
            if self.city_has_active_building_family(city, crate::name!("shrine")) {
                food += self.city_religion_belief_effect(city, "shrine_food");
            }
            if self.city_has_active_building_family(city, crate::name!("temple")) {
                food += self.city_religion_belief_effect(city, "temple_food");
            }
        }
        food += cands
            .iter()
            .enumerate()
            .filter(|(i, _)| selected[*i])
            .map(|(_, c)| c.yields.food)
            .sum::<f64>();

        // Meet nutrition through the smallest strategic-value sacrifice per
        // useful food. The loop is bounded by the candidate count because
        // every accepted swap strictly raises collected food.
        for _ in 0..cands.len() {
            if food + 1e-9 >= strategy.food_target {
                break;
            }
            let need = strategy.food_target - food;
            let mut best: Option<(f64, f64, String, String, usize, usize)> = None;
            for (out, a) in cands.iter().enumerate().filter(|(i, _)| selected[*i]) {
                for (inside, b) in cands.iter().enumerate().filter(|(i, _)| !selected[*i]) {
                    let food_gain = b.yields.food - a.yields.food;
                    if food_gain <= 1e-9 {
                        continue;
                    }
                    let value_gain = b.value - a.value;
                    let useful_food = food_gain.min(need);
                    let efficiency = value_gain / useful_food;
                    let candidate = (
                        efficiency,
                        value_gain,
                        a.key.clone(),
                        b.key.clone(),
                        out,
                        inside,
                    );
                    if best
                        .as_ref()
                        .map(|old| {
                            candidate.0 > old.0 + 1e-9
                                || ((candidate.0 - old.0).abs() < 1e-9
                                    && (candidate.1 > old.1 + 1e-9
                                        || ((candidate.1 - old.1).abs() < 1e-9
                                            && (candidate.2.as_str(), candidate.3.as_str())
                                                < (old.2.as_str(), old.3.as_str()))))
                        })
                        .unwrap_or(true)
                    {
                        best = Some(candidate);
                    }
                }
            }
            match best {
                Some((_, _, _, _, out, inside)) => {
                    selected[out] = false;
                    selected[inside] = true;
                    food += cands[inside].yields.food - cands[out].yields.food;
                }
                None => break,
            }
        }

        // One-swap local optimum under the nutrition constraint.
        for _ in 0..cands.len() {
            let mut best: Option<(f64, String, String, usize, usize)> = None;
            for (out, a) in cands.iter().enumerate().filter(|(i, _)| selected[*i]) {
                for (inside, b) in cands.iter().enumerate().filter(|(i, _)| !selected[*i]) {
                    if b.fallback_tier > a.fallback_tier {
                        continue;
                    }
                    let value_gain = b.value - a.value;
                    let next_food = food + b.yields.food - a.yields.food;
                    if value_gain <= 1e-9 || next_food + 1e-9 < strategy.food_target {
                        continue;
                    }
                    let candidate = (value_gain, a.key.clone(), b.key.clone(), out, inside);
                    if best
                        .as_ref()
                        .map(|old| {
                            candidate.0 > old.0 + 1e-9
                                || ((candidate.0 - old.0).abs() < 1e-9
                                    && (candidate.1.as_str(), candidate.2.as_str())
                                        < (old.1.as_str(), old.2.as_str()))
                        })
                        .unwrap_or(true)
                    {
                        best = Some(candidate);
                    }
                }
            }
            match best {
                Some((_, _, _, out, inside)) => {
                    selected[out] = false;
                    selected[inside] = true;
                    food += cands[inside].yields.food - cands[out].yields.food;
                }
                None => break,
            }
        }

        let mut worked_tiles: Vec<Pos> = cands
            .iter()
            .enumerate()
            .filter(|(i, _)| selected[*i])
            .filter_map(|(_, c)| c.pos)
            .collect();
        worked_tiles.sort();
        let mut specialists: Vec<String> = cands
            .iter()
            .enumerate()
            .filter(|(i, _)| selected[*i])
            .filter_map(|(_, c)| c.specialist.clone())
            .collect();
        specialists.sort();
        CitizenPlan {
            strategy,
            worked_tiles,
            specialists,
        }
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
        // their techs, their civics, their improvements. ⚠ The Nazca Line loop
        // that follows stays here rather than moving down with the ground: two
        // of its four terms read the owner's civics and techs, and a Nazca Line
        // stands inside a border by construction, so an unowned neighbour of
        // one is an edge the split does not need to carry.
        let mut yields = self.ground_tile_yields(pos, tile);
        for neighbor in self.nbrs(pos) {
            let adjacent = &self.map.tiles[&neighbor];
            if adjacent.pillaged || adjacent.improvement.as_deref() != Some("nazca_line") {
                continue;
            }
            let effects = &self.rules.improvements["nazca_line"].effects;
            yields.faith += effects.get("adjacent_faith").copied().unwrap_or(0.0);
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
            if !tile.hills
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
            yields.production += if self.world_era >= 4 { 2.0 } else { 1.0 };
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

    /// CIVVIS's own tile model, before any host correction: what
    /// [`Self::workable_tile_yields`] pays on a native game, and the number the
    /// mirror measures the host's per-plot export against.
    pub fn modeled_tile_yields(&self, pos: Pos) -> Yields {
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
            *self.query_memo.traversal.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.air_patrols.borrow_mut() = None;
            *self.query_memo.passage_improvements.borrow_mut() = None;
            *self.query_memo.movement.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.amenities.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.purchase_price.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.unit_ids.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.unit_territory_access.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.city_ids.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.lux_alloc.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.lux_names.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.housed_works.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.suzerain.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.gw_slots.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.gw_housing.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.regional.borrow_mut() = Some(BTreeMap::new());
            *self.query_memo.wonder_effects.borrow_mut() = Some(BTreeMap::new());
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
        let yields = self.city_yields_uncached(cid);
        if let Some(memo) = self.query_memo.yields.borrow_mut().as_mut() {
            memo.insert(cid, yields);
        }
        yields
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
        for b in &city.buildings {
            if city.pillaged_buildings.contains(b)
                || !self.building_district_is_active(city, b)
                || (city.encampment_pillaged
                    && self.rules.buildings[b].district == Some(crate::name!("encampment")))
            {
                continue;
            }
            let building = &self.rules.buildings[b];
            let mut yields = if building.regional_range > 0 {
                Yields::default()
            } else {
                building.yields
            };
            let built_era = city.building_eras.get(b).copied().unwrap_or(self.world_era);
            yields.faith += building
                .effects
                .get("faith_per_era_since_built")
                .copied()
                .unwrap_or(0.0)
                * self.world_era.saturating_sub(built_era) as f64;
            if city.loyalty >= 100.0 - f64::EPSILON {
                yields.culture += building
                    .effects
                    .get("full_loyalty_culture")
                    .copied()
                    .unwrap_or(0.0);
            }
            if matches!(self.players[city.owner].age.as_str(), "golden" | "heroic") {
                let multiplier = 1.0
                    + building
                        .effects
                        .get("golden_age_faith_tourism_pct")
                        .copied()
                        .unwrap_or(0.0)
                        / 100.0;
                yields.faith *= multiplier;
            }
            yields.add(self.district_building_yields(city, b));
            if building.regional_range <= 0 && self.city_is_powered(city) {
                Self::add_powered_building_yields(building, &mut yields);
            }
            for (family, counter) in [
                ("library", "great_person:library_science"),
                ("university", "great_person:university_science"),
                ("research_lab", "great_person:research_lab_science"),
            ] {
                if self.building_is_family(b, Name::new(family)) {
                    yields.science += self.players[city.owner]
                        .counters
                        .get(counter)
                        .copied()
                        .unwrap_or(0) as f64;
                }
            }
            if self.building_is_family(b, crate::name!("workshop")) {
                yields.culture += self.players[city.owner]
                    .counters
                    .get("great_person:workshop_culture")
                    .copied()
                    .unwrap_or(0) as f64;
            }
            // Rationalism, Free Market, Grand Opera and Simultaneum each pay
            // in two halves in Gathering Storm rather than one flat double:
            // half where the city has 15 Population, half where the district
            // already earns 4 of that yield from adjacency —
            // REQUIREMENT_CITY_HAS_HIGH_ADJACENCY_DISTRICT Amount=4, read on
            // the district's OWN adjacency, before Natural Philosophy and its
            // kin double it: live Ostia's Campus showed "+6" (3 doubled) and
            // Antium's "+4" (2 doubled) with Rationalism slotted, and neither
            // city's Library or University earned a point from it (run
            // civvis-20260816T233226Z t153-169).
            if let Some(family) = building.district {
                let (key, yield_of) = match family.as_str() {
                    "campus" => ("campus_building_science_pct", 0),
                    "commercial_hub" => ("commercial_building_gold_pct", 1),
                    "theater_square" => ("theater_building_culture_pct", 2),
                    "holy_site" => ("holy_site_building_faith_pct", 3),
                    _ => ("", usize::MAX),
                };
                if yield_of != usize::MAX {
                    let half = self.policy_effect(city.owner, key) / 2.0;
                    let mut pct = 0.0;
                    if city.pop >= 15 {
                        pct += half;
                    }
                    if self
                        .city_district_family_position(city, family)
                        .map(|position| {
                            let placed = self.map.tiles[&position].district.unwrap_or(family);
                            let mut adjacency = Yields::default();
                            for source in self.district_adjacency_sources(placed, position) {
                                if source.source != "adjacency_bonus" {
                                    adjacency.add(source.yields);
                                }
                            }
                            match yield_of {
                                0 => adjacency.science,
                                1 => adjacency.gold,
                                2 => adjacency.culture,
                                _ => adjacency.faith,
                            }
                        })
                        .unwrap_or(0.0)
                        >= 4.0
                    {
                        pct += half;
                    }
                    let multiplier = 1.0 + pct / 100.0;
                    match yield_of {
                        0 => yields.science *= multiplier,
                        1 => yields.gold *= multiplier,
                        2 => yields.culture *= multiplier,
                        _ => yields.faith *= multiplier,
                    }
                }
            }
            if let Some(district) = building.district {
                if let Some(position) = self.city_district_family_position(city, district) {
                    let placed = self.map.tiles[&position].district.unwrap_or(district);
                    let mut adjacency = self.district_yields(Name::new(placed.as_str()), position);
                    let base = self.rules.districts[placed].yields;
                    adjacency.food -= base.food;
                    adjacency.production -= base.production;
                    adjacency.gold -= base.gold;
                    adjacency.science -= base.science;
                    adjacency.culture -= base.culture;
                    adjacency.faith -= base.faith;
                    yields.production += building
                        .effects
                        .get("production_equal_harbor_adjacency")
                        .copied()
                        .unwrap_or(0.0)
                        * adjacency.gold;
                    yields.production += building
                        .effects
                        .get("production_equal_industrial_adjacency")
                        .copied()
                        .unwrap_or(0.0)
                        * adjacency.production;
                    yields.faith += building
                        .effects
                        .get("faith_equal_campus_adjacency")
                        .copied()
                        .unwrap_or(0.0)
                        * adjacency.science;
                    yields.science += building
                        .effects
                        .get("science_equal_harbor_adjacency")
                        .copied()
                        .unwrap_or(0.0)
                        * adjacency.gold;
                    yields.gold += building
                        .effects
                        .get("gold_equal_campus_adjacency")
                        .copied()
                        .unwrap_or(0.0)
                        * adjacency.science;
                    yields.culture += building
                        .effects
                        .get("culture_equal_commercial_adjacency")
                        .copied()
                        .unwrap_or(0.0)
                        * adjacency.gold;
                }
            }
            if self.players[city.owner].civ == "Vietnam" {
                if let Some(family) = building.district {
                    let family = self.district_family(Name::new(&family));
                    if self
                        .rules
                        .districts
                        .get(family.as_str())
                        .is_some_and(|district| district.specialty)
                    {
                        if let Some(position) = self.city_district_family_position(city, family) {
                            let amount = if self.world_era >= 4 {
                                3.0
                            } else if self.world_era >= 2 {
                                2.0
                            } else {
                                1.0
                            };
                            match self.map.tiles[&position].feature.as_deref() {
                                Some("forest") => yields.culture += amount,
                                Some("jungle") => yields.science += amount,
                                Some("marsh") => yields.production += amount,
                                _ => {}
                            }
                        }
                    }
                }
            }
            if matches!(
                b.as_str(),
                "military_academy" | "seaport" | "renaissance_walls"
            ) {
                yields.science += self.policy_effect(city.owner, "late_military_building_science");
            }
            if matches!(
                b.as_str(),
                "military_academy"
                    | "research_lab"
                    | "coal_power_plant"
                    | "oil_power_plant"
                    | "nuclear_power_plant"
            ) {
                yields.culture += self.policy_effect(city.owner, "late_building_culture");
                yields.gold += self.policy_effect(city.owner, "late_building_gold");
            }
            yields.science += building
                .effects
                .get("coastal_or_lake_tile_science")
                .copied()
                .unwrap_or(0.0)
                * city
                    .owned_tiles
                    .iter()
                    .filter(|position| {
                        matches!(self.map.tiles[position].terrain.as_str(), "coast" | "lake")
                    })
                    .count() as f64;
            yields.culture += building
                .effects
                .get("culture_per_population")
                .copied()
                .unwrap_or(0.0)
                * city.pop as f64;
            if self.players[city.owner]
                .techs
                .contains(&crate::name!("electricity"))
            {
                yields.culture += building
                    .effects
                    .get("electricity_culture")
                    .copied()
                    .unwrap_or(0.0);
            }
            yields.add(self.building_modifier_yields(city, b));
            ys.add(yields);
        }
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
        for r in self.routes.iter().filter(|r| r.origin == cid) {
            if let Some(dc) = self.cities.get(&r.dest) {
                let domestic = dc.owner == city.owner;
                let mut rys = self.trade_route_yields(city.owner, r.dest);
                rys.gold += self.trading_post_route_gold(city.owner, cid, r.dest);
                if self.grants_city_state_unique_bonus(city.owner, "Chinguetti") {
                    if let Some(religion) = self.players[city.owner]
                        .religion
                        .as_deref()
                        .or_else(|| self.city_religion(city))
                    {
                        rys.faith += self.religious_followers_in_city(city, religion);
                    }
                }
                if self.grants_city_state_unique_bonus(city.owner, "Hunza") {
                    rys.gold += (self.wdist(city.pos, dc.pos) / 5) as f64;
                }
                if !domestic
                    && self.players[dc.owner].is_minor
                    && !self.players[dc.owner].is_barbarian
                    && self.grants_city_state_unique_bonus(city.owner, "Kumasi")
                {
                    let specialty_districts = city
                        .districts
                        .iter()
                        .filter(|(district, position)| {
                            self.district_is_active(city, district, **position)
                                && self
                                    .rules
                                    .districts
                                    .get(district)
                                    .is_some_and(|spec| spec.specialty)
                        })
                        .count() as f64;
                    rys.culture += 2.0 * specialty_districts;
                    rys.gold += specialty_districts;
                }
                let route_multiplier = self.route_district_gold_multiplier(city.pos, dc.pos);
                if route_multiplier > 1.0 {
                    let district_gold = dc
                        .districts
                        .iter()
                        .filter(|(district, position)| {
                            self.district_is_active(dc, district, **position)
                        })
                        .map(
                            |(district, _)| match self.district_family(*district).as_str() {
                                "commercial_hub" | "harbor" => 3.0,
                                "government_plaza" => 2.0,
                                _ => 0.0,
                            },
                        )
                        .sum::<f64>();
                    rys.gold += district_gold * (route_multiplier - 1.0);
                }
                rys.gold += self.policy_effect(city.owner, "trade_gold");
                rys.food += self.policy_effect(city.owner, "trade_food");
                rys.production += self.policy_effect(city.owner, "trade_production");
                if domestic {
                    // Isolationism pays only at home, and pays all three.
                    rys.food += self.policy_effect(city.owner, "domestic_trade_food");
                    // Isolationism's rows carry `Intercontinental=0`: only a
                    // domestic route that stays on one continent pays them.
                    if self.same_continent(city.pos, dc.pos) {
                        rys.food +=
                            self.policy_effect(city.owner, "domestic_same_continent_trade_food");
                        rys.production += self
                            .policy_effect(city.owner, "domestic_same_continent_trade_production");
                    }
                    rys.production += self.policy_effect(city.owner, "domestic_trade_production");
                    rys.gold += self.policy_effect(city.owner, "domestic_trade_gold");
                }
                if !domestic {
                    if self.grants_city_state_unique_bonus(city.owner, "Bandar Brunei") {
                        let posts = self
                            .players
                            .iter()
                            .filter(|player| {
                                player
                                    .counters
                                    .contains_key(&format!("trading_post_city:{}", dc.id))
                            })
                            .count() as f64;
                        rys.gold += posts;
                    }
                    if self.grants_city_state_unique_bonus(city.owner, "Venice") {
                        rys.gold +=
                            dc.owned_tiles
                                .iter()
                                .filter(|position| {
                                    self.map.tiles[position].resource.as_ref().is_some_and(
                                        |resource| self.rules.resources[resource].class == "luxury",
                                    )
                                })
                                .count() as f64;
                    }
                    rys.gold += self.trading_dome_origin_route_gold(cid);
                    // E-Commerce pays only on international routes.
                    rys.gold += self.policy_effect(city.owner, "international_trade_gold");
                    rys.production +=
                        self.policy_effect(city.owner, "international_trade_production");
                    // Trade Confederation and Market Economy both ship as
                    // ADJUST_TRADE_ROUTE_YIELD_FOR_INTERNATIONAL, and they are
                    // the only owners of these two yields anywhere in the
                    // policy tree, so no domestic route earns either.
                    rys.culture += self.policy_effect(city.owner, "international_trade_culture");
                    rys.science += self.policy_effect(city.owner, "international_trade_science");
                    // Market Economy's Gold is not a flat amount: it ships as
                    // PER_DESTINATION_LUXURY_RESOURCE and
                    // PER_DESTINATION_STRATEGIC_RESOURCE, one Gold each, so it
                    // is worth what the far city actually owns.
                    let luxury = self.policy_effect(city.owner, "international_luxury_gold");
                    let strategic = self.policy_effect(city.owner, "international_strategic_gold");
                    if luxury != 0.0 || strategic != 0.0 {
                        let owned = |class: &str| {
                            dc.owned_tiles
                                .iter()
                                .filter(|position| {
                                    self.map.tiles[position].resource.as_ref().is_some_and(
                                        |resource| self.rules.resources[resource].class == class,
                                    )
                                })
                                .count() as f64
                        };
                        rys.gold += luxury * owned("luxury") + strategic * owned("strategic");
                    }
                }
                rys.faith += self.policy_effect(city.owner, "trade_faith");
                rys.food += self.governor_effect(dc.owner, dc.id, "incoming_trade_food");
                if !domestic
                    && self.city_has_active_district_family(city, crate::name!("holy_site"))
                {
                    let religious_community =
                        self.city_religion_belief_effect(city, "trade_gold_per_holy_building");
                    if religious_community > 0.0 {
                        let holy_site_buildings = city
                            .buildings
                            .iter()
                            .filter(|building| {
                                !city.pillaged_buildings.contains(*building)
                                    && self.building_district_is_active(city, building)
                            })
                            .filter(|building| {
                                self.rules.buildings[building]
                                    .district
                                    .is_some_and(|district| {
                                        self.district_is_family(district, crate::name!("holy_site"))
                                    })
                            })
                            .count();
                        rys.gold += religious_community * (1 + holy_site_buildings) as f64;
                    }
                }
                // Reform the Coinage's Golden Age half is
                // `COMMEMORATION_ECONOMIC_GA_TRADE_ROUTE_YIELDS`: +3 Gold PER
                // SPECIALTY DISTRICT in the destination, international routes
                // only — not a flat 3 on every route.
                if !domestic && self.dedication_active(city.owner, "reform_the_coinage") {
                    rys.gold += 3.0 * self.city_specialty_district_count(dc) as f64;
                }
                if let Some(alliance) = self.alliance_with(city.owner, dc.owner) {
                    match alliance.kind.as_str() {
                        "research" => rys.science += 2.0,
                        "cultural" => rys.culture += 2.0,
                        "economic" => rys.gold += 4.0,
                        "religious" => rys.faith += 2.0,
                        _ => {}
                    }
                }
                if self.players[dc.owner].is_minor {
                    rys.gold += self.policy_effect(city.owner, "city_state_route_gold");
                    rys.gold += self.players[city.owner]
                        .counters
                        .get("emergency_city_state_route_gold")
                        .copied()
                        .unwrap_or(0) as f64;
                }
                if domestic {
                    rys.gold += self.city_building_effect(city, "domestic_route_gold");
                } else {
                    rys.production +=
                        self.city_building_effect(city, "international_route_production");
                }
                if dc
                    .wonders
                    .contains_key(&crate::name!("university_of_sankore"))
                {
                    if domestic {
                        rys.faith += self.rules.wonders["university_of_sankore"]
                            .effects
                            .get("domestic_route_city_faith")
                            .copied()
                            .unwrap_or(0.0);
                    } else {
                        rys.science += self.rules.wonders["university_of_sankore"]
                            .effects
                            .get("incoming_foreign_route_science")
                            .copied()
                            .unwrap_or(0.0);
                        rys.gold += self.rules.wonders["university_of_sankore"]
                            .effects
                            .get("incoming_foreign_route_gold")
                            .copied()
                            .unwrap_or(0.0);
                    }
                }
                if city.wonders.contains_key(&crate::name!("great_zimbabwe")) {
                    let spec = &self.rules.wonders["great_zimbabwe"];
                    let range = spec
                        .effects
                        .get("origin_route_bonus_resource_range")
                        .copied()
                        .unwrap_or(0.0) as i32;
                    let per_resource = spec
                        .effects
                        .get("origin_route_bonus_resource_gold")
                        .copied()
                        .unwrap_or(0.0);
                    rys.gold +=
                        per_resource
                            * city
                                .owned_tiles
                                .iter()
                                .filter(|position| self.wdist(city.pos, **position) <= range)
                                .filter(|position| {
                                    self.map.tiles[position].resource.as_ref().is_some_and(
                                        |resource| self.rules.resources[resource].class == "bonus",
                                    )
                                })
                                .count() as f64;
                }
                if !domestic && city.wonders.contains_key(&crate::name!("torre_de_belem")) {
                    let per_resource = self.rules.wonders["torre_de_belem"]
                        .effects
                        .get("origin_international_luxury_gold")
                        .copied()
                        .unwrap_or(0.0);
                    rys.gold +=
                        per_resource
                            * dc.owned_tiles
                                .iter()
                                .filter(|position| {
                                    self.map.tiles[position].resource.as_ref().is_some_and(
                                        |resource| self.rules.resources[resource].class == "luxury",
                                    )
                                })
                                .count() as f64;
                }
                let government = self.gov_effects(city.owner);
                rys.food += government.trade_food;
                rys.production += government.trade_production;
                if self.government_trade_partner(city.owner, dc.owner) {
                    rys.food += government.allied_suzerain_trade_food;
                    rys.production += government.allied_suzerain_trade_production;
                    // Wisselbanken is Democracy's policy twin at half rate: it
                    // ships the same eight rows, ORIGIN and DESTINATION halves
                    // of _FOR_ALLY_ROUTE and _FOR_SUZERAIN_ROUTE. This is the
                    // origin half, paid to the city sending the route.
                    rys.food += self.policy_effect(city.owner, "allied_suzerain_trade_food");
                    rys.production +=
                        self.policy_effect(city.owner, "allied_suzerain_trade_production");
                }
                // Where the host has said what THIS route pays its origin
                // (`observed_route_yields`, from `CalculateOriginYields…` the
                // way the shipped Trade Overview sums them), that figure stands
                // in for the model's — the destination's districts may sit on
                // ground the seat has never seen (Ostia's route to Stockholm
                // read "+1 Science" from a Campus on an unrevealed plot, run
                // civvis-20260816T233226Z t177+).
                if let Some(observed) = self.observed_route_yields.get(&(cid, r.dest)) {
                    rys = *observed;
                }
                ys.add(rys);
            }
        }
        let incoming_routes = self.routes.iter().filter(|route| route.dest == cid).count() as f64;
        let incoming_foreign_routes = self
            .routes
            .iter()
            .filter(|route| route.dest == cid && route.owner != city.owner)
            .count() as f64;
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
        ys.gold += (self.governor_effect(city.owner, city.id, "gold_per_citizen")
            + self.governor_effect(city.owner, city.id, "gold_per_pop"))
            * city.pop as f64;
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

    /// The Palace occupies the original capital while it is controlled;
    /// after that city is captured it moves to another owned city. City-states
    /// likewise have a Palace even though their city is not an original
    /// capital for Domination Victory purposes.
    pub(crate) fn city_has_palace(&self, city: &City) -> bool {
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
        if !natural_wonder {
            if let Some(excavation) = self.valid_excavation(pid, pos) {
                return vec![Name::new(&excavation)];
            }
        }
        let oc = match t.owner_city {
            Some(oc) => oc,
            None => return vec![],
        };
        let territory_owner = self.cities[&oc].owner;
        let visible_resource = t
            .resource
            .as_deref()
            .filter(|resource| self.resource_visible_to(pid, resource));
        let water = self.rules.is_water(t);
        let mut out = Vec::new();
        for (name, spec) in &self.rules.improvements {
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
            if matches!(name.as_str(), "fishery" | "city_park")
                && self.governor_effect(pid, oc, name) <= 0.0
            {
                continue;
            }
            if matches!(name.as_str(), "archaeological_dig" | "shipwreck_excavation")
                && !self.can_house_additional_great_work(pid, "artifact")
            {
                continue;
            }
            let seaside_volcanic =
                name == "seaside_resort" && t.feature.as_deref() == Some("volcanic_soil");
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
                && self.cities[&oc].owned_tiles.iter().any(|position| {
                    self.map.tiles[position].improvement.as_deref() == Some(name.as_str())
                });
            let territory_invalid = if spec.requires_foreign_territory {
                territory_owner == pid
                    || self.is_at_war(pid, territory_owner)
                    || (spec.requires_open_borders && !self.has_open_borders(pid, territory_owner))
            } else {
                !self.builder_may_improve_territory(pid, territory_owner)
            };
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
                || !self.unlocked(pid, &spec.tech, &spec.civic)
                || spec.unique_to.as_deref().is_some_and(|owner| {
                    !self.owns_civ_unique(pid, owner)
                        && !self.grants_city_state_unique_bonus(pid, owner)
                })
                || water != spec.water
                || t.improvement.as_deref() == Some(name)
                || (spec.requires_hills && !t.hills)
                || (spec.hills_or_resource && !t.hills && visible_resource.is_none())
                || (spec.hills_or_resource_or_feature
                    && !t.hills
                    && visible_resource.is_none()
                    && !feature_route)
                || (spec.hills_or_feature && !t.hills && !feature_route)
                || (spec.requires_flat
                    && t.hills
                    && !seaside_volcanic
                    && !(name == "farm" && self.tree_effect(pid, "hill_farms") > 0.0))
                || (spec.removes_feature
                    && !feature_route
                    && t.feature
                        .as_deref()
                        .is_some_and(|feature| !self.feature_removal_unlocked(pid, feature)))
                || incompatible_feature
                || (!sited && !seaside_volcanic)
                || seaside_invalid
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
                None if t.resource.is_some() => continue, // unrevealed resource
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
            || tile.improvement.is_some()
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

    pub(super) fn canonical_river_edge(a: Pos, b: Pos) -> (Pos, Pos) {
        if a <= b {
            (a, b)
        } else {
            (b, a)
        }
    }

    pub(super) fn river_edges_at(&self, position: Pos) -> BTreeSet<(Pos, Pos)> {
        self.nbrs(position)
            .into_iter()
            .filter(|neighbor| self.map.has_river_edge(position, *neighbor))
            .map(|neighbor| Self::canonical_river_edge(position, neighbor))
            .collect()
    }

    /// River segments meet at the vertices of their shared hex edges. The
    /// generated map stores only edge masks, so recovering this connected
    /// component is the save-compatible equivalent of Civ VI's named river
    /// identity for one-Dam-per-river placement.
    pub(super) fn river_component(&self, origin: (Pos, Pos)) -> BTreeSet<(Pos, Pos)> {
        if !self.map.has_river_edge(origin.0, origin.1) {
            return BTreeSet::new();
        }
        let mut component = BTreeSet::from([origin]);
        let mut frontier = VecDeque::from([origin]);
        while let Some((a, b)) = frontier.pop_front() {
            let b_neighbors: BTreeSet<Pos> = self.nbrs(b).into_iter().collect();
            for common in self
                .nbrs(a)
                .into_iter()
                .filter(|position| *position != b && b_neighbors.contains(position))
            {
                for next in [
                    Self::canonical_river_edge(a, common),
                    Self::canonical_river_edge(b, common),
                ] {
                    if self.map.has_river_edge(next.0, next.1) && component.insert(next) {
                        frontier.push_back(next);
                    }
                }
            }
        }
        component
    }

    pub(super) fn river_component_has_dam(&self, component: &BTreeSet<(Pos, Pos)>) -> bool {
        self.map.tiles.iter().any(|(position, tile)| {
            let is_dam = tile
                .district
                .is_some_and(|district| self.district_is_family(district, crate::name!("dam")))
                || tile.district_foundation.as_ref().is_some_and(|foundation| {
                    self.district_is_family(foundation.district, crate::name!("dam"))
                });
            is_dam
                && self
                    .river_edges_at(*position)
                    .iter()
                    .any(|edge| component.contains(edge))
        })
    }

    /// A Dam needs two edges belonging to the same river, and that river is
    /// reserved as soon as another Dam foundation is placed anywhere on it.
    pub(super) fn dam_has_available_river(&self, position: Pos) -> bool {
        let incident = self.river_edges_at(position);
        let mut examined = BTreeSet::new();
        for origin in incident.iter().copied() {
            if examined.contains(&origin) {
                continue;
            }
            let component = self.river_component(origin);
            examined.extend(component.iter().copied());
            if incident
                .iter()
                .filter(|edge| component.contains(edge))
                .count()
                >= 2
                && !self.river_component_has_dam(&component)
            {
                return true;
            }
        }
        false
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
            let capacity = 1 + (city.pop.max(1) - 1) as usize / 3;
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
            // Antiquity Sites and Shipwrecks are buried, not deposits: they
            // are invisible until Natural History or Cultural Heritage and
            // never reserve a tile. Building over one destroys it, exactly as
            // a Bonus resource is destroyed.
            if t.resource.as_ref().is_some_and(|resource| {
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
            if let Some(resource) = &t.resource {
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
                    !matches!(
                        self.rules.resources[resource].class.as_str(),
                        "bonus" | "artifact"
                    )
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
                            && self.cities[&candidate].is_capital
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

    pub fn item_cost(&self, item: &Item) -> f64 {
        self.game_speed.scale(self.base_item_cost(item))
    }

    pub(super) fn base_item_cost(&self, item: &Item) -> f64 {
        match item {
            Item::Formation { unit, formation } => {
                // UNIT_CORPS_COST_MODIFIER 1.5 and UNIT_ARMY_COST_MODIFIER 2.0.
                // Both are read against the base unit's cost, so an Army is 2x
                // rather than the 1.5 x 1.5 an extrapolated Corps rate gives.
                // The resource cost is separate and really does step 2 then 3,
                // one per unit folded in -- see unit_resource_cost.
                self.rules.units[unit].cost * if *formation >= 2 { 2.0 } else { 1.5 }
            }
            Item::Unit { unit } => self.rules.units[unit].cost,
            Item::Building { building } => self.rules.buildings[building].cost,
            Item::District { district, .. } => self.rules.districts[district].cost,
            Item::Wonder { wonder, .. } => self.rules.wonders[wonder].cost,
            Item::Repair { repair, pos } => {
                if repair == "district" {
                    self.map
                        .get(*pos)
                        .and_then(|tile| tile.district.as_ref())
                        .and_then(|district| self.rules.districts.get(district))
                        .map(|spec| spec.cost * 0.25)
                        .unwrap_or(1.0)
                } else {
                    self.rules
                        .buildings
                        .get(repair)
                        .map(|spec| spec.cost * 0.25)
                        .unwrap_or(1.0)
                }
            }
            Item::Project { project } => self.rules.projects[project].cost,
            Item::Product { .. } => 500.0,
        }
    }

    pub fn item_cost_for(&self, pid: usize, item: &Item) -> f64 {
        let standard = self.base_item_cost(item);
        match item {
            Item::Unit { unit } if unit == "settler" => self.game_speed.scale(
                standard
                    + 30.0
                        * self.players[pid]
                            .counters
                            .get("trained:settler")
                            .copied()
                            .unwrap_or(0) as f64,
            ),
            Item::Unit { unit } if unit == "builder" => self.game_speed.scale(
                standard
                    + 4.0
                        * self.players[pid]
                            .counters
                            .get("trained:builder")
                            .copied()
                            .unwrap_or(0) as f64,
            ),
            Item::Project { project } => {
                let maximum = self.rules.projects[project].cost_progression_max_pct;
                if maximum <= 0.0 {
                    self.game_speed.scale(standard)
                } else {
                    // GAME_PROGRESS linearly interpolates from the base to
                    // Param1 percent of base; 1500 therefore caps at 15x.
                    self.game_speed
                        .scale(
                            standard
                                * (1.0 + (maximum / 100.0 - 1.0) * self.game_progress_ratio(pid)),
                        )
                        .floor()
                }
            }
            _ => self.item_cost(item),
        }
    }

    /// Return the treasury cost of purchasing an ordinary city building.
    /// Gold follows Civ VI's four-times-Production rate; the narrower Faith
    /// purchase paths use the existing two-times rate. District purchase
    /// discounts apply to either currency.
    pub fn building_purchase_cost(
        &self,
        pid: usize,
        cid: u32,
        building: &str,
        currency: &str,
    ) -> Option<f64> {
        let city = self.cities.get(&cid).filter(|city| city.owner == pid)?;
        if city
            .queue
            .iter()
            .any(|item| matches!(item, Item::Building { building: queued } if queued == building))
        {
            return None;
        }
        if currency == "gold" {
            return self.building_gold_purchase_cost(pid, cid, building);
        }
        let spec = self.rules.buildings.get(building)?;
        if currency != "faith" {
            return None;
        }
        // ★ The host's own answer first — see `building_gold_purchase_cost`.
        let asked = Item::Building {
            building: Name::new(building),
        };
        if let Some(host) = self.host_purchase_price(cid, &asked, "faith") {
            return host.filter(|_| !self.purchase_is_blocked(cid, &asked));
        }
        let discount = self.city_district_effect(city, "gold_faith_purchase_discount_pct");
        Some(spec.cost * 2.0 * (1.0 - discount / 100.0).max(0.0))
    }

    pub(super) fn project_has_active_district(
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

    pub(super) fn district_type_available(&self, pid: usize, district: &str) -> bool {
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
    pub(super) fn district_underbuilt_discount(
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

    pub(super) fn district_cost_for_placement(
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

    /// City-sensitive production cost. Flood Barriers scale with the number
    /// of Coastal Lowland tiles they must protect; all other items retain the
    /// civilization-wide cost rules above.
    pub fn item_cost_for_city(&self, pid: usize, cid: u32, item: &Item) -> f64 {
        // ★ The host's own cost first, when its menu carries this item for
        // this city (`StateCity::buildable`): `GetXCost(row.Index)` after
        // every modifier the engine applies. A founded district keeps the cost
        // it was founded at, exactly as below.
        if !self.host_buildable.is_empty() {
            let founded = matches!(item, Item::District { district, pos }
                if self
                    .map
                    .get(*pos)
                    .and_then(|tile| tile.district_foundation.as_ref())
                    .is_some_and(|foundation| foundation.district == *district));
            if !founded {
                if let Some(cost) = self
                    .host_buildable
                    .get(&cid)
                    .and_then(|menu| menu.get(&Self::production_block_key(item)))
                    .and_then(|entry| entry.cost)
                    .filter(|cost| *cost > 0.0)
                {
                    return cost;
                }
            }
        }
        if let Item::District { district, pos } = item {
            if let Some(foundation) = self
                .map
                .get(*pos)
                .and_then(|tile| tile.district_foundation.as_ref())
            {
                if foundation.district == *district {
                    return foundation.cost;
                }
            }
            return self
                .game_speed
                .scale(self.district_cost_for_placement(pid, district, false));
        }
        if let Item::Building { building } = item {
            let spec = &self.rules.buildings[building];
            let district = spec
                .district
                .map(|district| self.district_family(district))
                .unwrap_or(crate::name!("city_center"));
            if self.congress_effect_active("urban_development_treaty", "B", district.as_str())
                || self.congress_effect_active("global_energy_treaty", "B", building)
            {
                return f64::INFINITY;
            }
            let per_lowland = spec
                .effects
                .get("cost_per_coastal_lowland")
                .copied()
                .unwrap_or(0.0);
            if per_lowland > 0.0 {
                let count = self.coastal_lowland_tiles(&self.cities[&cid]).len().max(1);
                let flood_level = (self.climate_phase / 2) as f64;
                return self
                    .game_speed
                    .scale(per_lowland * count as f64 * (1.0 + flood_level));
            }
        }
        let mut cost = self.item_cost_for(pid, item);
        let military = match item {
            Item::Unit { unit } | Item::Formation { unit, .. } => {
                self.rules.units[unit].class == "military"
            }
            _ => false,
        };
        if military {
            if self.congress_effect_active("mercenary_companies", "A", "production") {
                cost *= 2.0;
            } else if self.congress_effect_active("mercenary_companies", "B", "production") {
                cost *= 0.5;
            }
        }
        if let Item::Building { building } = item {
            if self.congress_effect_active("global_energy_treaty", "A", building) {
                cost *= 0.5;
            }
        }
        cost
    }

    pub(super) fn converted_power_plant(project: &str) -> Option<&'static str> {
        match project {
            "convert_reactor_to_coal" => Some("coal_power_plant"),
            "convert_reactor_to_oil" => Some("oil_power_plant"),
            "convert_reactor_to_uranium" => Some("nuclear_power_plant"),
            _ => None,
        }
    }

    pub(super) fn item_progress_key(item: &Item) -> String {
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

    /// Production still required after active progress, item-specific paused
    /// progress, and unassigned overflow are applied. Search agents use this
    /// instead of treating a nearly complete build like a fresh one.
    pub(crate) fn item_remaining_cost_for_city(&self, pid: usize, cid: u32, item: &Item) -> f64 {
        let city = &self.cities[&cid];
        let key = Self::item_progress_key(item);
        let mut invested = city.production_progress.get(&key).copied().unwrap_or(0.0);
        if city.queue.is_empty() || city.queue.first() == Some(item) {
            invested += city.production;
        }
        (self.item_cost_for_city(pid, cid, item) - invested).max(0.0)
    }

    pub(super) fn unit_resource_cost(&self, cid: u32, item: &Item) -> f64 {
        let (unit, multiplier) = match item {
            Item::Unit { unit } => (unit, 1.0),
            Item::Formation {
                unit, formation, ..
            } => (unit, if *formation >= 2 { 3.0 } else { 2.0 }),
            _ => return 0.0,
        };
        let Some(spec) = self.rules.units.get(unit) else {
            return 0.0;
        };
        let city = &self.cities[&cid];
        let discount = self
            .governor_effect(city.owner, cid, "strategic_resource_discount_pct")
            .clamp(0.0, 100.0);
        spec.resource_cost * multiplier * (100.0 - discount) / 100.0
    }

    /// Drop queued units the owner has just retired. Without this a city that
    /// started a Slinger keeps finishing it long after Machinery, so the very
    /// technology that should have modernized the army delivers one more copy
    /// of the unit it replaced.
    pub(super) fn drop_obsolete_production(&mut self, pid: usize) {
        for cid in self.player_city_ids(pid) {
            let doomed: Vec<Item> = self.cities[&cid]
                .queue
                .iter()
                .filter(|item| match item {
                    Item::Unit { unit } | Item::Formation { unit, .. } => {
                        self.unit_is_obsolete(pid, unit)
                    }
                    _ => false,
                })
                .cloned()
                .collect();
            if doomed.is_empty() {
                continue;
            }
            // Strategic material already committed to the abandoned order goes
            // back to the stockpile rather than evaporating with the item.
            for item in &doomed {
                let key = Self::item_progress_key(item);
                let Some(unit) = (match item {
                    Item::Unit { unit } | Item::Formation { unit, .. } => Some(*unit),
                    _ => None,
                }) else {
                    continue;
                };
                let refund = self.cities[&cid]
                    .strategic_resource_commitments
                    .get(&key)
                    .copied()
                    .unwrap_or(0.0);
                let city = self.cities.get_mut(&cid).unwrap();
                city.strategic_resource_commitments.remove(&key);
                city.production_progress.remove(&key);
                if refund > 0.0 {
                    if let Some(resource) = self.rules.units[unit].requires_resource {
                        let held = self.strategic_stockpile(pid, resource);
                        let capacity = self.strategic_stockpile_capacity(pid);
                        self.players[pid]
                            .strategic_resources
                            .insert(Name::new(&resource), (held + refund).min(capacity));
                    }
                }
            }
            let city = self.cities.get_mut(&cid).unwrap();
            let head_dropped = city.queue.first().is_some_and(|item| doomed.contains(item));
            city.queue.retain(|item| !doomed.contains(item));
            if head_dropped {
                city.production = 0.0;
            }
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
    /// Unit production rules without the recursive obsolescence check. The
    /// resource credit is used only while automatically migrating an existing
    /// queue whose already-committed material can be refunded atomically.
    pub(super) fn can_produce_unit(
        &self,
        pid: usize,
        cid: u32,
        unit: impl AsName,
        check_obsolete: bool,
        resource_credit: f64,
    ) -> bool {
        let unit = unit.as_name();
        let Some(city) = self.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        let Some(spec) = self.rules.units.get_interned(unit) else {
            return false;
        };
        if matches!(unit.as_str(), "rock_band" | "naturalist") {
            return false; // Faith purchase only (Civ VI)
        }
        // A city-state is permanently one city. This is an engine rule rather
        // than an AI preference so captured, granted, and externally commanded
        // minor Settlers cannot turn it into an ordinary empire.
        if unit == "settler"
            && (self.players[pid].is_minor || self.policy_effect(pid, "no_settling") > 0.0)
        {
            return false;
        }
        if !spec.buildable || !self.unlocked(pid, &spec.tech, &spec.civic) {
            return false;
        }
        if check_obsolete && self.unit_is_obsolete(pid, unit) {
            return false;
        }
        if unit == "spy" {
            // Counts mirrored `UNIT_SPY` units too — see `spy_agents`.
            let existing = self.spy_agents(pid);
            let queued = self
                .cities
                .values()
                .filter(|candidate| candidate.owner == pid)
                .filter(|candidate| {
                    candidate
                        .queue
                        .iter()
                        .any(|queued| matches!(queued, Item::Unit { unit } if unit == "spy"))
                })
                .count();
            if existing + queued >= self.spy_capacity(pid).max(0) as usize {
                return false;
            }
        }
        if unit == "settler" && city.pop < 2 {
            return false;
        }
        if spec.class == "religious" {
            return false; // faith purchase only (Civ VI)
        }
        if spec
            .unique_to
            .as_deref()
            .is_some_and(|civilization| !self.owns_civ_unique(pid, civilization))
        {
            return false;
        }
        // A civilization with a unique replacement cannot train the base.
        if self.player_unit_replacement(pid, unit) != unit {
            return false;
        }
        let item = Item::Unit { unit };
        if let Some(resource) = &spec.requires_resource {
            let resource_cost = self.unit_resource_cost(cid, &item);
            if resource_cost > 0.0
                && !self.unit_resource_is_committed(cid, &item)
                && self.strategic_stockpile(pid, resource) + resource_credit + f64::EPSILON
                    < resource_cost
            {
                return false;
            }
        }
        if spec
            .requires_building
            .as_ref()
            .is_some_and(|building| !self.city_has_building_family(city, Name::new(building)))
            || spec
                .requires_district
                .as_ref()
                .is_some_and(|district| !self.city_has_district_family(city, Name::new(district)))
        {
            return false;
        }
        if spec.domain.as_deref() == Some("sea") {
            let coastal = self.nbrs(city.pos).iter().any(|position| {
                self.map
                    .get(*position)
                    .is_some_and(|tile| self.rules.is_water(tile))
            });
            if !coastal {
                return false;
            }
        }
        true
    }

    /// Non-allocating twin of [`Game::production_block_key`], for callers
    /// that only ever compare or hash the key — never look it up in a
    /// `String`-keyed map fed by the live mirror (`blocked_production`,
    /// `blocked_purchases`, `host_buildable`, `host_purchasable`; those stay
    /// `String`-keyed because they cross the bridge's serde boundary).
    /// `Name` is already an interned, `Copy` id, so building this key touches
    /// no allocator.
    pub(crate) fn production_key(item: &Item) -> ProductionKey {
        match item {
            Item::Formation { unit, formation } => ProductionKey::Formation(*unit, *formation),
            Item::Unit { unit } => ProductionKey::Unit(*unit),
            Item::Building { building } => ProductionKey::Building(*building),
            Item::District { district, .. } => ProductionKey::District(*district),
            Item::Wonder { wonder, .. } => ProductionKey::Wonder(*wonder),
            Item::Repair { repair, .. } => ProductionKey::Repair(*repair),
            Item::Project { project } => ProductionKey::Project(*project),
            Item::Product { product } => ProductionKey::Product(*product),
        }
    }

    pub(crate) fn production_block_key(item: &Item) -> String {
        Self::production_key(item).to_block_string()
    }

    pub(crate) fn replace_blocked_production(&mut self, blocked: BTreeMap<u32, BTreeSet<String>>) {
        self.blocked_production = Arc::new(blocked);
        // This field is mirror input rather than an Action, so it does not cross the
        // ordinary successful-apply invalidation below. A menu derived before sync
        // must not survive after the host has rejected one of its entries.
        self.query_memo.producible.borrow_mut().clear();
    }

    /// Replace the current live-host competition snapshot. The production
    /// catalog persists beyond an ordinary read-only memo scope, so a host
    /// competition starting or ending must explicitly retire any menu cached
    /// before the new snapshot arrived.
    pub(crate) fn replace_host_competitions(&mut self, competitions: Vec<HostCompetition>) {
        if *self.host_competitions != competitions {
            self.host_competitions = Arc::new(competitions);
            self.query_memo.producible.borrow_mut().clear();
        }
    }

    /// The current host competition of this mirrored seat, if it is still
    /// active. Host player ids do not match generated CIVVIS seats, so the
    /// mirror intentionally supplies opportunities for seat zero only.
    pub fn host_competition(&self, pid: usize, kind: &str) -> Option<HostCompetition> {
        // A competition CIVVIS is running itself answers for every seat, and
        // presents in the same shape a mirrored one does — so the production
        // catalog and the AI's valuation cannot tell them apart and neither
        // needed changing.
        if let Some(native) = self
            .competition
            .as_ref()
            .filter(|running| running.kind == kind && running.ends > self.turn)
        {
            if native.target == Some(pid) {
                return None;
            }
            let ours = native.scores.get(&pid).copied().unwrap_or(0.0);
            let leader = native.scores.values().copied().fold(0.0, f64::max);
            return Some(HostCompetition {
                kind: native.kind.clone(),
                ends: native.ends,
                ours,
                leader,
            });
        }
        (pid == 0)
            .then(|| {
                self.host_competitions
                    .iter()
                    .find(|competition| competition.kind == kind && competition.ends > self.turn)
                    .cloned()
            })
            .flatten()
    }

    /// The competitions CIVVIS can seat itself, with the Diplomatic Victory
    /// Points Gathering Storm pays their winner.
    ///
    /// Every field is read off the installed Gathering Storm ruleset —
    /// `EmergencyAlliances` for the trigger, `Duration` and `LockoutTime`, that
    /// row's `TargetRequirementSet` for the era window, `EmergencyScoreSources`
    /// for what counts, and `EmergencyRewards` joined to `ModifierArguments`
    /// for the award:
    ///
    /// | competition | era | scores | first place |
    /// |---|---|---|---|
    /// | World's Fair | Modern only | Great Person Points per turn | 1 point, 50 Favor |
    /// | World Games | Atomic+ | athletes project, Stadiums and Aquatics Centers per turn | 1 point, 50 Favor |
    /// | Climate Accords | Information+ | the three decommissioning projects | 2 points, 100 Favor |
    /// | International Space Station | Future+ | astronauts project, Spaceports and Campuses per turn | 1 point, 50 Favor |
    /// | Nobel Peace Prize | Industrial+, Sweden in the game | Diplomatic Favor per turn | 1 point |
    /// | Send Aid | any | the aid project | 2 points, 100 Favor |
    /// | Send Military Aid | any | the aid project | 2 points, 100 Favor |
    ///
    /// ⚠ **The Nobel prizes for Literature and Physics are deliberately absent
    /// and are not a gap.** They are scored competitions like the rest, but
    /// `EmergencyRewards` gives neither of them a
    /// `NON_EMERGENCY_FIRST_PLACE_VICTORY_POINT` row — Literature's first place
    /// takes cheaper Rock Bands and Physics' a technology boost — so neither is
    /// a source of a Diplomatic Victory Point at all. Only Peace is.
    ///
    /// ⚠ **Order is the seating preference**, because the shipped data does not
    /// say how the congress picks among the competitions it could offer: that
    /// choice lives in the compiled engine, not in `Emergencies_XP2`. The list
    /// is newest-era-first, so the latest competition an era has unlocked takes
    /// the seat and the Nobel Peace Prize — the only one available before the
    /// Modern era — takes it in the Industrial era and in the gaps another
    /// competition's per-kind lockout leaves.
    pub(super) const NATIVE_COMPETITIONS: &'static [NativeCompetitionSpec] = &[
        NativeCompetitionSpec {
            kind: "EMERGENCY_SPACE_STATION",
            diplomatic_victory_points: 1,
            first_place_favor: 50.0,
            scoring: &[
                CompetitionScoreSource::Project,
                CompetitionScoreSource::DistrictPerTurn {
                    district: "spaceport",
                    amount: 5.0,
                },
                CompetitionScoreSource::DistrictPerTurn {
                    district: "campus",
                    amount: 1.0,
                },
            ],
            trigger: NativeCompetitionTrigger::Congress,
            duration: 29,
            lockout: 60,
            minimum_world_era: 8,
            maximum_world_era: None,
            required_civilization_ability: None,
        },
        NativeCompetitionSpec {
            kind: "EMERGENCY_CLIMATE_ACCORDS",
            diplomatic_victory_points: 2,
            first_place_favor: 100.0,
            scoring: &[CompetitionScoreSource::Project],
            trigger: NativeCompetitionTrigger::Congress,
            duration: 29,
            lockout: 60,
            minimum_world_era: 7,
            maximum_world_era: None,
            required_civilization_ability: None,
        },
        NativeCompetitionSpec {
            kind: "EMERGENCY_WORLD_GAMES",
            diplomatic_victory_points: 1,
            first_place_favor: 50.0,
            scoring: &[
                CompetitionScoreSource::Project,
                CompetitionScoreSource::BuildingPerTurn {
                    building: "stadium",
                    amount: 1.0,
                },
                CompetitionScoreSource::BuildingPerTurn {
                    building: "aquatics_center",
                    amount: 1.0,
                },
            ],
            trigger: NativeCompetitionTrigger::Congress,
            duration: 29,
            lockout: 60,
            minimum_world_era: 6,
            maximum_world_era: None,
            required_civilization_ability: None,
        },
        NativeCompetitionSpec {
            kind: "EMERGENCY_WORLDS_FAIR",
            diplomatic_victory_points: 1,
            first_place_favor: 50.0,
            scoring: &[CompetitionScoreSource::GreatPersonPointsPerTurn],
            trigger: NativeCompetitionTrigger::Congress,
            duration: 29,
            lockout: 60,
            minimum_world_era: 5,
            maximum_world_era: Some(5),
            required_civilization_ability: None,
        },
        // Sweden's `TRAIT_CIVILIZATION_NOBEL_PRIZE` is the whole reason the
        // Nobel prizes exist in a game: `NOBEL_PRIZE_TARGET_REQUIREMENTS` tests
        // `REQUIREMENT_GAME_HAS_CIVILIZATION_OR_LEADER_TRAIT` for it. Without
        // Sweden on the board the congress never offers one, which is why this
        // competition is rare rather than a route every empire has.
        NativeCompetitionSpec {
            kind: "EMERGENCY_NOBEL_PRIZE_PEACE",
            diplomatic_victory_points: 1,
            first_place_favor: 0.0,
            scoring: &[CompetitionScoreSource::DiplomaticFavorPerTurn],
            trigger: NativeCompetitionTrigger::Congress,
            duration: 29,
            lockout: 60,
            minimum_world_era: 4,
            maximum_world_era: None,
            required_civilization_ability: Some("nobelinstitution"),
        },
        NativeCompetitionSpec {
            kind: "EMERGENCY_SEND_AID",
            diplomatic_victory_points: 2,
            first_place_favor: 100.0,
            scoring: &[CompetitionScoreSource::Project],
            trigger: NativeCompetitionTrigger::RandomDisasterPopulationLoss,
            duration: 30,
            lockout: 30,
            minimum_world_era: 0,
            maximum_world_era: None,
            required_civilization_ability: None,
        },
        NativeCompetitionSpec {
            kind: "EMERGENCY_SEND_MILITARY_AID",
            diplomatic_victory_points: 2,
            first_place_favor: 100.0,
            scoring: &[CompetitionScoreSource::Project],
            trigger: NativeCompetitionTrigger::WarWithGrievances,
            duration: 30,
            lockout: 30,
            minimum_world_era: 0,
            maximum_world_era: None,
            required_civilization_ability: None,
        },
    ];

    pub(super) fn native_competition(kind: &str) -> Option<&'static NativeCompetitionSpec> {
        Self::NATIVE_COMPETITIONS
            .iter()
            .find(|competition| competition.kind == kind)
    }

    /// The Diplomatic Victory Points this competition's first place pays, or
    /// zero for a competition CIVVIS does not seat itself.
    ///
    /// The table above is the authority on what a competition is worth to the
    /// diplomatic race; a planner that wants to price those points must not
    /// carry its own copy of it. See `AdvancedAi::competition_victory_point_value`.
    pub fn competition_victory_points(kind: &str) -> i64 {
        Self::native_competition(kind).map_or(0, |spec| spec.diplomatic_victory_points)
    }

    /// Seat a competition if one may run and an empire could score in it.
    pub(super) fn open_native_competition(&mut self) {
        if !self.native_competitions || self.competition.is_some() {
            return;
        }
        let majors: Vec<usize> = self
            .players
            .iter()
            .filter(|player| self.victory_eligible(player.id))
            .map(|player| player.id)
            .collect();
        let Some(competition) = Self::NATIVE_COMPETITIONS.iter().find(|competition| {
            competition.trigger == NativeCompetitionTrigger::Congress
                && competition.offered_in_world_era(self.world_era)
                && self.game_meets_competition_trait(competition)
                && self
                    .competition_lockout_until
                    .get(competition.kind)
                    .is_none_or(|until| self.turn >= *until)
                && majors
                    .iter()
                    .any(|pid| self.can_score_competition(*pid, competition.kind))
        }) else {
            return;
        };
        self.competition = Some(Competition {
            kind: competition.kind.to_string(),
            ends: self.turn + self.standard_duration(competition.duration),
            target: None,
            scores: BTreeMap::new(),
        });
        self.query_memo.producible.borrow_mut().clear();
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

    /// Whether the game contains the civilization a competition's emergency
    /// requires before it exists at all.
    ///
    /// `NOBEL_PRIZE_TARGET_REQUIREMENTS` tests
    /// `REQUIREMENT_GAME_HAS_CIVILIZATION_OR_LEADER_TRAIT` for
    /// `TRAIT_CIVILIZATION_NOBEL_PRIZE`, which `CivilizationTraits` gives to
    /// `CIVILIZATION_SWEDEN` alone. A game with no Sweden in it never sees a
    /// Nobel prize, and that is the shipped rule rather than a simplification.
    pub(super) fn game_meets_competition_trait(&self, competition: &NativeCompetitionSpec) -> bool {
        let Some(ability) = competition.required_civilization_ability else {
            return true;
        };
        self.players
            .iter()
            .any(|player| self.has_ability(player.id, ability))
    }

    /// Whether this empire could score at all in a competition, after its exact
    /// era gate has selected it.
    ///
    /// Every source the competition declares is asked, because a competition
    /// nobody can score in pays nobody and still spends its lockout.
    pub(super) fn can_score_competition(&self, pid: usize, kind: &str) -> bool {
        let Some(competition) = Self::native_competition(kind) else {
            return false;
        };
        competition.scoring.iter().any(|source| match source {
            // Every empire generates Great Person Points and Diplomatic Favor,
            // so there is no ground to hold and nothing to gate on.
            CompetitionScoreSource::GreatPersonPointsPerTurn
            | CompetitionScoreSource::DiplomaticFavorPerTurn => true,
            CompetitionScoreSource::DistrictPerTurn { district, .. } => {
                let district = Name::new(district);
                self.cities
                    .values()
                    .any(|city| city.owner == pid && city.districts.contains_key(district))
            }
            CompetitionScoreSource::BuildingPerTurn { building, .. } => {
                let building = Name::new(building);
                self.cities
                    .values()
                    .any(|city| city.owner == pid && city.buildings.contains(&building))
            }
            CompetitionScoreSource::Project => self.rules.projects.iter().any(|(_, spec)| {
                if spec.competition_score <= 0.0
                    || !spec.host_competition_kinds().any(|k| k == kind)
                {
                    return false;
                }
                // ⚠ The district is not the whole requirement. A decommissioning
                // project also eats a power plant, and a competition offered to an
                // empire that holds none is a competition nobody can score in: the
                // first trace of this seated Climate Accords on turn 100 and closed
                // it on 119 with no score at all, having spent the lockout.
                self.cities.values().any(|city| {
                    city.owner == pid
                        && spec
                            .district
                            .is_none_or(|district| city.districts.contains_key(district))
                        && spec
                            .consumes_buildings
                            .iter()
                            .all(|building| city.buildings.contains(&Name::new(building)))
                })
            }),
        })
    }

    /// Add `amount` to this seat's score, if the running competition counts
    /// `source`.
    ///
    /// ⚠ **Nothing is paid on the mirrored path.** `competition` is set only by
    /// native seating; a mirrored competition lives in `host_competitions`, and
    /// the host has already counted its own score and paid its own award.
    pub(super) fn score_native_competition(
        &mut self,
        pid: usize,
        source: CompetitionScoreSource,
        amount: f64,
    ) {
        if amount <= 0.0 || !self.victory_eligible(pid) {
            return;
        }
        let turn = self.turn;
        let counts = self
            .competition
            .as_ref()
            .filter(|running| running.ends > turn && running.target != Some(pid))
            .and_then(|running| Self::native_competition(&running.kind))
            .is_some_and(|competition| competition.counts(source));
        if !counts {
            return;
        }
        if let Some(running) = self.competition.as_mut() {
            *running.scores.entry(pid).or_insert(0.0) += amount;
        }
    }

    /// The World's Fair's eight `WORLDS_FAIR_SCORE_GPP_*` rows: one point per
    /// Great Person Point generated this turn, whatever the class.
    ///
    /// ⚠ This counts **points**, not people. Every row is `ScoreAmount="1"`
    /// against a `FromGreatPerson` class, and their shared description is
    /// `LOC_EMERGENCY_SCORE_GPP_DESC`, "Generating Great People Points Per
    /// Turn". Counting recruits instead — which is what CIVVIS did until this
    /// change — reads the same table two orders of magnitude too small, and
    /// leaves a 29-turn competition to be decided by whether two empires each
    /// happened to claim one person, which is a tie, and a tie pays nobody.
    pub(super) fn score_great_person_point_competition(&mut self, pid: usize, points: f64) {
        self.score_native_competition(
            pid,
            CompetitionScoreSource::GreatPersonPointsPerTurn,
            points,
        );
    }

    /// `NOBEL_PRIZE_PEACE_SCORE_FROM_FAVOR`: one point per Diplomatic Favor
    /// generated this turn.
    ///
    /// The score source is described "Generating [ICON_Favor] Diplomatic
    /// Favor", the same "Generating … Per Turn" cadence the World's Fair uses,
    /// so it is this turn's favor *income* — what `process_diplomacy` computes
    /// and banks in the `diplomatic_favor` counter. It is deliberately not the
    /// balance, and deliberately not favor that merely arrives: a congress
    /// refund, a trade, or an emergency award is not favor the empire
    /// generated.
    pub(super) fn score_favor_competition(&mut self, pid: usize, favor: f64) {
        self.score_native_competition(pid, CompetitionScoreSource::DiplomaticFavorPerTurn, favor);
    }

    /// The `FromDistrict` and `FromBuilding` rows, which pay for *maintaining*
    /// what they name and therefore accrue every turn the competition runs.
    ///
    /// The International Space Station counts Spaceports at 5 and Campuses at
    /// 1; the World Games counts Stadiums and Aquatics Centers at 1. Without
    /// them a competition seated over ground nobody chooses to spend production
    /// on closes with an empty score table and pays nobody, which is exactly
    /// what the first native trace recorded.
    pub(super) fn score_competition_holdings(&mut self, pid: usize) {
        // ⚠ Majors only. `begin_turn` runs for every seat, and a city-state
        // holds Campuses like anyone else — but an emergency's members are the
        // majors, and a city-state that outscored them would take a Diplomatic
        // Victory Point off the board for nobody.
        if !self.victory_eligible(pid) {
            return;
        }
        let turn = self.turn;
        let Some(spec) = self
            .competition
            .as_ref()
            .filter(|running| running.ends > turn && running.target != Some(pid))
            .and_then(|running| Self::native_competition(&running.kind))
            .copied()
        else {
            return;
        };
        for source in spec.scoring {
            let held = match source {
                CompetitionScoreSource::DistrictPerTurn { district, amount } => {
                    let district = Name::new(district);
                    let cities = self
                        .cities
                        .values()
                        .filter(|city| city.owner == pid && city.districts.contains_key(district))
                        .count();
                    amount * cities as f64
                }
                CompetitionScoreSource::BuildingPerTurn { building, amount } => {
                    let building = Name::new(building);
                    let cities = self
                        .cities
                        .values()
                        .filter(|city| city.owner == pid && city.buildings.contains(&building))
                        .count();
                    amount * cities as f64
                }
                _ => 0.0,
            };
            if held > 0.0 {
                if let Some(running) = self.competition.as_mut() {
                    *running.scores.entry(pid).or_insert(0.0) += held;
                }
            }
        }
    }

    /// Close a finished competition, paying its winner what Gathering Storm
    /// pays: the Diplomatic Victory Point, and the Favor beside it.
    ///
    /// ⚠ First place only, and ties pay nobody — a tie has no first place, and
    /// inventing a tiebreak here would be inventing a rule. Nothing is paid on
    /// the mirrored path either: a host has already counted its own.
    pub(super) fn close_native_competition(&mut self) {
        let Some(running) = self.competition.as_ref() else {
            return;
        };
        if self.turn < running.ends {
            return;
        }
        let spec = Self::native_competition(&running.kind).copied();
        let award = spec
            .map(|competition| competition.diplomatic_victory_points)
            .unwrap_or(0);
        let kind = running.kind.clone();
        let until = self.turn
            + self.standard_duration(spec.map(|competition| competition.lockout).unwrap_or(60));
        let best = running.scores.values().copied().fold(0.0, f64::max);
        let winners: Vec<usize> = running
            .scores
            .iter()
            .filter(|(_, score)| **score >= best && best > 0.0)
            .map(|(pid, _)| *pid)
            .collect();
        let favor = spec
            .map(|competition| competition.first_place_favor)
            .unwrap_or(0.0);
        if let [winner] = winners[..] {
            self.players[winner].dvp += award;
            self.players[winner].diplomatic_favor += favor;
            self.add_historic_moment(winner, "MOMENT_PLAYER_EARNED_DIPLOMATIC_VICTORY_POINT");
        }
        self.competition = None;
        self.competition_lockout_until.insert(kind, until);
        self.query_memo.producible.borrow_mut().clear();
    }

    pub(crate) fn replace_blocked_purchases(&mut self, blocked: BTreeMap<u32, BTreeSet<String>>) {
        self.blocked_purchases = Arc::new(blocked);
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

    /// The host's own turns-to-complete for an item in this city
    /// (`BuildQueue:GetTurnsLeft`), when the export carried it.
    pub fn host_production_turns(&self, cid: u32, item: &Item) -> Option<f64> {
        self.host_buildable
            .get(&cid)?
            .get(&Self::production_block_key(item))?
            .turns
    }

    /// Whether the host menu is asked about this kind of item at all. Repairs
    /// and Corporation products are not among the exported families.
    pub(super) fn host_menu_gates(item: &Item) -> bool {
        !matches!(item, Item::Repair { .. } | Item::Product { .. })
    }

    /// The host's purchase price for an item in this city in a currency, when
    /// the export carried this city's purchase menu: `Some(None)` is the host
    /// declining to sell, `None` is no menu — ask the model.
    pub(super) fn host_purchase_price(
        &self,
        cid: u32,
        item: &Item,
        currency: &str,
    ) -> Option<Option<f64>> {
        let menu = self.host_purchasable.get(&cid)?;
        let entry = menu.get(&Self::production_block_key(item));
        Some(match currency {
            "gold" => entry.and_then(|entry| entry.gold),
            "faith" => entry.and_then(|entry| entry.faith),
            _ => None,
        })
    }

    /// Keep only the sites the host's own placement offer includes, when the
    /// export carried the complete offer for this district in this city.
    pub(super) fn host_offered_district_sites(
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

    /// Whether the host recently refused to let this city BUY this item.
    ///
    /// ⚠ `pub(crate)` because the legal-action enumeration is not the only way a
    /// purchase happens. `buy_gold_infrastructure`, `buy_gold_unit`,
    /// `buy_gold_military` and the missionary buyer all build an `Action::Buy*`
    /// themselves and call `apply` directly, so a gate that lives only in the
    /// enumeration never runs for them — which is exactly how the same Granary
    /// was re-bought in the same city on turns 114, 117, 121, 122, 123 and 128 of
    /// live run `civvis-20260804T091315Z`, 8 of that game's 9 purchases refused.
    pub(crate) fn purchase_is_blocked(&self, cid: u32, item: &Item) -> bool {
        let Some(blocked) = self.blocked_purchases.get(&cid) else {
            return false;
        };
        let key = Self::production_block_key(item);
        if blocked.contains(&key) {
            return true;
        }
        // The host purchase event names the unit type, not the Corps/Army wrapper.
        // One refusal therefore cools down every formation of that unit in this city.
        matches!(item, Item::Formation { unit, .. } if blocked.contains(&format!("unit:{unit}")))
    }

    pub(super) fn purchase_action_is_blocked(&self, action: &Action) -> bool {
        match action {
            Action::Buy {
                city,
                unit,
                formation,
                ..
            } => {
                let item = if *formation == 0 {
                    Item::Unit { unit: *unit }
                } else {
                    Item::Formation {
                        unit: *unit,
                        formation: *formation,
                    }
                };
                self.purchase_is_blocked(*city, &item)
            }
            Action::BuyBuilding { city, building, .. } => self.purchase_is_blocked(
                *city,
                &Item::Building {
                    building: *building,
                },
            ),
            Action::BuyDistrict {
                city,
                district,
                pos,
                ..
            } => self.purchase_is_blocked(
                *city,
                &Item::District {
                    district: *district,
                    pos: *pos,
                },
            ),
            _ => false,
        }
    }

    pub fn can_produce(&self, pid: usize, cid: u32, item: &Item) -> bool {
        // An arena builds things that fight and nothing else. Stated once
        // here, at the gate every queue, every AI production choice and every
        // client menu already asks, rather than as a rule each of them has to
        // remember separately.
        if self.is_arena() && !self.arena_allows_production(item) {
            return false;
        }
        // And nothing at all while the arena grants no Production to build
        // it with — the stock arena since the grants went to zero. A city
        // that will never finish anything has no business asking its player
        // to choose what it builds every turn; raise the grant and the
        // fighting-units menu is back. `arena_allows_production` stays the
        // roster rule, so a unique-units match still reads its roster
        // whatever the grant.
        if self.is_arena() && self.tactics.production == 0 {
            return false;
        }
        // Both maps a block key is looked up in are keyed by the live
        // mirror, and empty otherwise — so build the (allocating) `String`
        // form only when a map actually holds an entry for this city, not
        // unconditionally on every candidate item of every ordinary board.
        if self.blocked_production.get(&cid).is_some_and(|items| {
            !items.is_empty() && items.contains(Self::production_block_key(item).as_str())
        }) {
            return false;
        }
        let city = &self.cities[&cid];
        // ★ THE POSITIVE GATE. The set above says what the host REFUSED; this
        // one says what the host OFFERS — `BuildQueue:CanProduce(hash, false,
        // true)` for every family, exported per city (`StateCity::buildable`)
        // and translated onto the same keys. Empty on an ordinary board and on
        // an older export, so nothing changes there. The queue is exempt: the
        // head is the host's own `producing`, listed or not at its discretion.
        if Self::host_menu_gates(item) {
            if let Some(menu) = self.host_buildable.get(&cid) {
                // The queue scan below only ever compares keys, never looks
                // one up in a `String`-keyed map, so it stays on the
                // non-allocating `ProductionKey` throughout.
                let key = Self::production_key(item);
                if !menu.contains_key(key.to_block_string().as_str())
                    && !city
                        .queue
                        .iter()
                        .any(|queued| Self::production_key(queued) == key)
                {
                    return false;
                }
            }
            if let Item::District { district, pos } = item {
                if self
                    .host_district_plots
                    .get(&cid)
                    .and_then(|plots| plots.get(district))
                    .is_some_and(|plots| !plots.contains(pos))
                    && !city.queue.contains(item)
                {
                    return false;
                }
            }
        }
        match item {
            Item::Formation { unit, formation } => {
                let Some(spec) = self.rules.units.get(unit) else {
                    return false;
                };
                if self.players[pid].is_minor
                    || !matches!(formation, 1 | 2)
                    || spec.class != "military"
                    || spec.domain.as_deref() == Some("air")
                    || !spec.can_formations
                    || !self.formation_unlocked(pid, unit, *formation)
                {
                    return false;
                }
                let infrastructure = if spec.domain.as_deref() == Some("sea") {
                    self.city_has_building_family(city, crate::name!("seaport"))
                } else {
                    self.city_has_building_family(city, crate::name!("military_academy"))
                        || city.districts.iter().any(|(district, position)| {
                            district == "ikanda"
                                && self.district_is_active(city, district, *position)
                        })
                };
                if !infrastructure {
                    return false;
                }
                let committed = self.unit_resource_is_committed(cid, item);
                if !self.can_produce(pid, cid, &Item::Unit { unit: *unit })
                    && !(committed && spec.buildable && self.unlocked(pid, &spec.tech, &spec.civic))
                {
                    return false;
                }
                if let Some(resource) = spec.requires_resource.as_deref() {
                    let resource_cost = self.unit_resource_cost(cid, item);
                    if resource_cost > 0.0
                        && !committed
                        && self.strategic_stockpile(pid, Name::new(resource)) + f64::EPSILON
                            < resource_cost
                    {
                        return false;
                    }
                }
                true
            }
            Item::Unit { unit } => self.can_produce_unit(pid, cid, unit, true, 0.0),
            Item::Building { building } => {
                let spec = match self.rules.buildings.get(building) {
                    Some(s) => s,
                    None => return false,
                };
                let congress_district = spec
                    .district
                    .map(|district| self.district_family(district))
                    .unwrap_or(crate::name!("city_center"));
                if self.congress_effect_active(
                    "urban_development_treaty",
                    "B",
                    congress_district.as_str(),
                ) {
                    return false;
                }
                if self.congress_effect_active("global_energy_treaty", "B", building) {
                    return false;
                }
                let society_for = |candidate: &crate::rules::BuildingSpec| {
                    [
                        ("requires_hermetic_order", "hermetic_order"),
                        ("requires_owls_of_minerva", "owls_of_minerva"),
                        ("requires_voidsingers", "voidsingers"),
                    ]
                    .into_iter()
                    .find_map(|(effect, society)| {
                        (candidate.effects.get(effect).copied().unwrap_or(0.0) > 0.0)
                            .then_some(society)
                    })
                };
                let society_building = society_for(spec).is_some_and(|society| {
                    self.players[pid].secret_society.as_deref() == Some(society)
                });
                if city.buildings.contains(building)
                    || !self.unlocked(pid, &spec.tech, &spec.civic)
                    || (!spec.buildable && !society_building)
                    || spec.purchase_only
                {
                    return false;
                }
                if spec
                    .unique_to
                    .as_deref()
                    .is_some_and(|civ| !self.owns_civ_unique(pid, civ))
                    || self.rules.buildings.values().any(|candidate| {
                        candidate.replaces == Some(*building)
                            && (candidate
                                .unique_to
                                .as_deref()
                                .is_some_and(|owner| self.owns_civ_unique(pid, owner))
                                || society_for(candidate).is_some_and(|society| {
                                    self.players[pid].secret_society.as_deref() == Some(society)
                                }))
                    })
                    || !spec
                        .requires
                        .iter()
                        .all(|required| self.city_has_building_family(city, *required))
                    || (!spec.requires_any.is_empty()
                        && !spec
                            .requires_any
                            .iter()
                            .any(|required| self.city_has_building_family(city, *required)))
                    || spec
                        .excludes
                        .iter()
                        .any(|excluded| self.city_has_building_family(city, *excluded))
                {
                    return false;
                }
                for (effect, society) in [
                    ("requires_hermetic_order", "hermetic_order"),
                    ("requires_owls_of_minerva", "owls_of_minerva"),
                    ("requires_voidsingers", "voidsingers"),
                ] {
                    if spec.effects.get(effect).copied().unwrap_or(0.0) > 0.0
                        && self.players[pid].secret_society.as_deref() != Some(society)
                    {
                        return false;
                    }
                }
                if spec.outer_defense > 0
                    && (!spec
                        .requires
                        .iter()
                        .all(|required| self.city_has_building_family(city, *required))
                        || city.wall_hp < self.city_max_wall_hp(city))
                {
                    return false;
                }
                if spec
                    .effects
                    .get("protect_coastal_lowlands")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                    && self.coastal_lowland_tiles(city).is_empty()
                {
                    return false;
                }
                if spec.wonder && self.wonder_built(building) {
                    return false; // one per world
                }
                if spec.coastal {
                    let ok = self.nbrs(city.pos).iter().any(|n| {
                        self.map
                            .get(*n)
                            .map(|t| self.rules.is_water(t))
                            .unwrap_or(false)
                    });
                    if !ok {
                        return false;
                    }
                }
                if spec
                    .effects
                    .get("requires_river_city")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                    && !self.map.tiles[&city.pos].has_river()
                {
                    return false;
                }
                if spec
                    .effects
                    .get("requires_fresh_water_city")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                {
                    let fresh = self.map.tiles[&city.pos].has_river()
                        || self.nbrs(city.pos).iter().any(|neighbor| {
                            self.map.get(*neighbor).is_some_and(|tile| {
                                tile.terrain == "lake" || tile.feature.as_deref() == Some("oasis")
                            })
                        });
                    if !fresh {
                        return false;
                    }
                }
                match &spec.district {
                    None => true,
                    Some(d) if self.district_family(*d) == "city_center" => true,
                    Some(d) => city.districts.iter().any(|(built, position)| {
                        self.district_is_family(built, d)
                            && self.district_is_active(city, built, *position)
                            && !self.unit_ids_at(*position).iter().any(|unit| {
                                self.units[unit].owner != pid
                                    && self.is_at_war(pid, self.units[unit].owner)
                            })
                    }),
                }
            }
            Item::District { district, pos } => {
                let spec = match self.rules.districts.get(district) {
                    Some(s) => s,
                    None => return false,
                };
                if !self.unlocked(pid, &spec.tech, &spec.civic)
                    || !spec.buildable
                    || spec
                        .unique_to
                        .as_deref()
                        .is_some_and(|civ| !self.owns_civ_unique(pid, civ))
                    || self.rules.districts.values().any(|candidate| {
                        candidate.replaces == Some(*district)
                            && candidate
                                .unique_to
                                .as_deref()
                                .is_some_and(|owner| self.owns_civ_unique(pid, owner))
                    })
                {
                    return false;
                }
                self.district_sites(cid, district).contains(pos)
            }
            Item::Wonder { wonder, pos } => {
                if self.players[pid].is_minor {
                    return false;
                }
                let Some(spec) = self.rules.wonders.get(wonder) else {
                    return false;
                };
                self.unlocked(pid, &spec.tech, &spec.civic)
                    && !self.wonder_built(wonder)
                    && self.wonder_sites(cid, wonder).contains(pos)
            }
            Item::Repair { repair, pos } => {
                let Some(tile) = self.map.get(*pos) else {
                    return false;
                };
                if tile.owner_city != Some(cid)
                    || tile.district.is_none()
                    || self.unit_ids_at(*pos).iter().any(|unit| {
                        self.units[unit].owner != pid && self.is_at_war(pid, self.units[unit].owner)
                    })
                {
                    return false;
                }
                if repair == "district" {
                    tile.pillaged
                } else {
                    city.pillaged_buildings
                        .iter()
                        .any(|built| *built == *repair)
                        && self.rules.buildings.get(repair).is_some_and(|building| {
                            building.district.as_ref().is_some_and(|family| {
                                tile.district.is_some_and(|district| {
                                    self.district_is_family(district, family)
                                })
                            })
                        })
                }
            }
            Item::Project { project } => {
                if project == "repair_outer_defenses" {
                    let max = self.city_max_wall_hp(city);
                    return max > 0
                        && city.wall_hp < max
                        && self.turn.saturating_sub(city.last_attacked) >= 3;
                }
                if project == "repair_encampment" {
                    let max_wall = self.city_max_wall_hp(city);
                    return self.city_has_district_family(city, crate::name!("encampment"))
                        && (city.encampment_pillaged
                            || city.encampment_hp < 100
                            || city.encampment_wall_hp < max_wall)
                        && self.turn.saturating_sub(city.encampment_last_attacked) >= 3;
                }
                // Repairs keep the one city functional; ordinary district,
                // Great Person, loyalty, victory, and conversion projects are
                // major-civilization activity and are never a minor fallback.
                if self.players[pid].is_minor {
                    return false;
                }
                let spec = match self.rules.projects.get(project) {
                    Some(s) => s,
                    None => return false,
                };
                if self.players[pid].is_barbarian {
                    return false;
                }
                if spec.requires_host_competition()
                    && !spec
                        .host_competition_kinds()
                        .any(|kind| self.host_competition(pid, kind).is_some())
                {
                    return false;
                }
                if spec
                    .tech
                    .as_ref()
                    .is_some_and(|t| !self.players[pid].techs.contains(t))
                {
                    return false;
                }
                if spec
                    .civic
                    .as_ref()
                    .is_some_and(|c| !self.players[pid].civics.contains(c))
                {
                    return false;
                }
                if matches!(
                    project.as_str(),
                    "lagrange_laser_station" | "terrestrial_laser_station"
                ) && self.tree_effect(pid, "laser_station_projects") <= 0.0
                {
                    return false;
                }
                if !self.project_has_active_district(city, spec) {
                    return false;
                }
                if !spec.requires.iter().all(|required| {
                    self.players[pid]
                        .science_projects
                        .contains(required.as_str())
                }) {
                    return false;
                }
                if !spec
                    .requires_buildings
                    .iter()
                    .chain(&spec.consumes_buildings)
                    .all(|building| self.city_has_building_family(city, Name::new(building)))
                {
                    return false;
                }
                if let Some(target) = Self::converted_power_plant(project) {
                    let current = city.buildings.iter().find(|building| {
                        matches!(
                            building.as_str(),
                            "coal_power_plant" | "oil_power_plant" | "nuclear_power_plant"
                        )
                    });
                    if current.is_none_or(|plant| plant == target) {
                        return false;
                    }
                }
                spec.repeatable
                    || !self.players[pid]
                        .science_projects
                        .contains(project.as_str())
            }
            Item::Product { product } => {
                if self.players[pid].is_minor
                    || self.players[pid].is_barbarian
                    || self
                        .rules
                        .resources
                        .get(product)
                        .is_none_or(|resource| resource.class != "luxury")
                    || self.world_product_count(product) >= 5
                    || self.product_slot_city(pid, cid).is_none()
                {
                    return false;
                }
                self.city_active_economic_improvement(city)
                    .is_some_and(|(resource, corporation)| corporation && resource == *product)
            }
        }
    }

    pub fn producible_items(&self, pid: usize, cid: u32) -> Vec<Item> {
        if let Some(items) = self
            .query_memo
            .producible
            .borrow()
            .get(&(pid, cid))
            .cloned()
        {
            return items;
        }
        // Every unit, building, district and wonder in the ruleset is offered
        // to the same city in turn, and each offer re-asks that city the same
        // things.
        let _memo = self.query_memo();
        let mut items = Vec::new();
        for name in self.rules.units.keys() {
            let it = Item::Unit { unit: *name };
            if self.can_produce(pid, cid, &it) {
                items.push(it);
            }
            for formation in 1..=2 {
                let formation_item = Item::Formation {
                    unit: *name,
                    formation,
                };
                if self.can_produce(pid, cid, &formation_item) {
                    items.push(formation_item);
                }
            }
        }
        for name in self.rules.buildings.keys() {
            let it = Item::Building { building: *name };
            if self.can_produce(pid, cid, &it) {
                items.push(it);
            }
        }
        for name in self.rules.wonders.keys() {
            let mut sites = self.wonder_sites(cid, name);
            sites.sort();
            for pos in sites.into_iter().take(2) {
                // `wonder_sites` already applies the complete world-unique,
                // unlock, prerequisite, and placement validation used by the
                // `Item::Wonder` arm of `can_produce`.
                let item = Item::Wonder { wonder: *name, pos };
                if self.can_produce(pid, cid, &item) {
                    items.push(item);
                }
            }
        }
        let city = &self.cities[&cid];
        for (_, pos) in &city.districts {
            let district = self.map.tiles[pos].district.unwrap_or(crate::name!(""));
            let district_repair = Item::Repair {
                repair: crate::name!("district"),
                pos: *pos,
            };
            if self.can_produce(pid, cid, &district_repair) {
                items.push(district_repair);
            }
            for building in &city.pillaged_buildings {
                let matches_district = self.rules.buildings.get(building).is_some_and(|spec| {
                    spec.district
                        .as_ref()
                        .is_some_and(|family| self.district_is_family(district, family))
                });
                if matches_district {
                    let repair = Item::Repair {
                        repair: Name::new(building),
                        pos: *pos,
                    };
                    if self.can_produce(pid, cid, &repair) {
                        items.push(repair);
                    }
                }
            }
        }
        for name in self.rules.projects.keys() {
            let it = Item::Project { project: *name };
            if self.can_produce(pid, cid, &it) {
                items.push(it);
            }
        }
        if let Some((resource, true)) = self.city_active_economic_improvement(city) {
            let product = Item::Product { product: resource };
            if self.can_produce(pid, cid, &product) {
                items.push(product);
            }
        }
        for (name, spec) in &self.rules.districts {
            // The sites below are validated by hand rather than through
            // `can_produce`, so the arena's answer is stated here as well:
            // a battlefield builds no districts.
            if self.is_arena()
                || !spec.buildable
                || !self.unlocked(pid, &spec.tech, &spec.civic)
                || spec
                    .unique_to
                    .as_deref()
                    .is_some_and(|civ| !self.owns_civ_unique(pid, civ))
                || self.rules.districts.values().any(|candidate| {
                    candidate.replaces == Some(*name)
                        && candidate
                            .unique_to
                            .as_deref()
                            .is_some_and(|owner| self.owns_civ_unique(pid, owner))
                })
            {
                continue;
            }
            // Rank the sites on values taken once each. Deriving a site's
            // yields means walking its neighbours for adjacency, and a
            // comparator that re-derives both sides pays that for every
            // comparison in the sort rather than for every site.
            let mut ranked: Vec<(bool, f64, Pos)> = self
                .district_sites(cid, name)
                .into_iter()
                .map(|site| {
                    (
                        self.map.tiles[&site].district_foundation.is_some(),
                        self.district_yields(name, site).total(),
                        site,
                    )
                })
                .collect();
            ranked.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| b.1.partial_cmp(&a.1).unwrap())
                    .then(a.2.cmp(&b.2))
            });
            let sites: Vec<Pos> = ranked.into_iter().map(|(_, _, site)| site).collect();
            let mut fresh_sites = 0usize;
            for s in sites {
                let foundation = self.map.tiles[&s].district_foundation.is_some();
                if !foundation && fresh_sites >= 2 {
                    continue;
                }
                // The filters above and `district_sites` together are the
                // complete `Item::District` validation from `can_produce`.
                items.push(Item::District {
                    district: *name,
                    pos: s,
                });
                fresh_sites += usize::from(!foundation);
            }
        }
        self.query_memo
            .producible
            .borrow_mut()
            .insert((pid, cid), items.clone());
        items
    }
}
