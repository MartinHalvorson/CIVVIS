//! Validate the whole proposed turn before committing combat orders. Enemy
//! damage is a focus-fire budget, never divided among friendly targets. A
//! survivor must also have a survivable stand after the next full movement
//! refresh against enemies that had a turn to reposition. Withdrawn units
//! keep recovering until full health, apart from a guaranteed, survivable
//! adjacent city occupation. Off, the original turn is untouched.

use super::battle_planner::{melee_health_floor, DangerField};
use super::AdvancedAi;
use crate::game::{expected_damage, Action, Game, Unit};
use crate::think;
use crate::Pos;
use std::collections::{BTreeMap, BTreeSet};

const RESERVE_HP: i32 = 15;
// `game::damage` and the live finisher use the same 0.8 lower roll.
const MIN_COMBAT_ROLL: f64 = 0.8;

fn damage_floor(att: f64, def: f64) -> i32 {
    (30.0 * ((att - def) / 25.0).exp() * MIN_COMBAT_ROLL)
        .round()
        .clamp(1.0, 100.0) as i32
}

/// Share the enemy movement forecasts among units on the same proposed board.
/// The second field is built only when an immediate reply is survivable.
struct ReplyForecast {
    first: DangerField,
    second: Option<DangerField>,
}

impl ReplyForecast {
    fn new(g: &Game, pid: usize) -> Self {
        Self {
            first: DangerField::with_reach(g, pid, true),
            second: None,
        }
    }
}

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
        | Action::FoundCity { unit }
        | Action::Pillage { unit }
        | Action::Fortify { unit } => vec![*unit],
        Action::Swap { unit, other } => vec![*unit, *other],
        _ => Vec::new(),
    }
}

struct ReplayOutcome {
    board: Game,
    failed_advances: BTreeMap<u32, BTreeSet<Pos>>,
    uncertain_captures: BTreeSet<u32>,
    guaranteed_dead: BTreeSet<u32>,
}

