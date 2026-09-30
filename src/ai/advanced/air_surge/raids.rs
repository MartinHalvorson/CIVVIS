//! Cavalry raiders behind the wing: spare surge cavalry pillage the enemy's
//! districts and improvements while the Bombers empty the objective.
//!
//! ★★★ THE CAVALRY ONLY EVER WAITED. Live King 20260930T211803Z carried six
//! surge cavalry against a four-body quota through the Edirne campaign and
//! issued 18 PILLAGE orders in 248 turns, every one of them a unit spending
//! leftover movement on the tile it already stood on. The game was lost to a
//! rival's Technology victory at turn 248. A pillaged Campus stops that
//! rival's science until a Builder repairs it and pays us its plunder;
//! a Commercial Hub, a mine or a lumber mill pays Gold into a treasury that
//! read -30 a turn at turn 150. Cavalry are the units that can walk onto the
//! tile and still have the movement to pillage it.
//!
//! **Bounded on purpose.** Only an `air-surge-2` Domination surge at war with
//! its target raids, and only with cavalry the capture does not need: the
//! nearest [`AIR_SURGE_LAUNCH_BODIES`] healthy land melee bodies to the
//! objective are never sent. A raid is simulated on a board clone first (the
//! walk, the pillage and the danger the raider is left in), so the plunder it
//! is valued by is the engine's own, era scaling included, and a raid that
//! would leave the raider in reach of more than half its health in blows is
//! never ordered.

use super::*;
use std::collections::BTreeSet;

/// A raider stays at least this healthy.
const RAID_MIN_HP: i32 = 60;
/// How much of its health a raider may be left exposed to after pillaging.
const RAID_DANGER_SHARE: f64 = 0.5;
/// What one point of each plunder is worth to a Domination seat. Science is
/// priced above Gold because the same pillage also stops a rival's Campus.
const RAID_SCIENCE: f64 = 2.0;
const RAID_GOLD: f64 = 1.0;
const RAID_CULTURE: f64 = 1.2;
const RAID_FAITH: f64 = 0.6;
/// Health restored to a wounded raider, per point.
const RAID_HEAL: f64 = 0.8;
/// The denial worth of taking a district's yields away from its owner, on
/// top of its plunder, and the smaller worth of a pillaged improvement.
const RAID_DISTRICT_DENIAL: f64 = 40.0;
const RAID_IMPROVEMENT_DENIAL: f64 = 8.0;
/// Each tile of distance from the objective costs this much: a raider that
/// stays near the city is a capture body again next turn.
const RAID_DISTANCE_COST: f64 = 4.0;
/// Reachable tiles a raider simulates, nearest the objective first.
const RAID_TILES: usize = 16;

