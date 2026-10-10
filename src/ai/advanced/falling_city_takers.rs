//! `falling-city-calls-the-takers`: a hostile city with its walls down and at
//! most [`TAKER_CALL_CITY_HP`] health is taken this turn when a body can, and
//! otherwise calls the nearest healthy land melee bodies one safe step in.
//!
//! **Why (10-08/10-09 live Emperor census, host combat rows).** 27 siege
//! targets were driven to walls 0 and at most 25 health by our own strikes;
//! 17 fell within two turns, 4 took three to eight, and 6 never fell. In the
//! ten slow or lost cases healthy melee bodies stood 4 to 9 tiles off and
//! held there, or walked away, while the city sat at 1 to 25 health for 3 to
//! 7 turns and then healed: Amboise (civvis-20261009T212029Z, t149) "waits
//! for a body" with a Knight at 8 tiles and a Man-at-Arms at 9; Majapahit
//! (civvis-20261009T114819Z) at 1 health turns 163 to 169 with a full
//! Pike-and-Shot holding 7 tiles out; Shizuoka (civvis-20261009T065447Z) at 1
//! health four turns with a Knight at 5 and a Courser at 4 to 7. The breach
//! the air surge finishes covers only cities at one health, inside six tiles,
//! with the surge running, and moves only the single nearest body.
//!
//! A city behind fallen walls cannot strike, so the approach costs only the
//! danger of the units around it, which the step reads.

use super::{AdvancedAi, StrategicPlan};
use crate::game::Game;
use std::collections::BTreeSet;

/// A falling city: walls down and at most this much health
/// (`falling-city-outranks-the-heal`'s line).
pub(super) const TAKER_CALL_CITY_HP: i32 = super::battle_planner::FALLING_CITY_HP;
/// How far a falling city calls a taker from.
pub(super) const TAKER_CALL_REACH: i32 = super::battle_planner::FALLING_CITY_REACH;
/// Turns of its own movement within which a called taker's road must reach
/// the city's ring.
pub(super) const TAKER_CALL_TURNS: f64 = 3.0;
/// Bodies called to one falling city on one turn.
pub(super) const TAKER_CALL_PER_CITY: usize = 2;
/// Falling cities served on one turn.
pub(super) const TAKER_CALL_CITIES: usize = 2;
/// The health a called body must have: fit to fight
/// (`battle_planner::ROTATE_HP`).
pub(super) const TAKER_CALL_MIN_HP: i32 = super::battle_planner::ROTATE_HP;

