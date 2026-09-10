//! Movement pays for the enemy's next turn, not just adjacent enemies.
//! City-shot sharing is a planning allowance, never a reduction in the
//! conservative damage used to price losing an individual unit.
use super::{AttackEnvelopes, BasicAi, EnvelopeReach, COMBAT_ROLL_MAX};
use crate::game::{effective_strength, Game};
use crate::Pos;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub(super) struct MovementRiskFrame {
    envelopes: Arc<AttackEnvelopes>,
    target_city: Option<u32>,
    target: Pos,
    assault_positions: BTreeSet<Pos>,
    assault_units: BTreeSet<u32>,
    ready: usize,
}

// The existing per-enemy cache invalidates a reach when anything in its
// movement footprint (including adjacent ZOC sources) changes. Reuse that
// identity for the corrected strike reach too: moving a distant Builder must
// not flood every visible enemy's movement again. Arcs prevent address reuse.
#[derive(Default)]
struct MovementReachCache {
    table: Option<(Arc<AttackEnvelopes>, Arc<AttackEnvelopes>)>,
    enemies: BTreeMap<(usize, u32), (Arc<EnvelopeReach>, Arc<EnvelopeReach>)>,
}
thread_local! {
    static MOVEMENT_REACH: std::cell::RefCell<MovementReachCache> = std::cell::RefCell::new(MovementReachCache::default());
}

fn assign_assault_position(
    uid: u32,
    options: &BTreeMap<u32, Vec<Pos>>,
    assigned: &mut BTreeMap<Pos, u32>,
    visited: &mut BTreeSet<Pos>,
) -> bool {
    for position in &options[&uid] {
        if !visited.insert(*position) {
            continue;
        }
        let previous = assigned.get(position).copied();
        if previous.is_none_or(|other| assign_assault_position(other, options, assigned, visited)) {
            assigned.insert(*position, uid);
            return true;
        }
    }
    false
}

