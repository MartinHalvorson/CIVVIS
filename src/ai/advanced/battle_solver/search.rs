//! The search: which turn, judged by how the battle comes out.
//!
//! A turn is one line per unit (`model::Line`). Enumerating those jointly is
//! hopeless and a one-turn reading is what the planner already has, so the
//! search does what the literature in `docs/TACTICS.md` §1 says wins this
//! shape, with the piece that work never had: each candidate turn is scored by
//! **playing the battle out** for `horizon` rounds against a modelled enemy,
//! on the same dice for every candidate.
//!
//! Three layers, cheapest first:
//!
//! 1. A short list of lines per unit, ranked by a closed-form prior and by
//!    the one-turn threat map.
//! 2. For each posture the objective allows (`Press`, `Hold`, `Withdraw`), a
//!    seeded turn improved one unit at a time on `coarse` rollouts.
//! 3. A final between the postures' best turns on `fine` rollouts. Because
//!    `Withdraw` and `Hold` are in that final, walking away is always
//!    weighed against fighting, in the same units.

use super::dice::{kill_probability, mean_damage};
use super::model::{
    Aim, Board, Line, Objective, Posture, Sim, State, Weights, F_MELEE, NO_TILE, NO_UNIT, OURS,
    THEIRS,
};

/// How much search a decision gets.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub rollouts: u32,
    pub horizon: u32,
    pub coarse: u32,
    pub fine: u32,
    pub max_lines: usize,
    pub passes: u32,
    /// Stands kept per melee target when a unit's candidates are built.
    pub melee_stands: usize,
}

impl Budget {
    /// The live seat's budget: a few thousand rollouts per battle.
    pub const DEFAULT: Budget = Budget {
        rollouts: 2400,
        horizon: 2,
        coarse: 6,
        fine: 40,
        max_lines: 9,
        passes: 2,
        melee_stands: 2,
    };
}

/// What the chosen turn is expected to bring, over `samples` rollouts.
#[derive(Clone, Debug, Default)]
pub struct Forecast {
    pub samples: u32,
    /// Mean final score, in hammers.
    pub value: f32,
    /// Mean units of ours lost.
    pub own_lost: f32,
    /// Mean enemy units destroyed.
    pub killed: f32,
    /// Probability the objective is met at the horizon.
    pub p_objective: f32,
    /// Mean rounds until the objective, among rollouts that reached it.
    pub turns_to_objective: f32,
    /// Exact chance each struck target dies to this turn's planned blows.
    pub kills: Vec<(Aim, f64)>,
}

#[derive(Clone, Debug)]
pub struct Solution {
    pub plan: Vec<(usize, Line)>,
    pub posture: Posture,
    pub forecast: Forecast,
    pub rollouts: u32,
    /// The score of each posture's best turn, in search order.
    pub by_posture: Vec<(Posture, f32)>,
}

/// One unit's short list.
#[derive(Clone, Debug)]
struct Choices {
    unit: usize,
    lines: Vec<(Line, f32)>,
}

struct Searcher<'b> {
    b: &'b Board,
    init: &'b State,
    sim: Sim<'b>,
    budget: Budget,
    used: u32,
    seed: u64,
    threat: Vec<f32>,
}

impl<'b> Searcher<'b> {
    /// Each sample's score, in sample order, for a paired comparison.
    fn sample_scores(
        &mut self,
        plan: &[(usize, Line)],
        posture: Posture,
        samples: u32,
    ) -> Vec<f32> {
        let mut scores = Vec::with_capacity(samples as usize);
        for k in 0..samples {
            self.sim.reset(self.init, self.seed.wrapping_add(k as u64));
            self.sim.rollout(plan, posture, self.budget.horizon);
            scores.push(self.sim.score());
        }
        self.used += samples;
        scores
    }

    fn eval(&mut self, plan: &[(usize, Line)], posture: Posture, samples: u32) -> f32 {
        let mut total = 0.0f32;
        for k in 0..samples {
            self.sim.reset(self.init, self.seed.wrapping_add(k as u64));
            self.sim.rollout(plan, posture, self.budget.horizon);
            total += self.sim.score();
        }
        self.used += samples;
        total / samples.max(1) as f32
    }

