//! Counterfactual full-cost Settler investments.
//!
//! The expansion oracle establishes that an extra city can be valuable, but it
//! creates a free Settler and consequently removes the exact trade that a real
//! policy must make: production, population, escorting, travel, founding, and
//! the time needed for the new city to pay back its cost. This evaluator takes
//! the opposite approach. At an actual live production opportunity it submits
//! one legal `Produce(settler)` action, then gives the unmodified `AdvancedAi`
//! control of every subsequent turn through the terminal game result.
//!
//! The control and each forced-production branch start from the identical game
//! state and policy memory. Opponent doctrines rotate across matched replicas,
//! rather than treating repeat deterministic rollouts as independent samples.
//! The output is diagnostic data only; it neither changes a production policy
//! nor selects an evaluator arm.
//!
//! ```text
//! expansion_investment --games 8 --players 4 --warmup 1 --spacing 5 \
//!   --decisions-per-game 20 --alternatives 2 --replicas 4 \
//!   --out /tmp/expansion-investment.csv
//! ```
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::game::{default_speed, Action, Game, GameOptions, Item, VictoryConditions};
use civvis::parallel;
use civvis::setup::{MapPoles, MapScript, MapTopology};
use civvis::strategic::Doctrine;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;

const EPS: f64 = 1e-9;

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

fn settler() -> Item {
    Item::Unit {
        unit: civvis::name!("settler"),
    }
}

fn living_major(game: &Game, pid: usize) -> bool {
    game.players
        .get(pid)
        .is_some_and(|player| player.alive && !player.is_minor && !player.is_barbarian)
}

/// This is the current live `assess()` city appetite, duplicated here only to
/// select genuine expansion opportunities. It deliberately does not change the
/// agent or manufacture a target above the policy's own stated appetite.
fn desired_cities(game: &Game) -> usize {
    let land = game
        .map
        .tiles
        .values()
        .filter(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .count();
    let map_capacity = (2 + land / 55).clamp(3, 9);
    let cadence = game.standard_duration(90).max(1) as usize;
    (3 + game.turn as usize / cadence).min(map_capacity).min(6)
}

fn owned_founded_cities(game: &Game, pid: usize) -> usize {
    game.cities
        .values()
        .filter(|city| city.owner == pid && city.original_owner == pid)
        .count()
}

fn score_share(game: &Game, pid: usize) -> f64 {
    if !game.players.get(pid).is_some_and(|player| player.alive) {
        return 0.0;
    }
    let mut own = 0.0;
    let mut total = 0.0;
    for player in &game.players {
        if player.is_minor || player.is_barbarian || !player.alive {
            continue;
        }
        let score = game.score(player.id).max(0) as f64;
        total += score;
        if player.id == pid {
            own = score;
        }
    }
    if total > 0.0 {
        own / total
    } else {
        0.5
    }
}

#[derive(Clone, Debug)]
struct CityCandidate {
    city: u32,
    production: f64,
    population: i32,
}

#[derive(Clone)]
struct InvestmentDecision {
    game: Game,
    fleet: Vec<AdvancedAi>,
    game_id: u64,
    seat: usize,
    candidates: Vec<CityCandidate>,
}

/// Return up to `limit` legal, immediately selectable Settler cities in a
/// stable order. A city already building something is intentionally excluded:
/// this instrument measures a real queue decision, not the separate cost of
/// cancelling a prior commitment.
fn opportunities(game: &Game, ai: &AdvancedAi, pid: usize, limit: usize) -> Vec<CityCandidate> {
    if !living_major(game, pid) || limit == 0 {
        return Vec::new();
    }
    let cities = game.player_city_ids(pid);
    let walking = game.player_unit_ids(pid).into_iter().any(|unit| {
        game.units
            .get(&unit)
            .is_some_and(|unit| unit.kind == "settler")
    });
    if cities.len() >= desired_cities(game)
        || walking
        || game.turn >= game.standard_duration(175)
        || !ai.any_settle_site(game, pid)
    {
        return Vec::new();
    }
    let item = settler();
    let mut candidates: Vec<CityCandidate> = cities
        .into_iter()
        .filter_map(|city| {
            let state = game.cities.get(&city)?;
            (state.queue.is_empty() && state.pop >= 2 && game.can_produce(pid, city, &item)).then(
                || CityCandidate {
                    city,
                    production: game.city_yields(city).production,
                    population: state.pop,
                },
            )
        })
        .collect();
    candidates.sort_by(|left, right| {
        right
            .production
            .partial_cmp(&left.production)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.city.cmp(&right.city))
    });
    candidates.truncate(limit);
    candidates
}

