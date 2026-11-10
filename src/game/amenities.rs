//! Amenities: what a city needs, where its amenities come from, and what
//! they do.
//!
//! Luxuries (`empire_luxuries`, `shared_luxuries`, `luxury_access_count`,
//! the per-city luxury allocation), a city's required and local amenities,
//! `city_amenities` and the surplus, war weariness, and the yield and growth
//! multipliers amenities apply.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Montezuma: he approves of luxuries he already enjoys, and covets the
    /// ones he does not.
    pub(super) fn shared_luxuries(&self, observer: usize, subject: usize) -> f64 {
        let mine = self.empire_luxury_names(observer);
        let theirs = self.empire_luxury_names(subject);
        if theirs.is_empty() {
            return 0.0;
        }
        let shared = theirs.iter().filter(|name| mine.contains(*name)).count() as f64;
        // Half the range rewards overlap, half punishes what he is missing.
        (2.0 * shared / theirs.len() as f64) - 1.0
    }

    /// Shipped `WAR_WEARINESS_POINTS_FOR_AMENITY_LOSS`: every 400 points cost
    /// one Amenity, in every city the civilization owns.
    pub fn war_weariness_amenity_loss(&self, pid: usize) -> i64 {
        (self.players[pid].war_weariness / 400.0).floor() as i64
    }

    pub(super) fn city_governor_active(&self, pid: usize, cid: u32) -> bool {
        self.players[pid].governors.contains(&cid)
            && self.players[pid]
                .governor_roster
                .values()
                .any(|state| state.city == Some(cid) && self.turn >= state.disabled_until)
    }

    /// Non-stacking regional building yields and Amenities reaching a city.
    pub(super) fn regional_building_effects(&self, city: &City) -> (Yields, f64) {
        if let Some(memo) = self.query_memo.regional.borrow().as_ref() {
            if let Some(value) = memo.get(&city.id) {
                return *value;
            }
        }
        let value = self.regional_building_effects_uncached(city);
        if let Some(memo) = self.query_memo.regional.borrow_mut().as_mut() {
            memo.insert(city.id, value);
        }
        value
    }

    pub(super) fn regional_slots(&self, pid: usize) -> Arc<Vec<RegionalSlot>> {
        if let Some(memo) = self.query_memo.regional_slots.borrow().as_ref() {
            if let Some(slots) = memo.get(&pid) {
                return Arc::clone(slots);
            }
        }
        let slots = Arc::new(self.regional_slots_uncached(pid));
        if let Some(memo) = self.query_memo.regional_slots.borrow_mut().as_mut() {
            memo.insert(pid, Arc::clone(&slots));
        }
        slots
    }

    /// Every active regional building `pid` owns, in city then building
    /// order: that is the order the one additive term, integrated Industrial
    /// Zone production, is summed in.
    fn regional_slots_uncached(&self, pid: usize) -> Vec<RegionalSlot> {
        // ⭐ HOISTED. Mexico City's suzerain bonus depends on the owner and a
        // literal. `grants_city_state_unique_bonus` walks an alliance map and
        // then every player, so asking it per building was once 7.2% of
        // running samples on its own.
        let mexico_city_regional_range = self.grants_city_state_unique_bonus(pid, "Mexico City");
        let mut slots = Vec::new();
        for source in self.cities.values().filter(|source| source.owner == pid) {
            for building in &source.buildings {
                if source.pillaged_buildings.contains(building) {
                    continue;
                }
                let spec = &self.rules.buildings[building];
                let range = spec.regional_range
                    + if mexico_city_regional_range
                        && spec.district.is_some_and(|district| {
                            self.district_is_family(district, crate::name!("industrial_zone"))
                                || self.district_is_family(
                                    district,
                                    crate::name!("entertainment_complex"),
                                )
                                || self.district_is_family(district, crate::name!("water_park"))
                        })
                    {
                        3
                    } else {
                        0
                    };
                // Most buildings have no regional effect. Include Mexico
                // City's range bonus before rejecting them, then resolve
                // activity and the origin only for buildings that can reach.
                if range <= 0 || !self.building_district_is_active(source, building) {
                    continue;
                }
                let origin = spec
                    .district
                    .and_then(|district| self.city_district_family_position(source, district))
                    .unwrap_or(source.pos);
                slots.push(RegionalSlot {
                    source: source.id,
                    building: *building,
                    origin,
                    range,
                });
            }
        }
        slots
    }

    pub(super) fn regional_building_effects_uncached(&self, city: &City) -> (Yields, f64) {
        // ⭐ BORROWED KEYS, NOT OWNED ONES. This map was keyed by `String` and
        // every active building allocated one to look its group up: either
        // `spec.regional_group.clone()` or `building.to_string()`. The function
        // runs once per city per amenity derivation, over every building of
        // every city, so that is one allocation and one copy per building per
        // call, feeding the allocator and `memmove` leaves that a batch profile
        // shows at roughly 17% and 3.5% of running samples.
        //
        // ⚠⚠ THE KEY TYPE HAD TO KEEP ITS ORDER. The fold at the end of this
        // function sums `f64` yields over `groups.values()`, so the map's
        // iteration order is part of the answer -- switching to a `Name` id key
        // would reorder the summation and perturb the result. `Ord for &str` is
        // `Ord for String`'s own implementation, byte for byte, so a
        // `BTreeMap<&str, _>` holds these keys in exactly the same order and the
        // fold is bit-identical. `Name::as_str` returns `&'static str` (the
        // interning registry leaks its text) and `spec.regional_group` lives as
        // long as the `&self` borrow, so both sources outlive the map.
        let mut groups: BTreeMap<&str, (Yields, f64)> = BTreeMap::new();
        let integrate_industry =
            self.governor_effect(city.owner, city.id, "regional_industry_all") > 0.0;
        for slot in self.regional_slots(city.owner).iter() {
            if self.wdist(slot.origin, city.pos) > slot.range {
                continue;
            }
            let source = &self.cities[&slot.source];
            let building = &slot.building;
            let spec = &self.rules.buildings[building];
            let group: &str = if !spec.regional_group.is_empty() {
                spec.regional_group.as_str()
            } else {
                spec.replaces
                    .map_or_else(|| building.as_str(), |name| name.as_str())
            };
            let integrate_this_group = integrate_industry
                && spec.district.is_some_and(|district| {
                    self.district_is_family(district, crate::name!("industrial_zone"))
                });
            let entry = groups.entry(group).or_default();
            let mut source_yields = spec.yields;
            let mut source_amenity = spec.amenity;
            if self.city_is_powered(source) {
                Self::add_powered_building_yields(spec, &mut source_yields);
                source_amenity += spec.effects.get("powered_amenity").copied().unwrap_or(0.0);
            }
            entry.0.food = entry.0.food.max(source_yields.food);
            if integrate_this_group {
                entry.0.production += source_yields.production;
            } else {
                entry.0.production = entry.0.production.max(source_yields.production);
            }
            entry.0.gold = entry.0.gold.max(source_yields.gold);
            entry.0.science = entry.0.science.max(source_yields.science);
            entry.0.culture = entry.0.culture.max(source_yields.culture);
            entry.0.faith = entry.0.faith.max(source_yields.faith);
            entry.1 = entry.1.max(source_amenity);
        }
        groups.values().fold(
            (Yields::default(), 0.0),
            |(mut yields, amenities), (group_yields, group_amenities)| {
                yields.add(*group_yields);
                (yields, amenities + group_amenities)
            },
        )
    }

    /// World-wonder yields whose Civilopedia text names a radius are emitted
    /// from the wonder tile, not duplicated as an ordinary local yield.
    pub(super) fn regional_wonder_effects(&self, city: &City) -> (Yields, f64) {
        let mut yields = Yields::default();
        let mut amenities = 0.0;
        for source in self
            .cities
            .values()
            .filter(|source| source.owner == city.owner)
        {
            for (wonder, position) in &source.wonders {
                let spec = &self.rules.wonders[wonder];
                if spec.regional_range > 0 && self.wdist(*position, city.pos) <= spec.regional_range
                {
                    yields.add(spec.yields);
                    amenities += spec.amenity;
                }
                let faith = spec.effects.get("regional_faith").copied().unwrap_or(0.0);
                let range = spec
                    .effects
                    .get("regional_faith_range")
                    .copied()
                    .unwrap_or(0.0) as i32;
                if faith != 0.0 && self.wdist(*position, city.pos) <= range {
                    yields.faith += faith;
                }
            }
        }
        (yields, amenities)
    }

    pub fn empire_luxuries(&self, pid: usize) -> usize {
        self.empire_luxury_names(pid).len()
    }

    /// Distinct luxury resources that actually supply the empire. A resource
    /// must be improved (or lie under a City Center); merely owning an
    /// unimproved copy does not unlock its Amenities or Aztec combat bonus.
    /// Zanzibar's two synthetic luxuries participate in those same systems.
    /// Asked once per luxury allocation, which is itself asked once per city,
    /// so a six-city empire re-derived its own luxuries dozens of times a turn.
    pub(super) fn empire_luxury_names(&self, pid: usize) -> BTreeSet<Name> {
        if let Some(memo) = self.query_memo.lux_names.borrow().as_ref() {
            if let Some(value) = memo.get(&pid) {
                return value.clone();
            }
        }
        let value = self.empire_luxury_names_uncached(pid);
        if let Some(memo) = self.query_memo.lux_names.borrow_mut().as_mut() {
            memo.insert(pid, value.clone());
        }
        value
    }

    fn empire_luxury_names_uncached(&self, pid: usize) -> BTreeSet<Name> {
        // Which city-states this player is suzerain of, and whether an Amani
        // is sharing their luxuries, are the same for every luxury in the
        // ruleset; deciding them per resource meant re-deriving every
        // city-state's patron twenty-odd times over.
        let clients = self.suzerained_minors(pid);
        let held = LuxuryHoldings {
            own: self.connected_resource_census(pid),
            clients: clients
                .iter()
                .map(|minor| self.connected_resource_census(*minor))
                .collect(),
            amani: self
                .amani_city_state_for_effect(pid, "luxury_resources")
                .map(|(_, minor)| self.connected_resource_census(minor)),
        };
        let mut luxuries: BTreeSet<Name> = self
            .rules
            .resources
            .iter()
            .filter(|(_, spec)| spec.class == "luxury")
            .filter(|(resource, _)| self.luxury_access_count(pid, resource, &held) > 0)
            .map(|(resource, _)| *resource)
            .collect();
        if self.grants_city_state_unique_bonus(pid, "Zanzibar") {
            luxuries.insert(crate::name!("cinnamon"));
            luxuries.insert(crate::name!("cloves"));
        }
        if self.grants_city_state_unique_bonus(pid, "Buenos Aires") {
            for (resource, spec) in &self.rules.resources {
                if spec.class == "bonus" && self.luxury_access_count(pid, resource, &held) > 0 {
                    luxuries.insert(*resource);
                }
            }
        }
        luxuries
    }

    /// Gathering Storm removed the free population Amenity: cities require
    /// ceil(population / 2), while the final Palace supplies the capital with
    /// two Amenities.
    pub fn city_amenities_required(city: &City) -> i64 {
        (city.pop.max(1) as i64 + 1) / 2
    }

    /// `luxury_amenity_allocations_uncached` values every city in the empire, so asking
    /// each of N cities for its Amenities cost N * N of these. Under a memo
    /// scope each city is valued once.
    pub fn city_local_amenities(&self, city: &City) -> i64 {
        if let Some(memo) = self.query_memo.amenities.borrow().as_ref() {
            if let Some(value) = memo.get(&city.id) {
                return *value;
            }
        }
        let value = self.city_local_amenities_uncached(city);
        if let Some(memo) = self.query_memo.amenities.borrow_mut().as_mut() {
            memo.insert(city.id, value);
        }
        value
    }

    pub(super) fn city_local_amenities_uncached(&self, city: &City) -> i64 {
        let mut supply = self.policy_effect(city.owner, "city_amenities");
        supply += if self.city_has_palace(city) {
            self.rules.buildings["palace"].amenity
        } else {
            0.0
        };
        for (district, position) in &city.districts {
            if self.district_is_active(city, district, *position) {
                supply += self.district_amenity(district, *position);
                if matches!(self.district_family(*district).as_str(), "canal" | "dam") {
                    supply += self.governor_effect(city.owner, city.id, "canal_dam_amenity");
                }
                if self.district_family(*district) == "commercial_hub"
                    && self.grants_city_state_unique_bonus(city.owner, "Muscat")
                {
                    supply += 1.0;
                }
            }
        }
        for b in &city.buildings {
            let spec = &self.rules.buildings[b];
            if spec.regional_range <= 0
                && !city.pillaged_buildings.contains(b)
                && self.building_district_is_active(city, b)
            {
                supply += spec.amenity;
                if b == "stadium" {
                    supply += self.policy_effect(city.owner, "stadium_amenity");
                }
                if let Some(&powered_amenity) = spec.effects.get("powered_amenity") {
                    if self.city_is_powered(city) {
                        supply += powered_amenity;
                    }
                }
            }
        }
        supply += self.regional_building_effects(city).1;
        supply += self.regional_wonder_effects(city).1;
        for wonder in city.wonders.keys() {
            let spec = &self.rules.wonders[wonder];
            if spec.regional_range <= 0 {
                supply += spec.amenity;
            }
        }
        if self.city_governor_active(city.owner, city.id) {
            supply += self.city_building_effect(city, "governor_city_amenity");
        }
        for position in &city.owned_tiles {
            let Some(feature) = self.map.tiles[position].feature.as_deref() else {
                continue;
            };
            let effects = &self.rules.features[feature].effects;
            supply += effects.get("city_amenity").copied().unwrap_or(0.0);
            // Pamukkale grants one more Amenity while its plot is adjacent to
            // an Entertainment Complex.
            let adjacent = effects
                .get("entertainment_complex_adjacent_amenity")
                .copied()
                .unwrap_or(0.0);
            if adjacent != 0.0
                && self.nbrs(*position).iter().any(|neighbour| {
                    self.map.get(*neighbour).is_some_and(|tile| {
                        tile.district == Some(crate::name!("entertainment_complex"))
                    })
                })
            {
                supply += adjacent;
            }
        }
        let geothermal = self.city_building_effect(city, "geothermal_fissure_amenity");
        if geothermal != 0.0
            && city.owned_tiles.iter().any(|position| {
                self.map.tiles[position].feature.as_deref() == Some("geothermal_fissure")
            })
        {
            supply += geothermal;
        }
        let per_luxury = self.city_building_effect(city, "city_luxury_amenity");
        if per_luxury != 0.0 {
            let luxuries: BTreeSet<&str> = city
                .owned_tiles
                .iter()
                .filter_map(|position| {
                    let tile = &self.map.tiles[position];
                    let resource = tile.resource?;
                    let spec = self.rules.resources.get_interned(resource)?;
                    (spec.class == "luxury"
                        && self.tile_connects_resource(tile, resource, *position == city.pos))
                    .then_some(resource.as_str())
                })
                .collect();
            supply += per_luxury * luxuries.len() as f64;
        }
        for (wonder, position) in &city.wonders {
            let spec = &self.rules.wonders[wonder];
            if let Some(range) = spec.effects.get("improvement_amenity_range") {
                supply += self
                    .wdisk(*position, *range as i32)
                    .into_iter()
                    .filter(|candidate| {
                        self.map.get(*candidate).is_some_and(|tile| {
                            tile.owner_city
                                .and_then(|owner_city| self.cities.get(&owner_city))
                                .is_some_and(|owner_city| owner_city.owner == city.owner)
                                && matches!(
                                    tile.improvement.as_deref(),
                                    Some("camp" | "pasture" | "plantation")
                                )
                                && !tile.pillaged
                        })
                    })
                    .count() as f64;
            }
            let lake_amenity = spec
                .effects
                .get("adjacent_lake_amenity")
                .copied()
                .unwrap_or(0.0);
            if lake_amenity != 0.0 {
                supply += lake_amenity
                    * self
                        .nbrs(*position)
                        .iter()
                        .filter(|candidate| self.map.tiles[candidate].terrain == "lake")
                        .count() as f64;
            }
        }
        for position in &city.owned_tiles {
            if self.map.tiles[position].pillaged {
                continue;
            }
            if let Some(improvement) = self.map.tiles[position].improvement.as_deref() {
                supply += self.rules.improvements[improvement]
                    .effects
                    .get("amenity")
                    .copied()
                    .unwrap_or(0.0);
                if improvement == "city_park"
                    && (self.map.tiles[position].has_river()
                        || self.nbrs(*position).iter().any(|neighbor| {
                            self.map
                                .get(*neighbor)
                                .is_some_and(|tile| self.rules.is_water(tile))
                        }))
                {
                    supply += self.rules.improvements[improvement]
                        .effects
                        .get("adjacent_water_amenity")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
        }
        let mound = &self.rules.improvements["mound"];
        let mound_count = city
            .owned_tiles
            .iter()
            .filter(|position| {
                let tile = &self.map.tiles[position];
                !tile.pillaged && tile.improvement.as_deref() == Some("mound")
            })
            .count() as f64;
        let mound_limit = if self.players[city.owner]
            .civics
            .contains(&crate::name!("natural_history"))
        {
            mound
                .effects
                .get("city_amenity_limit_after_natural_history")
                .copied()
                .unwrap_or(0.0)
        } else {
            mound
                .effects
                .get("city_amenity_limit")
                .copied()
                .unwrap_or(0.0)
        };
        supply += mound_count.min(mound_limit)
            * mound.effects.get("city_amenity").copied().unwrap_or(0.0);
        let active_specialty_districts = city
            .districts
            .iter()
            .filter(|(district, position)| {
                self.rules.districts[district].specialty
                    && self.district_is_active(city, district, **position)
            })
            .count();
        if active_specialty_districts >= 2 {
            supply += self.city_religion_belief_effect(city, "amenity_two_specialty");
        }
        for (source_city, positions) in self.established_national_parks(city.owner) {
            let effects = &self.rules.improvements["national_park"].effects;
            if source_city == city.id {
                supply += effects.get("host_city_amenity").copied().unwrap_or(0.0);
                continue;
            }
            let mut nearest: Vec<(i32, u32)> = self
                .cities
                .values()
                .filter(|candidate| candidate.owner == city.owner && candidate.id != source_city)
                .map(|candidate| {
                    (
                        positions
                            .iter()
                            .map(|position| self.wdist(candidate.pos, *position))
                            .min()
                            .unwrap_or(i32::MAX),
                        candidate.id,
                    )
                })
                .collect();
            nearest.sort();
            if nearest
                .into_iter()
                .take(
                    effects
                        .get("nearby_city_count")
                        .copied()
                        .unwrap_or(0.0)
                        .max(0.0) as usize,
                )
                .any(|(_, candidate)| candidate == city.id)
            {
                supply += effects.get("nearby_city_amenity").copied().unwrap_or(0.0);
            }
        }
        if city.pop >= 10 {
            supply += self.policy_effect(city.owner, "large_city_amenity");
        }
        let government = self.gov_effects(city.owner);
        supply += government.amenity;
        if !city.districts.is_empty() {
            supply += government.district_city_amenity;
        }
        let garrison = self.unit_ids_at(city.pos).iter().any(|id| {
            let o = &self.units[id];
            o.owner == city.owner && self.rules.units[o.kind].class == "military"
        });
        if garrison {
            supply += self.policy_effect(city.owner, "garrison_amenity");
        }
        let specialty_districts = self.city_specialty_district_count(city);
        if specialty_districts >= 2 {
            supply += self.policy_effect(city.owner, "amenity_at_2_districts");
        }
        if specialty_districts >= 3 {
            supply += self.policy_effect(city.owner, "amenity_at_3_districts");
        }
        if self.city_governor_active(city.owner, city.id) {
            supply += self.policy_effect(city.owner, "governor_amenity");
        }
        // War Weariness "is applied to every city you own as a negative
        // Amenity" — it is empire-wide, not a property of the cities doing
        // the fighting.
        supply.round() as i64 - self.war_weariness_amenity_loss(city.owner)
    }

    /// Each distinct luxury gives +1 Amenity to the four cities that need it
    /// most. Gifts for the Tlatoani raises that reach to six for the Aztecs.
    /// Allocating luxuries is an empire-wide decision, but every caller wants
    /// one city's share of it. Deriving it per city was the single largest
    /// redundancy in the city layer.
    pub(super) fn city_luxury_amenities(&self, city: &City) -> i64 {
        let pid = city.owner;
        if let Some(memo) = self.query_memo.lux_alloc.borrow().as_ref() {
            if let Some(value) = memo.get(&pid) {
                return value.get(&city.id).copied().unwrap_or(0);
            }
        }
        let value = self.luxury_amenity_allocations_uncached(pid);
        let amenities = value.get(&city.id).copied().unwrap_or(0);
        if let Some(memo) = self.query_memo.lux_alloc.borrow_mut().as_mut() {
            memo.insert(pid, value);
        }
        amenities
    }

    pub(super) fn luxury_amenity_allocations_uncached(&self, pid: usize) -> BTreeMap<u32, i64> {
        let cities: Vec<&City> = self
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .collect();
        let mut reaches = Vec::new();
        for luxury in self.empire_luxury_names(pid) {
            if self.congress_effect_active("luxury_policy", "B", &luxury) {
                continue;
            }
            let copies = if self.congress_effect_active("luxury_policy", "A", &luxury) {
                self.resource_access_count(pid, &luxury).max(1) as usize
            } else {
                1_usize
            };
            // Cinnamon and Cloves each provide six Amenities. The Aztec
            // ability independently raises every ordinary Luxury to the same
            // six-city reach.
            let reach = if matches!(luxury.as_str(), "cinnamon" | "cloves")
                || self.has_ability(pid, "gifts_for_the_tlatoani")
            {
                6
            } else {
                4
            };
            reaches.extend(std::iter::repeat_n(reach, copies));
        }
        // If every luxury reaches every city (including no luxuries at all),
        // need cannot affect the answer. Avoid valuing the whole empire just
        // to discover that every city receives the same supply.
        if reaches.iter().all(|reach| *reach >= cities.len()) {
            return cities
                .iter()
                .map(|city| (city.id, reaches.len() as i64))
                .collect();
        }
        let mut allocations: BTreeMap<u32, i64> = cities.iter().map(|city| (city.id, 0)).collect();
        let mut surplus: BTreeMap<u32, i64> = cities
            .iter()
            .map(|city| {
                (
                    city.id,
                    self.city_local_amenities(city) - Self::city_amenities_required(city),
                )
            })
            .collect();
        let mut neediest: Vec<u32> = cities.iter().map(|city| city.id).collect();
        for reach in reaches {
            neediest.sort_by_key(|cid| (surplus[cid], *cid));
            for cid in neediest.iter().copied().take(reach) {
                *allocations.get_mut(&cid).unwrap() += 1;
                *surplus.get_mut(&cid).unwrap() += 1;
            }
        }
        allocations
    }

    /// Amenities supplied to this city before subtracting its population need
    /// or an empire-wide bankruptcy penalty.
    ///
    /// On a mirrored board this includes the host-to-model correction the
    /// mirror derived (`observed_city_amenity_adjustments`), so the count a
    /// viewer reads beside `amenities_required` is the host's own count and
    /// not the model's — the two disagreed by up to three on live Rome
    /// (host 3, model 6, run civvis-20260816T011314Z t151) because Firaxis
    /// does not hand luxuries to the neediest city first the way CIVVIS does.
    /// The surplus below reads through here, so the correction is applied
    /// exactly once.
    pub fn city_amenities(&self, city: &City) -> i64 {
        let luxury = self.city_luxury_amenities(city);
        self.city_local_amenities(city)
            + luxury
            + self
                .observed_city_amenity_adjustments
                .get(&city.id)
                .copied()
                .unwrap_or(0)
    }

    pub fn city_amenity_surplus(&self, city: &City) -> i64 {
        self.city_amenities(city)
            - Self::city_amenities_required(city)
            - self.players[city.owner].bankruptcy_amenity_penalty
    }

    pub(super) fn amenity_yield_mult(&self, city: &City) -> f64 {
        Self::amenity_yield_mult_for(self.city_amenity_surplus(city))
    }

    /// Final Gathering Storm satisfaction bands after the December 2020
    /// restoration: Ecstatic begins at +5 and Happy at +3; the negative
    /// bands remain Displeased -1/-2 through Revolt at -7 or worse.
    pub(super) fn amenity_yield_mult_for(surplus: i64) -> f64 {
        if surplus >= 5 {
            1.20
        } else if surplus >= 3 {
            1.10
        } else if surplus >= 0 {
            1.0
        } else if surplus >= -2 {
            0.90
        } else if surplus >= -4 {
            0.80
        } else if surplus >= -6 {
            0.70
        } else {
            0.60
        }
    }

    /// Final Gathering Storm growth bands: Ecstatic +20%, Happy +10%,
    /// Displeased -15%, Unhappy -30%, no growth from Unrest down.
    pub(super) fn amenity_growth_mult(surplus: i64) -> f64 {
        if surplus >= 5 {
            1.20
        } else if surplus >= 3 {
            1.10
        } else if surplus >= 0 {
            1.0
        } else if surplus >= -2 {
            0.85
        } else if surplus >= -4 {
            0.70
        } else {
            0.0
        }
    }

    /// How many connected copies of every resource a player holds, in one
    /// pass over their cities.
    ///
    /// [`Self::connected_resource_count_unchecked`] walks every tile of every
    /// city the player owns to count one resource. Anything that sweeps the
    /// whole luxury or strategic table was doing that walk twenty-odd times
    /// over for a set of answers one walk produces.
    pub(super) fn connected_resource_census(&self, pid: usize) -> BTreeMap<&str, i32> {
        let mut counts: BTreeMap<&str, i32> = BTreeMap::new();
        for city in self.cities.values().filter(|city| city.owner == pid) {
            for position in &city.owned_tiles {
                let tile = &self.map.tiles[position];
                let Some(resource) = tile.resource else {
                    continue;
                };
                if self.tile_connects_resource(tile, resource, *position == city.pos) {
                    *counts.entry(resource.as_str()).or_insert(0) += 1;
                }
            }
        }
        counts
    }

    fn resource_trade_balance(&self, pid: usize, res: &str) -> i32 {
        self.active_trade_deals
            .iter()
            .filter(|deal| deal.ends > self.turn)
            .map(|deal| {
                let offered = deal.offer.resources.get(res).copied().unwrap_or(0);
                let requested = deal.request.resources.get(res).copied().unwrap_or(0);
                if deal.from == pid {
                    requested - offered
                } else if deal.to == pid {
                    offered - requested
                } else {
                    0
                }
            })
            .sum()
    }

    /// Luxuries use connected and temporarily traded copies. Gathering Storm
    /// strategic resources instead expose the integer amount in their stockpile.
    /// [`Self::resource_access_count`] for a luxury, with the two answers
    /// that do not depend on which luxury it is passed in.
    fn luxury_access_count(&self, pid: usize, res: &str, held: &LuxuryHoldings) -> i32 {
        let visible = self.resource_visible_to(pid, res);
        let count = |census: &BTreeMap<&str, i32>| census.get(res).copied().unwrap_or(0);
        let ordinary = if visible {
            count(&held.own) + held.clients.iter().map(count).sum::<i32>()
        } else {
            0
        } + self.resource_trade_balance(pid, res);
        let shared = held
            .amani
            .as_ref()
            .filter(|census| visible && count(census) > 0)
            .map(|_| 1)
            .unwrap_or(0);
        ordinary.max(shared).max(0)
    }

    pub fn resource_access_count(&self, pid: usize, res: &str) -> i32 {
        match self
            .rules
            .resources
            .get(res)
            .map(|spec| spec.class.as_str())
        {
            Some("strategic") => self
                .strategic_stockpile(pid, Name::new(res))
                .floor()
                .max(0.0) as i32,
            _ => {
                let ordinary = self.controlled_resource_count(pid, res)
                    + self.resource_trade_balance(pid, res);
                let amani = self
                    .amani_city_state_for_effect(pid, "luxury_resources")
                    .filter(|(_, minor)| {
                        self.resource_visible_to(pid, res)
                            && self.connected_resource_count_unchecked(*minor, res) > 0
                    })
                    .map(|_| 1)
                    .unwrap_or(0);
                ordinary.max(amani).max(0)
            }
        }
    }

    // -------------------------------------------------------- unit helpers
}