    /// Expected blows each tile could take next turn, from every enemy's
    /// reach, read once from the model. Only ranks stands; the rollouts are
    /// what judge them.
    fn threat_map(&mut self) -> Vec<f32> {
        let b = self.b;
        let mut threat = vec![0.0f32; b.tiles];
        self.sim.reset(self.init, self.seed);
        for j in 0..self.init.units.len() {
            let unit = self.init.units[j];
            if !unit.alive() || unit.side != THEIRS {
                continue;
            }
            self.sim.s.units[j].mp = unit.max_mp;
            self.sim.flood_public(j);
            let mut blow = 0.0f32;
            for i in 0..self.init.units.len() {
                if self.init.units[i].side == OURS && self.init.units[i].alive() {
                    let (att, def) = if unit.shoots() {
                        self.sim.ranged_strengths(j, i)
                    } else {
                        self.sim.melee_strengths(j, i)
                    };
                    blow = blow.max(mean_damage(att, def));
                }
            }
            let reach = self.sim.reach_public().to_vec();
            let mut hit = vec![false; b.tiles];
            for (t, kept) in reach {
                if unit.shoots() {
                    if !unit.fires_after_moving() && t != unit.tile {
                        continue;
                    }
                    for (other, slot) in hit.iter_mut().enumerate() {
                        if b.d(t, other as u16) as i32 <= unit.range as i32 {
                            *slot = true;
                        }
                    }
                } else if kept > 0 || t == unit.tile {
                    for n in b.nbrs[t as usize] {
                        if n != NO_TILE {
                            hit[n as usize] = true;
                        }
                    }
                }
            }
            for (tile, reached) in hit.iter().enumerate() {
                if *reached {
                    threat[tile] += blow;
                }
            }
        }
        threat
    }

    fn reach_of(&mut self, i: usize) -> Vec<(u16, u16)> {
        if !self.b.reach0[i].is_empty() {
            return self.b.reach0[i].clone();
        }
        self.sim.reset(self.init, self.seed);
        self.sim.flood_public(i);
        self.sim.reach_public().to_vec()
    }

