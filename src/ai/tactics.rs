//! Joint multi-unit tactical planning.
//!
//! The shipped tactical layer is a strong *single-unit* evaluator wired into a
//! weak *multi-unit* commitment rule. `advanced_military_step_with_decline`
//! scores every attack a unit can make on an exact cloned forward model and
//! extends the line with a quiescence-style reply search — that part is close
//! to the published state of the art for one piece. But the turn as a whole is
//! assembled by walking the units in a fixed class order (ranged, then siege,
//! then melee) and letting each one commit greedily and irreversibly before the
//! next is considered.
//!
//! That commitment rule costs four things, and all four are structural rather
//! than a matter of tuning:
//!
//! 1. **The reply term is biased by position in the order.** Each unit prices
//!    the enemy's answer against a board where its own teammates have not moved
//!    yet. Ranged units act first and so over-price their exposure (the screen
//!    that will stand in front of them does not exist yet); melee acts last and
//!    under-prices it. The bias runs in exactly the wrong direction.
//! 2. **No joint target assignment.** With four attackers and three defenders,
//!    picking each attacker's individually-best target is not the best set of
//!    attacks. The shipped `focus_fire` bonus is a flat nudge toward one shared
//!    tile, which trades spread-fire for overkill rather than solving it.
//! 3. **The order itself is never questioned.** Softening before capturing is
//!    usually right, which is why the fixed order works as well as it does, but
//!    it is wrong whenever a melee kill has to clear a tile or a firing lane
//!    first.
//! 4. **Movement is blind to the attacks it enables.** Position is chosen by
//!    depth-to-target, adjacent support and incoming threat; nothing scores a
//!    tile for the shot it opens. This is the largest of the four by a wide
//!    margin, and reaching it needed one extra evaluation term — see
//!    [`disturbance`].
//!
//! ## What this module does instead
//!
//! It plans the whole engagement at once, following the line of work that owns
//! this exact problem shape — games where one turn is a *set* of unit actions,
//! so the per-turn branching factor is the product of the per-unit ones and
//! ordinary tree search never gets past the root:
//!
//! - Churchill & Buro, *Portfolio Greedy Search and Simulation for Large-Scale
//!   Combat in StarCraft* (CIG 2013) — search the space of script assignments
//!   to units instead of the space of actions, and hill-climb one unit at a
//!   time over the joint assignment.
//! - Justesen, Mahlmann & Togelius, *Online Evolution for Multi-Action
//!   Adversarial Games* (EvoApplications 2016) — evolve the whole turn as a
//!   genome, evaluate it with a state evaluation function. Measured on Hero
//!   Academy, a turn-based multi-action tactics game with the same structure as
//!   a Civ turn, this beat both MCTS and greedy search by a wide margin.
//! - Wang et al., *Portfolio Online Evolution in StarCraft* (AIIDE 2016) — the
//!   combination, which is what this implements.
//!
//! The adaptation to CIVVIS is:
//!
//! - **Portfolio.** Each engaged unit gets a short list of candidate *lines* —
//!   an attack from where it stands, a step onto an adjacent tile followed by
//!   the attack that step opens, or the empty line that declines. Lines are
//!   generated geometrically (no clones) and pruned to the best few by a
//!   closed-form damage prior.
//! - **Genome.** A choice of line per unit, plus a permutation giving the order
//!   they are played in. Evolving the order is what fixes defect 3; the choice
//!   vector is what fixes defects 2 and 4.
//! - **Fitness.** Clone once, play the whole turn, and score the *resulting
//!   position*: material swing over the entire board via [`position_delta`],
//!   minus the enemy's best answer to the finished position via
//!   [`reply_estimate`]. Pricing the reply once against the completed turn —
//!   rather than once per unit against a half-finished one — is the direct
//!   repair for defect 1. Only the *change* in the enemy's answer is charged:
//!   an army that is going to be struck anyway does not avoid it by standing
//!   still, and charging the absolute answer values paralysis. A plan is also
//!   charged the shipped `attack_threshold` for each attack it takes, so the
//!   treatment changes *which* attacks are made rather than how cheaply the
//!   agent is willing to be hit, and [`FORTIFICATION_FORFEIT`] for each unit it
//!   moved.
//! - **Seeding.** The population is built by sequential greedy — the shipped
//!   construction — restarted from several orders, which is Portfolio Greedy
//!   Search's own remedy for the order-dependence it inherits.
//!
//! ## Approach moves, and the one term that made them work
//!
//! Letting a unit step onto a tile and then take the attack that step opens is
//! where most of the value is — and it is *actively harmful* until the
//! evaluation can price what the step gave up. Measured on `battle_bench`,
//! paired material swing per scenario against the stock agent:
//!
//! | portfolio | melee-only | combined arms |
//! |---|---|---|
//! | attacks only, no stepping | +5.6 | +28.2 |
//! | stepping, no forfeit term | **−228.9** | +178.8 |
//! | stepping, only if the step does not thin the line | −44.7 | +119.0 |
//! | **stepping + [`disturbance`] (shipped)** | **+7.5** | **+241.3** |
//!
//! The obvious diagnosis for that −228.9 is that the unit broke formation and
//! the whole enemy front answered it. **That diagnosis is wrong**, and it cost
//! two attempts to find out: an explicit adjacent-friendly-support term was
//! built and swept and could not be distinguished from zero at any weight, with
//! combined arms in fact *highest* with it off. What the evaluation was
//! actually missing is that a unit which holds its ground can dig in and one
//! that stepped cannot — a *future* action, invisible to every term that prices
//! the position as it stands. Pricing it is one subtraction, and it moves
//! combined arms from +28.2 to +241.3.
//!
//! ## Cost
//!
//! One `Game` clone per fitness evaluation, `population * generations` of them
//! per engagement, plus the sequential seeds. The enemy reply is closed-form
//! arithmetic over strengths rather than a cloned search, which is what keeps
//! this affordable: the shipped path already spends two clones per candidate
//! action per unit plus a nested cloned reply search inside each.
//! `docs/TACTICS.md` carries the measured numbers.

