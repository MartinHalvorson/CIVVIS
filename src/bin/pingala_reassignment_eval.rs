//! Matched evaluation of adaptive Pingala reassignment.
//!
//! The focal treatment replays one complete stock `AdvancedAi` turn, retains
//! the controller state it produced, and then conditionally applies one legal
//! `ReassignGovernor` after stock play. No engine rule or shipped AI default
//! changes.
use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, Game, GameOptions, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use std::collections::{BTreeMap, BTreeSet};

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 10_029_999;
const SCREEN_MAPS: usize = 36;
const SCREEN_SEED: u64 = 10_030_000;
const HOLDOUT_MAPS: usize = 63;
const HOLDOUT_SEED: u64 = 10_031_000;
const NOMINAL_TURNS: u32 = 250;
const OBSERVE_THROUGH: u32 = 320;
const FIRST_REASSIGNMENT_TURN: u32 = 80;
const FINAL_VALUE_WINDOW: u32 = 20;
const CADENCE: u32 = 10;
const COOLDOWN: u32 = 40;
const MIN_LOYALTY: f64 = 90.0;
const MIN_ABSOLUTE_GAP: f64 = 180.0;
const MIN_RELATIVE_MULTIPLIER: f64 = 1.25;
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

fn deployment_conflicts(args: &[String]) -> Vec<&'static str> {
    PROFILE_OVERRIDE_FLAGS
        .iter()
        .copied()
        .filter(|flag| has_arg(args, flag))
        .collect()
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
    Reassignment,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RelocationChoice {
    source: u32,
    target: u32,
    source_score: f64,
    target_score: f64,
    source_pop: i32,
    target_pop: i32,
}

fn pingala_established(game: &Game, pid: usize) -> bool {
    let Some(state) = game
        .players
        .get(pid)
        .and_then(|player| player.governor_roster.get("pingala"))
    else {
        return false;
    };
    let Some(city) = state.city.and_then(|city| game.cities.get(&city)) else {
        return false;
    };
    let Some(spec) = game.rules.governors.get("pingala") else {
        return false;
    };
    city.owner == pid
        && game.turn >= state.assigned_turn + game.standard_duration(spec.establish_turns)
        && game.turn >= state.disabled_until
}

fn pingala_score(game: &Game, city_id: u32) -> f64 {
    let city = &game.cities[&city_id];
    let yields = game.city_yields(city_id);
    (100.0 - city.loyalty).max(0.0) * 2.0
        + city.pop as f64 * 14.0
        + yields.science * 9.0
        + yields.culture * 9.0
}

fn compare_scored_city(
    left: u32,
    left_score: f64,
    right: u32,
    right_score: f64,
) -> std::cmp::Ordering {
    left_score
        .total_cmp(&right_score)
        .then_with(|| right.cmp(&left))
}

fn candidate_relocation(game: &Game, pid: usize) -> Option<RelocationChoice> {
    if !pingala_established(game, pid) {
        return None;
    }
    let player = game.players.get(pid)?;
    let source = player.governor_roster.get("pingala")?.city?;
    let source_city = game.cities.get(&source)?;
    if source_city.owner != pid || source_city.loyalty + f64::EPSILON < MIN_LOYALTY {
        return None;
    }
    let occupied: BTreeSet<u32> = player
        .governor_roster
        .iter()
        .filter(|(governor, _)| governor.as_str() != "pingala")
        .filter_map(|(_, state)| state.city)
        .collect();
    let _memo = game.query_memo();
    let target = game
        .player_city_ids(pid)
        .into_iter()
        .filter(|city| !occupied.contains(city))
        .filter(|city| game.cities[city].loyalty + f64::EPSILON >= MIN_LOYALTY)
        .max_by(|left, right| {
            compare_scored_city(
                *left,
                pingala_score(game, *left),
                *right,
                pingala_score(game, *right),
            )
        })?;
    (target != source).then(|| RelocationChoice {
        source,
        target,
        source_score: pingala_score(game, source),
        target_score: pingala_score(game, target),
        source_pop: source_city.pop,
        target_pop: game.cities[&target].pop,
    })
}

fn absolute_gate(choice: RelocationChoice) -> bool {
    choice.target_score - choice.source_score + f64::EPSILON >= MIN_ABSOLUTE_GAP
}

fn relative_gate(choice: RelocationChoice) -> bool {
    choice.target_score + f64::EPSILON >= choice.source_score * MIN_RELATIVE_MULTIPLIER
}

