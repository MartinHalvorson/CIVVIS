//! Matched evaluation of cash recovery for captive Spies.
//!
//! The focal treatment replays one complete stock `AdvancedAi` turn, retains
//! the controller state it produced, and then makes at most one ordinary
//! mutually beneficial `Trade` on the stock diplomacy cadence. The captor is
//! paid the rounded-up fair midpoint in real lump Gold; no engine rule or
//! shipped AI default changes.
use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, DealItems, Game, GameOptions, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 10_019_999;
const SCREEN_MAPS: usize = 18;
const SCREEN_SEED: u64 = 10_020_000;
const HOLDOUT_MAPS: usize = 63;
const HOLDOUT_SEED: u64 = 10_021_000;
const NOMINAL_TURNS: u32 = 250;
const OBSERVE_THROUGH: u32 = 320;
const REQUIRED_SCIENCE_PROJECTS: [&str; 4] = [
    "launch_earth_satellite",
    "launch_moon_landing",
    "launch_mars_colony",
    "exoplanet_expedition",
];
const DEPLOYMENT_PLAYERS: [usize; 7] = [4, 6, 8, 10, 5, 7, 9];
const DEPLOYMENT_SCRIPTS: [MapScript; 9] = [
    MapScript::LandOnly,
    MapScript::WaterWorld,
    MapScript::Continents,
    MapScript::TrueStartEarth,
    MapScript::Lakes,
    MapScript::InlandSea,
    MapScript::Pangaea,
    MapScript::SmallContinents,
    MapScript::Islands,
];
const DEPLOYMENT_TOPOLOGIES: [MapTopology; 2] = [MapTopology::Flat, MapTopology::Planet];
const PROFILE_OVERRIDE_FLAGS: [&str; 7] = [
    "--players",
    "--width",
    "--height",
    "--city-states",
    "--map",
    "--shape",
    "--shapes",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeploymentProfile {
    players: usize,
    width: i32,
    height: i32,
    city_states: usize,
    map_script: MapScript,
    map_topology: MapTopology,
}

fn deployment_profile(map: usize) -> DeploymentProfile {
    let players = DEPLOYMENT_PLAYERS[map % DEPLOYMENT_PLAYERS.len()];
    let size = MapSize::for_players(players);
    DeploymentProfile {
        players,
        width: size.width,
        height: size.height,
        city_states: size.default_city_states,
        map_script: DEPLOYMENT_SCRIPTS[map % DEPLOYMENT_SCRIPTS.len()],
        map_topology: DEPLOYMENT_TOPOLOGIES[map % DEPLOYMENT_TOPOLOGIES.len()],
    }
}

fn deployment_counts<T: Copy + Eq>(
    maps: usize,
    select: impl Fn(DeploymentProfile) -> T,
) -> Vec<(T, usize)> {
    let mut counts = Vec::new();
    for map in 0..maps {
        let value = select(deployment_profile(map));
        if let Some((_, count)) = counts.iter_mut().find(|(seen, _)| *seen == value) {
            *count += 1;
        } else {
            counts.push((value, 1));
        }
    }
    counts
}

fn has_arg(args: &[String], key: &str) -> bool {
    args.iter().any(|arg| arg == key)
}

fn number(args: &[String], key: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text_arg(args: &[String], key: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn topology_schedule(args: &[String]) -> Result<Vec<MapTopology>, String> {
    let has_shape = has_arg(args, "--shape");
    let has_shapes = has_arg(args, "--shapes");
    if has_shape && has_shapes {
        return Err("choose either --shape or --shapes, not both".to_string());
    }
    let names = if has_shapes {
        text_arg(args, "--shapes", "")
    } else if has_shape {
        text_arg(args, "--shape", "")
    } else {
        DEPLOYMENT_TOPOLOGIES
            .iter()
            .map(|topology| topology.id())
            .collect::<Vec<_>>()
            .join(",")
    };
    let topologies = names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| MapTopology::from_id(name).ok_or_else(|| format!("unknown map shape {name:?}")))
        .collect::<Result<Vec<_>, _>>()?;
    if topologies.is_empty() {
        return Err("--shapes must name at least one topology".to_string());
    }
    Ok(topologies)
}

fn topology_for(map: usize, schedule: &[MapTopology]) -> MapTopology {
    schedule[map % schedule.len()]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Stock,
    NullReplay,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RecoveryQuote {
    spy: u32,
    captor: usize,
    level: i64,
    promotions: usize,
    payment: f64,
}

fn owner_receive_value(level: i64, promotions: usize) -> f64 {
    325.0 + 70.0 * level.max(0) as f64 + 35.0 * promotions as f64
}

fn captor_release_cost(level: i64, promotions: usize) -> f64 {
    90.0 + 30.0 * level.max(0) as f64 + 15.0 * promotions as f64
}

fn recovery_midpoint(level: i64, promotions: usize) -> f64 {
    (owner_receive_value(level, promotions) + captor_release_cost(level, promotions)) / 2.0
}

fn recovery_payment(level: i64, promotions: usize) -> f64 {
    recovery_midpoint(level, promotions).ceil()
}

fn cash_reserve(gold: f64) -> f64 {
    (0.30 * gold.max(0.0)).min(40.0)
}

fn can_afford(gold: f64, payment: f64) -> bool {
    gold - payment + f64::EPSILON >= cash_reserve(gold)
}

fn eligible_quotes(game: &Game, pid: usize) -> Vec<RecoveryQuote> {
    if pid >= game.players.len() || !game.players[pid].alive {
        return Vec::new();
    }
    game.spies
        .values()
        .filter_map(|spy| {
            let captor = spy.captured_by?;
            (spy.owner == pid
                && captor != pid
                && captor < game.players.len()
                && game.players[captor].alive
                && !game.players[captor].is_minor
                && !game.players[captor].is_barbarian
                && game.has_met(pid, captor)
                && !game.is_at_war(pid, captor))
            .then(|| RecoveryQuote {
                spy: spy.id,
                captor,
                level: spy.level.max(0),
                promotions: spy.promotions.len(),
                payment: recovery_payment(spy.level, spy.promotions.len()),
            })
        })
        .collect()
}

fn choose_recovery(game: &Game, pid: usize) -> Option<RecoveryQuote> {
    let gold = game.players.get(pid)?.gold;
    eligible_quotes(game, pid)
        .into_iter()
        .filter(|quote| can_afford(gold, quote.payment))
        .min_by_key(|quote| (Reverse(quote.level), Reverse(quote.promotions), quote.spy))
}

#[derive(Clone, Debug, Default, PartialEq)]
struct RecoveryCensus {
    cadence_turns: u32,
    peace_opportunity_turns: u32,
    affordable_turns: u32,
    recoveries: u32,
    failed_applications: u32,
    gold_paid: f64,
    recovered_levels: i64,
    recovered_promotions: u32,
    recovered_spy_actions: u32,
    recovered_assignments: u32,
    recovered_promotions_used: u32,
    recovered_missions: u32,
    total_trade_actions: u32,
}

#[derive(Default)]
struct RecoveryState {
    census: RecoveryCensus,
    recovered: BTreeSet<u32>,
    attempted_turns: BTreeSet<u32>,
}

fn record_actions(actions: &[(usize, Action)], pid: usize, state: &mut RecoveryState) {
    for (owner, action) in actions {
        if *owner != pid {
            continue;
        }
        match action {
            Action::Trade { .. } => state.census.total_trade_actions += 1,
            Action::AssignSpy { spy, .. } if state.recovered.contains(spy) => {
                state.census.recovered_spy_actions += 1;
                state.census.recovered_assignments += 1;
            }
            Action::PromoteSpy { spy, .. } if state.recovered.contains(spy) => {
                state.census.recovered_spy_actions += 1;
                state.census.recovered_promotions_used += 1;
            }
            Action::SpyMission { spy, .. } if state.recovered.contains(spy) => {
                state.census.recovered_spy_actions += 1;
                state.census.recovered_missions += 1;
            }
            _ => {}
        }
    }
}

fn recover_one(game: &mut Game, pid: usize, state: &mut RecoveryState) {
    if game.turn % 6 != pid as u32 % 6 || !state.attempted_turns.insert(game.turn) {
        return;
    }
    state.census.cadence_turns += 1;
    let eligible = eligible_quotes(game, pid);
    if eligible.is_empty() {
        return;
    }
    state.census.peace_opportunity_turns += 1;
    let Some(quote) = choose_recovery(game, pid) else {
        return;
    };
    state.census.affordable_turns += 1;
    let action = Action::Trade {
        player: quote.captor,
        offer: Box::new(DealItems {
            gold: quote.payment,
            ..DealItems::default()
        }),
        request: Box::new(DealItems {
            captured_spies: vec![quote.spy],
            ..DealItems::default()
        }),
    };
    match game.apply(pid, &action) {
        Ok(()) => {
            state.census.recoveries += 1;
            state.census.gold_paid += quote.payment;
            state.census.recovered_levels += quote.level;
            state.census.recovered_promotions += quote.promotions as u32;
            state.census.total_trade_actions += 1;
            state.recovered.insert(quote.spy);
        }
        Err(_) => state.census.failed_applications += 1,
    }
}

/// Run the stock controller on a clone, preserve its resulting state, and
/// replay every successful action except its final `EndTurn`.
fn replay_stock_turn(
    game: &mut Game,
    ai: &mut AdvancedAi,
    pid: usize,
    state: &mut RecoveryState,
) -> Result<(), String> {
    let mut observed = game.clone();
    let before = observed.log.len();
    let mut actor = ai.clone();
    actor.take_turn(&mut observed, pid);
    let mut actions: Vec<(usize, Action)> = observed.log.since(before).cloned().collect();
    let ended = actions
        .last()
        .is_some_and(|(owner, action)| *owner == pid && matches!(action, Action::EndTurn));
    if ended {
        actions.pop();
    }
    for (owner, action) in &actions {
        if *owner != pid {
            return Err(format!(
                "stock seat {pid} logged an action for seat {owner}: {action:?}"
            ));
        }
        game.apply(*owner, action).map_err(|why| {
            format!("stock action replay failed for seat {pid}: {why}; {action:?}")
        })?;
    }
    record_actions(&actions, pid, state);
    *ai = actor;
    if ended && game.winner.is_none() && game.current != pid {
        return Err(format!(
            "stock replay advanced from seat {pid} before the deferred EndTurn"
        ));
    }
    Ok(())
}

fn science_progress(game: &Game, pid: usize) -> (usize, f64) {
    let expedition = game.players[pid]
        .science_projects
        .contains("exoplanet_expedition");
    let completed = REQUIRED_SCIENCE_PROJECTS
        .iter()
        .filter(|project| {
            game.players[pid]
                .science_projects
                .iter()
                .any(|finished| finished.as_str() == **project)
        })
        .count();
    let distance = if expedition {
        game.players[pid].exoplanet_distance.clamp(0.0, 50.0) / 50.0
    } else {
        0.0
    };
    (completed, completed as f64 + distance)
}

#[derive(Clone, Debug, PartialEq)]
struct GameResult {
    won: bool,
    victory: Option<String>,
    reported_turn: u32,
    policy_max_turns: u32,
    score: i64,
    gold: f64,
    cities: usize,
    techs: usize,
    civics: usize,
    science_projects: usize,
    science_progress: f64,
    culture_lifetime: f64,
    tourism_lifetime: f64,
    military_power: f64,
    captured_spies: usize,
    active_spies: usize,
    active_missions: usize,
    spy_levels: i64,
    spy_promotions: usize,
    census: RecoveryCensus,
}

struct Played {
    result: GameResult,
    serialized: Option<String>,
}

fn play(
    options: GameOptions,
    focal: usize,
    mode: Mode,
    observe_through: u32,
    serialize: bool,
) -> Played {
    let mut game = Game::new_with(options);
    let policy_max_turns = game.max_turns;
    assert!(
        observe_through >= policy_max_turns,
        "external observation turn {observe_through} precedes policy horizon {policy_max_turns}"
    );
    game.set_fog_memory(false);
    game.victory_conditions = VictoryConditions {
        science: true,
        culture: true,
        religious: false,
        diplomatic: false,
        domination: true,
        score: false,
    };
    let mut ais = AdvancedAi::fleet(&game);
    let mut state = RecoveryState::default();

    while game.winner.is_none() && game.turn <= observe_through {
        assert_eq!(
            game.max_turns, policy_max_turns,
            "external continuation changed the policy-visible horizon"
        );
        let pid = game.current;
        let before = game.log.len();
        if pid == focal && mode != Mode::Stock {
            replay_stock_turn(&mut game, &mut ais[pid], pid, &mut state)
                .unwrap_or_else(|why| panic!("turn {} seat {pid}: {why}", game.turn));
            if mode == Mode::Recovery && game.winner.is_none() && game.current == pid {
                recover_one(&mut game, pid, &mut state);
            }
        } else {
            ais[pid].take_turn(&mut game, pid);
            if pid == focal {
                let actions = game.log.since(before).cloned().collect::<Vec<_>>();
                record_actions(&actions, pid, &mut state);
            }
        }
        if game.winner.is_none() && game.current == pid {
            game.apply(pid, &Action::EndTurn).unwrap_or_else(|why| {
                panic!("turn {} seat {pid}: deferred EndTurn: {why}", game.turn)
            });
        }
    }
    assert_eq!(
        game.max_turns, policy_max_turns,
        "external continuation changed the policy-visible horizon"
    );

    let player = &game.players[focal];
    let owned_spies = game.spies.values().filter(|spy| spy.owner == focal);
    let captured_spies = owned_spies
        .clone()
        .filter(|spy| spy.captured_by.is_some())
        .count();
    let active_spies = owned_spies
        .clone()
        .filter(|spy| spy.captured_by.is_none())
        .count();
    let active_missions = owned_spies
        .clone()
        .filter(|spy| spy.captured_by.is_none() && spy.mission.is_some())
        .count();
    let spy_levels = owned_spies.clone().map(|spy| spy.level.max(0)).sum();
    let spy_promotions = owned_spies.map(|spy| spy.promotions.len()).sum();
    let (science_projects, science_progress) = science_progress(&game, focal);
    let result = GameResult {
        won: game.winner == Some(focal),
        victory: (game.winner == Some(focal))
            .then(|| game.victory_type.clone())
            .flatten(),
        reported_turn: if game.winner.is_some() {
            game.reported_turn()
        } else {
            observe_through
        },
        policy_max_turns,
        score: game.score(focal),
        gold: player.gold,
        cities: game.player_city_ids(focal).len(),
        techs: player.techs.len(),
        civics: player.civics.len(),
        science_projects,
        science_progress,
        culture_lifetime: player.culture_lifetime,
        tourism_lifetime: player.tourism_lifetime,
        military_power: game.military_power(focal),
        captured_spies,
        active_spies,
        active_missions,
        spy_levels,
        spy_promotions,
        census: state.census,
    };
    let serialized = serialize
        .then(|| serde_json::to_string(&game).expect("terminal Game must remain serializable"));
    Played { result, serialized }
}

#[derive(Clone, Debug)]
struct MapResult {
    control: [GameResult; 2],
    comparison: [GameResult; 2],
    exact: [bool; 2],
}

fn map_win_score(control_wins: usize, treatment_wins: usize) -> f64 {
    0.5 + (treatment_wins as f64 - control_wins as f64) / 4.0
}

fn paired_share(control: f64, treatment: f64) -> f64 {
    let control = control.max(0.0);
    let treatment = treatment.max(0.0);
    let total = control + treatment;
    if total > 0.0 {
        treatment / total
    } else {
        0.5
    }
}

fn exact_two_sided(hits: usize, n: usize) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let extreme = hits.min(n - hits);
    let mut coefficient = 1.0_f64;
    let mut tail = 0.0_f64;
    for k in 0..=n {
        if k > 0 {
            coefficient *= (n - k + 1) as f64 / k as f64;
        }
        if k <= extreme || k >= n - extreme {
            tail += coefficient;
        }
    }
    (tail / 2f64.powi(n as i32)).min(1.0)
}

#[derive(Default)]
struct ArmSummary {
    games: usize,
    wins: usize,
    turns: u64,
    score: i64,
    gold: f64,
    cities: usize,
    techs: usize,
    civics: usize,
    science_projects: usize,
    science_progress: f64,
    culture_lifetime: f64,
    tourism_lifetime: f64,
    military_power: f64,
    captured_spies: usize,
    active_spies: usize,
    active_missions: usize,
    spy_levels: i64,
    spy_promotions: usize,
    cadence_turns: u64,
    peace_opportunity_turns: u64,
    affordable_turns: u64,
    recoveries: u64,
    failed_applications: u64,
    gold_paid: f64,
    recovered_levels: i64,
    recovered_promotions: u64,
    recovered_spy_actions: u64,
    recovered_assignments: u64,
    recovered_promotions_used: u64,
    recovered_missions: u64,
    total_trade_actions: u64,
    recovery_games: usize,
    recovered_action_games: usize,
    victories: BTreeMap<String, usize>,
}

impl ArmSummary {
    fn record(&mut self, result: &GameResult) {
        self.games += 1;
        self.wins += result.won as usize;
        self.turns += result.reported_turn as u64;
        self.score += result.score;
        self.gold += result.gold;
        self.cities += result.cities;
        self.techs += result.techs;
        self.civics += result.civics;
        self.science_projects += result.science_projects;
        self.science_progress += result.science_progress;
        self.culture_lifetime += result.culture_lifetime;
        self.tourism_lifetime += result.tourism_lifetime;
        self.military_power += result.military_power;
        self.captured_spies += result.captured_spies;
        self.active_spies += result.active_spies;
        self.active_missions += result.active_missions;
        self.spy_levels += result.spy_levels;
        self.spy_promotions += result.spy_promotions;
        let census = &result.census;
        self.cadence_turns += census.cadence_turns as u64;
        self.peace_opportunity_turns += census.peace_opportunity_turns as u64;
        self.affordable_turns += census.affordable_turns as u64;
        self.recoveries += census.recoveries as u64;
        self.failed_applications += census.failed_applications as u64;
        self.gold_paid += census.gold_paid;
        self.recovered_levels += census.recovered_levels;
        self.recovered_promotions += census.recovered_promotions as u64;
        self.recovered_spy_actions += census.recovered_spy_actions as u64;
        self.recovered_assignments += census.recovered_assignments as u64;
        self.recovered_promotions_used += census.recovered_promotions_used as u64;
        self.recovered_missions += census.recovered_missions as u64;
        self.total_trade_actions += census.total_trade_actions as u64;
        self.recovery_games += (census.recoveries > 0) as usize;
        self.recovered_action_games += (census.recovered_spy_actions > 0) as usize;
        if result.won {
            *self
                .victories
                .entry(
                    result
                        .victory
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                )
                .or_default() += 1;
        }
    }
}

#[derive(Default)]
struct StudySummary {
    maps: usize,
    control: ArmSummary,
    treatment: ArmSummary,
    score_delta: f64,
    favorable: usize,
    adverse: usize,
    win_score: f64,
    terminal_share: f64,
}

impl StudySummary {
    fn record(&mut self, result: &MapResult) {
        self.maps += 1;
        let control_wins = result.control.iter().filter(|game| game.won).count();
        let treatment_wins = result.comparison.iter().filter(|game| game.won).count();
        self.win_score += map_win_score(control_wins, treatment_wins);
        self.terminal_share += result
            .control
            .iter()
            .zip(&result.comparison)
            .map(|(old, new)| paired_share(old.score as f64, new.score as f64))
            .sum::<f64>()
            / 2.0;
        let delta = result
            .control
            .iter()
            .zip(&result.comparison)
            .map(|(old, new)| (new.score - old.score) as f64)
            .sum::<f64>()
            / 2.0;
        self.score_delta += delta;
        if delta > f64::EPSILON {
            self.favorable += 1;
        } else if delta < -f64::EPSILON {
            self.adverse += 1;
        }
        for (control, treatment) in result.control.iter().zip(&result.comparison) {
            self.control.record(control);
            self.treatment.record(treatment);
        }
    }
}

#[derive(Clone, Copy)]
struct GateInputs {
    recovery_games: usize,
    recoveries: u64,
    failed_applications: u64,
    recovered_action_games: usize,
    recovered_spy_actions: u64,
    control_captives: usize,
    treatment_captives: usize,
    control_active: usize,
    treatment_active: usize,
    score_delta: f64,
    favorable: usize,
    adverse: usize,
    sign_p: f64,
    terminal_score_share: f64,
    paired_win_score: f64,
    control_wins: usize,
    treatment_wins: usize,
    control_science_wins: usize,
    treatment_science_wins: usize,
    control_culture_wins: usize,
    treatment_culture_wins: usize,
    control_domination_wins: usize,
    treatment_domination_wins: usize,
}

fn mechanism_passes(gate: GateInputs) -> bool {
    gate.recoveries >= 18
        && gate.recovery_games >= 12
        && gate.failed_applications == 0
        && gate.recovered_spy_actions >= 18
        && gate.recovered_action_games >= 8
        && gate.treatment_captives.saturating_mul(100) <= gate.control_captives.saturating_mul(75)
        && gate.treatment_active >= gate.control_active
}

fn victory_types_nonlower(gate: GateInputs) -> bool {
    gate.treatment_science_wins >= gate.control_science_wins
        && gate.treatment_culture_wins >= gate.control_culture_wins
        && gate.treatment_domination_wins >= gate.control_domination_wins
}

fn screen_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate)
        && gate.favorable > gate.adverse
        && gate.sign_p <= 0.20
        && gate.terminal_score_share >= 0.50
        && gate.score_delta >= 0.0
        && gate.paired_win_score >= 0.50
        && gate.treatment_wins >= gate.control_wins
        && victory_types_nonlower(gate)
}