use std::collections::BTreeMap;

use crate::ai::{BasicAi, Weights};
use crate::game::{effective_strength, Action, Game};
use crate::rng::Rng;
use crate::Pos;

/// A unit's candidate action sequence for this turn: a single attack, or one
/// step onto a tile from which an attack becomes available followed by that
/// attack, or the empty line that declines to fight.
#[derive(Clone, Debug)]
struct Line {
    actions: Vec<Action>,
    /// Closed-form estimate used to prune the portfolio and to build the
    /// greedy incumbent. Never used as fitness — that is always the exact
    /// forward model.
    prior: f64,
    /// What the shipped agent charges this attack before it will take it:
    /// `attack_threshold` for the unit and target, plus the extra margin a
    /// wounded unit has to clear.
    ///
    /// Carrying this into the joint evaluation is what keeps the treatment
    /// honest. Without it the search is not "the same agent choosing a better
    /// set of attacks", it is also a much more aggressive agent — and measured
    /// that way it wins with ranged support and loses badly without it,
    /// because the floor is exactly what stops a melee line from trading
    /// itself down. The search should change *which* attacks are made, not how
    /// cheaply the agent is willing to be hit.
    toll: f64,
}

/// The lines available to one engaged unit.
#[derive(Clone, Debug)]
struct Portfolio {
    unit: u32,
    /// The shipped class ordering (ranged 0, siege 1, melee 2), used to seed
    /// the incumbent permutation.
    class_order: u8,
    lines: Vec<Line>,
}

/// One candidate turn: which line each unit plays, and in what order.
#[derive(Clone, Debug)]
struct Genome {
    choice: Vec<usize>,
    order: Vec<usize>,
}

/// Search budget.
///
/// **This is binding, not a formality.** Unlike the macro search — where
/// doubling the budget is the one reproducible win and quadrupling does
/// nothing — combat quality here keeps climbing with budget across the whole
/// range measured, 700 scenarios a cell on combined arms:
///
/// | pop / gen / lines | combined arms | melee | seated cost |
/// |---|---|---|---|
/// | 12 / 6 / 6 | +236.9 | +6.2 | 1.13x |
/// | **20 / 10 / 10 (shipped)** | **+279.1** | **+17.6** | **1.29x** |
/// | 32 / 20 / 16 | +300.0 | +17.6 | 2.19x |
///
/// 20/10/10 is chosen as the knee: it takes most of the available gain at a
/// cost still far under the 6.4x a searching macro seat costs. Raising it
/// further is a live option if the compute is ever worth spending.
#[derive(Clone, Copy, Debug)]
pub(crate) struct JointTactics {
    pub population: usize,
    pub generations: usize,
    /// Units in one engagement. Beyond this the least-engaged units fall
    /// through to the ordinary per-unit path.
    pub max_units: usize,
    /// Lines kept per unit, including the declining line.
    pub max_lines: usize,
}

impl Default for JointTactics {
    fn default() -> Self {
        JointTactics {
            population: 20,
            generations: 10,
            max_units: 8,
            max_lines: 10,
        }
    }
}

/// The result of planning one engagement.
pub(crate) struct TacticalPlan {
    /// Exactly the actions that succeeded during the winning evaluation, in
    /// the order they succeeded. Replaying them onto the position the search
    /// started from reproduces that evaluation exactly, including the seeded
    /// combat rolls.
    pub actions: Vec<Action>,
    /// Every unit the search reached a decision for, including the ones it
    /// decided should not attack. The caller suppresses its own greedy attack
    /// selection for these so a declined attack is not immediately re-taken.
    pub resolved: Vec<u32>,
    /// Fitness of the played plan and of the greedy incumbent, for reporting.
    pub score: f64,
    pub greedy_score: f64,
}

