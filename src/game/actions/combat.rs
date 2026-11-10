//! Combat actions: attacking, ranged and air strikes, WMDs, city strikes,
//! and pillaging.
//!
//! `do_attack` and `do_ranged` with the exchange strengths they resolve
//! (melee and ranged), the matchup, flanking and support bonuses, the GDR
//! and siege-support effects, capture odds, and kill rewards. The
//! Encampment's melee, ranged and strike actions. Air rebase, strike,
//! pillage and patrol, with interceptions. WMD strikes, their launch
//! platforms and the nuclear announcement. `do_city_strike`, `do_pillage`,
//! and entering a tile after a win.
//!
//! Moved verbatim out of the `impl Game` block in `game/actions.rs`
//! (2026-11-05). Methods the rest of the action layer calls are
//! `pub(super)`; methods the rest of `game` calls keep that reach as
//! `pub(in crate::game)`; public ones stay public. The code is otherwise
//! unchanged.

use super::*;

impl Game {
    /// The two strengths `do_attack` resolves a melee blow with, `(attacker,
    /// defender)`, each already through [`effective_strength`]. `None` when
    /// either unit is gone.
    ///
    /// ★★★★ THIS IS THE ENGINE'S OWN ARITHMETIC, EXPOSED — NOT A COPY OF IT.
    /// `do_attack` calls it, so a controller that prices a melee exchange
    /// *before* it happens cannot drift from the exchange the engine will
    /// actually resolve. That matters here more than for an attack scan,
    /// because the terms this function carries and a strength-only estimate
    /// does not — matchup, flanking, the amphibious and river penalties,
    /// terrain, adjacent support, and fortification — are exactly the ones
    /// that decide whether standing still beats swinging. See
    /// `ranged_order_is_legal` for the same argument about the predicates.
    ///
    /// Feed both halves to [`expected_damage`] to get the two blows of one
    /// melee round at the engine's unrandomized centre: the defender takes
    /// `expected_damage(att, def)` and the attacker takes
    /// `expected_damage(def, att)`.
    ///
    /// `pub`, not `pub(crate)`: this is the engine's own arithmetic, exposed
    /// for planners and for the divergence instrument
    /// (`live_divergence::combat_pairs`) so that nothing outside the crate
    /// re-derives a melee exchange from two bare strengths.
    pub fn melee_exchange_strengths(&self, uid: u32, did: u32) -> Option<(f64, f64)> {
        let attacker = self.units.get(&uid)?;
        let defender = self.units.get(&did)?;
        let target = defender.pos;
        let unamphibious = self.promotion_effect(attacker, "amphibious") == 0.0;
        let mut att_base = self.unit_unembarked_strength(attacker)
            + self.matchup_bonus(uid, defender, true)
            + self.flanking_bonus(uid, target)
            + self.vs_bonus(attacker.owner, defender.owner);
        if self.is_embarked(attacker) && unamphibious {
            att_base -= 10.0;
        }
        let mut def_base = self.unit_strength(defender, true)
            + self.matchup_bonus(did, attacker, false)
            + self.tile_defense_bonus(target)
            + self.support_bonus(defender)
            + self.vs_bonus(defender.owner, attacker.owner);
        if self.crosses_river(attacker.pos, target) && unamphibious {
            def_base += 5.0;
        }
        Some((
            effective_strength(att_base, attacker.hp),
            effective_strength(def_base, defender.hp),
        ))
    }

    /// The two strengths `do_ranged` resolves a shot with, `(shooter,
    /// defender)`, each already through [`effective_strength`]. `target` is
    /// the tile fired at, which is the defender's own tile except for an air
    /// unit answered on patrol. `None` when either unit is gone.
    ///
    /// The companion of [`melee_exchange_strengths`], and the asymmetry
    /// between them is the whole point: a shot returns *nothing*. There is no
    /// second blow in `do_ranged` for a controller to price, which is why
    /// standing under one is a straight loss and standing under a melee
    /// attack need not be.
    ///
    /// `pub` for the same reason as [`melee_exchange_strengths`]: the
    /// engine's own arithmetic, exposed for planners and the divergence
    /// instrument rather than copied by them.
    pub fn ranged_strike_strengths(&self, uid: u32, did: u32, target: Pos) -> Option<(f64, f64)> {
        let shooter = self.units.get(&uid)?;
        let defender = self.units.get(&did)?;
        let spec = &self.rules.units[shooter.kind];
        let defender_spec = &self.rules.units[defender.kind];
        let defender_is_sea = defender_spec.domain.as_deref() == Some("sea");
        let mut att_base = self.unit_ranged_attack_strength(shooter)
            + self.matchup_bonus(uid, defender, true)
            + if defender_is_sea {
                self.promotion_effect(shooter, "ranged_vs_units")
                    + self.promotion_effect(shooter, "ranged_vs_naval")
                    + self.promotion_effect(shooter, "siege_vs_naval")
            } else {
                self.promotion_effect(shooter, "ranged_vs_land")
                    + self.promotion_effect(shooter, "ranged_vs_units")
                    + self.promotion_effect(shooter, "siege_vs_land")
            }
            + self.vs_bonus(shooter.owner, defender.owner);
        if (spec.bombard_strength > 0.0 && !defender_is_sea)
            || (spec.ranged_strength > 0.0
                && spec.domain.as_deref() != Some("sea")
                && defender_is_sea)
        {
            att_base -= 17.0;
        }
        Some((
            effective_strength(att_base, shooter.hp),
            self.ranged_defender_strength(defender, target, shooter.owner),
        ))
    }

    /// Advanced Power Cells fits the Giant Death Robot with the Particle Beam
    /// Siege Cannon: +30 Ranged Strength against cities and Encampments, and
    /// those attacks are fully effective against walls rather than half.
    pub(in crate::game) fn gdr_siege_bonus(&self, unit: &Unit) -> f64 {
        if unit.kind == "giant_death_robot" {
            self.tree_effect(unit.owner, "gdr_ranged_vs_district")
        } else {
            0.0
        }
    }

    pub(in crate::game) fn gdr_full_wall_damage(&self, unit: &Unit) -> bool {
        unit.kind == "giant_death_robot"
            && self.tree_effect(unit.owner, "gdr_full_wall_damage") > 0.0
    }

    // Expansion2/Data/Expansion1_Leaders.xml:781-788: the major-civilization
    // unit modifier is Amount=3 with DeltaWithOpponent=true. It applies on
    // defense against a ranged shot too, before the wound penalty.

    pub(in crate::game) fn matchup_bonus(&self, uid: u32, opponent: &Unit, attacking: bool) -> f64 {
        let u = &self.units[&uid];
        let spec = &self.rules.units[u.kind];
        let other = &self.rules.units[opponent.kind];
        let mut bonus = 0.0;
        if self.active_emergencies.iter().any(|emergency| {
            emergency.ends > self.turn
                && emergency.target == opponent.owner
                && emergency.members.contains(&u.owner)
        }) {
            bonus += 2.0;
        }
        bonus += self.visibility_combat_bonus(u.owner, opponent.owner);
        if self.players[u.owner].religion.is_some()
            && self.players[opponent.owner].religion.is_some()
            && self.players[u.owner].religion != self.players[opponent.owner].religion
        {
            bonus += self.policy_effect(u.owner, "different_religion_combat");
        }
        // Cyber Warfare: CYBER_WARFARE_INFO_FUTURE_BUFF, +10 against a
        // civilization in the Information Era or later.
        if self.player_era(opponent.owner) >= 7 {
            bonus += self.policy_effect(u.owner, "combat_vs_information_era");
        }
        if spec.promotion_class == "anti_cavalry"
            && (matches!(
                other.promotion_class.as_str(),
                "light_cavalry" | "heavy_cavalry"
            ) || (other.cavalry && other.promotion_class == "ranged"))
            && opponent.kind != "war_cart"
        {
            bonus += 10.0;
        }
        if spec.promotion_class == "melee" && other.promotion_class == "anti_cavalry" {
            bonus += 5.0;
        }
        if u.kind == "nihang" && other.promotion_class == "anti_cavalry" {
            bonus += 5.0;
        }
        if u.kind == "hoplite"
            && self.nbrs(u.pos).into_iter().any(|p| {
                self.unit_ids_at(p).iter().any(|id| {
                    *id != uid
                        && self.units[id].owner == u.owner
                        && self.units[id].kind == "hoplite"
                })
            })
        {
            bonus += 10.0;
        }
        if attacking && self.has_ability(u.owner, "killer_of_cyrus") && opponent.hp < 100 {
            bonus += 5.0;
        }
        if attacking {
            if matches!(other.promotion_class.as_str(), "melee" | "ranged") {
                bonus += self.promotion_effect(u, "attack_vs_melee_ranged");
            }
            if other.promotion_class == "melee" {
                bonus += self.promotion_effect(u, "vs_melee");
            }
            if other.promotion_class == "anti_cavalry" {
                bonus += self.promotion_effect(u, "vs_anti_cavalry");
            }
            if matches!(
                other.promotion_class.as_str(),
                "light_cavalry" | "heavy_cavalry"
            ) {
                bonus += self.promotion_effect(u, "vs_cavalry");
            }
            if matches!(other.promotion_class.as_str(), "ranged" | "siege") {
                bonus += self.promotion_effect(u, "attack_vs_ranged_siege");
            }
            if other.promotion_class == "siege" {
                bonus += self.promotion_effect(u, "vs_siege");
            }
            if other.promotion_class == "heavy_cavalry" {
                bonus += self.promotion_effect(u, "vs_heavy_cavalry");
            }
            if opponent.hp < 100 {
                bonus += self.promotion_effect(u, "vs_damaged");
            }
            if opponent.fortify_turns > 0 {
                bonus += self.promotion_effect(u, "vs_fortified");
            }
            let tile = &self.map.tiles[&opponent.pos];
            if self.city_at(opponent.pos).is_some() || tile.district.is_some() {
                bonus += self.promotion_effect(u, "vs_unit_in_district");
                bonus += self.promotion_effect(u, "district_melee");
            }
            if other.domain.as_deref() == Some("sea") {
                bonus += self.promotion_effect(u, "vs_naval");
            }
            if other.promotion_class == "naval_raider" {
                bonus += self.promotion_effect(u, "vs_naval_raider");
            }
        } else {
            if other.promotion_class == "melee" {
                bonus += self.promotion_effect(u, "defend_melee");
            }
            if matches!(
                other.promotion_class.as_str(),
                "heavy_cavalry" | "anti_cavalry"
            ) {
                bonus += self.promotion_effect(u, "defend_heavy_anti");
            }
            let tile = &self.map.tiles[&u.pos];
            if self.city_at(u.pos).is_some() || tile.district.is_some() {
                bonus += self.promotion_effect(u, "district_melee");
            }
        }
        if matches!(
            other.promotion_class.as_str(),
            "light_cavalry" | "heavy_cavalry"
        ) {
            bonus += self
                .nbrs(u.pos)
                .into_iter()
                .flat_map(|pos| self.unit_ids_at(pos))
                .filter(|id| {
                    let ally = &self.units[id];
                    ally.owner == u.owner
                        && self.rules.units[ally.kind].promotion_class != spec.promotion_class
                })
                .map(|id| self.promotion_effect(&self.units[id], "adjacent_vs_cavalry"))
                .sum::<f64>();
        }
        bonus
    }

