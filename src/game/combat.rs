//! Combat: what a unit can attack, how strong it is, and what a fight
//! leaves behind.
//!
//! Attack range and reach (`unit_attack_range`, `attack_reach` and its
//! flood-fill cache), how many attacks a unit has and whether siege can
//! attack after moving, ranged attack strength and the government,
//! governor and ranged defence bonuses, city strikes and city damage, the
//! mass driver, combat XP, combat weariness, and emergency combat records.
//!
//! Experience and promotions: the XP a unit earns, scaled by difficulty
//! (`modified_xp`, `handicap_xp_pct`, `award_xp`), the threshold and
//! pending promotion, the promotions a unit may take and the offer key the
//! host uses, a promotion's or unit ability's effect (`promotion_effect`,
//! `unit_effect`), the Rock Band's any-promotion rule, the
//! versus-class bonus (`vs_bonus`), and formations (`formation_unlocked`,
//! `unit_formation_bonus`). These moved verbatim out of the main
//! `impl Game` block in `game.rs` later the same day.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

/// One read-only attack query's ordered target union. Tile indices already
/// follow Pos order. Coordinates that disagree with that index retain the
/// original Vec sort/dedup path, including off-map and mirrored records.
struct AttackTargetUnion<'a> {
    tiles: &'a crate::world::TileGrid,
    in_map: crate::world::TileBits,
    other: Vec<Pos>,
}

impl<'a> AttackTargetUnion<'a> {
    fn new(tiles: &'a crate::world::TileGrid) -> Self {
        Self {
            tiles,
            in_map: crate::world::TileBits::with_capacity(tiles.len()),
            other: Vec::new(),
        }
    }

    #[inline]
    fn push(&mut self, pos: Pos) {
        if let Some(index) = self.tiles.index_of(pos) {
            if self.tiles.values().as_slice()[index].pos == pos {
                self.in_map.insert(index);
                return;
            }
        }
        self.other.push(pos);
    }

    fn into_sorted(mut self) -> Vec<Pos> {
        let tile_values = self.tiles.values();
        let dense_tiles = tile_values.as_slice();
        let mut targets = Vec::with_capacity(self.in_map.iter().count() + self.other.len());
        targets.extend(self.in_map.iter().map(|index| dense_tiles[index].pos));
        if !self.other.is_empty() {
            targets.append(&mut self.other);
            targets.sort_unstable();
            targets.dedup();
        }
        targets
    }
}

impl Game {
    /// Charge one bout of fighting. `WAR_WEARINESS_PER_COMBAT_IN_FOREIGN_LANDS`
    /// is 2 and `_IN_ALLIED_LANDS` is 1; fighting on your own soil is the
    /// cheapest case. Policy and government reductions apply here, which is
    /// what makes `war_weariness_multiplier` mean anything.
    pub(super) fn accrue_combat_weariness(&mut self, pid: usize, at: Pos) {
        if !self.at_war_with_any_civilization(pid) {
            return;
        }
        let owner = self
            .map
            .get(at)
            .and_then(|tile| tile.owner_city)
            .and_then(|cid| self.cities.get(&cid))
            .map(|city| city.owner);
        let home = owner == Some(pid);
        let points = match owner {
            Some(other) if other == pid => 1.0,
            Some(other) if self.are_allied(pid, other) || self.same_team(pid, other) => 1.0,
            _ => 2.0,
        };
        self.add_war_weariness(pid, points * self.war_weariness_multiplier(pid, home));
    }

    /// Whether a siege unit may still attack after moving this turn.
    /// Civilization VI's data gives every siege unit "Cannot move and attack
    /// in the same turn" (`ABILITY_NO_MOVE_AND_SHOOT`), lifted by Expert Crew.
    /// Our own seat was exempt from October 2026 on a measurement that 94 of
    /// 104 such shots resolved. In the 33 live logs of 2026-11-09 and 10 with
    /// siege shots, the host refused 75 of 76, and the exemption left guns
    /// marched into a city's reach to fire shots that never came.
    pub(crate) fn siege_may_attack_after_moving(&self, unit: &Unit) -> bool {
        self.promotion_effect(unit, "attack_after_move") > 0.0
    }

    pub(super) fn unit_max_attacks(&self, uid: u32) -> i32 {
        self.units
            .get(&uid)
            .map(|unit| 1 + self.promotion_effect(unit, "extra_attacks") as i32)
            .unwrap_or(1)
    }