impl JointTactics {
    /// Plan the engagement for `pid`, or return `None` when there is no
    /// multi-unit engagement to plan. A single attacker has no joint problem
    /// to solve and is left to the cheaper per-unit path.
    pub(crate) fn plan(&self, g: &Game, pid: usize, base: &BasicAi) -> Option<TacticalPlan> {
        let w = &base.w;
        let portfolios = self.portfolios(g, pid, base);
        if portfolios.len() < 2 {
            return None;
        }

        let n = portfolios.len();
        // Deterministic: the same position always searches the same way, so a
        // game replayed from a seed is bit-identical.
        let mut rng = Rng::new(
            g.seed
                ^ (g.turn as u64).wrapping_mul(0x9E3779B97F4A7C15)
                ^ (pid as u64).wrapping_mul(0xBF58476D1CE4E5B9)
                ^ (n as u64),
        );

        // What the enemy could already do to this position before anybody
        // moves. Only the *change* a plan makes to that is chargeable: if the
        // enemy is going to strike an army anyway, standing still does not
        // avoid it, and pricing the absolute answer instead of the marginal
        // one values paralysis.
        let baseline_reply = reply_estimate(g, pid);

        // Seed with sequential greedy, which is the shipped behaviour: units
        // act in class order and each one chooses knowing what its teammates
        // have already done. Seeding a *static* assignment instead — every
        // unit choosing against the untouched board — measured much worse than
        // shipped, and for a specific reason: it spreads damage where the
        // sequential rule concentrates it. Over 400 melee scenarios that cost
        // 700 kills on identical total damage. The incumbent has to be the
        // real incumbent or the search starts behind and a small budget cannot
        // climb back.
        //
        // Restarting the same construction from several orders is Portfolio
        // Greedy Search's own remedy for the order-dependence it inherits.
        let mut population: Vec<Genome> = Vec::new();
        let identity: Vec<usize> = (0..n).collect();
        let mut reversed = identity.clone();
        reversed.reverse();
        for order in [identity, reversed] {
            population.push(Self::sequential_seed(
                g,
                pid,
                w,
                &portfolios,
                order,
                baseline_reply,
            ));
        }
        let incumbent = population[0].clone();
        for _ in 0..2 {
            let mut order: Vec<usize> = (0..n).collect();
            for index in (1..order.len()).rev() {
                order.swap(index, rng.below(index + 1));
            }
            population.push(Self::sequential_seed(
                g,
                pid,
                w,
                &portfolios,
                order,
                baseline_reply,
            ));
        }
        while population.len() < self.population.max(2) {
            population.push(Self::random_genome(&mut rng, &portfolios));
        }

        let mut best: Option<(f64, Vec<Action>)> = None;
        let mut greedy_score = f64::NEG_INFINITY;
        let mut scored: Vec<(f64, Genome, Vec<Action>)> = Vec::with_capacity(population.len());

        for generation in 0..self.generations.max(1) {
            scored.clear();
            for genome in &population {
                let (score, played) = self.fitness(g, pid, w, genome, &portfolios, baseline_reply);
                if generation == 0 && genome.choice == incumbent.choice && genome.order == incumbent.order
                {
                    greedy_score = greedy_score.max(score);
                }
                scored.push((score, genome.clone(), played));
            }
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            if best.as_ref().is_none_or(|(top, _)| scored[0].0 > *top) {
                best = Some((scored[0].0, scored[0].2.clone()));
            }
            if generation + 1 == self.generations.max(1) {
                break;
            }

            // Elitist truncation selection, then uniform crossover over the
            // choice vector and an order-preserving swap on the permutation.
            let survivors = (scored.len() / 2).max(1);
            population.clear();
            for (_, genome, _) in scored.iter().take(survivors) {
                population.push(genome.clone());
            }
            while population.len() < self.population.max(2) {
                let a = &scored[rng.below(survivors)].1;
                let b = &scored[rng.below(survivors)].1;
                let mut child = Self::crossover(&mut rng, a, b);
                Self::mutate(&mut rng, &mut child, &portfolios);
                population.push(child);
            }
        }

        let (score, actions) = best?;
        // A plan that does nothing is not a plan; let the ordinary path run so
        // this never suppresses behaviour it did not replace.
        if actions.is_empty() {
            return None;
        }
        Some(TacticalPlan {
            actions,
            resolved: portfolios.iter().map(|p| p.unit).collect(),
            score,
            greedy_score,
        })
    }

    // ---------------------------------------------------------------- portfolio

