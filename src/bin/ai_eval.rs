//! Paired, seat-balanced head-to-head evaluator for built-in AIs.
use civvis::ai::Ai;
use civvis::elo::{
    builtin_ai, builtin_provenances, collapsed_entrants, AgentProvenance, ARTIFACT_DIR,
    BUILTIN_AIS, EVAL_ONLY_AIS,
};
use civvis::game::{default_difficulty, Action, Game, GameOptions, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapTopology};
use civvis::strategic::ReviewCensus;
use std::collections::{BTreeMap, BTreeSet};

const PROMOTION_MIN_MAPS: usize = 20;
const Z_95: f64 = 1.959_963_984_540_054;
const TRIGGER_MIN_SHARE: f64 = 0.30;
const TRIGGER_MIN_PER_SEAT: f64 = 0.75;
const TRIGGER_MIN_SEAT_COVERAGE: f64 = 0.25;
/// Split a 5% two-sided error budget equally between promotion and retention.
const ANYTIME_TAIL_ALPHA: f64 = 0.025;
/// Fixed, pre-declared bets for a finite mixture e-process. At the parity null
/// every paired-map score is in [0, 1], so each factor
/// `1 + lambda * (score - 0.5)` is nonnegative and has expectation at most one
/// for the challenger-side test. Negating the bet tests the incumbent side.
const BET_LAMBDAS: [f64; 10] = [0.05, 0.10, 0.20, 0.35, 0.50, 0.70, 0.90, 1.15, 1.45, 1.80];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromotionVerdict {
    Insufficient,
    Promote,
    Retain,
    Inconclusive,
}

#[derive(Debug, Clone, Copy)]
struct PairedInference {
    maps: usize,
    score: f64,
    low: f64,
    high: f64,
    elo: f64,
    elo_low: f64,
    elo_high: f64,
    anytime: AnytimeEvidence,
    verdict: PromotionVerdict,
}