fn observe_opportunity(
    game: &Game,
    fleet: &[AdvancedAi],
    game_id: u64,
    alternatives: usize,
) -> Option<InvestmentDecision> {
    let seat = game.current;
    let ai = fleet.get(seat)?;
    let candidates = opportunities(game, ai, seat, alternatives);
    (!candidates.is_empty()).then(|| InvestmentDecision {
        game: game.clone(),
        fleet: fleet.to_vec(),
        game_id,
        seat,
        candidates,
    })
}

#[derive(Clone, Debug, PartialEq)]
struct BranchOutcome {
    win: f64,
    score_share: f64,
    peak_founded: usize,
    terminal_founded: usize,
    turn: u32,
    forced_queue_survived: bool,
}

/// Run one real control or one real paid Settler commitment to an engine result.
/// `force = None` is the untouched policy control. A forced city is queued
/// before its own AI turn, so the normal production logic sees an occupied
/// queue and the engine, not this binary, charges all later costs.
fn rollout(
    decision: &InvestmentDecision,
    force: Option<&CityCandidate>,
    replica: usize,
) -> Result<BranchOutcome, String> {
    let mut sim = decision.game.clone();
    sim.set_fog_memory(false);
    if sim.current != decision.seat {
        return Err(format!(
            "game {} observation turn {} changed current seat {} -> {}",
            decision.game_id, decision.game.turn, decision.seat, sim.current
        ));
    }
    let mut fleet = decision.fleet.clone();
    let baseline = Weights::default();
    for player in &sim.players {
        if player.id == decision.seat || player.is_minor || player.is_barbarian {
            continue;
        }
        let doctrine = Doctrine::ALL[(replica + player.id) % Doctrine::ALL.len()];
        fleet[player.id].reweight(doctrine.apply(&baseline));
    }

    let mut forced_queue_survived = force.is_none();
    if let Some(candidate) = force {
        let item = settler();
        if !sim.can_produce(decision.seat, candidate.city, &item) {
            return Err(format!(
                "game {} turn {} city {} cannot legally produce the sampled Settler",
                decision.game_id, decision.game.turn, candidate.city
            ));
        }
        sim.apply(
            decision.seat,
            &Action::Produce {
                city: candidate.city,
                item: item.clone(),
            },
        )
        .map_err(|error| {
            format!(
                "game {} turn {} rejected legal sampled Settler at city {}: {error}",
                decision.game_id, decision.game.turn, candidate.city
            )
        })?;
        if sim
            .cities
            .get(&candidate.city)
            .and_then(|city| city.queue.first())
            != Some(&item)
        {
            return Err(format!(
                "game {} turn {} city {} did not retain the forced Settler queue",
                decision.game_id, decision.game.turn, candidate.city
            ));
        }
    }

    let mut peak_founded = owned_founded_cities(&sim, decision.seat);
    let mut saw_focal_turn = false;
    while sim.winner.is_none() && sim.turn <= sim.max_turns {
        let current = sim.current;
        fleet[current].take_turn(&mut sim, current);
        if sim.winner.is_none() && sim.current == current {
            let _ = sim.apply(current, &Action::EndTurn);
        }
        peak_founded = peak_founded.max(owned_founded_cities(&sim, decision.seat));
        if current == decision.seat && !saw_focal_turn {
            saw_focal_turn = true;
            if let Some(candidate) = force {
                let item = settler();
                forced_queue_survived = sim
                    .cities
                    .get(&candidate.city)
                    .and_then(|city| city.queue.first())
                    == Some(&item);
            }
        }
    }
    let Some(winner) = sim.winner else {
        return Err(format!(
            "game {} turn {} reached cap {} without a terminal winner",
            decision.game_id, decision.game.turn, sim.max_turns
        ));
    };
    if force.is_some() && !forced_queue_survived {
        return Err(format!(
            "game {} turn {} AdvancedAi replaced the sampled Settler during its focal turn",
            decision.game_id, decision.game.turn
        ));
    }
    Ok(BranchOutcome {
        win: f64::from(winner == decision.seat),
        score_share: score_share(&sim, decision.seat),
        peak_founded,
        terminal_founded: owned_founded_cities(&sim, decision.seat),
        turn: sim.reported_turn(),
        forced_queue_survived,
    })
}

