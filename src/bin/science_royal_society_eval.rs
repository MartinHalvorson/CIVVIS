//! Matched evaluation of Royal Society as the Science Tier-3 plaza choice.
//!
//! The treatment observes one complete stock `AdvancedAi` turn, retains the
//! controller state it produced, and replays its successful actions with one
//! legal same-cost substitution: when the plan in force is Science, replace
//! `Produce(NationalHistoryMuseum)` with `Produce(RoyalSociety)` in the same
//! city. Every map is replayed from seats 0 and N-1 with and without the
//! treatment, and inference is aggregated by map.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::evolve::Champion;
use civvis::game::{Action, Game, GameOptions, Item, VictoryConditions};
use civvis::name::Name;
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use std::collections::BTreeMap;

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 9_987_999;
const SCREEN_MAPS: usize = 30;
const SCREEN_SEED: u64 = 9_988_000;
const HOLDOUT_MAPS: usize = 120;
const HOLDOUT_SEED: u64 = 9_989_000;
const NOMINAL_TURNS: u32 = 250;
const OBSERVE_THROUGH: u32 = 320;
const FROZEN_AI: &str = "advanced_evolved";
const FROZEN_CHAMPION_GENERATION: u32 = 14;
const FROZEN_CHAMPION_FNV1A: u64 = 0x40b1_fbb2_a5b8_8bc6;
const EMBEDDED_CHAMPION: &str = include_str!("../../data/evolved/best.json");
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
const PROFILE_OVERRIDE_FLAGS: [&str; 6] = [
    "--players",
    "--width",
    "--height",
    "--city-states",
    "--map",
    "--shape",
];
const FLAG_OPTIONS: [&str; 3] = ["--null", "--deployment-mix", "--randomize-civs"];
const VALUE_OPTIONS: [&str; 15] = [
    "--maps",
    "--players",
    "--width",
    "--height",
    "--city-states",
    "--turns",
    "--observe-through",
    "--seed",
    "--jobs",
    "--speed",
    "--map",
    "--shape",
    "--poles",
    "--victories",
    "--ai",
];
const REQUIRED_SCIENCE_PROJECTS: [&str; 4] = [
    "launch_earth_satellite",
    "launch_moon_landing",
    "launch_mars_colony",
    "exoplanet_expedition",
];

const fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}

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

fn validate_args(args: &[String]) -> Result<(), String> {
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        if FLAG_OPTIONS.contains(&argument) {
            index += 1;
        } else if VALUE_OPTIONS.contains(&argument) {
            match args.get(index + 1).map(String::as_str) {
                Some(value) if !value.starts_with("--") => index += 2,
                _ => return Err(format!("{argument} requires a value")),
            }
        } else {
            return Err(format!("unsupported argument {argument:?}"));
        }
    }
    Ok(())
}

/// Validate every supplied occurrence. A malformed duplicate must not hide
/// behind an earlier valid value and accidentally reach a diagnostic run.
fn option_values<'a>(args: &'a [String], key: &str) -> Result<Vec<&'a str>, String> {
    let mut values = Vec::new();
    for (index, argument) in args.iter().enumerate() {
        if argument != key {
            continue;
        }
        match args.get(index + 1).map(String::as_str) {
            Some(value) if !value.starts_with("--") => values.push(value),
            _ => return Err(format!("{key} requires a value")),
        }
    }
    Ok(values)
}

fn option_value<'a>(args: &'a [String], key: &str) -> Result<Option<&'a str>, String> {
    Ok(option_values(args, key)?.into_iter().next())
}

fn number_value(args: &[String], key: &str) -> Result<Option<i64>, String> {
    let values = option_values(args, key)?;
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        parsed.push(
            value
                .parse::<i64>()
                .map_err(|_| format!("{key} requires an integer value; got {value:?}"))?,
        );
    }
    Ok(parsed.into_iter().next())
}

fn number(args: &[String], key: &str, default: i64) -> i64 {
    number_value(args, key)
        .unwrap_or_else(|why| {
            eprintln!("{why}");
            std::process::exit(2);
        })
        .unwrap_or(default)
}

fn text(args: &[String], key: &str, default: &str) -> String {
    option_value(args, key)
        .unwrap_or_else(|why| {
            eprintln!("{why}");
            std::process::exit(2);
        })
        .unwrap_or(default)
        .to_string()
}

fn has_exact_value(args: &[String], key: &str, value: &str) -> bool {
    args.iter().filter(|arg| arg.as_str() == key).count() == 1
        && args
            .windows(2)
            .any(|pair| pair[0] == key && pair[1] == value)
}

fn has_exact_number(args: &[String], key: &str, value: i64) -> bool {
    has_exact_value(args, key, &value.to_string())
}

