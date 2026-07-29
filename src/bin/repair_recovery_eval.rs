//! Matched evaluation of deliberate Builder repair routing.
//!
//! The treatment spends only legal focal-Builder movement. Before the stock
//! adaptive controller acts, at most one Builder is routed toward an owned
//! remote pillaged improvement, prioritizing strategic and luxury tiles.
//! Two focal seats are paired within every map; the map is the inference unit.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::evolve::Champion;
use civvis::game::{Action, Game, GameOptions, Item, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use civvis::Pos;
use std::collections::BTreeMap;

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 9_995_000;
const SCREEN_MAPS: usize = 18;
const SCREEN_SEED: u64 = 9_996_000;
const HOLDOUT_MAPS: usize = 63;
const HOLDOUT_SEED: u64 = 9_997_000;
const NOMINAL_TURNS: u32 = 250;
const OBSERVE_THROUGH: u32 = 320;
const FROZEN_AI: &str = "advanced_evolved";
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
const PROFILE_OVERRIDE_FLAGS: [&str; 7] = [
    "--players",
    "--width",
    "--height",
    "--city-states",
    "--map",
    "--shape",
    "--shapes",
];

fn frozen_champion() -> Champion {
    serde_json::from_str(EMBEDDED_CHAMPION)
        .expect("the committed advanced_evolved champion must be valid JSON")
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
struct RepairAssignment {
    unit: u32,
    target: Pos,
    resource_tier: u8,
    distance: i32,
}

fn owned_repair_targets(game: &Game, pid: usize) -> Vec<Pos> {
    game.map
        .tiles
        .iter()
        .filter_map(|(position, tile)| {
            (tile.pillaged
                && tile.improvement.is_some()
                && tile
                    .owner_city
                    .and_then(|city| game.cities.get(&city))
                    .is_some_and(|city| city.owner == pid))
            .then_some(*position)
        })
        .collect()
}

fn resource_tier(game: &Game, position: Pos) -> u8 {
    let Some(resource) = game.map.tiles[&position].resource else {
        return 3;
    };
    match game
        .rules
        .resources
        .get_interned(resource)
        .map(|spec| spec.class.as_str())
    {
        Some("strategic") => 0,
        Some("luxury") => 1,
        Some("bonus") => 2,
        _ => 3,
    }
}

fn project_has_builder_priority(game: &Game, pid: usize) -> bool {
    let cities = game.player_city_ids(pid);
    let royal_society = cities.iter().any(|city| {
        game.cities[city]
            .buildings
            .iter()
            .any(|building| building == "royal_society")
    });
    royal_society
        && cities.iter().any(|city| {
            matches!(
                game.cities[city].queue.first(),
                Some(Item::Project { project }) if !project.starts_with("repair_")
            )
        })
}

fn choose_assignment(game: &Game, pid: usize) -> Option<RepairAssignment> {
    // A Builder already occupying a damaged tile will repair it under stock
    // control. Do not redirect a second unit toward work that is already
    // covered, and do not count that stock completion as treatment exposure.
    let all_targets = owned_repair_targets(game, pid);
    let targets = all_targets
        .iter()
        .copied()
        .filter(|target| {
            !game
                .units
                .values()
                .any(|unit| unit.owner == pid && unit.kind == "builder" && unit.pos == *target)
        })
        .collect::<Vec<_>>();
    game.units
        .values()
        .filter(|unit| {
            unit.owner == pid
                && unit.kind == "builder"
                && unit.moves_left > 0.0
                && !all_targets.contains(&unit.pos)
        })
        .flat_map(|unit| {
            targets.iter().map(move |target| RepairAssignment {
                unit: unit.id,
                target: *target,
                resource_tier: resource_tier(game, *target),
                distance: game.wdist(unit.pos, *target),
            })
        })
        .filter(|assignment| assignment.distance > 0)
        // The treatment has no privileged pathfinder. Skip a pair unless it
        // has at least one legal, strictly closer first step; the stock policy
        // keeps the turn if terrain or occupancy blocks every such move.
        .filter(|assignment| {
            let position = game.units[&assignment.unit].pos;
            game.nbrs(position).into_iter().any(|next| {
                game.can_move(assignment.unit, next)
                    && game.wdist(next, assignment.target) < assignment.distance
            })
        })
        .min_by_key(|assignment| {
            (
                assignment.resource_tier,
                assignment.distance,
                assignment.target,
                assignment.unit,
            )
        })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TreatmentCensus {
    eligible_turns: u32,
    project_deferrals: u32,
    routed_turns: u32,
    move_steps: u32,
    repairs: u32,
    blocked_attempts: u32,
}

impl TreatmentCensus {
    fn add(&mut self, other: TreatmentCensus) {
        self.eligible_turns += other.eligible_turns;
        self.project_deferrals += other.project_deferrals;
        self.routed_turns += other.routed_turns;
        self.move_steps += other.move_steps;
        self.repairs += other.repairs;
        self.blocked_attempts += other.blocked_attempts;
    }
}

/// Spend only one Builder's legal movement on a remote repair target.
fn repair_crew_step(game: &mut Game, pid: usize) -> TreatmentCensus {
    let mut census = TreatmentCensus::default();
    if project_has_builder_priority(game, pid) {
        census.project_deferrals = 1;
        return census;
    }
    let Some(assignment) = choose_assignment(game, pid) else {
        return census;
    };
    census.eligible_turns = 1;

    loop {
        let Some(unit) = game.units.get(&assignment.unit) else {
            census.blocked_attempts = 1;
            break;
        };
        let current = unit.pos;
        if current == assignment.target {
            match game.apply(
                pid,
                &Action::RepairImprovement {
                    unit: assignment.unit,
                },
            ) {
                Ok(()) => {
                    census.routed_turns = 1;
                    census.repairs = 1;
                }
                Err(_) => census.blocked_attempts = 1,
            }
            break;
        }
        if unit.moves_left <= 0.0 {
            break;
        }
        let old_distance = game.wdist(current, assignment.target);
        let next = game
            .nbrs(current)
            .into_iter()
            .filter(|position| game.can_move(assignment.unit, *position))
            .filter(|position| game.wdist(*position, assignment.target) < old_distance)
            .min_by_key(|position| (game.wdist(*position, assignment.target), *position));
        let Some(next) = next else {
            census.blocked_attempts = 1;
            break;
        };
        if game
            .apply(
                pid,
                &Action::Move {
                    unit: assignment.unit,
                    to: next,
                },
            )
            .is_err()
        {
            census.blocked_attempts = 1;
            break;
        }
        census.routed_turns = 1;
        census.move_steps += 1;
    }
    census
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct DamageYields {
    food: f64,
    production: f64,
    gold: f64,
    science: f64,
    culture: f64,
    faith: f64,
    housing: f64,
}

impl DamageYields {
    fn add(&mut self, other: DamageYields) {
        self.food += other.food;
        self.production += other.production;
        self.gold += other.gold;
        self.science += other.science;
        self.culture += other.culture;
        self.faith += other.faith;
        self.housing += other.housing;
    }
}

fn terminal_damage(game: &Game, pid: usize) -> (usize, usize, DamageYields) {
    let targets = owned_repair_targets(game, pid);
    let mut resources = 0;
    let mut yields = DamageYields::default();
    for position in &targets {
        let tile = &game.map.tiles[position];
        resources += tile.resource.is_some() as usize;
        let Some(improvement) = tile.improvement else {
            continue;
        };
        let Some(spec) = game.rules.improvements.get_interned(improvement) else {
            continue;
        };
        yields.food += spec.yields.food;
        yields.production += spec.yields.production;
        yields.gold += spec.yields.gold;
        yields.science += spec.yields.science;
        yields.culture += spec.yields.culture;
        yields.faith += spec.yields.faith;
        yields.housing += spec.housing;
    }
    (targets.len(), resources, yields)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Control,
    Null,
    Treatment,
}

#[derive(Clone, Debug, PartialEq)]
struct GameResult {
    won: bool,
    victory: Option<String>,
    reported_turn: u32,
    policy_max_turns: u32,
    score: i64,
    cities: usize,
    repair_debt: usize,
    resource_debt: usize,
    builders: usize,
    builder_charges: i64,
    damage_yields: DamageYields,
    census: TreatmentCensus,
}

fn play(
    options: GameOptions,
    focal: usize,
    mode: Mode,
    observe_through: u32,
    weights: &Weights,
) -> GameResult {
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
    let mut ais = AdvancedAi::fleet_weighted(&game, weights);
    let mut census = TreatmentCensus::default();

    while game.winner.is_none() && game.turn <= observe_through {
        assert_eq!(
            game.max_turns, policy_max_turns,
            "external continuation changed the policy-visible horizon"
        );
        let pid = game.current;
        if mode == Mode::Treatment && pid == focal {
            census.add(repair_crew_step(&mut game, pid));
        }
        if game.winner.is_none() && game.current == pid {
            ais[pid].take_turn(&mut game, pid);
        }
    }
    assert_eq!(
        game.max_turns, policy_max_turns,
        "external continuation changed the policy-visible horizon"
    );

    let cities = game.player_city_ids(focal).len();
    let (repair_debt, resource_debt, damage_yields) = terminal_damage(&game, focal);
    let builders = game
        .units
        .values()
        .filter(|unit| unit.owner == focal && unit.kind == "builder")
        .count();
    let builder_charges = game
        .units
        .values()
        .filter(|unit| unit.owner == focal && unit.kind == "builder")
        .map(|unit| unit.charges.max(0) as i64)
        .sum();

    GameResult {
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
        cities,
        repair_debt,
        resource_debt,
        builders,
        builder_charges,
        damage_yields,
        census,
    }
}

#[derive(Clone, Debug)]
struct MapResult {
    profile: DeploymentProfile,
    control: [GameResult; 2],
    comparison: [GameResult; 2],
}

fn map_win_score(control_wins: usize, treatment_wins: usize) -> f64 {
    0.5 + (treatment_wins as f64 - control_wins as f64) / 4.0
}

fn terminal_share(control: i64, treatment: i64) -> f64 {
    let control = control.max(0) as f64;
    let treatment = treatment.max(0) as f64;
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
    cities: usize,
    repair_debt: usize,
    resource_debt: usize,
    builders: usize,
    builder_charges: i64,
    damage_yields: DamageYields,
    eligible_turns: u64,
    project_deferrals: u64,
    routed_turns: u64,
    move_steps: u64,
    repairs: u64,
    blocked_attempts: u64,
    fired_games: usize,
    routed_games: usize,
    victories: BTreeMap<String, usize>,
}

impl ArmSummary {
    fn record(&mut self, result: &GameResult) {
        self.games += 1;
        self.wins += result.won as usize;
        self.turns += result.reported_turn as u64;
        self.score += result.score;
        self.cities += result.cities;
        self.repair_debt += result.repair_debt;
        self.resource_debt += result.resource_debt;
        self.builders += result.builders;
        self.builder_charges += result.builder_charges;
        self.damage_yields.add(result.damage_yields);
        self.eligible_turns += result.census.eligible_turns as u64;
        self.project_deferrals += result.census.project_deferrals as u64;
        self.routed_turns += result.census.routed_turns as u64;
        self.move_steps += result.census.move_steps as u64;
        self.repairs += result.census.repairs as u64;
        self.blocked_attempts += result.census.blocked_attempts as u64;
        self.fired_games += (result.census.repairs > 0) as usize;
        self.routed_games += (result.census.routed_turns > 0) as usize;
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
struct StratumSummary {
    maps: usize,
    control: ArmSummary,
    comparison: ArmSummary,
    score_delta: f64,
    favorable: usize,
    adverse: usize,
    win_score: f64,
    terminal_share: f64,
}

impl StratumSummary {
    fn record(&mut self, result: &MapResult) {
        self.maps += 1;
        let control_wins = result.control.iter().filter(|game| game.won).count();
        let comparison_wins = result.comparison.iter().filter(|game| game.won).count();
        self.win_score += map_win_score(control_wins, comparison_wins);
        self.terminal_share += result
            .control
            .iter()
            .zip(&result.comparison)
            .map(|(old, new)| terminal_share(old.score, new.score))
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
        if delta > 1e-9 {
            self.favorable += 1;
        } else if delta < -1e-9 {
            self.adverse += 1;
        }
        for (old, new) in result.control.iter().zip(&result.comparison) {
            self.control.record(old);
            self.comparison.record(new);
        }
    }
}

fn summarize_stratum<'a>(results: impl Iterator<Item = &'a MapResult>) -> StratumSummary {
    let mut summary = StratumSummary::default();
    for result in results {
        summary.record(result);
    }
    summary
}

fn print_stratum(label: &str, summary: &StratumSummary) {
    let maps = summary.maps.max(1);
    println!(
        "  {label:<25} {:>3} maps; repaired {}/{} games, {} repairs; debt {}->{}, resource-bearing {}->{}; score delta {:+.2} (F/N/A {}/{}/{}); wins {}->{}; map win {:.1}%, score share {:.2}%",
        summary.maps,
        summary.comparison.fired_games,
        summary.comparison.games,
        summary.comparison.repairs,
        summary.control.repair_debt,
        summary.comparison.repair_debt,
        summary.control.resource_debt,
        summary.comparison.resource_debt,
        summary.score_delta / maps as f64,
        summary.favorable,
        maps - summary.favorable - summary.adverse,
        summary.adverse,
        summary.control.wins,
        summary.comparison.wins,
        100.0 * summary.win_score / maps as f64,
        100.0 * summary.terminal_share / maps as f64,
    );
}

fn axis_values<T: Copy + Eq>(
    results: &[MapResult],
    select: impl Fn(DeploymentProfile) -> T,
) -> Vec<T> {
    let mut values = Vec::new();
    for result in results {
        let value = select(result.profile);
        if !values.contains(&value) {
            values.push(value);
        }
    }
    values
}

#[derive(Clone, Copy)]
struct GateInputs {
    games: usize,
    fired_games: usize,
    repairs: u64,
    control_debt: usize,
    treatment_debt: usize,
    control_resource_debt: usize,
    treatment_resource_debt: usize,
    score_delta: f64,
    favorable: usize,
    adverse: usize,
    sign_p: f64,
    control_wins: usize,
    treatment_wins: usize,
    paired_win_score: f64,
    terminal_score_share: f64,
    control_cities: usize,
    treatment_cities: usize,
}

fn debt_reduction_passes(gate: GateInputs) -> bool {
    gate.treatment_debt.saturating_mul(100) <= gate.control_debt.saturating_mul(85)
        && gate.treatment_resource_debt.saturating_mul(100)
            <= gate.control_resource_debt.saturating_mul(85)
}

fn screen_passes(gate: GateInputs) -> bool {
    gate.fired_games >= 18
        && gate.repairs >= 36
        && debt_reduction_passes(gate)
        && gate.score_delta > 0.0
        && gate.favorable > gate.adverse
        && gate.sign_p <= 0.20
        && gate.treatment_wins + 1 >= gate.control_wins
        && gate.paired_win_score >= 0.48
        && gate.terminal_score_share >= 0.495
        && gate.treatment_cities.saturating_mul(100) >= gate.control_cities.saturating_mul(98)
}

fn holdout_passes(gate: GateInputs) -> bool {
    gate.fired_games.saturating_mul(2) >= gate.games
        && gate.repairs >= gate.games as u64
        && debt_reduction_passes(gate)
        && gate.score_delta > 0.0
        && gate.favorable > gate.adverse
        && gate.sign_p < 0.05
        && gate.treatment_wins >= gate.control_wins
        && gate.paired_win_score >= 0.50
        && gate.terminal_score_share >= 0.50
        && gate.treatment_cities >= gate.control_cities
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let null = has_arg(&args, "--null");
    let deployment_mix = has_arg(&args, "--deployment-mix");
    let ai_name = text_arg(&args, "--ai", FROZEN_AI);
    if ai_name != FROZEN_AI {
        eprintln!("this experiment is frozen for {FROZEN_AI}; got controller {ai_name:?}");
        std::process::exit(2);
    }
    let champion = frozen_champion();
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
    let expected_victories = VictoryConditions {
        science: true,
        culture: true,
        religious: false,
        diplomatic: false,
        domination: true,
        score: false,
    };
    if victories != expected_victories {
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

    println!("Deliberate Builder repair-routing evaluator");
    println!(
        "controller: {ai_name}; embedded champion generation {}",
        champion.gen
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
        let sizes = DEPLOYMENT_PLAYERS
            .iter()
            .map(|players| {
                let size = MapSize::for_players(*players);
                let planet = size.dimensions(MapTopology::Planet);
                format!(
                    "{players}p={}x{}/{}x{}+{}cs",
                    size.width, size.height, planet.0, planet.1, size.default_city_states
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "profile: deployment mix; players {player_batch}; scripts {script_batch}; topologies {topology_batch}"
        );
        println!("derived Flat/Planet size rows: {sizes}");
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
            "NULL identical evaluator loop with repair routing disabled"
        } else {
            "one legal resource-first remote repair crew before stock AdvancedAi"
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
            let control = [
                play(
                    options.clone(),
                    seats[0],
                    Mode::Control,
                    observe_through,
                    &champion.weights,
                ),
                play(
                    options.clone(),
                    seats[1],
                    Mode::Control,
                    observe_through,
                    &champion.weights,
                ),
            ];
            let comparison_mode = if null { Mode::Null } else { Mode::Treatment };
            let comparison = [
                play(
                    options.clone(),
                    seats[0],
                    comparison_mode,
                    observe_through,
                    &champion.weights,
                ),
                play(
                    options,
                    seats[1],
                    comparison_mode,
                    observe_through,
                    &champion.weights,
                ),
            ];
            MapResult {
                profile,
                control,
                comparison,
            }
        },
        |completed, _| eprintln!("progress: {}/{} maps complete", completed + 1, maps),
    );

    let mut summary = StratumSummary::default();
    let mut exact_mismatches = 0usize;
    let mut helped_cells = 0usize;
    let mut hurt_cells = 0usize;
    let mut win_favorable = 0usize;
    let mut win_adverse = 0usize;
    for result in &results {
        summary.record(result);
        let control_wins = result.control.iter().filter(|game| game.won).count();
        let comparison_wins = result.comparison.iter().filter(|game| game.won).count();
        match comparison_wins.cmp(&control_wins) {
            std::cmp::Ordering::Greater => win_favorable += 1,
            std::cmp::Ordering::Less => win_adverse += 1,
            std::cmp::Ordering::Equal => {}
        }
        for (old, new) in result.control.iter().zip(&result.comparison) {
            exact_mismatches += (old != new) as usize;
            match (old.won, new.won) {
                (false, true) => helped_cells += 1,
                (true, false) => hurt_cells += 1,
                _ => {}
            }
        }
    }

    let map_count = summary.maps.max(1);
    let score_delta = summary.score_delta / map_count as f64;
    let sign_p = exact_two_sided(summary.favorable, summary.favorable + summary.adverse);
    let win_p = exact_two_sided(win_favorable, win_favorable + win_adverse);
    let paired_win_score = summary.win_score / map_count as f64;
    let terminal_score_share = summary.terminal_share / map_count as f64;
    let control = &summary.control;
    let comparison = &summary.comparison;
    let gate = GateInputs {
        games: comparison.games,
        fired_games: comparison.fired_games,
        repairs: comparison.repairs,
        control_debt: control.repair_debt,
        treatment_debt: comparison.repair_debt,
        control_resource_debt: control.resource_debt,
        treatment_resource_debt: comparison.resource_debt,
        score_delta,
        favorable: summary.favorable,
        adverse: summary.adverse,
        sign_p,
        control_wins: control.wins,
        treatment_wins: comparison.wins,
        paired_win_score,
        terminal_score_share,
        control_cities: control.cities,
        treatment_cities: comparison.cities,
    };

    println!();
    println!("arm         wins    turns    score  cities  debt  resource  builders  charges");
    for (name, arm) in [("control", control), ("comparison", comparison)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{name:<11} {:>3}/{:<3} {:>7.1} {:>8.2} {:>7.2} {:>5} {:>9} {:>9} {:>8}",
            arm.wins,
            arm.games,
            arm.turns as f64 / n,
            arm.score as f64 / n,
            arm.cities as f64 / n,
            arm.repair_debt,
            arm.resource_debt,
            arm.builders,
            arm.builder_charges,
        );
    }
    println!(
        "victory types: control {:?}; comparison {:?}",
        control.victories, comparison.victories
    );
    println!(
        "treatment mechanism: repaired in {}/{} games, routed in {}/{}; {} repairs, {} move steps, {} eligible turns, {} project deferrals, {} blocked attempts",
        comparison.fired_games,
        comparison.games,
        comparison.routed_games,
        comparison.games,
        comparison.repairs,
        comparison.move_steps,
        comparison.eligible_turns,
        comparison.project_deferrals,
        comparison.blocked_attempts,
    );
    println!(
        "terminal debt: improvements {}->{}, resource-bearing improvements {}->{}; damaged base yields P {:.0}->{:.0}, F {:.0}->{:.0}, G {:.0}->{:.0}, housing {:.1}->{:.1}",
        control.repair_debt,
        comparison.repair_debt,
        control.resource_debt,
        comparison.resource_debt,
        control.damage_yields.production,
        comparison.damage_yields.production,
        control.damage_yields.food,
        comparison.damage_yields.food,
        control.damage_yields.gold,
        comparison.damage_yields.gold,
        control.damage_yields.housing,
        comparison.damage_yields.housing,
    );
    println!(
        "matched seat cells: helped {helped_cells}, hurt {hurt_cells}, unchanged {} (descriptive; map is the inference unit)",
        control.games - helped_cells - hurt_cells
    );
    println!(
        "paired map win score: {:.1}%; favorable {win_favorable}, neutral {}, adverse {win_adverse}; exact p={win_p:.4}",
        100.0 * paired_win_score,
        maps - win_favorable - win_adverse,
    );
    println!(
        "primary score delta: {score_delta:+.2}/map; favorable {}, neutral {}, adverse {}; exact p={sign_p:.4}",
        summary.favorable,
        maps - summary.favorable - summary.adverse,
        summary.adverse,
    );
    println!(
        "paired terminal-score share: {:.2}%",
        100.0 * terminal_score_share
    );

    println!("deployment-axis summaries (descriptive only; the decision gate is pooled):");
    for players in axis_values(&results, |profile| profile.players) {
        print_stratum(
            &format!("players={players}"),
            &summarize_stratum(
                results
                    .iter()
                    .filter(|result| result.profile.players == players),
            ),
        );
    }
    for script in axis_values(&results, |profile| profile.map_script) {
        print_stratum(
            &format!("map={}", script.id()),
            &summarize_stratum(
                results
                    .iter()
                    .filter(|result| result.profile.map_script == script),
            ),
        );
    }
    for topology in axis_values(&results, |profile| profile.map_topology) {
        print_stratum(
            &format!("shape={}", topology.id()),
            &summarize_stratum(
                results
                    .iter()
                    .filter(|result| result.profile.map_topology == topology),
            ),
        );
    }

    let exact_profile = deployment_mix
        && [
            "--ai",
            "--maps",
            "--seed",
            "--turns",
            "--observe-through",
            "--speed",
            "--poles",
            "--victories",
            "--jobs",
        ]
        .iter()
        .all(|flag| has_arg(&args, flag))
        && ai_name == FROZEN_AI
        && nominal_turns == NOMINAL_TURNS
        && observe_through == OBSERVE_THROUGH
        && speed == "online"
        && map_poles == MapPoles::Poles
        && randomize_civs;

    if null {
        if exact_mismatches > 0 {
            println!(
                "null sanity: BROKEN — {exact_mismatches}/{} matched focal cells differed",
                control.games
            );
            std::process::exit(3);
        }
        if exact_profile && maps == NULL_MAPS && seed == NULL_SEED {
            println!(
                "frozen null gate: PASS — all {} matched focal cells reproduced exactly",
                control.games
            );
        } else {
            println!(
                "diagnostic null sanity: PASS — all {} matched focal cells reproduced exactly",
                control.games
            );
        }
        return;
    }

    if exact_profile && maps == SCREEN_MAPS && seed == SCREEN_SEED {
        println!(
            "development gate: {}",
            if screen_passes(gate) {
                "PASS — run only the fixed disjoint holdout"
            } else {
                "STOP — retain AdvancedAi; do not tune, retry, or inspect the holdout"
            }
        );
    } else if exact_profile && maps == HOLDOUT_MAPS && seed == HOLDOUT_SEED {
        println!(
            "holdout gate: {}",
            if holdout_passes(gate) {
                "PASS — a separate gameplay-integration PR is permitted"
            } else {
                "RETAIN AdvancedAi — no gameplay integration"
            }
        );
    } else {
        println!("decision: DIAGNOSTIC ONLY — not a preregistered treatment batch");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repair_fixture(seed: u64) -> (Game, u32, u32, Vec<Pos>) {
        let mut game = Game::new(2, 20, 14, seed, 20, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        game.players[0].gold = 10_000.0;
        game.apply(
            0,
            &Action::Buy {
                city,
                unit: civvis::name!("builder"),
                formation: 0,
                currency: "gold".to_string(),
            },
        )
        .unwrap();
        let builder = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "builder")
            .unwrap();
        game.units.get_mut(&builder).unwrap().moves_left = 2.0;
        let center = game.cities[&city].pos;
        let mut owned = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .filter(|position| *position != center)
            .collect::<Vec<_>>();
        owned.sort_by_key(|position| (game.wdist(center, *position), *position));
        for position in &owned {
            let tile = game.map.tiles.get_mut(position).unwrap();
            tile.terrain = civvis::name!("plains");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.pillaged = false;
            tile.district = None;
            tile.district_foundation = None;
            tile.wonder = None;
        }
        (game, city, builder, owned)
    }

    fn damage(game: &mut Game, position: Pos, improvement: &str, resource: Option<&str>) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.improvement = Some(civvis::name::Name::new(improvement));
        tile.resource = resource.map(civvis::name::Name::new);
        tile.pillaged = true;
    }

    #[test]
    fn deployment_cycle_is_factorial_and_balances_frozen_batches() {
        let null_profiles = (0..NULL_MAPS).map(deployment_profile).collect::<Vec<_>>();
        assert_eq!(
            null_profiles
                .iter()
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
                "profile repeated before offset 126 at {index}: {profile:?}"
            );
        }
        assert_eq!(deployment_profile(126), deployment_profile(0));
        assert_eq!(
            deployment_counts(SCREEN_MAPS, |profile| profile.players),
            vec![(4, 3), (6, 3), (8, 3), (10, 3), (5, 2), (7, 2), (9, 2)]
        );
        assert_eq!(
            deployment_counts(SCREEN_MAPS, |profile| profile.map_script),
            DEPLOYMENT_SCRIPTS
                .iter()
                .copied()
                .map(|script| (script, 2))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            deployment_counts(SCREEN_MAPS, |profile| profile.map_topology),
            vec![(MapTopology::Flat, 9), (MapTopology::Planet, 9)]
        );
        assert_eq!(
            deployment_counts(HOLDOUT_MAPS, |profile| profile.players),
            DEPLOYMENT_PLAYERS
                .iter()
                .copied()
                .map(|players| (players, 9))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            deployment_counts(HOLDOUT_MAPS, |profile| profile.map_script),
            DEPLOYMENT_SCRIPTS
                .iter()
                .copied()
                .map(|script| (script, 7))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            deployment_counts(HOLDOUT_MAPS, |profile| profile.map_topology),
            vec![(MapTopology::Flat, 32), (MapTopology::Planet, 31)]
        );
    }

    #[test]
    fn resource_priority_precedes_distance_and_ties_are_stable() {
        let (mut game, _, builder, owned) = repair_fixture(79_500);
        let center = game.units[&builder].pos;
        let near = owned[0];
        let far = *owned
            .iter()
            .max_by_key(|position| (game.wdist(center, **position), **position))
            .unwrap();
        damage(&mut game, near, "farm", None);
        damage(&mut game, far, "mine", Some("iron"));
        let assignment = choose_assignment(&game, 0).unwrap();
        assert_eq!(assignment.resource_tier, 0);
        assert_eq!(assignment.target, far);
        assert_eq!(assignment.unit, builder);
    }

    #[test]
    fn equal_tier_and_distance_choose_the_lowest_target_position() {
        let (mut game, _, builder, owned) = repair_fixture(79_506);
        let center = game.units[&builder].pos;
        let mut candidates = owned
            .iter()
            .copied()
            .filter(|position| game.wdist(center, *position) == 1)
            .filter(|position| game.can_move(builder, *position))
            .collect::<Vec<_>>();
        candidates.sort();
        assert!(candidates.len() >= 2);
        damage(&mut game, candidates[0], "farm", None);
        damage(&mut game, candidates[1], "farm", None);
        assert_eq!(choose_assignment(&game, 0).unwrap().target, candidates[0]);
    }

    #[test]
    fn stock_covered_repair_is_not_a_treatment_assignment() {
        let (mut game, _, builder, owned) = repair_fixture(79_507);
        let target = owned[0];
        damage(&mut game, target, "farm", None);
        game.units.get_mut(&builder).unwrap().pos = target;
        assert!(choose_assignment(&game, 0).is_none());
        let before = game.log.len();
        assert_eq!(repair_crew_step(&mut game, 0), TreatmentCensus::default());
        assert!(game.map.tiles[&target].pillaged);
        assert_eq!(game.log.len(), before);
    }

    #[test]
    fn remote_repair_uses_legal_movement_and_no_charge() {
        let (mut game, _, builder, owned) = repair_fixture(79_501);
        let target = owned[0];
        damage(&mut game, target, "farm", None);
        let charges = game.units[&builder].charges;
        let census = repair_crew_step(&mut game, 0);
        assert_eq!(census.repairs, 1);
        assert_eq!(census.routed_turns, 1);
        assert!(census.move_steps >= 1);
        assert_eq!(game.units[&builder].pos, target);
        assert!(!game.map.tiles[&target].pillaged);
        assert_eq!(game.units[&builder].charges, charges);
    }

    #[test]
    fn no_remote_target_leaves_the_game_untouched() {
        let (mut game, _, builder, _) = repair_fixture(79_502);
        let position = game.units[&builder].pos;
        let moves = game.units[&builder].moves_left;
        let log = game.log.len();
        assert_eq!(repair_crew_step(&mut game, 0), TreatmentCensus::default());
        assert_eq!(game.units[&builder].pos, position);
        assert_eq!(game.units[&builder].moves_left, moves);
        assert_eq!(game.log.len(), log);
    }

    #[test]
    fn royal_society_project_defers_the_repair_crew() {
        let (mut game, city, builder, owned) = repair_fixture(79_503);
        let target = owned[0];
        damage(&mut game, target, "farm", None);
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(civvis::name!("royal_society"));
        game.cities
            .get_mut(&city)
            .unwrap()
            .queue
            .push(Item::Project {
                project: civvis::name!("campus_research_grants"),
            });
        let position = game.units[&builder].pos;
        let census = repair_crew_step(&mut game, 0);
        assert_eq!(census.project_deferrals, 1);
        assert_eq!(census.repairs, 0);
        assert_eq!(game.units[&builder].pos, position);
        assert!(game.map.tiles[&target].pillaged);
    }

    #[test]
    fn one_call_completes_at_most_one_repair() {
        let (mut game, _, _, owned) = repair_fixture(79_504);
        damage(&mut game, owned[0], "farm", None);
        damage(&mut game, owned[1], "mine", None);
        let before = owned
            .iter()
            .filter(|position| game.map.tiles[position].pillaged)
            .count();
        let census = repair_crew_step(&mut game, 0);
        let after = owned
            .iter()
            .filter(|position| game.map.tiles[position].pillaged)
            .count();
        assert!(census.repairs <= 1);
        assert!(before - after <= 1);
    }

    #[test]
    fn external_observation_preserves_the_policy_horizon() {
        let options = GameOptions::new(2, 20, 14, 79_505, 1, 0);
        let result = play(options, 0, Mode::Control, 3, &Weights::default());
        assert_eq!(result.policy_max_turns, 1);
        assert_eq!(result.reported_turn, 3);
    }

    #[test]
    fn frozen_controller_uses_the_committed_champion_weights() {
        let champion = frozen_champion();
        let game = Game::new(2, 20, 14, 79_508, 1, 0);
        let ais = AdvancedAi::fleet_weighted(&game, &champion.weights);
        assert!(champion.gen > 0);
        assert_eq!(ais[0].weights(), &champion.weights);
        assert_ne!(
            ais[0].weights(),
            &Weights::default(),
            "the frozen champion must not silently collapse to stock weights"
        );
    }

    #[test]
    fn frozen_gates_require_mechanism_debt_and_harm_guards() {
        let screen = GateInputs {
            games: 36,
            fired_games: 18,
            repairs: 36,
            control_debt: 100,
            treatment_debt: 85,
            control_resource_debt: 40,
            treatment_resource_debt: 34,
            score_delta: 2.0,
            favorable: 13,
            adverse: 5,
            sign_p: 0.096,
            control_wins: 10,
            treatment_wins: 9,
            paired_win_score: 0.49,
            terminal_score_share: 0.496,
            control_cities: 100,
            treatment_cities: 98,
        };
        assert!(screen_passes(screen));
        assert!(!screen_passes(GateInputs {
            treatment_resource_debt: 35,
            ..screen
        }));
        assert!(!screen_passes(GateInputs {
            treatment_wins: 8,
            ..screen
        }));

        let holdout = GateInputs {
            games: 126,
            fired_games: 63,
            repairs: 126,
            sign_p: 0.01,
            treatment_wins: 10,
            paired_win_score: 0.50,
            terminal_score_share: 0.50,
            treatment_cities: 100,
            ..screen
        };
        assert!(holdout_passes(holdout));
        assert!(!holdout_passes(GateInputs {
            treatment_cities: 99,
            ..holdout
        }));
    }
}
