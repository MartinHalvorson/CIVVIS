//! Barbarians: their camps, raids and scouts.
//!
//! The barbarian spec and difficulty, camp placement and spawning
//! (`spawn_camp`, `spawn_barbarian_camp_recon`/`_units`), the unit pools
//! (land, naval, guards, raid forces and their ranged share), the scout and
//! raid phases (`barbarian_scout_phase`, `barbarian_phase`), and clearing or
//! converting a camp.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// The rung the barbarian seat plays by: its raid band and spawn cadence.
    /// See [`default_barbarian_difficulty`]. Falls back to the seat's rung if
    /// a save names a rung this ruleset does not have.
    pub fn barbarian_spec(&self) -> &DifficultySpec {
        self.rules
            .difficulties
            .get(&self.barbarian_difficulty)
            .unwrap_or_else(|| self.difficulty_spec())
    }

    /// Set the barbarian seat's rung; refused for a key the ruleset lacks.
    pub fn set_barbarian_difficulty(&mut self, key: &str) -> Result<(), String> {
        if !self.rules.difficulties.contains_key(key) {
            return Err(format!("unknown barbarian difficulty {key}"));
        }
        self.barbarian_difficulty = key.to_string();
        Ok(())
    }

    /// Below Prince a human is paid more for clearing a Barbarian camp.
    pub(super) fn human_camp_gold(&self, pid: usize) -> f64 {
        if self.is_human_seat(pid) && self.takes_handicap(pid) {
            self.difficulty_spec().human_camp_gold
        } else {
            0.0
        }
    }

    /// Shipped `BARBARIAN_CAMP_MAX_PER_MAJOR_CIV` is 3, so the world aims for
    /// three camps per major civilization — twenty-four on the tournament
    /// lobby's eight-player map, not nine.
    pub(super) fn barbarian_camp_target(&self) -> usize {
        3 * self.players.iter().filter(|p| !p.is_minor).count()
    }

    /// Everything any non-Barbarian unit or city can currently see.
    ///
    /// The engine's own terrain-aware sight rather than a disk around each
    /// pair of eyes, so a camp seated outside it really is out of sight — and
    /// within one state of the world the per-unit answers come back from the
    /// vision cache the turn already filled.
    pub(super) fn watched_by_the_living(&self) -> TileBits {
        let mut heights = self.height_field();
        let mut watched = TileBits::with_capacity(self.map.tiles.len());
        for pid in 0..self.players.len() {
            if Some(pid) == self.barb_pid {
                continue;
            }
            watched.union_with(&self.player_vision(&mut heights, pid));
        }
        watched
    }

    fn spawn_camp(&mut self) {
        let watched = self.watched_by_the_living();
        self.spawn_camp_unwatched(&watched);
    }

    /// Seat one camp, given what the world can already see. Setup places a
    /// third of the target in one go and nothing moves between those
    /// placements, so it works the sight out once and passes it in rather
    /// than re-sweeping the whole world per camp — the difference between one
    /// visibility pass and twenty on a globe seating twenty-one civilizations.
    pub(super) fn spawn_camp_unwatched(&mut self, watched: &TileBits) {
        // The shipped placement floors: 4 tiles from any city, 7 from another
        // camp. Both are clearance radii, so they are read off a disk around
        // each city and camp rather than measured from every tile on the map.
        // The distinction is the whole cost of the routine on a globe: a flat
        // map answers `wdist` with arithmetic, but a sphere past its cached
        // ring answers with an A* search of the world, and asking that once
        // per land tile per city never finished. A land-heavy globe seating 30
        // civilizations hung in setup, before turn one, because a third of the
        // camp target is placed there. A disk of radius 3 or 6 stays inside
        // the sphere's precomputed rings, so it is a table lookup either way.
        let mut blocked: HashSet<Pos> = HashSet::new();
        for city in self.cities.values() {
            blocked.extend(self.wdisk(city.pos, BARBARIAN_CAMP_MINIMUM_DISTANCE_CITY - 1));
        }
        // A civilization that has not founded yet is still a civilization. At
        // setup every major is a Settler standing on the plot that becomes its
        // capital next turn and only the city-states hold cities, so a floor
        // measured from cities alone left the starts — the one place on the
        // map a camp must never be — guarded by nothing but a Settler's own
        // sight, and opening camps duly landed three tiles off them. A Settler
        // in the field is a city that has not happened yet, and it clears the
        // same four tiles.
        for unit in self.units.values() {
            if unit.kind == "settler" && !self.players[unit.owner].is_barbarian {
                blocked.extend(self.wdisk(unit.pos, BARBARIAN_CAMP_MINIMUM_DISTANCE_CITY - 1));
            }
        }
        for camp in self.barb_camps.keys() {
            blocked.extend(self.wdisk(*camp, BARBARIAN_CAMP_MINIMUM_DISTANCE_ANOTHER_CAMP - 1));
        }
        // And the rule that decides where a camp may be at all: nowhere any
        // non-Barbarian unit or city is looking. A camp is seated in the fog
        // and found by exploring; it is never watched into existence next to
        // a Warrior standing in the open, which is what a placement that only
        // measured distance to *cities* allowed. It is also what keeps the
        // world's camp count honest as the map fills — Civ VI seats fewer
        // camps late not because it wants to but because there is less dark
        // ground left to seat them on.
        let mut cands: Vec<Pos> = Vec::new();
        for (pos, t) in &self.map.tiles {
            if self.rules.is_water(t) || !self.rules.is_passable(t) {
                continue;
            }
            if t.owner_city.is_some()
                || t.improvement.is_some()
                || self.city_by_pos.contains_key(pos)
                || !self.unit_ids_at(*pos).is_empty()
            {
                continue;
            }
            if blocked.contains(pos) || self.sees(watched, *pos) {
                continue;
            }
            cands.push(*pos);
        }
        if cands.is_empty() {
            return;
        }
        let pos = cands[self.rng.below(cands.len())];
        self.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("barbarian_camp"));
        let coastal = self.nbrs(pos).into_iter().any(|neighbor| {
            self.map
                .get(neighbor)
                .is_some_and(|tile| self.rules.is_water(tile))
        });
        if coastal && self.rng.below(BARBARIAN_CAMP_COASTAL_SPAWN_ROLL) == 0 {
            self.barb_naval_camps.insert(pos);
        }
        self.barb_camps.insert(pos, self.turn + 2);
        self.spawn_barbarian_camp_units(pos);
    }

    /// The outpost assignment held by a barbarian unit. Scouts, standing
    /// guards, and raiders all use the same durable home relationship; callers
    /// may still supply a legacy nearest-camp fallback for an old save that
    /// predates raider assignments.
    pub(crate) fn barbarian_unit_home(&self, uid: u32) -> Option<Pos> {
        self.barb_camp_guards
            .iter()
            .find_map(|(camp, guard)| (*guard == uid).then_some(*camp))
            .or_else(|| self.barb_scout_homes.get(&uid).copied())
            .or_else(|| self.barb_raider_homes.get(&uid).copied())
    }

    /// `BarbarianAttackForces` supplies one melee attacker below Warlord,
    /// two melee plus one ranged unit through Emperor, and three melee plus
    /// two ranged units at Immortal and Deity. The difficulty data carries
    /// those three force bands as 0.5, 1.0, and 1.5 respectively — read from
    /// the barbarian seat's own rung (`barbarian_spec`), not the majors'.
    pub(super) fn barbarian_raid_force_size(&self) -> usize {
        match self.barbarian_spec().barb_force_scale {
            scale if scale <= 0.5 => 1,
            scale if scale <= 1.0 => 3,
            _ => 5,
        }
    }

    pub(super) fn barbarian_raid_ranged_size(&self) -> usize {
        self.barbarian_raid_force_size() / 2
    }

    pub(super) fn barbarian_scout_phase(&mut self, bpid: usize) {
        self.barb_scout_homes.retain(|unit, camp| {
            self.units
                .get(unit)
                .is_some_and(|scout| scout.owner == bpid)
                && self.barb_camps.contains_key(camp)
        });
        self.barb_scout_targets.retain(|unit, _| {
            self.units
                .get(unit)
                .is_some_and(|scout| scout.owner == bpid)
        });
        // Only the recon unit assigned to an outpost searches and reports.
        // Later-raised Scouts are ordinary raiders, while a naval camp's
        // Galley occupies this same role on water.
        let scouts: Vec<u32> = self.barb_scout_homes.keys().copied().collect();
        for unit in scouts {
            let position = self.units[&unit].pos;
            let Some(home) = self.barb_scout_homes.get(&unit).copied().or_else(|| {
                self.barb_camps
                    .keys()
                    .min_by_key(|camp| self.wdist(position, **camp))
                    .copied()
            }) else {
                continue;
            };
            self.barb_scout_homes.insert(unit, home);
            // A reported target is one raid, not a permanent alarm. The Scout
            // waits beside its camp while that party is raised, then goes back
            // out after the alert has expired. Without this hold it could see
            // the same city again on the next turn and renew the wave forever.
            if self
                .barb_alerted_until
                .get(&home)
                .is_some_and(|until| *until > self.turn)
            {
                self.barb_scout_targets.remove(&unit);
                if self.wdist(position, home) > 1 {
                    if let Some(step) = self.route_step(unit, home, 1) {
                        self.relocate(unit, step);
                    }
                }
                continue;
            }
            if !self.barb_scout_targets.contains_key(&unit) {
                let visible = self.unit_visible_tiles(unit);
                // ★★★★★ A SETTLER WALKING PAST A CAMP WAS NOT A SIGHTING, SO
                // NOTHING A CAMP EVER DID WAS TRIGGERED BY THE THING WORTH
                // RAIDING.
                //
                // Only a CITY could be reported, so a camp's entire raid
                // throughput was one Scout's round trip to a settlement and
                // back — and an empire's walkers, which is what a
                // Civilization VI barbarian actually takes, were invisible to
                // the pipeline that decides whether a camp raises anybody at
                // all. MEASURED with the report as cities-only, `ai_eval`
                // 72 seat-games an arm at 6p/150t/online: 0.47 civilians lost
                // to barbarians per game, against 8 Settlers in 104 turns on
                // the live Civilization VI seat — a seventeen-fold gap that
                // no tuning value inside the alert window can close, because
                // the window mostly never opens near an expanding empire.
                //
                // A Civilization VI Scout reports what it SEES. So does this
                // one now: the nearest visible major settlement or unit,
                // whichever is closer. Everything downstream is unchanged —
                // the Scout still has to walk home before the camp is
                // alerted, the alert still expires, and the party size still
                // comes from the difficulty band.
                let city = self
                    .cities
                    .values()
                    .filter(|city| {
                        let owner = &self.players[city.owner];
                        owner.alive && !owner.is_minor && !owner.is_barbarian
                    })
                    .filter(|city| visible.contains(&city.pos))
                    .map(|city| (self.wdist(position, city.pos), city.pos));
                // A Scout that has to walk home before anything happens cannot
                // chase; the sighting is a REPORT, and the position it carries
                // is where our people were standing when it saw them.
                let quarry = self
                    .units
                    .values()
                    .filter(|other| {
                        let owner = &self.players[other.owner];
                        owner.alive && !owner.is_minor && !owner.is_barbarian
                    })
                    .filter(|other| visible.contains(&other.pos))
                    .map(|other| (self.wdist(position, other.pos), other.pos));
                if let Some((_, target)) = city.chain(quarry).min() {
                    self.barb_scout_targets.insert(unit, target);
                }
            }
            if let Some(target) = self.barb_scout_targets.get(&unit).copied() {
                if self.wdist(position, home) <= 1 {
                    self.barb_alerted_until
                        .insert(home, self.turn + BARBARIAN_SCOUT_ALERT_TURNS);
                    self.barb_camp_targets.insert(home, target);
                    self.barb_camps.insert(home, self.turn + 1);
                    self.barb_scout_targets.remove(&unit);
                    continue;
                }
                if let Some(step) = self.route_step(unit, home, 1) {
                    self.relocate(unit, step);
                }
                continue;
            }

            // An unalerted Scout actively searches toward the nearest major
            // settlement until it reaches sight range, then reports home.
            let sight = self.unit_sight(unit);
            let nearest = self
                .cities
                .values()
                .filter(|city| {
                    let owner = &self.players[city.owner];
                    owner.alive && !owner.is_minor && !owner.is_barbarian
                })
                .min_by_key(|city| (self.wdist(position, city.pos), city.id))
                .map(|city| city.pos);
            if let Some(step) = nearest.and_then(|target| self.route_step(unit, target, sight)) {
                self.relocate(unit, step);
            }
        }
    }

    /// The shipped rule: Barbarians fight with half the leading
    /// civilization's technology (BARBARIAN_TECH_PERCENT). Rank the tech
    /// tree the way research walks it and give the camps everything the
    /// first half of the leader's tech count unlocks - muskets and tanks
    /// included, late enough.
    pub(crate) fn barbarian_unit_pool(&self) -> Vec<Name> {
        let leader_techs = self
            .players
            .iter()
            .filter(|p| !p.is_minor)
            .map(|p| p.techs.len())
            .max()
            .unwrap_or(0);
        let mut ranked: Vec<(&Name, &crate::rules::TechSpec)> = self.rules.techs.iter().collect();
        ranked
            .sort_by(|a, b| (a.1.era, a.1.cost as i64, a.0).cmp(&(b.1.era, b.1.cost as i64, b.0)));
        let known: BTreeSet<&str> = ranked
            .iter()
            .take(leader_techs * 50 / 100)
            .map(|(name, _)| name.as_str())
            .collect();
        self.rules
            .units
            .iter()
            .filter(|(_, spec)| {
                spec.class == "military"
                    && spec.buildable
                    && spec.unique_to.is_none()
                    && !matches!(spec.domain.as_deref(), Some("sea") | Some("air"))
                    && spec.promotion_class != "support"
                    && spec.promotion_class != "recon"
                    && spec.civic.is_none()
                    && spec.tech.as_deref().is_none_or(|tech| known.contains(tech))
            })
            .map(|(name, _)| *name)
            .collect()
    }

    /// The camp's standing defender is the strongest non-unique anti-cavalry
    /// unit available in the current world era. Unlike the ordinary raider
    /// pool, this deliberately uses era rather than the leaders' researched
    /// techs: a fresh outpost still opens with its era's Spearman/Pikeman.
    fn barbarian_guard_unit(&self) -> Name {
        self.rules
            .units
            .iter()
            .filter(|(_, spec)| {
                spec.class == "military"
                    && spec.buildable
                    && spec.unique_to.is_none()
                    && !matches!(spec.domain.as_deref(), Some("sea") | Some("air"))
                    && spec.promotion_class == "anti_cavalry"
                    && spec.civic.is_none()
                    && spec.tech.as_deref().is_none_or(|tech| {
                        self.rules
                            .techs
                            .get(tech)
                            .is_some_and(|tech| tech.era <= self.world_era)
                    })
            })
            .max_by_key(|(name, spec)| {
                let era = spec
                    .tech
                    .as_deref()
                    .and_then(|tech| self.rules.techs.get(tech))
                    .map_or(0, |tech| tech.era);
                (era, spec.strength.round() as i64, *name)
            })
            .map(|(name, _)| *name)
            .unwrap_or_else(|| crate::name!("spearman"))
    }

    /// Naval outposts use the same technology ceiling as land camps, but a
    /// Galley is always eligible for the special recon slot even before any
    /// major civilization has researched Sailing.
    fn barbarian_naval_unit_pool(&self) -> Vec<Name> {
        let leader_techs = self
            .players
            .iter()
            .filter(|p| !p.is_minor)
            .map(|p| p.techs.len())
            .max()
            .unwrap_or(0);
        let mut ranked: Vec<(&Name, &crate::rules::TechSpec)> = self.rules.techs.iter().collect();
        ranked
            .sort_by(|a, b| (a.1.era, a.1.cost as i64, a.0).cmp(&(b.1.era, b.1.cost as i64, b.0)));
        let known: BTreeSet<&str> = ranked
            .iter()
            .take(leader_techs * 50 / 100)
            .map(|(name, _)| name.as_str())
            .collect();
        self.rules
            .units
            .iter()
            .filter(|(name, spec)| {
                spec.class == "military"
                    && spec.buildable
                    && spec.unique_to.is_none()
                    && spec.domain.as_deref() == Some("sea")
                    && spec.promotion_class != "support"
                    && spec.civic.is_none()
                    && (name.as_str() == "galley"
                        || spec.tech.as_deref().is_none_or(|tech| known.contains(tech)))
            })
            .map(|(name, _)| *name)
            .collect()
    }

    fn choose_barbarian_unit(&mut self, pool: &[Name]) -> Option<Name> {
        if pool.is_empty() {
            return None;
        }
        (0..3)
            .map(|_| pool[self.rng.below(pool.len())])
            .max_by(|a, b| {
                let strength = |kind: &str| {
                    let spec = &self.rules.units[kind];
                    spec.strength.max(spec.ranged_strength) as i64
                };
                strength(a).cmp(&strength(b)).then_with(|| b.cmp(a))
            })
    }

    /// Build the documented melee/ranged mix of a reported raid where the
    /// current pool permits it. A coastal camp may not have a ranged ship at
    /// this technology, so it falls back to an otherwise legal hull instead
    /// of stalling the entire party.
    fn choose_barbarian_raid_unit(&mut self, pool: &[Name], want_ranged: bool) -> Option<Name> {
        let role_pool: Vec<Name> = pool
            .iter()
            .copied()
            .filter(|kind| self.rules.units[kind].has_ranged_attack() == want_ranged)
            .collect();
        self.choose_barbarian_unit(if role_pool.is_empty() {
            pool
        } else {
            &role_pool
        })
    }

    fn spawn_barbarian_camp_recon(&mut self, kind: &str, owner: usize, camp: Pos) -> Option<u32> {
        let want_sea = self.rules.units[kind].domain.as_deref() == Some("sea");
        self.nbrs(camp).into_iter().find_map(|position| {
            let tile = self.map.get(position)?;
            if !self.rules.is_passable(tile) || self.rules.is_water(tile) != want_sea {
                return None;
            }
            if self.city_at(position).is_some()
                || self
                    .unit_ids_at(position)
                    .iter()
                    .any(|unit| self.rules.units[self.units[unit].kind].class == "military")
            {
                return None;
            }
            Some(self.spawn_unit(kind, owner, position))
        })
    }

    pub(super) fn spawn_barbarian_camp_units(&mut self, camp: Pos) {
        let Some(bpid) = self.barb_pid else {
            return;
        };
        if self.barb_camp_guards.contains_key(&camp) {
            return;
        }
        let guard = self.barbarian_guard_unit();
        let guard_id = self.spawn_unit(guard.as_str(), bpid, camp);
        if let Some(unit) = self.units.get_mut(&guard_id) {
            unit.fortified = true;
            unit.moves_left = 0.0;
            unit.fortify_turns = 1;
            unit.moved = false;
            unit.acted = false;
        }
        self.barb_camp_guards.insert(camp, guard_id);

        let naval = self.barb_naval_camps.contains(&camp);
        let recon_kind = if naval && self.rules.units.contains_key("galley") {
            "galley"
        } else {
            "scout"
        };
        let recon = self
            .spawn_barbarian_camp_recon(recon_kind, bpid, camp)
            .or_else(|| self.place_new_unit(recon_kind, bpid, camp));
        if let Some(unit) = recon {
            self.barb_scout_homes.insert(unit, camp);
        }
    }

    pub(super) fn barbarian_phase(&mut self) {
        let bpid = match self.barb_pid {
            Some(p) => p,
            None => return,
        };
        self.barb_alerted_until
            .retain(|camp, until| self.barb_camps.contains_key(camp) && *until > self.turn);
        self.barb_camp_targets
            .retain(|camp, _| self.barb_alerted_until.contains_key(camp));
        self.barb_naval_camps
            .retain(|camp| self.barb_camps.contains_key(camp));
        self.barb_camp_guards.retain(|camp, guard| {
            self.barb_camps.contains_key(camp)
                && self
                    .units
                    .get(guard)
                    .is_some_and(|unit| unit.owner == bpid && unit.pos == *camp)
        });
        self.barb_raider_homes.retain(|unit, camp| {
            self.barb_camps.contains_key(camp)
                && self
                    .units
                    .get(unit)
                    .is_some_and(|raider| raider.owner == bpid)
        });
        self.barbarian_scout_phase(bpid);
        // Setup puts a third of the target on the map before turn one
        // (BARBARIAN_CAMP_FIRST_TURN_PERCENT_OF_TARGET_TO_ADD); from there the
        // shipped world rolls one in `BARBARIAN_CAMP_ODDS_OF_NEW_CAMP_SPAWNING`
        // *every* turn for one more, so the rest of the target arrives over the
        // opening dozens of turns and the first camps a civilization meets are
        // ones that appeared while it was walking toward them. This engine used
        // to try once every ten turns instead, which asked the eight-player
        // lobby to reach a target of 24 camps in 220 turns — in a 500-turn game
        // the map effectively never left its opening pair.
        if self.barb_camps.len() < self.barbarian_camp_target()
            && self.rng.below(BARBARIAN_CAMP_ODDS_OF_NEW_CAMP_SPAWNING) == 0
        {
            self.spawn_camp();
        }
        // A reported camp raises exactly its own party. The old global cap
        // let distant, unrelated camps consume the slots that a Scout had just
        // earned for this target, then let the successful camp continue adding
        // units after the party was already complete.
        let force_size = self.barbarian_raid_force_size();
        let ranged_size = self.barbarian_raid_ranged_size();
        let spawn_scale = self.barbarian_spec().barb_spawn_scale;
        let land_pool = self.barbarian_unit_pool();
        let naval_pool = self.barbarian_naval_unit_pool();
        let camps: Vec<(Pos, u32)> = self.barb_camps.iter().map(|(p, n)| (*p, *n)).collect();
        for (pos, nxt) in camps {
            let alerted = self
                .barb_alerted_until
                .get(&pos)
                .is_some_and(|until| *until > self.turn)
                && self.barb_camp_targets.contains_key(&pos);
            if self.turn < nxt || !alerted {
                continue;
            }
            let party: Vec<u32> = self
                .barb_raider_homes
                .iter()
                .filter_map(|(unit, home)| (*home == pos).then_some(*unit))
                .collect();
            if party.len() >= force_size {
                continue;
            }
            let ranged = party
                .iter()
                .filter(|unit| {
                    self.units
                        .get(unit)
                        .is_some_and(|raider| self.rules.units[raider.kind].has_ranged_attack())
                })
                .count();
            let melee = party.len().saturating_sub(ranged);
            let want_ranged = ranged < ranged_size && melee >= force_size - ranged_size;
            let pool = if self.barb_naval_camps.contains(&pos) {
                &naval_pool
            } else {
                &land_pool
            };
            let Some(utype) = self.choose_barbarian_raid_unit(pool, want_ranged) else {
                continue;
            };
            if let Some(unit) = self.place_new_unit(utype.as_str(), bpid, pos) {
                self.barb_raider_homes.insert(unit, pos);
                // BarbarianAttackForces.SpawnRate is 2 up to Emperor and 1
                // from Immortal, so the top band assembles forces twice as
                // often as well as fielding bigger ones.
                let wait = (2.0 * spawn_scale).round().max(1.0) as u32;
                self.barb_camps.insert(pos, self.turn + wait);
            }
        }
    }

    pub(super) fn maybe_clear_camp(&mut self, uid: u32) {
        let pos = self.units[&uid].pos;
        self.clear_barbarian_camp(uid, pos, false);
    }

    pub(super) fn clear_barbarian_camp(&mut self, uid: u32, pos: Pos, coastal: bool) -> bool {
        let (owner, kind) = {
            let unit = &self.units[&uid];
            (unit.owner, unit.kind)
        };
        if !self.barb_camps.contains_key(&pos)
            || Some(owner) == self.barb_pid
            || self.rules.units[kind].class != "military"
        {
            return false;
        }
        self.barb_camps.remove(&pos);
        self.barb_naval_camps.remove(&pos);
        self.barb_camp_guards.remove(&pos);
        self.barb_alerted_until.remove(&pos);
        self.barb_camp_targets.remove(&pos);
        self.barb_scout_homes.retain(|_, camp| *camp != pos);
        self.barb_raider_homes.retain(|_, camp| *camp != pos);
        let tile = self.map.tiles.get_mut(&pos).unwrap();
        if tile.improvement.as_deref() == Some("barbarian_camp") {
            tile.improvement = None;
        }
        self.players[owner].gold += 50.0 + self.human_camp_gold(owner);
        // Initiation Rites pays for the camp twice, and Gathering Storm's
        // second half is the one a base-game reading misses:
        // `INITIATION_RITES_FAITH_DISPERSAL_MODIFIER`
        // (`EFFECT_ADJUST_PLAYER_FAITH_FROM_DISPERSAL`, Amount 50) is the base
        // game's, and `INITIATION_RITES_HEALING_DISPERSAL_MODIFIER`
        // (`EFFECT_ADJUST_PLAYER_HEALING_FROM_DISPERSAL`, Amount 100) is added
        // by `Expansion2_Beliefs.xml` — "the unit that cleared the Barbarian
        // Outpost heals +100 HP". Neither id is in `Expansion2_RemoveData.xml`.
        self.players[owner].faith += self.pantheon_effect(owner, "camp_cleared_faith");
        let camp_heal = self.pantheon_effect(owner, "camp_cleared_heal");
        if camp_heal > 0.0 {
            if let Some(unit) = self.units.get_mut(&uid) {
                unit.hp = (unit.hp + camp_heal.round() as i32).min(100);
            }
        }
        if coastal {
            let loot = self.promotion_effect(&self.units[&uid], "coastal_raid_gold");
            self.players[owner].gold += self.game_speed.scale(loot);
        }
        let near_city = self
            .cities
            .values()
            .any(|city| city.owner == owner && self.wdist(city.pos, pos) <= 6);
        self.add_historic_moment(
            owner,
            if near_city {
                "MOMENT_BARBARIAN_CAMP_DESTROYED_NEAR_YOUR_CITY"
            } else {
                "MOMENT_BARBARIAN_CAMP_DESTROYED"
            },
        );
        if self.has_ability(owner, "epic_quest") {
            // Epic Quest: a full tribal village reward for the cleared camp.
            self.roll_goody_reward(owner, uid, pos);
        }
        bump(&mut self.players[owner], "camps");
        true
    }

    // ------------------------------------------------------------- queries

    pub(super) fn do_convert_barbarians(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let apostle = self.own_unit(pid, uid)?;
        if apostle.kind != "apostle"
            || apostle.moves_left <= 0.0
            || apostle.charges <= 0
            || self.promotion_effect(&apostle, "convert_barbarians") <= 0.0
        {
            return Err("Apostle cannot convert Barbarians".into());
        }
        let Some(barbarian) = self.barb_pid else {
            return Err("this game has no Barbarians".into());
        };
        let targets: Vec<u32> = self
            .wdisk(apostle.pos, 1)
            .into_iter()
            .flat_map(|position| self.units_at(position))
            .filter(|other| self.units[other].owner == barbarian)
            .collect();
        if targets.is_empty() {
            return Err("no adjacent Barbarian units".into());
        }
        for target in targets {
            self.transfer_unit_owner(target, pid);
        }
        let apostle = self.units.get_mut(&uid).unwrap();
        apostle.charges -= 1;
        apostle.moves_left = 0.0;
        apostle.acted = true;
        if apostle.charges <= 0 {
            self.remove_unit(uid);
        }
        Ok(())
    }
}