#[derive(Clone, Copy, Default)]
struct MeanOutcome {
    win: f64,
    score_share: f64,
    peak_founded: f64,
    terminal_founded: f64,
}

fn mean_outcome(outcomes: &[BranchOutcome]) -> MeanOutcome {
    let count = outcomes.len().max(1) as f64;
    MeanOutcome {
        win: outcomes.iter().map(|outcome| outcome.win).sum::<f64>() / count,
        score_share: outcomes
            .iter()
            .map(|outcome| outcome.score_share)
            .sum::<f64>()
            / count,
        peak_founded: outcomes
            .iter()
            .map(|outcome| outcome.peak_founded as f64)
            .sum::<f64>()
            / count,
        terminal_founded: outcomes
            .iter()
            .map(|outcome| outcome.terminal_founded as f64)
            .sum::<f64>()
            / count,
    }
}

#[derive(Clone, Copy, Default)]
struct Delta {
    win: f64,
    score_share: f64,
    peak_founded: f64,
    terminal_founded: f64,
}

fn delta(treatment: MeanOutcome, control: MeanOutcome) -> Delta {
    Delta {
        win: treatment.win - control.win,
        score_share: treatment.score_share - control.score_share,
        peak_founded: treatment.peak_founded - control.peak_founded,
        terminal_founded: treatment.terminal_founded - control.terminal_founded,
    }
}

#[derive(Default)]
struct DecisionHarvest {
    rows: String,
    branches: usize,
    rejected: usize,
    nondeterministic: usize,
    forced: Vec<Delta>,
    best: Option<Delta>,
}

fn write_row(
    out: &mut String,
    decision: &InvestmentDecision,
    candidate: Option<&CityCandidate>,
    outcomes: &[BranchOutcome],
) {
    let mean = mean_outcome(outcomes);
    let (forced, city, production, population) = candidate.map_or((0, 0, 0.0, 0), |candidate| {
        (
            1,
            candidate.city,
            candidate.production,
            candidate.population,
        )
    });
    let _ = write!(
        out,
        "{},{},{},{forced},{city},{production:.6},{population},{:.8},{:.8},{:.8},{:.8}",
        decision.game_id,
        decision.game.turn,
        decision.seat,
        mean.win,
        mean.score_share,
        mean.peak_founded,
        mean.terminal_founded,
    );
    for outcome in outcomes {
        let _ = write!(
            out,
            ",{:.0},{:.8},{},{},{}",
            outcome.win,
            outcome.score_share,
            outcome.peak_founded,
            outcome.terminal_founded,
            outcome.turn,
        );
    }
    out.push('\n');
}