    /// One unit's candidate lines, best prior first.
    fn lines_for(&mut self, i: usize, objective: Objective) -> Choices {
        let b = self.b;
        let me = self.init.units[i];
        let reach = self.reach_of(i);
        self.sim.reset(self.init, self.seed);
        let mut lines: Vec<(Line, f32)> = Vec::new();
        let in_reach = |tile: u16| reach.iter().any(|(t, _)| *t == tile);
        let keeps = |tile: u16| reach.iter().find(|(t, _)| *t == tile).map(|(_, k)| *k);
        let threat = &self.threat;
        let stand_cost =
            |t: u16| threat[t as usize] * 0.35 * (me.value / 100.0) - b.defense[t as usize] * 0.6;

        if me.attacks > 0 {
            for j in 0..self.init.units.len() {
                let other = self.init.units[j];
                if !other.alive() || other.side == me.side {
                    continue;
                }
                if me.shoots() {
                    let (att, def) = self.sim.ranged_strengths(i, j);
                    let dealt = mean_damage(att, def);
                    let mut best: Option<(u16, f32)> = None;
                    let fixed = !me.fires_after_moving();
                    for (t, kept) in &reach {
                        // A shot needs movement left to spend on it.
                        if (fixed && *t != me.tile) || (*kept == 0 && *t != me.tile) {
                            continue;
                        }
                        if b.d(*t, other.tile) as i32 > me.range as i32
                            || !b.shot_visible(*t, other.tile)
                        {
                            continue;
                        }
                        let cost = stand_cost(*t);
                        if best.is_none_or(|(_, c)| cost < c) {
                            best = Some((*t, cost));
                        }
                    }
                    if let Some((t, cost)) = best {
                        let value = blow_prior(dealt, 0.0, other, me) - cost;
                        lines.push((
                            Line {
                                tile: t,
                                aim: Aim::Unit(j as u16),
                            },
                            value,
                        ));
                    }
                } else if me.has(F_MELEE) {
                    let (att, def) = self.sim.melee_strengths(i, j);
                    let (dealt, taken) = (mean_damage(att, def), mean_damage(def, att));
                    let mut stands: Vec<(u16, f32)> = Vec::new();
                    for t in b.nbrs[other.tile as usize] {
                        if t == NO_TILE {
                            continue;
                        }
                        let usable = t == me.tile
                            || (keeps(t).is_some_and(|k| k > 0)
                                && self.init.occ[t as usize] == NO_UNIT);
                        if usable {
                            stands.push((t, stand_cost(t)));
                        }
                    }
                    stands.sort_by(|a, b| a.1.total_cmp(&b.1));
                    for (t, cost) in stands.into_iter().take(self.budget.melee_stands) {
                        let value = blow_prior(dealt, taken, other, me) - cost;
                        lines.push((
                            Line {
                                tile: t,
                                aim: Aim::Unit(j as u16),
                            },
                            value,
                        ));
                    }
                }
            }
            for c in 0..b.cities.len() {
                if self.init.city_side[c] == me.side {
                    continue;
                }
                let tile = b.cities[c].tile;
                let pull = if matches!(objective, Objective::Capture(city) if city as usize == c) {
                    60.0
                } else {
                    15.0
                };
                if me.shoots() {
                    for (t, kept) in &reach {
                        if (!me.fires_after_moving() && *t != me.tile)
                            || (*kept == 0 && *t != me.tile)
                        {
                            continue;
                        }
                        if b.d(*t, tile) as i32 <= me.range as i32 && b.shot_visible(*t, tile) {
                            lines.push((
                                Line {
                                    tile: *t,
                                    aim: Aim::City(c as u8),
                                },
                                pull - stand_cost(*t),
                            ));
                            break;
                        }
                    }
                } else if me.has(F_MELEE) {
                    for t in b.nbrs[tile as usize] {
                        if t == NO_TILE {
                            continue;
                        }
                        let usable = t == me.tile
                            || (keeps(t).is_some_and(|k| k > 0)
                                && self.init.occ[t as usize] == NO_UNIT);
                        if usable {
                            lines.push((
                                Line {
                                    tile: t,
                                    aim: Aim::City(c as u8),
                                },
                                pull - stand_cost(t),
                            ));
                            break;
                        }
                    }
                }
            }
        }

        // Moves. The stand where it is, the safest few, the strongest
        // ground, and the steps that close on the fight.
        let (near, _) = nearest_enemy(self.init, b, me.side, me.tile);
        let mut stands: Vec<(u16, f32)> = reach
            .iter()
            .filter(|(t, _)| *t == me.tile || self.init.occ[*t as usize] == NO_UNIT)
            .map(|(t, _)| {
                let closes = if near == NO_TILE {
                    0.0
                } else {
                    -(b.d(*t, near) as f32)
                };
                let heal = b.heal[*t as usize][me.side as usize] as f32 * 0.4;
                (*t, -stand_cost(*t) + closes * 0.6 + heal)
            })
            .collect();
        stands.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut safest: Vec<(u16, f32)> = stands.clone();
        safest.sort_by(|a, b| {
            self.threat[a.0 as usize]
                .total_cmp(&self.threat[b.0 as usize])
                .then(b.1.total_cmp(&a.1))
        });
        for (t, score) in stands
            .iter()
            .take(4)
            .chain(safest.iter().take(3))
            .chain(stands.iter().filter(|(t, _)| *t == me.tile))
        {
            lines.push((
                Line {
                    tile: *t,
                    aim: Aim::None,
                },
                *score,
            ));
        }
        let _ = in_reach;
        lines.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut seen = Vec::new();
        lines.retain(|(line, _)| {
            if seen.contains(line) {
                false
            } else {
                seen.push(*line);
                true
            }
        });
        lines.truncate(self.budget.max_lines);
        Choices { unit: i, lines }
    }

    /// The seed turn for a posture: each unit's first fitting line.
    fn seed_turn(&self, choices: &[Choices], posture: Posture) -> Vec<usize> {
        choices
            .iter()
            .map(|c| {
                let me = self.init.units[c.unit];
                let found = c.lines.iter().position(|(line, _)| match posture {
                    Posture::Press => true,
                    Posture::Hold => line.tile == me.tile,
                    Posture::Withdraw => {
                        line.aim == Aim::None
                            || (me.shoots()
                                && self.threat[line.tile as usize] <= self.threat[me.tile as usize])
                    }
                });
                found.unwrap_or(0)
            })
            .collect()
    }

    fn plan_of(choices: &[Choices], picks: &[usize]) -> Vec<(usize, Line)> {
        choices
            .iter()
            .zip(picks)
            .filter_map(|(c, k)| c.lines.get(*k).map(|(line, _)| (c.unit, *line)))
            .collect()
    }