fn cadence_open(turn: u32, pid: usize, observe_through: u32, last_relocation: Option<u32>) -> bool {
    turn >= FIRST_REASSIGNMENT_TURN
        && turn.saturating_add(FINAL_VALUE_WINDOW) <= observe_through
        && turn % CADENCE == pid as u32 % CADENCE
        && last_relocation.is_none_or(|last| turn >= last.saturating_add(COOLDOWN))
}

#[derive(Clone, Debug, Default, PartialEq)]
struct RelocationCensus {
    cadence_checks: u32,
    eligible_opportunities: u32,
    absolute_gate_passes: u32,
    relative_gate_passes: u32,
    relocations: u32,
    failed_applications: u32,
    established_followthrough: u32,
    pre_establishment_departures: u32,
    later_departures: u32,
    source_score_sum: f64,
    target_score_sum: f64,
    score_gap_sum: f64,
    source_population_sum: i64,
    target_population_sum: i64,
    population_gap_sum: i64,
}

#[derive(Clone, Copy, Debug)]
struct RelocationRecord {
    target: u32,
    established: bool,
    departed: bool,
}

#[derive(Default)]
struct RelocationState {
    census: RelocationCensus,
    last_relocation: Option<u32>,
    attempted_turns: BTreeSet<u32>,
    records: Vec<RelocationRecord>,
}

fn observe_followthrough(game: &Game, pid: usize, state: &mut RelocationState) {
    let assignment = game.players.get(pid).and_then(|player| {
        player
            .governor_roster
            .get("pingala")
            .and_then(|governor| governor.city)
    });
    let established = pingala_established(game, pid);
    for record in &mut state.records {
        if record.departed {
            continue;
        }
        if assignment == Some(record.target) {
            if established && !record.established {
                record.established = true;
                state.census.established_followthrough += 1;
            }
        } else {
            record.departed = true;
            if record.established {
                state.census.later_departures += 1;
            } else {
                state.census.pre_establishment_departures += 1;
            }
        }
    }
}

fn relocate_one(game: &mut Game, pid: usize, observe_through: u32, state: &mut RelocationState) {
    if !cadence_open(game.turn, pid, observe_through, state.last_relocation)
        || !state.attempted_turns.insert(game.turn)
    {
        return;
    }
    state.census.cadence_checks += 1;
    let Some(choice) = candidate_relocation(game, pid) else {
        return;
    };
    if choice.target_score <= choice.source_score + f64::EPSILON {
        return;
    };
    state.census.eligible_opportunities += 1;
    if !absolute_gate(choice) {
        return;
    }
    state.census.absolute_gate_passes += 1;
    if !relative_gate(choice) {
        return;
    }
    state.census.relative_gate_passes += 1;
    let action = Action::ReassignGovernor {
        governor: civvis::name::Name::new("pingala"),
        city: choice.target,
    };
    match game.apply(pid, &action) {
        Ok(()) => {
            state.census.relocations += 1;
            state.census.source_score_sum += choice.source_score;
            state.census.target_score_sum += choice.target_score;
            state.census.score_gap_sum += choice.target_score - choice.source_score;
            state.census.source_population_sum += i64::from(choice.source_pop);
            state.census.target_population_sum += i64::from(choice.target_pop);
            state.census.population_gap_sum += i64::from(choice.target_pop - choice.source_pop);
            state.last_relocation = Some(game.turn);
            state.records.push(RelocationRecord {
                target: choice.target,
                established: false,
                departed: false,
            });
        }
        Err(_) => state.census.failed_applications += 1,
    }
}