    pub(in crate::game) fn eagle_capture_chance(&self, uid: u32, opponent: &Unit) -> f64 {
        let unit = &self.units[&uid];
        if unit.kind != "eagle_warrior"
            || self.players[opponent.owner].is_barbarian
            || self.rules.units[opponent.kind].class != "military"
        {
            return 0.0;
        }
        let attacker = self.rules.units[unit.kind].strength;
        let defender = self.rules.units[opponent.kind].strength;
        (50.0 + (attacker - defender) * 2.5).clamp(0.0, 100.0)
    }

    /// Everything a defeated unit pays its killer.
    ///
    /// ⚠ Named for promotions and never only about them — the policy card
    /// `earlier_era_kill_gold_pct` and the building `heal_on_unit_kill` were
    /// already here — and God of War makes the pantheon the third source, so
    /// the name is now what the function does.
    pub(in crate::game) fn kill_rewards(&mut self, attacker: &Unit, defeated: &Unit) {
        let defeated_spec = &self.rules.units[defeated.kind];
        let defeated_era = defeated_spec
            .tech
            .as_ref()
            .and_then(|node| self.rules.techs.get(node).map(|spec| spec.era))
            .or_else(|| {
                defeated_spec
                    .civic
                    .as_ref()
                    .and_then(|node| self.rules.civics.get(node).map(|spec| spec.era))
            })
            .unwrap_or(0);
        if self.player_era(attacker.owner) > defeated_era {
            let pct = self.policy_effect(attacker.owner, "earlier_era_kill_gold_pct");
            self.players[attacker.owner].gold += defeated_spec.strength * pct / 100.0;
        }
        let faith_pct = self.promotion_effect(attacker, "faith_on_kill_strength_pct");
        if faith_pct > 0.0 {
            self.players[attacker.owner].faith += defeated_spec.strength * faith_pct / 100.0;
        }
        // God of War: GOD_OF_WAR_FAITH_KILLS_MODIFIER,
        // `MODIFIER_PLAYER_UNITS_ADJUST_POST_COMBAT_YIELD` with
        // `PercentDefeatedStrength 50` and `YieldType YIELD_FAITH`, over
        // `PLOT_EIGHT_INCLUDE_HOLY_SITE`. The same arithmetic as the promotion
        // above — a percentage of the dead unit's Combat Strength — with a plot
        // test instead of a promotion, so the two share this shape rather than
        // each inventing one. Not in `Expansion2_RemoveData.xml`.
        //
        // ⚠ Two things the shipped rows say that a reading from memory does
        // not. The requirement names no owner, and the text agrees by omission
        // — "within 8 tiles of a Holy Site district" — so a rival's Holy Site
        // pays as well as our own. And the text ends "(on Standard Speed)",
        // Civilization VI's marker for a one-off yield that scales with the
        // game speed, which `GameSpeed::scale` is.
        let war_pct = self.pantheon_effect(attacker.owner, "faith_on_kill_near_holy_site_pct");
        if war_pct > 0.0 && defeated_spec.class == "military" {
            let strength = defeated_spec.strength;
            let near_a_holy_site = self
                .wdisk(defeated.pos, GOD_OF_WAR_HOLY_SITE_RANGE)
                .into_iter()
                .any(|position| {
                    self.map.get(position).is_some_and(|tile| {
                        tile.district.is_some_and(|district| {
                            self.district_is_family(district, crate::name!("holy_site"))
                        })
                    })
                });
            if near_a_holy_site {
                self.players[attacker.owner].faith +=
                    self.game_speed.scale(strength * war_pct / 100.0);
            }
        }
        if self.rules.units[defeated.kind].domain.as_deref() == Some("sea") {
            let pct = self.promotion_effect(attacker, "gold_from_naval_kill_pct");
            if pct > 0.0 {
                let strength = self.rules.units[defeated.kind].strength;
                self.players[attacker.owner].gold += strength * pct / 100.0;
            }
        }
        let heal = self
            .cities
            .values()
            .filter(|city| city.owner == attacker.owner)
            .map(|city| self.city_building_effect(city, "heal_on_unit_kill"))
            .sum::<f64>() as i32;
        if heal > 0 {
            if let Some(unit) = self.units.get_mut(&attacker.id) {
                unit.hp = (unit.hp + heal).min(100);
            }
        }
        let pressure = self.promotion_effect(attacker, "religious_pressure_on_kill");
        if pressure > 0.0 {
            if let Some(religion) = attacker.religion.as_ref() {
                let cities: Vec<u32> = self
                    .cities
                    .values()
                    .filter(|city| self.wdist(city.pos, defeated.pos) <= 6)
                    .map(|city| city.id)
                    .collect();
                for cid in cities {
                    *self
                        .cities
                        .get_mut(&cid)
                        .unwrap()
                        .pressure
                        .entry(religion.clone())
                        .or_insert(0.0) += pressure;
                }
            }
        }
    }

    pub(in crate::game) fn flanking_support_unlocked(&self, owner: usize) -> bool {
        if self.players[owner].is_barbarian {
            let majors: Vec<&Player> = self
                .players
                .iter()
                .filter(|p| !p.is_minor && !p.is_barbarian)
                .collect();
            !majors.is_empty()
                && 2 * majors
                    .iter()
                    .filter(|p| self.tree_effect(p.id, "flanking_support") > 0.0)
                    .count()
                    >= majors.len()
        } else {
            self.tree_effect(owner, "flanking_support") > 0.0
        }
    }

    pub(in crate::game) fn flanking_bonus(&self, uid: u32, target: Pos) -> f64 {
        let owner = self.units[&uid].owner;
        let additional = self
            .nbrs(target)
            .into_iter()
            .flat_map(|p| self.unit_ids_at(p))
            .filter(|id| **id != uid)
            .filter(|id| {
                let u = &self.units[id];
                u.owner == owner
                    && self.rules.units[u.kind].class == "military"
                    && !self.is_embarked(u)
                    && !self.crosses_river(u.pos, target)
            })
            .count();
        let tegh = if additional > 0 {
            self.promotion_effect(&self.units[&uid], "flanking_combat_strength")
        } else {
            0.0
        };
        if !self.flanking_support_unlocked(owner) {
            return tegh;
        }
        let multiplier = self
            .promotion_effect(&self.units[&uid], "flanking_multiplier")
            .max(1.0);
        let naval_multiplier =
            if self.rules.units[self.units[&uid].kind].domain.as_deref() == Some("sea") {
                1.0 + self.players[owner]
                    .counters
                    .get("great_person:naval_flanking_bonus_pct")
                    .copied()
                    .unwrap_or(0) as f64
                    / 100.0
            } else {
                1.0
            };
        2.0 * additional as f64 * multiplier * naval_multiplier + tegh
    }

    pub(in crate::game) fn support_bonus(&self, defender: &Unit) -> f64 {
        if !self.flanking_support_unlocked(defender.owner) {
            return 0.0;
        }
        let adjacent = self
            .nbrs(defender.pos)
            .into_iter()
            .flat_map(|p| self.unit_ids_at(p))
            .filter(|id| {
                let u = &self.units[id];
                u.owner == defender.owner && self.rules.units[u.kind].class == "military"
            })
            .count();
        let multiplier = self
            .promotion_effect(defender, "support_multiplier")
            .max(1.0);
        2.0 * adjacent as f64 * multiplier
    }

    pub(in crate::game) fn consume_melee_attack(&mut self, uid: u32, target: Pos) {
        let cost = self.unit_step_cost(uid, self.units[&uid].pos, target);
        let remaining = (self.units[&uid].moves_left - cost).max(0.0);
        self.units.get_mut(&uid).unwrap().moves_left = remaining;
        self.consume_unit_attack(uid);
    }

    pub(in crate::game) fn pillage_encampment(&mut self, uid: u32, cid: u32, target: Pos) {
        let defender = self.cities[&cid].owner;
        let garrison: Vec<u32> = self
            .unit_ids_at(target)
            .iter()
            .filter(|id| {
                self.units[id].owner == defender
                    && self.rules.units[self.units[id].kind].class == "military"
            })
            .copied()
            .collect();
        for id in garrison {
            self.remove_unit(id);
            self.on_unit_lost(defender);
        }
        let mut state = self.defending_district_state(cid, target).unwrap();
        state.hp = 0;
        state.wall_hp = 0;
        state.pillaged = true;
        self.set_defending_district_state(cid, state);
        self.map.tiles.get_mut(&target).unwrap().pillaged = true;
        self.enter_tile(uid, target);
    }