impl BasicAi {
    pub(super) fn movement_risk_frame(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> MovementRiskFrame {
        self.movement_risk_frame_for_group(g, pid, uid, target, None)
    }

    pub(super) fn movement_risk_frame_for_group(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        target: Pos,
        members: Option<&[u32]>,
    ) -> MovementRiskFrame {
        let source = self.enemy_attack_envelopes(g, pid);
        let envelopes = MOVEMENT_REACH.with(|slot| {
            let mut cache = slot.borrow_mut();
            if let Some((old, reach)) = cache.table.as_ref() {
                if Arc::ptr_eq(old, &source) {
                    return Arc::clone(reach);
                }
            }
            let mut probe = None;
            // Keep other seats' entries through the round; camping enemies
            // often have unchanged reach next turn too. Bound retained worlds
            // when a worker is reused for successive simulations.
            if cache.enemies.len() > 1024 {
                cache.enemies.clear();
            }
            let mut envelopes = Vec::new();
            for unit in g.units.values() {
                let spec = &g.rules.units[unit.kind];
                if unit.owner == pid
                    || !g.is_at_war(pid, unit.owner)
                    || spec.class != "military"
                    || !(spec.is_melee_capable() || spec.has_ranged_attack())
                    || !g.unit_visible_to(unit.id, pid)
                {
                    continue;
                }
                let raw = source
                    .iter()
                    .find(|(id, _)| *id == unit.id)
                    .map(|(_, reach)| reach);
                let hit = raw.and_then(|raw| {
                    cache
                        .enemies
                        .get(&(pid, unit.id))
                        .filter(|(old, _)| Arc::ptr_eq(raw, old))
                });
                let reach = if let Some((_, reach)) = hit {
                    Arc::clone(reach)
                } else {
                    let mut targets: Vec<_> =
                        raw.map_or_else(Vec::new, |old| old.iter().copied().collect());
                    targets.extend(super::advanced::movement_strike_reach(
                        probe.get_or_insert_with(|| g.speculative_clone()),
                        pid,
                        unit.id,
                    ));
                    targets.sort_unstable();
                    targets.dedup();
                    Arc::new(EnvelopeReach::from_tiles(targets))
                };
                if let Some(raw) = raw {
                    cache
                        .enemies
                        .insert((pid, unit.id), (Arc::clone(raw), Arc::clone(&reach)));
                }
                envelopes.push((unit.id, reach));
            }
            let reach = Arc::new(envelopes);
            cache.table = Some((source, Arc::clone(&reach)));
            reach
        });
        let target_city = g.city_at(target).filter(|cid| {
            let city = &g.cities[cid];
            city.owner != pid
                && g.is_at_war(pid, city.owner)
                && city.wall_hp > 0
                && (g.players[pid].explored.contains(&target) || g.player_can_see(pid, target))
                && g.wdist(g.units[&uid].pos, target) <= 6
        });
        let mut assault_positions = BTreeSet::new();
        let mut assault_units = BTreeSet::new();
        let mut ready = 0;
        if let Some(cid) = target_city {
            let city = &g.cities[&cid];
            let mut options = BTreeMap::new();
            // Reserve distinct legal endpoints. Three units that all need the
            // same gap in a mountain pass are not three simultaneous entrants.
            for friend in g.units.values().filter(|friend| {
                let spec = &g.rules.units[friend.kind];
                friend.owner == pid
                    && members.is_none_or(|ids| ids.contains(&friend.id))
                    && friend.hp >= 60
                    && spec.class == "military"
                    && (spec.is_melee_capable() || spec.has_ranged_attack())
                    && !g.is_embarked(friend)
                    && g.wdist(friend.pos, target) <= 6
            }) {
                let spec = &g.rules.units[friend.kind];
                let range = if spec.has_ranged_attack() {
                    g.unit_attack_range(friend.id)
                } else {
                    1
                };
                let mut positions = vec![friend.pos];
                positions.extend(g.reachable(friend.id));
                positions.sort_by_key(|pos| (g.wdist(*pos, target), *pos));
                let positions: Vec<_> = positions
                    .into_iter()
                    .filter(|pos| {
                        g.wdist(*pos, target) > 0
                            && g.wdist(*pos, target) <= range
                            && Self::city_centre_strikes(g, pid, city, *pos)
                            && g.city_at(*pos).is_none()
                            && g.encampment_at(*pos).is_none()
                            && g.line_of_sight_from(*pos, target)
                            && !g.map.get(*pos).is_some_and(|t| {
                                g.rules.is_water(t) && spec.domain.as_deref() != Some("sea")
                            })
                            && Self::movement_hazard_damage(g, pid, *pos) == 0.0
                            && Self::incoming_damage(g, pid, friend.id, *pos, &envelopes).total
                                * COMBAT_ROLL_MAX
                                < f64::from(friend.hp - 15)
                    })
                    .collect();
                options.insert(friend.id, positions);
            }
            let mut assigned = BTreeMap::new();
            for id in options.keys() {
                assign_assault_position(*id, &options, &mut assigned, &mut BTreeSet::new());
            }
            assault_positions.extend(assigned.keys().copied());
            assault_units.extend(assigned.values().copied());
            ready = assigned.len();
            let capture_ready = assault_units.iter().any(|id| {
                g.rules.units[g.units[id].kind].is_melee_capable()
                    && g.unit_can_melee_target_domain(*id, target)
            });
            if !capture_ready || (ready < 3 && city.wall_hp > 20 && city.hp > 60) {
                ready = 0;
            }
        }
        MovementRiskFrame {
            envelopes,
            target_city,
            target,
            assault_positions,
            assault_units,
            ready,
        }
    }

    /// Route orders (including civilian travel) compare holding and legal
    /// alternatives under the same damage budget as tactical movement. Leave
    /// completely quiet routes on the existing inexpensive pathing fallback.
    pub(super) fn risk_aware_route_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
        stop_range: i32,
    ) -> Option<bool> {
        if self.legacy_movement {
            return None;
        }
        let here = g.units.get(&uid)?.pos;
        if g.wdist(here, target) <= stop_range {
            return None;
        }
        // Founding consumes the settler before any enemy reply. Price the
        // real completion, not a civilian left on the founding tile overnight.
        if g.units[&uid].kind == "settler"
            && g.can_move(uid, target)
            && Self::movement_hazard_damage(g, pid, target) == 0.0
        {
            let mut future = g.speculative_clone();
            if future
                .apply(
                    pid,
                    &crate::game::Action::Move {
                        unit: uid,
                        to: target,
                    },
                )
                .is_ok()
                && future.units[&uid].moves_left > 0.0
                && future.can_found_city(uid)
            {
                return Some(self.path_move(g, pid, uid, target));
            }
        }
        let risk = self.movement_risk_frame(g, pid, uid, target);
        let mut candidates: Vec<Pos> = g
            .nbrs(here)
            .into_iter()
            .filter(|pos| g.can_move(uid, *pos))
            .collect();
        let routed = g
            .route_step(uid, target, stop_range)
            .filter(|pos| g.can_move(uid, *pos));
        let route_escape = self.live_livelock_route_escape(uid) && routed.is_some();
        if !std::iter::once(&here)
            .chain(candidates.iter())
            .any(|pos| Self::anything_can_reach(g, pid, *pos, &risk.envelopes))
            && !route_escape
        {
            return None;
        }
        if let Some(pos) = routed {
            if !candidates.contains(&pos) {
                candidates.push(pos);
            }
        }
        let prefer_dry =
            self.come_ashore && g.rules.units[g.units[&uid].kind].domain.as_deref() != Some("sea");
        let score = |pos| {
            let distance = g.wdist(pos, target);
            // A routed detour is actual progress even if hex distance grows.
            let progress = if Some(pos) == routed {
                distance.min(g.wdist(here, target) - 1)
            } else {
                distance
            };
            -3.0 * f64::from(progress) + risk.score(g, pid, uid, pos, self.w.mv_threat)
                - if prefer_dry && g.map.get(pos).is_some_and(|t| g.rules.is_water(t)) {
                    super::WATER_MARCH_PENALTY
                } else {
                    0.0
                }
                + self.livelock_penalty(uid, pos)
                + if route_escape && Some(pos) == routed {
                    super::LIVELOCK_ESCAPE_VALUE
                } else {
                    0.0
                }
        };
        let stay = score(here);
        candidates.sort_by(|a, b| score(*b).total_cmp(&score(*a)).then(a.cmp(b)));
        let candidates: Vec<_> = candidates
            .into_iter()
            .take_while(|pos| self.move_beats_holding(g, uid, score(*pos), stay))
            .collect();
        for pos in candidates {
            if self.path_move(g, pid, uid, pos) {
                return Some(true);
            }
            if route_escape
                && Some(pos) == routed
                && self.tactical_apply_livelock_route_escape(g, pid, uid, pos)
            {
                return Some(true);
            }
        }
        Some(false)
    }

