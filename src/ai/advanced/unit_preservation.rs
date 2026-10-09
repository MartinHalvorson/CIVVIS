//! Validate the whole proposed turn before committing combat orders. Enemy
//! damage is a focus-fire budget, never divided among friendly targets. A
//! survivor must also have a survivable stand after the next full movement
//! refresh against enemies that had a turn to reposition. Withdrawn units
//! keep recovering until full health. Off, the original turn is untouched.

use super::battle_planner::{melee_health_floor, DangerField};
use super::AdvancedAi;
use crate::game::{expected_damage, Action, Game, Unit};
use crate::think;
use crate::Pos;
use std::collections::{BTreeMap, BTreeSet};

const RESERVE_HP: i32 = 15;
// `game::damage` and the live finisher use the same 0.8 lower roll.
const MIN_COMBAT_ROLL: f64 = 0.8;

/// Count each source once and price later blows at the health left by earlier
/// ones. Damage rolls are bounded independently, including their rounding.
fn reply_damage(field: &mut DangerField, pos: Pos, uid: u32, hp: i32) -> f64 {
    let blows = field.contributions_at_hp(pos, uid, hp);
    let mut order: Vec<usize> = (0..blows.len()).collect();
    order.sort_by(|a, b| blows[*b].1.total_cmp(&blows[*a].1).then(a.cmp(b)));
    let mut left = hp;
    for index in order {
        let current = field.contributions_at_hp(pos, uid, left.max(1));
        let mean = current.get(index).map_or(blows[index].1, |(_, blow)| *blow);
        let damage = (mean * crate::ai::COMBAT_ROLL_MAX).ceil().min(100.0) as i32;
        left -= damage;
        if left <= 0 {
            break;
        }
    }
    f64::from(hp - left)
}

fn military(g: &Game, pid: usize, uid: u32) -> bool {
    g.units
        .get(&uid)
        .is_some_and(|unit| unit.owner == pid && g.rules.units[unit.kind].class == "military")
}

fn actors(action: &Action) -> Vec<u32> {
    match action {
        Action::Move { unit, .. }
        | Action::MoveTo { unit, .. }
        | Action::Attack { unit, .. }
        | Action::Ranged { unit, .. }
        | Action::PriorityTarget { unit, .. }
        | Action::AirStrike { unit, .. }
        | Action::AirRebase { unit, .. }
        | Action::AirPatrol { unit, .. }
        | Action::AirPillage { unit, .. }
        | Action::CoastalRaid { unit, .. }
        | Action::Pillage { unit }
        | Action::Fortify { unit } => vec![*unit],
        Action::Swap { unit, other } => vec![*unit, *other],
        _ => Vec::new(),
    }
}

fn replay(before: &Game, pid: usize, actions: &[Action], blocked: &BTreeSet<u32>) -> Game {
    let mut after = before.speculative_clone();
    let mut health_floors = BTreeMap::new();
    let mut defender_floors: BTreeMap<u32, (Unit, i32)> = BTreeMap::new();
    for action in actions {
        if matches!(action, Action::EndTurn)
            || actors(action).iter().any(|uid| blocked.contains(uid))
        {
            continue;
        }
        let floor = match action {
            Action::Attack { unit, target } => after
                .city_at(*target)
                .filter(|city| after.is_at_war(pid, after.cities[city].owner))
                .and_then(|city| after.city_melee_exchange_strengths(*unit, city))
                .and_then(|(att, def)| {
                    after.units.get(unit).map(|actor| {
                        (
                            *unit,
                            actor.hp
                                - (expected_damage(def, att) * crate::ai::COMBAT_ROLL_MAX)
                                    .ceil()
                                    .min(100.0) as i32,
                        )
                    })
                })
                .or_else(|| melee_health_floor(&after, pid, action)),
            _ => None,
        };
        let victims = match action {
            Action::Attack { unit, target } | Action::Ranged { unit, target }
                if after.city_at(*target).is_none() && after.encampment_at(*target).is_none() =>
            {
                let ranged = matches!(action, Action::Ranged { .. });
                after
                    .unit_ids_at(*target)
                    .iter()
                    .filter_map(|uid| {
                        let defender = after.units.get(uid)?;
                        if !after.is_at_war(pid, defender.owner)
                            || after.rules.units[defender.kind].class != "military"
                        {
                            return None;
                        }
                        let pair = if ranged {
                            after.ranged_strike_strengths(*unit, *uid, *target)
                        } else {
                            after.melee_exchange_strengths(*unit, *uid)
                        }?;
                        let mean = after
                            .host_previews
                            .get(&(*unit, *target, ranged))
                            .map_or_else(
                                || expected_damage(pair.0, pair.1),
                                |preview| f64::from(preview.damage_to_defender),
                            );
                        Some((defender.clone(), (mean * MIN_COMBAT_ROLL).floor() as i32))
                    })
                    .collect::<Vec<_>>()
            }
            _ => Vec::new(),
        };
        let previous_hp = floor.and_then(|(uid, _)| after.units.get(&uid).map(|unit| unit.hp));
        if after.apply(pid, action).is_ok() {
            for (victim, damage) in victims {
                let entry = defender_floors
                    .entry(victim.id)
                    .or_insert((victim.clone(), victim.hp));
                entry.1 -= damage;
            }
            if let Some((uid, hp)) = floor {
                let previous_hp = previous_hp.expect("a floor has an actor");
                let budget = health_floors.entry(uid).or_insert(previous_hp);
                *budget -= previous_hp - hp;
            }
        }
    }
    // A favorable sampled kill is not a promised host kill. Keep any victim
    // the lower damage budget could leave alive in the reply forecast.
    for (uid, (mut saved, hp)) in defender_floors {
        if hp > 0 {
            if let Some(unit) = after.units.get_mut(&uid) {
                unit.hp = unit.hp.max(hp);
            } else {
                saved.hp = hp;
                let pos = saved.pos;
                after.units.insert(uid, saved);
                after.relocate(uid, pos);
            }
        }
    }
    // Keep the exact action sequence intact while replaying: reducing health
    // mid-sequence could change whether a later kill advances its attacker.
    // Only the reply assessment receives the conservative health budget.
    for (uid, hp) in health_floors {
        if let Some(unit) = after.units.get_mut(&uid) {
            unit.hp = unit.hp.min(hp);
        }
    }
    after
}

