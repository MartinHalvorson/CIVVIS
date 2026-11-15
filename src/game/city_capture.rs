//! City capture: taking a city, handing it over, and what becomes of it.
//!
//! `capture_city` and `transfer_city` (capture, cession, and the loyalty
//! flip that routes through it), the pending capture a conqueror decides
//! on, and the capture rewards; keeping, razing or liberating a city
//! (`do_keep_city`, `do_raze_city`, `do_liberate_city`); and the grievances
//! each fate incurs, including those for eliminating a city-state or a
//! civilization and the relief a liberation brings.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! and `pub(crate)` methods are unchanged; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Begin a military conquest. Ownership changes immediately so the melee
    /// unit can enter, but victory and elimination are deferred until the
    /// conqueror chooses the city's fate.
    pub(super) fn capture_city(&mut self, cid: u32, new_owner: usize) {
        self.transfer_city(cid, new_owner, true);
        // A battlefield puts no verdict to the captor. There is no reason to
        // raze the objective and nobody to liberate it for, so the city is
        // kept the moment it falls and the battle goes on — or ends, if that
        // was the other side's last stand — without "Keep city?" ever
        // stopping the turn.
        if self.is_arena() {
            let _ = self.do_keep_city(new_owner, cid);
        }
    }

    pub(super) fn transfer_city(&mut self, cid: u32, new_owner: usize, conquest: bool) {
        let old = self.cities[&cid].owner;
        if old != new_owner {
            // A hypothetical capture/trade can move either seat's Palace.
            // Revert those seats to modeled placement until the next host frame.
            self.forget_observed_palaces(old, new_owner);
        }
        let original_owner = self.cities[&cid].original_owner;
        let original_capital = self.cities[&cid].is_capital;
        let final_city_of_old = conquest
            && self
                .cities
                .values()
                .filter(|city| city.owner == old)
                .count()
                == 1;
        if old != new_owner
            && !self.players[new_owner].is_minor
            && !self.players[new_owner].is_barbarian
            && !self.players[new_owner].is_free_city
        {
            if original_owner == new_owner {
                self.add_historic_moment(new_owner, "MOMENT_CITY_TRANSFERRED_TO_ORIGINAL_OWNER");
            } else if original_capital && !final_city_of_old {
                self.add_historic_moment(new_owner, "MOMENT_CITY_TRANSFERRED_FOREIGN_CAPITAL");
            }
        }
        {
            let (name, pos, capital, pop) = {
                let city = &self.cities[&cid];
                (
                    city.name.clone(),
                    city.pos,
                    city.is_capital && city.original_owner == old,
                    city.pop,
                )
            };
            let verb = if conquest { "captured" } else { "took over" };
            let (taker, loser) = (self.civ_name(new_owner), self.civ_name(old));
            let message = format!("{taker} {verb} {name} from {loser}");
            self.note(new_owner, "War", message.clone(), Some(pos));
            self.note(old, "War", message, Some(pos));
            if conquest {
                self.record_war_city_loss(new_owner, old, &name, capital, pop, pos);
            }
        }
        let captured_works = self
            .housed_great_works(old)
            .get(&cid)
            .cloned()
            .unwrap_or_default();
        let evacuation_positions: BTreeSet<Pos> = std::iter::once(self.cities[&cid].pos)
            .chain(
                self.cities[&cid]
                    .districts
                    .iter()
                    .filter(|(district, _)| {
                        matches!(
                            self.district_family(**district).as_str(),
                            "encampment" | "aerodrome"
                        )
                    })
                    .map(|(_, position)| *position),
            )
            .collect();
        let defensive_buildings: BTreeSet<Name> = self
            .rules
            .buildings
            .iter()
            .filter(|(_, spec)| spec.outer_defense > 0)
            .map(|(name, _)| *name)
            .collect();
        let converted_districts: Vec<(Name, Pos, bool)> =
            self.cities[&cid]
                .districts
                .iter()
                .map(|(district, position)| {
                    let family = self.district_family(*district).to_string();
                    let replacement = self
                        .rules
                        .districts
                        .iter()
                        .find(|(_, spec)| {
                            spec.unique_to
                                .as_deref()
                                .is_some_and(|owner| self.owns_civ_unique(new_owner, owner))
                                && spec.replaces == Some(Name::new(family.as_str()))
                        })
                        .map(|(name, _)| *name)
                        .unwrap_or_else(|| Name::new(family.as_str()));
                    // Empire-unique administrative districts are never captured;
                    // changing the parent city's owner removes the district and
                    // all of its buildings. Other districts survive only when the
                    // new owner has unlocked the required technology or civic.
                    let retained =
                        !matches!(family.as_str(), "government_plaza" | "diplomatic_quarter")
                            && self.rules.districts.get(&replacement).is_some_and(|spec| {
                                self.unlocked(new_owner, &spec.tech, &spec.civic)
                            });
                    (replacement, *position, retained)
                })
                .collect();
        let retained_families: BTreeSet<String> = converted_districts
            .iter()
            .filter(|(_, _, retained)| *retained)
            .map(|(district, _, _)| self.district_family(*district).to_string())
            .collect();
        let convert_building = |building: &str| {
            let family = self.rules.buildings[building]
                .replaces
                .unwrap_or(Name::new(building));
            self.rules
                .buildings
                .iter()
                .find(|(_, spec)| {
                    spec.unique_to
                        .as_deref()
                        .is_some_and(|owner| self.owns_civ_unique(new_owner, owner))
                        && spec.replaces == Some(family)
                })
                .map(|(name, _)| *name)
                .unwrap_or_else(|| Name::new(family.as_str()))
        };
        let converted_buildings: BTreeMap<Name, (usize, bool, bool)> = self.cities[&cid]
            .buildings
            .iter()
            .filter(|building| !defensive_buildings.contains(*building))
            .filter_map(|building| {
                let converted = convert_building(building);
                let district = self.rules.buildings[&converted]
                    .district
                    .unwrap_or(crate::name!("city_center"));
                (district == "city_center" || retained_families.contains(district.as_str())).then(
                    || {
                        let era = self.cities[&cid]
                            .building_eras
                            .get(building)
                            .copied()
                            .unwrap_or(self.world_era);
                        let pillaged = self.cities[&cid].pillaged_buildings.contains(building);
                        (
                            Name::new(&converted),
                            (era, pillaged, district == "city_center"),
                        )
                    },
                )
            })
            .collect();
        let steel_retains_defenses = conquest
            && self.players[new_owner]
                .techs
                .contains(&crate::name!("steel"))
            && !self.players[old].techs.contains(&crate::name!("steel"));
        {
            let city = self.cities.get_mut(&cid).unwrap();
            city.owner = new_owner;
            city.queue.clear();
            city.production_progress
                .retain(|item, _| !item.starts_with("district:"));
            city.captured_from = conquest.then_some(old);
            city.occupied_from = conquest.then_some(old);
            // A new owner begins a new occupation ledger. This also clears
            // any prior capture amount when a city defects or is peacefully
            // returned, so a later peace cannot cede an already-resolved war.
            city.occupation_grievance = None;
            if conquest {
                // Gathering Storm removes approximately one quarter of the
                // population, retaining the ceiling of 75%.
                city.pop = ((city.pop * 3 + 3) / 4).max(1);
                city.hp = 100;
                city.loyalty = city.loyalty.min(50.0);
            }
            // Every change of owner strips the constructed Walls below, so the
            // pool they filled goes with them however the city changed hands.
            // Leaving it behind on a city that defected or was ceded gave it a
            // hundred hit points of walls that were not there, and with them a
            // ranged strike it had no Encampment or Walls to fire from.
            city.wall_hp = if steel_retains_defenses { 100 } else { 0 };
            city.encampment_wall_hp = if steel_retains_defenses { 100 } else { 0 };
            for state in &mut city.defending_districts {
                state.wall_hp = city.encampment_wall_hp;
                state.struck = false;
                state.extra_strikes_used = 0;
            }

            city.buildings = converted_buildings.keys().cloned().collect();
            city.building_eras = converted_buildings
                .iter()
                .map(|(building, (era, _, _))| (*building, *era))
                .collect();
            city.pillaged_buildings = city
                .buildings
                .iter()
                .filter(|building| {
                    let (_, was_pillaged, city_center) = converted_buildings[building];
                    was_pillaged || (conquest && city_center)
                })
                .cloned()
                .collect();

            city.districts.clear();
            for (district, position, retained) in &converted_districts {
                if *retained {
                    city.districts.insert(Name::new(district), *position);
                }
            }
        }
        self.restore_other_defending_districts(cid);
        if old != new_owner {
            self.invalidate_routing_paths();
        }
        for position in self.cities[&cid].owned_tiles.clone() {
            self.map
                .tiles
                .get_mut(&position)
                .unwrap()
                .district_foundation = None;
        }
        for (district, position, retained) in &converted_districts {
            let tile = self.map.tiles.get_mut(position).unwrap();
            tile.district = retained.then(|| Name::new(district));
            if !retained {
                tile.pillaged = false;
            }
        }
        if !retained_families.contains("encampment") {
            let city = self.cities.get_mut(&cid).unwrap();
            city.encampment_hp = 0;
            city.encampment_wall_hp = 0;
            city.encampment_pillaged = false;
        }
        for position in self.cities[&cid].owned_tiles.clone() {
            let Some(improvement) = self.map.tiles[&position].improvement else {
                continue;
            };
            let retained =
                self.rules
                    .improvements
                    .get(&improvement)
                    .is_some_and(|spec| {
                        spec.unique_to.as_deref().is_none_or(|civilization| {
                            self.owns_civ_unique(new_owner, civilization)
                        }) && self.unlocked(new_owner, &spec.tech, &spec.civic)
                    });
            if !retained {
                let tile = self.map.tiles.get_mut(&position).unwrap();
                tile.improvement = None;
                tile.pillaged = false;
            }
        }
        for (kind, count) in captured_works {
            let key = format!("great_work:{kind}");
            *self.players[old].counters.entry(key.clone()).or_insert(0) -= count as i64;
            *self.players[new_owner].counters.entry(key).or_insert(0) += count as i64;
        }
        // An empire may have only one Industry for a given luxury. If a
        // captured city would duplicate one already controlled by its new
        // owner, the captured Industry is removed (Corporations are globally
        // unique and therefore cannot collide this way).
        let duplicate_industry = self.cities[&cid]
            .owned_tiles
            .iter()
            .find_map(|position| {
                let tile = &self.map.tiles[position];
                (tile.improvement.as_deref() == Some("industry"))
                    .then(|| tile.resource.map(|resource| (*position, resource)))
                    .flatten()
            })
            .filter(|(_, resource)| {
                self.cities.values().any(|other| {
                    other.id != cid
                        && other.owner == new_owner
                        && self
                            .city_economic_improvement(other)
                            .is_some_and(|(candidate, _)| candidate == *resource)
                })
            });
        if let Some((position, _)) = duplicate_industry {
            let tile = self.map.tiles.get_mut(&position).unwrap();
            tile.improvement = None;
            tile.pillaged = false;
        }
        for position in evacuation_positions {
            for oid in self.units_at(position) {
                // Removing a carrier also removes aircraft whose IDs can still
                // be in this evacuation snapshot.
                let Some(unit) = self.units.get(&oid) else {
                    continue;
                };
                if unit.owner == old {
                    if conquest {
                        self.tally_combat_death(new_owner, oid);
                    }
                    if matches!(self.units[&oid].kind.as_str(), "builder" | "settler") {
                        self.transfer_unit_owner(oid, new_owner);
                    } else {
                        self.remove_unit(oid);
                    }
                }
            }
        }
        for p in self.players.iter_mut() {
            p.governors.retain(|g| *g != cid);
            for governor in p.governor_roster.values_mut() {
                if governor.city == Some(cid) {
                    governor.city = None;
                }
            }
        }
        let recalled = self
            .routes
            .iter()
            .filter(|route| route.origin == cid || route.dest == cid)
            .cloned()
            .collect();
        self.recall_trade_routes(recalled);
        if !conquest {
            self.players[new_owner].alive = true;
            self.check_elimination(old);
            self.check_domination();
        }
    }

    fn pending_city_capture(&self, pid: usize, cid: u32) -> Result<usize, String> {
        let city = self
            .cities
            .get(&cid)
            .ok_or_else(|| "no such captured city".to_string())?;
        if city.owner != pid {
            return Err("not your captured city".into());
        }
        city.captured_from
            .ok_or_else(|| "that city's fate is already resolved".to_string())
    }

    fn war_profile_for(&self, attacker: usize, defender: usize) -> CasusBelliProfile {
        self.war_sides(attacker, defender)
            .and_then(|(first, second)| self.wars.get(&pair(first, second)))
            .and_then(|war| war.casus_belli.as_deref())
            .and_then(casus_belli_profile)
            // Unit-test scenarios and legacy saves can resolve a capture
            // without a durable declaration record.  Preserve the old formal
            // capture baseline rather than charging a surprise multiplier
            // simply because the history predates the new field.
            .unwrap_or_else(|| casus_belli_profile("formal_war").unwrap())
    }

    fn city_fate_grievances(
        &self,
        conqueror: usize,
        defeated: usize,
        cid: u32,
        razed: bool,
    ) -> f64 {
        let Some(city) = self.cities.get(&cid) else {
            return 0.0;
        };
        // Retaking a city you originally founded is never a grievance event;
        // Reconquest makes that exception explicit, but the result is the
        // same even when a player had no named casus at declaration time.
        if city.original_owner == conqueror {
            return 0.0;
        }
        let profile = self.war_profile_for(conqueror, defeated);
        let base = self.city_capture_base_grievances(cid);
        if razed {
            base * (CITY_RAZE_GRIEVANCES / CITY_CAPTURE_GRIEVANCES) * profile.raze_multiplier
        } else {
            base * profile.capture_multiplier
        }
    }

    /// The capture charge is capped at 50. A city below the world's average
    /// population produces the corresponding fraction, while a city at or
    /// above that average reaches the cap. Captures call this after the
    /// standard conquest population loss, matching the population that
    /// actually changes hands.
    pub(super) fn city_capture_base_grievances(&self, cid: u32) -> f64 {
        let Some(city) = self.cities.get(&cid) else {
            return 0.0;
        };
        let city_count = self.cities.len().max(1) as f64;
        let average_population = self
            .cities
            .values()
            .map(|other| other.pop.max(1) as f64)
            .sum::<f64>()
            / city_count;
        CITY_CAPTURE_GRIEVANCES * (city.pop.max(1) as f64 / average_population.max(1.0)).min(1.0)
    }

    pub(super) fn capture_rewards(
        &mut self,
        conqueror: usize,
        defeated: usize,
        cid: u32,
        razed: bool,
    ) -> f64 {
        let grievances = self.city_fate_grievances(conqueror, defeated, cid, razed);
        bump(&mut self.players[conqueror], "captures");
        if !self.players[defeated].is_minor
            && !self.players[defeated].is_barbarian
            && !self.players[defeated].is_free_city
            && !self.cities.values().any(|other| other.owner == defeated)
        {
            self.first_historic_moment(
                conqueror,
                &format!("civilization_defeated:{defeated}"),
                Some("MOMENT_CITY_TRANSFERRED_PLAYER_DEFEATED"),
                None,
            );
        }
        if defeated != conqueror
            && !self.players[defeated].is_minor
            && !self.players[defeated].is_barbarian
            && !self.players[defeated].is_free_city
        {
            self.add_grievances(defeated, conqueror, grievances);
        }
        let duration = self
            .cities
            .values()
            .filter(|city| city.owner == conqueror)
            .map(|city| self.city_building_effect(city, "capture_city_duration"))
            .fold(0.0, f64::max) as i64;
        if duration > 0 {
            self.players[conqueror].counters.insert(
                "warlords_throne_until".to_string(),
                self.turn as i64 + duration,
            );
        }
        grievances
    }

    fn city_state_elimination_grievances(
        &mut self,
        defeated: usize,
        conqueror: usize,
        razed: bool,
    ) {
        if !self.players[defeated].is_minor
            || self.players[defeated].is_barbarian
            || self.players[defeated].alive
        {
            return;
        }
        for observer in 0..self.players.len() {
            if observer == conqueror
                || self.players[observer].is_minor
                || self.players[observer].is_barbarian
                || !self.players[observer].alive
                || !self.has_met(observer, defeated)
            {
                continue;
            }
            self.add_direct_grievances(
                observer,
                conqueror,
                if razed {
                    CITY_STATE_RAZE_GRIEVANCES
                } else {
                    CITY_STATE_CONQUEST_GRIEVANCES
                },
            );
        }
    }

    fn civilization_elimination_grievances(&mut self, defeated: usize, conqueror: usize) {
        if self.players[defeated].is_minor
            || self.players[defeated].is_barbarian
            || self.players[defeated].alive
        {
            return;
        }
        for observer in 0..self.players.len() {
            if observer == conqueror
                || observer == defeated
                || self.players[observer].is_minor
                || self.players[observer].is_barbarian
                || !self.players[observer].alive
            {
                continue;
            }
            self.add_direct_grievances(observer, conqueror, 150.0);
        }
    }

    pub(super) fn do_keep_city(&mut self, pid: usize, cid: u32) -> Result<(), String> {
        let defeated = self.pending_city_capture(pid, cid)?;
        if self.players[pid].is_minor && self.city_can_be_razed_by(pid, &self.cities[&cid]) {
            return Err("city-states must raze captured cities when possible".into());
        }
        self.cities.get_mut(&cid).unwrap().captured_from = None;
        let grievances = self.capture_rewards(pid, defeated, cid, false);
        self.cities.get_mut(&cid).unwrap().occupation_grievance = Some(grievances);
        self.request_city_capture_emergency(pid, cid, defeated);
        self.check_elimination(defeated);
        self.city_state_elimination_grievances(defeated, pid, false);
        self.civilization_elimination_grievances(defeated, pid);
        self.check_domination();
        Ok(())
    }

    pub(super) fn do_raze_city(&mut self, pid: usize, cid: u32) -> Result<(), String> {
        let defeated = self.pending_city_capture(pid, cid)?;
        let city = self.cities[&cid].clone();
        if !self.city_can_be_razed_by(pid, &city) {
            return Err("that captured city cannot be razed".into());
        }
        self.capture_rewards(pid, defeated, cid, true);
        self.record_war_moment(
            pid,
            defeated,
            "city_razed",
            Some(city.name.clone()),
            Some(city.pos),
        );

        for position in &city.owned_tiles {
            if let Some(tile) = self.map.tiles.get_mut(position) {
                if tile.owner_city == Some(cid) {
                    tile.owner_city = None;
                }
            }
        }
        for (_, position) in city.districts.iter() {
            if let Some(tile) = self.map.tiles.get_mut(position) {
                tile.district = None;
                tile.pillaged = false;
            }
        }
        for position in &city.owned_tiles {
            if let Some(tile) = self.map.tiles.get_mut(position) {
                tile.district_foundation = None;
            }
        }
        // Wonder tiles remain as inert ruins. `wonder_built` includes them so
        // a unique World Wonder cannot be constructed for a second time.
        self.city_by_pos.remove(&city.pos);
        self.cities.remove(&cid);
        self.check_elimination(defeated);
        self.city_state_elimination_grievances(defeated, pid, true);
        self.civilization_elimination_grievances(defeated, pid);
        self.check_domination();
        Ok(())
    }

    /// Liberation creates a matching negative grievance with every living
    /// major leader. The pairwise ledger is intentionally aggregate, so the
    /// goodwill subtracts from that leader's current balance without ever
    /// crossing zero into a fabricated grievance against the liberator.
    fn relieve_liberation_grievances(&mut self, liberator: usize, relief: f64) {
        let observers: Vec<usize> = self
            .players
            .iter()
            .filter(|observer| {
                observer.alive
                    && !observer.is_minor
                    && !observer.is_barbarian
                    && !observer.is_free_city
                    && observer.id != liberator
            })
            .map(|observer| observer.id)
            .collect();
        for observer in observers {
            self.relieve_direct_grievances(observer, liberator, relief);
        }
    }

    pub(super) fn do_liberate_city(&mut self, pid: usize, cid: u32) -> Result<(), String> {
        if self.players[pid].is_minor {
            return Err("city-states do not liberate captured cities".into());
        }
        let defeated = self.pending_city_capture(pid, cid)?;
        let original_owner = self.cities[&cid].original_owner;
        if original_owner == pid || original_owner == defeated {
            return Err("this city cannot be liberated".into());
        }
        let relief = if self.players[original_owner].is_minor {
            CITY_STATE_CONQUEST_GRIEVANCES
        } else {
            self.city_capture_base_grievances(cid)
        };
        let emergency_ids: Vec<u32> = self
            .active_emergencies
            .iter_mut()
            .filter(|emergency| emergency.city == cid && emergency.target == defeated)
            .map(|emergency| {
                if emergency.members.contains(&pid) {
                    *emergency.contributions.entry(pid).or_insert(0) += 1;
                }
                emergency.id
            })
            .collect();
        let restored_to_game = !self.players[original_owner].alive;
        self.forget_observed_palaces(pid, original_owner);
        {
            let city = self.cities.get_mut(&cid).unwrap();
            city.owner = original_owner;
            city.captured_from = None;
            city.occupied_from = None;
            city.occupation_grievance = None;
            city.loyalty = 100.0;
            // Capture can grant a Steel-equipped conqueror a temporary
            // minimum outer defense even after the constructed Walls were
            // stripped. Liberation is a non-conquest ownership change, so
            // that defense must not follow the liberator to a founder who
            // may have no wall pool at all.
            city.wall_hp = 0;
            city.encampment_wall_hp = 0;
            for state in &mut city.defending_districts {
                state.wall_hp = 0;
            }
        }
        self.invalidate_routing_paths();
        self.players[original_owner].alive = true;
        self.relieve_liberation_grievances(pid, relief);
        let favor = if self.players[original_owner].is_minor || restored_to_game {
            100.0
        } else {
            50.0
        };
        self.players[pid].diplomatic_favor += favor;
        self.players[pid]
            .counters
            .entry("cities_liberated".to_string())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        self.add_historic_moment(pid, "MOMENT_CITY_TRANSFERRED_TO_ORIGINAL_OWNER");
        if self.players[original_owner].is_minor {
            // Shipped `Eras_XP1.LiberatedEnvoys`, which is 2 in the Ancient
            // era rather than 3 and then climbs 3/3/6/6/9 through the rest.
            let liberation_envoys = ERA_NAMES
                .get(self.world_era)
                .and_then(|era| self.rules.eras.get(era))
                .map(|era| era.liberated_envoys as i64)
                .filter(|envoys| *envoys > 0)
                .unwrap_or(9);
            for player in self.players.iter_mut() {
                player.envoys.retain(|(minor, _)| *minor != original_owner);
            }
            self.players[pid]
                .envoys
                .push((original_owner, liberation_envoys));
            for _ in 0..liberation_envoys {
                self.expand_borders(cid);
            }
        }
        self.check_elimination(defeated);
        self.check_domination();
        for emergency in emergency_ids {
            self.resolve_emergency(emergency, true);
        }
        Ok(())
    }
}
