//! City power: demand, supply, renewables, and what being powered unlocks.
//!
//! `city_power_demand`, `city_power_supply`, `city_renewable_power`,
//! `city_is_powered`, whether a project fully powers a city, the turn's
//! `process_power`, and the yields of powered buildings.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub fn city_power_demand(&self, city: &City) -> f64 {
        city.buildings
            .iter()
            .filter(|building| {
                !city.pillaged_buildings.contains(*building)
                    && self.building_district_is_active(city, building)
            })
            .map(|building| self.rules.buildings[building].power)
            .sum()
    }

    pub(super) fn city_renewable_power(&self, city: &City) -> f64 {
        let mut renewable = city
            .buildings
            .iter()
            .filter(|building| {
                !city.pillaged_buildings.contains(*building)
                    && self.building_district_is_active(city, building)
            })
            .map(|building| {
                self.rules.buildings[building]
                    .effects
                    .get("renewable_power_generated")
                    .copied()
                    .unwrap_or(0.0)
            })
            .sum::<f64>();
        // Cardiff: `ADJUST_FREE_POWER` 2 for its Suzerain, one row each for the
        // Lighthouse, the Shipyard and the Seaport. The buildings are counted
        // first: they read this city alone, where
        // `grants_city_state_unique_bonus` scans every player.
        let harbor_buildings = [
            crate::name!("lighthouse"),
            crate::name!("shipyard"),
            crate::name!("seaport"),
        ]
        .into_iter()
        .filter(|building| self.city_has_active_building_family(city, *building))
        .count();
        if harbor_buildings > 0 && self.grants_city_state_unique_bonus(city.owner, "Cardiff") {
            renewable += 2.0 * harbor_buildings as f64;
        }
        for position in &city.owned_tiles {
            let tile = &self.map.tiles[position];
            if tile.pillaged {
                continue;
            }
            if let Some(improvement) = &tile.improvement {
                renewable += self.rules.improvements[improvement]
                    .effects
                    .get("power")
                    .copied()
                    .unwrap_or(0.0);
                if matches!(
                    improvement.as_str(),
                    "wind_farm" | "solar_farm" | "offshore_wind_farm" | "geothermal_plant"
                ) {
                    renewable += self.governor_effect(city.owner, city.id, "renewable_power_bonus");
                }
            }
        }
        if self.city_has_district_family(city, crate::name!("spaceport")) {
            renewable += self.policy_effect(city.owner, "spaceport_power");
        }
        if self.city_has_active_building_family(city, crate::name!("hydroelectric_dam")) {
            renewable += self.governor_effect(city.owner, city.id, "renewable_power_bonus");
        }
        renewable *= 1.0 + self.empire_wonder_effect(city.owner, "renewable_power_pct") / 100.0;
        renewable + self.gov_effects(city.owner).power_per_city
    }

    pub fn city_power_supply(&self, city: &City) -> f64 {
        // Industrial Zone Logistics (and any future project with the same
        // ruleset flag) fully powers its city while it is running.  The turn
        // processor records that in `city_power`, but mirror and planning
        // queries can run before `process_power` has populated that cache.
        // Keep the direct query on the same truth as turn processing.
        if self.city_project_fully_powers(city) {
            return self.city_power_demand(city);
        }
        self.players[city.owner]
            .city_power
            .get(&city.id)
            .copied()
            .unwrap_or_else(|| self.city_renewable_power(city))
    }

    pub fn city_is_powered(&self, city: &City) -> bool {
        if let Some(memo) = self.query_memo.powered.borrow().as_ref() {
            if let Some(powered) = memo.get(&city.id) {
                return *powered;
            }
        }
        let powered = self.city_is_powered_uncached(city);
        if let Some(memo) = self.query_memo.powered.borrow_mut().as_mut() {
            memo.insert(city.id, powered);
        }
        powered
    }

    pub(super) fn city_is_powered_uncached(&self, city: &City) -> bool {
        let demand = self.city_power_demand(city);
        demand <= 0.0 || self.city_power_supply(city) + 1e-9 >= demand
    }

    /// Whether an active project provides full Power independently of the
    /// empire's fuel and renewable allocation.  This is shared by the turn
    /// processor and read-only yield queries so a project cannot be powered in
    /// one path and unpowered in the other.
    fn city_project_fully_powers(&self, city: &City) -> bool {
        let Some(Item::Project { project }) = city.queue.first() else {
            return false;
        };
        let Some(spec) = self.rules.projects.get(project) else {
            return false;
        };
        spec.full_power_while_active && self.project_has_active_district(city, spec)
    }

    /// Allocate renewable Power first, then burn the most efficient reachable
    /// fuel. Equal-efficiency fuels follow the Civilopedia tie breakers:
    /// largest stockpile, highest income, then most advanced resource.
    pub(super) fn process_power(&mut self, pid: usize) {
        self.players[pid].city_power.clear();
        self.players[pid].power_fuel_consumed.clear();
        if self.players[pid].is_barbarian {
            return;
        }

        let city_ids = self.player_city_ids(pid);
        let fuel_specs: Vec<(&str, &str, f64, f64, i32)> = ["uranium", "oil", "coal"]
            .into_iter()
            .filter_map(|resource| {
                self.fuel_resource_profile(resource)
                    .map(|(plant, power, co2, priority)| (resource, plant, power, co2, priority))
            })
            .collect();
        let mut emitted = 0.0;
        let mut total_consumed = 0.0;

        for city_id in city_ids {
            let city = &self.cities[&city_id];
            let demand = self.city_power_demand(city);
            let mut supply = self.city_renewable_power(city);
            let project_fully_powers = self.city_project_fully_powers(city);
            if project_fully_powers {
                self.players[pid]
                    .city_power
                    .insert(city_id, supply.max(demand));
                continue;
            }
            let mut remaining = (demand - supply).max(0.0);
            let mut candidates: Vec<(&str, f64, f64, i32)> = fuel_specs
                .iter()
                .filter_map(|(resource, plant, power, co2, advanced)| {
                    let bonus = self
                        .cities
                        .values()
                        .filter_map(|source| {
                            if source.owner != pid
                                || !self.city_has_active_building_family(source, Name::new(plant))
                            {
                                return None;
                            }
                            self.city_active_district_family_position(
                                source,
                                crate::name!("industrial_zone"),
                            )
                            .filter(|position| self.wdist(*position, city.pos) <= 6)
                            .map(|_| {
                                self.governor_effect(pid, source.id, "power_per_resource_bonus")
                            })
                        })
                        .fold(None::<f64>, |best, candidate| {
                            Some(best.map_or(candidate, |current| current.max(candidate)))
                        })?;
                    Some((*resource, *power + bonus, *co2, *advanced))
                })
                .collect();
            candidates.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap()
                    .then_with(|| {
                        self.strategic_stockpile(pid, Name::new(b.0))
                            .partial_cmp(&self.strategic_stockpile(pid, Name::new(a.0)))
                            .unwrap()
                    })
                    .then_with(|| {
                        self.strategic_resource_rate(pid, b.0)
                            .partial_cmp(&self.strategic_resource_rate(pid, a.0))
                            .unwrap()
                    })
                    .then_with(|| b.3.cmp(&a.3))
            });

            for (resource, power_per_unit, co2_per_power, _) in candidates {
                if remaining <= f64::EPSILON {
                    break;
                }
                let stock = self.strategic_stockpile(pid, Name::new(resource));
                let available = stock.floor();
                if available <= 0.0 {
                    continue;
                }
                let consumed = (remaining / power_per_unit).ceil().min(available);
                if consumed <= 0.0 {
                    continue;
                }
                self.players[pid]
                    .strategic_resources
                    .insert(Name::new(resource), stock - consumed);
                *self.players[pid]
                    .power_fuel_consumed
                    .entry(resource.to_string())
                    .or_insert(0.0) += consumed;
                total_consumed += consumed;
                let generated = consumed * power_per_unit;
                emitted += generated * co2_per_power;
                supply += generated;
                remaining = (remaining - generated).max(0.0);
            }
            self.players[pid].city_power.insert(city_id, supply);
        }

        self.players[pid].co2_emissions += emitted;
        let first_for_civ = self.players[pid]
            .counters
            .get("first_resource_power")
            .copied()
            .unwrap_or(0)
            == 0;
        if total_consumed > 0.0 && first_for_civ {
            let first_in_world = !self.players.iter().any(|player| {
                player
                    .counters
                    .get("first_resource_power")
                    .copied()
                    .unwrap_or(0)
                    > 0
            });
            self.players[pid]
                .counters
                .insert("first_resource_power".to_string(), 1);
            self.add_historic_moment(
                pid,
                if first_in_world {
                    "MOMENT_CITY_POWER_GENERATED_FROM_RESOURCE_FIRST_IN_WORLD"
                } else {
                    "MOMENT_CITY_POWER_GENERATED_FROM_RESOURCE_FIRST"
                },
            );
        }
    }

    pub(super) fn add_powered_building_yields(
        spec: &crate::rules::BuildingSpec,
        yields: &mut Yields,
    ) {
        yields.food += spec.effects.get("powered_food").copied().unwrap_or(0.0);
        yields.production += spec
            .effects
            .get("powered_production")
            .copied()
            .unwrap_or(0.0);
        yields.gold += spec.effects.get("powered_gold").copied().unwrap_or(0.0);
        yields.science += spec.effects.get("powered_science").copied().unwrap_or(0.0);
        yields.culture += spec.effects.get("powered_culture").copied().unwrap_or(0.0);
        yields.faith += spec.effects.get("powered_faith").copied().unwrap_or(0.0);
    }
}