    /// Known hazards only. Random future eruptions/floods are not observations.
    /// Fires are host features: shipped GranColombia_Maya_Expansion2.xml
    /// UNIT_DAMAGE_LAND rows 341/347 give 50..101 HP; 75 is the planning mean.
    pub(super) fn movement_hazard_damage(g: &Game, pid: usize, position: Pos) -> f64 {
        let Some(tile) = g.map.get(position) else {
            return 0.0;
        };
        let burning = matches!(
            tile.feature.as_deref(),
            Some("burning_forest" | "burning_jungle")
        );
        if (tile.fallout_until <= g.turn && !burning && g.storms.is_empty())
            || !g.player_can_see(pid, position)
        {
            return 0.0;
        }
        let mut damage = if g.fallout_at(position) {
            f64::from(Game::FALLOUT_UNIT_DAMAGE)
        } else {
            0.0
        };
        let immune = tile
            .owner_city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| g.governor_effect(city.owner, city.id, "disaster_immunity") > 0.0);
        if immune {
            return damage;
        }
        if matches!(
            tile.feature.as_deref(),
            Some("burning_forest" | "burning_jungle")
        ) {
            damage += 75.0;
        }
        for storm in &g.storms {
            if storm.ends <= g.turn.saturating_add(1) || !g.player_can_see(pid, storm.pos) {
                continue;
            }
            let Some(spec) = g.rules.disasters.get(&storm.kind) else {
                continue;
            };
            let next = g.map.step(storm.pos, storm.heading).unwrap_or(storm.pos);
            if g.wdist(next, position) <= spec.radius(storm.severity) {
                damage += spec.unit_damage.round().max(0.0);
            }
        }
        damage
    }
}

impl MovementRiskFrame {
    pub(super) fn retain_enemies(&mut self, visible: impl Fn(u32) -> bool) {
        self.envelopes = Arc::new(
            self.envelopes
                .iter()
                .filter(|(id, _)| visible(*id))
                .cloned()
                .collect(),
        );
    }