    /// Improve the turn one unit at a time, on common dice. A change is kept
    /// only when the paired difference on the same rolls clears its own
    /// noise, so the climb does not wander on a handful of samples; and the
    /// seed stands unless the finished turn beats it on the larger sample.
    fn improve(
        &mut self,
        choices: &[Choices],
        seed: Vec<usize>,
        posture: Posture,
        budget: u32,
    ) -> Vec<usize> {
        let stop_at = self.used + budget;
        let coarse = self.budget.coarse;
        let mut picks = seed.clone();
        let mut best = self.sample_scores(&Self::plan_of(choices, &picks), posture, coarse);
        for _ in 0..self.budget.passes {
            let mut improved = false;
            for u in 0..choices.len() {
                for k in 0..choices[u].lines.len() {
                    if k == picks[u] || self.used + coarse > stop_at {
                        continue;
                    }
                    let keep = picks[u];
                    picks[u] = k;
                    let scores =
                        self.sample_scores(&Self::plan_of(choices, &picks), posture, coarse);
                    if paired_gain(&scores, &best) {
                        best = scores;
                        improved = true;
                    } else {
                        picks[u] = keep;
                    }
                }
            }
            if !improved || self.used + coarse > stop_at {
                break;
            }
        }
        if picks != seed {
            let fine = self.budget.fine;
            let finished = self.sample_scores(&Self::plan_of(choices, &picks), posture, fine);
            let seeded = self.sample_scores(&Self::plan_of(choices, &seed), posture, fine);
            if mean(&finished) < mean(&seeded) {
                return seed;
            }
        }
        picks
    }

    fn forecast(&mut self, plan: &[(usize, Line)], posture: Posture, samples: u32) -> Forecast {
        let (start_own, start_enemy) = {
            self.sim.reset(self.init, self.seed);
            (self.sim.own_alive(), self.sim.enemy_alive())
        };
        let mut f = Forecast {
            samples,
            ..Forecast::default()
        };
        let (mut reached, mut reached_turns) = (0u32, 0u32);
        for k in 0..samples {
            self.sim
                .reset(self.init, self.seed.wrapping_add(10_000 + k as u64));
            self.sim.rollout(plan, posture, self.budget.horizon);
            f.value += self.sim.score();
            f.own_lost += (start_own - self.sim.own_alive()) as f32;
            f.killed += (start_enemy - self.sim.enemy_alive()) as f32;
            if self.sim.objective_met() {
                reached += 1;
                reached_turns += self.sim.reached.unwrap_or(0);
            }
        }
        self.used += samples;
        let n = samples.max(1) as f32;
        f.value /= n;
        f.own_lost /= n;
        f.killed /= n;
        f.p_objective = reached as f32 / n;
        f.turns_to_objective = if reached > 0 {
            reached_turns as f32 / reached as f32
        } else {
            0.0
        };
        // Exact volleys: each struck target's kill chance under the planned
        // blows, shots first, from the position the plan starts in.
        self.sim.reset(self.init, self.seed);
        let mut targets: Vec<u16> = plan
            .iter()
            .filter_map(|(_, line)| match line.aim {
                Aim::Unit(j) => Some(j),
                _ => None,
            })
            .collect();
        targets.sort_unstable();
        targets.dedup();
        for j in targets {
            // Shots land first, so they lead the volley.
            let mut blows: Vec<(bool, (f32, f32))> = plan
                .iter()
                .filter(|(_, line)| line.aim == Aim::Unit(j))
                .map(|(i, _)| {
                    (
                        !self.init.units[*i].shoots(),
                        self.sim.blow_bases(*i, j as usize),
                    )
                })
                .collect();
            blows.sort_by_key(|(melee, _)| *melee);
            let blows: Vec<(f32, f32)> = blows.into_iter().map(|(_, blow)| blow).collect();
            f.kills.push((
                Aim::Unit(j),
                kill_probability(self.init.units[j as usize].hp as i32, &blows),
            ));
        }
        f
    }
}

fn mean(scores: &[f32]) -> f32 {
    scores.iter().sum::<f32>() / scores.len().max(1) as f32
}

