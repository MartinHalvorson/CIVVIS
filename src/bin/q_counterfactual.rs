//! Counterfactual returns for destinations the expert did and did not choose.
//!
//! `q_dataset` can prove that an encoding represents `AdvancedAi`'s policy,
//! but its sibling rows necessarily inherit the chosen action's outcome. They
//! cannot answer the causal question a stronger policy needs: what would have
//! happened if this same unit had moved somewhere else?
//!
//! This instrument observes one real `AdvancedAi` turn on a clone, replays its
//! successful action prefix to the first move with a same-unit alternative,
//! then branches the exact pre-action game. Every candidate starts with the
//! same game, the same policy memory, and the same matched opponent doctrine.
//! A resolved continuation returns 1/0; an unresolved one returns score share
//! among living major civilizations, matching the shipped strategic search.
//!
//! The simulation is deterministic. Replicas therefore vary bounded opponent
//! doctrines rather than pretending repeated identical continuations are
//! independent evidence. The chosen branch's first replica is repeated and
//! compared exactly; any disagreement makes the dataset fail closed.
//!
//! ```text
//! q_counterfactual --games 8 --players 4 --warmup 60 --horizon 80 \
//!   --alternatives 3 --replicas 4 --out /tmp/q-counterfactual.csv
//! ```
use civvis::action_space;
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::decision_features::{decision_features, WIDTH as STATE_WIDTH};
use civvis::game::{default_speed, Action, Game, GameOptions};
use civvis::parallel;
use civvis::strategic::Doctrine;
use civvis::Pos;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;

const DISCRIMINATING_SPREAD: f64 = 0.005;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

#[derive(Clone)]
struct ObservedDecision {
    game: Game,
    fleet: Vec<AdvancedAi>,
    game_id: u64,
    seat: usize,
    unit: u32,
    candidates: Vec<Action>,
    state_features: Vec<f32>,
    action_features: Vec<Vec<f32>>,
}

#[derive(Clone, Debug, PartialEq)]
struct BranchOutcome {
    value: f64,
    winner: Option<usize>,
    turn: u32,
    scores: Vec<i64>,
}

struct DecisionHarvest {
    rows: String,
    candidates: usize,
    branches: usize,
    resolved: usize,
    spread: f64,
    regret: f64,
    chosen_best: bool,
    robust_better: bool,
    mean_best_vote_share: f64,
    rejected: usize,
    nondeterministic: usize,
}

#[derive(Default)]
struct GameHarvest {
    rows: String,
    decisions: Vec<DecisionHarvest>,
    observed_without_choice: usize,
    errors: Vec<String>,
}

fn objectives(g: &Game, ai: &AdvancedAi) -> Vec<Pos> {
    let mut positions = Vec::new();
    if let Some(report) = ai.plan_report() {
        positions.extend(report.forces.into_iter().map(|force| force.objective));
        for city in [report.threatened_city, report.target_city]
            .into_iter()
            .flatten()
        {
            if let Some(city) = g.cities.get(&city) {
                positions.push(city.pos);
            }
        }
    }
    positions.sort_unstable();
    positions.dedup();
    positions
}

/// Chosen first, then an evenly spaced deterministic sample of legal siblings.
fn move_candidates(
    legal: &[Action],
    chosen: &Action,
    alternatives: usize,
    salt: u64,
) -> Vec<Action> {
    let Some(actor) = action_space::acting_unit(chosen) else {
        return Vec::new();
    };
    if action_space::kind_name(chosen) != "move" || !legal.contains(chosen) {
        return Vec::new();
    }
    let others: Vec<&Action> = legal
        .iter()
        .filter(|candidate| *candidate != chosen)
        .filter(|candidate| action_space::kind_name(candidate) == "move")
        .filter(|candidate| action_space::acting_unit(candidate) == Some(actor))
        .collect();
    if others.is_empty() || alternatives == 0 {
        return Vec::new();
    }
    let take = alternatives.min(others.len());
    let start = (salt.rotate_left(17) as usize) % others.len();
    let mut selected = Vec::with_capacity(take + 1);
    selected.push(chosen.clone());
    for index in 0..take {
        let offset = index * others.len() / take;
        selected.push(others[(start + offset) % others.len()].clone());
    }
    selected
}