fn replay(before: &Game, pid: usize, actions: &[Action], blocked: &BTreeSet<u32>) -> ReplayOutcome {
    let mut after = before.speculative_clone();
    let mut failed_advances: BTreeMap<u32, BTreeSet<Pos>> = BTreeMap::new();
    let mut uncertain_captures = BTreeSet::new();
    let mut health_floors = BTreeMap::new();
    let mut defender_floors: BTreeMap<u32, (Unit, i32)> = BTreeMap::new();
    for action in actions {
        if matches!(action, Action::EndTurn)
            || actors(action).iter().any(|uid| blocked.contains(uid))
        {
            continue;
        }
        // Reprice a later exchange at the health the earlier upper rolls
        // could leave, without changing the sampled movement/kill sequence.
        let risk = matches!(action, Action::Attack { .. } | Action::Ranged { .. }).then(|| {
            let mut risk = after.speculative_clone();
            for (uid, hp) in &health_floors {
                if let Some(unit) = risk.units.get_mut(uid) {
                    unit.hp = unit.hp.min(*hp);
                }
            }
            for (uid, (_, hp)) in &defender_floors {
                if let Some(unit) = risk.units.get_mut(uid) {
                    unit.hp = unit.hp.max(*hp);
                }
            }
            risk
        });
        let risk = risk.as_ref().unwrap_or(&after);
        let floor = match action {
            Action::Attack { unit, target } => risk
                .city_at(*target)
                .filter(|city| risk.is_at_war(pid, risk.cities[city].owner))
                .and_then(|city| risk.city_melee_exchange_strengths(*unit, city))
                .and_then(|(att, def)| {
                    risk.units.get(unit).map(|actor| {
                        (
                            *unit,
                            actor.hp
                                - (expected_damage(def, att) * crate::ai::COMBAT_ROLL_MAX)
                                    .ceil()
                                    .min(100.0) as i32,
                        )
                    })
                })
                .or_else(|| melee_health_floor(risk, pid, action)),
            _ => None,
        };
        let floor = floor.map(|(uid, hp)| {
            let Action::Attack { target, .. } = action else {
                return (uid, hp);
            };
            let actor = &risk.units[&uid];
            let Some(preview) = after.host_previews.get(&(uid, *target, false)) else {
                return (uid, hp);
            };
            let extra_wounds = f64::from((before.units[&uid].hp - actor.hp).max(0)) / 10.0;
            let host_retaliation = expected_damage(
                preview.defender_strength,
                (preview.attacker_strength - extra_wounds.ceil()).max(0.0),
            );
            (
                uid,
                hp.min(
                    actor.hp
                        - (host_retaliation * crate::ai::COMBAT_ROLL_MAX)
                            .ceil()
                            .min(100.0) as i32,
                ),
            )
        });
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
                            risk.ranged_strike_strengths(*unit, *uid, *target)
                        } else {
                            risk.melee_exchange_strengths(*unit, *uid)
                        }?;
                        let lower_damage = damage_floor(pair.0, pair.1);
                        let lower_damage = after
                            .host_previews
                            .get(&(*unit, *target, ranged))
                            .filter(|preview| {
                                preview.attacker_strength > 0.0 && preview.defender_strength > 0.0
                            })
                            .map_or(lower_damage, |preview| {
                                // A preview describes the observed frame,
                                // before any earlier exchange wounded us.
                                let wounds =
                                    f64::from((before.units[unit].hp - risk.units[unit].hp).max(0))
                                        / 10.0;
                                lower_damage.min(damage_floor(
                                    (preview.attacker_strength - wounds.ceil()).max(0.0),
                                    preview.defender_strength,
                                ))
                            });
                        Some((defender.clone(), lower_damage))
                    })
                    .collect::<Vec<_>>()
            }
            _ => Vec::new(),
        };
        let previous_hp = floor.and_then(|(uid, _)| risk.units.get(&uid).map(|unit| unit.hp));
        let approach = match action {
            Action::Attack { unit, target } => after.units.get(unit).map(|actor| {
                let city = after.city_at(*target).and_then(|cid| {
                    let city = &after.cities[&cid];
                    let initial = before.cities.get(&cid).unwrap_or(city);
                    after
                        .is_at_war(pid, city.owner)
                        .then_some((cid, initial.hp, initial.wall_hp))
                });
                (*unit, actor.pos, city)
            }),
            _ => None,
        };
        let healing_actors = match action {
            Action::Promote { unit, .. } => vec![*unit],
            _ => actors(action),
        };
        let healing: Vec<(u32, i32)> = healing_actors
            .into_iter()
            .filter(|uid| health_floors.contains_key(uid))
            .filter_map(|uid| after.units.get(&uid).map(|unit| (uid, unit.hp)))
            .collect();
        if after.apply(pid, action).is_ok() {
            let mut uncertain_kill = false;
            for (victim, damage) in victims {
                let entry = defender_floors
                    .entry(victim.id)
                    .or_insert((victim.clone(), victim.hp));
                entry.1 -= damage;
                uncertain_kill |= entry.1 > 0;
            }
            if let Some((uid, origin, city)) = approach {
                if after.units.get(&uid).is_some_and(|unit| unit.pos != origin) {
                    let uncertain_city = city.filter(|(cid, hp, walls)| {
                        after.cities.get(cid).is_some_and(|city| city.owner == pid)
                            && (*walls > 0 || *hp > 1)
                    });
                    if let Some((cid, _, _)) = uncertain_city {
                        uncertain_captures.insert(cid);
                    }
                    if uncertain_kill || uncertain_city.is_some() {
                        failed_advances.entry(uid).or_default().insert(origin);
                    }
                }
            }
            if let Some((uid, hp)) = floor {
                let previous_hp = previous_hp.expect("a floor has an actor");
                let budget = health_floors.entry(uid).or_insert(previous_hp);
                *budget -= previous_hp - hp;
            }
            // Pillaging and promotion can heal a wounded striker. Credit
            // only health actually gained by a successful explicit order.
            if floor.is_none() {
                for (uid, previous) in healing {
                    if let Some(unit) = after.units.get(&uid) {
                        let gained = (unit.hp - previous).max(0);
                        if let Some(hp) = health_floors.get_mut(&uid) {
                            *hp = (*hp + gained).min(100);
                        }
                    }
                }
            }
        }
    }
    // A favorable sampled kill is not a promised host kill. Keep any victim
    // the lower damage budget could leave alive in the reply forecast.
    let mut guaranteed_dead = BTreeSet::new();
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
        } else {
            guaranteed_dead.insert(uid);
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
    ReplayOutcome {
        board: after,
        failed_advances,
        uncertain_captures,
        guaranteed_dead,
    }
}