#[derive(Debug, Clone, Copy)]
struct AnytimeEvidence {
    challenger_peak_e: f64,
    incumbent_peak_e: f64,
    challenger_p: f64,
    incumbent_p: f64,
    challenger_crossed_at: Option<usize>,
    incumbent_crossed_at: Option<usize>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PairOutcomes {
    a_sweeps: usize,
    neutral: usize,
    b_sweeps: usize,
    mixed_with_draw: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct DirectionalOutcomes {
    challenger_favored: usize,
    neutral: usize,
    incumbent_favored: usize,
}

fn elo_edge(score: f64) -> f64 {
    let bounded = score.clamp(1e-6, 1.0 - 1e-6);
    400.0 * (bounded / (1.0 - bounded)).log10()
}

fn game_score(winner: Option<usize>, seats: &[&str], challenger: &str) -> f64 {
    winner
        .and_then(|pid| seats.get(pid))
        .map_or(0.5, |name| if *name == challenger { 1.0 } else { 0.0 })
}

/// Challenger share of terminal Civilization score across the evaluated
/// seats. This is a bounded secondary development diagnostic, not a win and
/// never an input to the promotion verdict.
fn terminal_score_share(g: &Game, seats: &[&str], challenger: &str) -> f64 {
    let mut challenger_score = 0_i64;
    let mut total_score = 0_i64;
    for (pid, name) in seats.iter().enumerate() {
        let score = g.score(pid).max(0);
        total_score += score;
        if *name == challenger {
            challenger_score += score;
        }
    }
    if total_score > 0 {
        challenger_score as f64 / total_score as f64
    } else {
        0.5
    }
}

fn log_mean_exp(values: &[f64]) -> f64 {
    let largest = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    largest
        + values
            .iter()
            .map(|value| (*value - largest).exp())
            .sum::<f64>()
            .ln()
        - (values.len() as f64).ln()
}

/// Anytime-valid evidence against parity from a finite mixture of betting
/// martingales. The process starts with one unit of wealth; Ville's inequality
/// makes `1 / peak wealth` a valid upper bound on the probability of ever
/// observing at least this much evidence under the null, even if the evaluator
/// is rerun with longer prefixes and stopped when a result looks favorable.
///
/// Monitoring begins only at `PROMOTION_MIN_MAPS`, so a lucky sub-minimum prefix
/// cannot earn a permanent promotion before the representativeness floor.
fn anytime_evidence(scores: &[f64]) -> AnytimeEvidence {
    let mut challenger_log_wealth = [0.0; BET_LAMBDAS.len()];
    let mut incumbent_log_wealth = [0.0; BET_LAMBDAS.len()];
    let mut challenger_peak_log_e = 0.0_f64;
    let mut incumbent_peak_log_e = 0.0_f64;
    let mut challenger_crossed_at = None;
    let mut incumbent_crossed_at = None;
    let crossing_log_e = -(ANYTIME_TAIL_ALPHA.ln());

    for (index, raw_score) in scores.iter().enumerate() {
        debug_assert!((0.0..=1.0).contains(raw_score));
        let edge = raw_score.clamp(0.0, 1.0) - 0.5;
        for (bet, lambda) in BET_LAMBDAS.iter().enumerate() {
            challenger_log_wealth[bet] += (1.0 + lambda * edge).ln();
            incumbent_log_wealth[bet] += (1.0 - lambda * edge).ln();
        }
        let maps = index + 1;
        if maps < PROMOTION_MIN_MAPS {
            continue;
        }
        let challenger_log_e = log_mean_exp(&challenger_log_wealth);
        let incumbent_log_e = log_mean_exp(&incumbent_log_wealth);
        challenger_peak_log_e = challenger_peak_log_e.max(challenger_log_e);
        incumbent_peak_log_e = incumbent_peak_log_e.max(incumbent_log_e);
        if challenger_crossed_at.is_none() && challenger_log_e >= crossing_log_e {
            challenger_crossed_at = Some(maps);
        }
        if incumbent_crossed_at.is_none() && incumbent_log_e >= crossing_log_e {
            incumbent_crossed_at = Some(maps);
        }
    }

    AnytimeEvidence {
        challenger_peak_e: challenger_peak_log_e.min(f64::MAX.ln()).exp(),
        incumbent_peak_e: incumbent_peak_log_e.min(f64::MAX.ln()).exp(),
        challenger_p: (-challenger_peak_log_e).exp().min(1.0),
        incumbent_p: (-incumbent_peak_log_e).exp().min(1.0),
        challenger_crossed_at,
        incumbent_crossed_at,
    }
}

/// A conservative Wilson score interval with one observation per mirrored map.
///
/// Pair scores can be fractional because a split scores 0.5 and a game without
/// a winner is a draw. Treating each bounded map score as one Bernoulli-equivalent
/// observation uses the maximum variance for that mean, so the swapped games are
/// never falsely counted as independent evidence.
fn paired_inference(scores: &[f64]) -> PairedInference {
    let maps = scores.len();
    let anytime = anytime_evidence(scores);
    if maps == 0 {
        return PairedInference {
            maps,
            score: 0.5,
            low: 0.0,
            high: 1.0,
            elo: 0.0,
            elo_low: elo_edge(0.0),
            elo_high: elo_edge(1.0),
            anytime,
            verdict: PromotionVerdict::Insufficient,
        };
    }

    let score = scores.iter().sum::<f64>() / maps as f64;
    let n = maps as f64;
    let z2 = Z_95 * Z_95;
    let denominator = 1.0 + z2 / n;
    let center = (score + z2 / (2.0 * n)) / denominator;
    let radius = Z_95 * ((score * (1.0 - score) / n + z2 / (4.0 * n * n)).sqrt()) / denominator;
    let low = (center - radius).clamp(0.0, 1.0);
    let high = (center + radius).clamp(0.0, 1.0);
    let challenger_evidence = anytime.challenger_p <= ANYTIME_TAIL_ALPHA;
    let incumbent_evidence = anytime.incumbent_p <= ANYTIME_TAIL_ALPHA;
    let verdict = if maps < PROMOTION_MIN_MAPS {
        PromotionVerdict::Insufficient
    } else if challenger_evidence && incumbent_evidence {
        // Strong evidence in both directions means the run is nonstationary
        // or its map order is pathological, not that either AI is promotable.
        PromotionVerdict::Inconclusive
    } else if challenger_evidence && low > 0.5 {
        PromotionVerdict::Promote
    } else if incumbent_evidence && high < 0.5 {
        PromotionVerdict::Retain
    } else {
        PromotionVerdict::Inconclusive
    };

    PairedInference {
        maps,
        score,
        low,
        high,
        elo: elo_edge(score),
        elo_low: elo_edge(low),
        elo_high: elo_edge(high),
        anytime,
        verdict,
    }
}

fn pair_outcomes(scores: &[f64]) -> PairOutcomes {
    let mut outcomes = PairOutcomes::default();
    for score in scores {
        if (*score - 1.0).abs() < f64::EPSILON {
            outcomes.a_sweeps += 1;
        } else if score.abs() < f64::EPSILON {
            outcomes.b_sweeps += 1;
        } else if (*score - 0.5).abs() < f64::EPSILON {
            outcomes.neutral += 1;
        } else {
            outcomes.mixed_with_draw += 1;
        }
    }
    outcomes
}

/// Exact two-sided sign-test probability under equally likely directions.
/// Neutral mirrored maps are deliberately excluded: they contain effect-size
/// information but no evidence about which AI is directionally stronger.
fn exact_sign_p(a_favored: usize, b_favored: usize) -> f64 {
    let n = a_favored + b_favored;
    if n == 0 {
        return 1.0;
    }
    let tail = a_favored.min(b_favored);
    let n_f = n as f64;
    let mut log_choose = 0.0;
    let mut log_terms = Vec::with_capacity(tail + 1);
    for k in 0..=tail {
        log_terms.push(log_choose - n_f * std::f64::consts::LN_2);
        if k < tail {
            log_choose += ((n - k) as f64).ln() - ((k + 1) as f64).ln();
        }
    }
    let largest = log_terms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let lower_tail = largest.exp()
        * log_terms
            .iter()
            .map(|term| (*term - largest).exp())
            .sum::<f64>();
    (2.0 * lower_tail).min(1.0)
}

fn directional_outcomes(scores: &[f64]) -> DirectionalOutcomes {
    let mut outcomes = DirectionalOutcomes::default();
    for score in scores {
        if *score > 0.5 + f64::EPSILON {
            outcomes.challenger_favored += 1;
        } else if *score < 0.5 - f64::EPSILON {
            outcomes.incumbent_favored += 1;
        } else {
            outcomes.neutral += 1;
        }
    }
    outcomes
}

/// How much of the map set a direction statistic actually rests on.
///
/// A mirrored A/B between close agents splits most maps by construction, and
/// a split carries no direction. The win statistic is therefore computed
/// from only the maps that broke, while the terminal-score statistic — being
/// continuous — breaks on nearly all of them. Reporting the two resolutions
/// side by side is what stops a 5-0 win margin drawn from five maps being
/// read as stronger evidence than a 10-8 score margin drawn from eighteen.
fn resolved_maps(directions: &DirectionalOutcomes) -> usize {
    directions.challenger_favored + directions.incumbent_favored
}

/// Sign of a direction: `Some(true)` favours the challenger, `Some(false)`
/// the incumbent, `None` when the maps that broke split evenly.
fn direction_sign(directions: &DirectionalOutcomes) -> Option<bool> {
    match directions
        .challenger_favored
        .cmp(&directions.incumbent_favored)
    {
        std::cmp::Ordering::Greater => Some(true),
        std::cmp::Ordering::Less => Some(false),
        std::cmp::Ordering::Equal => None,
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PlanTrace {
    observations: usize,
    switches: usize,
    rush_observations: usize,
    ever_rushed: bool,
    targets: BTreeMap<String, usize>,
    last_target: Option<String>,
    strategy_switches: usize,
    strategy_turns: BTreeMap<String, usize>,
    midgame_observations: usize,
    midgame_strategy_switches: usize,
    midgame_boundary_switches: usize,
    midgame_unanchored_switches: usize,
    midgame_war_boundary_switches: usize,
    midgame_threat_boundary_switches: usize,
    midgame_city_deficit_boundary_switches: usize,
    midgame_strategy_turns: BTreeMap<String, usize>,
    midgame_transitions: BTreeMap<String, usize>,
    midgame_reason_turns: BTreeMap<String, usize>,
    midgame_reason_transitions: BTreeMap<String, usize>,
    midgame_unanchored_reason_transitions: BTreeMap<String, usize>,
    midgame_unanchored_same_reason_transitions: BTreeMap<String, usize>,
    midgame_unanchored_reason_families: BTreeMap<String, usize>,
    last_strategy: Option<String>,
    last_reason: Option<String>,
    last_context: Option<StrategyContext>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StrategyContext {
    at_major_war: bool,
    threatened: bool,
    city_deficit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlanObservation {
    target: &'static str,
    strategy: &'static str,
    reason: &'static str,
    rush: bool,
    context: StrategyContext,
    midgame: bool,
}

fn ordered_transition(previous: &str, current: &str) -> String {
    format!("{previous} -> {current}")
}

fn reason_family(left: &str, right: &str) -> String {
    if left <= right {
        format!("{left} <-> {right}")
    } else {
        format!("{right} <-> {left}")
    }
}

impl PlanTrace {
    fn observe(&mut self, observation: PlanObservation) {
        let PlanObservation {
            target,
            strategy,
            reason,
            rush,
            context,
            midgame,
        } = observation;
        if self
            .last_target
            .as_deref()
            .is_some_and(|previous| previous != target)
        {
            self.switches += 1;
        }
        self.observations += 1;
        self.rush_observations += rush as usize;
        self.ever_rushed |= rush;
        *self.targets.entry(target.to_string()).or_default() += 1;
        self.last_target = Some(target.to_string());

        let strategy_changed = self
            .last_strategy
            .as_deref()
            .is_some_and(|previous| previous != strategy);
        if strategy_changed {
            self.strategy_switches += 1;
            if midgame {
                self.midgame_strategy_switches += 1;
                let previous = self.last_strategy.as_deref().unwrap();
                *self
                    .midgame_transitions
                    .entry(format!("{previous}->{strategy}"))
                    .or_default() += 1;
                let previous_context = self.last_context.unwrap();
                let war_changed = previous_context.at_major_war != context.at_major_war;
                let threat_changed = previous_context.threatened != context.threatened;
                let city_deficit_changed = previous_context.city_deficit != context.city_deficit;
                self.midgame_war_boundary_switches += war_changed as usize;
                self.midgame_threat_boundary_switches += threat_changed as usize;
                self.midgame_city_deficit_boundary_switches += city_deficit_changed as usize;
                let boundary_accompanied = war_changed || threat_changed || city_deficit_changed;
                if boundary_accompanied {
                    self.midgame_boundary_switches += 1;
                } else {
                    self.midgame_unanchored_switches += 1;
                }

                let previous_reason = self.last_reason.as_deref().unwrap();
                if previous_reason != reason {
                    let transition = ordered_transition(previous_reason, reason);
                    *self
                        .midgame_reason_transitions
                        .entry(transition.clone())
                        .or_default() += 1;
                    if !boundary_accompanied {
                        *self
                            .midgame_unanchored_reason_transitions
                            .entry(transition)
                            .or_default() += 1;
                    }
                } else if !boundary_accompanied {
                    *self
                        .midgame_unanchored_same_reason_transitions
                        .entry(format!("{previous}->{strategy} under {reason}"))
                        .or_default() += 1;
                }
                if !boundary_accompanied {
                    *self
                        .midgame_unanchored_reason_families
                        .entry(reason_family(previous_reason, reason))
                        .or_default() += 1;
                }
            }
        }
        *self.strategy_turns.entry(strategy.to_string()).or_default() += 1;
        if midgame {
            self.midgame_observations += 1;
            *self
                .midgame_strategy_turns
                .entry(strategy.to_string())
                .or_default() += 1;
            *self
                .midgame_reason_turns
                .entry(reason.to_string())
                .or_default() += 1;
        }
        self.last_strategy = Some(strategy.to_string());
        self.last_reason = Some(reason.to_string());
        self.last_context = Some(context);
    }

    /// Target used on the most observed player-turns. A tie keeps the final
    /// target, matching the tournament's dominant-strategy attribution.
    fn dominant_target(&self) -> &str {
        let most = self.targets.values().copied().max().unwrap_or(0);
        if self
            .last_target
            .as_ref()
            .is_some_and(|target| self.targets.get(target) == Some(&most))
        {
            return self.last_target.as_deref().unwrap();
        }
        self.targets
            .iter()
            .find(|(_, turns)| **turns == most)
            .map_or("unreported", |(target, _)| target.as_str())
    }
}

fn plan_observation(g: &Game, pid: usize, ai: &dyn Ai) -> PlanObservation {
    let midgame = g.turn >= g.standard_duration(60) && g.turn < g.standard_duration(180);
    let at_major_war = g.players.iter().any(|player| {
        player.id != pid
            && player.alive
            && !player.is_minor
            && !player.is_barbarian
            && g.is_at_war(pid, player.id)
    });
    ai.plan_report().map_or(
        PlanObservation {
            target: "unreported",
            strategy: "unreported",
            reason: "unreported",
            rush: false,
            context: StrategyContext {
                at_major_war,
                threatened: false,
                city_deficit: false,
            },
            midgame,
        },
        |plan| PlanObservation {
            target: plan.victory_target.unwrap_or("adaptive"),
            strategy: plan.strategy,
            // Filled from the exact assessment that produced the plan once
            // the currently claimed planner path is released.
            reason: "unreported",
            rush: plan.rush,
            context: StrategyContext {
                at_major_war,
                threatened: plan.threatened_city.is_some(),
                city_deficit: g.player_city_ids(pid).len() < plan.desired_cities,
            },
            midgame,
        },
    )
}

fn plan_target(g: &Game, pid: usize, ai: &dyn Ai) -> &'static str {
    plan_observation(g, pid, ai).target
}

fn run_traced_game(
    game: &mut Game,
    ais: &mut [Box<dyn Ai>],
    traced_players: usize,
) -> Vec<PlanTrace> {
    let mut traces: Vec<PlanTrace> = (0..traced_players).map(|_| PlanTrace::default()).collect();
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        ais[pid].take_turn(game, pid);
        if pid < traced_players {
            let observation = plan_observation(game, pid, ais[pid].as_ref());
            traces[pid].observe(observation);
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    traces
}

#[derive(Default)]
struct TargetOutcome {
    games: usize,
    wins: usize,
}

#[derive(Default)]
struct Metrics {
    games: usize,
    wins: usize,
    score: f64,
    cities: f64,
    population: f64,
    techs: f64,
    civics: f64,
    districts: f64,
    buildings: f64,
    military: f64,
    gold: f64,
    faith: f64,
    tourists: f64,
    dvp: f64,
    envoys: f64,
    suzerainties: f64,
    military_units: f64,
    civilian_units: f64,
    religious_units: f64,
    food_yield: f64,
    production_yield: f64,
    science_yield: f64,
    culture_yield: f64,
    queued_cost: f64,
    settlers: f64,
    builders: f64,
    traders: f64,
    active_routes: f64,
    trade_capacity: f64,
    support_units: f64,
    missionaries: f64,
    victories: BTreeMap<String, usize>,
    final_targets: BTreeMap<String, usize>,
    dominant_targets: BTreeMap<String, usize>,
    target_outcomes: BTreeMap<String, TargetOutcome>,
    plan_turns: BTreeMap<String, usize>,
    plan_observations: usize,
    plan_switches: usize,
    strategy_turns: BTreeMap<String, usize>,
    strategy_switches: usize,
    midgame_strategy_turns: BTreeMap<String, usize>,
    midgame_observations: usize,
    midgame_strategy_switches: usize,
    midgame_boundary_switches: usize,
    midgame_unanchored_switches: usize,
    midgame_war_boundary_switches: usize,
    midgame_threat_boundary_switches: usize,
    midgame_city_deficit_boundary_switches: usize,
    midgame_transitions: BTreeMap<String, usize>,
    midgame_reason_turns: BTreeMap<String, usize>,
    midgame_reason_transitions: BTreeMap<String, usize>,
    midgame_unanchored_reason_transitions: BTreeMap<String, usize>,
    midgame_unanchored_same_reason_transitions: BTreeMap<String, usize>,
    midgame_unanchored_reason_families: BTreeMap<String, usize>,
    midgame_unanchored_reason_family_seats: BTreeMap<String, usize>,
    rush_seats: usize,
    rush_turns: usize,
    /// Reviews summed over every seat this entrant played, so a run can say
    /// whether the macro search ever ran. Stays zero for agents that do not
    /// search, which is honest rather than missing.
    census: ReviewCensus,
    /// Seats whose agent reports a search at all, distinguishing "searched
    /// zero times" from "has no search to report".
    searching_seats: usize,
}

impl Metrics {
    fn record(&mut self, g: &Game, pid: usize, won: bool, final_target: &str, trace: &PlanTrace) {
        let cities = g.player_city_ids(pid);
        self.games += 1;
        self.wins += won as usize;
        *self
            .final_targets
            .entry(final_target.to_string())
            .or_default() += 1;
        let dominant_target = trace.dominant_target().to_string();
        *self
            .dominant_targets
            .entry(dominant_target.clone())
            .or_default() += 1;
        let outcome = self.target_outcomes.entry(dominant_target).or_default();
        outcome.games += 1;
        outcome.wins += won as usize;
        self.plan_observations += trace.observations;
        self.plan_switches += trace.switches;
        self.strategy_switches += trace.strategy_switches;
        self.midgame_observations += trace.midgame_observations;
        self.midgame_strategy_switches += trace.midgame_strategy_switches;
        self.midgame_boundary_switches += trace.midgame_boundary_switches;
        self.midgame_unanchored_switches += trace.midgame_unanchored_switches;
        self.midgame_war_boundary_switches += trace.midgame_war_boundary_switches;
        self.midgame_threat_boundary_switches += trace.midgame_threat_boundary_switches;
        self.midgame_city_deficit_boundary_switches += trace.midgame_city_deficit_boundary_switches;
        self.rush_seats += trace.ever_rushed as usize;
        self.rush_turns += trace.rush_observations;
        for (target, turns) in &trace.targets {
            *self.plan_turns.entry(target.clone()).or_default() += turns;
        }
        for (strategy, turns) in &trace.strategy_turns {
            *self.strategy_turns.entry(strategy.clone()).or_default() += turns;
        }
        for (strategy, turns) in &trace.midgame_strategy_turns {
            *self
                .midgame_strategy_turns
                .entry(strategy.clone())
                .or_default() += turns;
        }
        for (transition, count) in &trace.midgame_transitions {
            *self
                .midgame_transitions
                .entry(transition.clone())
                .or_default() += count;
        }
        self.record_assessment_trace(trace);
        if won {
            *self
                .victories
                .entry(
                    g.victory_type
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                )
                .or_default() += 1;
        }
        self.score += g.score(pid) as f64;
        self.cities += cities.len() as f64;
        self.population += cities.iter().map(|cid| g.cities[cid].pop).sum::<i32>() as f64;
        self.techs += g.players[pid].techs.len() as f64;
        self.civics += g.players[pid].civics.len() as f64;
        self.districts += cities
            .iter()
            .map(|cid| g.cities[cid].districts.len())
            .sum::<usize>() as f64;
        self.buildings += cities
            .iter()
            .map(|cid| g.cities[cid].buildings.len())
            .sum::<usize>() as f64;
        self.military += g.military_power(pid);
        self.gold += g.players[pid].gold;
        self.faith += g.players[pid].faith;
        self.tourists += g.foreign_tourists(pid) as f64;
        self.dvp += g.players[pid].dvp as f64;
        self.envoys += g.players[pid]
            .envoys
            .iter()
            .map(|(_, count)| *count)
            .sum::<i64>() as f64;
        self.suzerainties += g
            .players
            .iter()
            .filter(|minor| minor.alive && minor.is_minor && g.suzerain_of(minor.id) == Some(pid))
            .count() as f64;
        self.active_routes += g.active_routes(pid) as f64;
        self.trade_capacity += g.trade_capacity(pid) as f64;
        for unit in g.units.values().filter(|u| u.owner == pid) {
            match unit.kind.as_str() {
                "settler" => self.settlers += 1.0,
                "builder" => self.builders += 1.0,
                "trader" => self.traders += 1.0,
                "missionary" => self.missionaries += 1.0,
                _ if g.rules.units[unit.kind].class == "support" => {
                    self.support_units += 1.0
                }
                _ => {}
            }
            if g.rules.units[unit.kind].class == "military" {
                self.military_units += 1.0;
            } else {
                self.civilian_units += 1.0;
            }
            if g.rules.units[unit.kind].class == "religious" {
                self.religious_units += 1.0;
            }
        }
        for cid in &cities {
            let yields = g.city_yields(*cid);
            self.food_yield += yields.food;
            self.production_yield += yields.production;
            self.science_yield += yields.science;
            self.culture_yield += yields.culture;
            if let Some(item) = g.cities[cid].queue.first() {
                self.queued_cost += g.item_cost_for(pid, item);
            }
        }
    }

    fn record_assessment_trace(&mut self, trace: &PlanTrace) {
        for (reason, turns) in &trace.midgame_reason_turns {
            *self.midgame_reason_turns.entry(reason.clone()).or_default() += turns;
        }
        for (transition, count) in &trace.midgame_reason_transitions {
            *self
                .midgame_reason_transitions
                .entry(transition.clone())
                .or_default() += count;
        }
        for (transition, count) in &trace.midgame_unanchored_reason_transitions {
            *self
                .midgame_unanchored_reason_transitions
                .entry(transition.clone())
                .or_default() += count;
        }
        for (transition, count) in &trace.midgame_unanchored_same_reason_transitions {
            *self
                .midgame_unanchored_same_reason_transitions
                .entry(transition.clone())
                .or_default() += count;
        }
        for (family, count) in &trace.midgame_unanchored_reason_families {
            *self
                .midgame_unanchored_reason_families
                .entry(family.clone())
                .or_default() += count;
            *self
                .midgame_unanchored_reason_family_seats
                .entry(family.clone())
                .or_default() += 1;
        }
    }
}

fn target_shares(metrics: &Metrics) -> String {
    metrics
        .plan_turns
        .iter()
        .map(|(target, turns)| {
            let share = 100.0 * *turns as f64 / metrics.plan_observations.max(1) as f64;
            format!("{target} {share:.1}%")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn shares(turns: &BTreeMap<String, usize>, observations: usize) -> String {
    turns
        .iter()
        .map(|(label, turns)| {
            let share = 100.0 * *turns as f64 / observations.max(1) as f64;
            format!("{label} {share:.1}%")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn transition_counts(transitions: &BTreeMap<String, usize>) -> String {
    let mut ranked: Vec<(&str, usize)> = transitions
        .iter()
        .map(|(transition, count)| (transition.as_str(), *count))
        .collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
    ranked
        .into_iter()
        .map(|(transition, count)| format!("{transition} {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn family_counts(families: &BTreeMap<String, usize>, seats: &BTreeMap<String, usize>) -> String {
    let mut ranked: Vec<(&str, usize)> = families
        .iter()
        .map(|(family, count)| (family.as_str(), *count))
        .collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
    ranked
        .into_iter()
        .map(|(family, count)| {
            format!(
                "{family} {count} across {} seat-games",
                seats.get(family).copied().unwrap_or(0)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn elective_reason(reason: &str) -> bool {
    matches!(
        reason,
        "strong enough to take what a neighbour has"
            | "already well down its best victory lane"
            | "short of cities with land still open"
            | "its best available victory lane"
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TriggerGate<'a> {
    family: &'a str,
    occurrences: usize,
    seats: usize,
    eligible: bool,
    share: f64,
    per_seat: f64,
    seat_coverage: f64,
}

impl TriggerGate<'_> {
    fn passes(self) -> bool {
        self.eligible
            && self.share >= TRIGGER_MIN_SHARE
            && self.per_seat >= TRIGGER_MIN_PER_SEAT
            && self.seat_coverage >= TRIGGER_MIN_SEAT_COVERAGE
    }
}

fn trigger_gate(metrics: &Metrics) -> Option<TriggerGate<'_>> {
    let (family, occurrences) = metrics
        .midgame_unanchored_reason_families
        .iter()
        .map(|(family, count)| (family.as_str(), *count))
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(left.0)))?;
    let (left, right) = family
        .split_once(" <-> ")
        .expect("canonical reason families always have two labels");
    let seats = metrics
        .midgame_unanchored_reason_family_seats
        .get(family)
        .copied()
        .unwrap_or(0);
    let games = metrics.games.max(1) as f64;
    Some(TriggerGate {
        family,
        occurrences,
        seats,
        eligible: elective_reason(left) && elective_reason(right),
        share: occurrences as f64 / metrics.midgame_unanchored_switches.max(1) as f64,
        per_seat: occurrences as f64 / games,
        seat_coverage: seats as f64 / games,
    })
}

fn trigger_gate_report(metrics: &Metrics) -> String {
    let Some(gate) = trigger_gate(metrics) else {
        return "REJECT (no unanchored reason family)".to_string();
    };
    format!(
        "{}: {} — {}/{} ({:.1}%), {:.2}/seat-game, {}/{} seats ({:.1}%), elective {}",
        if gate.passes() { "NOMINATE" } else { "REJECT" },
        gate.family,
        gate.occurrences,
        metrics.midgame_unanchored_switches,
        100.0 * gate.share,
        gate.per_seat,
        gate.seats,
        metrics.games,
        100.0 * gate.seat_coverage,
        if gate.eligible { "yes" } else { "no" },
    )
}

fn text(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn number(args: &[String], flag: &str, default: i64) -> i64 {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a = args.first().map(|name| name.as_str()).unwrap_or("advanced");
    let b = args.get(1).map(|name| name.as_str()).unwrap_or("basic");
    assert_ne!(a, b, "choose two different AIs");
    for name in [a, b] {
        assert!(
            BUILTIN_AIS.contains(&name) || EVAL_ONLY_AIS.contains(&name),
            "unknown AI {name:?}: builtins {BUILTIN_AIS:?}; evaluator-only {EVAL_ONLY_AIS:?}"
        );
    }
    // A learned name is only worth recording if its artifacts actually
    // loaded. Say what each entrant resolved to before playing anything, so
    // a result is never filed under an agent that was never in the game.
    let artifact_dir = text(&args, "--artifact-dir", ARTIFACT_DIR);
    let provenance = builtin_provenances(&[a, b], &artifact_dir);
    for entry in &provenance {
        println!("{}", entry.line());
    }
    for (left, right, shared) in collapsed_entrants(&[a, b], &artifact_dir) {
        println!(
            "warning: {left} and {right} both play as {shared}; this run measures \
             {shared} against itself and says nothing about either name"
        );
    }
    if args.iter().any(|arg| arg == "--require-artifacts") {
        let untrained: Vec<&AgentProvenance> = provenance
            .iter()
            .filter(|entry| entry.untrained())
            .collect();
        if !untrained.is_empty() {
            for entry in untrained {
                eprintln!(
                    "{}: missing {} in {artifact_dir}/",
                    entry.requested,
                    entry.missing().join(", ")
                );
            }
            eprintln!("--require-artifacts: refusing to record an untrained result");
            std::process::exit(3);
        }
    }
    let pairs = number(&args, "--pairs", 50).max(1) as usize;
    // How many games run at once. Every game is independent and seeded, so
    // this changes only wall-clock time, never a result.
    let jobs = match number(&args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    };
    let turns = number(&args, "--turns", 180).max(1) as u32;
    let players = number(&args, "--players", 2).max(2) as usize;
    let city_states = number(&args, "--city-states", 0).max(0) as usize;
    // Every result this evaluator has ever produced was measured at the default
    // game speed, while the exhibition and the live league both run **Online**
    // (`data/speeds.json`: 250 turns, cost_pct 50). A promoted gain is a gain on
    // the game it was measured on, and nothing in this repository has ever
    // checked that one transfers to the other. This flag is what makes that
    // check possible; it defaults to the previous behaviour.
    let speed = text(&args, "--speed", &civvis::game::default_speed());
    let width = number(&args, "--width", 24).max(8) as i32;
    let height = number(&args, "--height", 16).max(8) as i32;
    let seed = number(&args, "--seed", 4000).max(0) as u64;
    // The exhibition varies all three world axes and pins its enabled victory
    // set. An evaluator that cannot name them silently measures a different
    // game: historically Pangaea/flat/fixed-roster/all-victories, whatever the
    // command line appeared to say. Keep those historical defaults, but make
    // the deployment profile expressible and print the resolved values below.
    let map_name = text(&args, "--map", MapScript::default().id());
    let map_script = MapScript::from_id(&map_name).unwrap_or_else(|| {
        eprintln!("unknown map script {map_name:?}");
        std::process::exit(2);
    });
    let shape_name = text(&args, "--shape", MapTopology::default().id());
    let map_topology = MapTopology::from_id(&shape_name).unwrap_or_else(|| {
        eprintln!("unknown map shape {shape_name:?}");
        std::process::exit(2);
    });
    let poles_name = text(&args, "--poles", MapPoles::default().id());
    let map_poles = MapPoles::from_id(&poles_name).unwrap_or_else(|| {
        eprintln!("unknown thermal distribution {poles_name:?}");
        std::process::exit(2);
    });
    let default_victories = VictoryConditions::NAMES.join(",");
    let victory_names = text(&args, "--victories", &default_victories);
    let victory_conditions = VictoryConditions::parse(&victory_names).unwrap_or_else(|why| {
        eprintln!(
            "--victories: {why}; choose from {:?}",
            VictoryConditions::NAMES
        );
        std::process::exit(2);
    });
    let randomize_civs = args.iter().any(|arg| arg == "--randomize-civs");
    // The difficulty ladder as an external yardstick: the challenger plays
    // the human side of the handicap and its opponents play the AI side, so
    // "beats Emperor" means what a Civ player would expect it to mean.
    // Seats still swap, which moves the challenger around the map rather than
    // moving the handicap.
    let difficulty = text(&args, "--difficulty", &default_difficulty());
    if !Rules::embedded().difficulties.contains_key(&difficulty) {
        eprintln!("unknown difficulty {difficulty:?}");
        std::process::exit(2);
    }
    println!(
        "profile: speed {speed}, map {}, shape {}, poles {}, civilizations {}, victories {}",
        map_script.id(),
        map_topology.id(),
        map_poles.id(),
        if randomize_civs {
            "randomized"
        } else {
            "fixed"
        },
        VictoryConditions::NAMES
            .into_iter()
            .filter(|name| victory_conditions.is_enabled(name))
            .collect::<Vec<_>>()
            .join(","),
    );
    let mut totals: BTreeMap<String, Metrics> = [a, b]
        .into_iter()
        .map(|name| (name.to_string(), Metrics::default()))
        .collect();
    let mut total_turns = 0_u64;
    let mut pair_scores = Vec::with_capacity(pairs);
    let mut pair_terminal_scores = Vec::with_capacity(pairs);

    // One finished game, carried back from a worker so the fold below can
    // apply it in the order it would have happened serially.
    struct PlayedGame<'a> {
        game: Game,
        seats: Vec<&'a str>,
        traces: Vec<PlanTrace>,
        targets: Vec<&'static str>,
        censuses: Vec<Option<ReviewCensus>>,
    }

    // Games share nothing but the immutable ruleset, and every one is fully
    // determined by its seed, so a batch is embarrassingly parallel. Results
    // come back in index order and are folded sequentially, which makes a
    // parallel run produce byte-identical output to a serial one — only
    // sooner. That matters more here than anywhere else in the codebase:
    // this binary is the promotion gate, and how many maps it can afford is
    // what decides whether an effect is resolvable at all.
    //
    // Chunked rather than one flat batch so peak memory holds a chunk of
    // finished games rather than the whole run.
    let chunk_pairs = jobs.max(1);
    let mut pair = 0usize;
    while pair < pairs {
        let chunk = chunk_pairs.min(pairs - pair);
        let played = civvis::parallel::map(chunk * 2, jobs, |index| {
            let local_pair = pair + index / 2;
            let swap = index % 2;
            let game_seed = seed + local_pair as u64;
            let seats: Vec<&str> = (0..players)
                .map(|pid| if (pid + swap) % 2 == 0 { a } else { b })
                .collect();
            let challenger_seats: BTreeSet<usize> = seats
                .iter()
                .enumerate()
                .filter(|(_, name)| **name == a)
                .map(|(pid, _)| pid)
                .collect();
            let mut game = Game::new_with(GameOptions {
                difficulty: difficulty.clone(),
                human_seats: challenger_seats,
                speed: speed.clone(),
                map_script,
                map_topology,
                map_poles,
                randomize_civs,
                ..GameOptions::new(players, width, height, game_seed, turns, city_states)
            });
            game.victory_conditions = victory_conditions;
            let mut ais: Vec<Box<dyn Ai>> = game
                .players
                .iter()
                .map(|p| {
                    let name = if p.id < players { seats[p.id] } else { "basic" };
                    builtin_ai(name, game_seed + p.id as u64)
                })
                .collect();
            let traces = run_traced_game(&mut game, &mut ais, players);
            let targets = (0..players)
                .map(|pid| plan_target(&game, pid, ais[pid].as_ref()))
                .collect();
            let censuses = (0..players).map(|pid| ais[pid].review_census()).collect();
            PlayedGame {
                game,
                seats,
                traces,
                targets,
                censuses,
            }
        });
        for (index, result) in played.into_iter().enumerate() {
            let PlayedGame {
                game,
                seats,
                traces,
                targets,
                censuses,
            } = result;
            total_turns += game.reported_turn() as u64;
            let score = game_score(game.winner, &seats, a);
            let terminal = terminal_score_share(&game, &seats, a);
            if index % 2 == 0 {
                pair_scores.push(score);
                pair_terminal_scores.push(terminal);
            } else {
                *pair_scores.last_mut().expect("the swap follows its pair") += score;
                *pair_terminal_scores
                    .last_mut()
                    .expect("the swap follows its pair") += terminal;
            }
            // Legacy per-seat win metrics count a game nobody won as zero
            // wins. The paired promotion score above records it as a draw.
            for (pid, name) in seats.iter().enumerate() {
                if let Some(census) = censuses[pid] {
                    let metrics = totals.get_mut(*name).unwrap();
                    metrics.census.merge(census);
                    metrics.searching_seats += 1;
                }
                totals.get_mut(*name).unwrap().record(
                    &game,
                    pid,
                    game.winner == Some(pid),
                    targets[pid],
                    &traces[pid],
                );
            }
        }
        pair += chunk;
        eprintln!("progress: {pair}/{pairs} map pairs complete");
    }
    for score in pair_scores.iter_mut() {
        *score /= 2.0;
    }
    for score in pair_terminal_scores.iter_mut() {
        *score /= 2.0;
    }

    // Say what was measured, not just how much of it.
    //
    // Nineteen of the twenty `ai_eval` commands recorded in docs/EVAL.md do
    // not specify a map size, so every one of them ran at the 24x16 default
    // and a reader cannot tell without knowing that. The live exhibition now
    // varies dimensions with player count, while the old fixed 74x46-at-six
    // reference was 567 tiles per player against this binary's historical 96.
    // The shipped genome moved `city_target` -40% and `settler_min_pop` +123%,
    // which is the right answer at one density and plausibly the wrong one at
    // the other. Density belongs in the header of anything that claims a
    // strength result.
    let tiles_per_player = (width as f64 * height as f64) / players as f64;
    println!(
        "map: {width}x{height} = {} tiles, {tiles_per_player:.0} per player \
         (live exhibition dimensions vary with player count)",
        width * height
    );
    println!(
        "mirrored head-to-head: {pairs} maps, {} games, {players} players, average {:.1} turns",
        2 * pairs,
        total_turns as f64 / (2 * pairs) as f64
    );
    let games = 2 * pairs;
    print!("game-win share:");
    for name in [a, b] {
        let wins = totals[name].wins;
        print!(
            " {name} {wins}/{games} ({:.1}%)",
            100.0 * wins as f64 / games as f64
        );
    }
    println!();
    let inference = paired_inference(&pair_scores);
    let outcomes = pair_outcomes(&pair_scores);
    let directions = directional_outcomes(&pair_scores);
    println!(
        "paired-map score for {a}: {:.1}% (95% Wilson CI {:.1}%..{:.1}%), Elo-equivalent {:+.0} (CI {:+.0}..{:+.0})",
        100.0 * inference.score,
        100.0 * inference.low,
        100.0 * inference.high,
        inference.elo,
        inference.elo_low,
        inference.elo_high,
    );
    println!(
        "paired outcomes: {a} sweeps {}, neutral splits/draws {}, {b} sweeps {}, draw-mixed {}",
        outcomes.a_sweeps, outcomes.neutral, outcomes.b_sweeps, outcomes.mixed_with_draw
    );
    let sign_p = exact_sign_p(directions.challenger_favored, directions.incumbent_favored);
    let directional_verdict =
        if sign_p < 0.05 && directions.challenger_favored > directions.incumbent_favored {
            format!("SIGNIFICANT {a} DIRECTION")
        } else if sign_p < 0.05 && directions.incumbent_favored > directions.challenger_favored {
            format!("SIGNIFICANT {b} DIRECTION")
        } else {
            "INCONCLUSIVE DIRECTION".to_string()
        };
    println!(
        "paired direction: {a}-favored {}, neutral {}, {b}-favored {}; exact two-sided sign p={sign_p:.4} ({directional_verdict})",
        directions.challenger_favored, directions.neutral, directions.incumbent_favored
    );
    let challenger_crossing = inference
        .anytime
        .challenger_crossed_at
        .map_or("not crossed".to_string(), |map| {
            format!("crossed at map {map}")
        });
    let incumbent_crossing = inference
        .anytime
        .incumbent_crossed_at
        .map_or("not crossed".to_string(), |map| {
            format!("crossed at map {map}")
        });
    println!(
        "anytime-valid betting evidence (2.5% per direction after {PROMOTION_MIN_MAPS} maps): {a} peak e={:.3e}, p<={:.4} ({challenger_crossing}); {b} peak e={:.3e}, p<={:.4} ({incumbent_crossing})",
        inference.anytime.challenger_peak_e,
        inference.anytime.challenger_p,
        inference.anytime.incumbent_peak_e,
        inference.anytime.incumbent_p,
    );
    match inference.verdict {
        PromotionVerdict::Insufficient => println!(
            "promotion gate: INSUFFICIENT — {} independent maps; require at least {PROMOTION_MIN_MAPS}",
            inference.maps
        ),
        PromotionVerdict::Promote => println!(
            "promotion gate: PASS — {a}'s effect interval and anytime-valid evidence both clear parity after {} maps",
            inference.maps,
        ),
        PromotionVerdict::Retain => println!(
            "promotion gate: RETAIN {b} — {b}'s effect interval and anytime-valid evidence both clear parity after {} maps",
            inference.maps,
        ),
        PromotionVerdict::Inconclusive => println!(
            "promotion gate: INCONCLUSIVE — effect size or anytime-valid evidence has not cleared parity after {} maps",
            inference.maps,
        ),
    }
    let terminal_mean = pair_terminal_scores.iter().sum::<f64>() / pairs as f64;
    let terminal_directions = directional_outcomes(&pair_terminal_scores);
    let terminal_sign_p = exact_sign_p(
        terminal_directions.challenger_favored,
        terminal_directions.incumbent_favored,
    );
    let terminal_anytime = anytime_evidence(&pair_terminal_scores);
    println!(
        "paired terminal-score diagnostic for {a}: {:.1}% (not a promotion input)",
        100.0 * terminal_mean
    );
    println!(
        "terminal-score direction: {a}-favored {}, neutral {}, {b}-favored {}; exact two-sided sign p={terminal_sign_p:.4}",
        terminal_directions.challenger_favored,
        terminal_directions.neutral,
        terminal_directions.incumbent_favored,
    );
    // Wins and terminal score are computed from the same games but measure
    // different things: wins count victories, terminal score counts
    // economy. An agent that routes to a victory better wins more games
    // without out-scoring anyone, so the two disagreeing is a finding
    // rather than a fault — it localizes the change to routing rather than
    // development. What must not happen is reading whichever one looks
    // better, so both directions and the number of maps each rests on are
    // reported together.
    let win_resolved = resolved_maps(&directions);
    let terminal_resolved = resolved_maps(&terminal_directions);
    println!(
        "direction resolution: wins rest on {win_resolved} of {} maps that broke, terminal score on {terminal_resolved}",
        inference.maps,
    );
    match (
        direction_sign(&directions),
        direction_sign(&terminal_directions),
    ) {
        (Some(win), Some(terminal)) if win != terminal => println!(
            "note: wins favour {} and terminal score favours {}. Wins count victories and score counts economy, so this separates victory routing from development rather than contradicting itself",
            if win { a } else { b },
            if terminal { a } else { b },
        ),
        (Some(win), None) => println!(
            "note: wins favour {} while terminal score is flat — a routing change without an economic one",
            if win { a } else { b },
        ),
        _ => {}
    }
    println!(
        "terminal-score anytime evidence (2.5% per direction after {PROMOTION_MIN_MAPS} maps): {a} peak e={:.3e}, p<={:.4}; {b} peak e={:.3e}, p<={:.4}",
        terminal_anytime.challenger_peak_e,
        terminal_anytime.challenger_p,
        terminal_anytime.incumbent_peak_e,
        terminal_anytime.incumbent_p,
    );
    println!("AI          seat-win% score cities pop tech civic dist build military gold");
    for name in [a, b] {
        let m = &totals[name];
        let n = m.games as f64;
        println!(
            "{name:<11} {:>7.1}% {:>5.1} {:>6.2} {:>3.1} {:>4.1} {:>5.1} {:>4.1} {:>5.1} {:>8.1} {:>5.1}",
            100.0 * m.wins as f64 / n,
            m.score / n,
            m.cities / n,
            m.population / n,
            m.techs / n,
            m.civics / n,
            m.districts / n,
            m.buildings / n,
            m.military / n,
            m.gold / n,
        );
    }
    println!("\nAI          faith tourists dvp envoys suzerain religious#");
    for name in [a, b] {
        let m = &totals[name];
        let n = m.games as f64;
        println!(
            "{name:<11} {:>5.1} {:>8.1} {:>3.1} {:>6.1} {:>8.2} {:>10.2}",
            m.faith / n,
            m.tourists / n,
            m.dvp / n,
            m.envoys / n,
            m.suzerainties / n,
            m.religious_units / n,
        );
    }
    println!("\nAI          mil# civ#  food prod science culture queued-cost");
    for name in [a, b] {
        let m = &totals[name];
        let n = m.games as f64;
        println!(
            "{name:<11} {:>4.1} {:>4.1} {:>5.1} {:>4.1} {:>7.1} {:>7.1} {:>11.1}",
            m.military_units / n,
            m.civilian_units / n,
            m.food_yield / n,
            m.production_yield / n,
            m.science_yield / n,
            m.culture_yield / n,
            m.queued_cost / n,
        );
    }
    println!("\nAI          settler builder trader routes/cap support missionary");
    for name in [a, b] {
        let m = &totals[name];
        let n = m.games as f64;
        println!(
            "{name:<11} {:>7.2} {:>7.2} {:>6.2} {:>5.2}/{:<4.2} {:>7.2} {:>10.2}",
            m.settlers / n,
            m.builders / n,
            m.traders / n,
            m.active_routes / n,
            m.trade_capacity / n,
            m.support_units / n,
            m.missionaries / n,
        );
    }
    println!("\nVictory types:");
    for name in [a, b] {
        println!("  {name:<11} {:?}", totals[name].victories);
    }
    println!("\nPlan commitment by observed player-turn:");
    for name in [a, b] {
        let metrics = &totals[name];
        println!(
            "  {name:<11} switches/game {:.2}; {}",
            metrics.plan_switches as f64 / metrics.games.max(1) as f64,
            target_shares(metrics)
        );
    }
    println!("\nAdaptive grand strategy by observed player-turn:");
    for name in [a, b] {
        let metrics = &totals[name];
        let games = metrics.games.max(1);
        let switches = metrics.midgame_strategy_switches.max(1);
        println!(
            "  {name:<11} all-game switches/seat-game {:.2}, switches/100t {:.2}; {}",
            metrics.strategy_switches as f64 / games as f64,
            100.0 * metrics.strategy_switches as f64 / metrics.plan_observations.max(1) as f64,
            shares(&metrics.strategy_turns, metrics.plan_observations),
        );
        println!(
            "  {name:<11} midgame switches/seat-game {:.2}; unanchored/seat-game {:.2}, {}/{} ({:.1}%); {}",
            metrics.midgame_strategy_switches as f64 / games as f64,
            metrics.midgame_unanchored_switches as f64 / games as f64,
            metrics.midgame_unanchored_switches,
            metrics.midgame_strategy_switches,
            100.0 * metrics.midgame_unanchored_switches as f64 / switches as f64,
            shares(
                &metrics.midgame_strategy_turns,
                metrics.midgame_observations
            ),
        );
        println!(
            "  {name:<11} boundary-accompanied {}/{}; war {}, threat {}, city-deficit {}; transitions {}",
            metrics.midgame_boundary_switches,
            metrics.midgame_strategy_switches,
            metrics.midgame_war_boundary_switches,
            metrics.midgame_threat_boundary_switches,
            metrics.midgame_city_deficit_boundary_switches,
            transition_counts(&metrics.midgame_transitions),
        );
        println!(
            "  {name:<11} assessment reasons {}",
            shares(&metrics.midgame_reason_turns, metrics.midgame_observations),
        );
        println!(
            "  {name:<11} strategy-switch reason changes {}",
            transition_counts(&metrics.midgame_reason_transitions),
        );
        println!(
            "  {name:<11} unanchored reason changes {}; same-reason strategy changes {}",
            transition_counts(&metrics.midgame_unanchored_reason_transitions),
            transition_counts(&metrics.midgame_unanchored_same_reason_transitions),
        );
        println!(
            "  {name:<11} unanchored reason families {}",
            family_counts(
                &metrics.midgame_unanchored_reason_families,
                &metrics.midgame_unanchored_reason_family_seats,
            ),
        );
        println!(
            "  {name:<11} trigger-scoped experiment gate {}",
            trigger_gate_report(metrics),
        );
    }
    println!("\nAncient-rush treatment exposure:");
    for name in [a, b] {
        let metrics = &totals[name];
        println!(
            "  {name:<11} {}/{} seat-games ever rushed ({:.1}%); {}/{} observed player-turns rushing ({:.1}%)",
            metrics.rush_seats,
            metrics.games,
            100.0 * metrics.rush_seats as f64 / metrics.games.max(1) as f64,
            metrics.rush_turns,
            metrics.plan_observations,
            100.0 * metrics.rush_turns as f64 / metrics.plan_observations.max(1) as f64,
        );
    }
    // A searching entrant that never reached its search is a scripted agent
    // under a searching agent's name. Say so beside the win rate, because
    // the win rate cannot.
    if [a, b].iter().any(|name| totals[*name].searching_seats > 0) {
        println!("\nMacro search exposure (reviews that reached the rollouts):");
        for name in [a, b] {
            let metrics = &totals[name];
            if metrics.searching_seats == 0 {
                println!("  {name:<11} no search to report");
                continue;
            }
            let census = metrics.census;
            match census.search_exposure() {
                None => println!(
                    "  {name:<11} never reviewed ({} seats; games ended before turn {})",
                    metrics.searching_seats,
                    civvis::strategic::FIRST_REVIEW_TURN
                ),
                Some(share) => println!(
                    "  {name:<11} {}/{} ({:.0}%) reached the rollouts; priors: \
                     duel-religion {}, urgent-counter {}, irreversible-religion {}",
                    census.rollouts,
                    census.total(),
                    100.0 * share,
                    census.duel_religion,
                    census.urgent_counter,
                    census.irreversible_religion
                ),
            }
        }
        if [a, b]
            .iter()
            .all(|name| totals[*name].searching_seats > 0 && totals[*name].census.rollouts == 0)
        {
            println!(
                "  warning: neither entrant reached its macro search, so this run \
                 compares priors and the scripted parent, not search or evaluator"
            );
        }
    }
    println!("\nFinal plan targets:");
    for name in [a, b] {
        println!("  {name:<11} {:?}", totals[name].final_targets);
    }
    println!("\nDominant plan targets and seat outcomes:");
    for name in [a, b] {
        println!("  {name:<11} {:?}", totals[name].dominant_targets);
        for (target, outcome) in &totals[name].target_outcomes {
            println!(
                "    {target:<11} {}/{} wins ({:.1}%)",
                outcome.wins,
                outcome.games,
                100.0 * outcome.wins as f64 / outcome.games.max(1) as f64
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use civvis::rng::Rng;

    /// A 95% interval for the mean of paired map scores that uses their
    /// observed variance instead of the worst case.
    ///
    /// The Wilson interval the promotion gate turns on treats every map as a
    /// maximum-variance Bernoulli draw. That is the right worst case for a
    /// coin and the wrong one here: a mirrored A/B between close agents splits
    /// most maps, and a split scores exactly 0.5, so the realised per-map
    /// variance is a small fraction of the assumed `p(1-p)`. On a 120-map run
    /// where 103 maps split, the observed variance is about 0.05 against
    /// Wilson's assumed 0.25, and Wilson spans 46.5%..64.0% around a 55.4%
    /// mean — unable to clear parity however consistent the decisive maps are.
    ///
    /// This is the ordinary normal interval on the sample variance. The
    /// non-asymptotic alternative for bounded variables, the empirical
    /// Bernstein bound, was tried first and rejected by measurement: its
    /// additive `3 ln(3/delta) / n` term dominates at these sample sizes and
    /// made the interval *wider* than Wilson (0.32 against 0.18 at n=120), so
    /// it pays for its worst-case guarantee with exactly the width this is
    /// meant to remove. Coverage is therefore established by simulation rather
    /// than by a finite-sample proof — see
    /// `the_variance_adaptive_interval_covers_the_null_it_claims`, which
    /// checks it against the map shapes these runs actually produce.
    fn bootstrap_interval(scores: &[f64], seed: u64) -> (f64, f64) {
        let n = scores.len();
        if n < 2 {
            return (0.0, 1.0);
        }
        let mut rng = civvis::rng::Rng::new(seed);
        let mut means: Vec<f64> = (0..BOOTSTRAP_RESAMPLES)
            .map(|_| (0..n).map(|_| scores[rng.below(n)]).sum::<f64>() / n as f64)
            .collect();
        means.sort_by(|a, b| a.partial_cmp(b).expect("means are finite"));
        let low = means[(BOOTSTRAP_RESAMPLES as f64 * 0.025) as usize];
        let high =
            means[((BOOTSTRAP_RESAMPLES as f64 * 0.975) as usize).min(BOOTSTRAP_RESAMPLES - 1)];
        (low, high)
    }

    const BOOTSTRAP_RESAMPLES: usize = 2000;

    fn variance_adaptive_interval(scores: &[f64]) -> (f64, f64) {
        let n = scores.len();
        if n < 2 {
            return (0.0, 1.0);
        }
        let count = n as f64;
        let mean = scores.iter().sum::<f64>() / count;
        let variance = scores
            .iter()
            .map(|score| (score - mean) * (score - mean))
            .sum::<f64>()
            / (count - 1.0);
        let radius = Z_95 * (variance / count).sqrt();
        (
            (mean - radius).clamp(0.0, 1.0),
            (mean + radius).clamp(0.0, 1.0),
        )
    }

    #[test]
    fn confidence_uses_mirrored_maps_as_independent_observations() {
        let one_map = paired_inference(&[1.0]);
        let two_maps = paired_inference(&[1.0, 1.0]);
        assert_eq!(one_map.maps, 1);
        assert!(one_map.low < two_maps.low);
        assert!(one_map.high <= 1.0);
        assert_eq!(one_map.verdict, PromotionVerdict::Insufficient);
    }

    #[test]
    fn strong_replicated_edge_passes_promotion_gate() {
        let scores = vec![1.0; 30];
        let result = paired_inference(&scores);
        assert!(result.low > 0.5);
        assert!(result.anytime.challenger_p <= ANYTIME_TAIL_ALPHA);
        assert_eq!(
            result.anytime.challenger_crossed_at,
            Some(PROMOTION_MIN_MAPS)
        );
        assert_eq!(result.verdict, PromotionVerdict::Promote);
    }

    #[test]
    fn minimum_map_gate_overrides_an_early_clean_sweep() {
        let result = paired_inference(&vec![1.0; PROMOTION_MIN_MAPS - 1]);
        assert!(result.low > 0.5);
        assert_eq!(result.anytime.challenger_peak_e, 1.0);
        assert_eq!(result.anytime.challenger_p, 1.0);
        assert_eq!(result.verdict, PromotionVerdict::Insufficient);
    }

    #[test]
    fn decisive_incumbent_edge_retains_it() {
        let result = paired_inference(&vec![0.0; 30]);
        assert!(result.high < 0.5);
        assert!(result.anytime.incumbent_p <= ANYTIME_TAIL_ALPHA);
        assert_eq!(result.verdict, PromotionVerdict::Retain);
    }

    #[test]
    fn balanced_maps_are_inconclusive() {
        let scores: Vec<f64> = (0..40)
            .map(|index| if index % 2 == 0 { 1.0 } else { 0.0 })
            .collect();
        let result = paired_inference(&scores);
        assert!(result.low < 0.5 && result.high > 0.5);
        assert_eq!(result.anytime.challenger_p, 1.0);
        assert_eq!(result.anytime.incumbent_p, 1.0);
        assert_eq!(result.verdict, PromotionVerdict::Inconclusive);
    }

    #[test]
    fn neutral_maps_neither_spend_nor_create_betting_evidence() {
        let result = paired_inference(&vec![0.5; 100]);
        assert_eq!(result.anytime.challenger_peak_e, 1.0);
        assert_eq!(result.anytime.incumbent_peak_e, 1.0);
        assert_eq!(result.anytime.challenger_p, 1.0);
        assert_eq!(result.anytime.incumbent_p, 1.0);
        assert_eq!(result.verdict, PromotionVerdict::Inconclusive);
    }

    #[test]
    fn repeated_draw_mixed_edges_accumulate_bounded_score_evidence() {
        let result = paired_inference(&vec![0.75; 80]);
        assert!(result.anytime.challenger_p <= ANYTIME_TAIL_ALPHA);
        assert_eq!(result.anytime.incumbent_p, 1.0);
        assert_eq!(result.verdict, PromotionVerdict::Promote);
    }

    #[test]
    fn subminimum_lucky_prefix_cannot_bank_a_later_promotion() {
        let mut scores = vec![1.0; PROMOTION_MIN_MAPS / 2];
        scores.extend(vec![0.0; PROMOTION_MIN_MAPS / 2]);
        let result = paired_inference(&scores);
        assert_eq!(result.anytime.challenger_crossed_at, None);
        assert_eq!(result.anytime.challenger_p, 1.0);
        assert_eq!(result.verdict, PromotionVerdict::Inconclusive);
    }

    #[test]
    fn contradictory_anytime_crossings_flag_nonstationarity() {
        let mut scores = vec![1.0; 30];
        scores.extend(vec![0.0; 100]);
        let result = paired_inference(&scores);
        assert!(result.anytime.challenger_p <= ANYTIME_TAIL_ALPHA);
        assert!(result.anytime.incumbent_p <= ANYTIME_TAIL_ALPHA);
        assert_eq!(result.verdict, PromotionVerdict::Inconclusive);
    }

    #[test]
    fn elo_equivalent_is_symmetric_around_parity() {
        assert!((elo_edge(0.64) + elo_edge(0.36)).abs() < 1e-9);
        assert_eq!(elo_edge(0.5), 0.0);
    }

    #[test]
    fn pair_outcome_counts_keep_draw_mixed_maps_visible() {
        assert_eq!(
            pair_outcomes(&[1.0, 0.5, 0.0, 0.25, 0.75]),
            PairOutcomes {
                a_sweeps: 1,
                neutral: 1,
                b_sweeps: 1,
                mixed_with_draw: 2,
            }
        );
    }

    #[test]
    fn games_without_a_head_to_head_winner_are_draws() {
        let seats = ["challenger", "incumbent"];
        assert_eq!(game_score(Some(0), &seats, "challenger"), 1.0);
        assert_eq!(game_score(Some(1), &seats, "challenger"), 0.0);
        assert_eq!(game_score(None, &seats, "challenger"), 0.5);
        assert_eq!(game_score(Some(2), &seats, "challenger"), 0.5);
    }

    /// A threaded batch must produce the serial numbers exactly. Every game
    /// is determined by its seed and shares nothing mutable, and results are
    /// folded in index order, so the only thing `--jobs` may change is how
    /// long the run takes. If this ever fails, the evaluator has started
    /// reporting a different answer depending on the machine it ran on.
    /// A direction is only as strong as the number of maps that broke. The
    /// win statistic and the terminal-score statistic run on the same games
    /// and routinely rest on very different map counts, which is the fact
    /// that stops a 5-0 win margin from five maps reading as stronger than
    /// a 10-8 score margin from eighteen.
    /// Why the gate's conservatism was measured and then left alone.
    ///
    /// Under the null — mean exactly 0.5, with the map shape these runs
    /// produce — a 95% interval should contain 0.5 in about 95% of
    /// replications. Wilson contains it in *all* of them at 2.2x the width
    /// of the alternatives, and both natural alternatives land near 93.5%,
    /// slightly under nominal. So the conservatism is real and there is no
    /// drop-in replacement that is both narrower and calibrated. Recorded
    /// as an experiment rather than shipped as a statistic.
    #[test]
    fn no_narrower_interval_here_is_also_calibrated() {
        let mut rng = Rng::new(20_260_726);
        let mut covered = 0;
        let mut wilson_covered = 0;
        let mut eb_width = 0.0;
        let mut boot_covered = 0;
        let mut boot_width = 0.0;
        let mut wilson_width = 0.0;
        let trials = 400;
        let maps = 120;
        for _ in 0..trials {
            // The observed shape: most maps split, the rest break evenly
            // either way, so the mean is 0.5 under the null.
            let scores: Vec<f64> = (0..maps)
                .map(|_| match rng.below(10) {
                    0 => 1.0,
                    1 => 0.0,
                    _ => 0.5,
                })
                .collect();
            let (low, high) = variance_adaptive_interval(&scores);
            if low <= 0.5 && 0.5 <= high {
                covered += 1;
            }
            eb_width += high - low;
            let (blow, bhigh) = bootstrap_interval(&scores, 900 + covered as u64);
            if blow <= 0.5 && 0.5 <= bhigh {
                boot_covered += 1;
            }
            boot_width += bhigh - blow;
            let inference = paired_inference(&scores);
            if inference.low <= 0.5 && 0.5 <= inference.high {
                wilson_covered += 1;
            }
            wilson_width += inference.high - inference.low;
        }
        println!(
            "normal/sample-variance: {covered}/{trials} covered, mean width {:.4}",
            eb_width / trials as f64
        );
        println!(
            "bootstrap percentile:   {boot_covered}/{trials} covered, mean width {:.4}",
            boot_width / trials as f64
        );
        println!(
            "wilson:                 {wilson_covered}/{trials} covered, mean width {:.4}",
            wilson_width / trials as f64
        );
        // The finding, pinned so it is not rediscovered: on the map shape
        // these runs produce, Wilson covers every replication — it is not
        // 95% conservative, it is total — at 2.2x the width of either
        // variance-adaptive alternative, and both of those land slightly
        // *under* nominal rather than at it. There is no drop-in narrower
        // interval here that is also calibrated.
        assert_eq!(
            wilson_covered, trials,
            "Wilson is expected to cover every replication on this shape"
        );
        assert!(
            covered * 100 < trials * 95,
            "the normal interval undercovered when measured: {covered}/{trials}"
        );
        assert!(
            boot_covered * 100 < trials * 95,
            "the bootstrap interval undercovered when measured: {boot_covered}/{trials}"
        );
        assert!(
            eb_width * 2.0 < wilson_width,
            "the adaptive intervals should be far narrower: {:.4} against {:.4}",
            eb_width / trials as f64,
            wilson_width / trials as f64
        );
    }

    /// It must not narrow when the data really is maximum-variance: an
    /// interval that only ever shrinks is not adapting, it is broken.
    #[test]
    fn the_variance_adaptive_interval_stays_wide_on_coin_flips() {
        let mut rng = Rng::new(7);
        let scores: Vec<f64> = (0..120).map(|_| rng.below(2) as f64).collect();
        let (low, high) = variance_adaptive_interval(&scores);
        assert!(
            high - low > 0.15,
            "coin flips gave a {:.3}-wide interval",
            high - low
        );
        assert!(low <= 0.5 && 0.5 <= high);
    }

    #[test]
    fn resolution_counts_only_the_maps_that_broke() {
        let mostly_neutral = [0.5, 0.5, 0.5, 1.0, 0.5, 0.0];
        let directions = directional_outcomes(&mostly_neutral);
        assert_eq!(directions.neutral, 4);
        assert_eq!(resolved_maps(&directions), 2);
        assert_eq!(
            direction_sign(&directions),
            None,
            "one each way is no direction"
        );

        let decisive = [1.0, 1.0, 1.0, 0.5, 0.0];
        assert_eq!(resolved_maps(&directional_outcomes(&decisive)), 4);
        assert_eq!(direction_sign(&directional_outcomes(&decisive)), Some(true));

        let against = [0.0, 0.0, 0.5];
        assert_eq!(direction_sign(&directional_outcomes(&against)), Some(false));
        assert_eq!(direction_sign(&directional_outcomes(&[0.5, 0.5])), None);
        assert_eq!(resolved_maps(&directional_outcomes(&[])), 0);
    }

    #[test]
    fn parallel_batches_match_a_serial_run() {
        let play = |jobs: usize| {
            civvis::parallel::map(4, jobs, |index| {
                let seed = 52_000 + index as u64 / 2;
                let swap = index % 2;
                let seats: Vec<&str> = (0..2)
                    .map(|pid| {
                        if (pid + swap) % 2 == 0 {
                            "advanced"
                        } else {
                            "basic"
                        }
                    })
                    .collect();
                let mut game = Game::new(2, 20, 14, seed, 40, 0);
                let mut ais: Vec<Box<dyn Ai>> = game
                    .players
                    .iter()
                    .map(|p| {
                        let name = if p.id < 2 { seats[p.id] } else { "basic" };
                        civvis::elo::builtin_ai(name, seed + p.id as u64)
                    })
                    .collect();
                run_traced_game(&mut game, &mut ais, 2);
                (
                    game.turn,
                    game.winner,
                    game_score(game.winner, &seats, "advanced"),
                )
            })
        };
        assert_eq!(play(1), play(4));
    }

    #[test]
    fn terminal_score_share_is_bounded_symmetric_and_independent_of_winner() {
        let mut game = Game::new(2, 20, 14, 71, 40, 0);
        let seats = ["challenger", "incumbent"];
        // There is already score on the board at turn 0: a start whose units
        // can see a Natural Wonder is paid Era Score for finding it, and which
        // start that is belongs to the map. Level the seats so this measures
        // the share function rather than the generator that fed it.
        for pid in 0..seats.len() {
            game.players[pid].era_score = 0;
        }
        let baseline = terminal_score_share(&game, &seats, "challenger");
        assert!((baseline - 0.5).abs() < 1e-12);

        game.players[0].techs.insert(civvis::name!("writing"));
        game.winner = Some(1);
        let challenger = terminal_score_share(&game, &seats, "challenger");
        let incumbent = terminal_score_share(&game, &seats, "incumbent");
        assert!(challenger > baseline);
        assert!((challenger + incumbent - 1.0).abs() < 1e-12);
    }

    #[test]
    fn plan_trace_counts_exposure_and_switches() {
        let mut trace = PlanTrace::default();
        for (target, strategy, rush) in [
            ("adaptive", "expansion", false),
            ("religion", "religion", true),
            ("religion", "religion", true),
            ("adaptive", "science", false),
        ] {
            trace.observe(PlanObservation {
                target,
                strategy,
                reason: "test reason",
                rush,
                context: StrategyContext {
                    at_major_war: false,
                    threatened: false,
                    city_deficit: false,
                },
                midgame: false,
            });
        }
        assert_eq!(trace.observations, 4);
        assert_eq!(trace.switches, 2);
        assert_eq!(trace.strategy_switches, 2);
        assert_eq!(trace.rush_observations, 2);
        assert!(trace.ever_rushed);
        assert_eq!(trace.targets["adaptive"], 2);
        assert_eq!(trace.targets["religion"], 2);
        assert_eq!(trace.strategy_turns["religion"], 2);
        assert_eq!(trace.dominant_target(), "adaptive");
    }

    #[test]
    fn midgame_strategy_switches_separate_visible_boundaries_from_the_residual() {
        let mut trace = PlanTrace::default();
        for (strategy, at_major_war, threatened, city_deficit) in [
            ("expansion", false, false, true),
            ("conquest", true, false, true),
            ("recovery", true, true, true),
            ("conquest", true, true, true),
        ] {
            trace.observe(PlanObservation {
                target: "adaptive",
                strategy,
                reason: strategy,
                rush: false,
                context: StrategyContext {
                    at_major_war,
                    threatened,
                    city_deficit,
                },
                midgame: true,
            });
        }

        assert_eq!(trace.switches, 0, "the assigned target never changed");
        assert_eq!(trace.strategy_switches, 3);
        assert_eq!(trace.midgame_strategy_switches, 3);
        assert_eq!(trace.midgame_boundary_switches, 2);
        assert_eq!(trace.midgame_unanchored_switches, 1);
        assert_eq!(trace.midgame_war_boundary_switches, 1);
        assert_eq!(trace.midgame_threat_boundary_switches, 1);
        assert_eq!(trace.midgame_city_deficit_boundary_switches, 0);
        assert_eq!(trace.midgame_transitions["expansion->conquest"], 1);
        assert_eq!(trace.midgame_transitions["conquest->recovery"], 1);
        assert_eq!(trace.midgame_transitions["recovery->conquest"], 1);
    }

    #[test]
    fn assessment_trace_separates_reason_boundaries_from_same_reason_argmax_changes() {
        let mut trace = PlanTrace::default();
        for (strategy, reason, at_major_war) in [
            ("science", "its best available victory lane", false),
            ("culture", "its best available victory lane", false),
            ("expansion", "short of cities with land still open", false),
            ("science", "its best available victory lane", true),
            ("culture", "its best available victory lane", true),
        ] {
            trace.observe(PlanObservation {
                target: "adaptive",
                strategy,
                reason,
                rush: false,
                context: StrategyContext {
                    at_major_war,
                    threatened: false,
                    city_deficit: false,
                },
                midgame: true,
            });
        }

        let best_lane = "its best available victory lane";
        let short = "short of cities with land still open";
        assert_eq!(trace.midgame_strategy_switches, 4);
        assert_eq!(trace.midgame_unanchored_switches, 3);
        assert_eq!(trace.midgame_reason_turns[best_lane], 4);
        assert_eq!(
            trace.midgame_reason_transitions[&ordered_transition(best_lane, short)],
            1
        );
        assert_eq!(
            trace.midgame_reason_transitions[&ordered_transition(short, best_lane)],
            1
        );
        assert_eq!(
            trace.midgame_unanchored_reason_transitions[&ordered_transition(best_lane, short)],
            1
        );
        assert_eq!(
            trace.midgame_unanchored_same_reason_transitions
                ["science->culture under its best available victory lane"],
            2
        );
        assert_eq!(
            trace.midgame_unanchored_reason_families[&reason_family(best_lane, best_lane)],
            2
        );
        assert_eq!(
            trace.midgame_unanchored_reason_families[&reason_family(best_lane, short)],
            1
        );
    }

    #[test]
    fn overlapping_visible_boundaries_are_one_switch_but_retain_each_component() {
        let mut trace = PlanTrace::default();
        for (strategy, reason, context) in [
            (
                "expansion",
                "short of cities with land still open",
                StrategyContext {
                    at_major_war: false,
                    threatened: false,
                    city_deficit: true,
                },
            ),
            (
                "recovery",
                "at war and losing ground at home",
                StrategyContext {
                    at_major_war: true,
                    threatened: true,
                    city_deficit: false,
                },
            ),
        ] {
            trace.observe(PlanObservation {
                target: "adaptive",
                strategy,
                reason,
                rush: false,
                context,
                midgame: true,
            });
        }

        assert_eq!(trace.midgame_strategy_switches, 1);
        assert_eq!(trace.midgame_boundary_switches, 1);
        assert_eq!(trace.midgame_unanchored_switches, 0);
        assert_eq!(trace.midgame_war_boundary_switches, 1);
        assert_eq!(trace.midgame_threat_boundary_switches, 1);
        assert_eq!(trace.midgame_city_deficit_boundary_switches, 1);
        assert!(trace.midgame_unanchored_reason_families.is_empty());
    }

    #[test]
    fn assessment_family_coverage_counts_each_seat_once_and_ranks_ties_by_label() {
        let family = reason_family("already at war", "at war and losing ground at home");
        let mut first = PlanTrace::default();
        first
            .midgame_unanchored_reason_families
            .insert(family.clone(), 3);
        let mut second = PlanTrace::default();
        second
            .midgame_unanchored_reason_families
            .insert(family.clone(), 1);
        second
            .midgame_unanchored_reason_families
            .insert("alpha <-> zeta".to_string(), 4);

        let mut metrics = Metrics::default();
        metrics.record_assessment_trace(&first);
        metrics.record_assessment_trace(&second);

        assert_eq!(metrics.midgame_unanchored_reason_families[&family], 4);
        assert_eq!(metrics.midgame_unanchored_reason_family_seats[&family], 2);
        assert_eq!(
            family_counts(
                &metrics.midgame_unanchored_reason_families,
                &metrics.midgame_unanchored_reason_family_seats,
            ),
            format!(
                "alpha <-> zeta 4 across 1 seat-games, {family} 4 across 2 seat-games"
            )
        );
    }

    #[test]
    fn trigger_gate_requires_the_global_dominant_family_to_be_elective() {
        let urgent = reason_family("already at war", "at war and losing ground at home");
        let elective = reason_family(
            "already well down its best victory lane",
            "its best available victory lane",
        );
        let mut metrics = Metrics {
            games: 4,
            midgame_unanchored_switches: 8,
            ..Metrics::default()
        };
        metrics
            .midgame_unanchored_reason_families
            .insert(urgent.clone(), 5);
        metrics
            .midgame_unanchored_reason_family_seats
            .insert(urgent.clone(), 3);
        metrics
            .midgame_unanchored_reason_families
            .insert(elective.clone(), 3);
        metrics
            .midgame_unanchored_reason_family_seats
            .insert(elective, 2);

        let gate = trigger_gate(&metrics).unwrap();
        assert_eq!(gate.family, urgent);
        assert!(!gate.eligible);
        assert!(!gate.passes(), "an elective runner-up cannot be selected");
    }

    #[test]
    fn trigger_gate_automates_all_three_preregistered_thresholds() {
        let family = reason_family(
            "already well down its best victory lane",
            "its best available victory lane",
        );
        let make_metrics = |occurrences, total, seats| {
            let mut metrics = Metrics {
                games: 4,
                midgame_unanchored_switches: total,
                ..Metrics::default()
            };
            metrics
                .midgame_unanchored_reason_families
                .insert(family.clone(), occurrences);
            metrics
                .midgame_unanchored_reason_family_seats
                .insert(family.clone(), seats);
            metrics
        };

        assert!(trigger_gate(&make_metrics(3, 10, 1)).unwrap().passes());
        assert!(!trigger_gate(&make_metrics(2, 10, 1)).unwrap().passes());
        assert!(!trigger_gate(&make_metrics(2, 4, 1)).unwrap().passes());
        assert!(!trigger_gate(&make_metrics(3, 10, 0)).unwrap().passes());
    }

    #[test]
    fn trigger_gate_breaks_dominant_count_ties_by_family_label() {
        let mut metrics = Metrics {
            games: 4,
            midgame_unanchored_switches: 6,
            ..Metrics::default()
        };
        for family in ["zeta <-> zeta", "alpha <-> alpha"] {
            metrics
                .midgame_unanchored_reason_families
                .insert(family.to_string(), 3);
            metrics
                .midgame_unanchored_reason_family_seats
                .insert(family.to_string(), 2);
        }
        assert_eq!(trigger_gate(&metrics).unwrap().family, "alpha <-> alpha");
    }

    #[test]
    fn empty_plan_trace_is_explicitly_unreported() {
        assert_eq!(PlanTrace::default().dominant_target(), "unreported");
    }

    #[test]
    fn traced_loop_preserves_headless_game_result() {
        let make_game = || Game::new(2, 16, 12, 9123, 30, 0);
        let mut plain = make_game();
        let mut traced = make_game();
        let mut plain_ais: Vec<Box<dyn Ai>> = (0..plain.players.len())
            .map(|pid| builtin_ai("basic", pid as u64 + 1))
            .collect();
        let mut traced_ais: Vec<Box<dyn Ai>> = (0..traced.players.len())
            .map(|pid| builtin_ai("basic", pid as u64 + 1))
            .collect();

        civvis::ai::run_game(&mut plain, &mut plain_ais);
        let traces = run_traced_game(&mut traced, &mut traced_ais, 2);

        assert_eq!(traced.winner, plain.winner);
        assert_eq!(traced.victory_type, plain.victory_type);
        assert_eq!(traced.turn, plain.turn);
        assert_eq!(traced.score(0), plain.score(0));
        assert_eq!(traced.score(1), plain.score(1));
        assert!(traces.iter().all(|trace| trace.observations > 0));
    }

    #[test]
    fn exact_sign_test_detects_replicated_map_direction() {
        let mut scores = vec![1.0; 8];
        scores.extend(vec![0.5; 16]);
        scores.push(0.0);
        let outcomes = directional_outcomes(&scores);
        assert_eq!(
            outcomes,
            DirectionalOutcomes {
                challenger_favored: 8,
                neutral: 16,
                incumbent_favored: 1,
            }
        );
        assert!((exact_sign_p(8, 1) - 0.039_062_5).abs() < 1e-12);
        assert_eq!(exact_sign_p(1, 8), exact_sign_p(8, 1));
    }

    #[test]
    fn sign_test_keeps_neutral_and_balanced_maps_inconclusive() {
        assert_eq!(directional_outcomes(&[0.5; 20]).neutral, 20);
        assert_eq!(exact_sign_p(0, 0), 1.0);
        assert_eq!(exact_sign_p(4, 4), 1.0);
    }

    #[test]
    fn draw_mixed_maps_still_have_a_direction() {
        assert_eq!(
            directional_outcomes(&[0.75, 0.25, 0.5]),
            DirectionalOutcomes {
                challenger_favored: 1,
                neutral: 1,
                incumbent_favored: 1,
            }
        );
    }
}