fn harvest_decision(decision: &InvestmentDecision, replicas: usize) -> DecisionHarvest {
    let replicas = replicas.max(1);
    let mut harvest = DecisionHarvest::default();
    let mut all = Vec::<(Option<&CityCandidate>, Vec<BranchOutcome>)>::new();
    for candidate in std::iter::once(None).chain(decision.candidates.iter().map(Some)) {
        let mut outcomes = Vec::with_capacity(replicas);
        for replica in 0..replicas {
            match rollout(decision, candidate, replica) {
                Ok(outcome) => {
                    harvest.branches += 1;
                    if candidate.is_none() && replica == 0 {
                        match rollout(decision, candidate, replica) {
                            Ok(repeated) if repeated == outcome => {}
                            Ok(_) => harvest.nondeterministic += 1,
                            Err(_) => harvest.rejected += 1,
                        }
                    }
                    outcomes.push(outcome);
                }
                Err(_) => harvest.rejected += 1,
            }
        }
        if outcomes.len() == replicas {
            all.push((candidate, outcomes));
        }
    }
    let Some((_, control)) = all.first() else {
        return harvest;
    };
    let control_mean = mean_outcome(control);
    let mut best = Delta::default();
    let mut best_score = control_mean.score_share;
    for (candidate, outcomes) in &all {
        write_row(&mut harvest.rows, decision, *candidate, outcomes);
        if candidate.is_some() {
            let compared = delta(mean_outcome(outcomes), control_mean);
            harvest.forced.push(compared);
            if control_mean.score_share + compared.score_share > best_score + EPS {
                best_score = control_mean.score_share + compared.score_share;
                best = compared;
            }
        }
    }
    if !harvest.forced.is_empty() {
        harvest.best = Some(best);
    }
    harvest
}

#[derive(Default)]
struct GameHarvest {
    rows: String,
    decisions: Vec<DecisionHarvest>,
    missed: usize,
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
    map_script: MapScript,
    map_topology: MapTopology,
    map_poles: MapPoles,
    randomize_civs: bool,
    victory_conditions: VictoryConditions,
    warmup: u32,
    spacing: u32,
    decisions: usize,
    alternatives: usize,
    replicas: usize,
) -> GameHarvest {
    let mut game = Game::new_with(GameOptions {
        speed: speed.to_string(),
        map_script,
        map_topology,
        map_poles,
        randomize_civs,
        ..GameOptions::new(players, width, height, game_id, turns, city_states)
    });
    game.victory_conditions = victory_conditions;
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
        if let Some(decision) = observe_opportunity(&game, &fleet, game_id, alternatives) {
            let measured = harvest_decision(&decision, replicas);
            harvest.rows.push_str(&measured.rows);
            harvest.decisions.push(measured);
        } else {
            harvest.missed += 1;
        }
        let next = game.turn.saturating_add(spacing.max(1));
        while game.winner.is_none() && game.turn < next {
            step(&mut game, &mut fleet);
        }
    }
    harvest
}

fn header(replicas: usize) -> String {
    let mut out = String::from(
        "game,turn,seat,forced,city,production,population,win,score_share,peak_founded,terminal_founded",
    );
    for replica in 0..replicas.max(1) {
        let _ = write!(
            out,
            ",w{replica},s{replica},peak{replica},terminal{replica},end{replica}"
        );
    }
    out.push('\n');
    out
}