fn holdout_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate)
        && gate.favorable > gate.adverse
        && gate.sign_p < 0.05
        && gate.terminal_score_share >= 0.505
        && gate.score_delta > 0.0
        && gate.paired_win_score >= 0.50
        && gate.treatment_wins >= gate.control_wins
        && victory_types_nonlower(gate)
}

fn victory_count(summary: &ArmSummary, kind: &str) -> usize {
    summary.victories.get(kind).copied().unwrap_or(0)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let null = has_arg(&args, "--null");
    let deployment_mix = has_arg(&args, "--deployment-mix");
    if deployment_mix {
        let conflicts = PROFILE_OVERRIDE_FLAGS
            .iter()
            .copied()
            .filter(|flag| has_arg(&args, flag))
            .collect::<Vec<_>>();
        if !conflicts.is_empty() {
            eprintln!(
                "--deployment-mix derives every world profile; remove conflicting flags: {}",
                conflicts.join(", ")
            );
            std::process::exit(2);
        }
    }

    let maps = number(
        &args,
        "--maps",
        if null {
            NULL_MAPS as i64
        } else {
            SCREEN_MAPS as i64
        },
    )
    .max(1) as usize;
    let seed = number(
        &args,
        "--seed",
        if null {
            NULL_SEED as i64
        } else {
            SCREEN_SEED as i64
        },
    )
    .max(0) as u64;
    let players = number(&args, "--players", 8).max(2) as usize;
    let width = number(&args, "--width", 84).max(8) as i32;
    let height = number(&args, "--height", 54).max(8) as i32;
    let city_states = number(&args, "--city-states", 12).max(0) as usize;
    let nominal_turns = number(&args, "--turns", NOMINAL_TURNS as i64).max(1) as u32;
    let observe_through = number(&args, "--observe-through", OBSERVE_THROUGH as i64).max(1) as u32;
    if observe_through < nominal_turns {
        eprintln!("--observe-through must be at least --turns");
        std::process::exit(2);
    }
    let jobs = match number(&args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    }
    .clamp(1, 6);
    let speed = text_arg(&args, "--speed", "online");
    let map_name = text_arg(&args, "--map", "continents");
    let map_script = MapScript::from_id(&map_name).unwrap_or_else(|| {
        eprintln!("unknown map script {map_name:?}");
        std::process::exit(2);
    });
    let map_topologies = if deployment_mix {
        DEPLOYMENT_TOPOLOGIES.to_vec()
    } else {
        topology_schedule(&args).unwrap_or_else(|why| {
            eprintln!("{why}");
            std::process::exit(2);
        })
    };
    let poles_name = text_arg(&args, "--poles", "poles");
    let map_poles = MapPoles::from_id(&poles_name).unwrap_or_else(|| {
        eprintln!("unknown thermal distribution {poles_name:?}");
        std::process::exit(2);
    });
    let victory_names = text_arg(&args, "--victories", "science,culture,domination");
    let victories = VictoryConditions::parse(&victory_names).unwrap_or_else(|why| {
        eprintln!(
            "--victories: {why}; choose from {:?}",
            VictoryConditions::NAMES
        );
        std::process::exit(2);
    });
    let required_victories = VictoryConditions {
        science: true,
        culture: true,
        religious: false,
        diplomatic: false,
        domination: true,
        score: false,
    };
    if victories != required_victories {
        eprintln!(
            "this treatment is defined only for science,culture,domination; got {victory_names:?}"
        );
        std::process::exit(2);
    }
    let randomize_civs = has_arg(&args, "--randomize-civs");
    let rules = Rules::embedded();
    if !rules.speeds.contains_key(&speed) {
        eprintln!("unknown game speed {speed:?}");
        std::process::exit(2);
    }

    println!("Captive Spy cash-recovery evaluator");
    if deployment_mix {
        let player_batch = deployment_counts(maps, |profile| profile.players)
            .into_iter()
            .map(|(players, count)| format!("{players}p={count}"))
            .collect::<Vec<_>>()
            .join(",");
        let script_batch = deployment_counts(maps, |profile| profile.map_script)
            .into_iter()
            .map(|(script, count)| format!("{}={count}", script.id()))
            .collect::<Vec<_>>()
            .join(",");
        let topology_batch = deployment_counts(maps, |profile| profile.map_topology)
            .into_iter()
            .map(|(topology, count)| format!("{}={count}", topology.id()))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "profile: deployment mix; players {player_batch}; scripts {script_batch}; topologies {topology_batch}"
        );
    } else {
        println!(
            "profile: diagnostic fixed cell: {players}p requested {width}x{height}, {city_states} city-states, map {}, topology schedule {}",
            map_script.id(),
            map_topologies
                .iter()
                .map(|topology| topology.id())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    println!(
        "rules: {nominal_turns} policy-visible {speed} turns, observe through {observe_through}, poles {}, civilizations {}, victories {victory_names}",
        map_poles.id(),
        if randomize_civs { "randomized" } else { "fixed" },
    );
    println!(
        "batch: {maps} independent maps x seats 0/final x control/comparison = {} games; seed {seed}; {jobs} jobs",
        maps * 4
    );
    println!(
        "comparison: {}",
        if null {
            "NULL stock-action replay with recovery disabled"
        } else {
            "one rounded-midpoint lump-Gold recovery after stock play on the six-turn cadence"
        }
    );

    let results: Vec<MapResult> = civvis::parallel::map_reporting(
        maps,
        jobs,
        |map| {
            let profile = if deployment_mix {
                deployment_profile(map)
            } else {
                DeploymentProfile {
                    players,
                    width,
                    height,
                    city_states,
                    map_script,
                    map_topology: topology_for(map, &map_topologies),
                }
            };
            let options = GameOptions {
                speed: speed.clone(),
                map_script: profile.map_script,
                map_topology: profile.map_topology,
                map_poles,
                randomize_civs,
                ..GameOptions::new(
                    profile.players,
                    profile.width,
                    profile.height,
                    seed + map as u64,
                    nominal_turns,
                    profile.city_states,
                )
            };
            let seats = [0, profile.players - 1];
            let comparison_mode = if null {
                Mode::NullReplay
            } else {
                Mode::Recovery
            };
            let stock0 = play(
                options.clone(),
                seats[0],
                Mode::Stock,
                observe_through,
                null,
            );
            let compare0 = play(
                options.clone(),
                seats[0],
                comparison_mode,
                observe_through,
                null,
            );
            let exact0 = !null
                || (stock0.result == compare0.result && stock0.serialized == compare0.serialized);
            let stock1 = play(
                options.clone(),
                seats[1],
                Mode::Stock,
                observe_through,
                null,
            );
            let compare1 = play(options, seats[1], comparison_mode, observe_through, null);
            let exact1 = !null
                || (stock1.result == compare1.result && stock1.serialized == compare1.serialized);
            MapResult {
                control: [stock0.result, stock1.result],
                comparison: [compare0.result, compare1.result],
                exact: [exact0, exact1],
            }
        },
        |completed, _| eprintln!("progress: {}/{} maps complete", completed + 1, maps),
    );

    let mut summary = StudySummary::default();
    let mut exact_mismatches = 0usize;
    let mut helped_cells = 0usize;
    let mut hurt_cells = 0usize;
    for result in &results {
        summary.record(result);
        exact_mismatches += result.exact.iter().filter(|exact| !**exact).count();
        for (old, new) in result.control.iter().zip(&result.comparison) {
            match (old.won, new.won) {
                (false, true) => helped_cells += 1,
                (true, false) => hurt_cells += 1,
                _ => {}
            }
        }
    }
    let map_count = summary.maps.max(1) as f64;
    let score_delta = summary.score_delta / map_count;
    let sign_p = exact_two_sided(summary.favorable, summary.favorable + summary.adverse);
    let paired_win_score = summary.win_score / map_count;
    let terminal_score_share = summary.terminal_share / map_count;
    let control = &summary.control;
    let treatment = &summary.treatment;
    let gate = GateInputs {
        recovery_games: treatment.recovery_games,
        recoveries: treatment.recoveries,
        failed_applications: treatment.failed_applications,
        recovered_action_games: treatment.recovered_action_games,
        recovered_spy_actions: treatment.recovered_spy_actions,
        control_captives: control.captured_spies,
        treatment_captives: treatment.captured_spies,
        control_active: control.active_spies,
        treatment_active: treatment.active_spies,
        score_delta,
        favorable: summary.favorable,
        adverse: summary.adverse,
        sign_p,
        terminal_score_share,
        paired_win_score,
        control_wins: control.wins,
        treatment_wins: treatment.wins,
        control_science_wins: victory_count(control, "science"),
        treatment_science_wins: victory_count(treatment, "science"),
        control_culture_wins: victory_count(control, "culture"),
        treatment_culture_wins: victory_count(treatment, "culture"),
        control_domination_wins: victory_count(control, "domination"),
        treatment_domination_wins: victory_count(treatment, "domination"),
    };

    println!();
    println!(
        "arm        wins/games turns score  gold cities tech civic projects science captives active missions levels promos military"
    );
    for (name, arm) in [("control", control), ("treatment", treatment)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{name:<10} {:>3}/{:<3} {:>5.1} {:>5.1} {:>6.1} {:>6.2} {:>4.1} {:>5.1} {:>8.2} {:>7.3} {:>8} {:>6} {:>8} {:>6} {:>6} {:>8.1}",
            arm.wins,
            arm.games,
            arm.turns as f64 / n,
            arm.score as f64 / n,
            arm.gold / n,
            arm.cities as f64 / n,
            arm.techs as f64 / n,
            arm.civics as f64 / n,
            arm.science_projects as f64 / n,
            arm.science_progress / n,
            arm.captured_spies,
            arm.active_spies,
            arm.active_missions,
            arm.spy_levels,
            arm.spy_promotions,
            arm.military_power / n,
        );
    }
    println!(
        "victory types: control {:?}; treatment {:?}",
        control.victories, treatment.victories
    );
    println!(
        "lifetime endpoints (mean/seat): Culture {:.1}->{:.1}, Tourism {:.1}->{:.1}",
        control.culture_lifetime / control.games.max(1) as f64,
        treatment.culture_lifetime / treatment.games.max(1) as f64,
        control.tourism_lifetime / control.games.max(1) as f64,
        treatment.tourism_lifetime / treatment.games.max(1) as f64,
    );
    println!(
        "mechanism: cadence {}, peace opportunities {}, affordable {}; recoveries {} in {}/{} games, failures {}, Gold {:.1}, levels {}, promotions {}",
        treatment.cadence_turns,
        treatment.peace_opportunity_turns,
        treatment.affordable_turns,
        treatment.recoveries,
        treatment.recovery_games,
        treatment.games,
        treatment.failed_applications,
        treatment.gold_paid,
        treatment.recovered_levels,
        treatment.recovered_promotions,
    );
    println!(
        "recovered-spy follow-through: {} actions in {} games = {} assignments + {} promotions + {} missions; total trades {}->{}",
        treatment.recovered_spy_actions,
        treatment.recovered_action_games,
        treatment.recovered_assignments,
        treatment.recovered_promotions_used,
        treatment.recovered_missions,
        control.total_trade_actions,
        treatment.total_trade_actions,
    );
    println!(
        "matched seat cells: treatment helped {helped_cells}, hurt {hurt_cells}, unchanged {} (descriptive; map is the inference unit)",
        control.games - helped_cells - hurt_cells
    );
    println!(
        "paired maps: win score {:.1}%; terminal-score share {:.2}%; mean score delta {score_delta:+.2}; F/N/A {}/{}/{}; exact two-sided sign p={sign_p:.4}",
        100.0 * paired_win_score,
        100.0 * terminal_score_share,
        summary.favorable,
        summary.maps - summary.favorable - summary.adverse,
        summary.adverse,
    );

    if null {
        let official_null = deployment_mix
            && maps == NULL_MAPS
            && seed == NULL_SEED
            && nominal_turns == NOMINAL_TURNS
            && observe_through == OBSERVE_THROUGH
            && speed == "online"
            && map_poles == MapPoles::Poles
            && randomize_civs;
        if exact_mismatches == 0 {
            println!(
                "{}: PASS — all {} matched seat replays reproduced result, census, and serialized Game exactly",
                if official_null {
                    "preregistered null sanity"
                } else {
                    "diagnostic null sanity"
                },
                control.games
            );
        } else {
            println!(
                "null sanity: BROKEN — {exact_mismatches}/{} matched seat replays differed",
                control.games
            );
            std::process::exit(3);
        }
        return;
    }

    let exact_profile = deployment_mix
        && nominal_turns == NOMINAL_TURNS
        && observe_through == OBSERVE_THROUGH
        && speed == "online"
        && map_poles == MapPoles::Poles
        && randomize_civs;
    if exact_profile && maps == SCREEN_MAPS && seed == SCREEN_SEED {
        println!(
            "development gate: {}",
            if screen_passes(gate) {
                "PASS — run only the fixed disjoint holdout"
            } else {
                "STOP — at least one preregistered term failed; do not tune or retry"
            }
        );
    } else if exact_profile && maps == HOLDOUT_MAPS && seed == HOLDOUT_SEED {
        println!(
            "holdout gate: {}",
            if holdout_passes(gate) {
                "PASS — a separate gameplay integration PR is permitted"
            } else {
                "RETAIN AdvancedAi — no gameplay integration"
            }
        );
    } else {
        println!(
            "decision: DIAGNOSTIC ONLY — this is neither the preregistered screen nor holdout profile"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use civvis::game::Spy;

    fn captive_game() -> Game {
        let mut game = Game::new(2, 20, 14, 100_200, 20, 0);
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
        game.players[0].gold = 1_000.0;
        game.players[1].gold = 0.0;
        game.spies.insert(
            700,
            Spy {
                id: 700,
                owner: 0,
                level: 2,
                promotions: ["cat_burglar".to_string()].into_iter().collect(),
                city: None,
                ready_turn: u32::MAX,
                mission: None,
                sources_city: None,
                sources_until: 0,
                captured_by: Some(1),
            },
        );
        game
    }

    fn spy(id: u32, level: i64, promotions: &[&str], captor: usize) -> Spy {
        Spy {
            id,
            owner: 0,
            level,
            promotions: promotions.iter().map(|name| (*name).to_string()).collect(),
            city: None,
            ready_turn: u32::MAX,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: Some(captor),
        }
    }

    fn passing_gate() -> GateInputs {
        GateInputs {
            recovery_games: 12,
            recoveries: 18,
            failed_applications: 0,
            recovered_action_games: 8,
            recovered_spy_actions: 18,
            control_captives: 40,
            treatment_captives: 30,
            control_active: 20,
            treatment_active: 20,
            score_delta: 1.0,
            favorable: 8,
            adverse: 2,
            sign_p: 0.10,
            terminal_score_share: 0.505,
            paired_win_score: 0.50,
            control_wins: 3,
            treatment_wins: 3,
            control_science_wins: 1,
            treatment_science_wins: 1,
            control_culture_wins: 1,
            treatment_culture_wins: 1,
            control_domination_wins: 1,
            treatment_domination_wins: 1,
        }
    }

    #[test]
    fn deployment_cycle_is_the_frozen_rollover_population() {
        let profiles = (0..18).map(deployment_profile).collect::<Vec<_>>();
        assert_eq!(profiles[0].players, 4);
        assert_eq!(profiles[6].players, 9);
        assert_eq!(profiles[7].players, 4);
        assert_eq!(profiles[0].map_script, MapScript::LandOnly);
        assert_eq!(profiles[8].map_script, MapScript::Islands);
        assert_eq!(profiles[9].map_script, MapScript::LandOnly);
        assert_eq!(profiles[0].map_topology, MapTopology::Flat);
        assert_eq!(profiles[1].map_topology, MapTopology::Planet);
        assert_eq!(profiles[0].width, MapSize::for_players(4).width);
    }

    #[test]
    fn every_midpoint_has_the_frozen_half_gold_tail() {
        for level in 0..=4 {
            for promotions in 0..=4 {
                let midpoint = recovery_midpoint(level, promotions);
                assert!((midpoint.fract() - 0.5).abs() < 1e-12);
                assert!((recovery_payment(level, promotions) - midpoint - 0.5).abs() < 1e-12);
            }
        }
        assert_eq!(recovery_midpoint(0, 0), 207.5);
    }

    #[test]
    fn reserve_blocks_a_payment_that_crosses_it() {
        let payment = recovery_payment(0, 0);
        assert_eq!(payment, 208.0);
        assert!(!can_afford(247.0, payment));
        assert!(can_afford(248.0, payment));
        assert_eq!(cash_reserve(248.0), 40.0);
    }

    #[test]
    fn eligibility_excludes_every_frozen_diplomatic_case() {
        let game = captive_game();
        assert_eq!(eligible_quotes(&game, 0).len(), 1);

        let mut unmet = game.clone();
        unmet.players[0].met.remove(&1);
        assert!(eligible_quotes(&unmet, 0).is_empty());
        let mut war = game.clone();
        war.at_war.insert((0, 1));
        assert!(eligible_quotes(&war, 0).is_empty());
        let mut dead = game.clone();
        dead.players[1].alive = false;
        assert!(eligible_quotes(&dead, 0).is_empty());
        let mut minor = game.clone();
        minor.players[1].is_minor = true;
        assert!(eligible_quotes(&minor, 0).is_empty());
        let mut barbarian = game.clone();
        barbarian.players[1].is_barbarian = true;
        assert!(eligible_quotes(&barbarian, 0).is_empty());
        let mut self_held = game.clone();
        self_held.spies.get_mut(&700).unwrap().captured_by = Some(0);
        assert!(eligible_quotes(&self_held, 0).is_empty());
        let mut poor = game;
        poor.players[0].gold = 247.0;
        assert!(choose_recovery(&poor, 0).is_none());
    }

    #[test]
    fn ordering_is_level_then_promotions_then_lowest_id() {
        let mut game = captive_game();
        game.spies.clear();
        game.spies.insert(50, spy(50, 2, &["a"], 1));
        game.spies.insert(40, spy(40, 2, &["a", "b"], 1));
        game.spies.insert(30, spy(30, 2, &["a", "b"], 1));
        game.spies.insert(20, spy(20, 1, &["a", "b", "c"], 1));
        assert_eq!(choose_recovery(&game, 0).unwrap().spy, 30);
    }

    #[test]
    fn legal_recovery_transfers_exact_gold_and_releases_the_spy() {
        let mut game = captive_game();
        game.turn = 6;
        let owner_before = game.players[0].gold;
        let captor_before = game.players[1].gold;
        let payment = recovery_payment(2, 1);
        let mut state = RecoveryState::default();
        recover_one(&mut game, 0, &mut state);
        assert_eq!(state.census.recoveries, 1);
        assert_eq!(state.census.failed_applications, 0);
        assert_eq!(game.players[0].gold, owner_before - payment);
        assert_eq!(game.players[1].gold, captor_before + payment);
        assert_eq!(game.spies[&700].captured_by, None);
        assert_eq!(game.spies[&700].ready_turn, game.turn);
    }

    #[test]
    fn cadence_and_attempt_guard_limit_recovery_to_one() {
        let mut game = captive_game();
        game.spies.insert(701, spy(701, 1, &[], 1));
        let mut state = RecoveryState::default();
        game.turn = 5;
        recover_one(&mut game, 0, &mut state);
        assert_eq!(state.census.recoveries, 0);
        game.turn = 6;
        recover_one(&mut game, 0, &mut state);
        recover_one(&mut game, 0, &mut state);
        assert_eq!(state.census.cadence_turns, 1);
        assert_eq!(state.census.recoveries, 1);
        assert_eq!(
            game.spies
                .values()
                .filter(|spy| spy.captured_by.is_some())
                .count(),
            1
        );
    }

    #[test]
    fn null_action_log_replay_reconstructs_stock_world_and_controller() {
        let mut stock = Game::new(2, 20, 14, 100_201, 20, 0);
        stock.set_fog_memory(false);
        let mut replay = stock.clone();
        let mut stock_ai = AdvancedAi::new();
        let mut replay_ai = stock_ai.clone();
        stock_ai.take_turn(&mut stock, 0);
        let mut state = RecoveryState::default();
        replay_stock_turn(&mut replay, &mut replay_ai, 0, &mut state).unwrap();
        if replay.winner.is_none() && replay.current == 0 {
            replay.apply(0, &Action::EndTurn).unwrap();
        }
        assert_eq!(
            serde_json::to_string(&replay).unwrap(),
            serde_json::to_string(&stock).unwrap()
        );
        assert_eq!(replay_ai.plan_report(), stock_ai.plan_report());
    }

    #[test]
    fn follow_through_census_counts_only_focal_recovered_spies() {
        let mut state = RecoveryState::default();
        state.recovered.insert(700);
        let actions = vec![
            (0, Action::AssignSpy { spy: 700, city: 10 }),
            (
                0,
                Action::PromoteSpy {
                    spy: 700,
                    promotion: civvis::name::Name::new("cat_burglar"),
                },
            ),
            (
                0,
                Action::SpyMission {
                    spy: 700,
                    mission: "siphon_funds".to_string(),
                    target: (4, 5),
                },
            ),
            (0, Action::AssignSpy { spy: 701, city: 10 }),
            (1, Action::AssignSpy { spy: 700, city: 10 }),
            (
                0,
                Action::Trade {
                    player: 1,
                    offer: Box::new(DealItems {
                        gold: 1.0,
                        ..DealItems::default()
                    }),
                    request: Box::new(DealItems::default()),
                },
            ),
        ];

        record_actions(&actions, 0, &mut state);

        assert_eq!(state.census.recovered_spy_actions, 3);
        assert_eq!(state.census.recovered_assignments, 1);
        assert_eq!(state.census.recovered_promotions_used, 1);
        assert_eq!(state.census.recovered_missions, 1);
        assert_eq!(state.census.total_trade_actions, 1);
    }

    #[test]
    fn gates_reject_each_missing_mechanism_or_harm() {
        let passing = passing_gate();
        assert!(screen_passes(passing));
        assert!(!holdout_passes(passing));
        assert!(holdout_passes(GateInputs {
            sign_p: 0.049,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            recoveries: 17,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            recovery_games: 11,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            failed_applications: 1,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            recovered_spy_actions: 17,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            recovered_action_games: 7,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_captives: 31,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_active: 19,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            favorable: 2,
            adverse: 2,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            sign_p: 0.201,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            terminal_score_share: 0.499,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            score_delta: -0.01,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            paired_win_score: 0.499,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_wins: 2,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_science_wins: 0,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_culture_wins: 0,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_domination_wins: 0,
            ..passing
        }));
    }
}
