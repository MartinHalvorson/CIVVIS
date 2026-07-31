//! Matched evaluation of marginal power-plant conversion valuation.
//!
//! The treatment changes only how the focal `AdvancedAi` values the three
//! ordinary conversion projects: target utility minus the utility of the
//! plant already owned. Every cost, legality rule, action, and non-conversion
//! production score remains stock. Two focal seats are averaged within each
//! map, and the map is the only inference unit.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::evolve::Champion;
use civvis::game::{Action, Game, GameOptions, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapTopology};
use std::collections::{BTreeMap, BTreeSet};

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 9_975_999;
const SCREEN_MAPS: usize = 12;
const SCREEN_SEED: u64 = 9_976_000;
const CONFIRM_MAPS: usize = 60;
const CONFIRM_SEED: u64 = 9_977_000;
const NOMINAL_TURNS: u32 = 250;
const OBSERVE_THROUGH: u32 = 320;
const REGISTERED_WIDTH: i32 = 105;
const REGISTERED_HEIGHT: i32 = 44;
const REGISTERED_TILES: usize = 4_412;
const FROZEN_AI: &str = "advanced_evolved";
const FROZEN_CHAMPION_GENERATION: u32 = 14;
/// Fingerprint of `data/evolved/best.json`, re-pinned 2026-07-31.
///
/// This binary froze `advanced_evolved` so its own screens stayed
/// interpretable, and its experiment finished: the registered 2026-07-29
/// result is **STOP, retain stock `AdvancedAi`**, a recorded null. Those
/// numbers were measured on the gen-14 champion as it stood then.
///
/// The champion has since been replaced deliberately — the same genome with
/// `docs/GENOME.md`'s eleven economy and expansion genes reverted to
/// `Weights::default()`, promoted on three `ai_eval --matrix` runs at 300
/// maps per profile (seeds 67,000,000 and 70,000,000, the last built from the
/// current tip), all `PASS`. See `docs/EVAL.md`.
///
/// ⚠ The re-pin does **not** revise the recorded reactor result, which stands
/// as measured. It does mean any *future* `reactor_conversion_eval` run is on
/// a different agent, so a new number here is not comparable with the
/// 2026-07-29 one and must say which champion it ran against.
const FROZEN_CHAMPION_FNV1A: u64 = 0x31cd_12c3_a1ba_5302;
const EMBEDDED_CHAMPION: &str = include_str!("../../data/evolved/best.json");
const FLAG_OPTIONS: [&str; 2] = ["--null", "--randomize-civs"];
const VALUE_OPTIONS: [&str; 15] = [
    "--ai",
    "--maps",
    "--players",
    "--width",
    "--height",
    "--city-states",
    "--turns",
    "--observe-through",
    "--speed",
    "--map",
    "--shape",
    "--poles",
    "--victories",
    "--seed",
    "--jobs",
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

fn frozen_champion() -> Champion {
    assert_eq!(
        fnv1a(EMBEDDED_CHAMPION.as_bytes()),
        FROZEN_CHAMPION_FNV1A,
        "data/evolved/best.json changed after the reactor evaluator pin"
    );
    let champion: Champion = serde_json::from_str(EMBEDDED_CHAMPION)
        .expect("the committed advanced_evolved champion must be valid JSON");
    assert_eq!(
        champion.gen, FROZEN_CHAMPION_GENERATION,
        "reactor evaluator champion generation changed"
    );
    champion
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RawArgs {
    flags: BTreeSet<String>,
    values: BTreeMap<String, String>,
}

impl RawArgs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut parsed = Self::default();
        let mut index = 0;
        while index < args.len() {
            let key = args[index].as_str();
            if FLAG_OPTIONS.contains(&key) {
                if !parsed.flags.insert(key.to_string()) {
                    return Err(format!("duplicate argument {key}"));
                }
                index += 1;
                continue;
            }
            if !VALUE_OPTIONS.contains(&key) {
                return Err(format!("unsupported argument {key:?}"));
            }
            if parsed.values.contains_key(key) {
                return Err(format!("duplicate argument {key}"));
            }
            let value = args
                .get(index + 1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| format!("{key} requires a value"))?;
            parsed.values.insert(key.to_string(), value.clone());
            index += 2;
        }
        Ok(parsed)
    }

    fn flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }

    fn value<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.values.get(key).map(String::as_str).unwrap_or(default)
    }

    fn integer(&self, key: &str, default: i64) -> Result<i64, String> {
        let Some(value) = self.values.get(key) else {
            return Ok(default);
        };
        value
            .parse::<i64>()
            .map_err(|_| format!("{key} requires an integer value; got {value:?}"))
    }
}

#[derive(Clone, Debug)]
struct Config {
    null: bool,
    maps: usize,
    players: usize,
    width: i32,
    height: i32,
    city_states: usize,
    turns: u32,
    observe_through: u32,
    speed: String,
    map_script: MapScript,
    map_topology: MapTopology,
    map_poles: MapPoles,
    randomize_civs: bool,
    seed: u64,
    jobs: usize,
}