impl AdvancedAi {
    /// The falling cities this turn, plan target and capitals first, then the
    /// weakest. Empty with the gene off.
    pub(super) fn falling_cities_calling(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> Vec<u32> {
        if !self.falling_city_calls_the_takers {
            return Vec::new();
        }
        let mut falling: Vec<u32> = g
            .cities
            .values()
            .filter(|city| {
                city.owner != pid
                    && g.is_at_war(pid, city.owner)
                    && city.wall_hp <= 0
                    && city.hp <= TAKER_CALL_CITY_HP
                    && self.air_assault_target_visible(g, pid, city.pos)
            })
            .map(|city| city.id)
            .collect();
        falling.sort_by_key(|cid| {
            let city = &g.cities[cid];
            (Some(*cid) != plan.target_city, !city.is_capital, city.hp, *cid)
        });
        falling.truncate(TAKER_CALL_CITIES);
        falling
    }

    /// Our land melee bodies with movement left that could take or close on
    /// the city at `cid`: unspent, unlinked, not already claimed this turn.
    fn falling_city_bodies(&self, g: &Game, pid: usize, cid: u32, claimed: &BTreeSet<u32>, min_hp: i32) -> Vec<u32> {
        let target = g.cities[&cid].pos;
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                !claimed.contains(uid)
                    && spec.class == "military"
                    && spec.is_melee_capable()
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && !g.is_embarked(unit)
                    && unit.hp >= min_hp
                    && unit.moves_left > 0.0
                    && !unit.acted
                    && unit.linked_to.is_none()
                    && g.wdist(unit.pos, target) <= TAKER_CALL_REACH
            })
            .collect()
    }

    /// Run after the air assault's breach pass, before the unit loop: take
    /// each falling city a body can take this turn, else call up to
    /// [`TAKER_CALL_PER_CITY`] bodies one safe step closer. Returns the
    /// bodies it moved, which the unit loop then leaves alone.
    pub(super) fn plan_falling_city_takers(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        claimed: &BTreeSet<u32>,
    ) -> BTreeSet<u32> {
        let mut reserved = BTreeSet::new();
        for cid in self.falling_cities_calling(g, pid, plan) {
            let Some(city) = g.cities.get(&cid) else {
                continue;
            };
            if city.owner == pid || !g.is_at_war(pid, city.owner) || Self::should_defer_city_capture(g, pid, cid) {
                continue;
            }
            let (target, name, health) = (city.pos, city.name.clone(), city.hp);
            let mut taken: BTreeSet<u32> = claimed.union(&reserved).copied().collect();
            // Take it now if a body's ring step and blow do. Only bodies that
            // could reach the ring this turn are simulated.
            let mut ready: Vec<u32> = self
                .falling_city_bodies(g, pid, cid, &taken, super::air_city_assault::AIR_ASSAULT_BREACH_TAKER_HP)
                .into_iter()
                .filter(|uid| {
                    let unit = &g.units[uid];
                    f64::from(g.wdist(unit.pos, target)) <= unit.moves_left.ceil() + 1.0
                })
                .collect();
            ready.sort_by(|left, right| {
                let blow = |uid: &u32| {
                    let unit = &g.units[uid];
                    g.unit_strength(unit, false) * f64::from(unit.hp.clamp(0, 100)) / 100.0
                };
                blow(right).total_cmp(&blow(left)).then_with(|| left.cmp(right))
            });
            if let Some((taker, actions)) = self
                .air_assault_best_capture(g, pid, cid, &ready)
                .filter(|(taker, _)| self.capture_holdable(g, pid, cid, *taker))
            {
                let origin = g.units[&taker].pos;
                if actions.iter().all(|action| g.apply(pid, action).is_ok()) {
                    self.resolve_city_dispositions(g, pid, plan.strategy);
                    reserved.insert(taker);
                    crate::think!(self.journal(), Military, Decision,
                        "Taking the falling city of {}", name;
                        "{} {} walks in from {} tiles on {} health behind fallen walls; city captured {}",
                        crate::reasoning::plain(g.units.get(&taker).map_or("unit", |unit| unit.kind.as_str())),
                        taker, g.wdist(origin, target), health,
                        g.cities.get(&cid).is_some_and(|city| city.owner == pid);
                        target);
                    continue;
                }
            }
            // Nobody takes it this turn: call the nearest fit bodies in.
            taken = claimed.union(&reserved).copied().collect();
            let mut callers: Vec<(usize, u32)> = self
                .falling_city_bodies(g, pid, cid, &taken, TAKER_CALL_MIN_HP)
                .into_iter()
                .filter(|uid| g.wdist(g.units[uid].pos, target) >= 2)
                .filter_map(|uid| {
                    let moves = g.rules.units[g.units[&uid].kind].moves.max(1.0);
                    g.route_distance(uid, target, 1)
                        .filter(|steps| *steps as f64 <= TAKER_CALL_TURNS * moves)
                        .map(|steps| (steps, uid))
                })
                .collect();
            callers.sort();
            let mut called = 0;
            for (steps, uid) in callers {
                if called >= TAKER_CALL_PER_CITY {
                    break;
                }
                let from = g.wdist(g.units[&uid].pos, target);
                let mut board = g.speculative_clone();
                if self
                    .air_assault_advance(&mut board, pid, uid, target, true)
                    .filter(|action| g.apply(pid, action).is_ok())
                    .is_none()
                {
                    continue;
                }
                reserved.insert(uid);
                called += 1;
                crate::think!(self.journal(), Military, Decision,
                    "The {} closes on the falling city of {}", crate::reasoning::plain(g.units[&uid].kind.as_str()), name;
                    "{} health behind fallen walls; from {} tiles to {} ({} road steps out)",
                    health, from, g.wdist(g.units[&uid].pos, target), steps;
                    target);
            }
        }
        reserved
    }
}

#[cfg(test)]
mod tests;
