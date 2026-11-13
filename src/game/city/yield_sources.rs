//! The two longest sources of a city's yields: its buildings and the trade
//! routes it sends.
//!
//! `city_yields_inner` adds up everything a city makes, one source after the
//! next. These two loops were over 500 of its lines; each reads the city and
//! adds to the running total, so each is a method here.
//!
//! Moved verbatim out of `city_yields_inner` in `game/city.rs` (2026-11-06).

use super::*;

impl Game {
    /// What `city`'s buildings pay it, skipping any that are pillaged or whose
    /// district is not active.
    pub(super) fn add_building_yields(&self, city: &City, ys: &mut Yields) {
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
                yields.science += self.policy_effect_named(
                    city.owner,
                    crate::name!("late_military_building_science"),
                );
            }
            if matches!(
                b.as_str(),
                "military_academy"
                    | "research_lab"
                    | "coal_power_plant"
                    | "oil_power_plant"
                    | "nuclear_power_plant"
            ) {
                yields.culture +=
                    self.policy_effect_named(city.owner, crate::name!("late_building_culture"));
                yields.gold +=
                    self.policy_effect_named(city.owner, crate::name!("late_building_gold"));
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
    }

    /// What the trade routes `cid` sends pay it.
    pub(super) fn add_trade_route_yields(&self, cid: u32, city: &City, ys: &mut Yields) {
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
                rys.gold += self.policy_effect_named(city.owner, crate::name!("trade_gold"));
                rys.food += self.policy_effect_named(city.owner, crate::name!("trade_food"));
                rys.production +=
                    self.policy_effect_named(city.owner, crate::name!("trade_production"));
                if domestic {
                    // Isolationism pays only at home, and pays all three.
                    rys.food +=
                        self.policy_effect_named(city.owner, crate::name!("domestic_trade_food"));
                    // Isolationism's rows carry `Intercontinental=0`: only a
                    // domestic route that stays on one continent pays them.
                    if self.same_continent(city.pos, dc.pos) {
                        rys.food += self.policy_effect_named(
                            city.owner,
                            crate::name!("domestic_same_continent_trade_food"),
                        );
                        rys.production += self.policy_effect_named(
                            city.owner,
                            crate::name!("domestic_same_continent_trade_production"),
                        );
                    }
                    rys.production += self
                        .policy_effect_named(city.owner, crate::name!("domestic_trade_production"));
                    rys.gold +=
                        self.policy_effect_named(city.owner, crate::name!("domestic_trade_gold"));
                }
                if !domestic {
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
                    rys.gold += self
                        .policy_effect_named(city.owner, crate::name!("international_trade_gold"));
                    rys.production += self.policy_effect_named(
                        city.owner,
                        crate::name!("international_trade_production"),
                    );
                    // Trade Confederation and Market Economy both ship as
                    // ADJUST_TRADE_ROUTE_YIELD_FOR_INTERNATIONAL, and they are
                    // the only owners of these two yields anywhere in the
                    // policy tree, so no domestic route earns either.
                    rys.culture += self.policy_effect_named(
                        city.owner,
                        crate::name!("international_trade_culture"),
                    );
                    rys.science += self.policy_effect_named(
                        city.owner,
                        crate::name!("international_trade_science"),
                    );
                    // Market Economy's Gold is not a flat amount: it ships as
                    // PER_DESTINATION_LUXURY_RESOURCE and
                    // PER_DESTINATION_STRATEGIC_RESOURCE, one Gold each, so it
                    // is worth what the far city actually owns.
                    let luxury = self
                        .policy_effect_named(city.owner, crate::name!("international_luxury_gold"));
                    let strategic = self.policy_effect_named(
                        city.owner,
                        crate::name!("international_strategic_gold"),
                    );
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
                rys.faith += self.policy_effect_named(city.owner, crate::name!("trade_faith"));
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
                    rys.gold +=
                        self.policy_effect_named(city.owner, crate::name!("city_state_route_gold"));
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
                    rys.food += self.policy_effect_named(
                        city.owner,
                        crate::name!("allied_suzerain_trade_food"),
                    );
                    rys.production += self.policy_effect_named(
                        city.owner,
                        crate::name!("allied_suzerain_trade_production"),
                    );
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
    }
}