    pub(in crate::game) fn do_encampment_melee(
        &mut self,
        pid: usize,
        uid: u32,
        cid: u32,
        target: Pos,
        embarked: bool,
    ) -> Result<(), String> {
        let defender = self.cities[&cid].owner;
        let participant = self.units[&uid].clone();
        self.record_war_unit_participation(&participant, defender);
        if self.defending_district_state(cid, target).unwrap().hp <= 0 {
            self.consume_melee_attack(uid, target);
            self.pillage_encampment(uid, cid, target);
            return Ok(());
        }
        let attacker = self.units[&uid].clone();
        let spec = self.rules.units[attacker.kind].clone();
        let mut attack_base =
            self.unit_unembarked_strength(&attacker) + self.vs_bonus(pid, self.cities[&cid].owner);
        if embarked && self.promotion_effect(&attacker, "amphibious") == 0.0 {
            attack_base -= 10.0;
        }
        let mut defense = self.defending_district_strength(cid, target);
        if self.crosses_river(attacker.pos, target)
            && self.promotion_effect(&attacker, "amphibious") == 0.0
        {
            defense += 5.0;
        }
        let attack = effective_strength(attack_base, attacker.hp);
        let dealt = damage(attack, defense, &mut self.rng);
        let received = damage(defense, attack, &mut self.rng);
        let (ram, tower) = self.siege_support_effects(pid, cid, target, &spec.promotion_class);
        self.defending_district_take_damage(
            cid,
            target,
            dealt,
            if ram { 1.0 } else { 0.15 },
            tower,
        );
        self.units.get_mut(&uid).unwrap().hp -= received;
        self.consume_melee_attack(uid, target);
        if self.units[&uid].hp <= 0 {
            self.remove_unit(uid);
            self.on_unit_lost(pid);
            let mut state = self.defending_district_state(cid, target).unwrap();
            state.hp = state.hp.max(1);
            self.set_defending_district_state(cid, state);
            return Ok(());
        }
        if self.defending_district_state(cid, target).unwrap().hp <= 0 {
            self.award_initiated_combat_xp(uid, 10.0);
            self.pillage_encampment(uid, cid, target);
        } else {
            self.award_initiated_combat_xp(uid, 3.0);
        }
        Ok(())
    }

    /// Whether this city's defenses permit a support kind's breach effect.
    /// Shared by combat and AI equipment selection, including unique walls.
    pub(crate) fn city_allows_siege_support(&self, cid: u32, kind: &str) -> bool {
        let city = &self.cities[&cid];
        if self.tree_effect(city.owner, "urban_defenses") > 0.0 {
            return false;
        }
        let immunity = match kind {
            "battering_ram" => "battering_ram_immunity",
            "siege_tower" => "siege_support_immunity",
            _ => return false,
        };
        // `breach-support-reads-the-wall-tier`: a mirrored city shows its wall
        // pool, not its buildings. Each tier adds 100 to the pool (Ancient,
        // Medieval, Renaissance), so a pool of 200 stands behind Medieval
        // Walls, which a ram cannot work on, and 300 behind Renaissance Walls,
        // which a tower cannot. Live King civvis-20261005T152615Z (game 139)
        // sent a Siege Tower to Mikisiw-Wacihk's 300/300 walls six times.
        if self.observed_wall_tier_rules {
            if let Some(max) = self.observed_city_max_wall_hp.get(&cid) {
                let immune_from = if kind == "battering_ram" { 200 } else { 300 };
                if *max >= immune_from {
                    return false;
                }
            }
        }
        self.city_building_effect(city, immunity) <= 0.0
    }

    /// Active Battering Ram and Siege Tower effects against a City Center or
    /// Encampment. Their wall-era limits also apply to replacement buildings,
    /// and both support units are ineffective against Steel's Urban Defenses.
    pub(crate) fn siege_support_effects(
        &self,
        attacker: usize,
        cid: u32,
        target: Pos,
        promotion_class: &str,
    ) -> (bool, bool) {
        if !matches!(promotion_class, "melee" | "anti_cavalry") {
            return (false, false);
        }
        let adjacent_support = |kind: &str| {
            self.nbrs(target).into_iter().any(|position| {
                self.unit_ids_at(position)
                    .iter()
                    .any(|id| self.units[id].owner == attacker && self.units[id].kind == kind)
            })
        };
        let ram = adjacent_support("battering_ram")
            && self.city_allows_siege_support(cid, "battering_ram");
        let tower =
            adjacent_support("siege_tower") && self.city_allows_siege_support(cid, "siege_tower");
        (ram, tower)
    }

    pub(in crate::game) fn do_encampment_ranged(
        &mut self,
        pid: usize,
        uid: u32,
        cid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let defender = self.cities[&cid].owner;
        let participant = self.units[&uid].clone();
        self.record_war_unit_participation(&participant, defender);
        if self.defending_district_state(cid, target).unwrap().hp <= 0 {
            self.consume_unit_attack(uid);
            self.set_defending_district_hp(cid, target, 0);
            return Ok(());
        }
        let attacker = self.units[&uid].clone();
        let spec = self.rules.units[attacker.kind].clone();
        let mut attack_base = self.unit_ranged_attack_strength(&attacker)
            + self.vs_bonus(pid, self.cities[&cid].owner)
            + self.promotion_effect(&attacker, "ranged_vs_district")
            + self.gdr_siege_bonus(&attacker);
        if spec.ranged_strength > 0.0 && spec.domain.as_deref() != Some("sea") {
            attack_base -= 17.0;
        }
        let attack = effective_strength(attack_base, attacker.hp);
        let dealt = damage(
            attack,
            self.defending_district_strength(cid, target),
            &mut self.rng,
        );
        self.defending_district_take_damage(
            cid,
            target,
            dealt,
            if spec.siege { 1.0 } else { 0.5 },
            false,
        );
        self.consume_unit_attack(uid);
        if self.defending_district_state(cid, target).unwrap().hp <= 0 {
            if spec.siege {
                self.set_defending_district_hp(cid, target, 0);
                self.award_initiated_combat_xp(uid, 10.0);
            } else {
                self.set_defending_district_hp(cid, target, 1);
                self.award_initiated_combat_xp(uid, 3.0);
            }
        } else {
            self.award_initiated_combat_xp(uid, 3.0);
        }
        Ok(())
    }

