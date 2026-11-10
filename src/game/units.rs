//! Units: creating them, removing them, and moving them between owners and
//! tiles outside an ordinary move.
//!
//! Creating a unit (`spawn_unit`, and `spawn_test_unit` for fixtures) with
//! its builder charges; removing one (`remove_unit`, a combined unit's
//! parts, `remove_unit_recording_lifetime` with the lifetime stats it
//! records) and the accounting cost a unit carries; setting its formation;
//! handing it to another owner (`transfer_unit_owner`); and placing it on a
//! tile it did not walk to (`relocate`, `forced_retreat_tile`).
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! and `pub(crate)` methods are unchanged; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Charges a Builder trained by this player would carry today: the
    /// ruleset base plus building, wonder, Serfdom and Dynastic Cycle
    /// bonuses. `spawn_unit` applies exactly this sum, so a production
    /// valuation can price the Builder it is deciding whether to train by
    /// the number of jobs that Builder will actually be able to do. The
    /// governor placement bonus (Liang) is applied at spawn position and is
    /// deliberately absent here — production does not know where the unit
    /// will stand.
    pub fn builder_charges(&self, owner: usize) -> i32 {
        let mut charges = self.rules.units["builder"].charges;
        charges += self.empire_building_sum(owner, |b| b.builder_charges as f64) as i32;
        charges += self.empire_wonder_effect(owner, "builder_charges") as i32;
        if self.has_policy(owner, "serfdom") {
            charges += 2;
        }
        if self.has_ability(owner, "dynastic_cycle") {
            charges += 1; // China: First Emperor
        }
        charges
    }

    /// `pub(crate)` for `oracle.rs`, which places units directly the same way
    /// `relocate` was widened for it. Nothing outside the crate can reach it.
    pub(crate) fn spawn_unit(&mut self, kind: &str, owner: usize, pos: Pos) -> u32 {
        let spec = &self.rules.units[kind];
        if spec.class == "military" {
            let best = self.players[owner]
                .counters
                .entry("strongest_unit_built".to_string())
                .or_insert(0);
            *best = (*best).max(spec.strength.round() as i64);
            if spec.ranged_strength > 0.0 {
                let best_ranged = self.players[owner]
                    .counters
                    .entry("strongest_ranged_built".to_string())
                    .or_insert(0);
                *best_ranged = (*best_ranged).max(spec.ranged_strength.round() as i64);
            }
        }
        let mut charges = if kind == "builder" {
            self.builder_charges(owner)
        } else {
            spec.charges
        };
        if spec.religious_spread > 0.0 {
            charges += self.empire_wonder_effect(owner, "religious_spread_charges") as i32;
            if let Some(city_id) = self.city_at(pos) {
                if self.cities[&city_id].owner == owner {
                    charges += self.city_building_effect(
                        &self.cities[&city_id],
                        "religious_unit_spread_charges",
                    ) as i32;
                }
            }
            if self.dedication_active(owner, "exodus_of_the_evangelists") {
                charges += 2;
            }
        }
        let religion = if spec.class == "religious" {
            // Stock rule: purchased religious units belong to the majority
            // religion of their city, which is how civilizations that never
            // founded a religion still field Missionaries of an adopted
            // faith. Only the owner's own city majority applies (purchases
            // happen at home); the founder fallback covers spawns outside a
            // majority city (a fresh Holy City, scripted spawns).
            self.city_at(pos)
                .map(|city_id| &self.cities[&city_id])
                .filter(|city| city.owner == owner)
                .and_then(|city| self.city_religion(city))
                .map(str::to_string)
                .or_else(|| self.players[owner].religion.clone())
        } else if kind == "warrior_monk" {
            self.city_at(pos)
                .and_then(|city_id| self.city_religion(&self.cities[&city_id]))
                .map(str::to_string)
                .or_else(|| self.players[owner].religion.clone())
        } else {
            None
        };
        let mut u = Unit {
            id: self.next_id,
            kind: Name::new(kind),
            owner,
            pos,
            hp: 100,
            damage_dealt: 0,
            production_cost: spec.cost,
            moves_left: spec.moves + self.civ_effect(owner, "unit_movement"),
            charges,
            xp: 0,
            album_sales: 0,
            level: 1,
            promotions: BTreeSet::new(),
            extra_first_promotion: false,
            free_upkeep: false,
            formation: 0,
            linked_to: None,
            religion,
            attacks_left: 1,
            moved: false,
            fortified: false,
            fortify_turns: 0,
            acted: false,
            zoc_stopped: false,
            host_embarked: None,
            started_turn_in_zoc: false,
            air_patrol: false,
            air_patrol_pos: None,
            bonus_moves: 0.0,
            xp_bonus_pct: 0.0,
            levied_from: None,
            levied_until: 0,
        };
        if matches!(kind, "apostle" | "rock_band") {
            // Apostles and Rock Bands choose one promotion immediately after
            // purchase instead of first earning combat experience.
            u.xp = Self::promotion_threshold(1);
        }
        if spec.class == "military"
            && !spec.promotion_class.is_empty()
            && self.alliance_partner(owner, "military", 3).is_some()
        {
            u.xp = u.xp.max(Self::promotion_threshold(u.level));
        }
        if kind == "apostle" && self.empire_wonder_effect(owner, "apostles_gain_martyr") > 0.0 {
            u.promotions.insert(crate::name!("martyr"));
        }
        self.next_id += 1;
        let id = u.id;
        let sight = spec.sight;
        self.occ.entry(pos).or_default().push(id);
        self.units.insert(id, u);
        self.reveal(owner, pos, sight);
        self.note_unit_created_moments(owner, kind);
        id
    }

    /// A working Missile Silo for a nuclear test. It stands on the city's own
    /// tile so that the distances a fixture already measured still hold.
    #[cfg(test)]
    pub(crate) fn install_test_silo(&mut self, city: u32) {
        let position = self.cities[&city].pos;
        let tile = self.map.tiles.get_mut(&position).expect("the city's tile");
        tile.improvement = Some(crate::name!("missile_silo"));
        tile.pillaged = false;
        let owned = &mut self.cities.get_mut(&city).expect("the city").owned_tiles;
        if !owned.contains(&position) {
            owned.push(position);
        }
    }

    #[cfg(test)]
    pub(crate) fn spawn_test_unit(&mut self, kind: &str, owner: usize, pos: Pos) -> u32 {
        self.spawn_unit(kind, owner, pos)
    }

    pub(crate) fn remove_unit(&mut self, uid: u32) {
        self.remove_unit_recording_lifetime(uid, true);
    }

    /// Remove a constituent whose identity continues in a combined formation.
    /// Its damage and Production are transferred to the surviving unit first,
    /// so recording a second completed life here would count the same
    /// investment twice.
    pub(super) fn remove_combined_unit(&mut self, uid: u32) {
        self.remove_unit_recording_lifetime(uid, false);
    }

    pub(super) fn unit_accounting_cost(&self, unit: &Unit) -> f64 {
        if unit.production_cost > 0.0 {
            return unit.production_cost;
        }
        // Saves written before lifetime accounting have no stored investment.
        // Reconstruct the nominal value rather than turning all of their
        // surviving units into zero-cost infinite returns.
        self.rules.units[unit.kind].cost
            * match unit.formation {
                0 => 1.0,
                1 => 1.5,
                _ => 2.0,
            }
    }

    pub(super) fn set_unit_formation(&mut self, uid: u32, formation: u8) {
        let kind = self.units[&uid].kind;
        let cost = self.rules.units[kind].cost
            * match formation {
                0 => 1.0,
                1 => 1.5,
                _ => 2.0,
            };
        let unit = self.units.get_mut(&uid).unwrap();
        unit.formation = formation;
        unit.production_cost = cost;
    }

    fn remove_unit_recording_lifetime(&mut self, uid: u32, record_lifetime: bool) {
        self.barb_scout_homes.remove(&uid);
        self.barb_raider_homes.remove(&uid);
        self.barb_scout_targets.remove(&uid);
        self.barb_camp_guards.retain(|_, guard| *guard != uid);
        let carried_aircraft: Vec<u32> = self
            .units
            .get(&uid)
            .filter(|unit| unit.kind == "aircraft_carrier")
            .map(|carrier| {
                self.unit_ids_at(carrier.pos)
                    .iter()
                    .filter(|other| {
                        **other != uid
                            && self.units[other].owner == carrier.owner
                            && self.rules.units[self.units[other].kind].domain.as_deref()
                                == Some("air")
                    })
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        if record_lifetime {
            if let Some(unit) = self
                .units
                .get(&uid)
                .filter(|unit| self.rules.units[unit.kind].class == "military")
                .cloned()
            {
                let cost = self.unit_accounting_cost(&unit);
                self.players[unit.owner]
                    .unit_lifetimes
                    .entry(unit.kind)
                    .or_default()
                    .observe(&unit, cost);
            }
        }
        if let Some(u) = self.units.remove(&uid) {
            if let Some(other) = u.linked_to {
                if let Some(peer) = self.units.get_mut(&other) {
                    peer.linked_to = None;
                }
            }
            if let Some(ids) = self.occ.get_mut(&u.pos) {
                ids.retain(|i| *i != uid);
                if ids.is_empty() {
                    self.occ.remove(&u.pos);
                }
            }
        }
        for aircraft in carried_aircraft {
            self.remove_unit(aircraft);
        }
    }

    /// Combat return by military unit kind for one civilization, including
    /// completed lives and every unit still on the field at this instant.
    ///
    /// At a Tactics result this is the full roster: defeated units came
    /// through `remove_unit`, and survivors are added here at their final
    /// accumulated damage. During a live battle the survivor rows are useful
    /// but naturally still growing.
    pub fn unit_lifetime_stats(&self, pid: usize) -> BTreeMap<Name, UnitLifetimeStats> {
        let mut stats = self
            .players
            .get(pid)
            .map(|player| player.unit_lifetimes.clone())
            .unwrap_or_default();
        for unit in self
            .units
            .values()
            .filter(|unit| unit.owner == pid && self.rules.units[unit.kind].class == "military")
        {
            let cost = self.unit_accounting_cost(unit);
            stats.entry(unit.kind).or_default().observe(unit, cost);
        }
        stats
    }

    /// Change control of a unit without leaving an escort formation spanning
    /// two owners. Levies, captures and conversions can all transfer only one
    /// member of a linked pair; every formation consumer assumes that both
    /// members still belong to the same player.
    pub(super) fn transfer_unit_owner(&mut self, uid: u32, owner: usize) {
        let (old_owner, peer) = {
            let unit = &self.units[&uid];
            (unit.owner, unit.linked_to)
        };
        if old_owner == owner {
            return;
        }
        self.units.get_mut(&uid).unwrap().linked_to = None;
        if let Some(peer) = peer.and_then(|peer| self.units.get_mut(&peer)) {
            peer.linked_to = None;
        }
        self.units.get_mut(&uid).unwrap().owner = owner;
    }

    /// Where a defender forced out of its tile can go, if anywhere.
    ///
    /// ★★★★★ POLAND'S WINGED HUSSAR HAD ITS ABILITY IN THE DATA AND NOWHERE
    /// ELSE. `data/units.json` gives the unit `force_retreat: 1`, the engine
    /// read the key nowhere, and the audit that would have said so
    /// (`tools/civvis_inert.py`) was never wired into CI. So the replacement
    /// for a Cuirassier was a Cuirassier: same strength, same movement, no
    /// unique. This is the missing half.
    ///
    /// Civilization VI pushes the loser directly away from its attacker and
    /// damages it further when it cannot go. "Away" is every neighbour strictly
    /// farther from the attacker than the defender's own tile, which on a hex
    /// board is at most three tiles and on a globe is whatever the sphere says.
    /// The choice is the first legal one in map order — deliberately not a
    /// random pick, because combat already consumes the RNG and a retreat that
    /// drew from it would move every later roll in the game.
    pub(super) fn forced_retreat_tile(&self, defender: u32, attacker_at: Pos) -> Option<Pos> {
        let from = self.units.get(&defender)?.pos;
        let standing = self.wdist(attacker_at, from);
        self.wdisk(from, 1)
            .into_iter()
            .filter(|pos| *pos != from)
            .filter(|pos| self.wdist(attacker_at, *pos) > standing)
            .find(|pos| self.can_enter(defender, from, *pos))
    }

    /// Move a unit and keep the occupancy index and revealed ground with it.
    ///
    /// Crate-visible because `oracle.rs` places units directly. Writing
    /// `unit.pos` instead leaves `occ` holding the unit at its old tile, and
    /// `units_at` then hands out an id that `units` no longer contains — the
    /// panic surfaces much later, in whatever next reads a neighbouring tile.
    pub(crate) fn relocate(&mut self, uid: u32, pos: Pos) {
        let (old, owner) = {
            let u = &self.units[&uid];
            (u.pos, u.owner)
        };
        if let Some(ids) = self.occ.get_mut(&old) {
            ids.retain(|i| *i != uid);
            if ids.is_empty() {
                self.occ.remove(&old);
            }
        }
        let fortification = self.improvement_fortification_at(pos);
        let unit = self.units.get_mut(&uid).unwrap();
        unit.pos = pos;
        unit.fortify_turns = unit.fortify_turns.max(fortification);
        self.occ.entry(pos).or_default().push(uid);
        let sight = self.unit_sight(uid);
        self.reveal(owner, pos, sight);
        // Taking the ENEMY's flag decides a capture-the-flag battle on the
        // spot. Every way a unit changes tiles — a march, a melee advance,
        // an airlift, a retreat — funnels through here, so this is the one
        // place the objective needs to be watched from. A side standing on
        // its own flag has captured nothing; that is a garrison, and it is
        // how a flag gets defended.
        if self
            .arena_flag_holder(pos)
            .is_some_and(|held| !self.same_side(held, owner))
        {
            self.set_winner(owner, FLAG_VICTORY);
        }
    }
}