/// Whether `scores` beats `best` on the same dice by more than the noise of
/// the difference (one standard error, and at least half a hammer).
fn paired_gain(scores: &[f32], best: &[f32]) -> bool {
    let n = scores.len().min(best.len());
    if n == 0 {
        return false;
    }
    let diffs: Vec<f32> = (0..n).map(|k| scores[k] - best[k]).collect();
    let centre = mean(&diffs);
    let variance = diffs
        .iter()
        .map(|d| (d - centre) * (d - centre))
        .sum::<f32>()
        / (n.max(2) - 1) as f32;
    centre > 0.5 && centre > (variance / n as f32).sqrt()
}

fn blow_prior(dealt: f32, taken: f32, target: super::model::Unit, me: super::model::Unit) -> f32 {
    let killed = dealt >= target.hp as f32;
    let dies = taken >= me.hp as f32 && !killed;
    let mut v = (dealt.min(target.hp as f32) / 100.0) * target.value;
    if killed {
        v += 0.9 * target.value;
    }
    v -= (taken.min(me.hp as f32) / 100.0) * me.value;
    if dies {
        v -= 0.9 * me.value;
    }
    v
}

fn nearest_enemy(s: &State, b: &Board, side: u8, tile: u16) -> (u16, u8) {
    let mut best = (NO_TILE, u8::MAX);
    for other in &s.units {
        if other.alive() && other.side != side {
            let d = b.d(tile, other.tile);
            if d < best.1 {
                best = (other.tile, d);
            }
        }
    }
    best
}