impl Config {
    fn from_raw(raw: &RawArgs) -> Result<Self, String> {
        let null = raw.flag("--null");
        let maps = raw.integer(
            "--maps",
            if null {
                NULL_MAPS as i64
            } else {
                SCREEN_MAPS as i64
            },
        )?;
        let players = raw.integer("--players", 8)?;
        let width = raw.integer("--width", 84)?;
        let height = raw.integer("--height", 54)?;
        let city_states = raw.integer("--city-states", 12)?;
        let turns = raw.integer("--turns", NOMINAL_TURNS as i64)?;
        let observe_through = raw.integer("--observe-through", OBSERVE_THROUGH as i64)?;
        let seed = raw.integer(
            "--seed",
            if null {
                NULL_SEED as i64
            } else {
                SCREEN_SEED as i64
            },
        )?;
        let requested_jobs = raw.integer("--jobs", 0)?;
        if maps <= 0 {
            return Err("--maps must be positive".to_string());
        }
        if players < 2 {
            return Err("--players must be at least 2".to_string());
        }
        if width < 8 || height < 8 {
            return Err("--width and --height must each be at least 8".to_string());
        }
        if city_states < 0 {
            return Err("--city-states cannot be negative".to_string());
        }
        if turns <= 0 || observe_through <= 0 {
            return Err("--turns and --observe-through must be positive".to_string());
        }
        if observe_through < turns {
            return Err("--observe-through must be at least --turns".to_string());
        }
        if seed < 0 {
            return Err("--seed cannot be negative".to_string());
        }
        let jobs = if requested_jobs == 0 {
            civvis::parallel::default_jobs().min(6)
        } else if (1..=6).contains(&requested_jobs) {
            requested_jobs as usize
        } else {
            return Err("--jobs must be between 1 and 6".to_string());
        };
        let ai = raw.value("--ai", FROZEN_AI);
        if ai != FROZEN_AI {
            return Err(format!(
                "this experiment is frozen for {FROZEN_AI}; got controller {ai:?}"
            ));
        }
        let speed = raw.value("--speed", "online").to_string();
        let rules = Rules::embedded();
        if !rules.speeds.contains_key(&speed) {
            return Err(format!("unknown game speed {speed:?}"));
        }
        let map_name = raw.value("--map", "continents");
        let map_script = MapScript::from_id(map_name)
            .ok_or_else(|| format!("unknown map script {map_name:?}"))?;
        let shape_name = raw.value("--shape", "planet");
        let map_topology = MapTopology::from_id(shape_name)
            .ok_or_else(|| format!("unknown map shape {shape_name:?}"))?;
        let poles_name = raw.value("--poles", "poles");
        let map_poles = MapPoles::from_id(poles_name)
            .ok_or_else(|| format!("unknown thermal distribution {poles_name:?}"))?;
        let victory_names = raw.value("--victories", "science,culture,domination");
        let victories =
            VictoryConditions::parse(victory_names).map_err(|why| format!("--victories: {why}"))?;
        let expected_victories = VictoryConditions {
            science: true,
            culture: true,
            religious: false,
            diplomatic: false,
            domination: true,
            score: false,
        };
        if victories != expected_victories {
            return Err(format!(
                "this treatment is defined only for science,culture,domination; got {victory_names:?}"
            ));
        }
        Ok(Self {
            null,
            maps: maps as usize,
            players: players as usize,
            width: width as i32,
            height: height as i32,
            city_states: city_states as usize,
            turns: turns as u32,
            observe_through: observe_through as u32,
            speed,
            map_script,
            map_topology,
            map_poles,
            randomize_civs: raw.flag("--randomize-civs"),
            seed: seed as u64,
            jobs,
        })
    }
}

fn registered_profile(raw: &RawArgs, null: bool, maps: &str, seed: &str) -> bool {
    let expected = [
        ("--ai", FROZEN_AI),
        ("--maps", maps),
        ("--players", "8"),
        ("--width", "84"),
        ("--height", "54"),
        ("--city-states", "12"),
        ("--turns", "250"),
        ("--observe-through", "320"),
        ("--speed", "online"),
        ("--map", "continents"),
        ("--shape", "planet"),
        ("--poles", "poles"),
        ("--victories", "science,culture,domination"),
        ("--seed", seed),
        ("--jobs", "6"),
    ];
    raw.values.len() == expected.len()
        && expected
            .iter()
            .all(|(key, value)| raw.values.get(*key).map(String::as_str) == Some(*value))
        && raw.flags.len() == 1 + usize::from(null)
        && raw.flag("--randomize-civs")
        && raw.flag("--null") == null
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Control,
    Null,
    Treatment,
}

fn require_policy_horizon(game: &Game, expected: u32, boundary: &str) -> Result<(), String> {
    if game.max_turns != expected {
        return Err(format!(
            "policy horizon changed at {boundary}: expected {expected}, observed {}",
            game.max_turns
        ));
    }
    Ok(())
}

