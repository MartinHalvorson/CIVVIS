//! Barbarians: what they threaten and how the empire answers.
//!
//! Reading them: what a raider is and where it is headed
//! (`is_barbarian_raider`, `barbarian_home`, `barbarian_assigned_target`,
//! `barbarian_target_allowed`), and the pressure they put on a city
//! (`barbarian_threat_pressure`, `barbarian_local_alarm`,
//! `barbarian_trade_risk`, `barbarian_presence_at_home_with_camp_radius`).
//! Answering them: the production and research a threat asks for
//! (`barbarian_defense_gap`, `barbarian_defense_item`,
//! `opening_barbarian_defense_item`, `barbarian_military_research_goal`), and
//! clearing camps (`camp_party`, `camp_bounty_target`,
//! `camp_is_a_neighbours_problem`, `clear_adjacent_empty_barbarian_camp`).
//! And how a barbarian unit itself acts (`barbarian_raid_action`).
//!
//! Moved verbatim out of the `impl BasicAi` blocks in `ai.rs` (2026-11-06).
//! Methods the rest of `ai` calls are `pub(super)`, the same reach a private
//! method of `ai` had; the code is otherwise unchanged.

use super::*;

impl BasicAi {
    pub(super) fn opening_barbarian_defense_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<Item> {
        let city = g.cities.get(&cid).filter(|city| city.owner == pid)?;
        if self.minor
            || self.barb
            || !city.is_capital
            || (self.book_pos >= 4 && !self.book_settler_pending)
        {
            return None;
        }
        let already_answering = city.queue.first().is_some_and(|current| match current {
            Item::Unit { unit } | Item::Formation { unit, .. } => g
                .rules
                .units
                .get(unit)
                .is_some_and(|spec| spec.class == "military" && spec.promotion_class != "recon"),
            _ => false,
        });
        if already_answering {
            return None;
        }
        self.barbarian_defense_item(g, pid, cid)
            .filter(|item| matches!(item, Item::Unit { .. }))
    }