    fn attack_reward(&self, g: &Game, pid: usize, uid: u32, position: Pos) -> f64 {
        let mut attacker = g.units[&uid].clone();
        attacker.pos = position;
        if position != g.units[&uid].pos {
            attacker.fortify_turns = 0;
            attacker.fortified = false;
        }
        let spec = &g.rules.units[attacker.kind];
        if spec.class != "military" || g.is_embarked(&attacker) {
            return 0.0;
        }
        let range = if spec.has_ranged_attack() {
            g.unit_attack_range(uid)
        } else if spec.is_melee_capable() {
            1
        } else {
            return 0.0;
        };
        let distance = g.wdist(position, self.target);
        if distance == 0 || distance > range + 1 {
            return 0.0;
        }
        let Some(enemy) = g
            .unit_ids_at(self.target)
            .iter()
            .filter_map(|id| g.units.get(id))
            .find(|enemy| {
                g.is_at_war(pid, enemy.owner)
                    && g.unit_visible_to(enemy.id, pid)
                    && self.envelopes.iter().any(|(id, _)| *id == enemy.id)
            })
        else {
            return 0.0;
        };
        if !spec.has_ranged_attack() && !g.unit_can_melee_target_domain(uid, self.target) {
            return 0.0;
        }
        if spec.has_ranged_attack()
            && distance <= range
            && !g.unit_has_line_of_sight_from(uid, position, self.target)
        {
            return 0.0;
        }
        let attack = if spec.has_ranged_attack() {
            g.unit_ranged_attack_strength(&attacker)
        } else {
            g.unit_strength(&attacker, false)
        };
        let defense = g.unit_strength(enemy, true) + g.tile_defense_bonus(enemy.pos);
        let damage = (30.0
            * ((effective_strength(attack, attacker.hp) - effective_strength(defense, enemy.hp))
                / 25.0)
                .exp())
        .clamp(1.0, f64::from(enemy.hp.max(1)));
        // Contact and the approach that opens it both have objective value.
        // This never reduces the independent loss-of-unit penalty below.
        damage * if distance <= range { 1.0 } else { 0.55 }
    }

    pub(super) fn score(&self, g: &Game, pid: usize, uid: u32, position: Pos, weight: f64) -> f64 {
        let unit = &g.units[&uid];
        let hazard = BasicAi::movement_hazard_damage(g, pid, position);
        let mut incoming = BasicAi::incoming_damage(g, pid, uid, position, &self.envelopes);
        if g.rules.units[unit.kind].class != "military" {
            // A stacked escort that can accompany this step takes the blows
            // first. Do not mistake the protected civilian for a lone unit;
            // a guard that can be killed is not protection from capture.
            if let Some(guard) = g
                .unit_ids_at(unit.pos)
                .iter()
                .filter_map(|id| g.units.get(id))
                .find(|guard| {
                    guard.owner == pid
                        && g.rules.units[guard.kind].class == "military"
                        && (guard.pos == position || g.reachable(guard.id).contains(&position))
                })
            {
                let cover = BasicAi::incoming_damage(g, pid, guard.id, position, &self.envelopes);
                if cover.total * COMBAT_ROLL_MAX < f64::from(guard.hp - 15) {
                    incoming.total = cover.total * 0.1 + hazard * 0.9;
                    incoming.worst = (cover.worst * 0.1).max(hazard);
                }
            }
        }
        let total = incoming.total;
        let mut charged = total;
        let mut opening = 0.0;
        if self.ready > 0 && unit.hp >= 60 && self.assault_units.contains(&uid) {
            if let Some(city) = self.target_city.and_then(|cid| g.cities.get(&cid)) {
                if BasicAi::city_centre_strikes(g, pid, city, position)
                    && g.city_at(position).is_none()
                    && g.encampment_at(position).is_none()
                {
                    let mut standing = unit.clone();
                    standing.pos = position;
                    if position != unit.pos {
                        standing.fortify_turns = 0;
                        standing.fortified = false;
                    }
                    let defense = effective_strength(
                        g.unit_strength(&standing, true) + g.tile_defense_bonus(position),
                        standing.hp,
                    );
                    let shot = (30.0 * ((g.city_ranged_strength(city.id) - defense) / 25.0).exp())
                        .clamp(1.0, 100.0);
                    // Only the objective city's one shot is shared. Another
                    // city and its encampment keep their independent budgets.
                    charged -= shot * (1.0 - 1.0 / self.ready.min(6) as f64);
                    if self
                        .assault_positions
                        .iter()
                        .any(|endpoint| g.wdist(position, *endpoint) <= 1)
                    {
                        opening = 6.0;
                    }
                }
            }
        }
        let hp = f64::from(unit.hp.max(1));
        let wounds = 1.0 + (100.0 - hp).max(0.0) / 100.0;
        // Focus fire remains possible: never divide the loss penalty by the
        // number of friends. The highest roll of a single shot stays fatal.
        let worst = (incoming.worst * COMBAT_ROLL_MAX).max(hazard);
        let loss = ((total - hp * 0.65).max(0.0) / hp).powi(2) * 100.0
            + if worst >= hp { 100.0 } else { 0.0 };
        let escort = if position == self.target
            && g.rules.units[unit.kind].class == "military"
            && g.unit_ids_at(position).iter().any(|id| {
                let civilian = &g.units[id];
                civilian.owner == pid && g.rules.units[civilian.kind].class != "military"
            }) {
            20.0
        } else {
            0.0
        };
        opening + escort + self.attack_reward(g, pid, uid, position)
            - weight.max(0.0) * charged.max(0.0) * wounds
            - loss
    }
}

#[cfg(test)]
mod tests;