    pub(in crate::game) fn do_attack(
        &mut self,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let u = self.own_unit(pid, uid)?;
        let spec = self.rules.units[u.kind].clone();
        if !spec.is_melee_capable() {
            return Err("unit cannot melee attack".into());
        }
        if self.strike_blocked(uid, target) {
            return Err("the host refused this strike this turn".into());
        }
        let amphibious = self.is_embarked(&u);
        if u.moves_left <= 0.0 || u.attacks_left <= 0 {
            return Err("no moves left".into());
        }
        if self.wdist(u.pos, target) != 1 {
            return Err("target not adjacent".into());
        }
        if !self.unit_can_melee_target_domain(uid, target) {
            return Err("unit cannot attack into that domain".into());
        }
        if !self.can_pay_melee_entry(uid, target) {
            return Err("not enough movement to attack".into());
        }
        if amphibious
            && self
                .map
                .get(target)
                .map(|t| self.rules.is_water(t))
                .unwrap_or(true)
        {
            return Err("embarked units can only attack onto land".into());
        }
        if let Some(cid) = self.defending_district_at(target) {
            let owner = self.cities[&cid].owner;
            if owner != pid && self.is_at_war(pid, owner) {
                return self.do_encampment_melee(pid, uid, cid, target, amphibious);
            }
        }
        let enemy_ids: Vec<u32> = self
            .unit_ids_at(target)
            .iter()
            .filter(|id| {
                let owner = self.units[id].owner;
                owner != pid && self.is_at_war(pid, owner)
            })
            .copied()
            .collect();
        let mut city_id = self.city_at(target);
        if let Some(cid) = city_id {
            let owner = self.cities[&cid].owner;
            if owner == pid || !self.is_at_war(pid, owner) {
                city_id = None;
            }
        }
        // See `peaceful_foreign_unit_at`. The enumeration above no longer
        // offers this plot; refuse it here too so a hand-written order — or a
        // replay of one the host already mangled — cannot take the same shot.
        // A city is its own defender, so only the unit fight is vetoed.
        if city_id.is_none() && self.peaceful_foreign_unit_at(pid, target) {
            return Err("a unit we are at peace with stands there".into());
        }
        if enemy_ids.is_empty() && city_id.is_none() {
            return Err("nothing to attack".into());
        }
        let military: Vec<u32> = enemy_ids
            .iter()
            .cloned()
            .filter(|id| {
                let spec = &self.rules.units[self.units[id].kind];
                spec.class == "military" && spec.domain.as_deref() != Some("air")
            })
            .collect();
        if military.is_empty() && city_id.is_none() {
            return Err("no combat target".into());
        }
        self.consume_melee_attack(uid, target);
        // A unit garrisoned in a City Center cannot be targeted directly;
        // attacks hit the city and the garrison only affects its strength.
        if city_id.is_none() && !military.is_empty() {
            let did = *military
                .iter()
                .max_by(|a, b| {
                    let ea = effective_strength(
                        self.unit_strength(&self.units[*a], true),
                        self.units[*a].hp,
                    );
                    let eb = effective_strength(
                        self.unit_strength(&self.units[*b], true),
                        self.units[*b].hp,
                    );
                    ea.partial_cmp(&eb).unwrap()
                })
                .unwrap();
            let d = self.units[&did].clone();
            let attacker = self.units[&uid].clone();
            self.record_war_unit_participation(&attacker, d.owner);
            self.record_war_unit_participation(&d, attacker.owner);
            // The strengths live in `melee_exchange_strengths` so a controller
            // can price this exact exchange before choosing to take it. The
            // unit has been consumed above, but nothing `consume_melee_attack`
            // touches (attacks, movement, fortification) enters an attacker's
            // strength, and it does not move the unit — so the shared reading
            // is the reading this line always had.
            let (att, ds) = self
                .melee_exchange_strengths(uid, did)
                .expect("both combatants exist at the blow");
            let dmg_out = damage(att, ds, &mut self.rng);
            let dmg_in = damage(ds, att, &mut self.rng);
            self.apply_unit_damage(uid, did, dmg_out);
            self.apply_unit_damage(did, uid, dmg_in);
            let d_dead = self.units[&did].hp <= 0;
            let downer = self.units[&did].owner;
            self.record_emergency_combat(pid, downer, d_dead);
            if d_dead && self.has_ability(pid, "killer_of_cyrus") {
                if let Some(mu) = self.units.get_mut(&uid) {
                    mu.hp = (mu.hp + 30).min(100); // Tomyris
                }
            }
            let attacker_dead = self.units[&uid].hp <= 0;
            if attacker_dead && !d_dead && self.has_ability(downer, "killer_of_cyrus") {
                if let Some(defender) = self.units.get_mut(&did) {
                    defender.hp = (defender.hp + 30).min(100);
                }
            }
            if !attacker_dead {
                self.award_unit_combat_xp(uid, &d, false, true, d_dead);
            }
            if !d_dead {
                self.award_unit_combat_xp(did, &attacker, false, false, attacker_dead);
            }
            // A survivor that loses to a Winged Hussar is pushed off its tile,
            // and pays for standing its ground when there is nowhere to go.
            if !d_dead
                && !attacker_dead
                && self.rules.units[attacker.kind]
                    .effects
                    .get("force_retreat")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
            {
                match self.forced_retreat_tile(did, attacker.pos) {
                    Some(to) => self.relocate(did, to),
                    None => {
                        // Cornered. Civilization VI charges the defender for the
                        // retreat it could not make; this can finish it, so the
                        // death is handled by the same path every other kill
                        // takes rather than a second copy of it here.
                        self.apply_unit_damage(uid, did, FORCED_RETREAT_BLOCKED_DAMAGE);
                    }
                }
            }
            let capture_chance = self.eagle_capture_chance(uid, &d);
            let captured_as_builder = d_dead
                && !attacker_dead
                && capture_chance > 0.0
                && self.rng.uniform(0.0, 100.0) < capture_chance;
            if d_dead {
                self.note_underdog_kill(pid, &attacker, &d);
                self.note_great_person_assisted_kill(pid, &attacker);
                self.record_kill(pid, Some(&attacker.kind), &d);
                self.kill_rewards(&attacker, &d);
                self.remove_unit(did);
                self.on_unit_lost(downer);
                if captured_as_builder {
                    self.spawn_unit("builder", pid, target);
                }
            }
            if attacker_dead {
                if self.units.contains_key(&uid) {
                    self.note_underdog_kill(downer, &d, &attacker);
                    self.record_kill(downer, Some(&d.kind), &attacker);
                    self.remove_unit(uid);
                    self.on_unit_lost(pid);
                }
                return Ok(());
            }
            if d_dead {
                let enemy_military_left = self.unit_ids_at(target).iter().any(|id| {
                    let o = &self.units[id];
                    o.owner != pid && self.rules.units[o.kind].class == "military"
                });
                if !enemy_military_left {
                    let city_blocks = match city_id {
                        Some(cid) => self.cities.get(&cid).map(|c| c.hp > 0).unwrap_or(false),
                        None => false,
                    };
                    if !city_blocks {
                        self.enter_tile(uid, target);
                    }
                }
            }
        } else if let Some(cid) = city_id {
            if self.cities[&cid].hp > 0 {
                let attacker = self.units[&uid].clone();
                let defender = self.cities[&cid].owner;
                self.record_war_unit_participation(&attacker, defender);
                self.record_war_city_garrison_participation(cid, attacker.owner);
                let (att, cs) = self.city_melee_exchange_strengths(uid, cid).unwrap();
                let dmg_out = damage(att, cs, &mut self.rng);
                let dmg_in = damage(cs, att, &mut self.rng);
                // battering ram: full melee damage vs ancient walls;
                // siege tower: only melee/anti-cavalry pour through the walls
                let (ram, tower) =
                    self.siege_support_effects(pid, cid, target, &spec.promotion_class);
                let basil_cavalry = self.has_ability(pid, "taxis")
                    && spec.cavalry
                    && self.players[pid]
                        .religion
                        .as_deref()
                        .is_some_and(|religion| {
                            self.city_religion(&self.cities[&cid]) == Some(religion)
                        });
                let akkad = self.grants_city_state_unique_bonus(pid, "Akkad")
                    && matches!(spec.promotion_class.as_str(), "melee" | "anti_cavalry");
                let mult = if ram || basil_cavalry || akkad {
                    1.0
                } else {
                    0.15
                };
                self.city_take_damage(pid, cid, dmg_out, mult, tower);
                self.units.get_mut(&uid).unwrap().hp -= dmg_in;
                if self.units[&uid].hp <= 0 {
                    self.remove_unit(uid);
                    self.on_unit_lost(pid);
                    let c = self.cities.get_mut(&cid).unwrap();
                    c.hp = c.hp.max(1);
                    return Ok(());
                }
                if self.cities[&cid].hp <= 0 {
                    if self.players[pid].is_barbarian {
                        self.award_initiated_combat_xp(uid, 3.0);
                        self.cities.get_mut(&cid).unwrap().hp = 1;
                    } else {
                        self.award_initiated_combat_xp(uid, 10.0);
                        self.capture_city(cid, pid);
                        self.apply_capture_formation_upgrade(pid, uid);
                        self.enter_tile(uid, target);
                    }
                } else {
                    self.award_initiated_combat_xp(uid, 3.0);
                }
            } else if self.players[pid].is_barbarian {
                self.cities.get_mut(&cid).unwrap().hp = 1;
            } else {
                // A previous ranged attack may have depleted the garrison
                // health. The melee unit captures it but earns no XP for an
                // attack made after the city was already at 0 HP.
                self.capture_city(cid, pid);
                self.apply_capture_formation_upgrade(pid, uid);
                self.enter_tile(uid, target);
            }
        }
        Ok(())
    }

    pub(in crate::game) fn enter_tile(&mut self, uid: u32, pos: Pos) {
        let linked = self
            .units
            .get(&uid)
            .and_then(|unit| unit.linked_to)
            .filter(|_| self.is_linked_leader(uid));
        self.resolve_entered_units(uid, pos);
        self.relocate(uid, pos);
        let linked = linked.filter(|peer| {
            self.units
                .get(&uid)
                .is_some_and(|unit| unit.linked_to == Some(*peer))
                && self.units.get(peer).is_some_and(|unit| {
                    unit.linked_to == Some(uid) && unit.owner == self.units[&uid].owner
                })
        });
        if let Some(peer) = linked {
            self.relocate(peer, pos);
            let peer_max = self.unit_max_moves(peer);
            self.units.get_mut(&peer).unwrap().moves_left =
                self.units[&peer].moves_left.min(peer_max);
        }
        let unit_max = self.unit_max_moves(uid);
        self.units.get_mut(&uid).unwrap().moves_left = self.units[&uid].moves_left.min(unit_max);
        if self.formation_enters_enemy_zoc(uid, pos) {
            self.stop_unit_by_zoc(uid);
            if let Some(peer) = linked {
                self.stop_unit_by_zoc(peer);
            }
        }
        self.maybe_clear_camp(uid);
        self.maybe_goody_hut(uid);
    }