    /// Kabul doubles experience from every battle the unit initiates,
    /// including attacks on City Centers and Encampments. Unit-vs-unit
    /// combat applies its strength-sensitive award separately below.
    pub(super) fn award_initiated_combat_xp(&mut self, uid: u32, amount: f64) {
        let multiplier = if self
            .units
            .get(&uid)
            .is_some_and(|unit| self.grants_city_state_unique_bonus(unit.owner, "Kabul"))
        {
            2.0
        } else {
            1.0
        };
        self.award_xp(uid, amount * multiplier);
    }

    /// Unit-vs-unit XP is based on relative base Combat Strength. Kills
    /// double that component. The final, modified award is capped at 8 XP.
    pub(super) fn award_unit_combat_xp(
        &mut self,
        uid: u32,
        opponent: &Unit,
        ranged: bool,
        attacking: bool,
        killed_opponent: bool,
    ) {
        let Some(unit) = self.units.get(&uid) else {
            return;
        };
        if !self.rules.units[unit.kind].earns_xp {
            return;
        }
        if self.promotion_pending(uid) {
            return;
        }
        if self.players[opponent.owner].is_barbarian && unit.level >= 2 {
            // Once a unit has its first promotion, combat with Barbarians and
            // Free Cities grants exactly 1 XP regardless of other modifiers.
            self.units.get_mut(&uid).unwrap().xp += 1;
            return;
        }
        let own = self.rules.units[unit.kind].strength.max(1.0);
        let other = self.rules.units[opponent.kind].strength.max(1.0);
        let mut relative = other / own;
        if killed_opponent {
            relative *= 2.0;
        }
        let mut amount =
            relative + if ranged { 1.0 } else { 2.0 } + if attacking { 1.0 } else { 0.0 };
        if attacking && self.grants_city_state_unique_bonus(unit.owner, "Kabul") {
            amount *= 2.0;
        }
        // The shipped cap on experience from one combat (8 since Rise and Fall).
        let gained = self.modified_xp(uid, amount).min(8);
        self.units.get_mut(&uid).unwrap().xp += gained;
    }

