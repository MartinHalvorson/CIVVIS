//! Housing: a city's housing, where it comes from, and what it does to
//! growth.
//!
//! `city_housing` and `city_housing_sources`, the housing floor and
//! headroom, the Aqueduct's gain, and `housing_growth_mult`.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Whether an existing or prospective city centre stands on fresh water,
    /// and whether it is coastal.
    ///
    /// Both decide the housing floor. Keeping this at a position instead of
    /// only on [`City`] lets settlement scoring ask the same question before
    /// founding, including the host's observed `Plot:IsFreshWater()` result.
    pub(crate) fn city_site_water(&self, pid: usize, pos: Pos) -> (bool, bool) {
        let center = &self.map.tiles[&pos];
        let fresh = self.grants_city_state_unique_bonus(pid, "Mohenjo-Daro")
            || center.has_river()
            || self.nbrs(pos).iter().any(|n| {
                self.map
                    .get(*n)
                    .is_some_and(|t| t.terrain == "lake" || t.feature.as_deref() == Some("oasis"))
            });
        // The host's own answer for the centre plot, when a live mirror has it:
        // `Plot:IsFreshWater()` already folds in rivers, lakes and oases, and a
        // lake the export names `TERRAIN_COAST` is one the derivation misses.
        let fresh = self
            .observed_fresh_water
            .get(&pos)
            .copied()
            .unwrap_or(fresh);
        let coastal = self.nbrs(pos).iter().any(|n| {
            self.map
                .get(*n)
                .map(|t| matches!(t.terrain.as_str(), "coast" | "ocean"))
                .unwrap_or(false)
        });
        (fresh, coastal)
    }

    /// Whether the city centre stands on fresh water, and whether it is
    /// coastal. Both decide the housing floor, and an Aqueduct's whole worth
    /// is the gap between the two floors — so the test lives in one place and
    /// `city_housing` and `aqueduct_housing_gain` read it from here.
    pub(super) fn city_water(&self, city: &City) -> (bool, bool) {
        self.city_site_water(city.owner, city.pos)
    }

    /// The housing a city centre carries before any building, improvement or
    /// district adds to it: fresh water 5, coastal 3, otherwise 2, and an
    /// Aqueduct raises the floor to 7 on fresh water or 6 without it (Civ 6).
    ///
    /// ⚠ Stated ONCE. `aqueduct_housing_gain` is this function's difference
    /// across `has_aqueduct`, so a governor deciding whether an Aqueduct is
    /// worth building cannot disagree with the model that pays for it.
    pub(crate) fn city_housing_floor(fresh: bool, coastal: bool, has_aqueduct: bool) -> f64 {
        if has_aqueduct {
            if fresh {
                7.0
            } else {
                6.0
            }
        } else if fresh {
            5.0
        } else if coastal {
            3.0
        } else {
            2.0
        }
    }

    /// What an Aqueduct this city does not yet have would add to its housing:
    /// **+2** on fresh water, **+3** coastal, **+4** on a dry inland centre.
    /// Zero once one is standing.
    ///
    /// This is the largest single housing step available to an early city and
    /// the district that supplies it is the cheapest in the ruleset (36), which
    /// is why the chooser is given the number rather than a flat weight — the
    /// dry city that needs it most gains twice what a river city does.
    pub fn aqueduct_housing_gain(&self, city: &City) -> f64 {
        if self.city_has_active_district_family(city, crate::name!("aqueduct")) {
            return 0.0;
        }
        let (fresh, coastal) = self.city_water(city);
        Self::city_housing_floor(fresh, coastal, true)
            - Self::city_housing_floor(fresh, coastal, false)
    }

    /// Housing headroom: what the city may still grow into before
    /// `housing_growth_mult` starts throttling it. Negative once population
    /// has overrun the ceiling.
    pub fn city_housing_headroom(&self, city: &City) -> f64 {
        self.city_housing(city) - city.pop as f64
    }

    pub fn city_housing(&self, city: &City) -> f64 {
        // A mirrored board reads the host's ceiling: the mirror stores
        // `host - model` here, so the number beside population is the one the
        // host grows against, while a planned building still moves it by its
        // modeled amount. Empty on a native game.
        self.city_housing_sources(city).total()
            + self
                .observed_city_housing_adjustments
                .get(&city.id)
                .copied()
                .unwrap_or(0.0)
    }

    /// Where this city's housing comes from, in the categories the host reports.
    ///
    /// ⚠⚠ THE HOST HAS ALWAYS EXPORTED ITS OWN BREAKDOWN AND WE COMPARED ONLY
    /// THE TOTAL. Every `state` record carries `housing_from_water`,
    /// `_buildings`, `_districts`, `_improvements`, `_civics`, `_great_people`,
    /// `_great_works` and `_starting_era`, and they sum exactly to `housing`.
    /// `civ6_yield_drift.py` compared one number against one number, so a
    /// persistent ±2 in Rome was a mystery for as long as it stood rather than a
    /// named category, and the live bridge's per-turn correction hid it
    /// everywhere except the instrument nobody could act on.
    ///
    /// The arithmetic here is exactly `city_housing`'s former body with each
    /// term written to a named field instead of one accumulator, so the total
    /// cannot drift from the parts — `city_housing` now derives from this.
    ///
    /// Two mappings are worth stating because they are not one-to-one:
    ///
    /// * **The Aqueduct counts as WATER to the host, not as a district.** An
    ///   Aqueduct grants its city fresh water, so Civilization VI folds its
    ///   lift into `housing_from_water` and leaves `housing_from_districts` to
    ///   the Neighborhoods and the rest. This was written the other way round
    ///   first, and the per-source comparison found it within one run: every
    ///   aqueduct city reported `districts` and `water` wrong by exactly equal
    ///   and opposite amounts, and those amounts were exactly the aqueduct
    ///   lifts this ruleset gives — +2 fresh, +3 coastal, +4 dry. A total-only
    ///   comparison could not have seen it at all, because the total was right.
    /// * **`other` has no host counterpart.** Resource industries and the
    ///   government's wall-level housing are CIVVIS terms with nowhere to go in
    ///   the host's list. They are reported separately rather than hidden
    ///   inside a category the host also fills, because a category that mixes
    ///   comparable and incomparable terms cannot be compared at all.
    pub fn city_housing_sources(&self, city: &City) -> HousingSources {
        let mut s = HousingSources::default();
        let (fresh, coastal) = self.city_water(city);
        let has_aqueduct = self.city_has_active_district_family(city, crate::name!("aqueduct"));
        s.water = Self::city_housing_floor(fresh, coastal, has_aqueduct);
        if self.city_has_palace(city) {
            s.buildings += self.rules.buildings["palace"].housing;
        }
        for pos in city
            .owned_tiles
            .iter()
            .filter(|p| self.wdist(city.pos, **p) <= 3)
        {
            // ⚠ A PILLAGED IMPROVEMENT GIVES NOTHING UNTIL IT IS REPAIRED, AND
            // THIS LOOP USED TO IGNORE THAT. The building loop below has always
            // honoured `city.pillaged_buildings`; this one never looked at
            // `tile.pillaged`, so a razed farm kept feeding the city's growth
            // ceiling forever. Housing is what caps growth, so the city went on
            // planning against a ceiling the host had already taken away.
            //
            // The per-source drift comparison is what surfaced it: on the live
            // seat `improvements` read HIGH against the host — Ostia +1.5,
            // Cumae +2, Aquileia +1 — while every other category agreed. A
            // total-only comparison had shown the same cities off by ±1 to ±2
            // for weeks without naming anything.
            if self.map.tiles[pos].pillaged {
                continue;
            }
            if let Some(improvement) = self.map.tiles[pos].improvement.as_deref() {
                let spec = self
                    .rules
                    .improvements
                    .get(improvement)
                    .expect("a placed improvement is in the ruleset");
                s.improvements += spec.housing;
                if self.players[city.owner]
                    .civics
                    .contains(&crate::name!("colonialism"))
                {
                    s.improvements += spec
                        .effects
                        .get("housing_after_colonialism")
                        .copied()
                        .unwrap_or(0.0);
                }
                if improvement == "mound"
                    && self.players[city.owner]
                        .civics
                        .contains(&crate::name!("cultural_heritage"))
                {
                    s.improvements += spec
                        .effects
                        .get("housing_after_cultural_heritage")
                        .copied()
                        .unwrap_or(0.0);
                }
                if improvement == "mekewap"
                    && self.players[city.owner]
                        .civics
                        .contains(&crate::name!("civil_service"))
                {
                    s.improvements += spec
                        .effects
                        .get("housing_after_civil_service")
                        .copied()
                        .unwrap_or(0.0);
                }
                if improvement == "kampung"
                    && self.players[city.owner]
                        .civics
                        .contains(&crate::name!("civil_engineering"))
                {
                    s.improvements += spec
                        .effects
                        .get("housing_after_civil_engineering")
                        .copied()
                        .unwrap_or(0.0);
                }
            }
        }
        for b in city.buildings.iter().filter(|building| {
            !city.pillaged_buildings.contains(*building)
                && self.building_district_is_active(city, building)
        }) {
            let building = &self.rules.buildings[b];
            s.buildings += building.housing;
            if coastal {
                s.buildings += building
                    .effects
                    .get("coastal_city_housing")
                    .copied()
                    .unwrap_or(0.0);
            }
            if self.city_governor_active(city.owner, city.id) {
                s.buildings += building
                    .effects
                    .get("governor_city_housing")
                    .copied()
                    .unwrap_or(0.0);
            }
        }
        if self.city_religion(city).is_some() {
            if self.city_has_active_building_family(city, crate::name!("shrine")) {
                s.buildings += self.city_religion_belief_effect(city, "shrine_housing");
            }
            if self.city_has_active_building_family(city, crate::name!("temple")) {
                s.buildings += self.city_religion_belief_effect(city, "temple_housing");
            }
        }
        for (district, position) in &city.districts {
            if self.district_is_active(city, district, *position) {
                s.districts += self.district_housing(district, *position);
                if matches!(
                    self.district_family(*district).as_str(),
                    "neighborhood" | "aqueduct"
                ) {
                    s.districts +=
                        self.governor_effect(city.owner, city.id, "neighborhood_aqueduct_housing");
                }
            }
        }
        for wonder in city.wonders.keys() {
            s.buildings += self.rules.wonders[wonder].housing;
        }
        s.buildings += self.empire_wonder_effect(city.owner, "empire_housing");
        // Collectivism's COLLECTIVISM_ADD_HOUSING: +2 in every city.
        s.civics += self.policy_effect(city.owner, "city_housing");
        let specialty_districts = self.city_specialty_district_count(city);
        if specialty_districts >= 2 {
            s.civics += self.policy_effect(city.owner, "housing_at_2_districts");
        }
        if specialty_districts >= 3 {
            s.civics += self.policy_effect(city.owner, "housing_at_3_districts");
        }
        if self.city_governor_active(city.owner, city.id) {
            s.civics += self.policy_effect(city.owner, "governor_housing");
        }
        s.other += self.city_resource_industry_effects(city).housing;
        let government = self.gov_effects(city.owner);
        s.civics += government.housing;
        if !city.districts.is_empty() {
            s.civics += government.district_city_housing;
        }
        let active_wall_levels = city
            .buildings
            .iter()
            .filter(|building| {
                !city.pillaged_buildings.contains(*building)
                    && self.building_district_is_active(city, building)
                    && self.rules.buildings[building].outer_defense > 0
            })
            .count() as f64;
        s.other += government.wall_level_housing * active_wall_levels;
        s
    }

    /// Housing left (Housing minus Population) throttles growth at the three
    /// shipped steps: `CITY_HOUSING_LEFT_50PCT_GROWTH` 1,
    /// `CITY_HOUSING_LEFT_25PCT_GROWTH` 0 and `CITY_HOUSING_LEFT_ZERO_GROWTH`
    /// -4. Growth stops *at* -4, not one short of -5.
    pub(super) fn housing_growth_mult(headroom: f64) -> f64 {
        if headroom >= 2.0 {
            1.0
        } else if headroom >= 1.0 {
            0.5
        } else if headroom > -4.0 {
            0.25
        } else {
            0.0
        }
    }
}