impl AdvancedAi {
    /// A bound, stacked land escort with the same proposed movement endpoint
    /// is one departure. A binding alone does not license rewriting a soldier
    /// or Settler whose proposed route has a different destination.
    fn shared_settler_departures(
        &self,
        before: &Game,
        pid: usize,
        actions: &[Action],
    ) -> BTreeMap<u32, u32> {
        let destinations: BTreeMap<u32, Pos> = actions
            .iter()
            .filter_map(|action| match action {
                Action::Move { unit, to } | Action::MoveTo { unit, to } => Some((*unit, *to)),
                _ => None,
            })
            .collect();
        self.settler_guards
            .iter()
            .filter_map(|(settler, guard)| {
                let civilian = before.units.get(settler)?;
                let soldier = before.units.get(guard)?;
                let destination = destinations.get(settler)?;
                (civilian.owner == pid
                    && civilian.kind == "settler"
                    && military(before, pid, *guard)
                    && before.rules.units[soldier.kind].domain.as_deref() != Some("air")
                    && civilian.pos == soldier.pos
                    && destinations.get(guard) == Some(destination))
                .then_some((*guard, *settler))
            })
            .collect()
    }

    pub(super) fn preservation_finishing_safe(
        &self,
        before: &Game,
        pid: usize,
        actions: &[Action],
    ) -> bool {
        let after = replay(before, pid, actions, &BTreeSet::new());
        let mut forecast = ReplyForecast::new(&after.board, pid);
        actions
            .iter()
            .filter_map(|action| match action {
                Action::Attack { unit, .. } | Action::Ranged { unit, .. } => Some(*unit),
                _ => None,
            })
            .all(|uid| {
                (!self.battle_planner_recovering.contains(&uid)
                    || self
                        .recovery_city_capture(before, pid, uid, actions)
                        .is_some_and(|target| {
                            after
                                .board
                                .units
                                .get(&uid)
                                .is_some_and(|unit| unit.pos == target)
                                && after
                                    .board
                                    .city_at(target)
                                    .is_some_and(|cid| after.board.cities[&cid].owner == pid)
                        }))
                    && self.reply_outcomes_are_safe(before, &after, pid, uid, &mut forecast)
            })
    }

    /// Admit only a capture this actor can complete alone from its observed
    /// adjacent tile. Keep the recovery latch and price its wounded exposed
    /// stand too: garrison shielding or an observed-world elimination must
    /// not manufacture permission to attack.
    fn recovery_city_capture(
        &self,
        before: &Game,
        pid: usize,
        uid: u32,
        actions: &[Action],
    ) -> Option<Pos> {
        let actor = before.units.get(&uid)?;
        let spec = &before.rules.units[actor.kind];
        if actor.owner != pid
            || spec.class != "military"
            || !spec.is_melee_capable()
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
            || before.is_embarked(actor)
            || self.settler_guards.iter().any(|(settler, guard)| {
                *guard == uid
                    && before
                        .units
                        .get(settler)
                        .is_some_and(|unit| unit.owner == pid && unit.kind == "settler")
            })
        {
            return None;
        }
        let sequence: Vec<Action> = actions
            .iter()
            .filter(|action| actors(action).contains(&uid))
            .cloned()
            .collect();
        let mut capture = None;
        for action in &sequence {
            match action {
                Action::Attack { target, .. }
                    if capture.is_none() && before.wdist(actor.pos, *target) == 1 =>
                {
                    capture = Some(*target);
                }
                Action::Fortify { .. } if capture.is_some() => {}
                _ => return None,
            }
        }
        let target = capture?;
        let city = &before.cities[&before.city_at(target)?];
        if !before.is_at_war(pid, city.owner) || city.hp > 1 || city.wall_hp > 0 {
            return None;
        }
        let outcome = replay(before, pid, &sequence, &BTreeSet::new());
        let survivor = outcome.board.units.get(&uid)?;
        if outcome.board.cities.get(&city.id)?.owner != pid
            || survivor.pos != target
            || !outcome.uncertain_captures.is_empty()
            || outcome.failed_advances.contains_key(&uid)
        {
            return None;
        }
        // Retain every observed enemy on this separate probe. The host may
        // know another city which the mirrored board has not observed yet.
        let mut exposed = before.speculative_clone();
        exposed.units.get_mut(&uid)?.hp = survivor.hp;
        if !self.unit_reply_is_safe(&exposed, pid, uid)
            || !self.reply_outcomes_are_safe(
                before,
                &outcome,
                pid,
                uid,
                &mut ReplyForecast::new(&outcome.board, pid),
            )
        {
            return None;
        }
        Some(target)
    }