    pub(in crate::game) fn do_ranged(
        &mut self,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let u = self.own_unit(pid, uid)?;
        let spec = self.rules.units[u.kind].clone();
        // Civ6Common.lua:139-148 routes DOMAIN_AIR through AIR_ATTACK;
        // RANGE_ATTACK belongs to the land/naval branch. Exact planning must
        // not accept a ground shot that bypasses the air-strike rules and
        // translates into an operation the native aircraft cannot perform.
        if spec.domain.as_deref() == Some("air") {
            return Err("aircraft must use an air strike".into());
        }
        if !spec.has_ranged_attack() {
            return Err("unit has no ranged attack".into());
        }
        if self.strike_blocked(uid, target) {
            return Err("the host refused this strike this turn".into());
        }
        if self.is_embarked(&u) {
            return Err("cannot attack while embarked".into());
        }
        if spec.siege && u.moved && !self.siege_may_attack_after_moving(&u) {
            return Err("siege units cannot move and attack in the same turn".into());
        }
        if self.shooter_stacked_in_city(uid) {
            return Err("a unit stacked on a City Center cannot fire".into());
        }
        if u.moves_left <= 0.0 || u.attacks_left <= 0 {
            return Err("no moves left".into());
        }
        let range = self.unit_attack_range(uid);
        if self.wdist(u.pos, target) > range {
            return Err("out of range".into());
        }
        let visible = self.player_vision_frame(pid);
        let viewers = self.visibility_viewers(pid);
        if !self.combat_target_visible_at(pid, target, visible.as_ref(), &viewers) {
            return Err("target is not visible".into());
        }
        if !self.unit_has_line_of_sight(uid, target) {
            return Err("line of sight blocked".into());
        }
        if let Some(cid) = self.defending_district_at(target) {
            let owner = self.cities[&cid].owner;
            if owner != pid && self.is_at_war(pid, owner) {
                return self.do_encampment_ranged(pid, uid, cid, target);
            }
        }
        let mut enemy_ids: Vec<u32> = self
            .unit_ids_at(target)
            .iter()
            .filter(|id| {
                let owner = self.units[id].owner;
                owner != pid
                    && self.is_at_war(pid, owner)
                    && self.unit_currently_visible_to(**id, pid)
            })
            .copied()
            .collect();
        enemy_ids.extend(self.units.values().filter_map(|unit| {
            (unit.air_patrol
                && unit.air_patrol_pos == Some(target)
                && unit.owner != pid
                && self.is_at_war(pid, unit.owner)
                && self.rules.units[unit.kind].promotion_class == "air_fighter")
                .then_some(unit.id)
                .filter(|unit| self.unit_currently_visible_to(*unit, pid))
        }));
        enemy_ids.sort_unstable();
        enemy_ids.dedup();
        let mut city_id = self.city_at(target);
        if let Some(cid) = city_id {
            let owner = self.cities[&cid].owner;
            if owner == pid || !self.is_at_war(pid, owner) {
                city_id = None;
            }
        }
        // See `peaceful_foreign_unit_at`, and `do_attack` for why the city
        // branch stays open.
        if city_id.is_none() && self.peaceful_foreign_unit_at(pid, target) {
            return Err("a unit we are at peace with stands there".into());
        }
        let military: Vec<u32> = enemy_ids
            .iter()
            .cloned()
            .filter(|id| {
                let spec = &self.rules.units[self.units[id].kind];
                spec.class == "military"
                    && (spec.domain.as_deref() != Some("air")
                        || self.units[id].air_patrol_pos == Some(target))
            })
            .collect();
        if military.is_empty() && city_id.is_none() {
            return Err("nothing to attack".into());
        }
        self.consume_unit_attack(uid);
        // City Center garrisons are protected while the city stands.
        if city_id.is_none() && !military.is_empty() {
            let did = *military
                .iter()
                .max_by(|a, b| {
                    let ea = effective_strength(
                        self.unit_strength(&self.units[*a], true),
                        self.units[*a].hp,
                    );
                    let eb = effective_strength(
                        self.unit_strength(&self.units[*b], true),
                        self.units[*b].hp,
                    );
                    ea.partial_cmp(&eb).unwrap()
                })
                .unwrap();
            let defender = self.units[&did].clone();
            let attacker = self.units[&uid].clone();
            let downer = defender.owner;
            self.record_war_unit_participation(&attacker, downer);
            self.record_war_unit_participation(&defender, attacker.owner);
            // As in `do_attack`: the strengths are stated once, in
            // `ranged_strike_strengths`, so a controller deciding whether to
            // stand under this shot prices the shot the engine will fire.
            let (att, ds) = self
                .ranged_strike_strengths(uid, did, target)
                .expect("both combatants exist at the shot");
            let dmg = damage(att, ds, &mut self.rng);
            self.apply_unit_damage(uid, did, dmg);
            let defender_dead = self.units[&did].hp <= 0;
            self.record_emergency_combat(pid, downer, defender_dead);
            self.award_unit_combat_xp(uid, &defender, true, true, defender_dead);
            if !defender_dead {
                self.award_unit_combat_xp(did, &attacker, true, false, false);
            }
            if defender_dead {
                self.note_underdog_kill(pid, &attacker, &defender);
                self.note_great_person_assisted_kill(pid, &attacker);
                self.record_kill(pid, Some(&attacker.kind), &defender);
                self.kill_rewards(&attacker, &defender);
                if self.has_ability(pid, "killer_of_cyrus") {
                    if let Some(attacker) = self.units.get_mut(&uid) {
                        attacker.hp = (attacker.hp + 30).min(100);
                    }
                }
                let downer = self.units[&did].owner;
                self.remove_unit(did);
                self.on_unit_lost(downer);
            }
        } else if let Some(cid) = city_id {
            let starting_hp = self.cities[&cid].hp;
            let defender = self.cities[&cid].owner;
            let attacker = self.units[&uid].clone();
            self.record_war_unit_participation(&attacker, defender);
            self.record_war_city_garrison_participation(cid, attacker.owner);
            let (att, cs) = self.ranged_city_strengths(uid, cid).unwrap();
            let dmg = damage(att, cs, &mut self.rng);
            let mult = if spec.siege || self.gdr_full_wall_damage(&self.units[&uid]) {
                1.0
            } else {
                0.5
            };
            self.city_take_damage(pid, cid, dmg, mult, false);
            if starting_hp <= 0 {
                // Shots after a Bombard attack has depleted the garrison
                // grant no XP and must not revive the city.
                self.cities.get_mut(&cid).unwrap().hp = 0;
            } else if self.cities[&cid].hp <= 0 && spec.siege {
                // Bombard-class shots may deplete a city, but still cannot
                // capture it. The depleting shot earns the city final-blow XP.
                self.cities.get_mut(&cid).unwrap().hp = 0;
                self.siege.left_depleted += 1;
                let city_pos = self.cities[&cid].pos;
                let taker_ready = self.units.values().any(|unit| {
                    unit.owner == pid
                        && unit.moves_left > 0.0
                        && self.wdist(unit.pos, city_pos) <= 1
                        && self.rules.units[unit.kind].is_melee_capable()
                });
                if taker_ready {
                    self.siege.depleted_with_a_taker_ready += 1;
                }
                self.award_initiated_combat_xp(uid, 10.0);
            } else {
                // Ordinary ranged attacks cannot reduce Garrison Health
                // below 1 and earn the normal city-attack XP.
                self.cities.get_mut(&cid).unwrap().hp = self.cities[&cid].hp.max(1);
                self.award_initiated_combat_xp(uid, 3.0);
            }
        }
        Ok(())
    }

    pub(in crate::game) fn do_pillage(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        let spec = &self.rules.units[unit.kind];
        if spec.class != "military"
            || spec.domain.as_deref() == Some("air")
            || self.is_embarked(&unit)
            || unit.moves_left <= 0.0
        {
            return Err("unit cannot pillage".into());
        }
        self.pillage_tile(pid, uid, unit.pos, false, true)?;
        let cost = if self.promotion_effect(&unit, "pillage_cost") > 0.0 {
            1.0
        } else {
            3.0
        };
        let unit = self.units.get_mut(&uid).unwrap();
        unit.moves_left = (unit.moves_left - cost).max(0.0);
        unit.acted = true;
        Ok(())
    }