    /// Build the candidate lines for every unit that can bring an attack to
    /// bear this turn, either from where it stands or after one step.
    fn portfolios(&self, g: &Game, pid: usize, base: &BasicAi) -> Vec<Portfolio> {
        let w = &base.w;
        let mut built: Vec<Portfolio> = Vec::new();
        for uid in g.player_unit_ids(pid) {
            let Some(unit) = g.units.get(&uid) else {
                continue;
            };
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military" || spec.domain.as_deref() == Some("air") {
                continue;
            }
            if unit.attacks_left <= 0 || unit.moves_left <= 0.0 {
                continue;
            }
            // Escorts stay with what they are escorting; the settler logic owns
            // them and a joint plan must not walk them off.
            if unit
                .linked_to
                .and_then(|peer| g.units.get(&peer))
                .is_some_and(|peer| g.rules.units[peer.kind].class != "military")
            {
                continue;
            }

            let range = if spec.has_ranged_attack() {
                g.unit_attack_range(uid).max(1)
            } else {
                1
            };
            let mut lines: Vec<Line> = Vec::new();
            // The same extra bar `advanced_military_step_with_decline` puts in
            // front of a wounded unit before it will spend itself.
            let wounded_margin = if unit.hp < 55 { 12.0 } else { 0.0 };

            // Attacks available from where the unit already stands.
            for (target, action) in Self::strikes_from(g, pid, uid, unit.pos, range) {
                let ranged = matches!(action, Action::Ranged { .. });
                lines.push(Line {
                    prior: Self::strike_prior(g, pid, uid, target, ranged, w),
                    toll: base.attack_threshold(g, uid, target) + wounded_margin,
                    actions: vec![action],
                });
            }

            // A step onto an adjacent tile, then the attack that step opens.
            // Gated: see "Approach moves" in the module docs.
            if unit.moves_left >= 1.0 {
                for to in g.nbrs(unit.pos) {
                    if !g.can_move(uid, to) {
                        continue;
                    }
                    for (target, action) in Self::strikes_from(g, pid, uid, to, range) {
                        let ranged = matches!(action, Action::Ranged { .. });
                        let prior = Self::strike_prior(g, pid, uid, target, ranged, w) - 4.0;
                        lines.push(Line {
                            prior,
                            toll: base.attack_threshold(g, uid, target) + wounded_margin,
                            actions: vec![Action::Move { unit: uid, to }, action],
                        });
                    }
                }
            }

            if lines.is_empty() {
                continue;
            }
            lines.sort_by(|a, b| {
                b.prior
                    .partial_cmp(&a.prior)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            lines.truncate(self.max_lines.max(2).saturating_sub(1));
            // Declining is always available, and is what lets the search
            // discover that a unit is worth more as a screen than as a trade.
            lines.push(Line {
                actions: Vec::new(),
                prior: 0.0,
                toll: 0.0,
            });

            let class_order = if spec.has_ranged_attack() && !spec.siege {
                0
            } else if spec.siege {
                1
            } else {
                2
            };
            built.push(Portfolio {
                unit: uid,
                class_order,
                lines,
            });
        }

        // Keep the most engaged units when an army is larger than the budget.
        if built.len() > self.max_units {
            built.sort_by(|a, b| {
                let key = |p: &Portfolio| {
                    p.lines
                        .iter()
                        .map(|line| line.prior)
                        .fold(f64::NEG_INFINITY, f64::max)
                };
                key(b)
                    .partial_cmp(&key(a))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            built.truncate(self.max_units);
        }
        built.sort_by_key(|p| (p.class_order, p.unit));
        built
    }

    /// Every attack `uid` could legally aim at from `from`, judged
    /// geometrically. Illegal lines are rejected by the engine when the plan is
    /// applied, so this only has to be a superset — which is what lets
    /// candidate generation cost no clones.
    ///
    /// Tiles holding only civilians are deliberately excluded: the engine
    /// rejects `Attack` on them, and capturing them is movement, owned by
    /// `capture_adjacent_civilian` together with the guard that refuses a
    /// Settler this empire cannot use.
    fn strikes_from(g: &Game, pid: usize, uid: u32, from: Pos, range: i32) -> Vec<(Pos, Action)> {
        let spec = &g.rules.units[g.units[&uid].kind];
        let mut out = Vec::new();
        for target in g.wdisk(from, range) {
            if target == from || g.map.get(target).is_none() {
                continue;
            }
            let hostile_city = g
                .city_at(target)
                .or_else(|| g.encampment_at(target))
                .is_some_and(|cid| {
                    let city = &g.cities[&cid];
                    city.owner != pid && g.is_at_war(pid, city.owner)
                });
            let hostile_unit = g.units_at(target).into_iter().any(|oid| {
                let other = &g.units[&oid];
                other.owner != pid
                    && g.is_at_war(pid, other.owner)
                    && g.rules.units[other.kind].class == "military"
            });
            if !hostile_city && !hostile_unit {
                continue;
            }
            let distance = g.wdist(from, target);
            if spec.has_ranged_attack() && distance <= range {
                out.push((
                    target,
                    Action::Ranged {
                        unit: uid,
                        target,
                    },
                ));
            }
            if spec.is_melee_capable() && distance == 1 {
                out.push((
                    target,
                    Action::Attack {
                        unit: uid,
                        target,
                    },
                ));
            }
        }
        out
    }

    /// Closed-form value of one strike, in the same units as
    /// [`position_delta`]. Used only to rank and prune candidate lines.
    fn strike_prior(g: &Game, pid: usize, uid: u32, target: Pos, ranged: bool, w: &Weights) -> f64 {
        let attacker = &g.units[&uid];
        let attack = if ranged {
            g.unit_ranged_attack_strength(attacker)
        } else {
            g.unit_strength(attacker, false)
        };
        let attack = effective_strength(attack, attacker.hp);

        if let Some(cid) = g.city_at(target).or_else(|| g.encampment_at(target)) {
            let city = &g.cities[&cid];
            if city.owner != pid && g.is_at_war(pid, city.owner) {
                let mut value = 20.0 + 0.5 * (100 - city.hp) as f64;
                if !ranged && city.hp <= 40 && city.wall_hp <= 0 {
                    value += 520.0;
                }
                return value;
            }
        }

        let defender = g
            .units_at(target)
            .into_iter()
            .filter(|oid| {
                let other = &g.units[oid];
                other.owner != pid && g.rules.units[other.kind].class == "military"
            })
            .max_by(|a, b| {
                let strength = |id: &u32| {
                    let unit = &g.units[id];
                    effective_strength(g.unit_strength(unit, true), unit.hp)
                };
                strength(a)
                    .partial_cmp(&strength(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        let Some(defender) = defender.map(|id| &g.units[&id]) else {
            return 0.0;
        };
        let defence = effective_strength(g.unit_strength(defender, true), defender.hp);
        let spec = &g.rules.units[defender.kind];
        let dealt = expected_damage(attack, defence);
        let mut value = dealt.min(defender.hp as f64) * (1.0 + defence / 100.0);
        if dealt >= defender.hp as f64 {
            value += 190.0 + spec.cost * 0.45 + defence * 1.8;
        }
        if !ranged {
            let their_attack = effective_strength(g.unit_strength(defender, false), defender.hp);
            let my_defence = effective_strength(g.unit_strength(attacker, true), attacker.hp);
            let received = expected_damage(their_attack, my_defence);
            value -= w.trade_caution * received.min(attacker.hp as f64);
        }
        value
    }

    // ------------------------------------------------------------------ genomes

    /// Build a plan the way the shipped agent does: walk the units in `order`
    /// and let each one commit to the line that is best *given everything
    /// already played*, on the exact forward model.
    ///
    /// This is both the incumbent the search must not fall below and, run from
    /// several different orders, the strongest cheap member of the starting
    /// population. It is Portfolio Greedy Search's construction step.
    fn sequential_seed(
        g: &Game,
        pid: usize,
        w: &Weights,
        portfolios: &[Portfolio],
        order: Vec<usize>,
        baseline_reply: f64,
    ) -> Genome {
        let mut sim = g.clone();
        sim.set_fog_memory(false);
        let mut choice = vec![0usize; portfolios.len()];
        for &index in &order {
            let portfolio = &portfolios[index];
            let mut best: Option<(f64, usize, Game)> = None;
            for (slot, line) in portfolio.lines.iter().enumerate() {
                let mut branch = sim.clone();
                let mut struck = false;
                for action in &line.actions {
                    if branch.apply(pid, action).is_err() {
                        break;
                    }
                    struck |= !matches!(action, Action::Move { .. });
                }
                // Scored against the turn's starting position, so the running
                // plan is judged as a whole rather than one blow at a time.
                let mut score = position_delta(g, &branch, pid)
                    - w.trade_caution * (reply_estimate(&branch, pid) - baseline_reply)
                    - FORTIFICATION_FORFEIT * disturbance(g, &branch, pid);
                if struck {
                    score -= line.toll;
                }
                if best.as_ref().is_none_or(|(top, _, _)| score > *top) {
                    best = Some((score, slot, branch));
                }
            }
            if let Some((_, slot, branch)) = best {
                choice[index] = slot;
                sim = branch;
            }
        }
        Genome { choice, order }
    }

    fn random_genome(rng: &mut Rng, portfolios: &[Portfolio]) -> Genome {
        let choice = portfolios
            .iter()
            .map(|p| rng.below(p.lines.len()))
            .collect();
        let mut order: Vec<usize> = (0..portfolios.len()).collect();
        for index in (1..order.len()).rev() {
            order.swap(index, rng.below(index + 1));
        }
        Genome { choice, order }
    }

    fn crossover(rng: &mut Rng, a: &Genome, b: &Genome) -> Genome {
        let choice = a
            .choice
            .iter()
            .zip(&b.choice)
            .map(|(x, y)| if rng.chance(0.5) { *x } else { *y })
            .collect();
        // Orders are permutations, so they are inherited whole rather than
        // spliced; mutation is what explores the ordering.
        let order = if rng.chance(0.5) {
            a.order.clone()
        } else {
            b.order.clone()
        };
        Genome { choice, order }
    }

    fn mutate(rng: &mut Rng, genome: &mut Genome, portfolios: &[Portfolio]) {
        if !genome.choice.is_empty() {
            let index = rng.below(genome.choice.len());
            genome.choice[index] = rng.below(portfolios[index].lines.len());
            if rng.chance(0.35) {
                let other = rng.below(genome.choice.len());
                genome.choice[other] = rng.below(portfolios[other].lines.len());
            }
        }
        if genome.order.len() > 1 && rng.chance(0.5) {
            let i = rng.below(genome.order.len());
            let j = rng.below(genome.order.len());
            genome.order.swap(i, j);
        }
    }

    // ------------------------------------------------------------------ fitness

    /// Play a whole candidate turn on one private clone and score the position
    /// it produces. Returns the actions that actually succeeded, so the winner
    /// can be replayed onto the real game and land on the same result.
    fn fitness(
        &self,
        g: &Game,
        pid: usize,
        w: &Weights,
        genome: &Genome,
        portfolios: &[Portfolio],
        baseline_reply: f64,
    ) -> (f64, Vec<Action>) {
        let mut sim = g.clone();
        // Nothing in a search branch reads a remembered fogged map.
        sim.set_fog_memory(false);
        let mut played = Vec::new();
        let mut tolls = 0.0;
        for &index in &genome.order {
            let Some(portfolio) = portfolios.get(index) else {
                continue;
            };
            let Some(line) = portfolio.lines.get(genome.choice[index]) else {
                continue;
            };
            let mut struck = false;
            for action in &line.actions {
                if sim.apply(pid, action).is_err() {
                    // A step that fails invalidates the attack it was opening.
                    break;
                }
                struck |= !matches!(action, Action::Move { .. });
                played.push(action.clone());
            }
            // Only an attack that actually landed pays the agent's price for
            // taking it. A line whose approach failed is a move, not a trade.
            if struck {
                tolls += line.toll;
            }
        }
        let score = position_delta(g, &sim, pid)
            - w.trade_caution * (reply_estimate(&sim, pid) - baseline_reply)
            - tolls
            - FORTIFICATION_FORFEIT * disturbance(g, &sim, pid);
        (score, played)
    }
}

/// Expected damage of one blow, the engine's roll with its uniform factor
/// taken at its mean. `Game::damage` is `30 * exp((att - def) / 25) * U(0.8,
/// 1.2)`, clamped into `1..=100`.
/// What each unit that stepped forward gave up by stepping.
///
/// **This one term is what makes approach moves shippable**, and finding it
/// took three attempts because the obvious hypothesis was wrong twice. A melee
/// unit that leaves the line to land a blow was measured at −228.9 against the
/// stock agent, and the natural explanation — it broke formation, so the whole
/// enemy front answers it — is **not** the reason. An explicit
/// adjacent-friendly-support term was built and swept and could not be
/// distinguished from zero at any weight; combined arms was in fact *highest*
/// with it switched off entirely.
///
/// What actually matters is far simpler: a unit that holds its ground can dig
/// in, and one that stepped cannot. In Civ that bonus is not small, and nothing
/// else in this evaluation could see it, because fortification is a *future*
/// action and every other term prices the position as it stands.
///
/// Priced per unit that moved, in the same units as [`position_delta`]. At
/// [`FORTIFICATION_FORFEIT`] it is roughly a seventh of a unit's life, and the
/// result is flat across 20..50 — this is a plateau, not a knife edge.
fn disturbance(before: &Game, after: &Game, pid: usize) -> f64 {
    let mut moved = 0.0;
    for (id, unit) in &before.units {
        if unit.owner != pid || before.rules.units[unit.kind].class != "military" {
            continue;
        }
        if after.units.get(id).is_some_and(|now| now.pos != unit.pos) {
            moved += 1.0;
        }
    }
    moved
}

/// Price of giving up the chance to fortify, per unit that stepped. Swept on
/// `battle_bench` over four army compositions and disjoint seed blocks; see
/// `docs/TACTICS.md`.
const FORTIFICATION_FORFEIT: f64 = 40.0;

fn expected_damage(attack: f64, defence: f64) -> f64 {
    (30.0 * ((attack - defence) / 25.0).exp()).clamp(1.0, 100.0)
}

/// Material swing between two positions from `pid`'s point of view.
///
/// The constants are deliberately the ones `tactical_attack_value` already
/// uses, so a joint plan and a single-unit attack are priced on one scale and
/// the tuning that exists for the latter carries over.
fn position_delta(before: &Game, after: &Game, pid: usize) -> f64 {
    let mut value = 0.0;

    for (id, unit) in &before.units {
        let spec = &before.rules.units[unit.kind];
        if spec.class != "military" {
            continue;
        }
        if unit.owner == pid {
            let cost = spec.cost;
            match after.units.get(id) {
                None => value -= 230.0 + cost * 0.65,
                Some(survivor) => {
                    value -= (unit.hp - survivor.hp).max(0) as f64 * (1.25 + cost / 800.0)
                }
            }
            continue;
        }
        if !before.is_at_war(pid, unit.owner) {
            continue;
        }
        let strength = before.unit_strength(unit, true);
        match after.units.get(id) {
            None => {
                value += 190.0
                    + spec.cost * 0.45
                    + strength * 1.8
                    + if spec.siege { 65.0 } else { 0.0 }
                    + if spec.is_melee_capable() { 30.0 } else { 0.0 }
            }
            Some(survivor) => {
                value += (unit.hp - survivor.hp).max(0) as f64 * (1.0 + strength / 100.0)
                    + if spec.siege { 18.0 } else { 0.0 }
                    + if spec.is_melee_capable() { 6.0 } else { 0.0 }
            }
        }
    }

    for (id, city) in &before.cities {
        let Some(now) = after.cities.get(id) else {
            continue;
        };
        if city.owner == pid {
            if now.owner != pid {
                value -= 520.0 + city.pop.max(1) as f64 * 14.0;
                continue;
            }
            value -= (city.hp - now.hp).max(0) as f64;
            continue;
        }
        if !before.is_at_war(pid, city.owner) {
            continue;
        }
        if now.owner == pid {
            value += 520.0
                + city.pop.max(1) as f64 * 14.0
                + city.districts.len() as f64 * 24.0
                + city.wonders.len() as f64 * 45.0
                + if city.is_capital { 180.0 } else { 0.0 };
            continue;
        }
        value += (city.wall_hp - now.wall_hp).max(0) as f64 * 1.35
            + (city.hp - now.hp).max(0) as f64
            + (city.encampment_wall_hp - now.encampment_wall_hp).max(0) as f64 * 1.35
            + (city.encampment_hp - now.encampment_hp).max(0) as f64;
        if !city.encampment_pillaged && now.encampment_pillaged {
            value += 180.0;
        }
    }

    value
}

/// What the enemy can do to the finished position, priced once for the whole
/// turn instead of once per unit against a half-played one.
///
/// Closed-form on purpose. The shipped per-unit path runs a cloned quiescence
/// search for every candidate action of every unit; doing that inside a
/// population-based search would dominate the cost. Damage is the engine's own
/// expectation, each enemy unit is assigned to the victim it hurts most, and
/// the damage aimed at a single victim is pooled so that a unit the enemy can
/// gang up on and kill is priced as a loss rather than as scratches.
fn reply_estimate(after: &Game, pid: usize) -> f64 {
    let mut incoming: BTreeMap<u32, f64> = BTreeMap::new();

    let mine: Vec<(u32, Pos, f64, i32)> = after
        .units
        .values()
        .filter(|unit| unit.owner == pid && after.rules.units[unit.kind].class == "military")
        .map(|unit| {
            (
                unit.id,
                unit.pos,
                effective_strength(after.unit_strength(unit, true), unit.hp),
                unit.hp,
            )
        })
        .collect();
    if mine.is_empty() {
        return 0.0;
    }

    let aim = |attack: f64, from: Pos, reach: i32, incoming: &mut BTreeMap<u32, f64>| {
        let mut best: Option<(f64, u32)> = None;
        for (id, pos, defence, _) in &mine {
            if after.wdist(from, *pos) > reach {
                continue;
            }
            let dealt = expected_damage(attack, *defence);
            if best.as_ref().is_none_or(|(top, _)| dealt > *top) {
                best = Some((dealt, *id));
            }
        }
        if let Some((dealt, victim)) = best {
            *incoming.entry(victim).or_insert(0.0) += dealt;
        }
    };

    for unit in after.units.values() {
        let spec = &after.rules.units[unit.kind];
        if unit.owner == pid || spec.class != "military" || !after.is_at_war(pid, unit.owner) {
            continue;
        }
        let ranged = spec.has_ranged_attack();
        let attack = if ranged {
            after.unit_ranged_attack_strength(unit)
        } else {
            after.unit_strength(unit, false)
        };
        // A unit that has not acted can normally move and then attack, so its
        // reach next turn is its range plus a step.
        let reach = if ranged {
            after.unit_attack_range(unit.id).max(1) + 1
        } else {
            2
        };
        aim(
            effective_strength(attack, unit.hp),
            unit.pos,
            reach,
            &mut incoming,
        );
    }

    // Cities shoot too, and standing next to one is the most common way a
    // besieging army bleeds.
    for city in after.cities.values() {
        if city.owner == pid || !after.is_at_war(pid, city.owner) {
            continue;
        }
        aim(after.city_strength(city.id), city.pos, 2, &mut incoming);
    }

    let mut total = 0.0;
    for (victim, dealt) in incoming {
        let Some(unit) = after.units.get(&victim) else {
            continue;
        };
        let cost = after.rules.units[unit.kind].cost;
        if dealt >= unit.hp as f64 {
            total += 230.0 + cost * 0.65;
        } else {
            total += dealt * (1.25 + cost / 800.0);
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Action;

    /// Two archers of ours in range of two enemy warriors, one of them nearly
    /// dead. Returns the game, our unit ids, and the enemy ids.
    fn firing_line(dying_hp: i32, healthy_hp: i32) -> (Game, Vec<u32>, Vec<u32>) {
        let mut g = Game::new_full(2, 24, 16, 5_150, 80, 0, false);
        for pid in 0..2 {
            let settler = g
                .player_unit_ids(pid)
                .into_iter()
                .find(|uid| g.units[uid].kind == "settler")
                .expect("a start has a settler");
            g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
            g.apply(pid, &Action::EndTurn).unwrap();
        }
        let home = g.cities[&g.player_city_ids(0)[0]].pos;
        // Clear the ordinary starting escort so the scenario contains exactly
        // the units under test. `remove_unit` and not `units.remove`: writing
        // the map directly leaves the occupancy index holding a unit id that
        // `units` no longer has, and the panic surfaces much later in whatever
        // next reads a neighbouring tile.
        for uid in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(uid);
        }

        // A pocket of open ground away from the city tile itself.
        let open: Vec<crate::Pos> = g
            .wdisk(home, 4)
            .into_iter()
            .filter(|pos| {
                g.map
                    .get(*pos)
                    .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
                    && g.city_at(*pos).is_none()
                    && g.units_at(*pos).is_empty()
            })
            .collect();
        // Ours stand together; theirs stand within the archers' reach of both.
        // Search for a pocket that admits all four rather than assuming the
        // first open tile has neighbours — on a real map it often does not.
        let pocket = open.iter().find_map(|ours| {
            let mate = *open.iter().find(|pos| g.wdist(**pos, *ours) == 1)?;
            let targets: Vec<crate::Pos> = open
                .iter()
                .copied()
                .filter(|pos| {
                    *pos != *ours
                        && *pos != mate
                        && g.wdist(*pos, *ours) <= 2
                        && g.wdist(*pos, mate) <= 2
                })
                .take(2)
                .collect();
            (targets.len() == 2).then_some((*ours, mate, targets))
        });
        let (ours, mate, targets) = pocket.expect("the map must offer a four-tile pocket");

        let mine = vec![
            g.spawn_unit("archer", 0, ours),
            g.spawn_unit("archer", 0, mate),
        ];
        let theirs = vec![
            g.spawn_unit("warrior", 1, targets[0]),
            g.spawn_unit("warrior", 1, targets[1]),
        ];
        g.units.get_mut(&theirs[0]).unwrap().hp = dying_hp;
        g.units.get_mut(&theirs[1]).unwrap().hp = healthy_hp;
        g.record_contact(0, 1);
        g.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
        (g, mine, theirs)
    }

    fn targets_of(plan: &TacticalPlan) -> Vec<crate::Pos> {
        plan.actions
            .iter()
            .filter_map(|action| match action {
                Action::Attack { target, .. }
                | Action::Ranged { target, .. }
                | Action::PriorityTarget { target, .. } => Some(*target),
                _ => None,
            })
            .collect()
    }

    /// One archer is already enough to finish the wounded warrior, so spending
    /// the second on it too throws a whole attack away.
    ///
    /// This is a necessary property rather than a discriminating one — the
    /// shipped sequential rule also avoids this particular overkill, because
    /// the second archer sees the first one's damage already applied. What it
    /// guards is that planning the turn as a set does not *lose* a property
    /// the incumbent has. The gain over the incumbent is a matter of degree
    /// across many positions and is measured by `battle_bench`, not asserted
    /// here.
    #[test]
    fn two_attackers_do_not_both_spend_themselves_on_one_dying_enemy() {
        let (g, _, theirs) = firing_line(8, 100);
        let dying = g.units[&theirs[0]].pos;
        let plan = JointTactics::default()
            .plan(&g, 0, &BasicAi::new())
            .expect("two engaged archers are a joint problem");
        let aimed = targets_of(&plan);
        assert_eq!(aimed.len(), 2, "both archers should shoot: {aimed:?}");
        assert!(
            aimed.iter().filter(|pos| **pos == dying).count() <= 1,
            "both archers piled onto a warrior on 8 health: {aimed:?}"
        );
    }

    /// A searching agent that answers the same position differently on
    /// different runs cannot be measured — the repository's evaluators rely on
    /// the same game replaying bit-identically.
    #[test]
    fn the_same_position_is_always_answered_the_same_way() {
        let (g, _, _) = firing_line(30, 80);
        let search = JointTactics::default();
        let first = search.plan(&g, 0, &BasicAi::new()).expect("a plan");
        for _ in 0..3 {
            let again = search.plan(&g, 0, &BasicAi::new()).expect("a plan");
            assert_eq!(first.actions, again.actions);
        }
    }

    /// The search must never return a turn its own evaluator rates below the
    /// sequential-greedy construction it started from. That property is what
    /// makes a small budget safe: the worst case is today's behaviour, not
    /// noise.
    #[test]
    fn the_plan_never_scores_below_the_greedy_incumbent() {
        for (dying, healthy) in [(8, 100), (30, 80), (55, 55), (95, 20)] {
            let (g, _, _) = firing_line(dying, healthy);
            let Some(plan) = JointTactics::default().plan(&g, 0, &BasicAi::new()) else {
                continue;
            };
            assert!(
                plan.score >= plan.greedy_score - 1e-9,
                "at {dying}/{healthy} the search returned {} against an incumbent {}",
                plan.score,
                plan.greedy_score
            );
        }
    }

    /// A single engaged unit has no joint problem to solve, so the cheaper
    /// per-unit path keeps it and the search does not run at all.
    #[test]
    fn one_lone_attacker_is_left_to_the_per_unit_path() {
        let (mut g, mine, _) = firing_line(40, 40);
        g.remove_unit(mine[1]);
        assert!(JointTactics::default().plan(&g, 0, &BasicAi::new()).is_none());
    }

    /// The expectation used by the search has to be the engine's own damage
    /// curve, or every prior in this module is calibrated against a different
    /// game than the one being played.
    #[test]
    fn expected_damage_is_the_engine_curve_at_its_mean() {
        let mut rng = Rng::new(7);
        for (attack, defence) in [(30.0, 30.0), (45.0, 25.0), (20.0, 40.0), (36.0, 30.0)] {
            let mut total = 0.0;
            let samples = 20_000;
            for _ in 0..samples {
                total += crate::game::damage(attack, defence, &mut rng) as f64;
            }
            let empirical = total / samples as f64;
            let predicted = expected_damage(attack, defence);
            assert!(
                (empirical - predicted).abs() <= predicted * 0.05 + 0.5,
                "attack {attack} vs defence {defence}: predicted {predicted:.2}, \
                 engine averages {empirical:.2}"
            );
        }
    }

}