/// Plan this turn for our side of the battle in `init`.
pub fn solve(
    b: &Board,
    init: &State,
    objective: Objective,
    budget: Budget,
    weights: Weights,
    seed: u64,
) -> Solution {
    let mut searcher = Searcher {
        b,
        init,
        sim: Sim::new(b, init, weights, objective),
        budget,
        used: 0,
        seed,
        threat: Vec::new(),
    };
    searcher.threat = searcher.threat_map();
    let ours: Vec<usize> = init
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.alive() && u.side == OURS && u.mp > 0)
        .map(|(i, _)| i)
        .collect();
    let choices: Vec<Choices> = ours
        .iter()
        .map(|&i| searcher.lines_for(i, objective))
        .collect();
    let postures: &[Posture] = match objective {
        Objective::Field => &[Posture::Press, Posture::Hold, Posture::Withdraw],
        Objective::Capture(_) => &[Posture::Press, Posture::Withdraw],
        Objective::Hold(_) => &[Posture::Hold, Posture::Press, Posture::Withdraw],
    };
    let share = budget.rollouts / postures.len() as u32;
    let mut finalists: Vec<(Posture, Vec<usize>)> = Vec::new();
    for &posture in postures {
        let seed_picks = searcher.seed_turn(&choices, posture);
        let picks = searcher.improve(&choices, seed_picks, posture, share);
        finalists.push((posture, picks));
    }
    let mut by_posture = Vec::new();
    let mut winner: Option<(f32, Posture, Vec<usize>)> = None;
    for (posture, picks) in finalists {
        let score = searcher.eval(&Searcher::plan_of(&choices, &picks), posture, budget.fine);
        by_posture.push((posture, score));
        if winner.as_ref().is_none_or(|(s, _, _)| score > *s) {
            winner = Some((score, posture, picks));
        }
    }
    let (_, posture, picks) = winner.expect("at least one posture");
    let plan = Searcher::plan_of(&choices, &picks);
    let forecast = searcher.forecast(&plan, posture, budget.fine);
    Solution {
        plan,
        posture,
        forecast,
        rollouts: searcher.used,
        by_posture,
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::model::{CitySpec, Unit, F_CAPTURES, F_RANGED, F_ZOC, MP};
    use super::*;

    /// A flat `w`×`h` odd-r hex board of plains, no rivers.
    pub fn plain_board(w: i32, h: i32, units: &[Unit], heal: u8) -> Board {
        let idx = |x: i32, y: i32| (y * w + x) as usize;
        let tiles = (w * h) as usize;
        let cube = |t: usize| {
            let (x, y) = (t as i32 % w, t as i32 / w);
            let q = x - (y - (y & 1)) / 2;
            (q, y, -q - y)
        };
        let mut nbrs = vec![[NO_TILE; 6]; tiles];
        for y in 0..h {
            for x in 0..w {
                let deltas: [(i32, i32); 6] = if y & 1 == 0 {
                    [(1, 0), (-1, 0), (0, -1), (-1, -1), (0, 1), (-1, 1)]
                } else {
                    [(1, 0), (-1, 0), (1, -1), (0, -1), (1, 1), (0, 1)]
                };
                for (d, (dx, dy)) in deltas.iter().enumerate() {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && nx < w && ny >= 0 && ny < h {
                        nbrs[idx(x, y)][d] = idx(nx, ny) as u16;
                    }
                }
            }
        }
        let mut dist = vec![0u8; tiles * tiles];
        for a in 0..tiles {
            for b in 0..tiles {
                let (p, q) = (cube(a), cube(b));
                dist[a * tiles + b] =
                    (((p.0 - q.0).abs() + (p.1 - q.1).abs() + (p.2 - q.2).abs()) / 2) as u8;
            }
        }
        let n = units.len();
        let mut board = Board {
            tiles,
            nbrs,
            river: vec![0; tiles],
            defense: vec![0.0; tiles],
            land: vec![true; tiles],
            dist,
            edge: vec![MP; tiles * 6],
            city_at: vec![u8::MAX; tiles],
            cities: Vec::new(),
            heal: vec![[heal, heal]; tiles],
            flank: [2.0, 2.0],
            support: [2.0, 2.0],
            n,
            melee_att: vec![0.0; n * n],
            melee_def: vec![0.0; n * n],
            ranged_att: vec![0.0; n * n],
            ranged_def: vec![0.0; n * n],
            city_melee: Vec::new(),
            city_ranged: Vec::new(),
            reach0: vec![Vec::new(); n],
            los0: Vec::new(),
            has_los: false,
        };
        board.city_melee = vec![0.0; 0];
        board.city_ranged = vec![0.0; 0];
        board
    }

    pub fn set_strengths(b: &mut Board, i: usize, j: usize, att: f32, def: f32) {
        let n = b.n;
        b.melee_att[i * n + j] = att;
        b.melee_def[i * n + j] = def;
        b.ranged_att[i * n + j] = att;
        b.ranged_def[i * n + j] = def;
    }

    fn fill_strengths(b: &mut Board, table: &[(f32, f32)]) {
        let n = b.n;
        for i in 0..n {
            for j in 0..n {
                set_strengths(b, i, j, table[i].0, table[j].1);
            }
        }
    }

    fn soldier(tile: u16, side: u8, hp: i16) -> Unit {
        Unit {
            tile,
            hp,
            mp: 2 * MP,
            max_mp: 2 * MP,
            value: 80.0,
            side,
            attacks: 1,
            max_attacks: 1,
            fort: 0,
            flags: F_MELEE | F_ZOC | F_CAPTURES,
            range: 1,
            class: 0,
            acted: false,
        }
    }

    fn archer(tile: u16, side: u8, hp: i16) -> Unit {
        Unit {
            flags: F_RANGED | F_ZOC,
            range: 2,
            ..soldier(tile, side, hp)
        }
    }

    const FAST: Budget = Budget {
        rollouts: 1200,
        horizon: 3,
        coarse: 4,
        fine: 24,
        max_lines: 8,
        passes: 2,
        melee_stands: 2,
    };

    #[test]
    fn shooters_finish_the_wounded_target_before_anything_else() {
        // Two archers, one wounded enemy two tiles off and a healthy one.
        let units = vec![
            archer(6 * 10 + 2, OURS, 100),
            archer(6 * 10 + 3, OURS, 100),
            soldier(4 * 10 + 3, THEIRS, 30),
            soldier(4 * 10 + 8, THEIRS, 100),
        ];
        let mut b = plain_board(10, 10, &units, 0);
        fill_strengths(
            &mut b,
            &[(30.0, 25.0), (30.0, 25.0), (20.0, 20.0), (20.0, 20.0)],
        );
        let s = State::new(&b, units, vec![]);
        let solution = solve(&b, &s, Objective::Field, FAST, Weights::default(), 7);
        let aimed: Vec<Aim> = solution.plan.iter().map(|(_, l)| l.aim).collect();
        assert!(aimed.contains(&Aim::Unit(2)), "{:?}", solution.plan);
        assert!(solution.forecast.killed >= 0.9, "{:?}", solution.forecast);
    }

    #[test]
    fn a_hopeless_fight_is_left_to_the_enemy() {
        // One soldier beside three far stronger ones: every turn it stays it
        // dies; a retreat keeps it alive if it can outrun them.
        let units = vec![
            soldier(5 * 12 + 6, OURS, 100),
            soldier(5 * 12 + 8, THEIRS, 100),
            soldier(4 * 12 + 8, THEIRS, 100),
            soldier(6 * 12 + 8, THEIRS, 100),
        ];
        let mut b = plain_board(12, 12, &units, 10);
        fill_strengths(
            &mut b,
            &[(20.0, 20.0), (45.0, 40.0), (45.0, 40.0), (45.0, 40.0)],
        );
        let s = State::new(&b, units, vec![]);
        let solution = solve(&b, &s, Objective::Field, FAST, Weights::default(), 11);
        let line = solution.plan[0].1;
        assert!(
            b.d(line.tile, 5 * 12 + 8) > 2,
            "it should get out of reach: {:?}",
            solution.plan
        );
        assert_eq!(line.aim, Aim::None, "{:?}", solution.plan);
    }

    #[test]
    fn shooters_never_step_beside_melee() {
        // Archers facing melee at range: the turn shoots, it does not walk in.
        let units = vec![
            archer(7 * 12 + 5, OURS, 100),
            archer(7 * 12 + 6, OURS, 100),
            soldier(3 * 12 + 5, THEIRS, 100),
        ];
        let mut b = plain_board(12, 12, &units, 0);
        fill_strengths(&mut b, &[(32.0, 24.0), (32.0, 24.0), (28.0, 28.0)]);
        // Move the enemy into shot range: two rows up from the archers' row.
        let mut units = units;
        units[2].tile = 5 * 12 + 5;
        let s = State::new(&b, units, vec![]);
        let solution = solve(&b, &s, Objective::Field, FAST, Weights::default(), 5);
        for (_, line) in &solution.plan {
            assert!(
                b.d(line.tile, 5 * 12 + 5) >= 2,
                "a shooter must not step beside melee: {:?}",
                solution.plan
            );
        }
    }

    #[test]
    fn a_city_with_no_hit_points_is_walked_into() {
        let mut units = vec![soldier(4 * 12 + 5, OURS, 100)];
        units.push(soldier(9 * 12 + 2, THEIRS, 100));
        let mut b = plain_board(12, 12, &units, 0);
        fill_strengths(&mut b, &[(30.0, 30.0), (20.0, 20.0)]);
        b.cities = vec![CitySpec {
            tile: 4 * 12 + 6,
            strength: 30.0,
            wall_max: 0,
            strike: 0.0,
            value: 400.0,
            zoc: true,
        }];
        b.city_at[4 * 12 + 6] = 0;
        b.city_melee = vec![30.0, 20.0];
        b.city_ranged = vec![0.0, 0.0];
        let s = State::new(&b, units, vec![(0, 0, THEIRS)]);
        let solution = solve(&b, &s, Objective::Capture(0), FAST, Weights::default(), 3);
        assert_eq!(solution.plan[0].1.aim, Aim::City(0), "{:?}", solution.plan);
        assert!(
            solution.forecast.p_objective > 0.9,
            "{:?}",
            solution.forecast
        );
        assert_eq!(solution.forecast.turns_to_objective, 0.0);
    }

    #[test]
    fn it_is_fast_enough_to_look_turns_ahead() {
        let mut units = Vec::new();
        for k in 0..8u16 {
            units.push(if k % 3 == 0 {
                archer(9 * 16 + 3 + k, OURS, 100)
            } else {
                soldier(8 * 16 + 3 + k, OURS, 100)
            });
            units.push(if k % 3 == 0 {
                archer(5 * 16 + 3 + k, THEIRS, 100)
            } else {
                soldier(6 * 16 + 3 + k, THEIRS, 100)
            });
        }
        let mut b = plain_board(16, 16, &units, 10);
        let table: Vec<(f32, f32)> = units.iter().map(|_| (30.0, 28.0)).collect();
        fill_strengths(&mut b, &table);
        let s = State::new(&b, units, vec![]);
        let started = std::time::Instant::now();
        let solution = solve(
            &b,
            &s,
            Objective::Field,
            Budget::DEFAULT,
            Weights::default(),
            1,
        );
        let elapsed = started.elapsed().as_secs_f64();
        let per = elapsed / solution.rollouts as f64 * 1e6;
        println!(
            "8v8 solve: {} rollouts, {:.0} ms, {:.0} µs per rollout, posture {:?}",
            solution.rollouts,
            elapsed * 1e3,
            per,
            solution.posture
        );
        assert!(solution.rollouts > 200);
    }
}