    /// Land defenders that can answer a barbarian raid from this city's tile.
    /// Recon, air, and naval units do not count: they may be valuable elsewhere
    /// but cannot hold the local approaches or clear a camp on foot.
    pub(super) fn barbarian_local_defenders(
        g: &Game,
        pid: usize,
        city: &crate::game::City,
    ) -> usize {
        g.units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && g.rules.units[unit.kind].class == "military"
                    && matches!(
                        g.rules.units[unit.kind].domain.as_deref(),
                        None | Some("land")
                    )
                    && g.wdist(unit.pos, city.pos) <= BARBARIAN_LOCAL_DEFENDER_RADIUS
            })
            .count()
    }

    /// Whether the barbarian ring pressing this city is made mostly of units
    /// that shoot. Strictly more shooters than melee, so an even ring keeps the
    /// historical melee answer and only a genuinely ranged siege changes it.
    /// See `barbarian_ranged_answer`.
    fn barbarian_ring_is_mostly_ranged(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        let mut ranged = 0usize;
        let mut melee = 0usize;
        for unit in g.units.values().filter(|unit| {
            Self::is_barbarian_raider(g, unit)
                && g.wdist(unit.pos, city.pos) <= HOME_THREAT_RADIUS
                && self.barbarian_raider_counts_as_threat(g, pid, unit)
        }) {
            if g.rules.units[unit.kind].has_ranged_attack() {
                ranged += 1;
            } else {
                melee += 1;
            }
        }
        ranged > melee
    }

    /// The local body count a barbarian threat asks for. One defender covers a
    /// camp or lone raider; two hold a city against a small early raiding party
    /// while the field army closes the camp. This is deliberately bounded so a
    /// crowded map does not turn every camp into an unbounded production sink.
    pub(crate) fn barbarian_defense_gap(&self, g: &Game, pid: usize, cid: u32) -> usize {
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return 0;
        };
        if !self.barbarian_local_alarm_for_controller(g, pid, cid) {
            return 0;
        }
        let raiders = g
            .units
            .values()
            .filter(|unit| Self::is_barbarian_raider(g, unit))
            .filter(|unit| g.wdist(unit.pos, city.pos) <= HOME_THREAT_RADIUS)
            .filter(|unit| self.barbarian_raider_counts_as_threat(g, pid, unit))
            .count();
        // The live opening cannot call two bodies sufficient against an
        // entire raid. Athens' t18--20 horse raid still saw a zero gap with
        // two local defenders, so its Settler queue remained the answer.
        // Match the observed party up to a bounded four-body response while
        // the seat is in the Ancient/Classical window. The demand recedes
        // with the raiders; later eras and unarmed controllers keep the old
        // floor. Existing threat/visibility and naval-triage filters apply.
        let wanted = if self.garrison_under_fire
            && advanced::AdvancedAi::early_archers_window_open(g, pid)
        {
            raiders.clamp(1, 4)
        } else {
            raiders.clamp(1, 2)
        };
        wanted.saturating_sub(self.barbarian_local_defenders_for_controller(g, pid, cid))
    }

    /// The local defenders `barbarian_defense_gap` credits. Under
    /// `siege-preempts-the-queue` or the live garrison policy, a recon unit
    /// is not one of them: a Scout
    /// is class `military` and was counted, so on live run
    /// civvis-20260901T193130Z t36 a Scout bought two turns earlier made the
    /// gap read zero with a barbarian Slinger adjacent to the capital, and
    /// the city started a Government Plaza.
    pub(crate) fn barbarian_local_defenders_for_controller(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> usize {
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return 0;
        };
        let defenders = Self::barbarian_local_defenders(g, pid, city);
        if !self.siege_preempts_the_queue && !self.garrison_under_fire {
            return defenders;
        }
        let recon = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && spec.class == "military"
                    && spec.promotion_class == "recon"
                    && matches!(spec.domain.as_deref(), None | Some("land"))
                    && g.wdist(unit.pos, city.pos) <= BARBARIAN_LOCAL_DEFENDER_RADIUS
            })
            .count();
        defenders.saturating_sub(recon)
    }

    /// Production answer for an active barbarian ring. The city first gets a
    /// melee-capable land defender, then walls once a compact local garrison
    /// exists. This branch is independent of the optional major-war defense
    /// experiment because barbarian pressure is a routine early-game hazard.
    pub(crate) fn barbarian_defense_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if self.minor
            || self.barb
            || !self.barbarian_tactics
            || !self.barbarian_local_alarm_for_controller(g, pid, cid)
        {
            return None;
        }
        // A local body count does not replace the first city-defense shooter.
        if let Some(shooter) = self.early_local_shooter_item(g, pid, cid) {
            return Some(shooter);
        }
        if self.barbarian_defense_gap(g, pid, cid) > 0 {
            // A ring of shooters wants a shooter back. See
            // `barbarian_ranged_answer`.
            let want = if self.barbarian_ranged_answer
                && self.barbarian_ring_is_mostly_ranged(g, pid, cid)
            {
                Some(true)
            } else {
                Some(false)
            };
            let unit = self
                .best_military(g, pid, cid, want)
                .or_else(|| self.best_military(g, pid, cid, Some(false)))
                .or_else(|| self.best_military(g, pid, cid, None))?;
            return Some(Item::Unit {
                unit: Name::new(&unit),
            });
        }
        for building in ["walls", "medieval_walls", "renaissance_walls"] {
            let wall = Item::Building {
                building: Name::new(building),
            };
            if g.can_produce(pid, cid, &wall) {
                return Some(wall);
            }
        }
        None
    }

    /// ★★★★★ THE HOMELAND HAD NO CLAIM ON THE ARMY AT ALL.
    ///
    /// Measured on live run `civvis-20260803T005930Z` (Kongo, Small, 154 turns):
    /// **116 of 154 turns had a hostile standing inside or beside this empire's
    /// own territory.** A full-health Crossbowman sat four tiles from two cities,
    /// unmoved and unengaged, for **21 consecutive turns**. Earlier in the same
    /// game a barbarian Warrior occupied a city tile for 11 straight turns, and a
    /// Man-at-Arms roamed the interior for thirty — *healing* from 83 to 92 while
    /// it did, because nothing ever touched it.
    ///
    /// ⚠ THE CAUSE IS NOT VISIBILITY AND NOT THE BARBARIAN SEAT. `is_at_war` is
    /// true for `barb_pid`, the seat is alive, and every one of those raiders was
    /// in `enemy_ids` the whole time. The cause is that the only target selector
    /// this AI had, `nearest_enemy`, ranks candidates by **distance from the
    /// asking unit**. For an army deployed on a war front an enemy city is always
    /// nearer than a raider back home, so every unit converged on the offensive
    /// and the empire's own ground was nobody's job. The fallback, `patrol_step`,
    /// walks a frontier ring by `uid % posts.len()` and is threat-blind.
    ///
    /// So this is an ABSENT assignment rather than a broken one: nothing anywhere
    /// Whether the barbarian seat has a military presence — a raider or a
    /// camp — within [`HOME_THREAT_RADIUS`] of one of this empire's cities.
    ///
    /// This is the admission test the Advanced military step uses before it
    /// lets the barbarian seat into a unit's enemy list: a raider pillaging
    /// inside the empire is a war at home whatever the diplomatic table says,
    /// while a camp on the far side of the map is not a reason to mobilise.
    /// The chase itself stays bounded by [`BasicAi::nearest_enemy`]'s own
    /// near-home and exchange-score gates.
    /// `barbarian_presence_at_home` with the camp radius chosen by the caller:
    /// raiders always count within `HOME_THREAT_RADIUS`, camps within
    /// `camp_radius`. See `camp_reach`.
    #[cfg(test)]
    pub(crate) fn barbarian_presence_at_home_with_camp_radius(
        g: &Game,
        pid: usize,
        camp_radius: i32,
    ) -> bool {
        Self::barbarian_presence_at_home_inner(g, pid, camp_radius, false, false)
    }

    /// `leave_neighbours_camps` is `enemy_of_my_enemy`'s reading: a camp
    /// that is a neighbour's problem (`camp_is_a_neighbours_problem_inner`)
    /// raises no alarm at home; a raider inside the ring still does.
    pub(super) fn barbarian_presence_at_home_inner(
        g: &Game,
        pid: usize,
        camp_radius: i32,
        naval_threat_triage: bool,
        leave_neighbours_camps: bool,
    ) -> bool {
        let Some(_barb) = g.barb_pid else {
            return false;
        };
        let my_cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| city.pos)
            .collect();
        if my_cities.is_empty() {
            return false;
        }
        let near_home =
            |pos: Pos, radius: i32| my_cities.iter().any(|city| g.wdist(pos, *city) <= radius);
        if g.barb_camps.keys().any(|camp| {
            near_home(*camp, camp_radius)
                && !(leave_neighbours_camps
                    && Self::camp_is_a_neighbours_problem_inner(g, pid, *camp))
        }) {
            return true;
        }
        g.units.values().any(|unit| {
            Self::is_barbarian_raider(g, unit)
                && near_home(unit.pos, HOME_THREAT_RADIUS)
                && (!naval_threat_triage
                    || Self::naval_raider_can_deal_serious_damage(g, pid, unit))
        })
    }

    /// Whether a visible barbarian combat unit near one of our cities is
    /// materially stronger than our best land unit. This is intentionally a
    /// quality alarm, not a headcount alarm: the ordinary barbarian defense
    /// branch already handles how many bodies a city needs, while this one
    /// decides whether research must retire obsolete bodies.
    fn barbarian_strength_gap(g: &Game, pid: usize) -> Option<(f64, f64)> {
        let barb_pid = g.barb_pid?;
        let player = g.players.get(pid)?;
        if player.is_minor || player.is_barbarian || barb_pid == pid {
            return None;
        }
        let cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| city.pos)
            .collect();
        if cities.is_empty() {
            return None;
        }
        let near_home = |position: Pos| {
            cities
                .iter()
                .any(|city| g.wdist(position, *city) <= HOME_THREAT_RADIUS)
        };
        let combat_power = |kind: Name| {
            let spec = &g.rules.units[kind];
            spec.strength.max(spec.ranged_attack_strength())
        };
        let strongest_barbarian = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == barb_pid
                    && Self::is_barbarian_raider(g, unit)
                    && near_home(unit.pos)
                    && g.player_can_see(pid, unit.pos)
            })
            .map(|unit| combat_power(unit.kind))
            .fold(0.0, f64::max);
        if strongest_barbarian <= 0.0 {
            return None;
        }
        let strongest_own = g
            .units
            .values()
            .filter(|unit| {
                if unit.owner != pid {
                    return false;
                }
                let spec = &g.rules.units[unit.kind];
                spec.class == "military" && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            })
            .map(|unit| combat_power(unit.kind))
            .fold(0.0, f64::max);
        (strongest_barbarian
            > strongest_own * BARBARIAN_MILITARY_GAP_RATIO + BARBARIAN_MILITARY_GAP_MARGIN)
            .then_some((strongest_barbarian, strongest_own))
    }

    /// The technology whose absence strands the largest share of the current
    /// land army behind a nearby barbarian's quality. The returned goal may be
    /// several prerequisites away; `research_step_toward` turns it into the
    /// cheapest legal next step. Resource-gated successors with no stockpile
    /// are skipped so the AI does not beeline toward a unit it cannot field.
    pub(crate) fn barbarian_military_research_goal(g: &Game, pid: usize) -> Option<String> {
        Self::barbarian_strength_gap(g, pid)?;
        let player = &g.players[pid];
        let mut goals: BTreeMap<Name, (f64, f64, usize)> = BTreeMap::new();
        for unit in g.units.values().filter(|unit| {
            if unit.owner != pid {
                return false;
            }
            let spec = &g.rules.units[unit.kind];
            spec.class == "military" && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        }) {
            let mut current = unit.kind;
            let mut visited = BTreeSet::new();
            loop {
                if !visited.insert(current) {
                    break;
                }
                let Some(held) = g.rules.units.get(&current) else {
                    break;
                };
                let Some(generic_target) = held.upgrade_to else {
                    break;
                };
                let target_kind = g.player_unit_replacement(pid, generic_target);
                let Some(target) = g.rules.units.get(&target_kind) else {
                    break;
                };
                if target.class != "military"
                    || matches!(target.domain.as_deref(), Some("sea" | "air"))
                    || !target.buildable
                {
                    break;
                }
                if target.requires_resource.is_some_and(|resource| {
                    target.resource_cost > 0.0
                        && g.strategic_stockpile(pid, resource) + f64::EPSILON
                            < target.resource_cost
                }) {
                    break;
                }
                let tech_known = target.tech.is_none_or(|tech| player.techs.contains(&tech));
                let civic_known = target
                    .civic
                    .is_none_or(|civic| player.civics.contains(&civic));
                if !tech_known {
                    let Some(tech) = target.tech else {
                        break;
                    };
                    let held_power = held.strength.max(held.ranged_attack_strength());
                    let target_power = target.strength.max(target.ranged_attack_strength());
                    let gain = (target_power - held_power).max(0.0);
                    if gain > 0.0 {
                        let entry = goals.entry(tech).or_insert((0.0, 0.0, 0));
                        entry.0 += gain;
                        entry.1 = entry.1.max(target_power);
                        entry.2 += 1;
                    }
                    break;
                }
                if !civic_known {
                    break;
                }
                current = target_kind;
            }
        }
        goals
            .into_iter()
            .max_by(|(left_name, left), (right_name, right)| {
                left.0
                    .total_cmp(&right.0)
                    .then(left.1.total_cmp(&right.1))
                    .then(left.2.cmp(&right.2))
                    .then(left_name.cmp(right_name))
            })
            .map(|(tech, _)| tech.to_string())
    }

    pub(crate) fn barbarian_military_gap(g: &Game, pid: usize) -> bool {
        Self::barbarian_strength_gap(g, pid).is_some()
    }

    /// The camp assignment is the only durable home a barbarian raider has.
    /// Guards and the engine-managed recon units are included here for
    /// completeness, although their movement paths stop before this helper is
    /// used by the raider policy.
    pub(super) fn barbarian_home(g: &Game, uid: u32) -> Option<Pos> {
        g.barbarian_unit_home(uid).or_else(|| {
            let position = g.units.get(&uid)?.pos;
            g.barb_camps
                .keys()
                .min_by_key(|camp| (g.wdist(position, **camp), **camp))
                .copied()
        })
    }

    pub(super) fn is_barbarian_scout(g: &Game, uid: u32) -> bool {
        g.barb_scout_homes.contains_key(&uid)
    }

    pub(crate) fn is_barbarian_raider(g: &Game, unit: &crate::game::Unit) -> bool {
        Some(unit.owner) == g.barb_pid
            && g.rules.units[unit.kind].class == "military"
            && !g.barb_camp_guards.values().any(|guard| *guard == unit.id)
            && !Self::is_barbarian_scout(g, unit.id)
    }

    /// A camp's reported city is a stronger objective than an arbitrary
    /// nearest city. The fallback is deliberately absent: before a Scout has
    /// reported, a raider holds near its camp instead of gaining clairvoyant
    /// knowledge of the map.
    pub(super) fn barbarian_assigned_target(g: &Game, uid: u32) -> Option<Pos> {
        let home = Self::barbarian_home(g, uid)?;
        g.barb_camp_targets.get(&home).copied()
    }

    pub(super) fn barbarian_target_allowed(g: &Game, uid: u32, target: Pos) -> bool {
        Self::barbarian_home(g, uid)
            .is_some_and(|home| g.wdist(home, target) <= BARBARIAN_RAID_RADIUS)
    }

    pub(super) fn barbarian_target_allowed_for_controller(
        &self,
        g: &Game,
        uid: u32,
        target: Pos,
    ) -> bool {
        !self.barb || !self.barbarian_tactics || Self::barbarian_target_allowed(g, uid, target)
    }

    /// Pressure is intentionally a small integer rather than a combat score:
    /// production needs to answer "one defender or two?", while tactical
    /// movement uses the full exchange evaluator below.
    pub(super) fn naval_raider_can_deal_serious_damage(
        g: &Game,
        pid: usize,
        raider: &crate::game::Unit,
    ) -> bool {
        let spec = &g.rules.units[raider.kind];
        if spec.domain.as_deref() != Some("sea") {
            return true;
        }
        let reach = g.attack_reach(raider.id);
        if reach.is_empty() {
            return false;
        }
        let reaches = |position: Pos| reach.binary_search(&position).is_ok();

        for city in g.cities.values().filter(|city| city.owner == pid) {
            if !reaches(city.pos) {
                continue;
            }
            let mut projected = 0.0_f64;
            if spec.is_melee_capable() {
                let attack = effective_strength(g.unit_strength(raider, false), raider.hp);
                projected = projected.max(expected_damage(attack, g.city_strength(city.id)));
            }
            if spec.has_ranged_attack() {
                let attack = effective_strength(g.unit_ranged_attack_strength(raider), raider.hp);
                let wall_share = if spec.siege { 1.0 } else { 0.5 };
                projected =
                    projected.max(expected_damage(attack, g.city_strength(city.id)) * wall_share);
            }
            if projected + f64::EPSILON >= SERIOUS_NAVAL_DAMAGE {
                return true;
            }
        }

        for defender in g
            .units
            .values()
            .filter(|unit| unit.owner == pid && reaches(unit.pos))
        {
            let defender_spec = &g.rules.units[defender.kind];
            if defender_spec.class != "military" {
                // A melee ship whose domain-valid envelope reaches an embarked
                // civilian can take it outright; that is serious at any HP.
                if spec.is_melee_capable() {
                    return true;
                }
                continue;
            }
            let mut projected = 0.0_f64;
            if spec.has_ranged_attack() {
                if let Some((attack, defence)) =
                    g.ranged_strike_strengths(raider.id, defender.id, defender.pos)
                {
                    projected = projected.max(expected_damage(attack, defence));
                }
            }
            if spec.is_melee_capable() {
                let attack = effective_strength(g.unit_strength(raider, false), raider.hp);
                let defence = effective_strength(g.unit_strength(defender, true), defender.hp);
                projected = projected.max(expected_damage(attack, defence));
            }
            if projected + f64::EPSILON >= SERIOUS_NAVAL_DAMAGE {
                return true;
            }
        }
        false
    }

    pub(super) fn barbarian_raider_counts_as_threat(
        &self,
        g: &Game,
        pid: usize,
        raider: &crate::game::Unit,
    ) -> bool {
        !self.naval_threat_triage || Self::naval_raider_can_deal_serious_damage(g, pid, raider)
    }

    fn barbarian_threat_pressure_inner(
        g: &Game,
        pid: usize,
        cid: u32,
        naval_threat_triage: bool,
    ) -> usize {
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return 0;
        };
        if g.barb_pid.is_none() {
            return 0;
        }
        let raiders = g
            .units
            .values()
            .filter(|unit| Self::is_barbarian_raider(g, unit))
            .filter(|unit| g.wdist(unit.pos, city.pos) <= HOME_THREAT_RADIUS)
            .filter(|unit| {
                !naval_threat_triage || Self::naval_raider_can_deal_serious_damage(g, pid, unit)
            })
            .count();
        let camps = g
            .barb_camps
            .keys()
            .filter(|camp| g.wdist(**camp, city.pos) <= HOME_CAMP_RADIUS)
            .count();
        // Keep the return value bounded. A map can have several camps in a
        // city's nine-tile ring, but the local response still needs only a
        // compact early defense rather than one unit per camp.
        raiders.min(3) + camps.min(1)
    }

    #[cfg(test)]
    pub(crate) fn barbarian_threat_pressure(g: &Game, pid: usize, cid: u32) -> usize {
        Self::barbarian_threat_pressure_inner(g, pid, cid, false)
    }

    pub(crate) fn barbarian_threat_pressure_for_controller(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> usize {
        Self::barbarian_threat_pressure_inner(g, pid, cid, self.naval_threat_triage)
    }

    /// Immediate production alarm: a live raider in the home ring or a camp
    /// close enough that a new raider can reach the city before a fresh unit
    /// finishes. A camp farther out still informs the bounded camp party and
    /// trader caution, but should not displace an unrelated culture or science
    /// build by itself.
    fn barbarian_local_alarm_inner(
        g: &Game,
        pid: usize,
        cid: u32,
        naval_threat_triage: bool,
    ) -> bool {
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        if g.barb_pid.is_none() {
            return false;
        }
        let camp_in_reach = g
            .barb_camps
            .keys()
            .any(|camp| g.wdist(*camp, city.pos) <= BARBARIAN_TRADE_RISK_RADIUS);
        if camp_in_reach {
            return true;
        }
        g.units.values().any(|unit| {
            Self::is_barbarian_raider(g, unit)
                && g.wdist(unit.pos, city.pos) <= HOME_THREAT_RADIUS
                && (!naval_threat_triage
                    || Self::naval_raider_can_deal_serious_damage(g, pid, unit))
        })
    }

    #[cfg(test)]
    pub(crate) fn barbarian_local_alarm(g: &Game, pid: usize, cid: u32) -> bool {
        Self::barbarian_local_alarm_inner(g, pid, cid, false)
    }

    pub(crate) fn barbarian_local_alarm_for_controller(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> bool {
        Self::barbarian_local_alarm_inner(g, pid, cid, self.naval_threat_triage)
    }

    /// A Trader is safe to start once the local barbarian ring is quiet. Native
    /// routes complete immediately from a city, while the live bridge may have
    /// to walk the unit to its origin, so the conservative gate protects both
    /// representations and keeps the early queue on military/repair work.
    #[cfg(test)]
    pub(crate) fn barbarian_trade_risk(g: &Game, pid: usize) -> bool {
        if g.barb_pid.is_none() || g.player_city_ids(pid).is_empty() {
            return false;
        }
        g.player_city_ids(pid)
            .into_iter()
            .any(|cid| Self::barbarian_local_alarm(g, pid, cid))
    }

    pub(crate) fn barbarian_trade_risk_for_controller(&self, g: &Game, pid: usize) -> bool {
        if g.barb_pid.is_none() || g.player_city_ids(pid).is_empty() {
            return false;
        }
        g.player_city_ids(pid)
            .into_iter()
            .any(|cid| self.barbarian_local_alarm_for_controller(g, pid, cid))
    }

    /// Barbarian units do not use the major-faction doctrine gate, but they do
    /// use the same engine legality checks. Attack opportunities are evaluated
    /// first; this action is the raid that follows when the tile is safe.
    pub(super) fn barbarian_raid_action(&self, g: &Game, pid: usize, uid: u32) -> Option<Action> {
        if !self.barb || !self.barbarian_tactics {
            return None;
        }
        g.legal_doctrine_actions(pid, uid)
            .into_iter()
            .find(|action| {
                matches!(
                    action,
                    Action::CoastalRaid { unit, .. } | Action::Pillage { unit } if *unit == uid
                )
            })
    }

    /// Whether the peacetime camp party is on. See `camp_party`.
    pub fn camp_party(&self) -> bool {
        self.camp_party
    }

    /// The camp errand's target for this unit: the nearest standing
    /// barbarian camp on home ground this unit can profitably take, claimed
    /// so no second unit runs the same errand. `None` stands the errand
    /// down: for minors and barbarians, for recon, when every nearby camp is
    /// outmatched, or when both claim slots are already spent. The caller
    /// owns the peacetime gate.
    pub(super) fn camp_bounty_target(&mut self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        if !self.camp_bounty || self.minor || self.barb {
            return None;
        }
        g.barb_pid?;
        // Peacetime only: a war economy has no spare errand-runners, and
        // wartime barbarian answering belongs to the bounded response path.
        if g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        }) {
            return None;
        }
        let (upos, ranged) = {
            let unit = &g.units[&uid];
            let spec = &g.rules.units[unit.kind];
            // A scout prices an EMPTY camp above its own threshold (the
            // undefended-tile branch of `exchange_score` pays 27.5 against
            // Recon's +14), so the class filter cannot be left to the
            // gate: a bounty is never a recon job.
            if spec.class != "military" || spec.promotion_class == "recon" {
                return None;
            }
            (unit.pos, spec.has_ranged_attack())
        };
        let my_cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|c| c.owner == pid)
            .map(|c| c.pos)
            .collect();
        if my_cities.is_empty() {
            return None;
        }
        let camp_radius = HOME_THREAT_RADIUS;
        let turn = g.turn;
        self.camp_bounty_claims
            .retain(|_, (claimed_turn, _)| *claimed_turn == turn);
        let claims_spent = self
            .camp_bounty_claims
            .values()
            .filter(|(_, claimant)| *claimant != uid)
            .count();
        // `quest_camp_errand`: among the camps this errand would already run,
        // the one a city-state named is the one that pays an Envoy. A
        // PREFERENCE, not a reach: the errand's own radius, war gate, claim
        // ledger and exchange threshold all still decide which camps are
        // eligible, and a named camp outside them is not chased. Off, `named`
        // is always false and the ordering below is unchanged. See
        // `advanced/city_state_quests.rs`.
        let mut best: Option<(bool, i32, Pos)> = None;
        for camp in g.barb_camps.keys() {
            let named = self.quest_camp_is_named(g, pid, *camp);
            match self.camp_bounty_claims.get(camp) {
                // Another unit already runs this camp's errand.
                Some((_, claimant)) if *claimant != uid => continue,
                Some(_) => {}
                // A fresh claim is available only while a slot is open.
                None if claims_spent >= CAMP_BOUNTY_PARTY => continue,
                None => {}
            }
            if my_cities.iter().map(|c| g.wdist(*camp, *c)).min().unwrap() > camp_radius {
                continue;
            }
            // `enemy_of_my_enemy`: the neighbour's camp is not our errand.
            if self.camp_is_a_neighbours_problem(g, pid, *camp) {
                continue;
            }
            let d = g.wdist(upos, *camp);
            if d > BARBARIAN_RESPONSE_RECALL_RANGE {
                continue;
            }
            if self.exchange_score(g, uid, *camp, ranged) <= self.attack_threshold(g, uid, *camp) {
                continue;
            }
            // `!named` first: a named camp sorts ahead of every unnamed one,
            // and among equals the nearest still wins.
            if best.map(|b| (!named, d, *camp) < b).unwrap_or(true) {
                best = Some((!named, d, *camp));
            }
        }
        let (_, _, camp) = best?;
        self.camp_bounty_claims.insert(camp, (turn, uid));
        Some(camp)
    }

    /// `enemy_of_my_enemy`: whether this camp is a neighbour's problem more
    /// than ours, so that every deliberate clear leaves it standing. A
    /// camp's Scout reports the nearest major settlement it sees and the
    /// camp raises its party against that report, so the camp whose nearest
    /// major city is a rival's raids the rival: the test is that a living
    /// major we are not allied with has a city strictly nearer the camp than
    /// any of ours, and that the camp is outside our own worked ring
    /// (`ENEMY_OF_MY_ENEMY_HOME_RING`), inside which its raiders pillage us
    /// whoever they were raised against. Always false with the gene off and
    /// for minors and barbarians. See `advanced/enemy_of_my_enemy.rs`.
    pub(crate) fn camp_is_a_neighbours_problem(&self, g: &Game, pid: usize, camp: Pos) -> bool {
        if !self.enemy_of_my_enemy || self.minor || self.barb {
            return false;
        }
        Self::camp_is_a_neighbours_problem_inner(g, pid, camp)
    }

    /// The geometry of `camp_is_a_neighbours_problem`, without the flag.
    pub(crate) fn camp_is_a_neighbours_problem_inner(g: &Game, pid: usize, camp: Pos) -> bool {
        let Some(ours) = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| g.wdist(city.pos, camp))
            .min()
        else {
            return false;
        };
        if ours <= crate::ai::advanced::enemy_of_my_enemy::ENEMY_OF_MY_ENEMY_HOME_RING {
            return false;
        }
        g.cities
            .values()
            .filter(|city| city.owner != pid)
            .filter(|city| {
                let owner = &g.players[city.owner];
                owner.alive
                    && !owner.is_minor
                    && !owner.is_barbarian
                    && !g.are_allied(pid, city.owner)
            })
            .map(|city| g.wdist(city.pos, camp))
            .min()
            .is_some_and(|theirs| theirs < ours)
    }

    /// Enter a visible, undefended barbarian camp that is one legal step
    /// away. Camps are captured by movement rather than an attack, so the
    /// ordinary tactical scan sees no enemy tile here and otherwise holds a
    /// melee unit beside a free clear. Keep this local: a camp that needs a
    /// march remains a separately priced errand, while an adjacent clear pays
    /// its Gold, Era Score, and spawn prevention immediately.
    pub(crate) fn clear_adjacent_empty_barbarian_camp(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> bool {
        // Gated so this controller treatment stays outside the frozen rating
        // anchor: `AdvancedAi::legacy()` ships it OFF, everything current
        // ships it ON, and `advanced_without_adjacent_camp_clear` withholds
        // it for pricing. Minors keep their own defense behaviour, as with
        // the errand.
        if !self.adjacent_camp_clear || self.minor {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        if unit.owner != pid
            || Some(pid) == g.barb_pid
            || g.rules.units[unit.kind].class != "military"
        {
            return false;
        }
        let clearable: Vec<Pos> = g
            .nbrs(unit.pos)
            .into_iter()
            .filter(|position| g.barb_camps.contains_key(position))
            // `Game` contains the whole board for simulation. Do not turn a
            // camp in fog into an AI objective merely because its coordinate
            // happens to be present in the model.
            .filter(|position| g.player_can_see(pid, *position))
            .filter(|position| g.unit_ids_at(*position).is_empty())
            .filter(|position| g.can_move(uid, *position))
            .collect();
        // `enemy_of_my_enemy`: a camp that raids the neighbour is left to it.
        let camp = clearable
            .iter()
            .copied()
            .filter(|position| !self.camp_is_a_neighbours_problem(g, pid, *position))
            .min();
        if camp.is_none() && !clearable.is_empty() {
            *g.players[pid]
                .counters
                .entry("eoe:camps_left".to_string())
                .or_insert(0) += 1;
            return false;
        }
        camp.is_some_and(|to| g.apply(pid, &Action::Move { unit: uid, to }).is_ok())
    }
}
