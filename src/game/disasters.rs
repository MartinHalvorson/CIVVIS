//! Disasters: damage to tiles and units from natural disasters and storms.
//!
//! `strike_disaster_tile`, `strike_storm`, `damage_disaster_tile`,
//! `damage_disaster_units` and `damage_tile_area`.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub(super) fn damage_disaster_tile(&mut self, position: Pos, unit_damage: i32) {
        self.damage_tile_area(position, unit_damage, None);
    }

    /// Hurt every unit standing on a tile without touching the tile itself. A
    /// storm that spares a city's infrastructure still catches whoever is out
    /// in it.
    fn damage_disaster_units(&mut self, position: Pos, unit_damage: i32) {
        for unit_id in self.units_at(position) {
            let hp = self.units[&unit_id].hp - unit_damage;
            if hp <= 0 {
                self.remove_unit(unit_id);
            } else {
                self.units.get_mut(&unit_id).unwrap().hp = hp;
            }
        }
    }

    /// Blast damage over one tile. `culprit` is the civilization that caused
    /// it where somebody did — a nuclear strike kills through the same path a
    /// volcano does, and those casualties belong in its war's ledger.
    pub(super) fn damage_tile_area(
        &mut self,
        position: Pos,
        unit_damage: i32,
        culprit: Option<usize>,
    ) {
        let Some(tile) = self.map.get(position) else {
            return;
        };
        let owner_city = tile.owner_city;
        let district = tile.district;
        let damageable = tile.improvement.is_some() || district.is_some();
        if damageable {
            self.map.tiles.get_mut(&position).unwrap().pillaged = true;
        }
        if let (Some(city_id), Some(district)) = (owner_city, district) {
            let buildings: Vec<Name> = self.cities[&city_id]
                .buildings
                .iter()
                .filter(|building| {
                    let spec = &self.rules.buildings[building];
                    spec.district
                        .is_some_and(|family| self.district_is_family(district, family))
                        && spec
                            .effects
                            .get("immune_to_disasters")
                            .copied()
                            .unwrap_or(0.0)
                            <= 0.0
                })
                .cloned()
                .collect();
            self.cities
                .get_mut(&city_id)
                .unwrap()
                .pillaged_buildings
                .extend(buildings);
        }
        let units = self.units_at(position);
        for unit_id in units {
            // Removing an Aircraft Carrier removes the aircraft it carries,
            // which stand on this tile and are later in `units`: one lethal
            // strike on a loaded carrier used to index a unit it had already
            // removed and panic ("unit N is not present").
            let Some(unit) = self.units.get(&unit_id) else {
                continue;
            };
            let hp = unit.hp - unit_damage;
            if hp <= 0 {
                let (owner, kind, formation) = {
                    let unit = &self.units[&unit_id];
                    (unit.owner, unit.kind, unit.formation)
                };
                if let Some(killer) = culprit {
                    self.record_war_unit_loss(killer, owner, &kind, formation, position);
                }
                self.remove_unit(unit_id);
            } else {
                self.units.get_mut(&unit_id).unwrap().hp = hp;
            }
        }
        if damageable {
            self.scatter_aircraft_from(position);
        }
    }

    /// Apply one disaster class to one tile: damage whoever is standing there,
    /// maybe pillage what is built there, and maybe leave the ground better
    /// than it was.
    ///
    /// Returns whether the ground came out of it richer, which is what decides
    /// an eruption's Volcanic Soil: `RandomEvent_Yields` carries
    /// `ReplaceFeature="true"` on the volcano's own yield rows, so the ash
    /// settles exactly where the fertility did.
    pub(super) fn strike_disaster_tile(
        &mut self,
        position: Pos,
        spec: &DisasterSpec,
        severity: u8,
        protection: Option<&str>,
        rng: &mut Rng,
    ) -> bool {
        if self.disaster_immune(position, protection) {
            return false;
        }
        let damage = spec.unit_damage.round() as i32;
        if rng.chance(spec.pillage_chance(severity)) {
            self.damage_disaster_tile(position, damage);
        } else if damage > 0 {
            self.damage_disaster_units(position, damage);
        }
        // ⚠ TWO ROLLS, NOT ONE. `RandomEvent_Yields` rates each yield type
        // apart — a Catastrophic eruption leaves Food half the time and
        // Production a quarter of the time, independently — and this used to
        // roll once and grant Food only.
        let mut fertilized = rng.chance(spec.fertility_chance(severity));
        if fertilized {
            self.fertilize_tile(position, Fertility::Food);
        }
        if rng.chance(spec.fertility_production_chance(severity)) {
            self.fertilize_tile(position, Fertility::Production);
            fertilized = true;
        }
        fertilized
    }

    /// Permanent fertility left behind by a disaster. Land keeps what a storm's
    /// silt leaves; the sea keeps nothing.
    fn fertilize_tile(&mut self, position: Pos, kind: Fertility) {
        if self
            .map
            .get(position)
            .is_none_or(|tile| self.rules.is_water(tile))
        {
            return;
        }
        let tile = self.map.tiles.get_mut(&position).unwrap();
        let held = match kind {
            Fertility::Food => &mut tile.disaster_food,
            Fertility::Production => &mut tile.disaster_production,
        };
        if *held >= DISASTER_FERTILITY_CAP {
            return;
        }
        *held += 1.0;
    }

    /// Damage everything under a storm's footprint and leave the marker the
    /// clients draw.
    pub(super) fn strike_storm(&mut self, storm: &Storm, spec: &DisasterSpec) {
        let mut rng =
            self.disaster_rng(&storm.kind, storm.pos.0 as u64 ^ (storm.pos.1 as u64) << 8);
        for position in self.wdisk(storm.pos, spec.radius(storm.severity)) {
            self.map.tiles.get_mut(&position).unwrap().storm = Some(storm.kind.clone());
            self.strike_disaster_tile(position, spec, storm.severity, None, &mut rng);
            self.disaster_population_loss(position, spec.population_loss(storm.severity));
        }
    }
}
