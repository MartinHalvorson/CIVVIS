//! The battle solver: a local fight, worked out to its end.
//!
//! Gene `battle-solver` (`Kind::OptIn`, off and byte-identical when off).
//! `battle_planner` plans one turn: the blows, priced against one reply. It
//! cannot see that a position it likes loses the fight two turns later, or
//! that walking away now wins it, because nothing in it plays past the
//! enemy's next move. This does, for every local fight on the board:
//!
//! - **Exact dice.** `dice` carries a blow's whole distribution through a
//!   volley, wound penalty included, so a kill is a probability and not a
//!   margin. Checked against the live host: 4,669 strikes, dealt/preview
//!   mean 0.997, range 0.80–1.20.
//! - **The fight, played out.** `model` plays a compact copy of the battle
//!   forward for several rounds, both sides on scripted turns, at a few tens
//!   of microseconds a rollout. Strengths come from the engine once per
//!   snapshot (`Game::melee_pair_bases` and its kin, `extract`) and are held
//!   to `melee_exchange_strengths` by `extract_tests`.
//! - **Plans judged by how the battle ends.** `search` improves a joint turn
//!   one unit at a time on rollouts with common dice, under each posture the
//!   objective allows, then runs the postures against each other. `Withdraw`
//!   is always in that final, so leaving is priced against fighting in the
//!   same hammers.
//! - **An objective.** `extract::choose_objective` names what the fight is
//!   for: keep this city, take this city, or destroy what is here. The
//!   forecast reports the chance of reaching it and in how many turns.
//!
//! Units a siege has reserved are left to the siege train; anything the
//! solver cannot model (ships, aircraft, embarked units) stays with the
//! planner and the ladder.

mod dice;
mod extract;
mod model;
mod search;

#[cfg(test)]
mod extract_tests;

use std::collections::BTreeSet;

use super::{AdvancedAi, StrategicPlan};
use crate::game::{Action, Game};
use crate::think;
use extract::Snapshot;
use model::{Aim, Objective, Posture, Weights};
use search::{solve, Budget, Solution};

/// The solver's search budget. Rollouts are cheap enough to afford this on
/// every fight of every frame; the live seat runs 2-3 frames a turn.
const BATTLE_SOLVER_BUDGET: Budget = Budget::DEFAULT;
/// Experiment knobs while the search is tuned on the benches: `CIVVIS_BS_*`
/// environment variables, developer builds only.
#[cfg(feature = "developer-tools")]
fn knob<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(not(feature = "developer-tools"))]
fn knob<T>(_name: &str, default: T) -> T {
    default
}

fn tuned_budget() -> Budget {
    Budget {
        rollouts: knob("CIVVIS_BS_ROLLOUTS", BATTLE_SOLVER_BUDGET.rollouts),
        horizon: knob("CIVVIS_BS_HORIZON", BATTLE_SOLVER_BUDGET.horizon),
        coarse: knob("CIVVIS_BS_COARSE", BATTLE_SOLVER_BUDGET.coarse),
        fine: knob("CIVVIS_BS_FINE", BATTLE_SOLVER_BUDGET.fine),
        max_lines: knob("CIVVIS_BS_LINES", BATTLE_SOLVER_BUDGET.max_lines),
        passes: knob("CIVVIS_BS_PASSES", BATTLE_SOLVER_BUDGET.passes),
        melee_stands: knob("CIVVIS_BS_STANDS", BATTLE_SOLVER_BUDGET.melee_stands),
    }
}

fn tuned_weights() -> Weights {
    let base = Weights::default();
    Weights {
        loss: knob("CIVVIS_BS_LOSS", base.loss),
        alive_floor: knob("CIVVIS_BS_FLOOR", base.alive_floor),
        objective_bonus: knob("CIVVIS_BS_BONUS", base.objective_bonus),
        haste: knob("CIVVIS_BS_HASTE", base.haste),
        enemy_retreat_hp: knob("CIVVIS_BS_ERET", base.enemy_retreat_hp),
        our_retreat_hp: knob("CIVVIS_BS_ORET", base.our_retreat_hp),
        enemy_aggression: knob("CIVVIS_BS_AGGRO", base.enemy_aggression),
        tempo: knob("CIVVIS_BS_TEMPO", base.tempo),
        siege_hp: knob("CIVVIS_BS_SHP", base.siege_hp),
        siege_wall: knob("CIVVIS_BS_SWALL", base.siege_wall),
    }
}