/// Run the stock controller on a clone, preserve its resulting state, and
/// replay every successful action except its final `EndTurn`.
fn replay_stock_turn(game: &mut Game, ai: &mut AdvancedAi, pid: usize) -> Result<(), String> {
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
    city_science: f64,
    city_culture: f64,
    great_person_points: f64,
    great_people_claimed: i64,
    low_loyalty_cities: usize,
    lost_capital: bool,
    pingala_assigned: bool,
    pingala_established: bool,
    pingala_population: i32,
    pingala_science: f64,
    pingala_culture: f64,
    census: RelocationCensus,
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
    let mut state = RelocationState::default();

    while game.winner.is_none() && game.turn <= observe_through {
        assert_eq!(
            game.max_turns, policy_max_turns,
            "external continuation changed the policy-visible horizon"
        );
        let pid = game.current;
        if pid == focal {
            observe_followthrough(&game, focal, &mut state);
        }
        if pid == focal && mode != Mode::Stock {
            replay_stock_turn(&mut game, &mut ais[pid], pid)
                .unwrap_or_else(|why| panic!("turn {} seat {pid}: {why}", game.turn));
            if mode == Mode::Reassignment && game.winner.is_none() && game.current == pid {
                relocate_one(&mut game, pid, observe_through, &mut state);
            }
        } else {
            ais[pid].take_turn(&mut game, pid);
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
    observe_followthrough(&game, focal, &mut state);

    let player = &game.players[focal];
    let city_ids = game.player_city_ids(focal);
    let _memo = game.query_memo();
    let (city_science, city_culture) = city_ids.iter().fold((0.0, 0.0), |total, city| {
        let yields = game.city_yields(*city);
        (total.0 + yields.science, total.1 + yields.culture)
    });
    let pingala_city = player
        .governor_roster
        .get("pingala")
        .and_then(|governor| governor.city)
        .and_then(|city| {
            game.cities
                .get(&city)
                .filter(|candidate| candidate.owner == focal)
        });
    let pingala_yields = pingala_city.map(|city| game.city_yields(city.id));
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
        city_science,
        city_culture,
        great_person_points: player.gpp.values().sum(),
        great_people_claimed: player.gp_claimed.values().sum(),
        low_loyalty_cities: city_ids
            .iter()
            .filter(|city| game.cities[city].loyalty < 70.0)
            .count(),
        lost_capital: game
            .cities
            .values()
            .any(|city| city.is_capital && city.original_owner == focal && city.owner != focal),
        pingala_assigned: pingala_city.is_some(),
        pingala_established: pingala_established(&game, focal),
        pingala_population: pingala_city.map_or(0, |city| city.pop),
        pingala_science: pingala_yields.map_or(0.0, |yields| yields.science),
        pingala_culture: pingala_yields.map_or(0.0, |yields| yields.culture),
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
    city_science: f64,
    city_culture: f64,
    great_person_points: f64,
    great_people_claimed: i64,
    low_loyalty_cities: usize,
    lost_capitals: usize,
    pingala_assigned: usize,
    pingala_established: usize,
    pingala_population: i64,
    pingala_science: f64,
    pingala_culture: f64,
    cadence_checks: u64,
    eligible_opportunities: u64,
    absolute_gate_passes: u64,
    relative_gate_passes: u64,
    relocations: u64,
    failed_applications: u64,
    established_followthrough: u64,
    pre_establishment_departures: u64,
    later_departures: u64,
    source_score_sum: f64,
    target_score_sum: f64,
    score_gap_sum: f64,
    source_population_sum: i64,
    target_population_sum: i64,
    population_gap_sum: i64,
    relocation_games: usize,
    establishment_games: usize,
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
        self.city_science += result.city_science;
        self.city_culture += result.city_culture;
        self.great_person_points += result.great_person_points;
        self.great_people_claimed += result.great_people_claimed;
        self.low_loyalty_cities += result.low_loyalty_cities;
        self.lost_capitals += result.lost_capital as usize;
        self.pingala_assigned += result.pingala_assigned as usize;
        self.pingala_established += result.pingala_established as usize;
        self.pingala_population += i64::from(result.pingala_population);
        self.pingala_science += result.pingala_science;
        self.pingala_culture += result.pingala_culture;
        let census = &result.census;
        self.cadence_checks += census.cadence_checks as u64;
        self.eligible_opportunities += census.eligible_opportunities as u64;
        self.absolute_gate_passes += census.absolute_gate_passes as u64;
        self.relative_gate_passes += census.relative_gate_passes as u64;
        self.relocations += census.relocations as u64;
        self.failed_applications += census.failed_applications as u64;
        self.established_followthrough += census.established_followthrough as u64;
        self.pre_establishment_departures += census.pre_establishment_departures as u64;
        self.later_departures += census.later_departures as u64;
        self.source_score_sum += census.source_score_sum;
        self.target_score_sum += census.target_score_sum;
        self.score_gap_sum += census.score_gap_sum;
        self.source_population_sum += census.source_population_sum;
        self.target_population_sum += census.target_population_sum;
        self.population_gap_sum += census.population_gap_sum;
        self.relocation_games += (census.relocations > 0) as usize;
        self.establishment_games += (census.established_followthrough > 0) as usize;
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
    science_delta: f64,
    science_share: f64,
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
        self.science_share += result
            .control
            .iter()
            .zip(&result.comparison)
            .map(|(old, new)| paired_share(old.science_progress, new.science_progress))
            .sum::<f64>()
            / 2.0;
        self.science_delta += result
            .control
            .iter()
            .zip(&result.comparison)
            .map(|(old, new)| new.science_progress - old.science_progress)
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
    relocation_games: usize,
    relocations: u64,
    failed_applications: u64,
    establishment_games: usize,
    established_followthrough: u64,
    control_low_loyalty: usize,
    treatment_low_loyalty: usize,
    control_lost_capitals: usize,
    treatment_lost_capitals: usize,
    score_delta: f64,
    science_delta: f64,
    favorable: usize,
    adverse: usize,
    sign_p: f64,
    terminal_score_share: f64,
    science_progress_share: f64,
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
    gate.relocations >= 6
        && gate.relocation_games >= 5
        && gate.failed_applications == 0
        && gate.established_followthrough >= 4
        && gate.establishment_games >= 3
}

fn safety_passes(gate: GateInputs) -> bool {
    gate.treatment_low_loyalty <= gate.control_low_loyalty
        && gate.treatment_lost_capitals <= gate.control_lost_capitals
}

fn victory_types_nonlower(gate: GateInputs) -> bool {
    gate.treatment_science_wins >= gate.control_science_wins
        && gate.treatment_culture_wins >= gate.control_culture_wins
        && gate.treatment_domination_wins >= gate.control_domination_wins
}

fn screen_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate)
        && safety_passes(gate)
        && gate.favorable > gate.adverse
        && gate.sign_p <= 0.20
        && gate.terminal_score_share >= 0.50
        && gate.score_delta >= 0.0
        && gate.science_progress_share >= 0.50
        && gate.science_delta >= 0.0
        && gate.paired_win_score >= 0.50
        && gate.treatment_wins >= gate.control_wins
        && victory_types_nonlower(gate)
}

