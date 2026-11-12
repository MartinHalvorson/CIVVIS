//! Disasters and climate: natural disasters, the damage they do, climate
//! change, and reactor accidents.
//!
//! Damage: `strike_disaster_tile`, `strike_storm`, `damage_disaster_tile`,
//! `damage_disaster_units` and `damage_tile_area`.
//!
//! Disasters as events: their intensity, rate and seeded rolls
//! (`disaster_rate`, `disaster_rng`, `roll_severity`), which tiles and
//! cities are immune or protected, eruptions, river floods, droughts and
//! storms (`trigger_*`, `resolve_*`, `advance_storms`), the population a
//! disaster takes, and the turn's `process_disasters`.
//!
//! Climate: CO2 emissions and climate points, the climate phase
//! (`apply_climate_phase`, the live mirror's `mirror_set_climate_phase`),
//! coastal lowlands, barriers, flooding and submerging them, and the
//! turn's `process_climate`. Reactor accidents: the risk, an accident, and
//! the turn's `process_reactors`.
//!
//! The damage methods moved first; the rest moved verbatim out of the main
//! `impl Game` block in `game.rs` later the same day.
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
                    self.tally_combat_removal(killer, owner, kind);
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

    pub(super) fn city_disaster_protected(&self, city: &City, district_effect: &str) -> bool {
        self.governor_effect(city.owner, city.id, "disaster_immunity") > 0.0
            || self.city_district_effect(city, district_effect) > 0.0
    }

    /// Resolve a flood over the supplied river floodplain tiles. Dams and
    /// Great Bath prevent damage; every Great Bath-mitigated flood permanently
    /// adds its data-defined Faith to each affected floodplain tile.
    pub fn resolve_flood(&mut self, positions: &[Pos]) {
        let mut protected_by_infrastructure = BTreeSet::new();
        for position in positions.iter().copied() {
            let Some(tile) = self.map.get(position) else {
                continue;
            };
            if !matches!(
                tile.feature.as_deref(),
                Some("floodplains" | "grassland_floodplains" | "plains_floodplains")
            ) {
                continue;
            }
            let Some(city_id) = tile.owner_city else {
                self.damage_disaster_tile(position, 20);
                continue;
            };
            let city = &self.cities[&city_id];
            let great_bath = self.city_wonder_effect(city, "flood_immunity") > 0.0;
            let district_protection = self.city_district_effect(city, "flood_protection") > 0.0;
            let protected = great_bath
                || district_protection
                || self.governor_effect(city.owner, city.id, "disaster_immunity") > 0.0;
            if protected {
                if great_bath || district_protection {
                    protected_by_infrastructure.insert(city.owner);
                }
                if great_bath {
                    let faith = self.city_wonder_effect(city, "mitigated_flood_faith");
                    self.map.tiles.get_mut(&position).unwrap().disaster_faith += faith;
                }
            } else {
                self.damage_disaster_tile(position, 20);
            }
        }
        for owner in protected_by_infrastructure {
            self.add_historic_moment(owner, "MOMENT_MITIGATED_RIVER_FLOOD");
        }
    }

    /// Start drought effects on the supplied tiles. Aqueducts, Baths, and Dams
    /// prevent the -1 Food loss, but Civ VI still allows drought to pillage
    /// vulnerable improvements; Liang's Reinforced Materials prevents that
    /// environmental pillaging as well.
    pub fn resolve_drought(&mut self, positions: &[Pos]) {
        for position in positions.iter().copied() {
            let immune = self
                .map
                .get(position)
                .and_then(|tile| tile.owner_city)
                .and_then(|city_id| self.cities.get(&city_id))
                .is_some_and(|city| {
                    self.governor_effect(city.owner, city.id, "disaster_immunity") > 0.0
                });
            let Some(tile) = self.map.tiles.get_mut(&position) else {
                continue;
            };
            tile.drought = true;
            if !immune
                && matches!(
                    tile.improvement.as_deref(),
                    Some("farm" | "camp" | "plantation" | "pasture")
                )
            {
                tile.pillaged = true;
            }
        }
    }

    /// End a drought and restore its temporary Food modifier. Pillaged
    /// improvements remain pillaged until repaired.
    pub fn clear_drought(&mut self, positions: &[Pos]) {
        for position in positions {
            if let Some(tile) = self.map.tiles.get_mut(position) {
                tile.drought = false;
            }
        }
    }

    /// The disaster-intensity setting, clamped to the five Gathering Storm
    /// values. Zero leaves the map's volcanoes dormant and stops every random
    /// disaster; sea-level rise still follows CO2, because that is climate
    /// change rather than a random event.
    pub fn disaster_intensity(&self) -> u8 {
        self.disaster_intensity.min(4)
    }

    /// Whether this class of random disaster may strike a world of this
    /// script. A battlefield is decided by the fighting, so the arena runs no
    /// random disasters at all — a tornado deciding a posed engagement is
    /// noise, not history — except the one class a named battle is actually
    /// remembered for: the thirst at Hattin, the storms that broke the
    /// Armada, the winter at Stalingrad. Each named battle's allowance, and
    /// the near-misses deliberately left out, are recorded on
    /// [`crate::historical_scenarios::HistoricalScenario::disasters`]. A
    /// generic arena — Battlefield, the Tactics globes — allows none.
    pub(crate) fn script_disaster_allowed(script: MapScript, class: &str) -> bool {
        if !script.is_battlefield() {
            return true;
        }
        crate::historical_scenarios::by_script(script)
            .is_some_and(|scenario| scenario.disasters.contains(&class))
    }

    /// Expected occurrences of one disaster class over a whole game, after the
    /// intensity setting and the warming already banked. Gathering Storm makes
    /// a hotter world a stormier one, so every phase adds to the rate.
    pub(super) fn disaster_rate(&self, class: &str) -> f64 {
        if !Self::script_disaster_allowed(self.map_script, class) {
            return 0.0;
        }
        let Some(spec) = self.rules.disasters.get(class) else {
            return 0.0;
        };
        let intensity = DISASTER_INTENSITY_SCALE[self.disaster_intensity() as usize];
        let climate = 1.0 + CLIMATE_DISASTER_SCALE * f64::from(self.climate_phase);
        spec.per_game * intensity * climate
    }

    /// Disasters draw from a turn-keyed stream of their own so that adding or
    /// removing one never shifts the main RNG sequence — the same discipline
    /// the meteor shower uses.
    pub(super) fn disaster_rng(&self, class: &str, salt: u64) -> Rng {
        let mut key = 0xD15A_5732_0000_0000u64;
        for byte in class.as_bytes() {
            key = key.rotate_left(7) ^ u64::from(*byte);
        }
        Rng::new(self.seed ^ key ^ ((self.turn as u64) << 17) ^ salt)
    }

    /// Severity is weighted toward the mild end, and each climate phase moves
    /// weight toward the severe one.
    fn roll_severity(&self, spec: &DisasterSpec, rng: &mut Rng) -> u8 {
        let tiers = spec.severities.max(1);
        if tiers == 1 {
            return 1;
        }
        let bias = CLIMATE_SEVERITY_BIAS * f64::from(self.climate_phase);
        let weights: Vec<f64> = (0..tiers)
            .map(|tier| f64::from(tiers - tier) * (1.0 + bias * f64::from(tier)))
            .collect();
        let total: f64 = weights.iter().sum();
        let mut roll = rng.f64() * total;
        for (tier, weight) in weights.iter().enumerate() {
            roll -= weight;
            if roll <= 0.0 {
                return tier as u8 + 1;
            }
        }
        tiers
    }

    /// Whether this volcano is one of the map's active cones. Gathering Storm
    /// activates between 45% and 95% of them depending on disaster intensity;
    /// the answer is derived from the seed and the tile so it survives a save
    /// without being stored.
    ///
    /// The test is the shipped `Features_XP2.Volcano` flag, so the three
    /// volcanic Natural Wonders stand in the lottery beside the generic cone.
    pub fn volcano_active(&self, position: Pos) -> bool {
        if !self
            .map
            .get(position)
            .is_some_and(|tile| self.rules.is_volcano(tile))
        {
            return false;
        }
        let share = ACTIVE_VOLCANO_SHARE[self.disaster_intensity() as usize];
        let key = (u64::from(position.0 as u32) << 32) | u64::from(position.1 as u32);
        Rng::new(self.seed ^ 0x564F_4C43_414E_4F00 ^ key).f64() < share
    }

    /// Whether a tile's city, if it has one, shrugs a disaster off. Governors'
    /// Reinforced Materials is the blanket immunity; the district families are
    /// each disaster's own protection.
    pub(super) fn disaster_immune(&self, position: Pos, district_effect: Option<&str>) -> bool {
        let Some(city) = self
            .map
            .get(position)
            .and_then(|tile| tile.owner_city)
            .and_then(|city_id| self.cities.get(&city_id))
        else {
            return false;
        };
        if self.governor_effect(city.owner, city.id, "disaster_immunity") > 0.0 {
            return true;
        }
        district_effect.is_some_and(|effect| self.city_district_effect(city, effect) > 0.0)
    }

    /// Kill citizens in a city a disaster reached, never below one.
    pub(super) fn disaster_population_loss(&mut self, position: Pos, loss: i32) {
        if loss <= 0 {
            return;
        }
        let Some(city_id) = self.city_at(position) else {
            return;
        };
        if self.governor_effect(self.cities[&city_id].owner, city_id, "disaster_immunity") > 0.0 {
            return;
        }
        let owner = self.cities[&city_id].owner;
        let city = self.cities.get_mut(&city_id).unwrap();
        let before = city.pop;
        city.pop = (city.pop - loss).max(1);
        if city.pop < before {
            self.open_native_aid_request(
                owner,
                NativeCompetitionTrigger::RandomDisasterPopulationLoss,
            );
        }
    }

    /// Tell whoever has something at stake on the tile what just happened to
    /// it: the civilization that owns it, and anyone with a unit standing
    /// there when it hit.
    fn note_disaster(&mut self, position: Pos, text: String) {
        let mut told: BTreeSet<usize> = BTreeSet::new();
        let owner = self
            .map
            .get(position)
            .and_then(|tile| tile.owner_city)
            .and_then(|city_id| self.cities.get(&city_id))
            .map(|city| city.owner);
        told.extend(owner);
        for unit_id in self.unit_ids_at(position) {
            told.insert(self.units[unit_id].owner);
        }
        for pid in told {
            self.note(pid, "World", text.clone(), Some(position));
        }
    }

    /// A volcano erupts. Everything inside the eruption radius is damaged, the
    /// features there are burned away, and the surviving land is fertilised
    /// into Volcanic Soil — which is why people keep farming next to volcanoes.
    pub fn resolve_eruption(&mut self, volcano: Pos, severity: u8) {
        let Some(spec) = self.rules.disasters.get("volcanic_eruption").cloned() else {
            return;
        };
        let severity = severity.max(1);
        // The shipped eruption reaches one ring, or two on the top two
        // disaster-intensity settings.
        let radius = if self.disaster_intensity() >= 3 {
            spec.radius(severity).max(2)
        } else {
            spec.radius(severity)
        };
        let mut rng = self.disaster_rng("volcanic_eruption", 0x0045_5255_5054);
        for position in self.wdisk(volcano, radius) {
            if position == volcano {
                continue;
            }
            let Some(tile) = self.map.get(position) else {
                continue;
            };
            if self.rules.is_water(tile) || self.rules.is_volcano(tile) {
                continue;
            }
            let natural_wonder = tile
                .feature
                .as_deref()
                .is_some_and(|feature| self.rules.features[feature].natural_wonder);
            if natural_wonder {
                continue;
            }
            // `Features_XP2.ValidForReplacement` is the exact list of ordinary
            // ground cover an eruption may bury. Fissures and other permanent
            // features still suffer the blast, but remain what they were.
            let replaceable_feature = tile.feature.as_deref().is_none_or(|feature| {
                matches!(
                    feature,
                    "forest"
                        | "jungle"
                        | "marsh"
                        | "floodplains"
                        | "grassland_floodplains"
                        | "plains_floodplains"
                        | "oasis"
                        | "volcanic_soil"
                        | "burning_forest"
                        | "burnt_forest"
                        | "burning_jungle"
                        | "burnt_jungle"
                )
            });
            let can_leave_soil = volcanic_soil_valid_terrain(tile) && replaceable_feature;
            let immune = self.disaster_immune(position, None);
            let fertilized = self.strike_disaster_tile(position, &spec, severity, None, &mut rng);
            self.disaster_population_loss(position, spec.population_loss(severity));
            if immune {
                continue;
            }
            // Ash buries whatever was growing and leaves Volcanic Soil — on the
            // plots the eruption actually enriched, because the shipped
            // `ReplaceFeature="true"` rides on those same yield rows. This used
            // to roll the fertility chance a SECOND time, so a tile could take
            // the Food and keep its Forest, or lose its Forest and take nothing.
            if can_leave_soil && fertilized {
                let tile = self.map.tiles.get_mut(&position).unwrap();
                // Volcanic Soil ships both ValidWonderPlacement and
                // ValidDistrictPlacement, so an eruption may deposit it under
                // either kind of built site.
                tile.feature = Some(crate::name!("volcanic_soil"));
            }
        }
        self.note_disaster(volcano, "a volcano erupted".to_string());
    }

    /// Pick an active volcano and erupt it.
    pub(super) fn trigger_eruption(&mut self, severity: u8, rng: &mut Rng) {
        let volcanoes: Vec<Pos> = self
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| self.rules.is_volcano(tile))
            .map(|(position, _)| *position)
            .filter(|position| self.volcano_active(*position))
            .collect();
        if volcanoes.is_empty() {
            return;
        }
        let volcano = volcanoes[rng.below(volcanoes.len())];
        self.resolve_eruption(volcano, severity);
    }

    /// A river bursts its banks. Gathering Storm floods the Floodplains along
    /// one stretch of river; Dams, the Great Bath and Egypt's Iteru hold.
    fn trigger_river_flood(&mut self, severity: u8, rng: &mut Rng) {
        let Some(spec) = self.rules.disasters.get("river_flood").cloned() else {
            return;
        };
        let sources: Vec<Pos> = self
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| {
                matches!(
                    tile.feature.as_deref(),
                    Some("floodplains" | "grassland_floodplains" | "plains_floodplains")
                )
            })
            .map(|(position, _)| *position)
            .collect();
        if sources.is_empty() {
            return;
        }
        let source = sources[rng.below(sources.len())];
        let reach: Vec<Pos> = self
            .wdisk(source, spec.radius(severity))
            .into_iter()
            .filter(|position| {
                matches!(
                    self.map.tiles[position].feature.as_deref(),
                    Some("floodplains" | "grassland_floodplains" | "plains_floodplains")
                )
            })
            .collect();
        self.resolve_flood(&reach);
        for position in &reach {
            self.disaster_population_loss(*position, spec.population_loss(severity));
        }
        self.note_disaster(source, "a river flooded".to_string());
    }

    /// A drought settles over featureless land and stays for several turns.
    fn trigger_drought(&mut self, severity: u8, rng: &mut Rng) {
        let Some(spec) = self.rules.disasters.get("drought").cloned() else {
            return;
        };
        // The shipped drought targets open ground: terrain without a feature.
        let sources: Vec<Pos> = self
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| {
                !self.rules.is_water(tile) && tile.feature.is_none() && !tile.drought
            })
            .map(|(position, _)| *position)
            .collect();
        if sources.is_empty() {
            return;
        }
        let source = sources[rng.below(sources.len())];
        let tiles: Vec<Pos> = self
            .wdisk(source, spec.radius(severity))
            .into_iter()
            .filter(|position| {
                let tile = &self.map.tiles[position];
                !self.rules.is_water(tile) && tile.feature.is_none()
            })
            .collect();
        if tiles.is_empty() {
            return;
        }
        self.resolve_drought(&tiles);
        self.droughts.push(Drought {
            tiles,
            severity,
            ends: self.turn + spec.duration(severity).max(1),
        });
        self.note_disaster(source, "a drought set in".to_string());
    }

    /// Lift every drought whose turn has come and give the land its Food back.
    fn lift_expired_droughts(&mut self) {
        let expired: Vec<Drought> = self
            .droughts
            .iter()
            .filter(|drought| drought.ends <= self.turn)
            .cloned()
            .collect();
        self.droughts.retain(|drought| drought.ends > self.turn);
        for drought in expired {
            self.clear_drought(&drought.tiles);
            if let Some(position) = drought.tiles.first().copied() {
                self.note_disaster(position, "a drought broke".to_string());
            }
        }
    }

    /// Spawn one of the four terrain-bound storm systems.
    fn trigger_storm(&mut self, class: &str, severity: u8, rng: &mut Rng) {
        let Some(spec) = self.rules.disasters.get(class).cloned() else {
            return;
        };
        let sources: Vec<Pos> = self
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| spec.terrains.contains(&tile.terrain))
            .map(|(position, _)| *position)
            .collect();
        if sources.is_empty() {
            return;
        }
        let storm = Storm {
            kind: class.to_string(),
            pos: sources[rng.below(sources.len())],
            heading: rng.below(6),
            severity,
            ends: self.turn + spec.duration(severity).max(1),
        };
        self.note_disaster(storm.pos, format!("{} formed", pretty(class)));
        self.strike_storm(&storm, &spec);
        self.storms.push(storm);
    }

    /// Move every live storm one tile along its heading and dissipate the ones
    /// whose three turns are up.
    fn advance_storms(&mut self) {
        // Only last turn's footprints need clearing; sweeping the whole map to
        // erase a handful of markers is work proportional to the wrong thing.
        let footprints: Vec<(Pos, i32)> = self
            .storms
            .iter()
            .map(|storm| {
                let radius = self
                    .rules
                    .disasters
                    .get(&storm.kind)
                    .map_or(1, |spec| spec.radius(storm.severity));
                (storm.pos, radius)
            })
            .collect();
        for (center, radius) in footprints {
            for position in self.wdisk(center, radius) {
                self.map.tiles.get_mut(&position).unwrap().storm = None;
            }
        }
        let mut storms = std::mem::take(&mut self.storms);
        storms.retain(|storm| storm.ends > self.turn);
        for storm in &mut storms {
            let Some(spec) = self.rules.disasters.get(&storm.kind).cloned() else {
                continue;
            };
            if let Some(next) = self.map.step(storm.pos, storm.heading) {
                storm.pos = next;
            }
            let moved = storm.clone();
            self.strike_storm(&moved, &spec);
        }
        self.storms = storms;
    }

    /// One turn of Gathering Storm's random disasters: drift the storms
    /// already on the map, lift the droughts whose time is up, then roll each
    /// class for a new event.
    pub(super) fn process_disasters(&mut self) {
        self.advance_storms();
        self.lift_expired_droughts();
        if self.disaster_intensity() == 0 || self.max_turns == 0 {
            return;
        }
        let classes: Vec<Name> = self.rules.disasters.iter().map(|(name, _)| *name).collect();
        for class in classes {
            let chance = self.disaster_rate(&class) / f64::from(self.max_turns);
            let mut rng = self.disaster_rng(&class, 0);
            if !rng.chance(chance) {
                continue;
            }
            let Some(spec) = self.rules.disasters.get(&class).cloned() else {
                continue;
            };
            let severity = self.roll_severity(&spec, &mut rng);
            match class.as_str() {
                "volcanic_eruption" => self.trigger_eruption(severity, &mut rng),
                "river_flood" => self.trigger_river_flood(severity, &mut rng),
                "drought" => self.trigger_drought(severity, &mut rng),
                _ => self.trigger_storm(&class, severity, &mut rng),
            }
        }
    }

    pub(super) fn coastal_lowland_tiles(&self, city: &City) -> Vec<Pos> {
        city.owned_tiles
            .iter()
            .copied()
            .filter(|position| {
                let tile = &self.map.tiles[position];
                tile.coastal_lowland > 0 && !tile.submerged
            })
            .collect()
    }

    /// Apply one coastal-flood stage to every owned coastal lowland. An
    /// active Flood Barrier protects all such tiles in its city.
    pub fn resolve_coastal_flooding(&mut self) {
        let city_ids: Vec<u32> = self.cities.keys().copied().collect();
        let mut protected_by_barrier = BTreeSet::new();
        for city_id in city_ids {
            let lowlands = self.coastal_lowland_tiles(&self.cities[&city_id]);
            let barrier =
                self.city_building_effect(&self.cities[&city_id], "protect_coastal_lowlands") > 0.0;
            let protected = barrier
                || self.governor_effect(self.cities[&city_id].owner, city_id, "disaster_immunity")
                    > 0.0;
            if protected {
                if barrier && !lowlands.is_empty() {
                    protected_by_barrier.insert(self.cities[&city_id].owner);
                }
                continue;
            }
            for position in lowlands {
                self.map.tiles.get_mut(&position).unwrap().flooded = true;
                self.damage_disaster_tile(position, 20);
            }
        }
        for owner in protected_by_barrier {
            self.add_historic_moment(owner, "MOMENT_MITIGATED_COASTAL_FLOOD");
        }
    }

    pub fn global_co2_emissions(&self) -> f64 {
        // A mirrored board carries the host's world total; its rivals'
        // emissions never cross, so the sum below would be ours alone.
        if let Some(total) = self
            .observed_climate
            .as_ref()
            .and_then(|climate| climate.co2_total)
        {
            return total.max(0.0);
        }
        self.players
            .iter()
            .map(|player| player.co2_emissions)
            .sum::<f64>()
            .max(0.0)
    }

    pub(super) fn co2_per_climate_point(&self) -> f64 {
        match self.map_size().id {
            "duel" => 250_000.0,
            "tiny" => 500_000.0,
            "small" => 750_000.0,
            "standard" => 1_000_000.0,
            "large" => 1_250_000.0,
            "huge" => 1_500_000.0,
            _ => 1_000_000.0,
        }
    }

    pub fn climate_points(&self) -> u8 {
        (self.global_co2_emissions() / self.co2_per_climate_point())
            .floor()
            .clamp(0.0, 8.0) as u8
    }

    fn lowland_has_barrier(&self, position: Pos) -> bool {
        self.map
            .get(position)
            .and_then(|tile| tile.owner_city)
            .and_then(|city| self.cities.get(&city))
            .is_some_and(|city| self.city_building_effect(city, "protect_coastal_lowlands") > 0.0)
    }

    fn flood_lowland_band(&mut self, band: u8) {
        let positions: Vec<Pos> = self
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| tile.coastal_lowland == band && !tile.submerged)
            .map(|(position, _)| *position)
            .collect();
        let mut protected_by_barrier = BTreeSet::new();
        for position in positions {
            if self.lowland_has_barrier(position) {
                if let Some(owner) = self.map.tiles[&position]
                    .owner_city
                    .and_then(|city| self.cities.get(&city))
                    .map(|city| city.owner)
                {
                    protected_by_barrier.insert(owner);
                }
                continue;
            }
            self.map.tiles.get_mut(&position).unwrap().flooded = true;
            self.damage_disaster_tile(position, 20);
            if let Some(city) = self.city_at(position) {
                self.cities.get_mut(&city).unwrap().pop = (self.cities[&city].pop - 1).max(1);
            }
        }
        for owner in protected_by_barrier {
            self.add_historic_moment(owner, "MOMENT_MITIGATED_COASTAL_FLOOD");
        }
    }

    fn submerge_lowland_band(&mut self, band: u8) {
        let positions: Vec<Pos> = self
            .map
            .tiles
            .iter()
            .filter(|(position, tile)| {
                tile.coastal_lowland == band
                    && !tile.submerged
                    && self.city_at(**position).is_none()
            })
            .map(|(position, _)| *position)
            .collect();
        for position in positions {
            if self.lowland_has_barrier(position) {
                continue;
            }
            for unit in self.units_at(position) {
                self.remove_unit(unit);
            }
            let owner_city = self.map.tiles[&position].owner_city;
            let district = self.map.tiles[&position].district;
            let wonder = self.map.tiles[&position].wonder;
            if let Some(city_id) = owner_city {
                if let Some(district) = district.as_deref() {
                    let removed_buildings: Vec<Name> = self.cities[&city_id]
                        .buildings
                        .iter()
                        .filter(|building| {
                            self.rules.buildings[building]
                                .district
                                .is_some_and(|family| {
                                    self.district_is_family(Name::new(district), family)
                                })
                        })
                        .cloned()
                        .collect();
                    let city = self.cities.get_mut(&city_id).unwrap();
                    city.districts
                        .remove_position(Name::new(district), position);
                    city.buildings
                        .retain(|building| !removed_buildings.contains(building));
                    city.pillaged_buildings
                        .retain(|building| !removed_buildings.contains(building));
                }
                if let Some(wonder) = wonder {
                    self.cities
                        .get_mut(&city_id)
                        .unwrap()
                        .wonders
                        .remove(&Name::new(&wonder));
                }
            }
            let tile = self.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("coast");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.pillaged = false;
            tile.district = None;
            tile.wonder = None;
            tile.road = 0;
            tile.continent = None;
            tile.coastal_lowland = 0;
            tile.flooded = false;
            tile.submerged = true;
        }
    }

    /// Carry the host's climate-change level onto a mirrored board. The level
    /// is the phase; the bands it has flooded follow the shipped
    /// `CoastalLowlands` table (`COASTAL_LOWLAND_1M` floods at
    /// `RANDOM_EVENT_SEA_LEVEL_RISE2`, 2M at RISE3, 3M at RISE5 — the same
    /// rows [`Game::apply_climate_phase`] runs), marked without the damage,
    /// the population loss and the moments a native rise books, because the
    /// host booked those itself and the export already shows their result. A
    /// band the host has SUBMERGED crosses as coast on the plot record and
    /// keeps no lowland type, so only flooding is marked here. A city whose
    /// Flood Barrier stands keeps its ground, as the host's does.
    pub(crate) fn mirror_set_climate_phase(&mut self, phase: u8) {
        self.climate_phase = phase.min(7);
        let flooded_bands: Vec<u8> = [(2u8, 1u8), (3, 2), (5, 3)]
            .iter()
            .filter(|(at, _)| self.climate_phase >= *at)
            .map(|(_, band)| *band)
            .collect();
        let positions: Vec<Pos> = self
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| {
                tile.coastal_lowland > 0
                    && !tile.submerged
                    && !tile.flooded
                    && flooded_bands.contains(&tile.coastal_lowland)
            })
            .map(|(position, _)| *position)
            .collect();
        for position in positions {
            if self.lowland_has_barrier(position) {
                continue;
            }
            if let Some(tile) = self.map.tiles.get_mut(&position) {
                tile.flooded = true;
            }
        }
    }

    pub(super) fn apply_climate_phase(&mut self, phase: u8) {
        match phase {
            2 => self.flood_lowland_band(1),
            3 => self.flood_lowland_band(2),
            4 => self.submerge_lowland_band(1),
            5 => self.flood_lowland_band(3),
            6 => self.submerge_lowland_band(2),
            7 => self.submerge_lowland_band(3),
            _ => {}
        }
    }

    pub(super) fn process_climate(&mut self) {
        let points = self.climate_points();
        let target_phase = if points >= 2 { points - 1 } else { 0 }.min(7);
        while self.climate_phase < target_phase {
            self.climate_phase += 1;
            self.apply_climate_phase(self.climate_phase);
        }
    }

    /// Per-turn accident probability for an active Nuclear Power Plant. The
    /// first ten turns are safe; risk then rises with reactor age until the
    /// city recommissions the plant.
    pub fn reactor_accident_risk(&self, city_id: u32) -> f64 {
        let Some(city) = self.cities.get(&city_id) else {
            return 0.0;
        };
        if self.city_building_effect(city, "reactor_accident_risk") <= 0.0 || city.reactor_age <= 10
        {
            return 0.0;
        }
        ((city.reactor_age - 10) as f64 * 0.0005).min(0.05)
    }

    /// Resolve one of Gathering Storm's three reactor-accident severities.
    /// The plant is pillaged, nearby tiles receive fallout and disaster
    /// damage, and the owning city can lose population at higher severity.
    pub fn resolve_reactor_accident(&mut self, city_id: u32, severity: u8) {
        let Some(city) = self.cities.get(&city_id) else {
            return;
        };
        if !city
            .buildings
            .iter()
            .any(|building| building == "nuclear_power_plant")
        {
            return;
        }
        let severity = severity.clamp(1, 3);
        let owner = city.owner;
        let center = self
            .city_district_family_position(city, crate::name!("industrial_zone"))
            .unwrap_or(city.pos);
        for position in self.wdisk(center, severity as i32 - 1) {
            if let Some(tile) = self.map.tiles.get_mut(&position) {
                tile.fallout_until = tile
                    .fallout_until
                    .max(self.turn.saturating_add(10 * severity as u32));
            }
            self.damage_disaster_tile(position, 20 * severity as i32);
        }
        let city = self.cities.get_mut(&city_id).unwrap();
        city.pillaged_buildings
            .insert(crate::name!("nuclear_power_plant"));
        city.pop = (city.pop - severity.saturating_sub(1) as i32).max(1);
        *self.players[owner]
            .counters
            .entry(format!("reactor_accident:{severity}"))
            .or_insert(0) += 1;
    }

    pub(super) fn process_reactors(&mut self, pid: usize) {
        let reactors: Vec<u32> = self
            .player_city_ids(pid)
            .into_iter()
            .filter(|city_id| {
                self.city_building_effect(&self.cities[city_id], "reactor_accident_risk") > 0.0
            })
            .collect();
        for city_id in reactors {
            self.cities.get_mut(&city_id).unwrap().reactor_age += 1;
            let risk = self.reactor_accident_risk(city_id);
            if risk > 0.0 && self.rng.chance(risk) {
                let age = self.cities[&city_id].reactor_age;
                let severity = if age >= 50 {
                    3
                } else if age >= 30 {
                    2
                } else {
                    1
                };
                self.resolve_reactor_accident(city_id, severity);
            }
        }
    }
}