    pub(in crate::game) fn do_air_rebase(
        &mut self,
        pid: usize,
        uid: u32,
        to: Pos,
    ) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        let spec = &self.rules.units[unit.kind];
        if spec.domain.as_deref() != Some("air")
            || unit.moves_left <= 0.0
            || unit.pos == to
            || self.wdist(unit.pos, to) > self.air_rebase_range(uid)
            || !self.can_air_base_at(pid, to, Some(uid))
        {
            return Err("aircraft cannot rebase there".into());
        }
        self.relocate(uid, to);
        let aircraft = self.units.get_mut(&uid).unwrap();
        aircraft.moves_left = 0.0;
        aircraft.attacks_left = 0;
        aircraft.acted = true;
        aircraft.air_patrol = false;
        aircraft.air_patrol_pos = None;
        Ok(())
    }

    pub(in crate::game) fn air_strike_unit_bonus(&self, attacker: &Unit, defender: &Unit) -> f64 {
        let attacker_spec = &self.rules.units[attacker.kind];
        let defender_spec = &self.rules.units[defender.kind];
        let mut bonus = self.air_matchup_bonus(attacker, defender);
        if defender_spec.domain.as_deref() == Some("sea") {
            bonus += self.promotion_effect(attacker, "air_vs_naval");
        } else if defender_spec.domain.as_deref() != Some("air") {
            bonus += self.promotion_effect(attacker, "air_vs_land");
        }
        if attacker_spec.promotion_class == "air_fighter"
            && matches!(
                defender_spec.promotion_class.as_str(),
                "light_cavalry" | "heavy_cavalry"
            )
        {
            bonus += self.promotion_effect(attacker, "vs_cavalry");
        } else if attacker_spec.promotion_class == "air_fighter"
            && defender_spec.domain.as_deref() != Some("air")
        {
            bonus += self.promotion_effect(attacker, "vs_non_cavalry");
        }
        bonus
    }

    pub(in crate::game) fn air_strike_unit_strength(
        &self,
        attacker: &Unit,
        defender: &Unit,
    ) -> f64 {
        let attacker_spec = &self.rules.units[attacker.kind];
        let defender_spec = &self.rules.units[defender.kind];
        self.unit_ranged_attack_strength(attacker) + self.air_strike_unit_bonus(attacker, defender)
            - if (attacker_spec.bombard_strength > 0.0
                && defender_spec.domain.as_deref() != Some("sea"))
                || (attacker_spec.ranged_strength > 0.0
                    && defender_spec.domain.as_deref() == Some("sea"))
            {
                17.0
            } else {
                0.0
            }
    }

    /// Resolve the strongest non-air interceptor and the strongest patrolling
    /// fighter separately. Civ VI permits one interception from each category
    /// on the same mission; adjacent same-category defenses add +5 support.
    /// Returns whether the attacker was destroyed and whether a fighter-class
    /// attacker was diverted into a dogfight instead of reaching its target.
    pub(in crate::game) fn resolve_air_interceptions(
        &mut self,
        attacker_id: u32,
        target: Pos,
    ) -> (bool, bool) {
        let attacker = self.units[&attacker_id].clone();
        let mut ground: Vec<(f64, u32)> = Vec::new();
        let mut fighters: Vec<(f64, u32)> = Vec::new();
        for unit in self.units.values().filter(|unit| {
            unit.owner != attacker.owner && self.is_at_war(attacker.owner, unit.owner)
        }) {
            let spec = &self.rules.units[unit.kind];
            if spec.domain.as_deref() == Some("air")
                && spec.promotion_class == "air_fighter"
                && unit.air_patrol
                && unit
                    .air_patrol_pos
                    .is_some_and(|patrol| self.wdist(patrol, target) <= 1)
            {
                let support = self
                    .units
                    .values()
                    .filter(|other| {
                        other.id != unit.id
                            && other.owner == unit.owner
                            && other.air_patrol
                            && self.rules.units[other.kind].promotion_class == "air_fighter"
                            && other
                                .air_patrol_pos
                                .is_some_and(|patrol| self.wdist(patrol, target) == 1)
                    })
                    .count() as f64
                    * 5.0;
                fighters.push((
                    self.unit_unembarked_strength(unit)
                        + self.air_matchup_bonus(unit, &attacker)
                        + support,
                    unit.id,
                ));
            } else if spec.domain.as_deref() != Some("air")
                && spec.anti_air_strength > 0.0
                && self.wdist(unit.pos, target) <= spec.anti_air_range.max(1)
            {
                let support = self
                    .units
                    .values()
                    .filter(|other| {
                        other.id != unit.id
                            && other.owner == unit.owner
                            && self.rules.units[other.kind].domain.as_deref() != Some("air")
                            && self.rules.units[other.kind].anti_air_strength > 0.0
                            && self.wdist(other.pos, target) == 1
                    })
                    .count() as f64
                    * 5.0;
                ground.push((self.unit_anti_air_strength(unit) + support, unit.id));
            }
        }
        ground.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        fighters.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));

        let mut fighter_engaged = false;
        for (strength, interceptor_id, air_interceptor) in ground
            .first()
            .map(|(strength, id)| (*strength, *id, false))
            .into_iter()
            .chain(
                fighters
                    .first()
                    .map(|(strength, id)| (*strength, *id, true)),
            )
        {
            if !self.units.contains_key(&attacker_id) {
                return (true, fighter_engaged);
            }
            let current_attacker = self.units[&attacker_id].clone();
            let interceptor = self.units[&interceptor_id].clone();
            self.record_war_unit_participation(&current_attacker, interceptor.owner);
            self.record_war_unit_participation(&interceptor, current_attacker.owner);
            let mut defense = self.unit_unembarked_strength(&current_attacker);
            if air_interceptor {
                defense += self.promotion_effect(&current_attacker, "defend_air_fighter")
                    + self.air_matchup_bonus(&current_attacker, &self.units[&interceptor_id]);
            } else {
                defense += self.promotion_effect(&current_attacker, "defend_anti_air");
            }
            let incoming = damage(
                effective_strength(strength, self.units[&interceptor_id].hp),
                effective_strength(defense.max(1.0), current_attacker.hp),
                &mut self.rng,
            );

            if air_interceptor {
                self.units.get_mut(&interceptor_id).unwrap().acted = true;
            }

            if air_interceptor
                && self.rules.units[current_attacker.kind].promotion_class == "air_fighter"
            {
                fighter_engaged = true;
                let counter_strength = self.unit_unembarked_strength(&current_attacker)
                    + self.air_matchup_bonus(&current_attacker, &interceptor);
                let interceptor_defense = self.unit_unembarked_strength(&interceptor)
                    + self.air_matchup_bonus(&interceptor, &current_attacker);
                let counter = damage(
                    effective_strength(counter_strength, current_attacker.hp),
                    effective_strength(interceptor_defense, interceptor.hp),
                    &mut self.rng,
                );
                self.apply_unit_damage(attacker_id, interceptor_id, counter);
                if self.units[&interceptor_id].hp <= 0 {
                    let owner = self.units[&interceptor_id].owner;
                    self.remove_unit(interceptor_id);
                    self.on_unit_lost(owner);
                }
            }

            self.apply_unit_damage(interceptor_id, attacker_id, incoming);
            if self.units[&attacker_id].hp <= 0 {
                self.remove_unit(attacker_id);
                self.on_unit_lost(attacker.owner);
                return (true, fighter_engaged);
            }
        }
        (false, fighter_engaged)
    }

    pub(in crate::game) fn do_air_strike(
        &mut self,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let attacker = self.own_unit(pid, uid)?;
        let spec = self.rules.units[attacker.kind].clone();
        if spec.domain.as_deref() != Some("air")
            || attacker.moves_left <= 0.0
            || attacker.attacks_left <= 0
            || self.strike_blocked(uid, target)
            || self.wdist(self.air_operation_origin(uid), target) > self.unit_attack_range(uid)
            || !self.enemy_air_strike_target_at(pid, target)
        {
            return Err("invalid air strike".into());
        }
        let visible = self.player_vision_frame(pid);
        let viewers = self.visibility_viewers(pid);
        if !self.combat_target_visible_at(pid, target, visible.as_ref(), &viewers) {
            return Err("invalid air strike".into());
        }
        let (destroyed, fighter_engaged) = self.resolve_air_interceptions(uid, target);
        if destroyed {
            return Ok(());
        }
        if fighter_engaged {
            self.consume_unit_attack(uid);
            if let Some(aircraft) = self.units.get_mut(&uid) {
                aircraft.air_patrol = false;
                aircraft.air_patrol_pos = None;
            }
            return Ok(());
        }
        let district_attack = effective_strength(
            self.unit_ranged_attack_strength(&self.units[&uid])
                - if spec.ranged_strength > 0.0 {
                    17.0
                } else {
                    0.0
                },
            self.units[&uid].hp,
        );
        if let Some(cid) = self.city_at(target) {
            if self.cities[&cid].owner != pid && self.is_at_war(pid, self.cities[&cid].owner) {
                self.record_war_unit_participation(&attacker, self.cities[&cid].owner);
                self.record_war_city_garrison_participation(cid, attacker.owner);
                let dealt = damage(district_attack, self.city_strength(cid), &mut self.rng);
                self.city_take_damage(pid, cid, dealt, if spec.siege { 1.0 } else { 0.5 }, false);
                if self.cities[&cid].hp <= 0 {
                    self.cities.get_mut(&cid).unwrap().hp = 1;
                }
            }
        } else if let Some(cid) = self.defending_district_at(target) {
            if self.cities[&cid].owner != pid && self.is_at_war(pid, self.cities[&cid].owner) {
                self.record_war_unit_participation(&attacker, self.cities[&cid].owner);
                let dealt = damage(
                    district_attack,
                    self.defending_district_strength(cid, target),
                    &mut self.rng,
                );
                let effectiveness = if spec.siege || self.gdr_full_wall_damage(&attacker) {
                    1.0
                } else {
                    0.5
                };
                self.defending_district_take_damage(cid, target, dealt, effectiveness, false);
                if self.defending_district_state(cid, target).unwrap().hp <= 0 {
                    self.set_defending_district_hp(cid, target, 1);
                }
            }
        } else if let Some(defender_id) = self.units_at(target).into_iter().find(|id| {
            self.units[id].owner != pid
                && self.is_at_war(pid, self.units[id].owner)
                && self.rules.units[self.units[id].kind].class == "military"
                && self.rules.units[self.units[id].kind].domain.as_deref() != Some("air")
                && self.unit_currently_visible_to(*id, pid)
        }) {
            let defender = self.units[&defender_id].clone();
            self.record_war_unit_participation(&attacker, defender.owner);
            self.record_war_unit_participation(&defender, attacker.owner);
            let anti_air = self.promotion_effect(&defender, "defend_air");
            let defender_spec = &self.rules.units[defender.kind];
            let defense_base = if defender_spec.anti_air_strength > 0.0 {
                self.unit_anti_air_strength(&defender)
            } else {
                self.unit_strength(&defender, true)
            } + anti_air;
            let specialized_attack = effective_strength(
                self.air_strike_unit_strength(&self.units[&uid], &defender),
                self.units[&uid].hp,
            );
            let defense = effective_strength(defense_base, defender.hp);
            let dealt = damage(specialized_attack, defense, &mut self.rng);
            self.apply_unit_damage(uid, defender_id, dealt);
            let killed = self.units[&defender_id].hp <= 0;
            self.record_emergency_combat(pid, defender.owner, killed);
            self.award_unit_combat_xp(uid, &defender, true, true, killed);
            if killed {
                let weapon = self.units[&uid].kind;
                self.note_underdog_kill(pid, &attacker, &defender);
                self.note_great_person_assisted_kill(pid, &attacker);
                self.record_kill(pid, Some(&weapon), &defender);
                self.remove_unit(defender_id);
                self.on_unit_lost(defender.owner);
            }
        } else if let Some(defender_id) = self.unit_ids_at(target).iter().find(|id| {
            self.units[id].owner != pid
                && self.is_at_war(pid, self.units[id].owner)
                && self.rules.units[self.units[id].kind].class == "support"
                && self.unit_currently_visible_to(**id, pid)
        }) {
            self.priority_damage_support(pid, uid, *defender_id);
        }
        self.consume_unit_attack(uid);
        if let Some(aircraft) = self.units.get_mut(&uid) {
            aircraft.air_patrol = false;
            aircraft.air_patrol_pos = None;
        }
        Ok(())
    }

    pub(in crate::game) fn do_air_pillage(
        &mut self,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let bomber = self.own_unit(pid, uid)?;
        let spec = &self.rules.units[bomber.kind];
        if spec.domain.as_deref() != Some("air")
            || spec.promotion_class != "air_bomber"
            || bomber.moves_left <= 0.0
            || bomber.attacks_left <= 0
            || self.strike_blocked(uid, target)
            || (bomber.hp < 50
                && self.promotion_effect(&bomber, "air_pillage_at_low_health") <= 0.0)
            || self.wdist(self.air_operation_origin(uid), target) > self.unit_attack_range(uid)
            || !self.player_can_see(pid, target)
            || !self.air_pillageable_at(pid, target)
        {
            return Err("invalid air pillage".into());
        }

        let (destroyed, _) = self.resolve_air_interceptions(uid, target);
        if destroyed {
            return Ok(());
        }
        if self.units[&uid].hp < 50
            && self.promotion_effect(&self.units[&uid], "air_pillage_at_low_health") <= 0.0
        {
            // The official operation resolves interception before checking
            // whether an ordinary bomber still has the required 50 HP.
            self.consume_unit_attack(uid);
            return Ok(());
        }
        // This applies exactly one normal pillage layer, including scattering
        // aircraft from a disabled Aerodrome, but Air Pillage awards no loot.
        self.pillage_tile(pid, uid, target, false, false)?;
        self.consume_unit_attack(uid);
        if let Some(aircraft) = self.units.get_mut(&uid) {
            aircraft.air_patrol = false;
            aircraft.air_patrol_pos = None;
        }
        Ok(())
    }

    pub(in crate::game) fn do_air_patrol(
        &mut self,
        pid: usize,
        uid: u32,
        to: Pos,
    ) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        let spec = &self.rules.units[unit.kind];
        if spec.domain.as_deref() != Some("air")
            || spec.siege
            || unit.moves_left <= 0.0
            || unit.attacks_left <= 0
            || self.map.get(to).is_none()
            || self.wdist(unit.pos, to) > spec.moves.floor() as i32
            || !self.can_air_patrol_at(pid, to)
        {
            return Err("aircraft cannot patrol".into());
        }
        let unit = self.units.get_mut(&uid).unwrap();
        unit.air_patrol = true;
        unit.air_patrol_pos = Some(to);
        unit.moves_left = 0.0;
        unit.attacks_left = 0;
        unit.acted = true;
        self.reveal(pid, to, spec.sight);
        Ok(())
    }

    /// Isibongo upgrades the unit which actually takes a city to the highest
    /// formation tier Shaka's current civics permit.
    pub(in crate::game) fn apply_capture_formation_upgrade(&mut self, pid: usize, uid: u32) {
        if self.civ_effect(pid, "capture_formation_upgrade") <= 0.0 {
            return;
        }
        let Some(unit) = self.units.get(&uid) else {
            return;
        };
        let spec = &self.rules.units[unit.kind];
        if spec.class != "military" || spec.domain.as_deref() == Some("air") || !spec.can_formations
        {
            return;
        }
        let target = if self.formation_unlocked(pid, &unit.kind, 2) {
            2
        } else if self.formation_unlocked(pid, &unit.kind, 1) {
            1
        } else {
            0
        };
        let old = unit.formation;
        if target > old {
            self.set_unit_formation(uid, target);
            if old == 0 && target == 1 {
                bump(&mut self.players[pid], "corps");
            }
            self.note_formation_moment(pid, uid);
        }
    }

    /// Every unit able to deliver a device this turn: its id, where it would
    /// launch from, and how far it reaches.
    ///
    /// Civilization VI's rule, from the shipped Civilopedia
    /// (`LOC_PEDIA_CONCEPTS_PAGE_NUKES_1_CHAPTER_CONTENT_PARA_1`): a device "can
    /// then be used by any unit or improvement capable of deploying it on the
    /// map. This includes bomber aircraft, Nuclear Submarines, and the Missile
    /// Silo." The carriers are the `Units.WMDCapable` rows. A Nuclear Submarine
    /// launches at the device's own strike range. An aircraft drops the device
    /// and so reaches only as far as it can fly.
    pub(crate) fn wmd_carriers(&self, pid: usize, thermonuclear: bool) -> Vec<(u32, Pos, i32)> {
        let device_range = self.wmd_spec(thermonuclear).icbm_strike_range;
        self.units
            .iter()
            .filter(|(_, unit)| {
                unit.owner == pid
                    && unit.attacks_left > 0
                    && unit.moves_left > 0.0
                    && self.rules.units[unit.kind].wmd_capable
            })
            .map(|(uid, unit)| {
                let flies = self.rules.units[unit.kind].domain.as_deref() == Some("air");
                let reach = if flies {
                    self.unit_attack_range(*uid)
                } else {
                    device_range
                };
                (*uid, self.air_operation_origin(*uid), reach)
            })
            .collect()
    }

    fn wmd_spec(&self, thermonuclear: bool) -> &crate::rules::WmdSpec {
        &self.rules.wmds[if thermonuclear {
            "thermonuclear_device"
        } else {
            "nuclear_device"
        }]
    }

    /// Every way this civilization can put a device on `target` this turn,
    /// best first. `city` limits the silos to that city's; carriers answer to
    /// the empire, so they are offered either way.
    ///
    /// A silo goes first because a launch costs it nothing, where a carrier
    /// spends its attack. A city center is not a launch site: the only city
    /// launch the shipped game has is `CityManager.RequestCommand(fromCity,
    /// CityCommandTypes.WMD_STRIKE, …)` aimed from a silo's plot
    /// (`Base/Assets/UI/WorldInput.lua:2264-2272`), and the banner that offers
    /// it is `BANNERTYPE_MISSILE_SILO`.
    pub fn wmd_platforms(
        &self,
        pid: usize,
        city: Option<u32>,
        target: Pos,
        thermonuclear: bool,
    ) -> Vec<WmdPlatform> {
        let device_range = self.wmd_spec(thermonuclear).icbm_strike_range;
        let mut platforms = Vec::new();
        for owned in self
            .cities
            .values()
            .filter(|owned| owned.owner == pid && city.is_none_or(|named| named == owned.id))
        {
            for position in &owned.owned_tiles {
                let working_silo = self.map.tiles.get(position).is_some_and(|tile| {
                    tile.improvement.as_deref() == Some("missile_silo") && !tile.pillaged
                });
                let distance = self.wdist(*position, target);
                if working_silo && distance <= device_range {
                    platforms.push(WmdPlatform {
                        kind: crate::name!("missile_silo"),
                        from: *position,
                        unit: None,
                        distance,
                    });
                }
            }
        }
        for (uid, from, reach) in self.wmd_carriers(pid, thermonuclear) {
            let distance = self.wdist(from, target);
            if distance <= reach {
                platforms.push(WmdPlatform {
                    kind: self.units[&uid].kind,
                    from,
                    unit: Some(uid),
                    distance,
                });
            }
        }
        platforms.sort_by_key(|platform| {
            (
                platform.unit.is_some(),
                platform.distance,
                platform.from.0,
                platform.from.1,
                platform.unit,
            )
        });
        platforms
    }

    /// Whether anything this civilization holds can deliver a device there.
    pub fn wmd_can_reach(&self, pid: usize, target: Pos, thermonuclear: bool) -> bool {
        !self
            .wmd_platforms(pid, None, target, thermonuclear)
            .is_empty()
    }

    /// Launch a stockpiled device. Range, blast radius and fallout duration
    /// are the shipped WMDs rows; the per-ring unit damage is the one number
    /// the database does not carry.
    pub(in crate::game) fn do_wmd_strike(
        &mut self,
        pid: usize,
        cid: u32,
        target: Pos,
        thermonuclear: bool,
    ) -> Result<(), String> {
        if self
            .cities
            .get(&cid)
            .filter(|city| city.owner == pid)
            .is_none()
        {
            return Err("launch city must be yours".into());
        }
        let device_key = if thermonuclear {
            "project_effect:thermonuclear_devices"
        } else {
            "project_effect:nuclear_devices"
        };
        if self.players[pid]
            .counters
            .get(device_key)
            .copied()
            .unwrap_or(0)
            <= 0
        {
            return Err("no device of that type in the stockpile".into());
        }
        let spec = self.rules.wmds[if thermonuclear {
            "thermonuclear_device"
        } else {
            "nuclear_device"
        }]
        .clone();
        if !self.map.tiles.contains_key(&target) {
            return Err("no such tile".into());
        }
        if !self.players[pid].explored.contains(&target) {
            return Err("target tile is unrevealed".into());
        }
        let carrier = self
            .wmd_platforms(pid, Some(cid), target, thermonuclear)
            .into_iter()
            .next()
            .ok_or_else(|| "target out of ICBM range".to_string())?;
        let platform = carrier.kind.to_string();
        let launched_from = carrier.from;
        // Nuking a major you are at peace with is not a legal order.
        let blast: Vec<Pos> = self.wdisk(target, spec.blast_radius);
        let mut targeted_owners = BTreeSet::new();
        for position in &blast {
            let victims = self
                .unit_ids_at(*position)
                .iter()
                .map(|uid| self.units[uid].owner)
                .chain(self.city_at(*position).map(|c| self.cities[&c].owner))
                .chain(
                    self.defending_district_at(*position)
                        .map(|c| self.cities[&c].owner),
                );
            for owner in victims {
                let victim = &self.players[owner];
                if owner != pid && !victim.is_barbarian && !self.is_at_war(pid, owner) {
                    return Err("cannot nuke a civilization you are at peace with".into());
                }
                if owner != pid && !victim.is_barbarian {
                    targeted_owners.insert(owner);
                }
            }
        }
        let fallout_until = self.turn.saturating_add(spec.fallout_duration);
        *self.players[pid]
            .counters
            .entry(device_key.to_string())
            .or_insert(0) -= 1;
        bump(&mut self.players[pid], "wmd_strikes");
        let mut aggrieved: BTreeSet<usize> = BTreeSet::new();
        // The cities under the blast, named for the war ledger before the
        // detonation halves them.
        let struck_cities: Vec<(u32, usize, String)> = blast
            .iter()
            .filter_map(|position| self.city_at(*position))
            .filter_map(|cid| self.cities.get(&cid).map(|city| (cid, city)))
            .filter(|(_, city)| city.owner != pid)
            .map(|(cid, city)| (cid, city.owner, city.name.clone()))
            .collect();
        // Everything standing in the radius before the blast, so the record can
        // report what the device actually killed rather than what it aimed at.
        let exposed: Vec<u32> = blast
            .iter()
            .flat_map(|position| self.units_at(*position))
            .collect();
        let exposed_military = exposed
            .iter()
            .filter_map(|unit| self.units.get(unit))
            .filter(|unit| unit.owner != pid && self.rules.units[unit.kind].class == "military")
            .cloned()
            .collect::<Vec<_>>();
        for defender in &exposed_military {
            self.record_war_unit_participation(defender, pid);
        }
        let launch_unit = carrier.unit.and_then(|uid| self.units.get(&uid).cloned());
        // `UNITOPERATION_WMD_STRIKE` is an attack (`CategoryInUI="ATTACK"`,
        // Base/Assets/Gameplay/Data/UnitOperations.xml:87), so a carrier spends
        // its attack on it. An aircraft comes off patrol the way a sortie does.
        if let Some(uid) = carrier.unit {
            self.consume_unit_attack(uid);
            let aircraft = self.units.get_mut(&uid).expect("the carrier was just read");
            aircraft.air_patrol = false;
            aircraft.air_patrol_pos = None;
        }
        for position in blast.iter().copied() {
            if let Some(tile) = self.map.tiles.get_mut(&position) {
                tile.fallout_until = tile.fallout_until.max(fallout_until);
            }
            // Ground zero is lethal to full-health units; the outer rings
            // wound severely.
            let ring = self.wdist(position, target);
            let unit_damage = match ring {
                0 => 100,
                1 => 80,
                _ => 60,
            };
            if let Some(owner) = self
                .map
                .get(position)
                .and_then(|tile| tile.owner_city)
                .and_then(|c| self.cities.get(&c))
                .map(|c| c.owner)
            {
                if owner != pid {
                    aggrieved.insert(owner);
                }
            }
            self.damage_tile_area(position, unit_damage, Some(pid));
            if let Some(struck) = self.city_at(position) {
                let city = self.cities.get_mut(&struck).unwrap();
                // Half the population and the Outer Defenses go with the blast.
                city.pop = (city.pop - (city.pop / 2)).max(1);
                city.wall_hp = if ring == 0 { 0 } else { city.wall_hp / 2 };
            }
        }
        for owner in &aggrieved {
            // The world does not forgive a nuclear strike quickly.
            self.add_grievances(*owner, pid, 150.0);
        }
        // A detonation belongs in the account of the war it was used in — on
        // the city it hit where it hit one, and otherwise on the front whose
        // land took it.
        let mut struck_owners: BTreeSet<usize> = BTreeSet::new();
        let mut struck_names: Vec<String> = Vec::new();
        for (_, owner, name) in struck_cities {
            struck_owners.insert(owner);
            struck_names.push(name.clone());
            self.record_war_moment(pid, owner, "nuclear_strike", Some(name), Some(target));
        }
        for owner in &aggrieved {
            if !struck_owners.contains(owner) {
                self.record_war_moment(pid, *owner, "nuclear_strike", None, Some(target));
            }
        }
        let units_destroyed = exposed
            .into_iter()
            .filter(|uid| !self.units.contains_key(uid))
            .count() as u32;
        let victims: BTreeSet<usize> = targeted_owners
            .union(&aggrieved)
            .copied()
            .filter(|owner| *owner != pid)
            .collect();
        if let Some(launcher) = &launch_unit {
            for owner in &victims {
                self.record_war_unit_participation(launcher, *owner);
            }
        }
        let strike = NuclearStrike {
            id: self.allocate_conflict_id(),
            turn: self.turn,
            attacker: pid,
            target,
            thermonuclear,
            platform,
            launched_from,
            blast_radius: spec.blast_radius,
            fallout_until,
            victims: victims.clone(),
            cities: struck_names.clone(),
            units_destroyed,
        };
        self.announce_nuclear_strike(&strike, &struck_names);
        self.nuclear_strikes.push(strike);
        // A client animates the tail of this list and a log describes it; the
        // full history of a long nuclear war is not worth carrying in every
        // frame forever.
        const STRIKES_KEPT: usize = 32;
        if self.nuclear_strikes.len() > STRIKES_KEPT {
            let excess = self.nuclear_strikes.len() - STRIKES_KEPT;
            self.nuclear_strikes.drain(..excess);
        }
        Ok(())
    }

    /// Tell the world. A detonation is not private news: the launcher's own
    /// log, every civilization that lost something to it, and every other
    /// living major all get an entry, and all three are marked important so a
    /// log scrolling at speed holds them instead of letting the largest event
    /// in the game slide past between two city-growth notices.
    pub(in crate::game) fn announce_nuclear_strike(
        &mut self,
        strike: &NuclearStrike,
        struck_names: &[String],
    ) {
        let attacker = strike.attacker;
        let weapon = if strike.thermonuclear {
            "thermonuclear device"
        } else {
            "nuclear device"
        };
        // Name the place it landed on: the city if it hit one, otherwise the
        // civilization whose ground took it, otherwise bare coordinates.
        let place = struck_names.first().cloned().or_else(|| {
            strike
                .victims
                .iter()
                .next()
                .map(|victim| format!("{} territory", self.civ_name(*victim)))
        });
        let where_it_landed = match &place {
            Some(place) => format!(" on {place}"),
            None => String::new(),
        };
        let toll = match strike.units_destroyed {
            0 => String::new(),
            1 => " · 1 unit lost".to_string(),
            count => format!(" · {count} units lost"),
        };
        let from = match strike.platform.as_str() {
            "missile_silo" => " from a Missile Silo",
            "nuclear_submarine" => " from a Nuclear Submarine",
            "bomber" => " from a Bomber",
            "jet_bomber" => " from a Jet Bomber",
            _ => "",
        };
        self.note_important(
            attacker,
            "Nuclear",
            format!("detonated a {weapon}{where_it_landed}{from}{toll}"),
            Some(strike.target),
        );
        let aggressor = self.civ_name(attacker);
        for victim in strike.victims.iter().copied() {
            // A device that lands on open ground has no city to name, and
            // "…: was struck" is not a sentence. Name the ground instead.
            let hit = match struck_names.first() {
                Some(city) => format!("{city} was struck"),
                None => format!("{} territory was struck", self.civ_name(victim)),
            };
            self.note_important(
                victim,
                "Nuclear",
                format!("{aggressor} detonated a {weapon}: {hit}{toll}"),
                Some(strike.target),
            );
        }
        // Nobody misses a mushroom cloud, whatever the fog says.
        let bystanders: Vec<usize> = self
            .players
            .iter()
            .filter(|player| {
                player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && player.id != attacker
                    && !strike.victims.contains(&player.id)
            })
            .map(|player| player.id)
            .collect();
        for bystander in bystanders {
            self.note_important(
                bystander,
                "Nuclear",
                format!("{aggressor} detonated a {weapon}{where_it_landed}"),
                Some(strike.target),
            );
        }
    }

    pub(in crate::game) fn do_city_strike(
        &mut self,
        pid: usize,
        cid: u32,
        target: Pos,
    ) -> Result<(), String> {
        match self.cities.get(&cid) {
            Some(c) if c.owner == pid => {}
            _ => return Err("not your city".into()),
        }
        if !self.city_can_strike(&self.cities[&cid]) {
            return Err("city cannot strike".into());
        }
        if self.wdist(self.cities[&cid].pos, target) > 2 {
            return Err("out of range".into());
        }
        if self.city_at(target).is_some() || self.defending_district_at(target).is_some() {
            return Err("cities cannot strike defensible districts".into());
        }
        let visible = self.player_vision_frame(pid);
        let viewers = self.visibility_viewers(pid);
        if !self.combat_target_visible_at(pid, target, visible.as_ref(), &viewers) {
            return Err("target is not visible".into());
        }
        if !self.has_line_of_sight(self.cities[&cid].pos, target, true) {
            return Err("line of sight blocked".into());
        }
        // See `peaceful_foreign_unit_at`. The scan below is existential and
        // would happily name the barbarian standing beside a rival's Trader.
        if self.peaceful_foreign_unit_at(pid, target) {
            return Err("a unit we are at peace with stands there".into());
        }
        let enemies: Vec<u32> = self
            .unit_ids_at(target)
            .iter()
            .filter(|id| {
                let o = &self.units[id];
                o.owner != pid
                    && self.is_at_war(pid, o.owner)
                    && self.unit_currently_visible_to(**id, pid)
            })
            .copied()
            .collect();
        if enemies.is_empty() {
            return Err("no enemy target".into());
        }
        let military: Vec<u32> = enemies
            .iter()
            .cloned()
            .filter(|id| self.rules.units[self.units[id].kind].class == "military")
            .collect();
        if military.is_empty() {
            return Err("no enemy military target".into());
        }
        let did = *military
            .iter()
            .max_by(|a, b| {
                let ea = effective_strength(
                    self.unit_strength(&self.units[*a], true),
                    self.units[*a].hp,
                );
                let eb = effective_strength(
                    self.unit_strength(&self.units[*b], true),
                    self.units[*b].hp,
                );
                ea.partial_cmp(&eb).unwrap()
            })
            .unwrap();
        let d = self.units[&did].clone();
        self.record_war_unit_participation(&d, pid);
        let ds = effective_strength(
            self.unit_strength(&d, true) + self.ranged_defense_bonus(&d, true),
            d.hp,
        ) + self.tile_defense_bonus(target);
        let naval = self.rules.units[d.kind].domain.as_deref() == Some("sea");
        let emergency_bonus = self.players[pid]
            .counters
            .get(&format!("emergency_city_strike_vs:{}", d.owner))
            .copied()
            .unwrap_or(0) as f64;
        let att = self.city_ranged_strength(cid) + emergency_bonus - if naval { 17.0 } else { 0.0 };
        let dmg = damage(att, ds, &mut self.rng);
        self.units.get_mut(&did).unwrap().hp -= dmg;
        let defender_dead = self.units[&did].hp <= 0;
        self.record_emergency_combat(pid, d.owner, defender_dead);
        if !defender_dead {
            self.award_xp(did, 2.0);
        } else {
            self.record_kill(pid, None, &d);
            let downer = self.units[&did].owner;
            self.remove_unit(did);
            self.on_unit_lost(downer);
        }
        let city = self.cities.get_mut(&cid).unwrap();
        if city.struck {
            city.extra_strikes_used += 1;
        }
        city.struck = true;
        Ok(())
    }

    pub(in crate::game) fn do_encampment_strike(
        &mut self,
        pid: usize,
        cid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let city = self
            .cities
            .get(&cid)
            .filter(|city| city.owner == pid)
            .ok_or_else(|| "not your city".to_string())?;
        let position = self
            .city_district_family_position(city, crate::name!("encampment"))
            .ok_or_else(|| "city has no Encampment".to_string())?;
        self.do_defending_district_strike(pid, cid, position, target)
    }

    /// The two strengths a shot at a city resolves with, `(shooter, city)`,
    /// shared by `do_ranged` and the combat forecast.
    pub(crate) fn ranged_city_strengths(&self, uid: u32, cid: u32) -> Option<(f64, f64)> {
        let shooter = self.units.get(&uid)?;
        let city = self.cities.get(&cid)?;
        let spec = &self.rules.units[shooter.kind];
        let mut attack = self.unit_ranged_attack_strength(shooter)
            + self.promotion_effect(shooter, "ranged_vs_district")
            + self.gdr_siege_bonus(shooter)
            + self.vs_bonus(shooter.owner, city.owner);
        if spec.ranged_strength > 0.0 && spec.domain.as_deref() != Some("sea") {
            attack -= 17.0;
        }
        Some((
            effective_strength(attack, shooter.hp),
            self.city_strength(cid),
        ))
    }

    /// City melee strengths shared by the resolver and capture estimates.
    /// Embarked units attack with their land strength and a landing penalty;
    /// their embarked defensive strength is not an attack strength.
    pub(crate) fn city_melee_exchange_strengths(&self, uid: u32, cid: u32) -> Option<(f64, f64)> {
        let attacker = self.units.get(&uid)?;
        let city = self.cities.get(&cid)?;
        let unamphibious = self.promotion_effect(attacker, "amphibious") == 0.0;
        let mut attack =
            self.unit_unembarked_strength(attacker) + self.vs_bonus(attacker.owner, city.owner);
        if self.is_embarked(attacker) && unamphibious {
            attack -= 10.0;
        }
        let defense = self.city_strength(cid)
            + if self.crosses_river(attacker.pos, city.pos) && unamphibious {
                5.0
            } else {
                0.0
            };
        Some((effective_strength(attack, attacker.hp), defense))
    }
}
