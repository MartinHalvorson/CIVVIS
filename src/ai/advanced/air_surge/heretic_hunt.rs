//! Heretic hunt: a faithless Domination seat runs down the enemy Apostles and
//! Missionaries converting its cities.
//!
//! ★★★ THE CONQUEROR LOST TO MISSIONARIES. Live King 20261001T022028Z held
//! thirteen cities, eleven of them Muslim and two Catholic at turn 130; by
//! turn 140 Brazil's spreaders had turned seven Catholic and Brazil won a
//! Religious victory at turn 147, against three times its military. The seat
//! issued ONE `CONDEMN_HERETIC` that game. 024402Z (Religious loss at 168)
//! issued ten in a hundred-turn war with a 31-city faith. `condemn_step` only
//! condemns a spreader already standing on or beside the unit that happens to
//! be acting; one that walks past three tiles from every garrison converts
//! unopposed. Condemning needs only a war with the spreader's owner, a land
//! military unit on its tile and a move left, and the seat is at war most of
//! the game.
//!
//! **Bounded on purpose.** Only spreaders inside our borders or within
//! [`HUNT_HOME_REACH`] tiles of one of our cities are hunted, only by bodies
//! already near home, only when the walk leaves a move to condemn with and the
//! hunter is not left exposed to half its health in blows, and a spreader of
//! a competing faith the empire is deliberately sheltering is spared
//! (`preserve_competing_faith`).

use super::*;
use std::collections::BTreeSet;

/// Spreaders this close to one of our cities are hunted.
const HUNT_HOME_REACH: i32 = 4;
/// Hunters start this close to one of our cities, so the hunt never pulls a
/// unit off a distant front.
const HUNTER_HOME_REACH: i32 = 6;
/// A hunter keeps at least this much health.
const HUNTER_MIN_HP: i32 = 30;
/// How much of its health a hunter may be left exposed to after condemning.
const HUNTER_DANGER_SHARE: f64 = 0.5;

impl AdvancedAi {
    /// Plan and apply this frame's condemnations. Returns the hunters, which
    /// the ordinary unit loop must leave alone.
    pub(crate) fn plan_heretic_hunt(
        &mut self,
        g: &mut Game,
        pid: usize,
        reserved: &BTreeSet<u32>,
    ) -> BTreeSet<u32> {
        let mut hunters = BTreeSet::new();
        if !self.air_surge_2 || self.active_victory_target(g) != Some(VictoryTarget::Domination) {
            return hunters;
        }
        let homes: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        if homes.is_empty() {
            return hunters;
        }

        // Apostles first: they convert hardest and fight theologically.
        let mut spreaders: Vec<(u8, i32, u32, Pos)> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && g.rules.units[unit.kind].class == "religious"
                    && (in_our_land(g, pid, unit.pos) || near_home(g, &homes, unit.pos, HUNT_HOME_REACH))
                    && !self.preserve_competing_faith(g, pid, unit)
            })
            .map(|unit| {
                let rank = match unit.kind.as_str() {
                    "apostle" => 0,
                    "missionary" => 1,
                    _ => 2,
                };
                let distance = homes
                    .iter()
                    .map(|home| g.wdist(*home, unit.pos))
                    .min()
                    .unwrap_or(i32::MAX);
                (rank, distance, unit.id, unit.pos)
            })
            .collect();
        spreaders.sort();
        for (_, _, target, at) in spreaders {
            if !g.units.contains_key(&target) {
                continue;
            }
            let mut candidates: Vec<(i32, u32)> = g
                .player_unit_ids(pid)
                .into_iter()
                .filter(|uid| {
                    let unit = &g.units[uid];
                    let spec = &g.rules.units[unit.kind];
                    spec.class == "military"
                        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                        && !reserved.contains(uid)
                        && !hunters.contains(uid)
                        && unit.hp >= HUNTER_MIN_HP
                        && unit.moves_left > 0.0
                        && !unit.acted
                        && unit.linked_to.is_none()
                        && !g.is_embarked(unit)
                        && near_home(g, &homes, unit.pos, HUNTER_HOME_REACH)
                        && g.wdist(unit.pos, at) <= spec.moves.max(1.0) as i32
                })
                .map(|uid| (g.wdist(g.units[&uid].pos, at), uid))
                .collect();
            candidates.sort();
            for (_, uid) in candidates {
                let Some(actions) = Self::hunt_actions(g, pid, uid, target, at) else {
                    continue;
                };
                if !actions.iter().all(|action| g.apply(pid, action).is_ok()) {
                    continue;
                }
                hunters.insert(uid);
                *g.players[pid]
                    .counters
                    .entry("heretic_hunt:condemned".to_string())
                    .or_insert(0) += 1;
                think!(self.journal(), Faith, Decision,
                       "{} condemns a heretic", plain(g.units[&uid].kind.as_str());
                       "an enemy spreader {} tiles from our cities, hunted before it converts one; a faithless Domination seat has no other counter"
                       , homes.iter().map(|home| g.wdist(*home, at)).min().unwrap_or(0); at);
                break;
            }
        }
        hunters
    }

    /// The walk onto the spreader's tile and the condemnation, simulated
    /// together: the walk must land with a move to spare, the condemnation
    /// must remove the spreader, and the hunter must not stand in reach of
    /// half its health.
    fn hunt_actions(g: &Game, pid: usize, uid: u32, target: u32, at: Pos) -> Option<Vec<Action>> {
        let mut board = g.speculative_clone();
        let mut actions = Vec::new();
        if board.units.get(&uid)?.pos != at {
            let step = Action::MoveTo { unit: uid, to: at };
            if board.apply(pid, &step).is_err()
                || board.units.get(&uid).is_none_or(|unit| unit.pos != at)
                || board.units[&uid].moves_left <= 0.0
            {
                return None;
            }
            actions.push(step);
        }
        let condemn = Action::CondemnHeretic {
            unit: uid,
            target_unit: target,
        };
        if board.apply(pid, &condemn).is_err() || board.units.contains_key(&target) {
            return None;
        }
        actions.push(condemn);
        let hp = f64::from(board.units.get(&uid)?.hp);
        (super::super::battle_planner::strike_danger(&board, pid, at, uid) < hp * HUNTER_DANGER_SHARE)
            .then_some(actions)
    }
}

fn near_home(g: &Game, homes: &[Pos], pos: Pos, reach: i32) -> bool {
    homes.iter().any(|home| g.wdist(*home, pos) <= reach)
}

fn in_our_land(g: &Game, pid: usize, pos: Pos) -> bool {
    g.map
        .get(pos)
        .and_then(|tile| tile.owner_city)
        .and_then(|cid| g.cities.get(&cid))
        .is_some_and(|city| city.owner == pid)
}

#[cfg(test)]
mod tests;