    /// Whether this unit's domain lets it melee that tile at all -- the check
    /// `do_attack` applies before anything else about the fight. A land unit
    /// proposing a shot at a ship never gets past here.
    pub(crate) fn unit_can_melee_target_domain(&self, uid: u32, target: Pos) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        let Some(tile) = self.map.get(target) else {
            return false;
        };
        let sea_unit = self.rules.units[unit.kind].domain.as_deref() == Some("sea");
        if sea_unit || unit.kind == "giant_death_robot" {
            self.unit_can_traverse(uid, target)
        } else {
            // Land units may disembark into a melee attack, but may never
            // initiate one into a water tile.
            !self.rules.is_water(tile)
        }
    }

    fn oligarchy_applies(spec: &crate::rules::UnitSpec) -> bool {
        matches!(
            spec.promotion_class.as_str(),
            "melee" | "anti_cavalry" | "naval_melee"
        )
    }

    /// The government's Combat Strength, with the two conditions the shipped
    /// abilities carry. Oligarchy's +4 reaches melee, anti-cavalry and naval
    /// melee only. Fascism's is `FASCISM_ATTACK_BUFF`, whose
    /// `FASCISM_REQUIREMENTS` is a single `REQUIREMENT_PLAYER_IS_ATTACKING` —
    /// it is an attacking bonus, and paying it on defence made the domination
    /// government strictly stronger than the one Civ VI ships.
    pub(super) fn government_combat_bonus(&self, u: &Unit, attacking: bool) -> f64 {
        let bonus = self.gov_effects(u.owner).combat_strength;
        match self.players[u.owner].government.as_deref() {
            Some("oligarchy") if !Self::oligarchy_applies(&self.rules.units[u.kind]) => 0.0,
            Some("fascism") if !attacking => 0.0,
            _ => bonus,
        }
    }

    pub(super) fn governor_unit_defense_bonus(&self, unit: &Unit) -> f64 {
        self.map.tiles[&unit.pos]
            .owner_city
            .filter(|city_id| {
                self.cities
                    .get(city_id)
                    .is_some_and(|city| city.owner == unit.owner)
            })
            .map(|city_id| self.governor_effect(unit.owner, city_id, "city_unit_defense"))
            .unwrap_or(0.0)
    }

    pub(super) fn ranged_defense_bonus(&self, unit: &Unit, city_attack: bool) -> f64 {
        self.unit_effect(unit, "defend_ranged")
            + if city_attack {
                self.unit_effect(unit, "defend_city_attack")
            } else {
                0.0
            }
    }

    pub fn unit_ranged_strength(&self, u: &Unit) -> f64 {
        let rs = self.rules.units[u.kind].ranged_strength;
        if rs <= 0.0 {
            return 0.0;
        }
        // Ranged Strength is what a unit attacks with.
        rs + self.government_combat_bonus(u, true)
            // Leaders.xml:1463-1465 adjusts unit combat, with no weapon-class
            // restriction. The same difficulty bonus already reaches melee.
            + self.handicap_combat_strength(u.owner)
            + self.mongolian_cavalry_strength(u)
            + self.unit_formation_bonus(u)
            + self.congress_military_strength_bonus(u)
            + self.religious_combat_belief_bonus(u)
            + (10.0 - u.hp.clamp(0, 100) as f64 / 10.0).round()
                * self.policy_effect(u.owner, "wounded_penalty_reduction_pct")
                / 100.0
            + if self.has_ability(u.owner, "gifts_for_the_tlatoani") {
                self.empire_luxuries(u.owner) as f64
            } else {
                0.0
            }
    }

    pub fn unit_bombard_strength(&self, u: &Unit) -> f64 {
        let bs = self.rules.units[u.kind].bombard_strength;
        if bs <= 0.0 {
            return 0.0;
        }
        // Gathering Storm's CLASS_TARGETTING_ASSIST includes only these two
        // units (Expansion1_Units.xml:37-38), not every Bombard-stat attacker.
        bs + if matches!(u.kind.as_str(), "artillery" | "rocket_artillery") {
            self.adjacent_support_effect(u, "adjacent_siege_bombard")
        } else {
            0.0
        }
            // Bombarding a city is an attack.
            + self.government_combat_bonus(u, true)
            + self.handicap_combat_strength(u.owner)
            + self.unit_formation_bonus(u)
            + self.congress_military_strength_bonus(u)
            + self.religious_combat_belief_bonus(u)
            + (10.0 - u.hp.clamp(0, 100) as f64 / 10.0).round()
                * self.policy_effect(u.owner, "wounded_penalty_reduction_pct")
                / 100.0
            + if self.has_ability(u.owner, "gifts_for_the_tlatoani") {
                self.empire_luxuries(u.owner) as f64
            } else {
                0.0
            }
    }

    pub fn unit_ranged_attack_strength(&self, u: &Unit) -> f64 {
        self.unit_ranged_strength(u)
            .max(self.unit_bombard_strength(u))
    }

    pub fn city_can_strike(&self, city: &City) -> bool {
        city.wall_hp > 0
            && (!city.struck
                || city.extra_strikes_used
                    < self.governor_effect(city.owner, city.id, "city_extra_strike") as i32)
        // Ranged strikes require standing walls; Embrasure supplies one more.
    }

    /// Route an attack roll into a walled city: walls absorb it (melee does
    /// 15%, ranged 50%, siege 100% of the roll to walls), while the city
    /// itself takes 1 damage behind healthy walls (>=80%), half through
    /// damaged walls, and full damage once breached (<20%) or bare (Civ 6).
    pub(super) fn city_take_damage(
        &mut self,
        attacker: usize,
        cid: u32,
        dmg: i32,
        wall_mult: f64,
        bypass_walls: bool,
    ) {
        let (wall, max) = {
            let c = &self.cities[&cid];
            (c.wall_hp, self.city_max_wall_hp(c))
        };
        let c = self.cities.get_mut(&cid).unwrap();
        c.last_attacked = self.turn;
        let before_hp = c.hp;
        if wall > 0 && max > 0 {
            let frac = wall as f64 / max as f64;
            let through = if bypass_walls {
                dmg // siege tower: attackers pour past the walls (Civ 6)
            } else if frac >= 0.8 {
                1
            } else if frac >= 0.2 {
                dmg / 2
            } else {
                dmg
            };
            c.wall_hp = (wall - ((dmg as f64 * wall_mult).round() as i32).max(1)).max(0);
            c.hp -= through.max(1);
        } else {
            c.hp -= dmg;
        }
        let (after_hp, after_wall) = (c.hp, c.wall_hp);
        self.siege.blows += 1;
        self.siege.damage += i64::from(before_hp - after_hp);
        if wall > 0 && after_wall == 0 {
            self.siege.walls_breached += 1;
        }
        if before_hp > 0 && after_hp <= 0 {
            self.siege.cities_reduced += 1;
            let city_pos = self.cities[&cid].pos;
            let melee_on_the_doorstep = self.units.values().any(|unit| {
                unit.owner == attacker
                    && self.wdist(unit.pos, city_pos) <= 1
                    && self.rules.units[unit.kind].is_melee_capable()
            });
            if melee_on_the_doorstep {
                self.siege.reduced_with_melee_adjacent += 1;
            }
        }
    }

    /// Throw a slug at a tile instead of landing it as cargo.
    ///
    /// A mass driver on the Moon needs no launcher on the ground and no range
    /// to speak of — a rock leaving lunar orbit reaches anywhere on the world
    /// this civilization can see. What bounds it is the number of drivers and
    /// the ore: one shot per driver per turn, one unit of the aimed ore out of
    /// the stockpile per shot. It leaves no fallout and it is not a device, so
    /// unlike a detonation it earns a war's ordinary grievances rather than
    /// the world's lasting horror.
    pub(super) fn do_mass_driver_strike(&mut self, pid: usize, target: Pos) -> Result<(), String> {
        let drivers = self.mass_drivers(pid);
        if drivers <= 0 {
            return Err("no mass driver on the Moon".into());
        }
        if self.players[pid].mass_driver_shots >= drivers {
            return Err("every mass driver has already fired this turn".into());
        }
        let ore = self.players[pid]
            .mass_driver_ore
            .ok_or_else(|| "the mass drivers are not aimed at an ore".to_string())?;
        if self.strategic_stockpile(pid, ore) < 1.0 {
            return Err("no slug in the stockpile to throw".into());
        }
        if !self.map.tiles.contains_key(&target) {
            return Err("no such tile".into());
        }
        if !self.players[pid].explored.contains(&target) {
            return Err("target tile is unrevealed".into());
        }
        // Same rule a detonation is held to: whatever is standing under it
        // decides whether the order is legal, and dropping a rock on somebody
        // you are at peace with is not.
        let mut aggrieved = BTreeSet::new();
        let owners = self
            .unit_ids_at(target)
            .iter()
            .map(|uid| self.units[uid].owner)
            .chain(self.city_at(target).map(|city| self.cities[&city].owner))
            .chain(
                self.defending_district_at(target)
                    .map(|city| self.cities[&city].owner),
            )
            .chain(self.tile_owner(target));
        for owner in owners {
            if owner == pid || self.players[owner].is_barbarian {
                continue;
            }
            if !self.is_at_war(pid, owner) {
                return Err("cannot strike a civilization you are at peace with".into());
            }
            aggrieved.insert(owner);
        }
        let struck_city = self
            .city_at(target)
            .filter(|city| self.cities[city].owner != pid);
        let exposed: Vec<u32> = self.units_at(target);
        for defender in exposed
            .iter()
            .filter_map(|uid| self.units.get(uid))
            .filter(|unit| unit.owner != pid)
            .cloned()
            .collect::<Vec<_>>()
        {
            self.record_war_unit_participation(&defender, pid);
        }
        let spent = self.strategic_stockpile(pid, ore) - 1.0;
        self.players[pid].strategic_resources.insert(ore, spent);
        self.players[pid].mass_driver_shots += 1;
        bump(&mut self.players[pid], "mass_driver_strikes");
        // One tile, no blast radius: a slug is aimed, not detonated. It hits
        // as hard as a nuclear ground zero because it arrives with the whole
        // of a gravity well behind it, and it does it to exactly one hexagon.
        self.damage_tile_area(target, 100, Some(pid));
        if let Some(city) = struck_city {
            let city = self.cities.get_mut(&city).unwrap();
            city.wall_hp = 0;
            city.pop = (city.pop - 1).max(1);
        }
        let name = struck_city.map(|city| self.cities[&city].name.clone());
        for owner in aggrieved.iter().copied() {
            self.add_grievances(owner, pid, 50.0);
            self.record_war_moment(pid, owner, "mass_driver_strike", name.clone(), Some(target));
        }
        let destroyed = exposed
            .into_iter()
            .filter(|uid| !self.units.contains_key(uid))
            .count();
        let toll = match destroyed {
            0 => String::new(),
            1 => " · 1 unit lost".to_string(),
            count => format!(" · {count} units lost"),
        };
        let place = name
            .clone()
            .or_else(|| {
                aggrieved
                    .iter()
                    .next()
                    .map(|owner| format!("{} territory", self.civ_name(*owner)))
            })
            .unwrap_or_else(|| "open ground".to_string());
        self.note_important(
            pid,
            "War",
            format!("dropped a mass driver slug on {place}{toll}"),
            Some(target),
        );
        for owner in aggrieved {
            self.note_important(
                owner,
                "War",
                format!("{} dropped a slug from the Moon", self.civ_name(pid)),
                Some(target),
            );
        }
        // Throwing rocks at people from orbit is not free at home either,
        // though it is nothing like the price of a detonation.
        self.add_war_weariness(pid, 2.0);
        Ok(())
    }

    pub fn unit_attack_range(&self, uid: u32) -> i32 {
        let unit = &self.units[&uid];
        let spec = &self.rules.units[unit.kind];
        spec.range.max(1)
            + self.promotion_effect(unit, "range") as i32
            // CLASS_FORWARD_OBSERVER belongs to ground siege. `siege` also
            // marks Bombers for city-targeting, but they receive no Balloon
            // or Drone range bonus (Base Units.xml:611-623).
            + if spec.promotion_class == "siege" {
                self.adjacent_support_effect(unit, "adjacent_siege_range") as i32
            } else {
                0
            }
    }

    pub(crate) fn air_operation_origin(&self, uid: u32) -> Pos {
        let unit = &self.units[&uid];
        unit.air_patrol_pos.unwrap_or(unit.pos)
    }

    /// Every tile this unit could attack on a fresh turn.
    ///
    /// This is the combat counterpart to [`Game::threat_reach`]. It follows
    /// the same terrain, border, cliff, movement-cost, and zone-of-control
    /// rules, then reserves the Movement an actual melee attack needs or the
    /// remaining point a ranged attack needs. Occupying units are deliberately
    /// read through: callers use this as an enemy threat envelope, where a
    /// blocker may move away before the threatened unit's next turn.
    ///
    /// The result is a potential envelope rather than a permission: it does
    /// not require a current target, visibility, or a declaration of war.
    /// Those facts belong to the controller deciding which visible enemies
    /// matter. The returned order is stable for explainers and tests.
    pub fn attack_reach(&self, uid: u32) -> Vec<Pos> {
        self.attack_reach_from_flood(uid).0
    }

    /// The cached form of [`Self::attack_reach_from_flood`] for an AI envelope
    /// query. `key` is the caller's full board fingerprint; on a mismatch the
    /// old snapshot is discarded before this unit's reach can be read.
    ///
    /// The answer deliberately remains viewer-independent. Sharing an
    /// already-filtered hostile table would be wrong because different seats
    /// see and fight different units; sharing this raw flood is exact because
    /// `attack_reach_from_flood` receives only `self` and `uid`.
    pub(crate) fn cached_attack_reach_from_flood(
        &self,
        key: (u32, u64),
        uid: u32,
    ) -> Arc<AttackReachFromFlood> {
        let mut cache = self
            .attack_reach_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if cache.key != Some(key) {
            cache.key = Some(key);
            cache.reaches.clear();
        }
        if let Some(reach) = cache.reaches.get(&uid) {
            return Arc::clone(reach);
        }
        let (targets, flood) = self.attack_reach_from_flood(uid);
        let reach = Arc::new(AttackReachFromFlood {
            targets: Arc::from(targets),
            flood,
        });
        #[cfg(test)]
        {
            cache.computations += 1;
        }
        cache.reaches.insert(uid, Arc::clone(&reach));
        reach
    }

    #[cfg(test)]
    pub(crate) fn attack_reach_cache_computations(&self) -> u64 {
        self.attack_reach_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .computations
    }

    /// `attack_reach`, and the tiles the unit walked to get there.
    pub(crate) fn attack_reach_from_flood(&self, uid: u32) -> (Vec<Pos>, Vec<Pos>) {
        let Some(unit) = self.units.get(&uid) else {
            return (vec![], vec![]);
        };
        let spec = &self.rules.units[unit.kind];
        if spec.class != "military" || (!spec.is_melee_capable() && !spec.has_ranged_attack()) {
            return (vec![], vec![]);
        }

        let attack_range = self.unit_attack_range(uid);
        if spec.domain.as_deref() == Some("air") {
            let origin = self.air_operation_origin(uid);
            let mut targets = Vec::new();
            self.map.for_each_disk(origin, attack_range, |target| {
                if target != origin {
                    targets.push(target);
                }
            });
            targets.sort_unstable();
            targets.dedup();
            return (targets, vec![]);
        }

        let start = unit.pos;
        let max_moves = self.unit_max_moves(uid);
        // `targets` is sorted below and the sensitivity cache turns `flood`
        // into a set, so neither consumer observes the ordered-map contract of
        // `flow_past`. Keep this one flood in its already-dense discovery order
        // rather than allocating an intermediate `BTreeMap<Pos, f64>` merely to
        // immediately walk it again.
        let positions = self.flow_past_unordered(uid, start, max_moves, true);
        let flood: Vec<Pos> = positions.iter().map(|(position, _)| *position).collect();
        // Every accepted target is still evaluated below in the original
        // order. The map's dense position order can collect their union
        // without storing and sorting the same tile from every stride.
        let mut targets = AttackTargetUnion::new(&self.map.tiles);
        for (from, remaining) in positions {
            if remaining <= 0.0 {
                continue;
            }

            if spec.has_ranged_attack()
                && !self.unit_is_embarked_at(unit, from)
                && (!spec.siege || from == start || self.siege_may_attack_after_moving(unit))
            {
                self.map.for_each_disk(from, attack_range, |target| {
                    if target != from && self.unit_has_line_of_sight_from(uid, from, target) {
                        targets.push(target);
                    }
                });
            }

            if spec.is_melee_capable() {
                for target in self.nbrs(from) {
                    if !self.unit_can_melee_target_domain(uid, target)
                        || !self.unit_can_cross_cliff(uid, from, target)
                    {
                        continue;
                    }
                    let fresh = from == start && remaining >= max_moves;
                    if fresh || remaining >= self.unit_step_cost(uid, from, target) {
                        targets.push(target);
                    }
                }
            }
        }
        (targets.into_sorted(), flood)
    }

    /// The movement flood in traversal-discovery order. This is private to the
    /// attack-envelope path, whose two consumers either sort later or use set
    /// membership and therefore do not observe `flow_past`'s ordered-map
    /// contract.
    pub(super) fn flow_past_unordered(
        &self,
        uid: u32,
        start: Pos,
        moves: f64,
        through_units: bool,
    ) -> Vec<(Pos, f64)> {
        let mut positions = Vec::new();
        self.flow_past_into(uid, start, moves, through_units, &mut positions);
        positions
    }

    /// Combat is a stock Emergency contribution even when the attacker never
    /// finishes a turn inside the target's territory. One point records the
    /// attack and a second records a kill; rewards only require a positive
    /// contribution, while the score remains useful for observation/replays.
    pub(super) fn record_emergency_combat(&mut self, member: usize, target: usize, killed: bool) {
        for emergency in &mut self.active_emergencies {
            if emergency.ends > self.turn
                && emergency.target == target
                && emergency.members.contains(&member)
            {
                *emergency.contributions.entry(member).or_insert(0) += 1 + i64::from(killed);
            }
        }
    }

    /// Percentage added to this seat's experience awards.
    pub fn handicap_xp_pct(&self, pid: usize) -> f64 {
        if !self.takes_handicap(pid) {
            return 0.0;
        }
        let spec = self.difficulty_spec();
        if self.is_human_seat(pid) {
            spec.human_xp_pct
        } else {
            spec.ai_xp_pct
        }
    }

    pub fn rock_band_choose_any_promotion(&self, pid: usize) -> bool {
        self.policy_effect(pid, "rock_band_any_promotion") > 0.0
    }

    /// Apply unit-specific experience modifiers, then round to the nearest
    /// integer (with .5 rounding upward) as Civ VI does.
    pub(super) fn modified_xp(&self, uid: u32, amt: f64) -> i64 {
        let (owner, kind, trained_bonus) = match self.units.get(&uid) {
            Some(u) => (u.owner, u.kind, u.xp_bonus_pct),
            None => return 0,
        };
        let spec = &self.rules.units[kind];
        let mut multiplier = 1.0 + self.handicap_xp_pct(owner) / 100.0;
        if self.players[owner].government.as_deref() == Some("oligarchy") {
            multiplier += 0.2;
        }
        if spec.promotion_class == "recon" {
            multiplier += self.policy_effect(owner, "recon_xp_pct") / 100.0;
        }
        multiplier += self.policy_effect(owner, "unit_xp_pct") / 100.0;
        // To Arms! pays its Golden Age half in two parts: POLICY_TO_ARMS_GA_XP
        // is +50% experience beside POLICY_TO_ARMS_GA_PRODUCTION's +25%
        // Production. Only the Production half was modelled, and at 15%.
        if self.dedication_active(owner, "to_arms") {
            multiplier += 0.5;
        }
        multiplier += trained_bonus / 100.0;
        if spec.promotion_class == "ranged" && self.has_ability(owner, "ta_seti") {
            multiplier += 0.5;
        }
        multiplier += self.civ_effect(owner, "unit_xp_pct") / 100.0;
        (amt * multiplier).round() as i64
    }

    pub(super) fn promotion_threshold(level: i32) -> i64 {
        (15 * level as i64 * (level as i64 + 1)) / 2
    }

    pub fn promotion_pending(&self, uid: u32) -> bool {
        self.units.get(&uid).is_some_and(|unit| {
            let spec = &self.rules.units[unit.kind];
            let class = &spec.promotion_class;
            if matches!(class.as_str(), "rock_band" | "religious_apostle") {
                if let Some(menu) = self.host_band_promotions.get(&uid) {
                    if menu.held == unit.promotions {
                        return menu
                            .offered
                            .iter()
                            .any(|name| !unit.promotions.contains(name));
                    }
                }
            }
            let level_cap = if class == "rock_band" { 4 } else { 8 };
            spec.earns_xp
                && !class.is_empty()
                && (unit.promotions.is_empty() || unit.level < level_cap)
                && unit.xp >= Self::promotion_threshold(unit.level)
        })
    }

    fn promotion_offer_key(uid: u32, level: i32, name: &str) -> u64 {
        // Stable FNV-1a ranking gives every promotion event a deterministic
        // three-card offer without consuming the simulation RNG merely by
        // opening the action list.
        let mut hash = 0xcbf29ce484222325_u64 ^ uid as u64 ^ ((level as u64) << 32);
        for byte in name.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    pub(crate) fn promotion_effect(&self, unit: &Unit, effect: &str) -> f64 {
        unit.promotions
            .iter()
            .filter_map(|name| {
                self.rules
                    .promotions
                    .get(name)?
                    .effects
                    .get(effect)
                    .copied()
            })
            .sum()
    }

    /// Numeric abilities inherent to the unit type plus earned promotions.
    pub(super) fn unit_effect(&self, unit: &Unit, effect: &str) -> f64 {
        self.rules.units[unit.kind]
            .effects
            .get(effect)
            .copied()
            .unwrap_or(0.0)
            + self.promotion_effect(unit, effect)
    }

    pub fn available_promotions(&self, uid: u32) -> Vec<Name> {
        let Some(unit) = self.units.get(&uid) else {
            return vec![];
        };
        if unit.moves_left <= 0.0 || !self.promotion_pending(uid) {
            return vec![];
        }
        let class = self.rules.units[unit.kind].promotion_class.as_str();
        let mut available: Vec<Name> = self
            .rules
            .promotions
            .iter()
            .filter(|(name, spec)| {
                spec.class == class
                    && !unit.promotions.iter().any(|held| *held == **name)
                    && (spec.requires.is_empty()
                        || spec
                            .requires
                            .iter()
                            .any(|req| unit.promotions.iter().any(|held| *held == *req)))
            })
            .map(|(name, _)| *name)
            .collect();
        // Native bands and Apostles draw their own three choices. A current offer
        // is stronger evidence than simulated randomness or earlier rejected choices.
        if matches!(class, "rock_band" | "religious_apostle") {
            if let Some(menu) = self.host_band_promotions.get(&uid) {
                if menu.held == unit.promotions {
                    available.retain(|name| menu.offered.contains(name));
                    return available;
                }
            }
        }
        // ★★★★★ DROP PROMOTIONS THE HOST HAS ALREADY REFUSED FOR THIS UNIT.
        //
        // Filtered here rather than at the three appliers because every chooser and
        // `do_promote` itself reach the offer through this one function, so one gate
        // covers them all. Native games leave `blocked_promotions` empty and take the
        // fast path below.
        if let Some(blocked) = self.blocked_promotions.get(&uid) {
            // ⚠ TWO DISTINCT REFUSALS MEAN THE UNIT CANNOT PROMOTE AT ALL, not that
            // it should try the next name on the list.
            //
            // Blocking the pair alone is right for a unit the host refused ONE
            // promotion for, and wrong for a Civilization VI Apostle, which takes a
            // single promotion when it is created and can never take another.
            // CIVVIS keeps `promotion_pending` true for it forever, so a pair block
            // just walks the offer down the list: in the gate replay TRANSLATOR fell
            // 67 -> 1 and CHAPLAIN 59 -> 31 while PROSELYTIZER rose 0 -> 83.
            //
            // The threshold is read off the corpus rather than assumed. Of 12 units
            // the host ever refused, the 5 that were asked two or more DISTINCT
            // promotions are all Apostles walking the list; the 7 asked exactly one
            // are 4 Apostles that never got a second name and 3 Pike and Shot
            // refused `PROMOTION_ECHELON`, which the pair block fixes outright.
            if blocked.len() >= 2 {
                return vec![];
            }
            available.retain(|name| !blocked.contains(name));
        }
        let limited_offer = (class == "rock_band"
            && !self.rock_band_choose_any_promotion(unit.owner))
            || (class == "religious_apostle"
                && !self.grants_city_state_unique_bonus(unit.owner, "Yerevan"));
        if limited_offer {
            available
                .sort_by_key(|name| (Self::promotion_offer_key(unit.id, unit.level, name), *name));
            available.truncate(3);
        }
        available
    }

    /// Whether this civilization has reached the civic for a particular
    /// formation tier. Spain and Shaka replace the ordinary civic gates for
    /// naval and land formations respectively.
    pub(super) fn formation_unlocked(&self, pid: usize, unit: &str, formation: u8) -> bool {
        let Some(spec) = self.rules.units.get(unit) else {
            return false;
        };
        let standard = match formation {
            1 => self.tree_effect(pid, "corps_fleets") > 0.0,
            2 => self.tree_effect(pid, "armies_armadas") > 0.0,
            _ => false,
        };
        if standard {
            return true;
        }
        if spec.domain.as_deref() == Some("sea")
            && self.civ_effect(pid, "naval_formation_civic_mercantilism") > 0.0
        {
            return self.players[pid]
                .civics
                .contains(&crate::name!("mercantilism"));
        }
        if !matches!(spec.domain.as_deref(), Some("sea" | "air")) {
            return match formation {
                1 if self.civ_effect(pid, "land_corps_civic_mercenaries") > 0.0 => self.players
                    [pid]
                    .civics
                    .contains(&crate::name!("mercenaries")),
                2 if self.civ_effect(pid, "land_army_civic_nationalism") > 0.0 => self.players[pid]
                    .civics
                    .contains(&crate::name!("nationalism")),
                _ => false,
            };
        }
        false
    }

    pub(super) fn unit_formation_bonus(&self, unit: &Unit) -> f64 {
        let base = match unit.formation {
            1 => 10.0,
            2.. => 17.0,
            _ => 0.0,
        };
        if base > 0.0
            && !matches!(
                self.rules.units[unit.kind].domain.as_deref(),
                Some("sea" | "air")
            )
        {
            base + self.civ_effect(unit.owner, "formation_combat_strength")
        } else {
            base
        }
    }

    pub(super) fn award_xp(&mut self, uid: u32, amt: f64) {
        if self
            .units
            .get(&uid)
            .is_none_or(|unit| !self.rules.units[unit.kind].earns_xp)
        {
            return;
        }
        if self.promotion_pending(uid) {
            return; // XP gain pauses until the pending promotion is selected.
        }
        let gained = self.modified_xp(uid, amt);
        if let Some(unit) = self.units.get_mut(&uid) {
            unit.xp += gained;
        }
    }

    /// Discipline: +5 combat strength when fighting barbarians.
    pub(super) fn vs_bonus(&self, owner: usize, opponent: usize) -> f64 {
        if self.players[opponent].is_barbarian {
            self.policy_effect(owner, "barbarian_combat")
        } else if self.players[owner]
            .alliances
            .iter()
            .any(|(ally, alliance)| {
                alliance.ends > self.turn
                    && alliance.kind == "military"
                    && alliance.level >= 1
                    && self.is_at_war(*ally, opponent)
            })
        {
            5.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod target_union_tests;