impl AdvancedAi {
    /// Plan and apply this turn's raids. Returns the raiders, which the
    /// ordinary unit loop must leave alone.
    pub(crate) fn plan_air_surge_raids(
        &mut self,
        g: &mut Game,
        pid: usize,
        reserved: &BTreeSet<u32>,
    ) -> BTreeSet<u32> {
        let mut raiders = BTreeSet::new();
        let Some(plan) = self.air_surge_plan.clone() else {
            return raiders;
        };
        if !self.air_surge_2
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || plan.phase != AirSurgePhase::Exploit
            || !g.is_at_war(pid, plan.target_player)
            || self.threatened_city(g, pid).is_some()
        {
            return raiders;
        }
        let objective = plan.objective_pos;
        let mut bodies: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.class == "military"
                    && spec.is_melee_capable()
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && !reserved.contains(uid)
            })
            .collect();
        bodies.sort_by_key(|uid| (g.wdist(g.units[uid].pos, objective), *uid));
        // The capture needs its nearest bodies where they are.
        let takers: BTreeSet<u32> = bodies
            .iter()
            .copied()
            .take(AIR_SURGE_LAUNCH_BODIES)
            .collect();
        let candidates: Vec<u32> = bodies
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                !takers.contains(uid)
                    && spec.cavalry
                    && unit.hp >= RAID_MIN_HP
                    && unit.moves_left > 0.0
                    && !unit.acted
                    && unit.linked_to.is_none()
                    && !g.is_embarked(unit)
            })
            .collect();
        for uid in candidates {
            let Some((value, target, actions)) = self.air_surge_best_raid(g, pid, uid, objective)
            else {
                continue;
            };
            if !actions.iter().all(|action| g.apply(pid, action).is_ok()) {
                continue;
            }
            raiders.insert(uid);
            Self::raid_count(g, pid);
            think!(self.journal(), Military, Decision,
                   "{} raids behind the wing", plain(g.units[&uid].kind.as_str());
                   "pillages {} for {value:.0}; the capture keeps its {} nearest bodies",
                   Self::raid_label(g, target), AIR_SURGE_LAUNCH_BODIES; target);
        }
        raiders
    }

    fn raid_count(g: &mut Game, pid: usize) {
        *g.players[pid]
            .counters
            .entry("air_surge:raid_pillages".to_string())
            .or_insert(0) += 1;
    }

    fn raid_label(g: &Game, pos: Pos) -> String {
        g.map
            .get(pos)
            .and_then(|tile| {
                tile.improvement
                    .map(|improvement| plain(improvement.as_str()))
                    .or_else(|| tile.district.map(|district| plain(district.as_str())))
            })
            .unwrap_or_else(|| "a tile".to_string())
    }

    /// The most valuable raid this unit can make this turn, simulated.
    fn air_surge_best_raid(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        objective: Pos,
    ) -> Option<(f64, Pos, Vec<Action>)> {
        let origin = g.units.get(&uid)?.pos;
        let mut tiles: Vec<Pos> = g
            .reachable(uid)
            .into_iter()
            .chain(std::iter::once(origin))
            .filter(|pos| g.pillageable_at(pid, *pos))
            .collect();
        tiles.sort_by_key(|pos| (g.wdist(*pos, objective), *pos));
        tiles.dedup();
        let mut best: Option<(f64, Pos, Vec<Action>)> = None;
        for tile in tiles.into_iter().take(RAID_TILES) {
            let mut board = g.speculative_clone();
            let mut actions = Vec::new();
            if tile != origin {
                let step = Action::MoveTo { unit: uid, to: tile };
                if board.apply(pid, &step).is_err()
                    || board.units.get(&uid).is_none_or(|unit| unit.pos != tile)
                    || board.units[&uid].moves_left <= 0.0
                {
                    continue;
                }
                actions.push(step);
            }
            let district = board
                .map
                .get(tile)
                .is_some_and(|t| t.improvement.is_none() && t.district.is_some());
            let before = (
                board.players[pid].gold,
                board.players[pid].research_overflow,
                board.players[pid].civic_overflow,
                board.players[pid].faith,
                board.units[&uid].hp,
            );
            let pillage = Action::Pillage { unit: uid };
            if board.apply(pid, &pillage).is_err() {
                continue;
            }
            actions.push(pillage);
            let Some(raider) = board.units.get(&uid) else {
                continue;
            };
            if battle_planner_strike_danger(&board, pid, tile, uid)
                >= f64::from(raider.hp) * RAID_DANGER_SHARE
            {
                continue;
            }
            let player = &board.players[pid];
            let value = (player.gold - before.0) * RAID_GOLD
                + (player.research_overflow - before.1) * RAID_SCIENCE
                + (player.civic_overflow - before.2) * RAID_CULTURE
                + (player.faith - before.3) * RAID_FAITH
                + f64::from(raider.hp - before.4) * RAID_HEAL
                + if district {
                    RAID_DISTRICT_DENIAL
                } else {
                    RAID_IMPROVEMENT_DENIAL
                }
                - f64::from(g.wdist(tile, objective)) * RAID_DISTANCE_COST;
            if value > 0.0
                && best.as_ref().is_none_or(|(old, old_tile, _)| {
                    value > *old || (value == *old && tile < *old_tile)
                })
            {
                best = Some((value, tile, actions));
            }
        }
        best
    }
}

fn battle_planner_strike_danger(g: &Game, pid: usize, tile: Pos, uid: u32) -> f64 {
    super::super::battle_planner::strike_danger(g, pid, tile, uid)
}

#[cfg(test)]
mod tests;