/// Observe without mutating the harvested trajectory, then reproduce the exact
/// pre-action state by replaying only successful actions from this turn.
fn observe_move(
    game: &Game,
    fleet: &[AdvancedAi],
    game_id: u64,
    alternatives: usize,
) -> Result<Option<ObservedDecision>, String> {
    let seat = game.current;
    if seat >= fleet.len() || !game.players[seat].alive {
        return Ok(None);
    }
    let mut observed = game.clone();
    let before = observed.log.iter().count();
    let mut actor = fleet[seat].clone();
    actor.take_turn(&mut observed, seat);
    let plan = objectives(&observed, &actor);
    let actions: Vec<(usize, Action)> = observed.log.iter().skip(before).cloned().collect();
    let mut replay = game.clone();

    for (owner, action) in actions {
        if owner != seat {
            return Err(format!(
                "game {game_id} turn {}: observed seat {seat} logged action for seat {owner}",
                game.turn
            ));
        }
        if action_space::kind_name(&action) == "move" {
            let legal = replay.legal_actions(seat);
            let candidates = move_candidates(
                &legal,
                &action,
                alternatives,
                game_id
                    ^ u64::from(replay.turn)
                    ^ u64::from(action_space::acting_unit(&action).unwrap_or(0)),
            );
            if candidates.len() >= 2 {
                let unit = action_space::acting_unit(&action).expect("move has an actor");
                let context = action_space::FeatureContext::new(&replay, seat, &plan);
                let action_features = candidates
                    .iter()
                    .map(|candidate| {
                        action_space::features_with_context(&replay, seat, candidate, &context)
                    })
                    .collect();
                return Ok(Some(ObservedDecision {
                    game: replay.clone(),
                    fleet: fleet.to_vec(),
                    game_id,
                    seat,
                    unit,
                    candidates,
                    state_features: decision_features(&replay, seat),
                    action_features,
                }));
            }
        }
        replay.apply(seat, &action).map_err(|error| {
            format!(
                "game {game_id} turn {}: observed action prefix did not replay: {error}; {action:?}",
                game.turn
            )
        })?;
    }
    Ok(None)
}

fn score_share(g: &Game, seat: usize) -> f64 {
    if !g.players.get(seat).is_some_and(|player| player.alive) {
        return 0.0;
    }
    let mut own = 0.0;
    let mut total = 0.0;
    for player in &g.players {
        if player.is_minor || player.is_barbarian || !player.alive {
            continue;
        }
        let score = g.score(player.id).max(0) as f64;
        total += score;
        if player.id == seat {
            own = score;
        }
    }
    if total > 0.0 {
        own / total
    } else {
        0.5
    }
}

fn rollout(
    decision: &ObservedDecision,
    candidate: &Action,
    horizon: u32,
    replica: usize,
) -> Result<BranchOutcome, String> {
    let mut sim = decision.game.clone();
    sim.set_fog_memory(false);
    sim.apply(decision.seat, candidate).map_err(|error| {
        format!(
            "game {} turn {} candidate rejected: {error}; {candidate:?}",
            decision.game_id, decision.game.turn
        )
    })?;
    let mut fleet = decision.fleet.clone();
    let baseline = Weights::default();
    for player in &sim.players {
        if player.id == decision.seat || player.is_minor || player.is_barbarian {
            continue;
        }
        let doctrine = Doctrine::ALL[(replica + player.id) % Doctrine::ALL.len()];
        fleet[player.id].reweight(doctrine.apply(&baseline));
    }
    let stop = sim.turn.saturating_add(horizon.max(1));
    while sim.winner.is_none() && sim.turn < stop && sim.turn <= sim.max_turns {
        let current = sim.current;
        fleet[current].take_turn(&mut sim, current);
        if sim.winner.is_none() && sim.current == current {
            let _ = sim.apply(current, &Action::EndTurn);
        }
    }
    let value = match sim.winner {
        Some(winner) if winner == decision.seat => 1.0,
        Some(_) => 0.0,
        None => score_share(&sim, decision.seat),
    };
    Ok(BranchOutcome {
        value,
        winner: sim.winner,
        turn: sim.turn,
        scores: sim
            .players
            .iter()
            .map(|player| sim.score(player.id))
            .collect(),
    })
}