    /// A safe immediate reply is insufficient when the survivor cannot get
    /// away from the following one. Use only currently visible hostile facts;
    /// enemy relocation is a possible route, not knowledge of a future order.
    pub(super) fn unit_reply_is_safe(&self, g: &Game, pid: usize, uid: u32) -> bool {
        self.unit_reply_with_forecast(g, pid, uid, &mut ReplyForecast::new(g, pid))
    }

    fn unit_reply_with_forecast(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        forecast: &mut ReplyForecast,
    ) -> bool {
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
        let incoming = reply_damage(&mut forecast.first, unit.pos, uid, unit.hp);
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
        let second = forecast
            .second
            .get_or_insert_with(|| DangerField::second_turn(g, pid));
        let mut stands = vec![unit.pos];
        stands.extend(next.reachable(uid));
        stands.into_iter().any(|stand| {
            // Enemy-occupied endpoints are attacks, not retreat routes.
            !next
                .unit_ids_at(stand)
                .iter()
                .any(|other| next.is_at_war(pid, next.units[other].owner))
                && {
                    let damage = reply_damage(second, stand, uid, left).ceil() as i32;
                    left - damage > 0 && (damage == 0 || left - damage >= RESERVE_HP)
                }
        })
    }

    fn reply_outcomes_are_safe(
        &self,
        before: &Game,
        outcome: &ReplayOutcome,
        pid: usize,
        uid: u32,
        forecast: &mut ReplyForecast,
    ) -> bool {
        let after = &outcome.board;
        if !self.unit_reply_with_forecast(after, pid, uid, forecast) {
            return false;
        }
        // A favorable roll may advance a striker into a protected captured
        // city. Check every uncertain melee advance at its actual approach
        // tile, including moves earlier in the same turn.
        let mut origins = outcome
            .failed_advances
            .get(&uid)
            .cloned()
            .unwrap_or_default();
        if !outcome.uncertain_captures.is_empty() {
            // Other units cannot rely on a capture eliminating that army.
            origins.insert(after.units[&uid].pos);
        }
        for origin in origins {
            let mut failed_kill = after.speculative_clone();
            for cid in &outcome.uncertain_captures {
                if let Some(city) = before.cities.get(cid) {
                    failed_kill.cities.insert(*cid, city.clone());
                    failed_kill.players[city.owner].alive = before.players[city.owner].alive;
                    failed_kill
                        .at_war
                        .insert((pid.min(city.owner), pid.max(city.owner)));
                    // A capture can erase the garrison or eliminate its
                    // owner's remaining army. Those disappearances depend on
                    // the capture too, unless their own damage proves a kill.
                    for enemy in before
                        .units
                        .values()
                        .filter(|unit| unit.owner == city.owner)
                    {
                        if !failed_kill.units.contains_key(&enemy.id)
                            && !outcome.guaranteed_dead.contains(&enemy.id)
                        {
                            failed_kill.units.insert(enemy.id, enemy.clone());
                            failed_kill.relocate(enemy.id, enemy.pos);
                        }
                    }
                }
            }
            failed_kill.relocate(uid, origin);
            if !self.unit_reply_is_safe(&failed_kill, pid, uid) {
                return false;
            }
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
        let captures: BTreeMap<u32, Pos> = self
            .battle_planner_recovering
            .iter()
            .filter_map(|uid| {
                self.recovery_city_capture(before, pid, *uid, actions)
                    .map(|target| (*uid, target))
            })
            .collect();
        let mut blocked = self.battle_planner_recovering.clone();
        blocked.retain(|uid| !captures.contains_key(uid));
        let departures = self.shared_settler_departures(before, pid, actions);
        let affected: BTreeSet<u32> = actions
            .iter()
            .flat_map(actors)
            .filter(|uid| military(before, pid, *uid))
            .collect();
        loop {
            let companions: Vec<u32> = departures
                .iter()
                .filter_map(|(guard, settler)| blocked.contains(guard).then_some(*settler))
                .collect();
            // Withhold the whole civilian route, including a founding order
            // whose intended site that route would have reached.
            blocked.extend(companions);
            let after = replay(before, pid, actions, &blocked);
            let mut forecast = ReplyForecast::new(&after.board, pid);
            let unsafe_units: Vec<u32> = affected
                .iter()
                .copied()
                .filter(|uid| {
                    !blocked.contains(uid)
                        && (captures.get(uid).is_some_and(|target| {
                            after
                                .board
                                .units
                                .get(uid)
                                .is_none_or(|unit| unit.pos != *target)
                                || after
                                    .board
                                    .city_at(*target)
                                    .is_none_or(|cid| after.board.cities[&cid].owner != pid)
                        }) || !self.reply_outcomes_are_safe(
                            before,
                            &after,
                            pid,
                            *uid,
                            &mut forecast,
                        ))
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
        let outcome = replay(before, pid, &kept, &BTreeSet::new());
        let mut forecast = ReplyForecast::new(&outcome.board, pid);
        // Unordered units can be exposed too, e.g. a gun exempted by a siege.
        for uid in before.player_unit_ids(pid) {
            if military(before, pid, uid)
                && !self.reply_outcomes_are_safe(before, &outcome, pid, uid, &mut forecast)
            {
                blocked.insert(uid);
            }
        }
        let mut board = outcome.board;
        for uid in blocked {
            // Companions move only with their guard's selected retreat.
            if !military(&board, pid, uid) {
                continue;
            }
            let Some(unit) = board.units.get(&uid).cloned() else {
                continue;
            };
            if unit.moves_left <= 0.0 {
                continue;
            }
            let mut stands = vec![unit.pos];
            stands.extend(board.reachable(uid));
            let companion = departures.get(&uid).copied().filter(|settler| {
                board
                    .units
                    .get(settler)
                    .is_some_and(|civilian| civilian.pos == unit.pos)
            });
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
                let together = companion.is_none_or(|settler| {
                    if trial.units[&settler].pos == stand {
                        return true;
                    }
                    let mut joint = trial.speculative_clone();
                    if joint
                        .apply(
                            pid,
                            &Action::MoveTo {
                                unit: settler,
                                to: stand,
                            },
                        )
                        .is_ok()
                        && joint
                            .units
                            .get(&settler)
                            .is_some_and(|now| now.pos == stand)
                    {
                        trial = joint;
                        true
                    } else {
                        false
                    }
                });
                // Vacating the origin can open an enemy route; assess and rank
                // the destination on the board after that actual move.
                let mut forecast = ReplyForecast::new(&trial, pid);
                let safe = self.unit_reply_with_forecast(&trial, pid, uid, &mut forecast);
                let incoming = reply_damage(&mut forecast.first, stand, uid, unit.hp);
                choices.push((
                    !safe,
                    // Prefer a survivable joint retreat. If none exists, a
                    // stranded civilian cannot require a soldier to die too.
                    safe && !together,
                    incoming.ceil() as i32,
                    !together,
                    -trial.unit_heal_rate_at(uid, stand),
                    board.wdist(unit.pos, stand),
                    stand,
                ));
            }
            choices.sort_unstable();
            let Some(&(unsafe_stand, _, incoming, separated, _, _, stand)) = choices.first() else {
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
            if !separated && board.units[&uid].pos == stand {
                if let Some(settler) = companion.filter(|settler| board.units[settler].pos != stand)
                {
                    let action = Action::MoveTo {
                        unit: settler,
                        to: stand,
                    };
                    if board.apply(pid, &action).is_ok() {
                        kept.push(action);
                    }
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

#[cfg(test)]
mod capture_recovery_tests;