fn run_actor(game: &mut Game, actor: &mut dyn Ai, expected_horizon: u32) -> Result<(), String> {
    require_policy_horizon(game, expected_horizon, "actor entry")?;
    let pid = game.current;
    let before = (game.turn, game.current);
    actor.take_turn(game, pid);
    require_policy_horizon(game, expected_horizon, "after controller")?;
    if game.winner.is_none() && game.current == pid {
        game.apply(pid, &Action::EndTurn).map_err(|why| {
            format!(
                "turn {} seat {pid}: fallback EndTurn failed: {why}",
                game.turn
            )
        })?;
        require_policy_horizon(game, expected_horizon, "after fallback EndTurn")?;
    }
    if game.winner.is_none() && (game.turn, game.current) == before {
        return Err(format!(
            "turn {} seat {pid}: controller and fallback did not advance the actor",
            game.turn
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ConversionStats {
    coal: u64,
    oil: u64,
    uranium: u64,
    recommissions: u64,
    accidents: u64,
}

impl ConversionStats {
    fn total(self) -> u64 {
        self.coal + self.oil + self.uranium
    }

    fn nominal_online_production(self) -> i64 {
        self.coal as i64 * 100 + self.oil as i64 * 150 + self.uranium as i64 * 200
    }

    fn add(&mut self, other: Self) {
        self.coal += other.coal;
        self.oil += other.oil;
        self.uranium += other.uranium;
        self.recommissions += other.recommissions;
        self.accidents += other.accidents;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PlantMix {
    coal: usize,
    oil: usize,
    nuclear: usize,
}

impl PlantMix {
    fn add(&mut self, other: Self) {
        self.coal += other.coal;
        self.oil += other.oil;
        self.nuclear += other.nuclear;
    }
}

fn nonnegative_counter(game: &Game, pid: usize, key: &str) -> u64 {
    game.players[pid]
        .counters
        .get(key)
        .copied()
        .unwrap_or(0)
        .max(0) as u64
}

fn conversion_stats(game: &Game, pid: usize) -> ConversionStats {
    let accidents = game.players[pid]
        .counters
        .iter()
        .filter(|(key, _)| key.starts_with("reactor_accident:"))
        .map(|(_, value)| (*value).max(0) as u64)
        .sum();
    ConversionStats {
        coal: nonnegative_counter(game, pid, "project:convert_reactor_to_coal"),
        oil: nonnegative_counter(game, pid, "project:convert_reactor_to_oil"),
        uranium: nonnegative_counter(game, pid, "project:convert_reactor_to_uranium"),
        recommissions: nonnegative_counter(game, pid, "project:recommission_reactor"),
        accidents,
    }
}

fn terminal_power(game: &Game, pid: usize) -> (usize, usize, PlantMix) {
    let mut powered = 0;
    let mut demanding = 0;
    let mut plants = PlantMix::default();
    for city_id in game.player_city_ids(pid) {
        let city = &game.cities[&city_id];
        if game.city_power_demand(city) > f64::EPSILON {
            demanding += 1;
            powered += game.city_is_powered(city) as usize;
        }
        for building in &city.buildings {
            match building.as_str() {
                "coal_power_plant" => plants.coal += 1,
                "oil_power_plant" => plants.oil += 1,
                "nuclear_power_plant" => plants.nuclear += 1,
                _ => {}
            }
        }
    }
    (powered, demanding, plants)
}

fn science_progress(game: &Game, pid: usize) -> (usize, f64) {
    let completed = REQUIRED_SCIENCE_PROJECTS
        .iter()
        .filter(|project| {
            game.players[pid]
                .science_projects
                .iter()
                .any(|finished| finished.as_str() == **project)
        })
        .count();
    let distance = if game.players[pid]
        .science_projects
        .contains("exoplanet_expedition")
    {
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
    focal_turns: u64,
    policy_max_turns: u32,
    realized_width: i32,
    realized_height: i32,
    realized_tiles: usize,
    score: i64,
    cities: usize,
    districts: usize,
    buildings: usize,
    science_projects: usize,
    science_progress: f64,
    powered_cities: usize,
    demanding_cities: usize,
    shortage_records: i64,
    plants: PlantMix,
    conversions: ConversionStats,
    serialized_world: Option<Vec<u8>>,
}

fn play(
    options: GameOptions,
    focal: usize,
    mode: Mode,
    observe_through: u32,
    weights: &Weights,
    capture_world: bool,
    expected_geometry: Option<(i32, i32, usize)>,
) -> GameResult {
    let expected_horizon = options.max_turns;
    let mut game = Game::new_with(options);
    require_policy_horizon(&game, expected_horizon, "construction")
        .unwrap_or_else(|why| panic!("{why}"));
    if let Some(expected) = expected_geometry {
        assert_eq!(
            (game.map.width, game.map.height, game.map.tiles.len()),
            expected,
            "registered Planet geometry changed"
        );
    }
    assert!(
        observe_through >= expected_horizon,
        "external observation turn {observe_through} precedes policy horizon {expected_horizon}"
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
    if mode == Mode::Treatment {
        ais[focal].reactor_marginal = true;
    }
    let mut focal_turns = 0;

    while game.winner.is_none() && game.turn <= observe_through {
        let pid = game.current;
        focal_turns += (pid == focal) as u64;
        run_actor(&mut game, &mut ais[pid], expected_horizon).unwrap_or_else(|why| panic!("{why}"));
    }
    require_policy_horizon(&game, expected_horizon, "terminal")
        .unwrap_or_else(|why| panic!("{why}"));

    let city_ids = game.player_city_ids(focal);
    let districts = city_ids
        .iter()
        .map(|city| game.cities[city].districts.len())
        .sum();
    let buildings = city_ids
        .iter()
        .map(|city| game.cities[city].buildings.len())
        .sum();
    let (science_projects, science_progress) = science_progress(&game, focal);
    let (powered_cities, demanding_cities, plants) = terminal_power(&game, focal);
    let shortage_records = game.players[focal]
        .strategic_resource_shortages
        .values()
        .map(|value| (*value).max(0) as i64)
        .sum();
    let serialized_world = capture_world
        .then(|| serde_json::to_vec(&game).expect("terminal Game must remain serializable"));
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
        focal_turns,
        policy_max_turns: expected_horizon,
        realized_width: game.map.width,
        realized_height: game.map.height,
        realized_tiles: game.map.tiles.len(),
        score: game.score(focal),
        cities: city_ids.len(),
        districts,
        buildings,
        science_projects,
        science_progress,
        powered_cities,
        demanding_cities,
        shortage_records,
        plants,
        conversions: conversion_stats(&game, focal),
        serialized_world,
    }
}

#[derive(Clone, Debug)]
struct MapResult {
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
    focal_turns: u64,
    score: i64,
    cities: usize,
    districts: usize,
    buildings: usize,
    science_projects: usize,
    science_progress: f64,
    powered_cities: usize,
    demanding_cities: usize,
    shortage_records: i64,
    plants: PlantMix,
    conversions: ConversionStats,
    conversion_games: usize,
    victories: BTreeMap<String, usize>,
}

impl ArmSummary {
    fn record(&mut self, result: &GameResult) {
        self.games += 1;
        self.wins += result.won as usize;
        self.turns += result.reported_turn as u64;
        self.focal_turns += result.focal_turns;
        self.score += result.score;
        self.cities += result.cities;
        self.districts += result.districts;
        self.buildings += result.buildings;
        self.science_projects += result.science_projects;
        self.science_progress += result.science_progress;
        self.powered_cities += result.powered_cities;
        self.demanding_cities += result.demanding_cities;
        self.shortage_records += result.shortage_records;
        self.plants.add(result.plants);
        self.conversions.add(result.conversions);
        self.conversion_games += (result.conversions.total() > 0) as usize;
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

    fn conversion_rate(&self) -> f64 {
        100.0 * self.conversions.total() as f64 / self.focal_turns.max(1) as f64
    }

    fn powered_share(&self) -> f64 {
        if self.demanding_cities == 0 {
            1.0
        } else {
            self.powered_cities as f64 / self.demanding_cities as f64
        }
    }

    fn victory_count(&self, kind: &str) -> usize {
        self.victories.get(kind).copied().unwrap_or(0)
    }
}

#[derive(Default)]
struct StudySummary {
    maps: usize,
    control: ArmSummary,
    comparison: ArmSummary,
    score_delta: f64,
    score_favorable: usize,
    score_adverse: usize,
    win_favorable: usize,
    win_adverse: usize,
    win_score: f64,
    terminal_share: f64,
}

impl StudySummary {
    fn record(&mut self, result: &MapResult) {
        self.maps += 1;
        let control_wins = result.control.iter().filter(|game| game.won).count();
        let comparison_wins = result.comparison.iter().filter(|game| game.won).count();
        self.win_score += map_win_score(control_wins, comparison_wins);
        match comparison_wins.cmp(&control_wins) {
            std::cmp::Ordering::Greater => self.win_favorable += 1,
            std::cmp::Ordering::Less => self.win_adverse += 1,
            std::cmp::Ordering::Equal => {}
        }
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
            self.score_favorable += 1;
        } else if delta < -1e-9 {
            self.score_adverse += 1;
        }
        for (old, new) in result.control.iter().zip(&result.comparison) {
            self.control.record(old);
            self.comparison.record(new);
        }
    }
}

#[derive(Clone, Copy)]
struct GateInputs {
    games: usize,
    control_conversions: u64,
    control_conversion_games: usize,
    control_rate: f64,
    treatment_rate: f64,
    saving_per_game: f64,
    control_powered_share: f64,
    treatment_powered_share: f64,
    control_shortages: i64,
    treatment_shortages: i64,
    control_accidents: u64,
    treatment_accidents: u64,
    paired_win_score: f64,
    win_favorable: usize,
    win_adverse: usize,
    win_p: f64,
    terminal_score_share: f64,
    control_wins: usize,
    treatment_wins: usize,
    control_science_wins: usize,
    treatment_science_wins: usize,
}

fn mechanism_passes(gate: GateInputs, minimum_saving: f64) -> bool {
    gate.control_conversion_games.saturating_mul(4) >= gate.games
        && gate.treatment_rate <= gate.control_rate * 0.25 + 1e-12
        && gate.saving_per_game >= minimum_saving
}

fn screen_passes(gate: GateInputs) -> bool {
    gate.control_conversions >= 24
        && mechanism_passes(gate, 250.0)
        && gate.treatment_powered_share + 0.02 + 1e-12 >= gate.control_powered_share
        && gate.treatment_accidents <= gate.control_accidents + 2
        && gate.paired_win_score >= 0.495
        && gate.terminal_score_share >= 0.495
        && gate.treatment_wins >= gate.control_wins
        && gate.treatment_science_wins >= gate.control_science_wins
}

fn confirmation_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate, 500.0)
        && gate.treatment_powered_share + 1e-12 >= gate.control_powered_share
        && gate.treatment_shortages <= gate.control_shortages
        && gate.treatment_accidents <= gate.control_accidents
        && gate.paired_win_score >= 0.52
        && gate.win_favorable > gate.win_adverse
        && gate.win_p < 0.05
        && gate.treatment_wins >= gate.control_wins
        && gate.treatment_science_wins >= gate.control_science_wins
        && gate.terminal_score_share >= 0.50
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let raw = RawArgs::parse(&args).unwrap_or_else(|why| {
        eprintln!("{why}");
        std::process::exit(2);
    });
    let config = Config::from_raw(&raw).unwrap_or_else(|why| {
        eprintln!("{why}");
        std::process::exit(2);
    });
    let champion = frozen_champion();
    let registered_phase = if config.null {
        registered_profile(&raw, true, "4", "9975999")
    } else {
        registered_profile(&raw, false, "12", "9976000")
            || registered_profile(&raw, false, "60", "9977000")
    };
    let expected_geometry =
        registered_phase.then_some((REGISTERED_WIDTH, REGISTERED_HEIGHT, REGISTERED_TILES));
    println!("Marginal reactor-conversion evaluator");
    println!(
        "controller: {FROZEN_AI}; embedded champion generation {}; FNV-1a {:#018x}",
        champion.gen,
        fnv1a(EMBEDDED_CHAMPION.as_bytes())
    );
    println!(
        "profile: {}p requested {}x{}, {} city-states, {}, {}, {}",
        config.players,
        config.width,
        config.height,
        config.city_states,
        config.map_script.id(),
        config.map_topology.id(),
        config.map_poles.id(),
    );
    println!(
        "rules: {} policy-visible {} turns, observe through {}; civilizations {}; victories science,culture,domination",
        config.turns,
        config.speed,
        config.observe_through,
        if config.randomize_civs {
            "randomized"
        } else {
            "fixed"
        },
    );
    println!(
        "batch: {} maps x seats 0/{} x control/comparison = {} games; seed {}; {} jobs",
        config.maps,
        config.players - 1,
        config.maps * 4,
        config.seed,
        config.jobs,
    );
    println!(
        "comparison: {}",
        if config.null {
            "NULL identical controller with marginal conversion disabled"
        } else {
            "target-minus-current conversion utility; strict improvements only"
        }
    );

    let results: Vec<MapResult> = civvis::parallel::map_reporting(
        config.maps,
        config.jobs,
        |map| {
            let options = GameOptions {
                speed: config.speed.clone(),
                map_script: config.map_script,
                map_topology: config.map_topology,
                map_poles: config.map_poles,
                randomize_civs: config.randomize_civs,
                ..GameOptions::new(
                    config.players,
                    config.width,
                    config.height,
                    config.seed + map as u64,
                    config.turns,
                    config.city_states,
                )
            };
            let seats = [0, config.players - 1];
            let control = [
                play(
                    options.clone(),
                    seats[0],
                    Mode::Control,
                    config.observe_through,
                    &champion.weights,
                    config.null,
                    expected_geometry,
                ),
                play(
                    options.clone(),
                    seats[1],
                    Mode::Control,
                    config.observe_through,
                    &champion.weights,
                    config.null,
                    expected_geometry,
                ),
            ];
            let comparison_mode = if config.null {
                Mode::Null
            } else {
                Mode::Treatment
            };
            let comparison = [
                play(
                    options.clone(),
                    seats[0],
                    comparison_mode,
                    config.observe_through,
                    &champion.weights,
                    config.null,
                    expected_geometry,
                ),
                play(
                    options,
                    seats[1],
                    comparison_mode,
                    config.observe_through,
                    &champion.weights,
                    config.null,
                    expected_geometry,
                ),
            ];
            MapResult {
                control,
                comparison,
            }
        },
        |completed, _| eprintln!("progress: {}/{} maps complete", completed + 1, config.maps),
    );

    let mut summary = StudySummary::default();
    let mut exact_mismatches = 0;
    let mut helped_cells = 0;
    let mut hurt_cells = 0;
    for result in &results {
        summary.record(result);
        for (old, new) in result.control.iter().zip(&result.comparison) {
            exact_mismatches += (old != new) as usize;
            match (old.won, new.won) {
                (false, true) => helped_cells += 1,
                (true, false) => hurt_cells += 1,
                _ => {}
            }
        }
    }
    let realized = results
        .iter()
        .flat_map(|result| result.control.iter().chain(&result.comparison))
        .map(|game| {
            (
                game.realized_width,
                game.realized_height,
                game.realized_tiles,
            )
        })
        .collect::<BTreeSet<_>>();
    println!("realized geometry rows: {realized:?}");
    let maps = summary.maps.max(1) as f64;
    let paired_win_score = summary.win_score / maps;
    let terminal_score_share = summary.terminal_share / maps;
    let win_p = exact_two_sided(
        summary.win_favorable,
        summary.win_favorable + summary.win_adverse,
    );
    let score_p = exact_two_sided(
        summary.score_favorable,
        summary.score_favorable + summary.score_adverse,
    );
    let control = &summary.control;
    let comparison = &summary.comparison;
    let saving_per_game = (control.conversions.nominal_online_production()
        - comparison.conversions.nominal_online_production()) as f64
        / comparison.games.max(1) as f64;
    let gate = GateInputs {
        games: comparison.games,
        control_conversions: control.conversions.total(),
        control_conversion_games: control.conversion_games,
        control_rate: control.conversion_rate(),
        treatment_rate: comparison.conversion_rate(),
        saving_per_game,
        control_powered_share: control.powered_share(),
        treatment_powered_share: comparison.powered_share(),
        control_shortages: control.shortage_records,
        treatment_shortages: comparison.shortage_records,
        control_accidents: control.conversions.accidents,
        treatment_accidents: comparison.conversions.accidents,
        paired_win_score,
        win_favorable: summary.win_favorable,
        win_adverse: summary.win_adverse,
        win_p,
        terminal_score_share,
        control_wins: control.wins,
        treatment_wins: comparison.wins,
        control_science_wins: control.victory_count("science"),
        treatment_science_wins: comparison.victory_count("science"),
    };

    println!();
    println!("arm         wins  turns  score cities districts buildings projects science");
    for (name, arm) in [("control", control), ("comparison", comparison)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{name:<11} {:>3}/{:<3} {:>6.1} {:>6.1} {:>6.2} {:>9.2} {:>9.2} {:>8.2} {:>7.3}",
            arm.wins,
            arm.games,
            arm.turns as f64 / n,
            arm.score as f64 / n,
            arm.cities as f64 / n,
            arm.districts as f64 / n,
            arm.buildings as f64 / n,
            arm.science_projects as f64 / n,
            arm.science_progress / n,
        );
    }
    println!(
        "victory types: control {:?}; comparison {:?}",
        control.victories, comparison.victories
    );
    println!(
        "conversions coal/oil/uranium: {}/{}/{} -> {}/{}/{}; coverage {}/{} -> {}/{} games; observed focal turns {} -> {}; rates {:.3} -> {:.3} per 100 focal turns",
        control.conversions.coal,
        control.conversions.oil,
        control.conversions.uranium,
        comparison.conversions.coal,
        comparison.conversions.oil,
        comparison.conversions.uranium,
        control.conversion_games,
        control.games,
        comparison.conversion_games,
        comparison.games,
        control.focal_turns,
        comparison.focal_turns,
        control.conversion_rate(),
        comparison.conversion_rate(),
    );
    println!(
        "nominal Online conversion Production: {} -> {}; paired saving {saving_per_game:.1}/focal game; recommissions {} -> {}; accidents {} -> {}",
        control.conversions.nominal_online_production(),
        comparison.conversions.nominal_online_production(),
        control.conversions.recommissions,
        comparison.conversions.recommissions,
        control.conversions.accidents,
        comparison.conversions.accidents,
    );
    println!(
        "power: {}/{} ({:.2}%) -> {}/{} ({:.2}%); plants coal/oil/nuclear {}/{}/{} -> {}/{}/{}; shortage records {} -> {}",
        control.powered_cities,
        control.demanding_cities,
        100.0 * control.powered_share(),
        comparison.powered_cities,
        comparison.demanding_cities,
        100.0 * comparison.powered_share(),
        control.plants.coal,
        control.plants.oil,
        control.plants.nuclear,
        comparison.plants.coal,
        comparison.plants.oil,
        comparison.plants.nuclear,
        control.shortage_records,
        comparison.shortage_records,
    );
    println!(
        "matched cells: helped {helped_cells}, hurt {hurt_cells}, unchanged {} (descriptive; map is the inference unit)",
        control.games - helped_cells - hurt_cells
    );
    println!(
        "paired maps: win score {:.1}%; win F/N/A {}/{}/{} exact p={win_p:.4}; terminal-score share {:.2}%; mean score delta {:+.2}, score F/N/A {}/{}/{} exact p={score_p:.4}",
        100.0 * paired_win_score,
        summary.win_favorable,
        summary.maps - summary.win_favorable - summary.win_adverse,
        summary.win_adverse,
        100.0 * terminal_score_share,
        summary.score_delta / maps,
        summary.score_favorable,
        summary.maps - summary.score_favorable - summary.score_adverse,
        summary.score_adverse,
    );

    if config.null {
        if exact_mismatches > 0 {
            println!(
                "null sanity: BROKEN — {exact_mismatches}/{} matched focal results or serialized Games differed",
                control.games
            );
            std::process::exit(3);
        }
        if config.maps == NULL_MAPS
            && config.seed == NULL_SEED
            && registered_profile(&raw, true, "4", "9975999")
        {
            println!(
                "frozen null gate: PASS — all {} matched focal results and serialized Games reproduced exactly",
                control.games
            );
        } else {
            println!(
                "diagnostic null sanity: PASS — all {} matched focal results and serialized Games reproduced exactly",
                control.games
            );
        }
        return;
    }

    if config.maps == SCREEN_MAPS
        && config.seed == SCREEN_SEED
        && registered_profile(&raw, false, "12", "9976000")
    {
        println!(
            "development gate: {}",
            if screen_passes(gate) {
                "PASS — run only the fixed disjoint confirmation"
            } else {
                "STOP — retain AdvancedAi; do not tune, retry, or inspect confirmation"
            }
        );
    } else if config.maps == CONFIRM_MAPS
        && config.seed == CONFIRM_SEED
        && registered_profile(&raw, false, "60", "9977000")
    {
        println!(
            "confirmation gate: {}",
            if confirmation_passes(gate) {
                "PASS — a separate strategic transfer/integration PR is permitted"
            } else {
                "RETAIN AdvancedAi — no integration or rescue run"
            }
        );
    } else {
        println!("decision: DIAGNOSTIC ONLY — no preregistered gate applies");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(command: &str) -> Vec<String> {
        command.split_whitespace().map(str::to_string).collect()
    }

    fn passing_gate() -> GateInputs {
        GateInputs {
            games: 24,
            control_conversions: 24,
            control_conversion_games: 6,
            control_rate: 1.0,
            treatment_rate: 0.25,
            saving_per_game: 500.0,
            control_powered_share: 0.90,
            treatment_powered_share: 0.90,
            control_shortages: 3,
            treatment_shortages: 3,
            control_accidents: 2,
            treatment_accidents: 2,
            paired_win_score: 0.54,
            win_favorable: 8,
            win_adverse: 0,
            win_p: 0.0078125,
            terminal_score_share: 0.51,
            control_wins: 4,
            treatment_wins: 4,
            control_science_wins: 3,
            treatment_science_wins: 3,
        }
    }

    #[test]
    fn champion_and_registered_commands_are_exact() {
        let champion = frozen_champion();
        assert_eq!(champion.gen, 14);
        assert_eq!(fnv1a(EMBEDDED_CHAMPION.as_bytes()), FROZEN_CHAMPION_FNV1A);
        let null = RawArgs::parse(&strings(
            "--null --ai advanced_evolved --maps 4 --players 8 --width 84 --height 54 \
             --city-states 12 --turns 250 --observe-through 320 --speed online \
             --map continents --shape planet --poles poles --randomize-civs \
             --victories science,culture,domination --seed 9975999 --jobs 6",
        ))
        .unwrap();
        assert!(registered_profile(&null, true, "4", "9975999"));
        let screen = RawArgs::parse(&strings(
            "--ai advanced_evolved --maps 12 --players 8 --width 84 --height 54 \
             --city-states 12 --turns 250 --observe-through 320 --speed online \
             --map continents --shape planet --poles poles --randomize-civs \
             --victories science,culture,domination --seed 9976000 --jobs 6",
        ))
        .unwrap();
        assert!(registered_profile(&screen, false, "12", "9976000"));
        let mut noncanonical = screen.clone();
        noncanonical
            .values
            .insert("--turns".to_string(), "0250".to_string());
        assert!(!registered_profile(&noncanonical, false, "12", "9976000"));
    }

    #[test]
    fn parser_rejects_every_ambiguous_or_unsupported_form() {
        assert!(RawArgs::parse(&strings("--maps 1 --maps 2")).is_err());
        assert!(RawArgs::parse(&strings("--null --null")).is_err());
        assert!(RawArgs::parse(&strings("--maps")).is_err());
        assert!(RawArgs::parse(&strings("--mystery 1")).is_err());
        assert!(RawArgs::parse(&strings("positional")).is_err());
        let malformed = RawArgs::parse(&strings("--maps nope")).unwrap();
        assert!(Config::from_raw(&malformed).is_err());
        let excess = RawArgs::parse(&strings("--jobs 7")).unwrap();
        assert!(Config::from_raw(&excess).is_err());
    }

    struct Staller;

    impl Ai for Staller {
        fn take_turn(&mut self, _game: &mut Game, _pid: usize) {}
    }

    struct HorizonMutator;

    impl Ai for HorizonMutator {
        fn take_turn(&mut self, game: &mut Game, _pid: usize) {
            game.max_turns += 1;
        }
    }

    #[test]
    fn actor_boundary_falls_back_and_horizon_drift_fails_closed() {
        let mut game = Game::new(2, 20, 14, 97_591, 10, 0);
        let before = (game.turn, game.current);
        run_actor(&mut game, &mut Staller, 10).unwrap();
        assert_ne!((game.turn, game.current), before);

        let mut mutated = Game::new(2, 20, 14, 97_592, 10, 0);
        let error = run_actor(&mut mutated, &mut HorizonMutator, 10).unwrap_err();
        assert!(error.contains("policy horizon changed at after controller"));
    }

    #[test]
    fn default_off_null_reproduces_the_complete_terminal_world() {
        let weights = frozen_champion().weights;
        let options = GameOptions::new(2, 20, 14, 97_593, 1, 0);
        let control = play(options.clone(), 0, Mode::Control, 1, &weights, true, None);
        let null = play(options, 0, Mode::Null, 1, &weights, true, None);
        assert!(control.serialized_world.is_some());
        assert_eq!(control, null);
    }

    #[test]
    fn conversion_and_accident_counters_are_engine_sourced() {
        let mut game = Game::new(2, 20, 14, 97_594, 10, 0);
        game.players[0]
            .counters
            .insert("project:convert_reactor_to_coal".to_string(), 2);
        game.players[0]
            .counters
            .insert("project:convert_reactor_to_oil".to_string(), 3);
        game.players[0]
            .counters
            .insert("project:convert_reactor_to_uranium".to_string(), 4);
        game.players[0]
            .counters
            .insert("project:recommission_reactor".to_string(), 5);
        game.players[0]
            .counters
            .insert("reactor_accident:1".to_string(), 2);
        game.players[0]
            .counters
            .insert("reactor_accident:3".to_string(), 1);
        let stats = conversion_stats(&game, 0);
        assert_eq!(stats.total(), 9);
        assert_eq!(stats.nominal_online_production(), 1_450);
        assert_eq!(stats.recommissions, 5);
        assert_eq!(stats.accidents, 3);
    }

    #[test]
    fn conversion_rate_uses_exact_observed_focal_actor_turns() {
        let arm = ArmSummary {
            turns: 1_000,
            focal_turns: 40,
            conversions: ConversionStats {
                coal: 1,
                ..ConversionStats::default()
            },
            ..ArmSummary::default()
        };
        assert_eq!(arm.conversion_rate(), 2.5);
    }

    #[test]
    fn screen_gate_enforces_every_mechanism_and_safety_term() {
        let pass = passing_gate();
        assert!(screen_passes(pass));
        for broken in [
            GateInputs {
                control_conversions: 23,
                ..pass
            },
            GateInputs {
                control_conversion_games: 5,
                ..pass
            },
            GateInputs {
                treatment_rate: 0.251,
                ..pass
            },
            GateInputs {
                saving_per_game: 249.9,
                ..pass
            },
            GateInputs {
                treatment_powered_share: 0.879,
                ..pass
            },
            GateInputs {
                treatment_accidents: 5,
                ..pass
            },
            GateInputs {
                paired_win_score: 0.494,
                ..pass
            },
            GateInputs {
                terminal_score_share: 0.494,
                ..pass
            },
            GateInputs {
                treatment_wins: 3,
                ..pass
            },
            GateInputs {
                treatment_science_wins: 2,
                ..pass
            },
        ] {
            assert!(!screen_passes(broken));
        }
    }

    #[test]
    fn confirmation_gate_is_stricter_and_keeps_every_harm_guard() {
        let pass = passing_gate();
        assert!(confirmation_passes(pass));
        for broken in [
            GateInputs {
                saving_per_game: 499.9,
                ..pass
            },
            GateInputs {
                treatment_powered_share: 0.899,
                ..pass
            },
            GateInputs {
                treatment_shortages: 4,
                ..pass
            },
            GateInputs {
                treatment_accidents: 3,
                ..pass
            },
            GateInputs {
                paired_win_score: 0.519,
                ..pass
            },
            GateInputs {
                win_favorable: 4,
                win_adverse: 4,
                ..pass
            },
            GateInputs {
                win_p: 0.05,
                ..pass
            },
            GateInputs {
                terminal_score_share: 0.499,
                ..pass
            },
            GateInputs {
                treatment_wins: 3,
                ..pass
            },
            GateInputs {
                treatment_science_wins: 2,
                ..pass
            },
        ] {
            assert!(!confirmation_passes(broken));
        }
    }
}