fn has_exact_flag(args: &[String], key: &str) -> bool {
    args.iter().filter(|arg| arg.as_str() == key).count() == 1
}

fn frozen_champion() -> Champion {
    assert_eq!(
        fnv1a(EMBEDDED_CHAMPION.as_bytes()),
        FROZEN_CHAMPION_FNV1A,
        "data/evolved/best.json changed after the Royal Society preregistration"
    );
    let champion: Champion = serde_json::from_str(EMBEDDED_CHAMPION)
        .expect("the committed advanced_evolved champion must be valid JSON");
    assert_eq!(
        champion.gen, FROZEN_CHAMPION_GENERATION,
        "Royal Society evaluator champion generation changed"
    );
    champion
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Stock,
    NullReplay,
    RoyalSociety,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ChoiceCensus {
    opportunities: u32,
    substitutions: u32,
    contributions: u32,
}

fn science_plan(ai: &AdvancedAi) -> bool {
    ai.plan_report()
        .is_some_and(|plan| plan.strategy == "science")
}

fn is_national_history_choice(action: &Action) -> bool {
    matches!(
        action,
        Action::Produce {
            item: Item::Building { building },
            ..
        } if building == "national_history_museum"
    )
}

fn royal_society_replacement(action: &Action, science: bool) -> Option<Action> {
    if !science {
        return None;
    }
    let Action::Produce {
        city,
        item: Item::Building { building },
    } = action
    else {
        return None;
    };
    (building == "national_history_museum").then(|| Action::Produce {
        city: *city,
        item: Item::Building {
            building: Name::new("royal_society"),
        },
    })
}

fn count_opportunities(actions: &[(usize, Action)], pid: usize, science: bool) -> u32 {
    if !science {
        return 0;
    }
    actions
        .iter()
        .filter(|(owner, action)| *owner == pid && is_national_history_choice(action))
        .count() as u32
}

/// Run the stock controller on a clone, preserve the controller state it
/// reached, and replay every successful action except the final EndTurn.
/// Substitution changes one logged `Produce` action through `Game::apply`.
fn replay_stock_turn(
    game: &mut Game,
    ai: &mut AdvancedAi,
    pid: usize,
    substitute: bool,
    census: &mut ChoiceCensus,
) -> Result<(), String> {
    let mut observed = game.clone();
    let policy_max_turns = game.max_turns;
    if observed.max_turns != policy_max_turns {
        return Err(format!(
            "stock seat {pid} clone changed policy horizon from {policy_max_turns} to {}",
            observed.max_turns
        ));
    }
    let before = observed.log.len();
    let mut actor = ai.clone();
    actor.take_turn(&mut observed, pid);
    if observed.max_turns != policy_max_turns {
        return Err(format!(
            "stock seat {pid} changed policy horizon from {policy_max_turns} to {}",
            observed.max_turns
        ));
    }
    let science = science_plan(&actor);
    let mut actions: Vec<(usize, Action)> = observed.log.since(before).cloned().collect();
    census.opportunities += count_opportunities(&actions, pid, science);

    let ended = actions
        .last()
        .is_some_and(|(owner, action)| *owner == pid && matches!(action, Action::EndTurn));
    if !ended && observed.winner.is_none() {
        return Err(format!(
            "stock seat {pid} did not finish its trace with EndTurn"
        ));
    }
    if ended {
        actions.pop();
    }

    for (owner, stock) in actions {
        if owner != pid {
            return Err(format!(
                "stock seat {pid} logged an action for seat {owner}: {stock:?}"
            ));
        }
        let replacement = substitute
            .then(|| royal_society_replacement(&stock, science))
            .flatten();
        let action = replacement.as_ref().unwrap_or(&stock);
        game.apply(owner, action).map_err(|why| {
            format!(
                "stock action replay failed for seat {pid}: {why}; stock={stock:?}; replay={action:?}"
            )
        })?;
        census.substitutions += replacement.is_some() as u32;
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
    let expedition_launched = game.players[pid]
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
    let distance = if expedition_launched {
        game.players[pid].exoplanet_distance.clamp(0.0, 50.0) / 50.0
    } else {
        0.0
    };
    (completed, completed as f64 + distance)
}

fn empire_building_count(game: &Game, pid: usize, building: &str) -> usize {
    game.cities
        .values()
        .filter(|city| city.owner == pid)
        .map(|city| {
            city.buildings
                .iter()
                .filter(|held| held.as_str() == building)
                .count()
        })
        .sum()
}

#[derive(Clone, Debug, PartialEq)]
struct GameResult {
    won: bool,
    victory: Option<String>,
    reported_turn: u32,
    score: i64,
    faith: f64,
    cities: usize,
    builders: usize,
    national_history_museums: usize,
    royal_societies: usize,
    science_projects: usize,
    science_progress: f64,
    census: ChoiceCensus,
}

struct Played {
    result: GameResult,
    serialized: Option<String>,
}

fn play(
    options: GameOptions,
    focal: usize,
    mode: Mode,
    serialize: bool,
    weights: &Weights,
    observe_through: u32,
) -> Played {
    let mut game = Game::new_with(options);
    let policy_max_turns = game.max_turns;
    assert!(observe_through >= policy_max_turns);
    game.set_fog_memory(false);
    game.victory_conditions = VictoryConditions {
        science: true,
        culture: true,
        religious: false,
        diplomatic: false,
        domination: true,
        score: false,
    };
    let mut ais = AdvancedAi::fleet_weighted(&game, weights);
    let mut census = ChoiceCensus::default();

    while game.winner.is_none() && game.turn <= observe_through {
        assert_eq!(
            game.max_turns, policy_max_turns,
            "external continuation changed the policy-visible horizon"
        );
        let pid = game.current;
        let before = game.log.len();
        if pid == focal && mode != Mode::Stock {
            replay_stock_turn(
                &mut game,
                &mut ais[pid],
                pid,
                mode == Mode::RoyalSociety,
                &mut census,
            )
            .unwrap_or_else(|why| panic!("turn {} seat {pid}: {why}", game.turn));
        } else {
            ais[pid].take_turn(&mut game, pid);
            if pid == focal {
                let actions: Vec<(usize, Action)> = game.log.since(before).cloned().collect();
                census.opportunities += count_opportunities(&actions, pid, science_plan(&ais[pid]));
            }
        }
        assert_eq!(
            game.max_turns, policy_max_turns,
            "controller changed the policy-visible horizon"
        );
        if game.winner.is_none() && game.current == pid {
            game.apply(pid, &Action::EndTurn).unwrap_or_else(|why| {
                panic!("turn {} seat {pid}: EndTurn failed: {why}", game.turn)
            });
        }
        assert_eq!(
            game.max_turns, policy_max_turns,
            "turn progression changed the policy-visible horizon"
        );
        if pid == focal {
            census.contributions += game
                .log
                .since(before)
                .filter(|(owner, action)| {
                    *owner == pid && matches!(action, Action::ContributeProject { .. })
                })
                .count() as u32;
        }
    }
    assert_eq!(
        game.max_turns, policy_max_turns,
        "external continuation changed the policy-visible horizon"
    );

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
        score: game.score(focal),
        faith: game.players[focal].faith,
        cities: game.player_city_ids(focal).len(),
        builders: game
            .units
            .values()
            .filter(|unit| unit.owner == focal && unit.kind == "builder")
            .count(),
        national_history_museums: empire_building_count(&game, focal, "national_history_museum"),
        royal_societies: empire_building_count(&game, focal, "royal_society"),
        science_projects,
        science_progress,
        census,
    };
    let serialized = serialize
        .then(|| serde_json::to_string(&game).expect("terminal Game must remain serializable"));
    Played { result, serialized }
}

#[derive(Clone, Debug)]
struct MapResult {
    control: [GameResult; 2],
    treatment: [GameResult; 2],
    exact: [bool; 2],
}

fn map_score(control_wins: usize, treatment_wins: usize) -> f64 {
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
    faith: f64,
    cities: usize,
    builders: usize,
    opportunities: u64,
    substitutions: u64,
    substitution_games: usize,
    contributions: u64,
    contribution_games: usize,
    national_history_museums: usize,
    royal_societies: usize,
    science_projects: usize,
    science_progress: f64,
    victories: BTreeMap<String, usize>,
}

impl ArmSummary {
    fn record(&mut self, result: &GameResult) {
        self.games += 1;
        self.wins += result.won as usize;
        self.turns += result.reported_turn as u64;
        self.score += result.score;
        self.faith += result.faith;
        self.cities += result.cities;
        self.builders += result.builders;
        self.opportunities += result.census.opportunities as u64;
        self.substitutions += result.census.substitutions as u64;
        self.substitution_games += (result.census.substitutions > 0) as usize;
        self.contributions += result.census.contributions as u64;
        self.contribution_games += (result.census.contributions > 0) as usize;
        self.national_history_museums += result.national_history_museums;
        self.royal_societies += result.royal_societies;
        self.science_projects += result.science_projects;
        self.science_progress += result.science_progress;
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

#[derive(Clone, Copy)]
struct GateInputs {
    substitution_games: usize,
    substitutions: u64,
    contributions: u64,
    contribution_games: usize,
    control_national_history: usize,
    treatment_national_history: usize,
    control_royal_society: usize,
    treatment_royal_society: usize,
    paired_score: f64,
    win_favorable: usize,
    win_adverse: usize,
    win_p: f64,
    terminal_score: f64,
    terminal_favorable: usize,
    terminal_adverse: usize,
    progress_favorable: usize,
    progress_adverse: usize,
    control_progress: f64,
    treatment_progress: f64,
    control_science_wins: usize,
    treatment_science_wins: usize,
}

fn mechanism_passes(gate: GateInputs) -> bool {
    gate.substitution_games >= 10
        && gate.substitutions >= 10
        && gate.contributions >= 10
        && gate.contribution_games >= 5
        && gate.treatment_royal_society > gate.control_royal_society
        && gate.treatment_national_history <= gate.control_national_history
}

fn screen_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate)
        && gate.paired_score >= 0.525
        && gate.win_favorable > gate.win_adverse
        && gate.terminal_score >= 0.50
        && gate.progress_favorable >= gate.progress_adverse
        && gate.treatment_progress + f64::EPSILON >= gate.control_progress
        && gate.treatment_science_wins >= gate.control_science_wins
}

fn holdout_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate)
        && gate.win_favorable > gate.win_adverse
        && gate.win_p < 0.05
        && gate.terminal_score >= 0.50
        && gate.terminal_favorable >= gate.terminal_adverse
        && gate.progress_favorable >= gate.progress_adverse
        && gate.treatment_progress + f64::EPSILON >= gate.control_progress
        && gate.treatment_science_wins >= gate.control_science_wins
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(why) = validate_args(&args) {
        eprintln!("{why}");
        std::process::exit(2);
    }
    let explicit_frozen_ai = has_exact_value(&args, "--ai", FROZEN_AI);
    let ai_name = text(&args, "--ai", FROZEN_AI);
    if args.iter().any(|arg| arg == "--ai") && !explicit_frozen_ai {
        eprintln!("this experiment is frozen for {FROZEN_AI}; got controller {ai_name:?}");
        std::process::exit(2);
    }
    let champion = frozen_champion();
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
    let null_replay = has_arg(&args, "--null");
    let maps = number(
        &args,
        "--maps",
        if null_replay {
            NULL_MAPS as i64
        } else {
            SCREEN_MAPS as i64
        },
    )
    .max(1) as usize;
    let players = number(&args, "--players", 8).max(2) as usize;
    let width = number(&args, "--width", 84).max(8) as i32;
    let height = number(&args, "--height", 54).max(8) as i32;
    let city_states = number(&args, "--city-states", 12).max(0) as usize;
    let turns = number(&args, "--turns", NOMINAL_TURNS as i64).max(1) as u32;
    let observe_through = number(&args, "--observe-through", OBSERVE_THROUGH as i64).max(1) as u32;
    if observe_through < turns {
        eprintln!("--observe-through must be at least --turns");
        std::process::exit(2);
    }
    let seed = number(&args, "--seed", SCREEN_SEED as i64).max(0) as u64;
    let jobs = match number(&args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    }
    .clamp(1, 6);
    let default_speed = civvis::game::default_speed();
    let speed = text(&args, "--speed", &default_speed);
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
    let victory_names = text(&args, "--victories", "science,culture,domination");
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

    println!("Royal Society Science evaluator");
    println!(
        "controller: {ai_name}; embedded champion generation {}; FNV-1a {:#018x}",
        champion.gen,
        fnv1a(EMBEDDED_CHAMPION.as_bytes())
    );
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
        let stored_dimensions = MapSize::from_dimensions(width, height)
            .map(|size| size.dimensions(map_topology))
            .unwrap_or((width, height));
        println!(
            "profile: diagnostic fixed cell: {players}p requested {width}x{height}, stored {}x{}, \
             {city_states} city-states, map {}, shape {}",
            stored_dimensions.0,
            stored_dimensions.1,
            map_script.id(),
            map_topology.id(),
        );
    }
    println!(
        "rules: {turns} nominal {speed} turns, observe through {observe_through}, poles {}, \
         civilizations {}, victories {}",
        map_poles.id(),
        if randomize_civs {
            "randomized"
        } else {
            "fixed"
        },
        VictoryConditions::NAMES
            .into_iter()
            .filter(|name| victories.is_enabled(name))
            .collect::<Vec<_>>()
            .join(","),
    );
    println!(
        "batch: {maps} independent maps x seats 0/final x control/treatment = {} games; seed {seed}; {jobs} jobs",
        maps * 4
    );
    println!(
        "treatment: {}",
        if null_replay {
            "NULL action-log replay (no substitution)"
        } else {
            "on an untreated champion Science turn, replace National History Museum with Royal Society"
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
                    map_topology,
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
                    turns,
                    profile.city_states,
                )
            };
            let treatment_mode = if null_replay {
                Mode::NullReplay
            } else {
                Mode::RoyalSociety
            };

            let control0 = play(
                options.clone(),
                0,
                Mode::Stock,
                null_replay,
                &champion.weights,
                observe_through,
            );
            let treatment0 = play(
                options.clone(),
                0,
                treatment_mode,
                null_replay,
                &champion.weights,
                observe_through,
            );
            let exact0 = !null_replay
                || (control0.result == treatment0.result
                    && control0.serialized == treatment0.serialized);
            let last = profile.players - 1;
            let control1 = play(
                options.clone(),
                last,
                Mode::Stock,
                null_replay,
                &champion.weights,
                observe_through,
            );
            let treatment1 = play(
                options,
                last,
                treatment_mode,
                null_replay,
                &champion.weights,
                observe_through,
            );
            let exact1 = !null_replay
                || (control1.result == treatment1.result
                    && control1.serialized == treatment1.serialized);

            MapResult {
                control: [control0.result, control1.result],
                treatment: [treatment0.result, treatment1.result],
                exact: [exact0, exact1],
            }
        },
        |completed, _| eprintln!("progress: {}/{} maps complete", completed + 1, maps),
    );

    let mut control = ArmSummary::default();
    let mut treatment = ArmSummary::default();
    let mut paired_scores = Vec::with_capacity(maps);
    let mut paired_terminal = Vec::with_capacity(maps);
    let mut paired_progress = Vec::with_capacity(maps);
    let mut win_favorable = 0usize;
    let mut win_adverse = 0usize;
    let mut terminal_favorable = 0usize;
    let mut terminal_adverse = 0usize;
    let mut progress_favorable = 0usize;
    let mut progress_adverse = 0usize;
    let mut helped_cells = 0usize;
    let mut hurt_cells = 0usize;
    let mut exact_mismatches = 0usize;

    for result in &results {
        let control_wins = result.control.iter().filter(|game| game.won).count();
        let treatment_wins = result.treatment.iter().filter(|game| game.won).count();
        paired_scores.push(map_score(control_wins, treatment_wins));
        match treatment_wins.cmp(&control_wins) {
            std::cmp::Ordering::Greater => win_favorable += 1,
            std::cmp::Ordering::Less => win_adverse += 1,
            std::cmp::Ordering::Equal => {}
        }

        let terminal = result
            .control
            .iter()
            .zip(&result.treatment)
            .map(|(old, new)| paired_share(old.score as f64, new.score as f64))
            .sum::<f64>()
            / 2.0;
        paired_terminal.push(terminal);
        if terminal > 0.5 + f64::EPSILON {
            terminal_favorable += 1;
        } else if terminal < 0.5 - f64::EPSILON {
            terminal_adverse += 1;
        }

        let progress = result
            .control
            .iter()
            .zip(&result.treatment)
            .map(|(old, new)| paired_share(old.science_progress, new.science_progress))
            .sum::<f64>()
            / 2.0;
        paired_progress.push(progress);
        if progress > 0.5 + f64::EPSILON {
            progress_favorable += 1;
        } else if progress < 0.5 - f64::EPSILON {
            progress_adverse += 1;
        }

        for (old, new) in result.control.iter().zip(&result.treatment) {
            control.record(old);
            treatment.record(new);
            match (old.won, new.won) {
                (false, true) => helped_cells += 1,
                (true, false) => hurt_cells += 1,
                _ => {}
            }
        }
        exact_mismatches += result.exact.iter().filter(|exact| !**exact).count();
    }

    let paired_score = paired_scores.iter().sum::<f64>() / maps as f64;
    let terminal_score = paired_terminal.iter().sum::<f64>() / maps as f64;
    let progress_share = paired_progress.iter().sum::<f64>() / maps as f64;
    let win_p = exact_two_sided(win_favorable, win_favorable + win_adverse);
    let terminal_p = exact_two_sided(terminal_favorable, terminal_favorable + terminal_adverse);
    let progress_p = exact_two_sided(progress_favorable, progress_favorable + progress_adverse);
    let control_science_wins = control.victories.get("science").copied().unwrap_or(0);
    let treatment_science_wins = treatment.victories.get("science").copied().unwrap_or(0);
    let control_progress = control.science_progress / control.games.max(1) as f64;
    let treatment_progress = treatment.science_progress / treatment.games.max(1) as f64;
    let gate = GateInputs {
        substitution_games: treatment.substitution_games,
        substitutions: treatment.substitutions,
        contributions: treatment.contributions,
        contribution_games: treatment.contribution_games,
        control_national_history: control.national_history_museums,
        treatment_national_history: treatment.national_history_museums,
        control_royal_society: control.royal_societies,
        treatment_royal_society: treatment.royal_societies,
        paired_score,
        win_favorable,
        win_adverse,
        win_p,
        terminal_score,
        terminal_favorable,
        terminal_adverse,
        progress_favorable,
        progress_adverse,
        control_progress,
        treatment_progress,
        control_science_wins,
        treatment_science_wins,
    };

    println!();
    println!(
        "arm        wins/games  turns  score  faith  cities builders  NHM  RS  projects progress"
    );
    for (name, arm) in [("control", &control), ("treatment", &treatment)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{name:<10} {:>3}/{:<3} {:>6.1} {:>6.1} {:>6.1} {:>7.2} {:>8.2} {:>4} {:>3} {:>9.2} {:>8.3}",
            arm.wins,
            arm.games,
            arm.turns as f64 / n,
            arm.score as f64 / n,
            arm.faith / n,
            arm.cities as f64 / n,
            arm.builders as f64 / n,
            arm.national_history_museums,
            arm.royal_societies,
            arm.science_projects as f64 / n,
            arm.science_progress / n,
        );
    }
    println!(
        "victory types: control {:?}; treatment {:?}",
        control.victories, treatment.victories
    );
    println!(
        "mechanism: control opportunities {}; treatment opportunities {}, substitutions {} in {}/{} seat-games, contributions {} in {} seat-games",
        control.opportunities,
        treatment.opportunities,
        treatment.substitutions,
        treatment.substitution_games,
        treatment.games,
        treatment.contributions,
        treatment.contribution_games,
    );
    println!(
        "matched seat cells: treatment helped {helped_cells}, hurt {hurt_cells}, unchanged {} (descriptive; map is the inference unit)",
        control.games - helped_cells - hurt_cells
    );
    println!("paired map win score: {:.1}%", 100.0 * paired_score);
    println!(
        "win direction: favorable {win_favorable}, neutral {}, adverse {win_adverse}; exact two-sided sign p={win_p:.4}",
        maps - win_favorable - win_adverse
    );
    println!(
        "paired terminal-score share: {:.1}%; favorable {terminal_favorable}, neutral {}, adverse {terminal_adverse}; exact p={terminal_p:.4}",
        100.0 * terminal_score,
        maps - terminal_favorable - terminal_adverse,
    );
    println!(
        "paired Science-progress share: {:.1}%; favorable {progress_favorable}, neutral {}, adverse {progress_adverse}; exact p={progress_p:.4}; arm means {:.3}/{:.3}",
        100.0 * progress_share,
        maps - progress_favorable - progress_adverse,
        control_progress,
        treatment_progress,
    );

    let exact_profile = deployment_mix
        && has_exact_flag(&args, "--deployment-mix")
        && explicit_frozen_ai
        && ai_name == FROZEN_AI
        && has_exact_number(&args, "--turns", NOMINAL_TURNS as i64)
        && turns == NOMINAL_TURNS
        && has_exact_number(&args, "--observe-through", OBSERVE_THROUGH as i64)
        && observe_through == OBSERVE_THROUGH
        && has_exact_value(&args, "--speed", "online")
        && speed == "online"
        && has_exact_value(&args, "--poles", "poles")
        && map_poles == MapPoles::Poles
        && has_exact_flag(&args, "--randomize-civs")
        && randomize_civs
        && has_exact_value(&args, "--victories", "science,culture,domination")
        && has_exact_number(&args, "--jobs", 6)
        && jobs == 6;

    if null_replay {
        if exact_mismatches > 0 {
            println!(
                "null sanity: BROKEN — {exact_mismatches}/{} matched seat replays differed",
                control.games
            );
            std::process::exit(3);
        }
        if exact_profile
            && has_exact_flag(&args, "--null")
            && has_exact_number(&args, "--maps", NULL_MAPS as i64)
            && maps == NULL_MAPS
            && has_exact_number(&args, "--seed", NULL_SEED as i64)
            && seed == NULL_SEED
        {
            println!(
                "null sanity: PASS — all {} champion seat replays reproduced the result and serialized Game exactly",
                control.games
            );
        } else {
            println!(
                "diagnostic null: all {} champion seat replays matched exactly; no preregistered null gate applies",
                control.games
            );
        }
        return;
    }

    if exact_profile
        && !has_arg(&args, "--null")
        && has_exact_number(&args, "--maps", SCREEN_MAPS as i64)
        && maps == SCREEN_MAPS
        && has_exact_number(&args, "--seed", SCREEN_SEED as i64)
        && seed == SCREEN_SEED
    {
        println!(
            "development gate: {}",
            if screen_passes(gate) {
                "PASS — run only the fixed disjoint holdout"
            } else {
                "STOP — at least one preregistered term failed; do not tune or retry"
            }
        );
    } else if exact_profile
        && !has_arg(&args, "--null")
        && has_exact_number(&args, "--maps", HOLDOUT_MAPS as i64)
        && maps == HOLDOUT_MAPS
        && has_exact_number(&args, "--seed", HOLDOUT_SEED as i64)
        && seed == HOLDOUT_SEED
    {
        println!(
            "holdout gate: {}",
            if holdout_passes(gate) {
                "PASS — a separate gameplay integration PR is permitted"
            } else {
                "RETAIN advanced_evolved — no gameplay integration"
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

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn produce(building: &str, city: u32) -> Action {
        Action::Produce {
            city,
            item: Item::Building {
                building: Name::new(building),
            },
        }
    }

    #[test]
    fn replacement_is_exactly_the_science_tier_three_choice() {
        assert_eq!(
            royal_society_replacement(&produce("national_history_museum", 17), true),
            Some(produce("royal_society", 17))
        );
        assert!(
            royal_society_replacement(&produce("national_history_museum", 17), false).is_none()
        );
        assert!(royal_society_replacement(&produce("war_department", 17), true).is_none());
    }

    #[test]
    fn supplied_values_fail_closed_and_numbers_do_not_default() {
        assert_eq!(option_value(&[], "--speed").unwrap(), None);
        assert!(option_value(&["--speed".to_string()], "--speed").is_err());
        assert!(option_value(&["--speed".to_string(), "--maps".to_string()], "--speed").is_err());
        assert_eq!(
            number_value(&["--turns".to_string(), "250".to_string()], "--turns").unwrap(),
            Some(250)
        );
        assert!(number_value(
            &["--turns".to_string(), "not-a-number".to_string()],
            "--turns"
        )
        .is_err());
        assert!(number_value(
            &strings(&["--turns", "250", "--turns", "not-a-number"]),
            "--turns"
        )
        .is_err());
        assert!(option_value(
            &strings(&["--speed", "online", "--speed", "--jobs", "1"]),
            "--speed"
        )
        .is_err());
        assert!(validate_args(&strings(&["--unknown"])).is_err());
        assert!(validate_args(&strings(&["positional"])).is_err());
        assert!(validate_args(&strings(&["--maps"])).is_err());
        assert!(validate_args(&strings(&[
            "--maps",
            "1",
            "--deployment-mix",
            "--randomize-civs"
        ]))
        .is_ok());
    }

    #[test]
    fn formal_controller_flag_requires_one_explicit_champion_value() {
        let args = ["--ai".to_string(), FROZEN_AI.to_string()];
        assert!(has_exact_value(&args, "--ai", FROZEN_AI));
        assert!(!has_exact_value(&["--ai".to_string()], "--ai", FROZEN_AI));
        assert!(!has_exact_value(
            &["--ai".to_string(), "advanced".to_string()],
            "--ai",
            FROZEN_AI
        ));
        assert!(!has_exact_value(
            &[
                "--ai".to_string(),
                FROZEN_AI.to_string(),
                "--ai".to_string(),
                FROZEN_AI.to_string(),
            ],
            "--ai",
            FROZEN_AI
        ));
    }

    #[test]
    fn formal_numeric_and_boolean_flags_are_exactly_bound() {
        let args = [
            "--turns".to_string(),
            "250".to_string(),
            "--deployment-mix".to_string(),
        ];
        assert!(has_exact_number(&args, "--turns", 250));
        assert!(!has_exact_number(&args, "--turns", 320));
        assert!(has_exact_flag(&args, "--deployment-mix"));
        assert!(!has_exact_number(
            &["--turns".to_string(), "0250".to_string()],
            "--turns",
            250
        ));
        assert!(!has_exact_number(
            &["--turns".to_string(), "nope".to_string()],
            "--turns",
            250
        ));
        assert!(!has_exact_number(
            &[
                "--jobs".to_string(),
                "6".to_string(),
                "--jobs".to_string(),
                "6".to_string(),
            ],
            "--jobs",
            6
        ));
        assert!(!has_exact_flag(
            &["--null".to_string(), "--null".to_string(),],
            "--null"
        ));
    }

    #[test]
    fn deployment_cycle_is_factorial_and_balances_frozen_batches() {
        assert_eq!(
            (0..NULL_MAPS)
                .map(deployment_profile)
                .map(|profile| (
                    profile.players,
                    profile.width,
                    profile.height,
                    profile.city_states,
                    profile.map_script,
                    profile.map_topology,
                ))
                .collect::<Vec<_>>(),
            vec![
                (4, 60, 38, 6, MapScript::LandOnly, MapTopology::Flat),
                (6, 74, 46, 9, MapScript::WaterWorld, MapTopology::Planet),
                (8, 84, 54, 12, MapScript::Continents, MapTopology::Flat),
                (
                    10,
                    96,
                    60,
                    15,
                    MapScript::TrueStartEarth,
                    MapTopology::Planet,
                ),
            ]
        );

        let cycle = (0..126).map(deployment_profile).collect::<Vec<_>>();
        for (index, profile) in cycle.iter().enumerate() {
            assert!(
                !cycle[..index].contains(profile),
                "deployment profile repeated before offset 126 at {index}: {profile:?}"
            );
        }
        assert_eq!(deployment_profile(126), deployment_profile(0));
        assert_eq!(
            deployment_counts(SCREEN_MAPS, |profile| profile.players),
            vec![(4, 5), (6, 5), (8, 4), (10, 4), (5, 4), (7, 4), (9, 4)]
        );
        assert_eq!(
            deployment_counts(HOLDOUT_MAPS, |profile| profile.map_topology),
            vec![(MapTopology::Flat, 60), (MapTopology::Planet, 60)]
        );
    }

    #[test]
    fn null_action_log_replay_reconstructs_the_stock_turn() {
        let mut stock = Game::new(2, 20, 14, 88_001, 20, 0);
        stock.set_fog_memory(false);
        let mut replay = stock.clone();
        let weights = frozen_champion().weights;
        let mut stock_ai = AdvancedAi::with_weights(weights);
        let mut replay_ai = stock_ai.clone();
        stock_ai.take_turn(&mut stock, 0);
        let mut census = ChoiceCensus::default();
        replay_stock_turn(&mut replay, &mut replay_ai, 0, false, &mut census).unwrap();
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
    fn frozen_controller_uses_the_committed_champion_weights() {
        let champion = frozen_champion();
        let game = Game::new(2, 20, 14, 88_003, 1, 0);
        let ais = AdvancedAi::fleet_weighted(&game, &champion.weights);
        assert_eq!(champion.gen, FROZEN_CHAMPION_GENERATION);
        assert_eq!(fnv1a(EMBEDDED_CHAMPION.as_bytes()), FROZEN_CHAMPION_FNV1A);
        assert_eq!(ais[0].weights(), &champion.weights);
        assert_ne!(
            ais[0].weights(),
            &Weights::default(),
            "the frozen champion must not silently collapse to stock weights"
        );
    }

    #[test]
    fn external_observation_preserves_the_policy_horizon() {
        let champion = frozen_champion();
        let options = GameOptions::new(2, 20, 14, 88_004, 1, 0);
        let played = play(options, 0, Mode::Stock, false, &champion.weights, 3);
        assert_eq!(played.result.reported_turn, 3);
    }

    #[test]
    fn science_progress_is_bounded_and_counts_only_required_projects() {
        let mut game = Game::new(2, 20, 14, 88_002, 20, 0);
        game.players[0]
            .science_projects
            .insert("launch_earth_satellite".to_string());
        game.players[0]
            .science_projects
            .insert("manhattan_project".to_string());
        game.players[0].exoplanet_distance = 75.0;
        assert_eq!(science_progress(&game, 0), (1, 1.0));
        game.players[0]
            .science_projects
            .insert("exoplanet_expedition".to_string());
        assert_eq!(science_progress(&game, 0), (2, 3.0));
    }

    #[test]
    fn map_score_keeps_two_seats_inside_one_independent_observation() {
        assert_eq!(map_score(0, 2), 1.0);
        assert_eq!(map_score(0, 1), 0.75);
        assert_eq!(map_score(1, 1), 0.5);
        assert_eq!(map_score(2, 0), 0.0);
    }

    #[test]
    fn exact_sign_test_matches_known_edges() {
        assert!((exact_two_sided(5, 5) - 0.0625).abs() < 1e-12);
        assert!((exact_two_sided(8, 8) - 0.0078125).abs() < 1e-12);
        assert_eq!(exact_two_sided(0, 0), 1.0);
    }

    #[test]
    fn gates_enforce_mechanism_direction_and_confirmation() {
        let passing = GateInputs {
            substitution_games: 10,
            substitutions: 10,
            contributions: 10,
            contribution_games: 5,
            control_national_history: 10,
            treatment_national_history: 0,
            control_royal_society: 0,
            treatment_royal_society: 10,
            paired_score: 0.525,
            win_favorable: 8,
            win_adverse: 2,
            win_p: 0.05,
            terminal_score: 0.50,
            terminal_favorable: 6,
            terminal_adverse: 6,
            progress_favorable: 5,
            progress_adverse: 5,
            control_progress: 3.0,
            treatment_progress: 3.0,
            control_science_wins: 2,
            treatment_science_wins: 2,
        };
        assert!(screen_passes(passing));
        assert!(!holdout_passes(passing));
        assert!(holdout_passes(GateInputs {
            win_p: 0.049,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            contribution_games: 4,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            progress_adverse: 6,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_science_wins: 1,
            ..passing
        }));
    }
}