fn mean_se(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if values.len() < 2 {
        return (mean, 0.0);
    }
    let variance = values
        .iter()
        .map(|value| (*value - mean).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    (mean, (variance / values.len() as f64).sqrt())
}

fn print_delta(label: &str, deltas: &[Delta]) {
    let score: Vec<f64> = deltas.iter().map(|delta| delta.score_share).collect();
    let win: Vec<f64> = deltas.iter().map(|delta| delta.win).collect();
    let peak: Vec<f64> = deltas.iter().map(|delta| delta.peak_founded).collect();
    let terminal: Vec<f64> = deltas.iter().map(|delta| delta.terminal_founded).collect();
    let (score_mean, score_se) = mean_se(&score);
    let (win_mean, win_se) = mean_se(&win);
    let (peak_mean, peak_se) = mean_se(&peak);
    let (terminal_mean, terminal_se) = mean_se(&terminal);
    println!(
        "{label}: score-share {score_mean:+.4} +/- {score_se:.4}; \
         win {win_mean:+.4} +/- {win_se:.4}; peak founded {peak_mean:+.3} +/- {peak_se:.3}; \
         terminal founded {terminal_mean:+.3} +/- {terminal_se:.3}"
    );
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
    let map_name = text(&args, "--map", MapScript::default().id());
    let map_script = MapScript::from_id(&map_name).unwrap_or_else(|| {
        eprintln!("expansion_investment: unknown map script {map_name:?}");
        std::process::exit(2);
    });
    let shape_name = text(&args, "--shape", MapTopology::default().id());
    let map_topology = MapTopology::from_id(&shape_name).unwrap_or_else(|| {
        eprintln!("expansion_investment: unknown map shape {shape_name:?}");
        std::process::exit(2);
    });
    let poles_name = text(&args, "--poles", MapPoles::default().id());
    let map_poles = MapPoles::from_id(&poles_name).unwrap_or_else(|| {
        eprintln!("expansion_investment: unknown thermal distribution {poles_name:?}");
        std::process::exit(2);
    });
    let victory_names = text(&args, "--victories", &VictoryConditions::NAMES.join(","));
    let victory_conditions = VictoryConditions::parse(&victory_names).unwrap_or_else(|error| {
        eprintln!("expansion_investment: --victories: {error}");
        std::process::exit(2);
    });
    let randomize_civs = args.iter().any(|arg| arg == "--randomize-civs");
    // Empty-queue expansion opportunities are rare and brief. Dense default
    // observation improves coverage but does not duplicate a world: after each
    // probe the base trajectory advances at least `spacing` full turns.
    let warmup = number(&args, "--warmup", 1) as u32;
    let spacing = number(&args, "--spacing", 5) as u32;
    let decisions = number(&args, "--decisions-per-game", 20);
    let alternatives = number(&args, "--alternatives", 2);
    let replicas = number(&args, "--replicas", Doctrine::ALL.len()).max(1);
    let seed = number(&args, "--seed", 995_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let out = text(&args, "--out", "/tmp/expansion-investment.csv");
    if games == 0 || players < 2 || alternatives == 0 || decisions == 0 {
        eprintln!(
            "expansion_investment: games, alternatives, and decisions must be positive; players >= 2"
        );
        std::process::exit(2);
    }
    println!(
        "expansion_investment: {games} games, {players}p {width}x{height}, {speed}, \
         {city_states} city-states, map {}, shape {}, poles {}, warmup {warmup}, \
         {decisions} decision(s)/game every {spacing} turns, {alternatives} forced cities, \
         {replicas} matched replicas, terminal returns, jobs {jobs}",
        map_script.id(),
        map_topology.id(),
        map_poles.id(),
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
            map_script,
            map_topology,
            map_poles,
            randomize_civs,
            victory_conditions,
            warmup,
            spacing,
            decisions,
            alternatives,
            replicas,
        )
    });

    let mut decisions_seen = 0usize;
    let mut candidates = 0usize;
    let mut branches = 0usize;
    let mut rejected = 0usize;
    let mut nondeterministic = 0usize;
    let mut missed = 0usize;
    let mut all_forced = Vec::new();
    let mut best_by_game = BTreeMap::<u64, Vec<Delta>>::new();
    for (index, game) in harvests.iter().enumerate() {
        missed += game.missed;
        for decision in &game.decisions {
            decisions_seen += 1;
            candidates += decision.forced.len();
            branches += decision.branches;
            rejected += decision.rejected;
            nondeterministic += decision.nondeterministic;
            all_forced.extend(decision.forced.iter().copied());
            if let Some(best) = decision.best {
                best_by_game
                    .entry(seed + index as u64)
                    .or_default()
                    .push(best);
            }
        }
    }
    println!(
        "harvested {decisions_seen} eligible decisions, {candidates} paid Settler alternatives, \
         {branches} terminal continuations"
    );
    println!("observation: {missed} scheduled turns lacked a legal live expansion opportunity");
    println!(
        "integrity: {rejected} rejected branches, {nondeterministic} repeated-control mismatches"
    );
    if decisions_seen == 0 || candidates == 0 || rejected > 0 || nondeterministic > 0 {
        eprintln!("expansion_investment: integrity gate failed; refusing to write a dataset");
        std::process::exit(2);
    }
    print_delta("all forced city alternatives", &all_forced);
    let game_macro: Vec<Delta> = best_by_game
        .values()
        .filter(|rows| !rows.is_empty())
        .map(|rows| Delta {
            win: rows.iter().map(|row| row.win).sum::<f64>() / rows.len() as f64,
            score_share: rows.iter().map(|row| row.score_share).sum::<f64>() / rows.len() as f64,
            peak_founded: rows.iter().map(|row| row.peak_founded).sum::<f64>() / rows.len() as f64,
            terminal_founded: rows.iter().map(|row| row.terminal_founded).sum::<f64>()
                / rows.len() as f64,
        })
        .collect();
    print_delta("best forced city per decision, game-macro", &game_macro);
    let score_positive = all_forced
        .iter()
        .filter(|delta| delta.score_share > EPS)
        .count();
    let peak_positive = all_forced
        .iter()
        .filter(|delta| delta.peak_founded > EPS)
        .count();
    println!(
        "mechanism: {peak_positive}/{} forced alternatives ever produced an extra owned founded city; \
         {score_positive}/{} improved terminal score share",
        all_forced.len(),
        all_forced.len(),
    );

    let mut file = fs::File::create(&out).unwrap_or_else(|error| {
        eprintln!("expansion_investment: cannot create {out}: {error}");
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
    use super::{harvest_game, header, opportunities, rollout, settler, InvestmentDecision};
    use civvis::ai::AdvancedAi;
    use civvis::game::{Action, Game};

    fn city_opportunity(seed: u64, turns: u32) -> (Game, Vec<AdvancedAi>, u32) {
        let mut game = Game::new(3, 24, 16, seed, turns, 0);
        let founder = game.player_unit_ids(0)[0];
        game.apply(0, &Action::FoundCity { unit: founder }).unwrap();
        let city = game.player_city_ids(0)[0];
        game.cities.get_mut(&city).unwrap().pop = 2;
        (game.clone(), AdvancedAi::fleet(&game), city)
    }

    #[test]
    fn sampled_city_is_a_legal_empty_queue_settler_choice() {
        let (game, fleet, city) = city_opportunity(701, 40);
        let candidates = opportunities(&game, &fleet[0], 0, 2);
        assert!(candidates.iter().any(|candidate| candidate.city == city));
        assert!(game.cities[&city].queue.is_empty());
        assert!(game.can_produce(0, city, &settler()));
    }

    #[test]
    fn paid_settler_branch_reaches_a_terminal_result_and_repeats() {
        let (game, fleet, _) = city_opportunity(702, 20);
        let candidates = opportunities(&game, &fleet[0], 0, 1);
        let decision = InvestmentDecision {
            game,
            fleet,
            game_id: 702,
            seat: 0,
            candidates,
        };
        let first = rollout(&decision, Some(&decision.candidates[0]), 0).unwrap();
        let second = rollout(&decision, Some(&decision.candidates[0]), 0).unwrap();
        assert_eq!(first, second);
        assert!(first.win == 0.0 || first.win == 1.0);
        assert!(first.forced_queue_survived);
    }

    #[test]
    fn harvested_rows_match_the_declared_schema() {
        let run = harvest_game(
            703,
            0,
            3,
            24,
            16,
            40,
            0,
            "standard",
            Default::default(),
            Default::default(),
            Default::default(),
            false,
            Default::default(),
            1,
            1,
            1,
            1,
            1,
        );
        for row in run.rows.lines() {
            assert_eq!(
                row.split(',').count(),
                header(1).trim_end().split(',').count()
            );
        }
    }
}
