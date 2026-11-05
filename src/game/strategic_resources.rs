//! Strategic resources: which tiles supply them, what an empire earns each
//! turn, and its stockpiles.
//!
//! Whether an improvement or a tile connects a resource
//! (`improvement_connects_resource`, `tile_connects_resource`); the
//! per-turn income, as the host reports it and as the engine models it
//! (`strategic_resource_rate`, `modeled_strategic_resource_rate`); the
//! stockpile and its capacity; and the turn's
//! `process_strategic_resources`, which banks income and burns the fuel
//! that power plants consume. The fuel profile itself is shared with
//! `game/power.rs` and stays in `game.rs`.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Whether an improvement is one of the ruleset-defined ways to connect
    /// the resource beneath it. Water alternates such as Offshore Oil Rigs on
    /// Oil and Fishing Boats on Amber live in `ImprovementSpec.resources`,
    /// while Industries and Corporations inherit their resource from the tile.
    #[inline]
    pub(super) fn improvement_connects_resource(&self, improvement: Name, resource: Name) -> bool {
        let Some(resource_spec) = self.rules.resources.get_interned(resource) else {
            return false;
        };
        resource_spec.improvement == improvement.as_str()
            || self
                .rules
                .improvements
                .get_interned(improvement)
                .is_some_and(|spec| spec.resources.contains(&resource))
            || matches!(improvement.as_str(), "industry" | "corporation")
    }

    /// A resource connection is live only while the tile is unpillaged. City
    /// Centers connect their resource directly; every other tile must have a
    /// matching improvement under the shared ruleset predicate above.
    #[inline]
    pub(super) fn tile_connects_resource(
        &self,
        tile: &Tile,
        resource: Name,
        city_center: bool,
    ) -> bool {
        tile.resource == Some(resource)
            && !tile.pillaged
            && self.rules.resources.contains_name(resource)
            && (city_center
                || tile.improvement.is_some_and(|improvement| {
                    self.improvement_connects_resource(improvement, resource)
                }))
    }

    pub fn strategic_resource_rate(&self, pid: usize, res: &str) -> f64 {
        let modeled = self.modeled_strategic_resource_rate(pid, res);
        let adjustment = self
            .observed_strategic_income_adjustments
            .get(&pid)
            .and_then(|rates| rates.get(&Name::new(res)))
            .copied();
        adjustment.map_or(modeled, |delta| (modeled + delta).max(0.0))
    }

    pub(super) fn modeled_strategic_resource_rate(&self, pid: usize, res: &str) -> f64 {
        if !self.resource_visible_to(pid, res) {
            return 0.0;
        }
        let resource = Name::new(res);
        let mut improved = 0.0;
        let mut government_improved = 0.0;
        let mut governor_accumulation = 0.0;
        for c in self.cities.values().filter(|c| c.owner == pid) {
            for pos in &c.owned_tiles {
                let tile = &self.map.tiles[pos];
                if self.tile_connects_resource(tile, resource, *pos == c.pos) {
                    improved += 1.0;
                    if tile.improvement.is_some_and(|improvement| {
                        self.improvement_connects_resource(improvement, resource)
                    }) {
                        government_improved += 1.0;
                    }
                    governor_accumulation +=
                        self.governor_effect(pid, c.id, "strategic_resource_accumulation");
                }
            }
        }
        let extra = match res {
            "horses" | "iron" => self.policy_effect(pid, "early_strategic_resource_rate"),
            "niter" | "coal" => self.policy_effect(pid, "industrial_strategic_resource_rate"),
            "oil" | "aluminum" => self.policy_effect(pid, "modern_strategic_resource_rate"),
            _ => 0.0,
        };
        let spaceport = if res == "aluminum"
            && self.cities.values().any(|city| {
                city.owner == pid && self.city_has_district_family(city, crate::name!("spaceport"))
            }) {
            self.policy_effect(pid, "spaceport_aluminum")
        } else {
            0.0
        };
        let wonder = if res == "iron" {
            self.empire_wonder_effect(pid, "iron_per_turn")
        } else {
            0.0
        };
        let building = self
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .filter(|city| {
                city.owned_tiles.iter().any(|position| {
                    let tile = &self.map.tiles[position];
                    self.tile_connects_resource(tile, resource, *position == city.pos)
                })
            })
            .map(|city| self.city_building_effect(city, "strategic_resource_accumulation"))
            .sum::<f64>();
        // Gathering Storm sources do not all accumulate at one unit per turn:
        // Horses/Iron/Niter/Aluminum produce 2, while Coal/Oil/Uranium produce 3.
        // The three resource policies add one to each improved source.
        let base = match res {
            "horses" | "iron" | "niter" | "aluminum" => 2.0,
            "coal" | "oil" | "uranium" => 3.0,
            _ => 0.0,
        };
        let suzerain_city_states = self
            .players
            .iter()
            .filter(|player| {
                player.alive
                    && player.is_minor
                    && !player.is_barbarian
                    && self.suzerain_of(player.id) == Some(pid)
            })
            .map(|minor| self.connected_resource_count_unchecked(minor.id, res) as f64 * base)
            .sum::<f64>();
        let amani = self
            .amani_city_state_for_effect(pid, "city_state_resource_access")
            .map(|(_, minor)| {
                let sources = self
                    .cities
                    .values()
                    .filter(|city| city.owner == minor)
                    .flat_map(|city| {
                        city.owned_tiles
                            .iter()
                            .map(move |position| (city, position))
                    })
                    .filter(|(city, position)| {
                        let tile = &self.map.tiles[position];
                        self.tile_connects_resource(tile, resource, **position == city.pos)
                    })
                    .count() as f64;
                // Foreign Investor supplies the assigned state's resources
                // without Suzerainty. If Amani also makes us Suzerain, this
                // becomes the second copy; the ordinary Suzerain copy is
                // accounted for above.
                sources * base
            })
            .unwrap_or(0.0);
        let hattusa = if self.grants_city_state_unique_bonus(pid, "Hattusa")
            && self.connected_resource_count_unchecked(pid, res) == 0
        {
            2.0
        } else {
            0.0
        };
        let great_person = if res == "oil" {
            self.players[pid]
                .counters
                .get("great_person:oil_per_turn")
                .copied()
                .unwrap_or(0) as f64
        } else {
            0.0
        };
        improved * (base + extra)
            + government_improved * self.gov_effects(pid).improved_strategic_resource_rate
            + governor_accumulation
            + spaceport
            + wonder
            + building
            + suzerain_city_states
            + amani
            + hattusa
            + great_person
    }

    pub fn strategic_stockpile_capacity(&self, pid: usize) -> f64 {
        50.0 + self.empire_building_sum(pid, |building| {
            building
                .effects
                .get("strategic_stockpile")
                .copied()
                .unwrap_or(0.0)
        })
    }

    pub fn strategic_stockpile(&self, pid: usize, resource: impl AsName) -> f64 {
        self.players[pid]
            .strategic_resources
            .get(&resource.as_name())
            .copied()
            .unwrap_or(0.0)
    }

    /// Accumulate improved strategic resources, then pay fuel maintenance.
    /// Every unit using an under-supplied fuel receives the same -1 Strength
    /// per unpaid unit, capped at -20, matching Gathering Storm's shortage rule.
    pub(super) fn process_strategic_resources(&mut self, pid: usize) {
        if self.players[pid].is_barbarian {
            self.players[pid].strategic_resource_shortages.clear();
            return;
        }
        let resources: Vec<Name> = self
            .rules
            .resources
            .iter()
            .filter(|(_, spec)| spec.class == "strategic")
            .map(|(resource, _)| *resource)
            .collect();
        let capacity = self.strategic_stockpile_capacity(pid);
        let updates: Vec<(Name, f64, f64)> = resources
            .iter()
            .map(|resource| {
                let accumulated = (self.strategic_stockpile(pid, resource)
                    + self.strategic_resource_rate(pid, resource))
                .min(capacity);
                let demand = self
                    .units
                    .values()
                    .filter(|unit| unit.owner == pid && !unit.free_upkeep)
                    .filter_map(|unit| {
                        let spec = &self.rules.units[unit.kind];
                        (spec.requires_resource.as_deref() == Some(resource.as_str()))
                            .then_some(spec.resource_maintenance)
                    })
                    .sum::<f64>();
                (*resource, accumulated, demand)
            })
            .collect();
        let unit_co2_mult =
            (1.0 - self.tree_effect(pid, "unit_co2_reduction_pct") / 100.0).max(0.0);
        let fuel_emissions_per_resource: BTreeMap<Name, f64> = resources
            .iter()
            .filter_map(|resource| {
                self.fuel_resource_profile(resource)
                    .map(|(_, power, co2, _)| (*resource, power * co2))
            })
            .collect();
        let player = &mut self.players[pid];
        let mut maintenance_emissions = 0.0;
        player.strategic_resource_shortages.clear();
        for (resource, accumulated, demand) in updates {
            let paid = accumulated.min(demand);
            let plant_emissions_per_resource = fuel_emissions_per_resource
                .get(&resource)
                .copied()
                .unwrap_or(0.0);
            maintenance_emissions += paid * plant_emissions_per_resource * 0.5 * unit_co2_mult;
            player
                .strategic_resources
                .insert(resource, accumulated - paid);
            let unpaid = (demand - paid).ceil() as i32;
            if unpaid > 0 {
                player
                    .strategic_resource_shortages
                    .insert(resource, unpaid.min(20));
            }
        }
        player.co2_emissions += maintenance_emissions;
        self.process_power(pid);
    }
}