fn engage_radius() -> i32 {
    knob("CIVVIS_BS_RADIUS", extract::ENGAGE_RADIUS)
}

/// How many times one fight may be re-solved in a frame after the dice
/// surprise the plan: the rolls are real, so the rest of the turn is too.
const MAX_RESOLVES: u32 = 4;

/// What applying a plan changed.
#[derive(Default)]
struct Applied {
    struck: bool,
    surprised: bool,
    lines: u32,
    dropped: u32,
}

impl AdvancedAi {
    /// Whether another layer already owns this unit this turn.
    pub(super) fn solver_leaves_alone(&self, g: &Game, pid: usize, uid: u32) -> bool {
        self.battle_planner_ordered.contains(&uid)
            || self.guard_is_reserved_for_civilian(uid)
            || self.unit_is_reserved(uid)
            || self.active_siege_member(g, pid, uid)
            || self.staging_siege_member(g, pid, uid)
    }

    /// Solve and play every local fight. True if any unit struck.
    pub(super) fn run_battle_solver(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.battle_solver {
            return false;
        }
        let mut struck = false;
        for (index, theater) in extract::theaters(self, g, pid).into_iter().enumerate() {
            self.census.battle_solver_fights += 1;
            for attempt in 0..MAX_RESOLVES {
                let theater = extract::Theater {
                    ours: theater
                        .ours
                        .iter()
                        .copied()
                        .filter(|uid| {
                            g.units.contains_key(uid) && !self.battle_planner_ordered.contains(uid)
                        })
                        .collect(),
                    hostiles: theater
                        .hostiles
                        .iter()
                        .copied()
                        .filter(|uid| g.units.contains_key(uid))
                        .collect(),
                };
                if theater.ours.is_empty() || theater.hostiles.is_empty() {
                    break;
                }
                let Some(snap) = extract::snapshot(self, g, pid, &theater, plan) else {
                    break;
                };
                let seed = (g.turn as u64)
                    .wrapping_mul(1009)
                    .wrapping_add(index as u64 * 7919)
                    .wrapping_add(attempt as u64 * 104_729);
                let solution = solve(
                    &snap.board,
                    &snap.state,
                    snap.objective,
                    tuned_budget(),
                    tuned_weights(),
                    seed,
                );
                self.census.battle_solver_rollouts += solution.rollouts as u64;
                self.journal_solution(g, &snap, &solution);
                let applied = self.apply_solution(g, pid, &snap, &solution);
                struck |= applied.struck;
                self.census.battle_solver_lines += applied.lines;
                self.census.battle_solver_dropped += applied.dropped;
                if !applied.surprised {
                    break;
                }
                self.census.battle_solver_resolves += 1;
            }
        }
        struck
    }

