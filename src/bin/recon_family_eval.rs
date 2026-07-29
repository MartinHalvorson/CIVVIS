//! Preregistered matched evaluation of a rules-derived recon-family cap.
//!
//! Each deployment map is replayed from major seats 0 and N-1 under exact
//! stock `strategic_deep` and under the same controller with one default-off
//! production-eligibility flag enabled. The map, not the seat-game, is the
//! inference unit. `Game::max_turns` stays 250 while the external observer may
//! continue the same stateful agents through turn 320.

use civvis::ai::{Ai, Weights};
use civvis::elo::builtin_ai;
use civvis::evolve::Champion;
use civvis::game::{default_difficulty, Action, Game, GameOptions, Item, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use civvis::strategic::StrategicAi;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 9_971_999;
const SCREEN_MAPS: usize = 12;
const SCREEN_SEED: u64 = 9_972_000;
const CONFIRM_MAPS: usize = 60;
const CONFIRM_SEED: u64 = 9_973_000;
const NOMINAL_TURNS: u32 = 250;
const OBSERVE_THROUGH: u32 = 320;
const FROZEN_AI: &str = "strategic_deep";
const EMBEDDED_CHAMPION: &str = include_str!("../../data/evolved/best.json");
const FROZEN_CHAMPION_GENERATION: u32 = 14;
const FROZEN_CHAMPION_FNV1A: u64 = 0x40b1_fbb2_a5b8_8bc6;
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

fn frozen_champion_weights() -> Weights {
    static WEIGHTS: OnceLock<Weights> = OnceLock::new();
    WEIGHTS
        .get_or_init(|| {
            assert_eq!(
                fnv1a(EMBEDDED_CHAMPION.as_bytes()),
                FROZEN_CHAMPION_FNV1A,
                "data/evolved/best.json changed after the recon-cap controller pin"
            );
            let champion: Champion = serde_json::from_str(EMBEDDED_CHAMPION)
                .expect("the committed advanced_evolved champion must be valid JSON");
            assert_eq!(
                champion.gen, FROZEN_CHAMPION_GENERATION,
                "recon-cap champion generation changed"
            );
            champion.weights
        })
        .clone()
}

fn number(args: &[String], key: &str, default: i64) -> i64 {
    let Some(index) = args.iter().position(|arg| arg == key) else {
        return default;
    };
    let value = args.get(index + 1).unwrap_or_else(|| {
        eprintln!("{key} requires an integer value");
        std::process::exit(2);
    });
    value.parse().unwrap_or_else(|_| {
        eprintln!("{key} requires an integer value; got {value:?}");
        std::process::exit(2);
    })
}

fn text(args: &[String], key: &str, default: &str) -> String {
    let Some(index) = args.iter().position(|arg| arg == key) else {
        return default.to_string();
    };
    args.get(index + 1).cloned().unwrap_or_else(|| {
        eprintln!("{key} requires a value");
        std::process::exit(2);
    })
}

fn flag_once(args: &[String], key: &str) -> bool {
    args.iter().filter(|arg| arg.as_str() == key).count() == 1
}

fn value_once(args: &[String], key: &str, expected: &str) -> bool {
    let positions = args
        .iter()
        .enumerate()
        .filter(|(_, arg)| arg.as_str() == key)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    positions.len() == 1 && args.get(positions[0] + 1).map(String::as_str) == Some(expected)
}

/// Only the byte-for-byte registered invocation may spend a phase. Diagnostic
/// defaults remain convenient, but a missing, duplicate, noncanonical, or
/// extra argument cannot inherit an official PASS/STOP label.
fn registered_profile(args: &[String], null: bool, maps: &str, seed: &str) -> bool {
    let expected_values = [
        ("--ai", FROZEN_AI),
        ("--maps", maps),
        ("--turns", "250"),
        ("--observe-through", "320"),
        ("--speed", "online"),
        ("--poles", "poles"),
        ("--victories", "science,culture,domination"),
        ("--seed", seed),
        ("--jobs", "6"),
    ];
    let expected_len = expected_values.len() * 2 + 2 + usize::from(null);
    args.len() == expected_len
        && flag_once(args, "--deployment-mix")
        && flag_once(args, "--randomize-civs")
        && flag_once(args, "--null") == null
        && expected_values
            .iter()
            .all(|(key, value)| value_once(args, key, value))
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

fn is_recon(g: &Game, unit: &str) -> bool {
    g.rules
        .units
        .get(unit)
        .is_some_and(|spec| spec.promotion_class == "recon")
}

fn recon_kinds(g: &Game) -> Vec<String> {
    g.rules
        .units
        .iter()
        .filter(|(_, spec)| spec.promotion_class == "recon")
        .map(|(name, _)| name.to_string())
        .collect()
}

fn active_recon_count(g: &Game, pid: usize) -> usize {
    g.player_unit_ids(pid)
        .into_iter()
        .filter(|unit| is_recon(g, &g.units[unit].kind))
        .count()
}

fn explored_share(g: &Game, pid: usize) -> f64 {
    g.players[pid].explored.len() as f64 / g.map.tiles.len().max(1) as f64
}

fn action_unit(action: &Action) -> Option<u32> {
    match action {
        Action::Move { unit, .. }
        | Action::MoveTo { unit, .. }
        | Action::Attack { unit, .. }
        | Action::Ranged { unit, .. }
        | Action::FoundCity { unit }
        | Action::Improve { unit, .. }
        | Action::ContributeProject { unit, .. }
        | Action::ContributeDistrict { unit, .. }
        | Action::PerformConcert { unit }
        | Action::UpgradeUnit { unit }
        | Action::Pillage { unit }
        | Action::RepairImprovement { unit }
        | Action::CoastalRaid { unit, .. }
        | Action::AirRebase { unit, .. }
        | Action::AirStrike { unit, .. }
        | Action::AirPillage { unit, .. }
        | Action::PriorityTarget { unit, .. }
        | Action::AirPatrol { unit, .. }
        | Action::Fortify { unit }
        | Action::Promote { unit, .. }
        | Action::Upgrade { unit, .. }
        | Action::CombineUnits { unit, .. }
        | Action::LinkUnits { unit, .. }
        | Action::UnlinkUnits { unit }
        | Action::TradeRoute { unit, .. }
        | Action::Spread { unit }
        | Action::TheologicalAttack { unit, .. }
        | Action::CondemnHeretic { unit, .. }
        | Action::HealReligious { unit }
        | Action::RemoveHeresy { unit }
        | Action::LaunchInquisition { unit }
        | Action::EvangelizeBelief { unit, .. }
        | Action::ConvertBarbarians { unit }
        | Action::BuildRailroad { unit } => Some(*unit),
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TrainingCount {
    production: u64,
    gold: u64,
    faith: u64,
}

impl TrainingCount {
    fn total(&self) -> u64 {
        self.production + self.gold + self.faith
    }

    fn absorb(&mut self, other: &TrainingCount) {
        self.production += other.production;
        self.gold += other.gold;
        self.faith += other.faith;
    }
}

#[derive(Clone, Debug, PartialEq)]
struct MechanismCensus {
    focal_turns: u32,
    training: BTreeMap<String, TrainingCount>,
    nominal_commitment: f64,
    max_family: usize,
    orders_after_90: u64,
    orders_after_100: u64,
    exploration_turns: [Option<u32>; 4],
    turn_200_explored: f64,
    terminal_explored: f64,
}

impl Default for MechanismCensus {
    fn default() -> Self {
        Self {
            focal_turns: 0,
            training: BTreeMap::new(),
            nominal_commitment: 0.0,
            max_family: 0,
            orders_after_90: 0,
            orders_after_100: 0,
            exploration_turns: [None; 4],
            turn_200_explored: 0.0,
            terminal_explored: 0.0,
        }
    }
}

impl MechanismCensus {
    fn training_total(&self) -> u64 {
        self.training.values().map(TrainingCount::total).sum()
    }

    fn scout_training(&self) -> u64 {
        self.training
            .get("scout")
            .map(TrainingCount::total)
            .unwrap_or(0)
    }
}

fn recon_item(unit: &str, formation: u8) -> Item {
    if formation == 0 {
        Item::Unit {
            unit: civvis::name::Name::new(unit),
        }
    } else {
        Item::Formation {
            unit: civvis::name::Name::new(unit),
            formation,
        }
    }
}

fn mark_existing_recon_ids(g: &Game, seen: &mut HashSet<u32>) {
    seen.extend(
        g.units
            .values()
            .filter(|unit| is_recon(g, &unit.kind))
            .map(|unit| unit.id),
    );
}

/// Production completes in `begin_turn`, before the focal controller acts.
/// Stable unit IDs see normal and Formation completions alike; `trained:*`
/// cannot be the source because the engine deliberately omits Formation
/// production from that counter.
fn observe_recon_production(
    g: &Game,
    focal: usize,
    seen: &mut HashSet<u32>,
    census: &mut MechanismCensus,
) {
    let completed = g
        .units
        .values()
        .filter(|unit| unit.owner == focal && !seen.contains(&unit.id) && is_recon(g, &unit.kind))
        .map(|unit| (unit.id, unit.kind.to_string(), unit.formation))
        .collect::<Vec<_>>();
    for (unit, kind, formation) in completed {
        seen.insert(unit);
        census.training.entry(kind.clone()).or_default().production += 1;
        census.nominal_commitment += g.item_cost(&recon_item(&kind, formation));
    }
}

fn update_exploration_milestones(census: &mut MechanismCensus, share: f64, turn: u32) {
    for (index, threshold) in [0.50, 0.80, 0.90, 1.00].into_iter().enumerate() {
        if census.exploration_turns[index].is_none() && share + 1e-12 >= threshold {
            census.exploration_turns[index] = Some(turn);
        }
    }
}

fn science_race_progress(g: &Game, pid: usize) -> i32 {
    let player = &g.players[pid];
    if player.science_projects.contains("exoplanet_expedition") {
        78 + (22.0 * player.exoplanet_distance / 50.0).clamp(0.0, 22.0) as i32
    } else if player.science_projects.contains("launch_mars_colony") {
        65
    } else if player.science_projects.contains("launch_moon_landing") {
        45
    } else if player.science_projects.contains("launch_earth_satellite") {
        25
    } else {
        0
    }
}

#[derive(Clone, Debug, PartialEq)]
struct GameResult {
    won: bool,
    victory: Option<String>,
    finish_turn: u32,
    score: i64,
    science_progress: i32,
    military_power: f64,
    cities: usize,
    districts: usize,
    buildings: usize,
    gold: f64,
    faith: f64,
    family_count: usize,
    family_kills: i64,
    cities_captured: u32,
    cities_lost: u32,
    census: MechanismCensus,
    /// Canonical persisted world state for the default-off causal null. The
    /// treatment batches leave this absent so a 60-map confirmation does not
    /// retain four complete worlds per map in memory.
    serialized_game: Option<String>,
}

fn pinned_strategic_deep(enabled: bool) -> Box<dyn Ai> {
    // The deployed controller currently has no promoted value net. Keep that
    // score-share behavior explicit so a cwd-local artifact cannot change a
    // registered arm while leaving the default-off null internally exact.
    let mut ai = StrategicAi::score_only_with_weights(frozen_champion_weights());
    ai.review_every = 20;
    ai.horizon = 80;
    ai.set_recon_family_cap(enabled);
    Box::new(ai)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FocalMode {
    Stock,
    CapOff,
    CapOn,
}

fn controller_fleet(g: &Game, focal: usize, mode: FocalMode, seed: u64) -> Vec<Box<dyn Ai>> {
    g.players
        .iter()
        .map(|player| {
            if player.is_minor || player.is_barbarian {
                builtin_ai("basic", seed.wrapping_add(player.id as u64))
            } else {
                pinned_strategic_deep(player.id == focal && mode == FocalMode::CapOn)
            }
        })
        .collect()
}

fn ownership(g: &Game) -> HashMap<u32, usize> {
    g.cities
        .values()
        .map(|city| (city.id, city.owner))
        .collect()
}

fn observe_city_losses(
    before: &HashMap<u32, usize>,
    after: &HashMap<u32, usize>,
    focal: usize,
    lost: &mut u32,
) {
    for (city, old_owner) in before {
        if *old_owner == focal && after.get(city).is_none_or(|new_owner| *new_owner != focal) {
            *lost += 1;
        }
    }
}

fn play(
    options: GameOptions,
    focal: usize,
    mode: FocalMode,
    observe_through: u32,
    record_state: bool,
) -> GameResult {
    let map_seed = options.seed;
    let mut game = Game::new_with(options);
    game.victory_conditions = VictoryConditions::parse("science,culture,domination").unwrap();
    let nominal_limit = game.max_turns;
    assert!(observe_through >= nominal_limit);
    let kinds = recon_kinds(&game);
    let mut ais = controller_fleet(&game, focal, mode, map_seed);
    assert_eq!(ais.len(), game.players.len());
    let mut census = MechanismCensus::default();
    census.max_family = active_recon_count(&game, focal);
    update_exploration_milestones(&mut census, explored_share(&game, focal), game.turn);
    let mut seen_recon_ids = HashSet::new();
    mark_existing_recon_ids(&game, &mut seen_recon_ids);
    let mut owners = ownership(&game);
    let mut lost = 0;
    let mut turn_200 = None;

    while game.winner.is_none() && game.turn <= observe_through {
        assert_eq!(
            game.max_turns, nominal_limit,
            "observer changed policy horizon"
        );
        let pid = game.current;
        let actor_turn = game.turn;
        if pid == focal {
            observe_recon_production(&game, focal, &mut seen_recon_ids, &mut census);
        }
        // Mark rival creations before any later levy, conversion, or ownership
        // transfer can make an old body look like focal production.
        mark_existing_recon_ids(&game, &mut seen_recon_ids);
        let log_start = game.log.len();
        let share_before = (pid == focal).then(|| explored_share(&game, focal));
        let kinds_before: HashMap<u32, String> = if pid == focal {
            game.player_unit_ids(focal)
                .into_iter()
                .map(|unit| (unit, game.units[&unit].kind.to_string()))
                .collect()
        } else {
            HashMap::new()
        };
        ais[pid].take_turn(&mut game, pid);

        if pid == focal {
            census.focal_turns += 1;
            let actions: Vec<Action> = game
                .log
                .since(log_start)
                .filter(|(owner, _)| *owner == focal)
                .map(|(_, action)| action.clone())
                .collect();
            let mut purchases: BTreeMap<(String, u8), (u64, u64)> = BTreeMap::new();
            for action in &actions {
                if let Action::Buy {
                    unit,
                    formation,
                    currency,
                    ..
                } = action
                {
                    if is_recon(&game, unit) {
                        let entry = purchases.entry((unit.to_string(), *formation)).or_default();
                        if currency == "gold" {
                            entry.0 += 1;
                        } else if currency == "faith" {
                            entry.1 += 1;
                        }
                    }
                }
            }
            for ((kind, formation), (gold, faith)) in purchases {
                let entry = census.training.entry(kind.clone()).or_default();
                entry.gold += gold;
                entry.faith += faith;
                census.nominal_commitment +=
                    game.item_cost(&recon_item(&kind, formation)) * (gold + faith) as f64;
            }

            let recon_orders = actions
                .iter()
                .filter_map(action_unit)
                .filter(|unit| {
                    kinds_before
                        .get(unit)
                        .map(String::as_str)
                        .or_else(|| game.units.get(unit).map(|unit| unit.kind.as_str()))
                        .is_some_and(|kind| is_recon(&game, kind))
                })
                .count() as u64;
            let share_before = share_before.unwrap();
            if share_before + 1e-12 >= 0.90 {
                census.orders_after_90 += recon_orders;
            }
            if share_before + 1e-12 >= 1.00 {
                census.orders_after_100 += recon_orders;
            }
            let share_after = explored_share(&game, focal);
            update_exploration_milestones(&mut census, share_after, actor_turn);
            if actor_turn == 200 {
                turn_200 = Some(share_after);
            }
        }

        // Purchases happen during the focal action phase and were classified
        // from their successful actions above. Record their IDs before the
        // following `begin_turn` creates the next seat's production.
        mark_existing_recon_ids(&game, &mut seen_recon_ids);
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }

        census.max_family = census.max_family.max(active_recon_count(&game, focal));
        let new_owners = ownership(&game);
        observe_city_losses(&owners, &new_owners, focal, &mut lost);
        owners = new_owners;
        if game.turn > 200 && turn_200.is_none() {
            turn_200 = Some(explored_share(&game, focal));
        }
    }

    assert_eq!(game.max_turns, nominal_limit);
    let terminal_explored = explored_share(&game, focal);
    // A game ending before turn 200 contributes its terminal, absorbed state
    // instead of disappearing selectively from this fixed-time comparison.
    census.turn_200_explored = turn_200.unwrap_or(terminal_explored);
    census.terminal_explored = terminal_explored;
    let family_count = active_recon_count(&game, focal);
    let family_kills = kinds
        .iter()
        .map(|kind| {
            game.players[focal]
                .counters
                .get(&format!("kill_with:{kind}"))
                .copied()
                .unwrap_or(0)
        })
        .sum();
    let city_ids = game.player_city_ids(focal);
    let finish_turn = if game.winner.is_some() {
        game.reported_turn()
    } else {
        observe_through
    };
    let serialized_game = record_state
        .then(|| serde_json::to_string(&game).expect("the causal-null world must serialize"));
    GameResult {
        won: game.winner == Some(focal),
        victory: (game.winner == Some(focal))
            .then(|| game.victory_type.clone())
            .flatten(),
        finish_turn,
        score: game.score(focal),
        science_progress: science_race_progress(&game, focal),
        military_power: game.military_power(focal),
        cities: city_ids.len(),
        districts: city_ids
            .iter()
            .map(|city| game.cities[city].districts.len())
            .sum(),
        buildings: city_ids
            .iter()
            .map(|city| game.cities[city].buildings.len())
            .sum(),
        gold: game.players[focal].gold,
        faith: game.players[focal].faith,
        family_count,
        family_kills,
        cities_captured: game.players[focal]
            .counters
            .get("captures")
            .copied()
            .unwrap_or(0)
            .max(0) as u32,
        cities_lost: lost,
        census,
        serialized_game,
    }
}

#[derive(Clone, Debug)]
struct MapResult {
    profile: DeploymentProfile,
    control: [GameResult; 2],
    treatment: [GameResult; 2],
}

fn observed_profile_values<T: Copy + Eq>(
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

fn map_score(control_wins: usize, treatment_wins: usize) -> f64 {
    0.5 + (treatment_wins as f64 - control_wins as f64) / 4.0
}

fn terminal_share(control: &GameResult, treatment: &GameResult) -> f64 {
    let control_score = control.score.max(0) as f64;
    let treatment_score = treatment.score.max(0) as f64;
    let total = control_score + treatment_score;
    if total > 0.0 {
        treatment_score / total
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

fn median(values: &[u32]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut values = values.to_vec();
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        Some((values[middle - 1] as f64 + values[middle] as f64) / 2.0)
    } else {
        Some(values[middle] as f64)
    }
}

fn ratio_at_most(treatment: u64, control: u64, maximum: f64) -> bool {
    if control == 0 {
        treatment == 0
    } else {
        treatment as f64 / control as f64 <= maximum + 1e-12
    }
}

#[derive(Default)]
struct ArmSummary {
    games: usize,
    wins: usize,
    victories: BTreeMap<String, usize>,
    training: BTreeMap<String, TrainingCount>,
    training_total: u64,
    scout_training: u64,
    games_with_five: usize,
    focal_turns: u64,
    nominal_commitment: f64,
    family_count: usize,
    max_family: usize,
    orders_after_90: u64,
    orders_after_100: u64,
    turn_200_explored: f64,
    terminal_explored: f64,
    finish_turns: u64,
    score: i64,
    science_progress: i64,
    military_power: f64,
    cities: usize,
    districts: usize,
    buildings: usize,
    gold: f64,
    faith: f64,
    family_kills: i64,
    cities_captured: u64,
    cities_lost: u64,
    exploration_turns: [Vec<u32>; 4],
}

impl ArmSummary {
    fn record(&mut self, result: &GameResult) {
        self.games += 1;
        self.wins += result.won as usize;
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
        for (kind, count) in &result.census.training {
            self.training.entry(kind.clone()).or_default().absorb(count);
        }
        let training = result.census.training_total();
        self.training_total += training;
        self.scout_training += result.census.scout_training();
        self.games_with_five += (training >= 5) as usize;
        self.focal_turns += result.census.focal_turns as u64;
        self.nominal_commitment += result.census.nominal_commitment;
        self.family_count += result.family_count;
        self.max_family += result.census.max_family;
        self.orders_after_90 += result.census.orders_after_90;
        self.orders_after_100 += result.census.orders_after_100;
        self.turn_200_explored += result.census.turn_200_explored;
        self.terminal_explored += result.census.terminal_explored;
        self.finish_turns += result.finish_turn as u64;
        self.score += result.score;
        self.science_progress += result.science_progress as i64;
        self.military_power += result.military_power;
        self.cities += result.cities;
        self.districts += result.districts;
        self.buildings += result.buildings;
        self.gold += result.gold;
        self.faith += result.faith;
        self.family_kills += result.family_kills;
        self.cities_captured += result.cities_captured as u64;
        self.cities_lost += result.cities_lost as u64;
        for (index, turn) in result.census.exploration_turns.iter().enumerate() {
            if let Some(turn) = turn {
                self.exploration_turns[index].push(*turn);
            }
        }
    }

    fn victory(&self, kind: &str) -> usize {
        self.victories.get(kind).copied().unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug)]
struct GateFacts {
    control_training: u64,
    treatment_training: u64,
    control_games_with_five: usize,
    focal_games: usize,
    control_orders_after_90: u64,
    treatment_orders_after_90: u64,
    control_family_count: u64,
    treatment_family_count: u64,
    commitment_avoided_per_game: f64,
    control_turn_200_explored: f64,
    treatment_turn_200_explored: f64,
    control_turn_80: Option<f64>,
    treatment_turn_80: Option<f64>,
    paired_score: f64,
    terminal_score: f64,
    favorable: usize,
    adverse: usize,
    sign_p: f64,
    control_wins: usize,
    treatment_wins: usize,
    control_science_wins: usize,
    treatment_science_wins: usize,
    control_culture_wins: usize,
    treatment_culture_wins: usize,
}

fn mechanism_coverage(facts: GateFacts) -> bool {
    facts.control_training >= 120 && facts.control_games_with_five * 4 >= facts.focal_games
}

fn outcome_noninferiority(facts: GateFacts) -> bool {
    facts.treatment_wins >= facts.control_wins
        && facts.treatment_science_wins >= facts.control_science_wins
        && facts.treatment_culture_wins >= facts.control_culture_wins
}

fn exploration_median_within(
    control: Option<f64>,
    treatment: Option<f64>,
    maximum_delay: f64,
) -> bool {
    matches!((control, treatment), (Some(old), Some(new)) if new <= old + maximum_delay + 1e-12)
}

fn screen_passes(facts: GateFacts) -> bool {
    mechanism_coverage(facts)
        && ratio_at_most(facts.treatment_training, facts.control_training, 0.35)
        && ratio_at_most(
            facts.treatment_orders_after_90,
            facts.control_orders_after_90,
            0.25,
        )
        && ratio_at_most(
            facts.treatment_family_count,
            facts.control_family_count,
            0.50,
        )
        && facts.commitment_avoided_per_game + 1e-12 >= 75.0
        && facts.treatment_turn_200_explored + 0.020_000_000_001 >= facts.control_turn_200_explored
        && exploration_median_within(facts.control_turn_80, facts.treatment_turn_80, 8.0)
        && facts.paired_score + 1e-12 >= 0.495
        && facts.terminal_score + 1e-12 >= 0.495
        && outcome_noninferiority(facts)
}

fn confirmation_passes(facts: GateFacts) -> bool {
    facts.control_games_with_five * 4 >= facts.focal_games
        && ratio_at_most(facts.treatment_training, facts.control_training, 0.35)
        && ratio_at_most(
            facts.treatment_orders_after_90,
            facts.control_orders_after_90,
            0.25,
        )
        && ratio_at_most(
            facts.treatment_family_count,
            facts.control_family_count,
            0.50,
        )
        && facts.commitment_avoided_per_game + 1e-12 >= 100.0
        && facts.treatment_turn_200_explored + 0.005_000_000_001 >= facts.control_turn_200_explored
        && exploration_median_within(facts.control_turn_80, facts.treatment_turn_80, 4.0)
        && facts.paired_score + 1e-12 >= 0.52
        && facts.favorable > facts.adverse
        && facts.sign_p < 0.05
        && facts.terminal_score + 1e-12 >= 0.50
        && outcome_noninferiority(facts)
}

fn format_turn(value: Option<f64>) -> String {
    value
        .map(|turn| format!("{turn:.1}"))
        .unwrap_or_else(|| "-".to_string())
}

fn print_training(label: &str, arm: &ArmSummary) {
    println!("{label} completed recon actions by rules-derived type:");
    if arm.training.is_empty() {
        println!("  none");
        return;
    }
    for (kind, count) in &arm.training {
        println!(
            "  {kind:<14} production {:>5}  gold {:>5}  faith {:>5}  total {:>5}",
            count.production,
            count.gold,
            count.faith,
            count.total(),
        );
    }
}

fn print_axis_summary(label: &str, results: Vec<&MapResult>) {
    let maps = results.len();
    if maps == 0 {
        return;
    }
    let mut control_training = 0u64;
    let mut treatment_training = 0u64;
    let mut paired = 0.0;
    let mut control_explored = 0.0;
    let mut treatment_explored = 0.0;
    for result in results {
        let control_wins = result.control.iter().filter(|game| game.won).count();
        let treatment_wins = result.treatment.iter().filter(|game| game.won).count();
        paired += map_score(control_wins, treatment_wins);
        for game in &result.control {
            control_training += game.census.training_total();
            control_explored += game.census.turn_200_explored;
        }
        for game in &result.treatment {
            treatment_training += game.census.training_total();
            treatment_explored += game.census.turn_200_explored;
        }
    }
    let cells = (maps * 2) as f64;
    let ratio = if control_training == 0 {
        if treatment_training == 0 {
            "both-zero".to_string()
        } else {
            "undefined".to_string()
        }
    } else {
        format!("{:.3}", treatment_training as f64 / control_training as f64)
    };
    println!(
        "  {label:<25} {maps:>3} maps  training {control_training:>5}->{treatment_training:<5} ({ratio})  paired {:>5.1}%  explored@200 {:>5.1}->{:>5.1}%",
        100.0 * paired / maps as f64,
        100.0 * control_explored / cells,
        100.0 * treatment_explored / cells,
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let deployment_mix = args.iter().any(|arg| arg == "--deployment-mix");
    if deployment_mix {
        let conflicts = PROFILE_OVERRIDE_FLAGS
            .iter()
            .copied()
            .filter(|flag| args.iter().any(|arg| arg == flag))
            .collect::<Vec<_>>();
        if !conflicts.is_empty() {
            eprintln!(
                "--deployment-mix derives every world profile; remove conflicting flags: {}",
                conflicts.join(", ")
            );
            std::process::exit(2);
        }
    }

    let players = number(&args, "--players", 8).max(2) as usize;
    let size = MapSize::for_players(players);
    let width = number(&args, "--width", size.width as i64).max(8) as i32;
    let height = number(&args, "--height", size.height as i64).max(8) as i32;
    let city_states =
        number(&args, "--city-states", size.default_city_states as i64).max(0) as usize;
    let map_name = text(&args, "--map", "continents");
    let map_script = MapScript::from_id(&map_name).unwrap_or_else(|| {
        eprintln!("unknown map script {map_name:?}");
        std::process::exit(2);
    });
    let shape_name = text(&args, "--shape", "planet");
    let map_topology = MapTopology::from_id(&shape_name).unwrap_or_else(|| {
        eprintln!("unknown map shape {shape_name:?}");
        std::process::exit(2);
    });
    let null_replay = args.iter().any(|arg| arg == "--null");
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
    let turns = number(&args, "--turns", NOMINAL_TURNS as i64).max(1) as u32;
    let observe_through = number(&args, "--observe-through", OBSERVE_THROUGH as i64).max(1) as u32;
    if observe_through < turns {
        eprintln!("--observe-through must be at least --turns");
        std::process::exit(2);
    }
    let seed = number(
        &args,
        "--seed",
        if null_replay {
            NULL_SEED as i64
        } else {
            SCREEN_SEED as i64
        },
    )
    .max(0) as u64;
    let requested_jobs = number(&args, "--jobs", 0);
    let jobs = match requested_jobs {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    }
    .clamp(1, 6);
    let ai_name = text(&args, "--ai", FROZEN_AI);
    if ai_name != FROZEN_AI {
        eprintln!("this experiment is frozen for {FROZEN_AI}; got controller {ai_name:?}");
        std::process::exit(2);
    }
    let speed = text(&args, "--speed", "online");
    let difficulty = text(&args, "--difficulty", &default_difficulty());
    let poles_name = text(&args, "--poles", "poles");
    let map_poles = MapPoles::from_id(&poles_name).unwrap_or_else(|| {
        eprintln!("unknown thermal distribution {poles_name:?}");
        std::process::exit(2);
    });
    let victory_names = text(&args, "--victories", "science,culture,domination");
    let victories = VictoryConditions::parse(&victory_names).unwrap_or_else(|why| {
        eprintln!("--victories: {why}");
        std::process::exit(2);
    });
    let expected_victories = VictoryConditions::parse("science,culture,domination").unwrap();
    if victories != expected_victories {
        eprintln!(
            "this experiment is frozen for science,culture,domination; got {victory_names:?}"
        );
        std::process::exit(2);
    }
    let randomize_civs = args.iter().any(|arg| arg == "--randomize-civs");
    let rules = Rules::embedded();
    if !rules.speeds.contains_key(&speed) {
        eprintln!("unknown game speed {speed:?}");
        std::process::exit(2);
    }
    if !rules.difficulties.contains_key(&difficulty) {
        eprintln!("unknown difficulty {difficulty:?}");
        std::process::exit(2);
    }
    // Assert the embedded controller identity before constructing any game.
    let _ = frozen_champion_weights();
    println!(
        "agent: {FROZEN_AI}; embedded champion generation {FROZEN_CHAMPION_GENERATION}, \
         fnv1a:{FROZEN_CHAMPION_FNV1A:016x}; score-share evaluator; review 20, horizon 80"
    );

    println!("Recon-family production-cap evaluator");
    if deployment_mix {
        let player_batch = deployment_counts(maps, |profile| profile.players)
            .into_iter()
            .map(|(value, count)| format!("{value}p={count}"))
            .collect::<Vec<_>>()
            .join(",");
        let script_batch = deployment_counts(maps, |profile| profile.map_script)
            .into_iter()
            .map(|(value, count)| format!("{}={count}", value.id()))
            .collect::<Vec<_>>()
            .join(",");
        let topology_batch = deployment_counts(maps, |profile| profile.map_topology)
            .into_iter()
            .map(|(value, count)| format!("{}={count}", value.id()))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "profile: deployment mix; players {player_batch}; scripts {script_batch}; topologies {topology_batch}"
        );
    } else {
        println!(
            "profile: diagnostic fixed cell {players}p {width}x{height}+{city_states}cs, map {}, shape {}",
            map_script.id(),
            map_topology.id(),
        );
    }
    println!(
        "rules: {turns} nominal {speed} turns, observe through {observe_through}; poles {}; civilizations {}; victories {victory_names}; seed {seed}; {jobs} jobs; difficulty {difficulty}",
        map_poles.id(),
        if randomize_civs {
            "randomized"
        } else {
            "fixed stock"
        },
    );
    println!(
        "batch: {maps} maps x seats 0/N-1 x control/treatment = {} games; treatment {}",
        maps * 4,
        if null_replay {
            "NULL stock strategic_deep"
        } else {
            "strategic_deep with recon_family_cap"
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
            let map_seed = seed + map as u64;
            let mut options = GameOptions::new(
                profile.players,
                profile.width,
                profile.height,
                map_seed,
                turns,
                profile.city_states,
            );
            options.speed = speed.clone();
            options.difficulty = difficulty.clone();
            options.map_script = profile.map_script;
            options.map_topology = profile.map_topology;
            options.map_poles = map_poles;
            options.randomize_civs = randomize_civs;
            let seats = [0, profile.players - 1];
            let control = [
                play(
                    options.clone(),
                    seats[0],
                    FocalMode::Stock,
                    observe_through,
                    null_replay,
                ),
                play(
                    options.clone(),
                    seats[1],
                    FocalMode::Stock,
                    observe_through,
                    null_replay,
                ),
            ];
            let comparison_mode = if null_replay {
                FocalMode::CapOff
            } else {
                FocalMode::CapOn
            };
            let treatment = [
                play(
                    options.clone(),
                    seats[0],
                    comparison_mode,
                    observe_through,
                    null_replay,
                ),
                play(
                    options,
                    seats[1],
                    comparison_mode,
                    observe_through,
                    null_replay,
                ),
            ];
            MapResult {
                profile,
                control,
                treatment,
            }
        },
        |completed, _| eprintln!("progress: {}/{} maps complete", completed + 1, maps),
    );

    let mut control = ArmSummary::default();
    let mut treatment = ArmSummary::default();
    let mut paired_score = 0.0;
    let mut paired_terminal = 0.0;
    let mut favorable = 0usize;
    let mut adverse = 0usize;
    let mut terminal_favorable = 0usize;
    let mut terminal_adverse = 0usize;
    let mut control_mutual_80 = Vec::new();
    let mut treatment_mutual_80 = Vec::new();
    let mut exact_mismatches = 0usize;

    for result in &results {
        let control_wins = result.control.iter().filter(|game| game.won).count();
        let treatment_wins = result.treatment.iter().filter(|game| game.won).count();
        paired_score += map_score(control_wins, treatment_wins);
        match treatment_wins.cmp(&control_wins) {
            std::cmp::Ordering::Greater => favorable += 1,
            std::cmp::Ordering::Less => adverse += 1,
            std::cmp::Ordering::Equal => {}
        }
        let map_terminal = result
            .control
            .iter()
            .zip(&result.treatment)
            .map(|(old, new)| terminal_share(old, new))
            .sum::<f64>()
            / 2.0;
        paired_terminal += map_terminal;
        if map_terminal > 0.5 + 1e-12 {
            terminal_favorable += 1;
        } else if map_terminal < 0.5 - 1e-12 {
            terminal_adverse += 1;
        }
        for (old, new) in result.control.iter().zip(&result.treatment) {
            control.record(old);
            treatment.record(new);
            if let (Some(old_turn), Some(new_turn)) = (
                old.census.exploration_turns[1],
                new.census.exploration_turns[1],
            ) {
                control_mutual_80.push(old_turn);
                treatment_mutual_80.push(new_turn);
            }
            exact_mismatches += (old != new) as usize;
        }
    }
    paired_score /= maps as f64;
    paired_terminal /= maps as f64;
    let sign_p = exact_two_sided(favorable, favorable + adverse);
    let terminal_p = exact_two_sided(terminal_favorable, terminal_favorable + terminal_adverse);
    let mutual_80_cells = control_mutual_80.len();
    let control_mutual_80 = median(&control_mutual_80);
    let treatment_mutual_80 = median(&treatment_mutual_80);
    let focal_games = control.games.max(1);
    let commitment_avoided =
        (control.nominal_commitment - treatment.nominal_commitment) / focal_games as f64;
    let facts = GateFacts {
        control_training: control.training_total,
        treatment_training: treatment.training_total,
        control_games_with_five: control.games_with_five,
        focal_games: control.games,
        control_orders_after_90: control.orders_after_90,
        treatment_orders_after_90: treatment.orders_after_90,
        control_family_count: control.family_count as u64,
        treatment_family_count: treatment.family_count as u64,
        commitment_avoided_per_game: commitment_avoided,
        control_turn_200_explored: control.turn_200_explored / focal_games as f64,
        treatment_turn_200_explored: treatment.turn_200_explored / focal_games as f64,
        control_turn_80: control_mutual_80,
        treatment_turn_80: treatment_mutual_80,
        paired_score,
        terminal_score: paired_terminal,
        favorable,
        adverse,
        sign_p,
        control_wins: control.wins,
        treatment_wins: treatment.wins,
        control_science_wins: control.victory("science"),
        treatment_science_wins: treatment.victory("science"),
        control_culture_wins: control.victory("culture"),
        treatment_culture_wins: treatment.victory("culture"),
    };

    println!();
    print_training("control", &control);
    print_training("treatment", &treatment);
    println!();
    println!(
        "arm        wins/games  turns  score  sci%  power  cities  districts  buildings  gold  faith"
    );
    for (label, arm) in [("control", &control), ("treatment", &treatment)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{label:<10} {:>3}/{:<3} {:>6.1} {:>6.1} {:>5.1} {:>7.1} {:>7.2} {:>10.2} {:>10.2} {:>7.1} {:>7.1}",
            arm.wins,
            arm.games,
            arm.finish_turns as f64 / n,
            arm.score as f64 / n,
            arm.science_progress as f64 / n,
            arm.military_power / n,
            arm.cities as f64 / n,
            arm.districts as f64 / n,
            arm.buildings as f64 / n,
            arm.gold / n,
            arm.faith / n,
        );
    }
    println!(
        "victory types: control {:?}; treatment {:?}",
        control.victories, treatment.victories
    );
    for (label, arm) in [("control", &control), ("treatment", &treatment)] {
        let games = arm.games.max(1) as f64;
        let turns = arm.focal_turns.max(1) as f64;
        println!(
            "{label} mechanism: {} training ({:.2}/100 focal turns; Scouts {:.2}/100), {}/{} games >=5; nominal commitment {:.1}/game; terminal/max family {:.2}/{:.2}; orders after 90/100% {}/{}; kills {:.2}; captures/losses {:.2}/{:.2}",
            arm.training_total,
            100.0 * arm.training_total as f64 / turns,
            100.0 * arm.scout_training as f64 / turns,
            arm.games_with_five,
            arm.games,
            arm.nominal_commitment / games,
            arm.family_count as f64 / games,
            arm.max_family as f64 / games,
            arm.orders_after_90,
            arm.orders_after_100,
            arm.family_kills as f64 / games,
            arm.cities_captured as f64 / games,
            arm.cities_lost as f64 / games,
        );
        println!(
            "{label} exploration: turn 50/80/90/100% {}/{}/{}/{}; share turn 200 {:.2}%, terminal {:.2}%",
            format_turn(median(&arm.exploration_turns[0])),
            format_turn(median(&arm.exploration_turns[1])),
            format_turn(median(&arm.exploration_turns[2])),
            format_turn(median(&arm.exploration_turns[3])),
            100.0 * arm.turn_200_explored / games,
            100.0 * arm.terminal_explored / games,
        );
    }
    println!(
        "mutually observed 80% cells: {}/{}; median control {}, treatment {}",
        mutual_80_cells,
        control.games,
        format_turn(control_mutual_80),
        format_turn(treatment_mutual_80),
    );
    println!(
        "paired map win score: {:.1}%; favorable {favorable}, neutral {}, adverse {adverse}; exact two-sided sign p={sign_p:.4}",
        100.0 * paired_score,
        maps - favorable - adverse,
    );
    println!(
        "paired terminal-score share: {:.1}%; favorable {terminal_favorable}, neutral {}, adverse {terminal_adverse}; exact p={terminal_p:.4}",
        100.0 * paired_terminal,
        maps - terminal_favorable - terminal_adverse,
    );
    println!(
        "paired nominal commitment avoided: {commitment_avoided:.1} speed-scaled Production-equivalent per focal game"
    );

    println!("deployment-axis summaries (descriptive only; gates remain pooled):");
    for players in observed_profile_values(&results, |profile| profile.players) {
        print_axis_summary(
            &format!("players={players}"),
            results
                .iter()
                .filter(|result| result.profile.players == players)
                .collect(),
        );
    }
    for script in observed_profile_values(&results, |profile| profile.map_script) {
        print_axis_summary(
            &format!("map={}", script.id()),
            results
                .iter()
                .filter(|result| result.profile.map_script == script)
                .collect(),
        );
    }
    for topology in observed_profile_values(&results, |profile| profile.map_topology) {
        print_axis_summary(
            &format!("shape={}", topology.id()),
            results
                .iter()
                .filter(|result| result.profile.map_topology == topology)
                .collect(),
        );
    }

    if null_replay {
        if exact_mismatches == 0 {
            if maps == NULL_MAPS
                && seed == NULL_SEED
                && registered_profile(&args, true, "4", "9971999")
            {
                println!(
                    "frozen default-off null: PASS — all {} pinned/custom serialized worlds reproduced exactly",
                    control.games
                );
            } else {
                println!(
                    "diagnostic default-off null: PASS — all {} pinned/custom serialized worlds reproduced exactly; no registered gate spent",
                    control.games
                );
            }
        } else {
            println!(
                "default-off null: BROKEN — {exact_mismatches}/{} builtin/custom serialized worlds differed",
                control.games
            );
            std::process::exit(3);
        }
        return;
    }

    if maps == SCREEN_MAPS
        && seed == SCREEN_SEED
        && registered_profile(&args, false, "12", "9972000")
    {
        println!(
            "screen gate: {}",
            if screen_passes(facts) {
                "PASS — run only the fixed seed-9973000 confirmation"
            } else {
                "STOP — retain stock; do not tune, retry, or inspect confirmation"
            }
        );
    } else if maps == CONFIRM_MAPS
        && seed == CONFIRM_SEED
        && registered_profile(&args, false, "60", "9973000")
    {
        println!(
            "confirmation gate: {}",
            if confirmation_passes(facts) {
                "PASS — a separate gameplay-integration PR is permitted"
            } else {
                "RETAIN stock strategic_deep — no integration or rescue run"
            }
        );
    } else {
        println!("decision: DIAGNOSTIC ONLY — no preregistered gate applies");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use civvis::name::Name;

    fn passing_facts() -> GateFacts {
        GateFacts {
            control_training: 200,
            treatment_training: 60,
            control_games_with_five: 6,
            focal_games: 24,
            control_orders_after_90: 100,
            treatment_orders_after_90: 20,
            control_family_count: 100,
            treatment_family_count: 40,
            commitment_avoided_per_game: 110.0,
            control_turn_200_explored: 0.90,
            treatment_turn_200_explored: 0.895,
            control_turn_80: Some(150.0),
            treatment_turn_80: Some(153.0),
            paired_score: 0.53,
            terminal_score: 0.51,
            favorable: 20,
            adverse: 5,
            sign_p: 0.004,
            control_wins: 10,
            treatment_wins: 10,
            control_science_wins: 3,
            treatment_science_wins: 3,
            control_culture_wins: 4,
            treatment_culture_wins: 4,
        }
    }

    #[test]
    fn deployment_cycle_is_factorial_and_frozen_batches_restart_at_zero() {
        let cycle = (0..126).map(deployment_profile).collect::<Vec<_>>();
        for (index, profile) in cycle.iter().enumerate() {
            assert!(!cycle[..index].contains(profile), "duplicate {profile:?}");
        }
        assert_eq!(
            deployment_counts(NULL_MAPS, |profile| profile.players),
            vec![(4, 1), (6, 1), (8, 1), (10, 1)]
        );
        assert_eq!(
            deployment_counts(SCREEN_MAPS, |profile| profile.players),
            vec![(4, 2), (6, 2), (8, 2), (10, 2), (5, 2), (7, 1), (9, 1)]
        );
        assert_eq!(
            deployment_counts(CONFIRM_MAPS, |profile| profile.players),
            vec![(4, 9), (6, 9), (8, 9), (10, 9), (5, 8), (7, 8), (9, 8)]
        );
        assert_eq!(
            deployment_counts(SCREEN_MAPS, |profile| profile.map_topology),
            vec![(MapTopology::Flat, 6), (MapTopology::Planet, 6)]
        );
        assert_eq!(
            deployment_counts(CONFIRM_MAPS, |profile| profile.map_topology),
            vec![(MapTopology::Flat, 30), (MapTopology::Planet, 30)]
        );
        assert_eq!(deployment_profile(0), deployment_profile(126));
    }

    #[test]
    fn controller_and_registered_invocation_are_pinned_exactly() {
        assert_eq!(fnv1a(EMBEDDED_CHAMPION.as_bytes()), FROZEN_CHAMPION_FNV1A);
        let champion: Champion = serde_json::from_str(EMBEDDED_CHAMPION).unwrap();
        assert_eq!(champion.gen, FROZEN_CHAMPION_GENERATION);
        let _ = frozen_champion_weights();

        let args = "--null --deployment-mix --ai strategic_deep --maps 4 --turns 250 \
                    --observe-through 320 --speed online --poles poles --randomize-civs \
                    --victories science,culture,domination --seed 9971999 --jobs 6"
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert!(registered_profile(&args, true, "4", "9971999"));

        let mut noncanonical = args.clone();
        *noncanonical
            .iter_mut()
            .find(|arg| arg.as_str() == "250")
            .unwrap() = "0250".to_string();
        assert!(!registered_profile(&noncanonical, true, "4", "9971999"));

        let mut duplicate = args.clone();
        duplicate.extend(["--jobs".to_string(), "6".to_string()]);
        assert!(!registered_profile(&duplicate, true, "4", "9971999"));

        let mut extra = args;
        extra.extend(["--difficulty".to_string(), "prince".to_string()]);
        assert!(!registered_profile(&extra, true, "4", "9971999"));
    }

    #[test]
    fn custom_default_off_entrant_is_serialized_world_identical_to_pinned_stock() {
        let options = GameOptions::new(2, 20, 14, 99_719, 1, 0);
        let stock = play(options.clone(), 0, FocalMode::Stock, 1, true);
        let custom_off = play(options, 0, FocalMode::CapOff, 1, true);

        assert!(stock.serialized_game.is_some());
        assert_eq!(
            stock, custom_off,
            "the custom entrant changes persisted world or observer state while the cap is off"
        );
    }

    #[test]
    fn unit_order_extractor_excludes_queue_and_purchase_management() {
        assert_eq!(
            action_unit(&Action::Move {
                unit: 7,
                to: (1, 2),
            }),
            Some(7)
        );
        assert_eq!(action_unit(&Action::UpgradeUnit { unit: 9 }), Some(9));
        assert_eq!(
            action_unit(&Action::Buy {
                city: 1,
                unit: Name::new("scout"),
                formation: 0,
                currency: "gold".to_string(),
            }),
            None
        );
        assert_eq!(
            action_unit(&Action::Produce {
                city: 1,
                item: Item::Unit {
                    unit: Name::new("scout"),
                },
            }),
            None
        );
    }

    #[test]
    fn production_observer_counts_a_formation_once_at_its_nominal_cost() {
        let mut game = Game::new_full(1, 20, 14, 99_728, 100, 0, false);
        let ranger = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.rules.units[&game.units[unit].kind].class == "military")
            .unwrap();
        let unit = game.units.get_mut(&ranger).unwrap();
        unit.kind = Name::new("ranger");
        unit.formation = 1;
        let mut seen = HashSet::new();
        let mut census = MechanismCensus::default();

        observe_recon_production(&game, 0, &mut seen, &mut census);
        observe_recon_production(&game, 0, &mut seen, &mut census);

        assert_eq!(census.training["ranger"].production, 1);
        assert_eq!(census.training_total(), 1);
        assert_eq!(
            census.nominal_commitment.to_bits(),
            game.item_cost(&Item::Formation {
                unit: Name::new("ranger"),
                formation: 1,
            })
            .to_bits()
        );
    }

    #[test]
    fn denominator_zero_is_not_recast_as_a_zero_ratio() {
        assert!(ratio_at_most(0, 0, 0.35));
        assert!(!ratio_at_most(1, 0, 0.35));
        assert!(ratio_at_most(35, 100, 0.35));
        assert!(!ratio_at_most(36, 100, 0.35));
    }

    #[test]
    fn medians_and_sign_tests_keep_maps_as_the_inference_units() {
        assert_eq!(median(&[5, 1, 3]), Some(3.0));
        assert_eq!(median(&[4, 1, 3, 2]), Some(2.5));
        assert_eq!(median(&[]), None);
        assert!((exact_two_sided(5, 5) - 0.0625).abs() < 1e-12);
        assert_eq!(exact_two_sided(0, 0), 1.0);
        assert_eq!(map_score(0, 2), 1.0);
        assert_eq!(map_score(1, 1), 0.5);
        assert_eq!(map_score(2, 0), 0.0);
    }

    #[test]
    fn ownership_transitions_count_focal_losses_once_including_raze() {
        let before = HashMap::from([(1, 0), (2, 1), (3, 2)]);
        let after = HashMap::from([(1, 1), (3, 2)]);
        let mut lost = 0;
        observe_city_losses(&before, &after, 0, &mut lost);
        assert_eq!(lost, 1);
    }

    #[test]
    fn screen_and_confirmation_enforce_every_frozen_boundary() {
        let passing = passing_facts();
        assert!(screen_passes(passing));
        assert!(confirmation_passes(passing));
        assert!(!screen_passes(GateFacts {
            control_training: 119,
            ..passing
        }));
        assert!(!screen_passes(GateFacts {
            treatment_training: 71,
            ..passing
        }));
        assert!(!screen_passes(GateFacts {
            commitment_avoided_per_game: 74.99,
            ..passing
        }));
        assert!(!screen_passes(GateFacts {
            treatment_turn_80: Some(158.01),
            ..passing
        }));
        assert!(!screen_passes(GateFacts {
            treatment_science_wins: 2,
            ..passing
        }));
        assert!(!confirmation_passes(GateFacts {
            paired_score: 0.519,
            ..passing
        }));
        assert!(!confirmation_passes(GateFacts {
            sign_p: 0.05,
            ..passing
        }));
        assert!(!confirmation_passes(GateFacts {
            favorable: 5,
            adverse: 5,
            ..passing
        }));
    }
}
