//! Completion: what happens when a city finishes what it was building.
//!
//! `complete_item`, the turn's handling of a finished unit, building,
//! district, wonder or project, and `complete_item_and_collect_upkeep`
//! around it; a wonder's completion effects
//! (`apply_wonder_completion_effects`); the effects a training district
//! has on a unit it trains, the heavy cavalry a completion can grant, and
//! placing a new unit on the map (`place_new_unit`).
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub(super) fn apply_wonder_completion_effects(
        &mut self,
        pid: usize,
        cid: u32,
        _wonder: &str,
        position: Pos,
        spec: &crate::rules::WonderSpec,
    ) {
        let effect = |name: &str| spec.effects.get(name).copied().unwrap_or(0.0);
        let game_speed = self.game_speed;
        self.players[pid].envoys_free += effect("envoys") as i64;
        self.players[pid].dvp += effect("diplomatic_victory_points") as i64;
        *self.players[pid]
            .counters
            .entry("district_governor_titles".to_string())
            .or_insert(0) += effect("governor_titles") as i64;

        // Apadana includes itself and triggers for each later wonder in the
        // same city, so evaluate it after the new wonder has been inserted.
        if self.cities[&cid]
            .wonders
            .contains_key(&crate::name!("apadana"))
        {
            self.players[pid].envoys_free += self.rules.wonders["apadana"]
                .effects
                .get("envoys_per_wonder")
                .copied()
                .unwrap_or(0.0) as i64;
        }

        let population = effect("empire_population") as i32;
        if population > 0 {
            for city_id in self.player_city_ids(pid) {
                self.increase_city_population(city_id, population);
            }
        }
        let treasury_pct = effect("treasury_gold_pct");
        if treasury_pct != 0.0 {
            self.players[pid].gold *= 1.0 + treasury_pct / 100.0;
        }
        if effect("free_great_prophet") > 0.0
            && self.players[pid].religion.is_none()
            && !self.players[pid].prophet_pending
            && self.religions_founded() < self.max_religions()
        {
            self.players[pid].prophet_pending = true;
            self.add_historic_moment(pid, "MOMENT_GREAT_PERSON_CREATED_GAME_ERA");
            self.dedication_trigger(pid, "great_person", 1);
            bump(&mut self.players[pid], "great_people");
        }
        if effect("ancient_classical_tech_boosts") > 0.0 {
            let boosts: Vec<(Name, f64)> = self
                .rules
                .techs
                .iter()
                .filter(|(node, tech)| {
                    tech.era <= 1
                        && !self.players[pid].techs.contains(*node)
                        && !self.players[pid].boosted_techs.contains(*node)
                })
                .map(|(node, tech)| (*node, game_speed.scale(tech.cost)))
                .collect();
            for (node, cost) in boosts {
                let fraction = self.node_boost_frac(pid, &node, true);
                self.players[pid].boosted_techs.insert(Name::new(&node));
                if self.players[pid].research.as_deref() == Some(node.as_str()) {
                    self.players[pid].research_progress += fraction * cost;
                }
            }
        }
        self.complete_random_nodes(pid, effect("free_random_techs") as usize, true);
        self.complete_random_nodes(pid, effect("free_random_civics") as usize, false);
        if effect("free_atomic_civic_boosts") > 0.0 {
            let candidates: Vec<(Name, f64)> = self
                .rules
                .civics
                .iter()
                .filter(|(node, civic)| {
                    civic.era == 6
                        && !self.players[pid].civics.contains(*node)
                        && !self.players[pid].boosted_civics.contains(*node)
                })
                .map(|(node, civic)| (*node, game_speed.scale(civic.cost)))
                .collect();
            if !candidates.is_empty() {
                let (node, cost) = candidates[self.rng.below(candidates.len())];
                self.players[pid].boosted_civics.insert(Name::new(&node));
                if self.players[pid].civic.as_deref() == Some(node.as_str()) {
                    self.players[pid].civic_progress +=
                        self.node_boost_frac(pid, &node, false) * cost;
                }
            }
        }

        let city_pos = self.cities[&cid].pos;
        for (unit, key) in [
            ("builder", "free_builder"),
            ("trader", "free_trader"),
            ("apostle", "apostles"),
            ("guru", "gurus"),
            ("archer", "free_archers"),
            ("spearman", "free_spearmen"),
            ("battering_ram", "free_battering_rams"),
            ("warrior_monk", "free_warrior_monks"),
            ("settler", "free_settlers"),
        ] {
            for _ in 0..effect(key) as usize {
                self.place_new_unit(unit, pid, city_pos);
            }
        }

        let free_canals = effect("free_adjacent_canals") as usize;
        if free_canals > 0 {
            let legal: BTreeSet<Pos> = self
                .district_sites(cid, crate::name!("canal"))
                .into_iter()
                .collect();
            let mut sites: Vec<Pos> = self
                .nbrs(position)
                .into_iter()
                .filter(|site| legal.contains(site))
                .collect();
            sites.sort();
            for site in sites.into_iter().take(free_canals) {
                let tile = self.map.tiles.get_mut(&site).unwrap();
                tile.district = Some(crate::name!("canal"));
                tile.improvement = None;
                tile.feature = None;
                tile.pillaged = false;
                self.cities
                    .get_mut(&cid)
                    .unwrap()
                    .districts
                    .insert(crate::name!("canal"), site);
            }
        }

        if effect("bridge_passage") > 0.0 {
            self.map.tiles.get_mut(&position).unwrap().road = 4;
            let neighbors = self.nbrs(position);
            let land_sides: Vec<usize> = neighbors
                .iter()
                .enumerate()
                .filter(|(_, neighbor)| {
                    self.map
                        .get(**neighbor)
                        .is_some_and(|tile| !self.rules.is_water(tile))
                })
                .map(|(index, _)| index)
                .collect();
            if let Some((left, right)) = land_sides.iter().find_map(|left| {
                land_sides
                    .iter()
                    .find(|right| (*left as i32 - **right as i32).abs() == 3)
                    .map(|right| (*left, *right))
            }) {
                self.map.tiles.get_mut(&neighbors[left]).unwrap().road = 4;
                self.map.tiles.get_mut(&neighbors[right]).unwrap().road = 4;
            }
        }

        if effect("free_foreign_continent_city_center_building") > 0.0 {
            let targets: Vec<u32> = self
                .player_city_ids(pid)
                .into_iter()
                .filter(|city_id| self.on_foreign_continent(pid, self.cities[city_id].pos))
                .collect();
            for city_id in targets {
                let mut candidates: Vec<Name> = self
                    .rules
                    .buildings
                    .iter()
                    .filter(|(_, building)| {
                        building.buildable
                            && building.district.is_some_and(|district| {
                                self.district_is_family(district, crate::name!("city_center"))
                            })
                    })
                    .filter(|(building, _)| {
                        self.can_produce(
                            pid,
                            city_id,
                            &Item::Building {
                                building: **building,
                            },
                        )
                    })
                    .map(|(building, _)| *building)
                    .collect();
                candidates.sort_by(|a, b| {
                    self.rules.buildings[a]
                        .cost
                        .partial_cmp(&self.rules.buildings[b].cost)
                        .unwrap()
                        .then(a.cmp(b))
                });
                if let Some(building) = candidates.first() {
                    let city = self.cities.get_mut(&city_id).unwrap();
                    city.buildings.push(Name::new(building));
                    city.building_eras
                        .insert(Name::new(building), self.world_era);
                    self.note_building_completed_moments(pid, building);
                }
            }
        }
        if self.players[pid].dvp >= DIPLOMATIC_VICTORY_POINTS {
            self.set_winner(pid, "diplomatic");
        }
    }

    /// Complete an item and report any already-processed cities whose
    /// infrastructure upkeep changed as an empire-wide side effect. Ordinary
    /// items are local; only a wonder needs the defensive before/after census.
    pub(super) fn complete_item_and_collect_upkeep(
        &mut self,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> (bool, Vec<(u32, f64)>) {
        let before = matches!(item, Item::Wonder { .. }).then(|| self.city_upkeep_snapshot(pid));
        let completed = self.complete_item(pid, cid, item);
        let Some(before) = before else {
            return (completed, Vec::new());
        };
        if !completed {
            return (false, Vec::new());
        }
        let after = self.city_upkeep_snapshot(pid);
        let changed = after
            .into_iter()
            .filter_map(|(city_id, upkeep)| {
                (before.get(&city_id).copied() != Some(upkeep)).then_some((city_id, upkeep))
            })
            .collect();
        (true, changed)
    }

    pub(super) fn complete_item(&mut self, pid: usize, cid: u32, item: &Item) -> bool {
        match item {
            Item::Formation { unit, formation } => {
                if !self.commit_unit_resource(pid, cid, item) {
                    return false;
                }
                let pos = self.cities[&cid].pos;
                let Some(placed) = self.place_new_unit(unit, pid, pos) else {
                    return false;
                };
                self.set_unit_formation(placed, *formation);
                self.cities
                    .get_mut(&cid)
                    .unwrap()
                    .strategic_resource_commitments
                    .remove(&Self::item_progress_key(item));
                self.apply_training_district_effects(cid, placed);
                let science_pct =
                    self.city_building_effect(&self.cities[&cid], "science_per_unit_cost_pct");
                self.players[pid].research_overflow += self.item_cost(item) * science_pct / 100.0;
                if self.rules.units[unit].domain.as_deref() == Some("sea")
                    && self.empire_wonder_effect(pid, "duplicate_naval_units") > 0.0
                {
                    if let Some(second) = self.place_new_unit(unit, pid, pos) {
                        self.set_unit_formation(second, *formation);
                        self.apply_training_district_effects(cid, second);
                    }
                }
                if *formation == 1 {
                    bump(&mut self.players[pid], "corps");
                }
                self.note_formation_moment(pid, placed);
                true
            }
            Item::Unit { unit } => {
                if !self.commit_unit_resource(pid, cid, item) {
                    return false;
                }
                if unit == "spy" {
                    let id = self.next_id;
                    self.next_id += 1;
                    self.spies.insert(
                        id,
                        Spy {
                            id,
                            owner: pid,
                            level: 0,
                            promotions: BTreeSet::new(),
                            city: Some(cid),
                            ready_turn: self.turn,
                            mission: None,
                            sources_city: None,
                            sources_until: 0,
                            captured_by: None,
                        },
                    );
                    return true;
                }
                let pos = self.cities[&cid].pos;
                let Some(placed) = self.place_new_unit(unit, pid, pos) else {
                    return false;
                };
                self.cities
                    .get_mut(&cid)
                    .unwrap()
                    .strategic_resource_commitments
                    .remove(&Self::item_progress_key(item));
                self.apply_training_district_effects(cid, placed);
                if unit == "builder" {
                    self.units.get_mut(&placed).unwrap().charges +=
                        self.governor_effect(pid, cid, "builder_charges") as i32;
                }
                if unit == "settler" && self.settler_consumes_population(pid, cid) {
                    self.cities.get_mut(&cid).unwrap().pop -= 1;
                }
                bump(&mut self.players[pid], &format!("trained:{unit}"));
                if matches!(
                    self.rules.units[unit].class.as_str(),
                    "military" | "support"
                ) {
                    let science_pct =
                        self.city_building_effect(&self.cities[&cid], "science_per_unit_cost_pct");
                    self.players[pid].research_overflow +=
                        self.rules.units[unit].cost * science_pct / 100.0;
                }
                if self.rules.units[unit].domain.as_deref() == Some("sea")
                    && self.empire_wonder_effect(pid, "duplicate_naval_units") > 0.0
                {
                    if let Some(second) = self.place_new_unit(unit, pid, pos) {
                        self.apply_training_district_effects(cid, second);
                    }
                }
                true
            }
            Item::Building { building } => {
                // Older saves may contain a building both in the completed list and
                // at the front of the production queue after purchasing it. Treat
                // that stale queue entry as complete so it is discarded without
                // duplicating the building or reapplying its one-time effects.
                if self.cities[&cid]
                    .buildings
                    .iter()
                    .any(|owned| owned == building)
                {
                    return true;
                }
                let spec = self.rules.buildings[building].clone();
                if spec.wonder && self.wonder_built(building) {
                    // wonder race lost: drop the item, keep banked production
                    let city = self.cities.get_mut(&cid).unwrap();
                    city.queue.clear();
                    return false;
                }
                let city = self.cities.get_mut(&cid).unwrap();
                city.buildings.push(Name::new(building));
                city.building_eras
                    .insert(Name::new(building), self.world_era);
                if spec
                    .effects
                    .get("reactor_accident_risk")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                {
                    city.reactor_age = 0;
                }
                self.note_dedicated_building(pid, building, &spec);
                self.note_building_completed_moments(pid, building);
                if spec.district.is_some_and(|district| {
                    self.district_is_family(district, crate::name!("government_plaza"))
                }) {
                    *self.players[pid]
                        .counters
                        .entry("district_governor_titles".to_string())
                        .or_insert(0) += 1;
                }
                if spec.outer_defense > 0 {
                    let has_encampment = self
                        .city_has_district_family(&self.cities[&cid], crate::name!("encampment"));
                    // The new tier is already in `buildings`, so this pool
                    // includes it. Top the defences up to it rather than
                    // adding blind: Urban Defenses floors the pool at 400
                    // regardless of what is built, and a city repaired up to
                    // that floor and then finishing another set of Walls
                    // ended up holding more wall than the pool allows.
                    let pool = self.city_max_wall_hp(&self.cities[&cid]);
                    let city = self.cities.get_mut(&cid).unwrap();
                    city.wall_hp = (city.wall_hp + spec.outer_defense).min(pool);
                    for state in &mut city.defending_districts {
                        if !state.pillaged {
                            state.wall_hp = (state.wall_hp + spec.outer_defense).min(pool);
                        }
                    }
                    if has_encampment && !city.encampment_pillaged {
                        city.encampment_wall_hp =
                            (city.encampment_wall_hp + spec.outer_defense).min(pool);
                    }
                }
                if spec.wonder {
                    self.add_historic_moment(pid, self.wonder_historic_moment(building));
                }
                if spec.unit_levels > 0 {
                    for uid in self.player_unit_ids(pid) {
                        let mil = self.rules.units[self.units[&uid].kind].class == "military";
                        if mil {
                            let u = self.units.get_mut(&uid).unwrap();
                            u.xp = u.xp.max(Self::promotion_threshold(u.level));
                        }
                    }
                }
                if spec.district.is_some_and(|district| {
                    self.district_is_family(district, crate::name!("entertainment_complex"))
                }) && self.city_district_effect(&self.cities[&cid], "free_heavy_cavalry") > 0.0
                {
                    self.grant_heavy_cavalry(pid, cid);
                }
                if spec
                    .effects
                    .get("protect_coastal_lowlands")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                {
                    let lowlands = self.coastal_lowland_tiles(&self.cities[&cid]);
                    let mut repaired_buildings = BTreeSet::new();
                    for position in lowlands {
                        let district = self.map.tiles[&position].district;
                        let tile = self.map.tiles.get_mut(&position).unwrap();
                        tile.flooded = false;
                        tile.pillaged = false;
                        if let Some(district) = district {
                            repaired_buildings.extend(
                                self.cities[&cid]
                                    .buildings
                                    .iter()
                                    .filter(|candidate| {
                                        self.rules.buildings[candidate].district.is_some_and(
                                            |family| self.district_is_family(district, family),
                                        )
                                    })
                                    .cloned(),
                            );
                        }
                    }
                    self.cities
                        .get_mut(&cid)
                        .unwrap()
                        .pillaged_buildings
                        .retain(|candidate| !repaired_buildings.contains(candidate));
                }
                let completion_faith = spec.cost
                    * self.governor_effect(pid, cid, "building_completion_faith_pct")
                    / 100.0;
                self.players[pid].faith += completion_faith;
                true
            }
            Item::District { district, pos } => {
                if !self.district_sites(cid, district).contains(pos) {
                    return false;
                }
                let spec = self.rules.districts[district].clone();
                let preserve_feature = self.players[pid].civ == "Vietnam" && spec.specialty;
                let t = self.map.tiles.get_mut(pos).unwrap();
                t.district = Some(*district);
                t.district_foundation = None;
                t.improvement = None;
                t.pillaged = false;
                if !preserve_feature {
                    t.feature = None;
                }
                self.cities
                    .get_mut(&cid)
                    .unwrap()
                    .districts
                    .insert(Name::new(district), *pos);
                // PER_DISTRICT_CONSTRUCTED, not specialty: the database has a
                // SPECIALTY_DISTRICT_CONSTRUCTED vocabulary and uses it for the
                // free-building effects, so leaving it off here is deliberate.
                // The City Center is placed at founding rather than built, so
                // it never reaches this path.
                self.dedication_trigger(pid, "district", 1);
                self.note_district_completed_moments(pid, district, *pos);
                if self.district_is_family(district, crate::name!("aerodrome")) {
                    self.dedication_trigger(pid, "aerodrome", 1);
                }
                if self.district_has_defenses(*district) {
                    let max_wall = self.city_max_wall_hp(&self.cities[&cid]);
                    self.set_defending_district_state(
                        cid,
                        DefendingDistrict::healthy(*district, *pos, max_wall),
                    );
                }
                if let Some(amount) = spec.effects.get("governor_titles") {
                    *self.players[pid]
                        .counters
                        .entry("district_governor_titles".to_string())
                        .or_insert(0) += *amount as i64;
                }
                if let Some(amount) = spec.effects.get("envoys") {
                    self.players[pid].envoys_free += *amount as i64;
                }
                if let Some(amount) = spec.effects.get("envoy_if_adjacent_city_center") {
                    if self.nbrs(*pos).contains(&self.cities[&cid].pos) {
                        self.players[pid].envoys_free += *amount as i64;
                    }
                }
                if spec
                    .effects
                    .get("unlock_apprenticeship")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                {
                    let first = {
                        let player = &mut self.players[pid];
                        let first = player.techs.insert(crate::name!("apprenticeship"));
                        if player.research.as_deref() == Some("apprenticeship") {
                            player.research = None;
                            player.research_progress = 0.0;
                        }
                        first
                    };
                    if first {
                        self.apply_tree_completion(pid, true, "apprenticeship", true);
                        self.share_team_technology_boost(pid, "apprenticeship");
                    }
                }
                let warrior_monk_bomb =
                    self.players[pid]
                        .religion
                        .as_deref()
                        .is_some_and(|religion| {
                            self.religion_belief_effect(religion, "holy_site_culture_bomb") > 0.0
                        })
                        && self.district_is_family(district, crate::name!("holy_site"));
                if spec.effects.get("culture_bomb").copied().unwrap_or(0.0) > 0.0
                    || warrior_monk_bomb
                    || (self.players[pid].civ == "Gaul" && spec.specialty)
                {
                    self.culture_bomb(cid, *pos);
                }
                if self.congress_effect_active("border_control_treaty", "A", &pid.to_string()) {
                    self.culture_bomb(cid, *pos);
                }
                if spec
                    .effects
                    .get("free_heavy_cavalry")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                {
                    self.grant_heavy_cavalry(pid, cid);
                }
                true
            }
            Item::Wonder { wonder, pos } => {
                if self.wonder_built(wonder) {
                    self.cities.get_mut(&cid).unwrap().queue.clear();
                    return false;
                }
                if !self.wonder_sites(cid, wonder).contains(pos) {
                    return false;
                }
                let spec = self.rules.wonders[wonder].clone();
                self.note(
                    pid,
                    "Cities",
                    format!("completed {}", pretty(wonder)),
                    Some(*pos),
                );
                let tile = self.map.tiles.get_mut(pos).unwrap();
                tile.wonder = Some(Name::new(wonder));
                tile.improvement = None;
                tile.pillaged = false;
                tile.feature = None;
                self.cities
                    .get_mut(&cid)
                    .unwrap()
                    .wonders
                    .insert(Name::new(wonder), *pos);
                self.add_historic_moment(pid, self.wonder_historic_moment(wonder));
                if spec
                    .effects
                    .get("promote_all_current_units")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                {
                    for uid in self.player_unit_ids(pid) {
                        if self.rules.units[self.units[&uid].kind].class == "military" {
                            let unit = self.units.get_mut(&uid).unwrap();
                            unit.xp = unit.xp.max(Self::promotion_threshold(unit.level));
                        }
                    }
                }
                self.apply_wonder_completion_effects(pid, cid, wonder, *pos, &spec);
                true
            }
            Item::Repair { repair, pos } => {
                if repair == "district" {
                    self.map.tiles.get_mut(pos).unwrap().pillaged = false;
                    if let Some(mut state) = self.defending_district_state(cid, *pos) {
                        state.hp = 100;
                        state.wall_hp = self.city_max_wall_hp(&self.cities[&cid]);
                        state.pillaged = false;
                        self.set_defending_district_state(cid, state);
                    }
                } else {
                    let city = self.cities.get_mut(&cid).unwrap();
                    let repair = Name::new(repair);
                    city.pillaged_buildings.remove(&repair);
                    city.building_eras.insert(repair, self.world_era);
                }
                true
            }
            Item::Project { project } => {
                if project == "repair_outer_defenses" {
                    return true;
                }
                if project == "repair_encampment" {
                    let max_wall = self.city_max_wall_hp(&self.cities[&cid]);
                    let city = self.cities.get_mut(&cid).unwrap();
                    city.encampment_hp = 100;
                    city.encampment_wall_hp = max_wall;
                    city.encampment_pillaged = false;
                    let position = self.city_district_family_position(
                        &self.cities[&cid],
                        crate::name!("encampment"),
                    );
                    if let Some(tile) = position.and_then(|pos| self.map.tiles.get_mut(&pos)) {
                        tile.pillaged = false;
                    }
                    return true;
                }
                let spec = self.rules.projects[project].clone();
                // The score the project declares only means something while a
                // competition CIVVIS runs itself is open; a mirrored one is
                // counted by the host and must not be counted twice.
                if spec.competition_score > 0.0 && self.victory_eligible(pid) {
                    if let Some(running) = self.competition.as_mut() {
                        if running.ends > self.turn
                            && running.target != Some(pid)
                            && spec
                                .host_competition_kinds()
                                .any(|kind| kind == running.kind)
                        {
                            *running.scores.entry(pid).or_insert(0.0) += spec.competition_score;
                        }
                    }
                }
                if !spec.repeatable
                    && self.players[pid]
                        .science_projects
                        .contains(project.as_str())
                {
                    // Another city won this internal project race.
                    return true;
                }
                if spec.repeatable {
                    bump(&mut self.players[pid], &format!("project:{project}"));
                } else {
                    self.players[pid]
                        .science_projects
                        .insert(project.to_string());
                    self.note_project_founded_moment(pid, project);
                }
                for (kind, points) in self.project_completion_gpp_awards(pid, cid, project.as_str())
                {
                    *self.players[pid].gpp.entry(kind).or_insert(0.0) += points;
                }
                for (effect, amount) in &spec.effects {
                    *self.players[pid]
                        .counters
                        .entry(format!("project_effect:{effect}"))
                        .or_insert(0) += *amount as i64;
                }
                if let Some(loyalty) = spec.effects.get("completion_loyalty") {
                    let city = self.cities.get_mut(&cid).unwrap();
                    city.loyalty = (city.loyalty + loyalty).min(100.0);
                }
                if project == "carbon_recapture" {
                    // Gathering Storm tracks this as lifetime emissions. The
                    // project can drive that total below zero, allowing one
                    // civilization to offset pollution produced by others.
                    self.players[pid].co2_emissions -=
                        1_000.0 * spec.effects.get("co2_recaptured").copied().unwrap_or(0.0);
                    self.players[pid].diplomatic_favor +=
                        spec.effects.get("diplomatic_favor").copied().unwrap_or(0.0);
                }
                if spec
                    .effects
                    .get("reactor_age_reset")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                {
                    self.cities.get_mut(&cid).unwrap().reactor_age = 0;
                }
                let consumed_buildings: Vec<Name> = self.cities[&cid]
                    .buildings
                    .iter()
                    .copied()
                    .filter(|building| {
                        spec.consumes_buildings
                            .iter()
                            .any(|family| self.building_is_family(*building, Name::new(family)))
                    })
                    .collect();
                let consumed_nuclear_plant = consumed_buildings.iter().any(|building| {
                    self.building_is_family(*building, crate::name!("nuclear_power_plant"))
                });
                if !consumed_buildings.is_empty() {
                    let city = self.cities.get_mut(&cid).unwrap();
                    city.buildings
                        .retain(|building| !consumed_buildings.contains(building));
                    city.pillaged_buildings
                        .retain(|building| !consumed_buildings.contains(building));
                    city.building_eras
                        .retain(|building, _| !consumed_buildings.contains(building));
                    if consumed_nuclear_plant {
                        city.reactor_age = 0;
                    }
                }
                if let Some(target) = Self::converted_power_plant(project) {
                    let plants = ["coal_power_plant", "oil_power_plant", "nuclear_power_plant"];
                    let city = self.cities.get_mut(&cid).unwrap();
                    city.buildings
                        .retain(|building| !plants.contains(&building.as_str()));
                    city.pillaged_buildings
                        .retain(|building| !plants.contains(&building.as_str()));
                    city.building_eras
                        .retain(|building, _| !plants.contains(&building.as_str()));
                    city.buildings.push(Name::new(target));
                    city.building_eras.insert(Name::new(target), self.world_era);
                    city.reactor_age = 0;
                }
                if project == "launch_earth_satellite" {
                    self.players[pid]
                        .explored
                        .extend(self.map.tiles.keys().copied());
                }
                if project == "launch_moon_landing" {
                    let bonus = 10.0
                        * self
                            .player_city_ids(pid)
                            .into_iter()
                            .map(|city_id| self.city_yields(city_id).science)
                            .sum::<f64>();
                    let player = &mut self.players[pid];
                    player.culture_lifetime += bonus;
                    if player.civic.is_some() {
                        player.civic_progress += bonus;
                    } else {
                        player.civic_overflow += bonus;
                    }
                }
                if project == "exoplanet_expedition" {
                    self.players[pid].exoplanet_distance = 0.0;
                    // Where it is going is settled now and never again. A
                    // survey that deepens after the ship has left does not turn
                    // it round: the choice is the one that was available on the
                    // day, which is what makes finishing the Moon and Mars
                    // before launching worth anything.
                    let chosen = self.exoplanet_choice(pid);
                    self.players[pid].exoplanet_target = Some(chosen.to_string());
                }
                true
            }
            Item::Product { product } => {
                let Some(target) = self.product_slot_city(pid, cid) else {
                    // A slot can disappear while two cities race the final
                    // Product. Leave the completed project at cost until a
                    // Stock Exchange/Seaport slot becomes available again.
                    return false;
                };
                if self.world_product_count(product) >= 5 {
                    return true;
                }
                self.cities
                    .get_mut(&target)
                    .unwrap()
                    .products
                    .push(product.to_string());
                bump(&mut self.players[pid], "products");
                true
            }
        }
    }

    pub(super) fn apply_training_district_effects(&mut self, cid: u32, uid: u32) {
        let spec = self.rules.units[self.units[&uid].kind].clone();
        if spec.domain.as_deref() == Some("sea") {
            let movement = self.city_district_effect(&self.cities[&cid], "naval_movement");
            self.units.get_mut(&uid).unwrap().bonus_moves += movement;
        }
        let city = &self.cities[&cid];
        let mut xp_pct = if matches!(
            spec.promotion_class.as_str(),
            "melee" | "anti_cavalry" | "ranged"
        ) {
            self.city_building_effect(city, "melee_anti_cavalry_ranged_xp_pct")
        } else if spec.cavalry || spec.siege {
            self.city_building_effect(city, "cavalry_siege_xp_pct")
        } else {
            0.0
        };
        if spec.domain.as_deref() == Some("air") {
            xp_pct += self.city_building_effect(city, "air_unit_xp_pct");
        }
        if spec.domain.as_deref() == Some("sea") && spec.class == "military" {
            xp_pct += self.city_building_effect(city, "naval_unit_xp_pct");
        }
        if spec.domain.as_deref().is_none() && spec.class == "military" {
            xp_pct += self.city_building_effect(city, "all_land_unit_xp_pct");
        }
        let cavalry_movement = if spec.cavalry {
            self.city_building_effect(city, "cavalry_movement")
        } else {
            0.0
        };
        let patron_saint = matches!(self.units[&uid].kind.as_str(), "apostle" | "warrior_monk")
            && self.governor_effect(city.owner, cid, "apostle_extra_promotion") > 0.0;
        let embrasure_promotion = spec.class == "military"
            && !spec.promotion_class.is_empty()
            && self.governor_effect(city.owner, cid, "military_free_promotion") > 0.0;
        // ALLIANCE_FREE_UNIT_UPGRADE is named for something it does not do. Its
        // collection is COLLECTION_ALLIANCE_TRAINED_UNITS and its effect is
        // ADJUST_UNIT_GRANT_EXPERIENCE at -1 -- the same amount every row named
        // FREE_PROMOTION carries, and the same one the Terracotta Army uses. A
        // Level 3 Military Alliance trains its units already promoted.
        let allied_promotion = spec.class == "military"
            && !spec.promotion_class.is_empty()
            && self.alliance_partner(city.owner, "military", 3).is_some();
        let unit = self.units.get_mut(&uid).unwrap();
        if spec.earns_xp {
            unit.xp_bonus_pct += xp_pct;
        }
        if spec.earns_xp && (embrasure_promotion || allied_promotion) {
            // A free promotion is represented by exactly enough XP to expose
            // the first promotion choice. max() prevents free-promotion
            // effects from stacking with one another.
            unit.xp = unit.xp.max(Self::promotion_threshold(unit.level));
        }
        unit.bonus_moves += cavalry_movement;
        unit.extra_first_promotion |= patron_saint;
    }

    pub(super) fn grant_heavy_cavalry(&mut self, pid: usize, cid: u32) {
        let mut candidates: Vec<Name> = self
            .rules
            .units
            .iter()
            .filter(|(_, spec)| spec.promotion_class == "heavy_cavalry")
            .filter(|(_, spec)| self.unlocked(pid, &spec.tech, &spec.civic))
            .filter(|(name, _)| self.player_unit_replacement(pid, name) == **name)
            .filter(|(_, spec)| {
                spec.unique_to
                    .as_deref()
                    .is_none_or(|civilization| self.owns_civ_unique(pid, civilization))
            })
            .map(|(name, _)| *name)
            .collect();
        candidates.sort_by(|a, b| {
            self.rules.units[b]
                .cost
                .partial_cmp(&self.rules.units[a].cost)
                .unwrap()
                .then(a.cmp(b))
        });
        if let Some(kind) = candidates.first() {
            if let Some(unit) = self.place_new_unit(kind, pid, self.cities[&cid].pos) {
                self.apply_training_district_effects(cid, unit);
            }
        }
    }

    pub(super) fn place_new_unit(&mut self, kind: &str, owner: usize, pos: Pos) -> Option<u32> {
        let spec = self.rules.units[kind].clone();
        if spec.domain.as_deref() == Some("air") {
            let mut bases = Vec::new();
            if let Some(city_id) = self.city_at(pos) {
                let city = &self.cities[&city_id];
                bases.extend(city.districts.iter().filter_map(|(district, position)| {
                    (self.district_is_family(district, crate::name!("aerodrome"))
                        && self.district_is_active(city, district, *position))
                    .then_some(*position)
                }));
            }
            bases.push(pos);
            bases.extend(self.nbrs(pos));
            bases.sort();
            bases.dedup();
            bases.sort_by_key(|base| usize::from(*base == pos));
            if let Some(base) = bases
                .into_iter()
                .find(|base| self.can_air_base_at(owner, *base, None))
            {
                return Some(self.spawn_unit(kind, owner, base));
            }
            return None;
        }
        let want_sea = spec.domain.as_deref() == Some("sea");
        let mut cands = vec![pos];
        cands.extend(self.nbrs(pos));
        for cand in cands {
            let t = match self.map.get(cand) {
                Some(t) => t,
                None => continue,
            };
            if !self.rules.is_passable(t) || self.rules.is_water(t) != want_sea {
                continue;
            }
            let mut occupied = false;
            for oid in self.unit_ids_at(cand) {
                let o = &self.units[oid];
                if o.owner != owner || self.rules.units[o.kind].class == spec.class {
                    occupied = true;
                    break;
                }
            }
            if let Some(ccid) = self.city_at(cand) {
                if self.cities[&ccid].owner != owner {
                    occupied = true;
                }
            }
            if !occupied {
                return Some(self.spawn_unit(kind, owner, cand));
            }
        }
        None
    }
}