fn holdout_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate)
        && safety_passes(gate)
        && gate.favorable > gate.adverse
        && gate.sign_p < 0.05
        && gate.terminal_score_share >= 0.505
        && gate.score_delta > 0.0
        && gate.science_progress_share >= 0.505
        && gate.science_delta > 0.0
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
        let conflicts = deployment_conflicts(&args);
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

    println!("Adaptive Pingala reassignment evaluator");
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
            "NULL stock-action replay with reassignment disabled"
        } else {
            "one thresholded legal Pingala reassignment after stock play, at most every 40 turns"
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
                Mode::Reassignment
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
    let science_delta = summary.science_delta / map_count;
    let science_progress_share = summary.science_share / map_count;
    let control = &summary.control;
    let treatment = &summary.treatment;
    let gate = GateInputs {
        relocation_games: treatment.relocation_games,
        relocations: treatment.relocations,
        failed_applications: treatment.failed_applications,
        establishment_games: treatment.establishment_games,
        established_followthrough: treatment.established_followthrough,
        control_low_loyalty: control.low_loyalty_cities,
        treatment_low_loyalty: treatment.low_loyalty_cities,
        control_lost_capitals: control.lost_capitals,
        treatment_lost_capitals: treatment.lost_capitals,
        score_delta,
        science_delta,
        favorable: summary.favorable,
        adverse: summary.adverse,
        sign_p,
        terminal_score_share,
        science_progress_share,
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
        "arm        wins/games turns score cities tech civic projects progress city-sci city-cult GPP claimed low-loy lost-cap military"
    );
    for (name, arm) in [("control", control), ("treatment", treatment)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{name:<10} {:>3}/{:<3} {:>5.1} {:>5.1} {:>6.2} {:>4.1} {:>5.1} {:>8.2} {:>8.3} {:>8.1} {:>9.1} {:>5.1} {:>7.1} {:>7} {:>8} {:>8.1}",
            arm.wins,
            arm.games,
            arm.turns as f64 / n,
            arm.score as f64 / n,
            arm.cities as f64 / n,
            arm.techs as f64 / n,
            arm.civics as f64 / n,
            arm.science_projects as f64 / n,
            arm.science_progress / n,
            arm.city_science / n,
            arm.city_culture / n,
            arm.great_person_points / n,
            arm.great_people_claimed as f64 / n,
            arm.low_loyalty_cities,
            arm.lost_capitals,
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
        "Pingala terminal: assigned {}->{}, established {}->{}, mean population {:.1}->{:.1}, city Science {:.1}->{:.1}, city Culture {:.1}->{:.1}",
        control.pingala_assigned,
        treatment.pingala_assigned,
        control.pingala_established,
        treatment.pingala_established,
        control.pingala_population as f64 / control.pingala_assigned.max(1) as f64,
        treatment.pingala_population as f64 / treatment.pingala_assigned.max(1) as f64,
        control.pingala_science / control.pingala_assigned.max(1) as f64,
        treatment.pingala_science / treatment.pingala_assigned.max(1) as f64,
        control.pingala_culture / control.pingala_assigned.max(1) as f64,
        treatment.pingala_culture / treatment.pingala_assigned.max(1) as f64,
    );
    println!(
        "mechanism: cadence {}, better-city opportunities {}, absolute passes {}, relative passes {}; relocations {} in {}/{} games, failures {}",
        treatment.cadence_checks,
        treatment.eligible_opportunities,
        treatment.absolute_gate_passes,
        treatment.relative_gate_passes,
        treatment.relocations,
        treatment.relocation_games,
        treatment.games,
        treatment.failed_applications,
    );
    println!(
        "relocation follow-through: {} established in {} games; departures before establishment {}, later departures {}; mean score {:.1}->{:.1} (gap {:.1}), population {:.2}->{:.2} (gap {:+.2})",
        treatment.established_followthrough,
        treatment.establishment_games,
        treatment.pre_establishment_departures,
        treatment.later_departures,
        treatment.source_score_sum / treatment.relocations.max(1) as f64,
        treatment.target_score_sum / treatment.relocations.max(1) as f64,
        treatment.score_gap_sum / treatment.relocations.max(1) as f64,
        treatment.source_population_sum as f64 / treatment.relocations.max(1) as f64,
        treatment.target_population_sum as f64 / treatment.relocations.max(1) as f64,
        treatment.population_gap_sum as f64 / treatment.relocations.max(1) as f64,
    );
    println!(
        "matched seat cells: treatment helped {helped_cells}, hurt {hurt_cells}, unchanged {} (descriptive; map is the inference unit)",
        control.games - helped_cells - hurt_cells
    );
    println!(
        "paired maps: win score {:.1}%; terminal-score share {:.2}%, delta {score_delta:+.2}; Science-progress share {:.2}%, delta {science_delta:+.3}; score F/N/A {}/{}/{}; exact two-sided sign p={sign_p:.4}",
        100.0 * paired_win_score,
        100.0 * terminal_score_share,
        100.0 * science_progress_share,
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
    use civvis::game::{GovernorState, Unit};
    use std::cmp::Ordering;

    fn multi_city_game(city_count: usize) -> Game {
        assert!(city_count >= 2);
        let mut game = Game::new_full(1, 30, 20, 100_400, 200, 0, false);
        let settler_id = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("fixture needs its starting Settler");
        let template = game.units[&settler_id].clone();
        game.apply(0, &Action::FoundCity { unit: settler_id })
            .expect("starting position must found a capital");

        while game.player_city_ids(0).len() < city_count {
            let existing = game.player_city_ids(0);
            let positions = game.map.tiles.keys().copied().collect::<Vec<_>>();
            let mut founded = None;
            for position in positions {
                if existing
                    .iter()
                    .any(|city| game.wdist(game.cities[city].pos, position) < 4)
                {
                    continue;
                }
                let mut trial = game.clone();
                let unit_id = trial.next_id;
                trial.next_id += 1;
                let mut settler: Unit = template.clone();
                settler.id = unit_id;
                settler.pos = position;
                settler.owner = 0;
                settler.moves_left = 10.0;
                trial.units.insert(unit_id, settler);
                if trial.apply(0, &Action::FoundCity { unit: unit_id }).is_ok() {
                    founded = Some(trial);
                    break;
                }
            }
            game = founded.expect("fixture map needs another legal city site");
        }
        game
    }

    fn established_pingala(game: &mut Game, source: u32) {
        game.turn = 100;
        game.players[0].governor_roster.insert(
            "pingala".to_string(),
            GovernorState {
                city: Some(source),
                assigned_turn: 0,
                disabled_until: 0,
                promotions: BTreeSet::new(),
            },
        );
        game.players[0].governors = vec![source];
        assert!(pingala_established(game, 0));
    }

    fn strong_target_game() -> (Game, u32, u32) {
        let mut game = multi_city_game(2);
        let cities = game.player_city_ids(0);
        let source = cities[0];
        let target = cities[1];
        game.cities.get_mut(&source).unwrap().pop = 1;
        game.cities.get_mut(&target).unwrap().pop = 50;
        game.cities.get_mut(&source).unwrap().loyalty = 100.0;
        game.cities.get_mut(&target).unwrap().loyalty = 100.0;
        established_pingala(&mut game, source);
        assert_eq!(candidate_relocation(&game, 0).unwrap().target, target);
        (game, source, target)
    }

    fn passing_gate() -> GateInputs {
        GateInputs {
            relocation_games: 5,
            relocations: 6,
            failed_applications: 0,
            establishment_games: 3,
            established_followthrough: 4,
            control_low_loyalty: 2,
            treatment_low_loyalty: 2,
            control_lost_capitals: 1,
            treatment_lost_capitals: 1,
            score_delta: 1.0,
            science_delta: 0.1,
            favorable: 8,
            adverse: 2,
            sign_p: 0.10,
            terminal_score_share: 0.505,
            science_progress_share: 0.505,
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
    fn deployment_cycle_and_override_contract_are_frozen() {
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

        let args = vec![
            "--deployment-mix".to_string(),
            "--players".to_string(),
            "8".to_string(),
            "--shape".to_string(),
            "planet".to_string(),
        ];
        assert_eq!(deployment_conflicts(&args), vec!["--players", "--shape"]);
    }

    #[test]
    fn score_thresholds_and_lowest_city_tie_break_are_exact() {
        let (game, source, _) = strong_target_game();
        let city = &game.cities[&source];
        let yields = game.city_yields(source);
        assert_eq!(
            pingala_score(&game, source),
            (100.0 - city.loyalty).max(0.0) * 2.0
                + city.pop as f64 * 14.0
                + yields.science * 9.0
                + yields.culture * 9.0
        );
        let base = RelocationChoice {
            source: 1,
            target: 2,
            source_score: 720.0,
            target_score: 900.0,
            source_pop: 10,
            target_pop: 15,
        };
        assert!(absolute_gate(base));
        assert!(relative_gate(base));
        assert!(!absolute_gate(RelocationChoice {
            target_score: 899.9,
            ..base
        }));
        assert!(!relative_gate(RelocationChoice {
            source_score: 721.0,
            ..base
        }));
        assert_eq!(compare_scored_city(3, 10.0, 4, 10.0), Ordering::Greater);
        assert_eq!(compare_scored_city(4, 10.0, 3, 10.0), Ordering::Less);
        assert_eq!(compare_scored_city(9, 11.0, 1, 10.0), Ordering::Greater);
    }

    #[test]
    fn cadence_floor_cooldown_and_final_window_are_exact() {
        assert!(!cadence_open(79, 0, 320, None));
        assert!(cadence_open(80, 0, 320, None));
        assert!(!cadence_open(81, 0, 320, None));
        assert!(!cadence_open(110, 0, 320, Some(80)));
        assert!(cadence_open(120, 0, 320, Some(80)));
        assert!(cadence_open(300, 0, 320, None));
        assert!(!cadence_open(301, 1, 320, Some(261)));
        assert!(!cadence_open(310, 0, 320, None));
    }

    #[test]
    fn eligibility_rejects_unestablished_disabled_unsafe_foreign_and_occupied_states() {
        let (game, _source, target) = strong_target_game();
        assert!(candidate_relocation(&game, 0).is_some());

        let mut unestablished = game.clone();
        unestablished.players[0]
            .governor_roster
            .get_mut("pingala")
            .unwrap()
            .assigned_turn = unestablished.turn;
        assert!(candidate_relocation(&unestablished, 0).is_none());

        let mut disabled = game.clone();
        disabled.players[0]
            .governor_roster
            .get_mut("pingala")
            .unwrap()
            .disabled_until = disabled.turn + 1;
        assert!(candidate_relocation(&disabled, 0).is_none());

        let mut unsafe_source = game.clone();
        let source = unsafe_source.players[0].governor_roster["pingala"]
            .city
            .unwrap();
        unsafe_source.cities.get_mut(&source).unwrap().loyalty = 89.9;
        assert!(candidate_relocation(&unsafe_source, 0).is_none());

        let mut unsafe_target = game.clone();
        unsafe_target.cities.get_mut(&target).unwrap().loyalty = 89.9;
        assert!(candidate_relocation(&unsafe_target, 0).is_none());

        let mut foreign = game.clone();
        foreign.cities.get_mut(&target).unwrap().owner = 1;
        assert!(candidate_relocation(&foreign, 0).is_none());

        let mut occupied = game;
        occupied.players[0].governor_roster.insert(
            "magnus".to_string(),
            GovernorState {
                city: Some(target),
                assigned_turn: 0,
                disabled_until: 0,
                promotions: BTreeSet::new(),
            },
        );
        assert!(candidate_relocation(&occupied, 0).is_none());
    }

    #[test]
    fn legal_relocation_uses_engine_establishment_and_counts_once() {
        let (mut game, source, target) = strong_target_game();
        let mut state = RelocationState::default();
        relocate_one(&mut game, 0, 320, &mut state);
        relocate_one(&mut game, 0, 320, &mut state);
        assert_eq!(state.census.relocations, 1);
        assert_eq!(state.census.failed_applications, 0);
        assert_eq!(
            game.players[0].governor_roster["pingala"].city,
            Some(target)
        );
        assert_eq!(
            game.players[0].governor_roster["pingala"].assigned_turn,
            100
        );
        assert!(!pingala_established(&game, 0));
        assert_eq!(game.players[0].governors, vec![target]);

        game.turn += game.standard_duration(game.rules.governors["pingala"].establish_turns);
        observe_followthrough(&game, 0, &mut state);
        assert_eq!(state.census.established_followthrough, 1);

        game.apply(
            0,
            &Action::ReassignGovernor {
                governor: civvis::name::Name::new("pingala"),
                city: source,
            },
        )
        .unwrap();
        observe_followthrough(&game, 0, &mut state);
        assert_eq!(state.census.later_departures, 1);
    }

    #[test]
    fn followthrough_census_counts_only_recorded_targets() {
        let (game, source, target) = strong_target_game();
        let mut state = RelocationState::default();
        state.records.push(RelocationRecord {
            target,
            established: false,
            departed: false,
        });
        observe_followthrough(&game, 0, &mut state);
        assert_eq!(state.census.pre_establishment_departures, 1);
        assert_eq!(state.census.established_followthrough, 0);

        let mut unrelated = RelocationState::default();
        unrelated.records.push(RelocationRecord {
            target: source,
            established: false,
            departed: false,
        });
        observe_followthrough(&game, 0, &mut unrelated);
        assert_eq!(unrelated.census.established_followthrough, 1);
        observe_followthrough(&game, 0, &mut unrelated);
        assert_eq!(unrelated.census.established_followthrough, 1);
    }

    #[test]
    fn null_action_log_replay_reconstructs_stock_world_and_controller() {
        let mut stock = Game::new(2, 20, 14, 100_401, 20, 0);
        stock.set_fog_memory(false);
        let mut replay = stock.clone();
        let mut stock_ai = AdvancedAi::new();
        let mut replay_ai = stock_ai.clone();
        stock_ai.take_turn(&mut stock, 0);
        replay_stock_turn(&mut replay, &mut replay_ai, 0).unwrap();
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
    fn gates_reject_every_missing_mechanism_or_harm() {
        let passing = passing_gate();
        assert!(screen_passes(passing));
        assert!(!holdout_passes(passing));
        assert!(holdout_passes(GateInputs {
            sign_p: 0.049,
            ..passing
        }));
        let holdout = GateInputs {
            sign_p: 0.049,
            ..passing
        };
        assert!(!holdout_passes(GateInputs {
            terminal_score_share: 0.5049,
            ..holdout
        }));
        assert!(!holdout_passes(GateInputs {
            score_delta: 0.0,
            ..holdout
        }));
        assert!(!holdout_passes(GateInputs {
            science_progress_share: 0.5049,
            ..holdout
        }));
        assert!(!holdout_passes(GateInputs {
            science_delta: 0.0,
            ..holdout
        }));
        assert!(!screen_passes(GateInputs {
            relocations: 5,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            relocation_games: 4,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            failed_applications: 1,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            established_followthrough: 3,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            establishment_games: 2,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_low_loyalty: 3,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_lost_capitals: 2,
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
            science_progress_share: 0.499,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            science_delta: -0.01,
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
