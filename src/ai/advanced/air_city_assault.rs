//! A bounded city assault: preserve sight and a cavalry exit around the volley.
use super::{battle_planner, AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::{Action, Game};
use crate::Pos;
use std::collections::BTreeSet;

/// The chosen maneuver on a disposable planning board. Native adapters must
/// observe the spot and the volley before releasing the dependent phase.
#[derive(Clone, Debug)]
pub struct AirCityAssault {
    pub target: Pos,
    pub cavalry: u32,
    pub spot: Pos,
    pub moved_to_spot: bool,
    pub aircraft: Vec<u32>,
}

struct Opening {
    cavalry: u32,
    origin: Pos,
    spot: Pos,
    actions: Vec<Action>,
    board: Game,
    spent: f64,
}

fn walk(g: &mut Game, pid: usize, uid: u32, to: Pos) -> Option<Action> {
    let action = Action::MoveTo { unit: uid, to };
    g.apply(pid, &action).ok()?;
    (g.units.get(&uid)?.pos == to).then_some(action)
}

impl AdvancedAi {
    /// Set by a host adapter for each fresh board, before any speculation.
    pub fn observe_air_assault_frame(&mut self, visible: BTreeSet<Pos>, frames_left: u32) {
        self.air_assault_observation = Some((visible, frames_left));
    }

    fn air_assault_target_visible(&self, g: &Game, pid: usize, target: Pos) -> bool {
        self.air_assault_observation.as_ref().map_or_else(
            || g.player_can_see(pid, target),
            |(visible, _)| visible.contains(&target),
        )
    }

    pub fn planned_air_city_assault(&self) -> Option<&AirCityAssault> {
        self.air_city_assault.as_ref()
    }

    /// Run after the immediate-kill prepass, before individual units spend
    /// their whole turns. Only the opt-in air surge owns this maneuver.
    pub(super) fn plan_air_city_assault(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> BTreeSet<u32> {
        self.air_city_assault = None;
        let mut reserved = BTreeSet::new();
        if !self.air_surge_enabled() || plan.strategy != GrandStrategy::Conquest {
            return reserved;
        }
        let Some(city) = plan.target_city.and_then(|id| g.cities.get(&id)) else {
            return reserved;
        };
        if city.owner == pid || !g.is_at_war(pid, city.owner) {
            return reserved;
        }
        let (cid, target) = (city.id, city.pos);
        let aircraft: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.domain.as_deref() == Some("air")
                    && spec.siege
                    && unit.hp >= 50
                    && g.wdist(unit.pos, target) <= g.unit_attack_range(*uid)
            })
            .take(4)
            .collect();
        if aircraft.is_empty() {
            return reserved;
        }
        let ready = aircraft.iter().any(|uid| {
            let unit = &g.units[uid];
            unit.moves_left > 0.0 && !unit.acted && !g.strike_blocked(*uid, target)
        });
        // After an observed volley, the aircraft may be spent but the cavalry
        // still has its capture/retreat decision. Never scout for spent planes.
        let visible = self.air_assault_target_visible(g, pid, target);
        if !ready && !visible {
            return reserved;
        }
        if ready
            && self
                .air_assault_observation
                .as_ref()
                .is_some_and(|(_, left)| *left < if visible { 1 } else { 2 })
        {
            return reserved;
        }
        let Some(mut opening) = self.air_assault_opening(g, pid, target, ready) else {
            return reserved;
        };
        let mut sorties = Vec::new();
        for uid in aircraft {
            if opening.board.strike_blocked(uid, target) {
                continue;
            }
            let value = self.air_strike_value(&opening.board, pid, uid, target, plan);
            if value <= 0.0 || !value.is_finite() {
                continue;
            }
            let action = Action::AirStrike { unit: uid, target };
            if opening.board.apply(pid, &action).is_ok() {
                opening.actions.push(action);
                sorties.push(uid);
            }
        }
        if ready && sorties.is_empty() {
            return reserved;
        }
        let uid = opening.cavalry;
        let capture = self.air_assault_capture(&opening.board, pid, uid, cid);
        if let Some(actions) = capture {
            opening.actions.extend(actions);
        } else if opening.spot != opening.origin {
            // The opening already proved this return affordable before any
            // bomber was spent. Recheck against the board after the strikes.
            let Some(action) = walk(&mut opening.board, pid, uid, opening.origin) else {
                return reserved;
            };
            opening.actions.push(action);
        } else if let Some(action) = self.air_assault_withdraw(&mut opening.board, pid, uid) {
            opening.actions.push(action);
        } else if sorties.is_empty() {
            return reserved;
        }
        let report = AirCityAssault {
            target,
            cavalry: uid,
            spot: opening.spot,
            moved_to_spot: opening.spot != opening.origin,
            aircraft: sorties,
        };
        // All steps were checked together on one branch; replay only those
        // selected steps so unrelated decisions retain the real action log.
        for action in &opening.actions {
            if g.apply(pid, action).is_err() {
                break;
            }
        }
        // Capturing in a prepass opens the city's disposition prompt. Resolve
        // it through the existing policy before the remaining units act;
        // otherwise every later attack is rejected by the pending prompt.
        self.resolve_city_dispositions(g, pid, plan.strategy);
        reserved.insert(uid);
        reserved.extend(report.aircraft.iter().copied());
        crate::think!(self.journal(), Military, Decision,
            "Cavalry coordinates a bomber city assault";
            "cavalry {}; spotting move {}; {} sorties; city captured {}",
            uid, report.moved_to_spot, report.aircraft.len(),
            g.cities.get(&cid).is_some_and(|city| city.owner == pid));
        self.air_city_assault = Some(report);
        reserved
    }

    fn air_assault_opening(
        &self,
        g: &Game,
        pid: usize,
        target: Pos,
        ready: bool,
    ) -> Option<Opening> {
        let visible = self.air_assault_target_visible(g, pid, target);
        let mut cavalry: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.cavalry
                    && spec.is_melee_capable()
                    && unit.hp >= 60
                    && unit.moves_left > 0.0
                    && !unit.acted
                    && unit.linked_to.is_none()
                    && g.wdist(unit.pos, target) <= 6
            })
            .collect();
        cavalry.sort_by_key(|uid| (g.wdist(g.units[uid].pos, target), *uid));
        let mut best: Option<Opening> = None;
        for uid in cavalry.into_iter().take(4) {
            let origin = g.units[&uid].pos;
            let mut spots = if visible {
                vec![origin]
            } else {
                g.reachable(uid)
            };
            spots.retain(|pos| {
                *pos == origin || (ready && (1..=2).contains(&g.wdist(*pos, target)))
            });
            spots.sort_by_key(|pos| (g.wdist(origin, *pos), *pos));
            for spot in spots.into_iter().take(12) {
                let mut board = g.speculative_clone();
                let mut actions = Vec::new();
                if spot != origin {
                    let Some(action) = walk(&mut board, pid, uid, spot) else {
                        continue;
                    };
                    actions.push(action);
                }
                if !board.player_can_see(pid, target) || board.units[&uid].moves_left <= 0.0 {
                    continue;
                }
                if spot != origin {
                    let mut returning = board.speculative_clone();
                    if walk(&mut returning, pid, uid, origin).is_none()
                        || battle_planner::strike_danger(&returning, pid, origin, uid)
                            >= returning.units[&uid].hp as f64 * 0.5
                    {
                        continue;
                    }
                }
                let spent = g.units[&uid].moves_left - board.units[&uid].moves_left;
                if best.as_ref().is_none_or(|old| {
                    spent.total_cmp(&old.spent).is_lt()
                        || (spent == old.spent
                            && (g.wdist(spot, target), uid, spot)
                                < (g.wdist(old.spot, target), old.cavalry, old.spot))
                }) {
                    best = Some(Opening {
                        cavalry: uid,
                        origin,
                        spot,
                        actions,
                        board,
                        spent,
                    });
                }
            }
        }
        best
    }

    fn air_assault_capture(&self, g: &Game, pid: usize, uid: u32, cid: u32) -> Option<Vec<Action>> {
        if Self::should_defer_city_capture(g, pid, cid) {
            return None;
        }
        let target = g.cities[&cid].pos;
        let mut ring = g.wdisk(target, 1);
        ring.retain(|pos| *pos != target);
        ring.sort_by_key(|pos| (g.wdist(g.units[&uid].pos, *pos), *pos));
        for stand in ring {
            let mut after = g.speculative_clone();
            let mut actions = Vec::new();
            if stand != after.units[&uid].pos {
                let Some(action) = walk(&mut after, pid, uid, stand) else {
                    continue;
                };
                actions.push(action);
            }
            let finish = if after.cities[&cid].hp <= 0 {
                Action::Move {
                    unit: uid,
                    to: target,
                }
            } else {
                Action::Attack { unit: uid, target }
            };
            if after.apply(pid, &finish).is_err()
                || after.cities.get(&cid).is_none_or(|city| city.owner != pid)
                || after.units.get(&uid).is_none_or(|unit| unit.hp < 30)
                || battle_planner::strike_danger(&after, pid, target, uid)
                    >= after.units[&uid].hp as f64 * 0.5
            {
                continue;
            }
            actions.push(finish);
            return Some(actions);
        }
        None
    }

    fn air_assault_withdraw(&self, g: &mut Game, pid: usize, uid: u32) -> Option<Action> {
        let origin = g.units[&uid].pos;
        let danger = battle_planner::strike_danger(g, pid, origin, uid);
        if danger <= 0.0 {
            return None;
        }
        let mut candidates = g.reachable(uid);
        candidates.sort_by_key(|pos| (g.wdist(origin, *pos), *pos));
        for to in candidates.into_iter().take(24) {
            let mut after = g.speculative_clone();
            let Some(action) = walk(&mut after, pid, uid, to) else {
                continue;
            };
            let risk = battle_planner::strike_danger(&after, pid, to, uid);
            if risk < danger && risk < after.units[&uid].hp as f64 * 0.5 {
                *g = after;
                return Some(action);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