fn write_row(out: &mut String, decision: &ObservedDecision, candidate: usize, returns: &[f64]) {
    let mean = returns.iter().sum::<f64>() / returns.len().max(1) as f64;
    let _ = write!(
        out,
        "{},{},{},{},{}",
        decision.game_id,
        decision.game.turn,
        decision.seat,
        decision.unit,
        (candidate == 0) as u8
    );
    for value in decision
        .state_features
        .iter()
        .chain(&decision.action_features[candidate])
    {
        let _ = write!(out, ",{value:.6}");
    }
    let _ = write!(out, ",{mean:.8}");
    for value in returns {
        let _ = write!(out, ",{value:.8}");
    }
    out.push('\n');
}

fn harvest_decision(decision: &ObservedDecision, horizon: u32, replicas: usize) -> DecisionHarvest {
    let replicas = replicas.max(1);
    let mut outcomes = vec![vec![0.0; replicas]; decision.candidates.len()];
    let mut rows = String::new();
    let mut branches = 0;
    let mut resolved = 0;
    let mut rejected = 0;
    let mut nondeterministic = 0;
    for (candidate_index, (candidate, returns)) in
        decision.candidates.iter().zip(&mut outcomes).enumerate()
    {
        for (replica, value) in returns.iter_mut().enumerate() {
            match rollout(decision, candidate, horizon, replica) {
                Ok(outcome) => {
                    *value = outcome.value;
                    branches += 1;
                    resolved += outcome.winner.is_some() as usize;
                    if candidate_index == 0 && replica == 0 {
                        match rollout(decision, candidate, horizon, replica) {
                            Ok(repeated) if repeated == outcome => {}
                            Ok(_) => nondeterministic += 1,
                            Err(_) => rejected += 1,
                        }
                    }
                }
                Err(_) => rejected += 1,
            }
        }
    }

    let means: Vec<f64> = outcomes
        .iter()
        .map(|values| values.iter().sum::<f64>() / replicas as f64)
        .collect();
    let low = means.iter().copied().fold(f64::INFINITY, f64::min);
    let high = means.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let best = means
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(right.1).then_with(|| right.0.cmp(&left.0)))
        .map(|(index, _)| index)
        .unwrap_or(0);
    let robust_better = (1..means.len()).any(|candidate| {
        (0..replicas).all(|replica| outcomes[candidate][replica] > outcomes[0][replica])
    });
    let best_votes = (0..replicas)
        .filter(|replica| {
            outcomes[best][*replica]
                >= (0..outcomes.len())
                    .map(|candidate| outcomes[candidate][*replica])
                    .fold(f64::NEG_INFINITY, f64::max)
        })
        .count();
    for (candidate, returns) in outcomes.iter().enumerate() {
        write_row(&mut rows, decision, candidate, returns);
    }
    DecisionHarvest {
        rows,
        candidates: decision.candidates.len(),
        branches,
        resolved,
        spread: high - low,
        regret: high - means[0],
        chosen_best: (means[0] - high).abs() <= 1e-9,
        robust_better,
        mean_best_vote_share: best_votes as f64 / replicas as f64,
        rejected,
        nondeterministic,
    }
}

fn step(game: &mut Game, fleet: &mut [AdvancedAi]) {
    let current = game.current;
    fleet[current].take_turn(game, current);
    if game.winner.is_none() && game.current == current {
        let _ = game.apply(current, &Action::EndTurn);
    }
}

#[allow(clippy::too_many_arguments)]
fn harvest_game(
    game_id: u64,
    seat: usize,
    players: usize,
    width: i32,
    height: i32,
    turns: u32,
    city_states: usize,
    speed: &str,
    warmup: u32,
    spacing: u32,
    decisions: usize,
    alternatives: usize,
    horizon: u32,
    replicas: usize,
) -> GameHarvest {
    let options = GameOptions {
        speed: speed.to_string(),
        ..GameOptions::new(players, width, height, game_id, turns, city_states)
    };
    let mut game = Game::new_with(options);
    game.set_fog_memory(false);
    let mut fleet = AdvancedAi::fleet(&game);
    let mut harvest = GameHarvest::default();

    for sample in 0..decisions {
        let target = warmup.saturating_add(spacing.saturating_mul(sample as u32));
        while game.winner.is_none()
            && game.turn <= game.max_turns
            && game.players[seat].alive
            && (game.turn < target || game.current != seat)
        {
            step(&mut game, &mut fleet);
        }
        if game.winner.is_some() || game.turn > game.max_turns || !game.players[seat].alive {
            break;
        }
        match observe_move(&game, &fleet, game_id, alternatives) {
            Ok(Some(decision)) => {
                let measured = harvest_decision(&decision, horizon, replicas);
                harvest.rows.push_str(&measured.rows);
                harvest.decisions.push(measured);
            }
            Ok(None) => harvest.observed_without_choice += 1,
            Err(error) => harvest.errors.push(error),
        }
        // Advance at least one complete world turn before another sample even
        // when spacing is zero; duplicate positions are not new evidence.
        let next = game.turn.saturating_add(spacing.max(1));
        while game.winner.is_none() && game.turn < next {
            step(&mut game, &mut fleet);
        }
    }
    harvest
}