impl AdvancedAi {
    pub(super) fn preservation_finishing_safe(
        &self,
        before: &Game,
        pid: usize,
        actions: &[Action],
    ) -> bool {
        let after = replay(before, pid, actions, &BTreeSet::new());
        actions
            .iter()
            .filter_map(|action| match action {
                Action::Attack { unit, .. } | Action::Ranged { unit, .. } => Some(*unit),
                _ => None,
            })
            .all(|uid| {
                !self.battle_planner_recovering.contains(&uid)
                    && self.reply_outcomes_are_safe(before, &after, pid, uid)
            })
    }

    /// A safe immediate reply is insufficient when the survivor cannot get
    /// away from the following one. Use only currently visible hostile facts;
    /// enemy relocation is a possible route, not knowledge of a future order.
    pub(super) fn unit_reply_is_safe(&self, g: &Game, pid: usize, uid: u32) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        if unit.hp <= 0 {
            return false;
        }
        let near_hostile = g.units.values().any(|enemy| {
            let spec = &g.rules.units[enemy.kind];
            g.is_at_war(pid, enemy.owner)
                && g.unit_visible_to(enemy.id, pid)
                && spec.class == "military"
        });
        let near_structure = g
            .cities
            .values()
            .any(|city| g.is_at_war(pid, city.owner) && g.wdist(city.pos, unit.pos) <= 5);
        if !near_hostile
            && !near_structure
            && crate::ai::BasicAi::movement_hazard_damage(g, pid, unit.pos) <= 0.0
        {
            return true;
        }
        let mut first = DangerField::with_reach(g, pid, true);
        let incoming = reply_damage(&mut first, unit.pos, uid, unit.hp);
        let left = unit.hp - incoming.ceil() as i32;
        if left <= 0 || (incoming > 0.0 && left < RESERVE_HP) {
            return false;
        }
        // Aircraft cannot escape by a land movement flood. Their interception
        // damage is already applied by the speculative strike; retain the
        // immediate reply check rather than inventing a walking route.
        if g.rules.units[unit.kind].domain.as_deref() == Some("air") {
            return true;
        }
        let mut next = g.speculative_clone();
        let moves = next.unit_max_moves(uid);
        let survivor = next.units.get_mut(&uid).expect("present above");
        survivor.hp = left;
        survivor.moves_left = moves;
        survivor.moved = false;
        survivor.acted = false;
        survivor.zoc_stopped = false;
        survivor.started_turn_in_zoc = false;
        let mut second = DangerField::second_turn(&next, pid);
        let mut stands = vec![unit.pos];
        stands.extend(next.reachable(uid));
        stands.into_iter().any(|stand| {
            // Enemy-occupied endpoints are attacks, not retreat routes.
            !next
                .unit_ids_at(stand)
                .iter()
                .any(|other| next.is_at_war(pid, next.units[other].owner))
                && reply_damage(&mut second, stand, uid, left) < f64::from(left)
        })
    }

    fn reply_outcomes_are_safe(&self, before: &Game, after: &Game, pid: usize, uid: u32) -> bool {
        if !self.unit_reply_is_safe(after, pid, uid) {
            return false;
        }
        let Some(unit) = after.units.get(&uid) else {
            return false;
        };
        // A restored uncertain victim can share the sampled melee landing
        // tile on this forecast. Also assess the branch where it survived and
        // the attacker never advanced. Neither branch promises the other.
        if after
            .unit_ids_at(unit.pos)
            .iter()
            .any(|other| after.is_at_war(pid, after.units[other].owner))
        {
            let Some(start) = before.units.get(&uid) else {
                return false;
            };
            let mut failed_kill = after.speculative_clone();
            failed_kill.relocate(uid, start.pos);
            return self.unit_reply_is_safe(&failed_kill, pid, uid);
        }
        true
    }

    /// Remove unsafe unit sequences to a fixed point: rejecting a friendly
    /// kill can restore a threat to another unit. Then use the remaining
    /// board to select legal retreats and hold recovering units out of combat.
    pub(super) fn preserve_unit_actions(
        &mut self,
        before: &Game,
        pid: usize,
        actions: &[Action],
    ) -> Vec<Action> {
        self.battle_planner_recovering.retain(|uid| {
            before
                .units
                .get(uid)
                .is_some_and(|unit| unit.owner == pid && unit.hp < 100)
        });
        let mut blocked = self.battle_planner_recovering.clone();
        let affected: BTreeSet<u32> = actions
            .iter()
            .flat_map(actors)
            .filter(|uid| military(before, pid, *uid))
            .collect();
        loop {
            let after = replay(before, pid, actions, &blocked);
            let unsafe_units: Vec<u32> = affected
                .iter()
                .copied()
                .filter(|uid| {
                    !blocked.contains(uid)
                        && !self.reply_outcomes_are_safe(before, &after, pid, *uid)
                })
                .collect();
            if unsafe_units.is_empty() {
                break;
            }
            blocked.extend(unsafe_units);
        }
        let mut kept: Vec<Action> = actions
            .iter()
            .filter(|action| !actors(action).iter().any(|uid| blocked.contains(uid)))
            .filter(|action| !matches!(action, Action::EndTurn))
            .cloned()
            .collect();
        let mut board = replay(before, pid, &kept, &BTreeSet::new());
        // Unordered units can be exposed too, e.g. a gun exempted by a siege.
        for uid in before.player_unit_ids(pid) {
            if military(before, pid, uid) && !self.unit_reply_is_safe(&board, pid, uid) {
                blocked.insert(uid);
            }
        }
        for uid in blocked {
            let Some(unit) = board.units.get(&uid).cloned() else {
                continue;
            };
            if unit.moves_left <= 0.0 {
                continue;
            }
            let mut first = DangerField::with_reach(&board, pid, true);
            let mut stands = vec![unit.pos];
            stands.extend(board.reachable(uid));
            let mut choices = Vec::new();
            for stand in stands {
                let mut trial = board.speculative_clone();
                if stand != unit.pos
                    && trial
                        .apply(
                            pid,
                            &Action::MoveTo {
                                unit: uid,
                                to: stand,
                            },
                        )
                        .is_err()
                {
                    continue;
                }
                if trial.units.get(&uid).is_none_or(|now| now.pos != stand) {
                    continue;
                }
                let safe = self.unit_reply_is_safe(&trial, pid, uid);
                let incoming = reply_damage(&mut first, stand, uid, unit.hp);
                choices.push((
                    !safe,
                    incoming.ceil() as i32,
                    -trial.unit_heal_rate_at(uid, stand),
                    board.wdist(unit.pos, stand),
                    stand,
                ));
            }
            choices.sort_unstable();
            let Some(&(unsafe_stand, incoming, _, _, stand)) = choices.first() else {
                continue;
            };
            // If all retreats are losing, still reduce incoming damage rather
            // than spending the turn on a suicidal attack or holding worse.
            if stand != unit.pos {
                let action = Action::MoveTo {
                    unit: uid,
                    to: stand,
                };
                if board.apply(pid, &action).is_ok() {
                    kept.push(action);
                }
            }
            let fortify = Action::Fortify { unit: uid };
            if board.apply(pid, &fortify).is_ok() {
                kept.push(fortify);
            }
            if (!before.is_arena() || before.tactics.heal) && unit.hp < 100 {
                self.battle_planner_recovering.insert(uid);
            }
            think!(self.journal(), Military, Decision,
                "Preserve the {} at {:?}: retreat or heal", unit.kind, stand;
                "{} hp; upper reply {}{}; its proposed orders are withheld until the reply and the next escape are survivable",
                unit.hp, incoming, if unsafe_stand { "; no safe retreat is reachable" } else { "" });
        }
        if actions
            .iter()
            .any(|action| matches!(action, Action::EndTurn))
        {
            kept.push(Action::EndTurn);
        }
        kept
    }
}

#[cfg(test)]
mod tests;