    /// Play the chosen turn: shots, then blows, then everyone who only moves.
    fn apply_solution(
        &mut self,
        g: &mut Game,
        pid: usize,
        snap: &Snapshot,
        solution: &Solution,
    ) -> Applied {
        let mut applied = Applied::default();
        let blows_on = |j: usize| {
            solution
                .plan
                .iter()
                .filter(|(_, line)| line.aim == Aim::Unit(j as u16))
                .count()
        };
        let mut landed: Vec<(usize, u32)> = Vec::new();
        for pass in 0..3 {
            for (i, line) in &solution.plan {
                let uid = snap.unit_ids[*i];
                let shooter = snap.state.units[*i].shoots();
                let wanted = match pass {
                    0 => shooter && line.aim != Aim::None,
                    1 => !shooter && line.aim != Aim::None,
                    _ => line.aim == Aim::None,
                };
                if !wanted || !g.units.contains_key(&uid) {
                    continue;
                }
                if applied.surprised && pass < 2 {
                    // The rest of the blows are re-decided on the real rolls.
                    continue;
                }
                let to = snap.tiles[line.tile as usize];
                if g.units[&uid].pos != to
                    && (!self.base.path_walk_to(g, pid, uid, to)
                        || g.units.get(&uid).is_none_or(|unit| unit.pos != to))
                {
                    applied.dropped += 1;
                    continue;
                }
                let action = match line.aim {
                    Aim::None => {
                        self.base.fortify_or_stop(g, pid, uid);
                        self.battle_planner_ordered.insert(uid);
                        applied.lines += 1;
                        continue;
                    }
                    Aim::Unit(j) => {
                        let tid = snap.unit_ids[j as usize];
                        let Some(target) = g.units.get(&tid) else {
                            // Killed by an earlier blow: re-plan the rest.
                            applied.surprised = true;
                            continue;
                        };
                        let target = target.pos;
                        if shooter {
                            Action::Ranged { unit: uid, target }
                        } else {
                            Action::Attack { unit: uid, target }
                        }
                    }
                    Aim::City(c) => {
                        let target = g.cities[&snap.city_ids[c as usize]].pos;
                        if shooter {
                            Action::Ranged { unit: uid, target }
                        } else {
                            Action::Attack { unit: uid, target }
                        }
                    }
                };
                if g.apply(pid, &action).is_ok() {
                    applied.struck = true;
                    applied.lines += 1;
                    self.battle_planner_ordered.insert(uid);
                    self.force_groups_dirty = true;
                    if let Aim::Unit(j) = line.aim {
                        let tid = snap.unit_ids[j as usize];
                        let blows_landed =
                            landed.iter().filter(|(k, _)| *k == j as usize).count() + 1;
                        landed.push((j as usize, tid));
                        let dead = !g.units.contains_key(&tid);
                        let planned = blows_on(j as usize);
                        // Dead before its last planned blow, or alive after
                        // it when the plan counted on the kill.
                        let expected_kill = solution
                            .forecast
                            .kills
                            .iter()
                            .any(|(aim, p)| *aim == Aim::Unit(j) && *p >= 0.6);
                        if (dead && blows_landed < planned)
                            || (!dead && blows_landed >= planned && expected_kill)
                        {
                            applied.surprised = true;
                        }
                    }
                } else {
                    applied.dropped += 1;
                }
            }
        }
        applied
    }

    /// One "Military/Decision" line per solved fight.
    fn journal_solution(&self, g: &Game, snap: &Snapshot, solution: &Solution) {
        let posture = match solution.posture {
            Posture::Press => "press",
            Posture::Hold => "hold",
            Posture::Withdraw => "withdraw",
        };
        let forecast = &solution.forecast;
        let objective = match snap.objective {
            Objective::Field => "the field".to_string(),
            Objective::Capture(c) => {
                format!("take {}", g.cities[&snap.city_ids[c as usize]].name)
            }
            Objective::Hold(c) => {
                format!("keep {}", g.cities[&snap.city_ids[c as usize]].name)
            }
        };
        let kills: BTreeSet<String> = forecast
            .kills
            .iter()
            .filter_map(|(aim, p)| match aim {
                Aim::Unit(j) => Some(format!(
                    "{} {:.0}%",
                    g.units
                        .get(&snap.unit_ids[*j as usize])
                        .map(|u| u.kind.to_string())
                        .unwrap_or_default(),
                    p * 100.0
                )),
                _ => None,
            })
            .collect();
        think!(self.journal(), Military, Decision,
            "Battle solver: {objective}, {posture}: {} blows, kills {}",
            solution.plan.iter().filter(|(_, l)| l.aim != Aim::None).count(),
            kills.into_iter().collect::<Vec<_>>().join(", ");
            "{} rollouts over {} turns ({} samples): objective {:.0}%, lose {:.1} units, kill {:.1}, value {:+.0} ({})",
            solution.rollouts, tuned_budget().horizon, forecast.samples,
            forecast.p_objective * 100.0, forecast.own_lost, forecast.killed, forecast.value,
            solution.by_posture.iter()
                .map(|(p, v)| format!("{p:?} {v:+.0}"))
                .collect::<Vec<_>>().join(" / "));
    }
}