fn header(replicas: usize) -> String {
    let mut out = String::from("game,turn,seat,unit,chosen");
    for index in 0..STATE_WIDTH {
        let _ = write!(out, ",s{index}");
    }
    for index in 0..action_space::FEATURE_WIDTH {
        let _ = write!(out, ",a{index}");
    }
    out.push_str(",return");
    for replica in 0..replicas.max(1) {
        let _ = write!(out, ",r{replica}");
    }
    out.push('\n');
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let games = number(&args, "--games", 8);
    let players = number(&args, "--players", 4);
    let width = number(&args, "--width", 44) as i32;
    let height = number(&args, "--height", 28) as i32;
    let turns = number(&args, "--turns", 200) as u32;
    let city_states = number(&args, "--city-states", 0);
    let speed = text(&args, "--speed", &default_speed());
    let warmup = number(&args, "--warmup", 60) as u32;
    let spacing = number(&args, "--spacing", 20) as u32;
    let decisions = number(&args, "--decisions-per-game", 1);
    let alternatives = number(&args, "--alternatives", 3);
    let horizon = number(&args, "--horizon", 80).max(1) as u32;
    let replicas = number(&args, "--replicas", Doctrine::ALL.len()).max(1);
    let seed = number(&args, "--seed", 940_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let out = text(&args, "--out", "/tmp/q-counterfactual.csv");
    if games == 0 || players < 2 || alternatives == 0 || decisions == 0 {
        eprintln!(
            "q_counterfactual: games, alternatives, and decisions must be positive; players >= 2"
        );
        std::process::exit(2);
    }
    println!(
        "q_counterfactual: {games} games, {players}p {width}x{height}, {speed}, \
         warmup {warmup}, {decisions} decision(s)/game every {spacing} turns, \
         {alternatives} alternatives, horizon {horizon}, {replicas} matched replicas, jobs {jobs}"
    );

    let harvests = parallel::map(games, jobs, |index| {
        let game_id = seed + index as u64;
        harvest_game(
            game_id,
            index % players,
            players,
            width,
            height,
            turns,
            city_states,
            &speed,
            warmup,
            spacing,
            decisions,
            alternatives,
            horizon,
            replicas,
        )
    });

    let mut samples = 0usize;
    let mut candidates = 0usize;
    let mut branches = 0usize;
    let mut resolved = 0usize;
    let mut discriminating = 0usize;
    let mut chosen_best = 0usize;
    let mut robust_better = 0usize;
    let mut rejected = 0usize;
    let mut nondeterministic = 0usize;
    let mut missed = 0usize;
    let mut spread = 0.0;
    let mut regret = 0.0;
    let mut vote_share = 0.0;
    let mut errors = Vec::new();
    for game in &harvests {
        missed += game.observed_without_choice;
        errors.extend(game.errors.iter().cloned());
        for decision in &game.decisions {
            samples += 1;
            candidates += decision.candidates;
            branches += decision.branches;
            resolved += decision.resolved;
            discriminating += (decision.spread > DISCRIMINATING_SPREAD) as usize;
            chosen_best += decision.chosen_best as usize;
            robust_better += decision.robust_better as usize;
            rejected += decision.rejected;
            nondeterministic += decision.nondeterministic;
            spread += decision.spread;
            regret += decision.regret;
            vote_share += decision.mean_best_vote_share;
        }
    }
    println!(
        "harvested {samples} decisions, {candidates} candidate rows, {branches} continuations"
    );
    println!("observation: {missed} sampled turns had no move with a same-unit alternative");
    println!(
        "label spread: {discriminating}/{samples} decisions above {DISCRIMINATING_SPREAD:.3} \
         ({:.1}%), mean {:.4}",
        100.0 * discriminating as f64 / samples.max(1) as f64,
        spread / samples.max(1) as f64
    );
    println!(
        "expert choice: best mean return in {chosen_best}/{samples} ({:.1}%); mean oracle regret {:.4}; \
         a sibling beat it in every replica at {robust_better}/{samples} decisions",
        100.0 * chosen_best as f64 / samples.max(1) as f64,
        regret / samples.max(1) as f64
    );
    println!(
        "replication: mean-return winner also tied/won {:.1}% of doctrine replicas; \
         {resolved}/{branches} branches resolved to a victory",
        100.0 * vote_share / samples.max(1) as f64
    );
    println!(
        "integrity: {rejected} rejected candidate branches, {nondeterministic} repeated-branch mismatches, \
         {} observation errors",
        errors.len()
    );
    for error in errors.iter().take(5) {
        eprintln!("  {error}");
    }
    if samples == 0 || rejected > 0 || nondeterministic > 0 || !errors.is_empty() {
        eprintln!("q_counterfactual: integrity gate failed; refusing to write a dataset");
        std::process::exit(2);
    }
    let mut file = fs::File::create(&out).unwrap_or_else(|error| {
        eprintln!("q_counterfactual: cannot create {out}: {error}");
        std::process::exit(2);
    });
    file.write_all(header(replicas).as_bytes()).unwrap();
    for game in &harvests {
        file.write_all(game.rows.as_bytes()).unwrap();
    }
    println!("wrote {out}");
}

#[cfg(test)]
mod tests {
    use super::{harvest_game, header, move_candidates, observe_move, rollout};
    use civvis::ai::{AdvancedAi, Ai};
    use civvis::game::{Action, Game};

    fn first_move_choice(game: &Game, seat: usize) -> Option<(Action, Vec<Action>)> {
        let legal = game.legal_actions(seat);
        legal.iter().find_map(|chosen| {
            let candidates = move_candidates(&legal, chosen, 3, 7);
            (candidates.len() >= 2).then(|| (chosen.clone(), candidates))
        })
    }

    #[test]
    fn sampled_candidates_are_legal_moves_for_one_unit() {
        let game = Game::new(3, 24, 16, 71, 40, 0);
        let (chosen, candidates) = first_move_choice(&game, 0).expect("starting unit can move");
        let actor = civvis::action_space::acting_unit(&chosen);
        assert_eq!(candidates[0], chosen);
        assert!(candidates.iter().all(|candidate| {
            game.legal_actions(0).contains(candidate)
                && civvis::action_space::kind_name(candidate) == "move"
                && civvis::action_space::acting_unit(candidate) == actor
        }));
    }

    #[test]
    fn an_observed_branch_repeats_exactly() {
        let mut game = Game::new(3, 24, 16, 712, 40, 0);
        game.set_fog_memory(false);
        let mut fleet = AdvancedAi::fleet(&game);
        let mut decision = None;
        for _ in 0..8 {
            if game.current == 0 {
                decision = observe_move(&game, &fleet, 712, 2).unwrap();
                if decision.is_some() {
                    break;
                }
            }
            let current = game.current;
            fleet[current].take_turn(&mut game, current);
            if game.winner.is_none() && game.current == current {
                game.apply(current, &Action::EndTurn).unwrap();
            }
        }
        let decision = decision.expect("AdvancedAi exposes a move choice");
        let first = rollout(&decision, &decision.candidates[0], 3, 0).unwrap();
        let second = rollout(&decision, &decision.candidates[0], 3, 0).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn harvest_rows_match_the_declared_schema() {
        let run = harvest_game(919, 0, 3, 24, 16, 40, 0, "standard", 3, 1, 1, 2, 2, 1);
        assert!(run.errors.is_empty(), "{:?}", run.errors);
        assert!(!run.decisions.is_empty(), "pilot found no decision");
        let columns = header(1).trim_end().split(',').count();
        for row in run.rows.lines() {
            assert_eq!(row.split(',').count(), columns, "{row:.80}");
        }
    }
}
