//! Matched evaluation of persistent Domination commitment after a real conquest.
//!
//! The control and treatment for each focal seat start from one cloned world
//! and run in lockstep until the focal civilization first keeps a city conquered
//! directly from another major. Only then may the treatment retarget the same
//! stateful generation-14 champion to Domination. This binary never changes a
//! shipped AI default.

use civvis::ai::{AdvancedAi, Ai, VictoryTarget, Weights};
use civvis::elo::builtin_ai;
use civvis::evolve::Champion;
use civvis::game::{Action, Game, GameOptions, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapTopology};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 10_700_000;
const SCREEN_MAPS: usize = 30;
const SCREEN_SEED: u64 = 10_710_000;
const HOLDOUT_MAPS: usize = 120;
const HOLDOUT_SEED: u64 = 10_720_000;
const DEPLOYMENT_TURNS: u32 = 250;
const OBSERVE_TURNS: u32 = 320;
const PLAYERS: usize = 8;
const WIDTH: i32 = 84;
const HEIGHT: i32 = 54;
const CITY_STATES: usize = 12;
const FROZEN_AI: &str = "advanced_evolved";
const FROZEN_DIFFICULTY: &str = "prince";
const TREATMENT_ID: &str = "post_conquest_domination";
const EMBEDDED_CHAMPION: &str = include_str!("../../data/evolved/best.json");
const FROZEN_CHAMPION_GENERATION: u32 = 14;
const FROZEN_CHAMPION_FNV1A: u64 = 0x40b1_fbb2_a5b8_8bc6;
const BOOTSTRAP_SAMPLES: usize = 10_000;
const BOOTSTRAP_SEED: u64 = 0x0ad0_0107;

const VALUE_FLAGS: [&str; 19] = [
    "--phase",
    "--treatment",
    "--ai",
    "--maps",
    "--players",
    "--width",
    "--height",
    "--city-states",
    "--deployment-turns",
    "--observe-turns",
    "--speed",
    "--difficulty",
    "--map",
    "--shape",
    "--poles",
    "--victories",
    "--focal-seats",
    "--seed",
    "--jobs",
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
                "data/evolved/best.json changed after the Domination controller pin"
            );
            let champion: Champion = serde_json::from_str(EMBEDDED_CHAMPION)
                .expect("the committed advanced_evolved champion must be valid JSON");
            assert_eq!(
                champion.gen, FROZEN_CHAMPION_GENERATION,
                "Domination evaluator champion generation changed"
            );
            champion.weights
        })
        .clone()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Null,
    Screen,
    Holdout,
    Diagnostic,
}

impl Phase {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "null" => Ok(Self::Null),
            "screen" => Ok(Self::Screen),
            "holdout" => Ok(Self::Holdout),
            "diagnostic" => Ok(Self::Diagnostic),
            _ => Err(format!("unknown --phase {value:?}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Treatment {
    None,
    PostConquestDomination,
}

impl Treatment {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "none" => Ok(Self::None),
            TREATMENT_ID => Ok(Self::PostConquestDomination),
            _ => Err(format!("unknown --treatment {value:?}")),
        }
    }
}

#[derive(Clone, Debug)]
struct RawArgs {
    values: BTreeMap<String, String>,
    flags: BTreeSet<String>,
}

impl RawArgs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut values = BTreeMap::new();
        let mut flags = BTreeSet::new();
        let mut index = 0;
        while index < args.len() {
            let key = &args[index];
            if key == "--randomize-civs" {
                if !flags.insert(key.clone()) {
                    return Err(format!("duplicate argument {key}"));
                }
                index += 1;
                continue;
            }
            if !VALUE_FLAGS.contains(&key.as_str()) {
                return Err(format!("unknown argument {key:?}"));
            }
            if values.contains_key(key) {
                return Err(format!("duplicate argument {key}"));
            }
            let Some(value) = args.get(index + 1) else {
                return Err(format!("{key} requires a value"));
            };
            if value.starts_with("--") {
                return Err(format!("{key} requires a value; got {value:?}"));
            }
            values.insert(key.clone(), value.clone());
            index += 2;
        }
        Ok(Self { values, flags })
    }

    fn value<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.values.get(key).map(String::as_str).unwrap_or(default)
    }

    fn integer<T>(&self, key: &str, default: T) -> Result<T, String>
    where
        T: std::str::FromStr + Copy,
    {
        let Some(raw) = self.values.get(key) else {
            return Ok(default);
        };
        raw.parse()
            .map_err(|_| format!("{key} requires a canonical integer; got {raw:?}"))
    }

    fn has_flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }
}

#[derive(Clone, Debug)]
struct Config {
    phase: Phase,
    treatment: Treatment,
    maps: usize,
    players: usize,
    width: i32,
    height: i32,
    city_states: usize,
    deployment_turns: u32,
    observe_turns: u32,
    speed: String,
    difficulty: String,
    map_script: MapScript,
    map_topology: MapTopology,
    map_poles: MapPoles,
    randomize_civs: bool,
    victories: VictoryConditions,
    focal_seats: [usize; 2],
    seed: u64,
    jobs: usize,
}

impl Config {
    fn from_raw(raw: &RawArgs) -> Result<Self, String> {
        let phase = Phase::parse(raw.value("--phase", "diagnostic"))?;
        let treatment = Treatment::parse(raw.value("--treatment", "none"))?;
        let maps = raw.integer("--maps", 1usize)?;
        let players = raw.integer("--players", PLAYERS)?;
        let width = raw.integer("--width", WIDTH)?;
        let height = raw.integer("--height", HEIGHT)?;
        let city_states = raw.integer("--city-states", CITY_STATES)?;
        let deployment_turns = raw.integer("--deployment-turns", DEPLOYMENT_TURNS)?;
        let observe_turns = raw.integer("--observe-turns", OBSERVE_TURNS)?;
        let speed = raw.value("--speed", "online").to_string();
        let difficulty = raw.value("--difficulty", FROZEN_DIFFICULTY).to_string();
        let map_name = raw.value("--map", "continents");
        let map_script = MapScript::from_id(map_name)
            .ok_or_else(|| format!("unknown map script {map_name:?}"))?;
        let shape_name = raw.value("--shape", "planet");
        let map_topology = MapTopology::from_id(shape_name)
            .ok_or_else(|| format!("unknown map shape {shape_name:?}"))?;
        let poles_name = raw.value("--poles", "poles");
        let map_poles = MapPoles::from_id(poles_name)
            .ok_or_else(|| format!("unknown map poles {poles_name:?}"))?;
        let victory_names = raw.value("--victories", "science,culture,domination");
        let victories =
            VictoryConditions::parse(victory_names).map_err(|why| format!("--victories: {why}"))?;
        let focal_raw = raw.value("--focal-seats", "0,7");
        let focal_parts = focal_raw
            .split(',')
            .map(|part| {
                part.parse::<usize>()
                    .map_err(|_| format!("invalid --focal-seats {focal_raw:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let focal_seats: [usize; 2] = focal_parts
            .try_into()
            .map_err(|_| "--focal-seats requires exactly two seats".to_string())?;
        let seed = raw.integer("--seed", 99_000_001u64)?;
        let jobs = raw.integer("--jobs", 1usize)?;

        if maps == 0 || players < 2 || width < 8 || height < 8 || jobs == 0 || jobs > 6 {
            return Err("maps/players/dimensions/jobs are outside evaluator bounds".to_string());
        }
        if observe_turns < deployment_turns {
            return Err("--observe-turns must be at least --deployment-turns".to_string());
        }
        if focal_seats[0] >= players
            || focal_seats[1] >= players
            || focal_seats[0] == focal_seats[1]
        {
            return Err("focal seats must be distinct major-seat indexes".to_string());
        }
        let rules = Rules::embedded();
        if !rules.speeds.contains_key(&speed) {
            return Err(format!("unknown game speed {speed:?}"));
        }
        if !rules.difficulties.contains_key(&difficulty) {
            return Err(format!("unknown difficulty {difficulty:?}"));
        }
        Ok(Self {
            phase,
            treatment,
            maps,
            players,
            width,
            height,
            city_states,
            deployment_turns,
            observe_turns,
            speed,
            difficulty,
            map_script,
            map_topology,
            map_poles,
            randomize_civs: raw.has_flag("--randomize-civs"),
            victories,
            focal_seats,
            seed,
            jobs,
        })
    }
}

fn registered_profile(raw: &RawArgs, phase: Phase) -> bool {
    let (phase_name, treatment, maps, seed) = match phase {
        Phase::Null => ("null", "none", "4", "10700000"),
        Phase::Screen => ("screen", TREATMENT_ID, "30", "10710000"),
        Phase::Holdout => ("holdout", TREATMENT_ID, "120", "10720000"),
        Phase::Diagnostic => return false,
    };
    let expected = BTreeMap::from([
        ("--phase".to_string(), phase_name.to_string()),
        ("--treatment".to_string(), treatment.to_string()),
        ("--ai".to_string(), FROZEN_AI.to_string()),
        ("--maps".to_string(), maps.to_string()),
        ("--players".to_string(), "8".to_string()),
        ("--width".to_string(), "84".to_string()),
        ("--height".to_string(), "54".to_string()),
        ("--city-states".to_string(), "12".to_string()),
        ("--deployment-turns".to_string(), "250".to_string()),
        ("--observe-turns".to_string(), "320".to_string()),
        ("--speed".to_string(), "online".to_string()),
        ("--difficulty".to_string(), FROZEN_DIFFICULTY.to_string()),
        ("--map".to_string(), "continents".to_string()),
        ("--shape".to_string(), "planet".to_string()),
        ("--poles".to_string(), "poles".to_string()),
        (
            "--victories".to_string(),
            "science,culture,domination".to_string(),
        ),
        ("--focal-seats".to_string(), "0,7".to_string()),
        ("--seed".to_string(), seed.to_string()),
        ("--jobs".to_string(), "6".to_string()),
    ]);
    raw.values == expected && raw.flags == BTreeSet::from(["--randomize-civs".to_string()])
}

fn is_major(g: &Game, pid: usize) -> bool {
    g.players
        .get(pid)
        .is_some_and(|player| !player.is_minor && !player.is_barbarian && !player.is_free_city)
}

fn qualifying_capture(g: &Game, focal: usize) -> Option<(u32, usize)> {
    g.cities
        .values()
        .filter(|city| city.owner == focal && city.original_owner != focal)
        .filter_map(|city| {
            let conquered = city.occupied_from?;
            is_major(g, conquered).then_some((city.id, conquered))
        })
        .min()
}

fn foreign_capitals(g: &Game, focal: usize) -> usize {
    g.cities
        .values()
        .filter(|city| city.is_capital && city.original_owner != focal && city.owner == focal)
        .count()
}

fn owns_original_capital(g: &Game, focal: usize) -> bool {
    g.cities
        .values()
        .any(|city| city.is_capital && city.original_owner == focal && city.owner == focal)
}

fn capture_counter(g: &Game, focal: usize) -> i64 {
    g.players[focal]
        .counters
        .get("captures")
        .copied()
        .unwrap_or(0)
}

fn wars_declared_since(g: &Game, focal: usize, since: u32) -> usize {
    g.wars
        .values()
        .chain(g.concluded_wars.iter())
        .filter(|war| war.declarer == focal && war.started >= since)
        .count()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Trigger {
    turn: u32,
    city: u32,
    conquered_from: usize,
    foreign_capitals: usize,
    captures_before: i64,
    world_fnv1a: u64,
    prior_plan: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Observer {
    trigger: Option<Trigger>,
    retargets: usize,
    peak_foreign_capitals: usize,
    post_trigger_peak_foreign_capitals: usize,
}

impl Observer {
    fn observe(&mut self, g: &Game, focal: usize) {
        let capitals = foreign_capitals(g, focal);
        self.peak_foreign_capitals = self.peak_foreign_capitals.max(capitals);
        if self.trigger.is_some() {
            self.post_trigger_peak_foreign_capitals =
                self.post_trigger_peak_foreign_capitals.max(capitals);
        }
    }

    fn maybe_trigger(
        &mut self,
        g: &Game,
        focal: usize,
        ai: &mut AdvancedAi,
        retarget: bool,
    ) -> Result<bool, String> {
        if self.trigger.is_some() || !g.victory_conditions.domination {
            return Ok(false);
        }
        let Some((city, conquered_from)) = qualifying_capture(g, focal) else {
            return Ok(false);
        };
        let serialized = serde_json::to_string(g)
            .map_err(|why| format!("failed to serialize trigger world: {why}"))?;
        self.trigger = Some(Trigger {
            turn: g.turn,
            city,
            conquered_from,
            foreign_capitals: foreign_capitals(g, focal),
            captures_before: capture_counter(g, focal),
            world_fnv1a: fnv1a(serialized.as_bytes()),
            prior_plan: format!("{:?}", ai.plan_report()),
        });
        self.post_trigger_peak_foreign_capitals = foreign_capitals(g, focal);
        if retarget {
            ai.retarget(VictoryTarget::Domination);
            self.retargets += 1;
        }
        Ok(true)
    }
}

fn pinned_advanced() -> AdvancedAi {
    AdvancedAi::with_weights(frozen_champion_weights())
}

fn support_fleet(g: &Game, focal: usize, seed: u64) -> Vec<Box<dyn Ai>> {
    g.players
        .iter()
        .map(|player| {
            if player.id == focal || player.is_minor || player.is_barbarian || player.is_free_city {
                builtin_ai("basic", seed.wrapping_add(player.id as u64))
            } else {
                Box::new(pinned_advanced()) as Box<dyn Ai>
            }
        })
        .collect()
}

fn advance_one(
    game: &mut Game,
    focal: usize,
    focal_ai: &mut AdvancedAi,
    support: &mut [Box<dyn Ai>],
    observer: &mut Observer,
    retarget: bool,
) -> Result<bool, String> {
    if game.winner.is_some() {
        return Ok(false);
    }
    let pid = game.current;
    observer.observe(game, focal);
    let triggered = if pid == focal {
        observer.maybe_trigger(game, focal, focal_ai, retarget)?
    } else {
        false
    };
    if pid == focal {
        focal_ai.take_turn(game, pid);
    } else {
        support[pid].take_turn(game, pid);
    }
    if game.winner.is_none() && game.current == pid {
        let _ = game.apply(pid, &Action::EndTurn);
    }
    observer.observe(game, focal);
    Ok(triggered)
}

fn finish_arm(
    game: &mut Game,
    focal: usize,
    focal_ai: &mut AdvancedAi,
    support: &mut [Box<dyn Ai>],
    observer: &mut Observer,
    retarget: bool,
    observe_turns: u32,
) -> Result<(), String> {
    while game.winner.is_none() && game.turn <= observe_turns {
        advance_one(game, focal, focal_ai, support, observer, retarget)?;
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct GameResult {
    eligible: bool,
    trigger: Option<Trigger>,
    retargets: usize,
    peak_foreign_capitals: usize,
    terminal_foreign_capitals: usize,
    post_trigger_capital_gain: usize,
    reached_two: bool,
    reached_three: bool,
    captures_after_trigger: i64,
    wars_after_trigger: usize,
    survived: bool,
    retained_own_capital: bool,
    won: bool,
    victory: Option<String>,
    finish_turn: u32,
    score: i64,
    field_score: i64,
    utility: f64,
}

fn result_from(game: &Game, focal: usize, observer: Observer, observe_turns: u32) -> GameResult {
    let field_score = game
        .players
        .iter()
        .filter(|player| is_major(game, player.id))
        .map(|player| game.score(player.id).max(0))
        .sum::<i64>()
        .max(1);
    let score = game.score(focal).max(0);
    let won = game.winner == Some(focal);
    let baseline = observer
        .trigger
        .as_ref()
        .map(|trigger| trigger.foreign_capitals)
        .unwrap_or(0);
    let post_trigger_capital_gain = observer
        .post_trigger_peak_foreign_capitals
        .saturating_sub(baseline);
    let captures_after_trigger = observer
        .trigger
        .as_ref()
        .map(|trigger| capture_counter(game, focal) - trigger.captures_before)
        .unwrap_or(0)
        .max(0);
    let wars_after_trigger = observer
        .trigger
        .as_ref()
        .map(|trigger| wars_declared_since(game, focal, trigger.turn))
        .unwrap_or(0);
    GameResult {
        eligible: observer.trigger.is_some(),
        trigger: observer.trigger,
        retargets: observer.retargets,
        peak_foreign_capitals: observer.peak_foreign_capitals,
        terminal_foreign_capitals: foreign_capitals(game, focal),
        post_trigger_capital_gain,
        reached_two: observer.peak_foreign_capitals >= 2,
        reached_three: observer.peak_foreign_capitals >= 3,
        captures_after_trigger,
        wars_after_trigger,
        survived: game.players[focal].alive,
        retained_own_capital: owns_original_capital(game, focal),
        won,
        victory: won.then(|| game.victory_type.clone()).flatten(),
        finish_turn: game.winner.map_or(observe_turns, |_| game.reported_turn()),
        score,
        field_score,
        utility: 0.80 * f64::from(won) + 0.20 * score as f64 / field_score as f64,
    }
}

#[derive(Clone, Debug)]
struct CellResult {
    exact_prefix: bool,
    exact_terminal: bool,
    control: GameResult,
    treatment: GameResult,
}

fn play_pair(
    options: GameOptions,
    focal: usize,
    treatment: Treatment,
    victories: VictoryConditions,
    observe_turns: u32,
) -> Result<CellResult, String> {
    let mut control_game = Game::new_with(options.clone());
    control_game.victory_conditions = victories.clone();
    let mut treatment_game = control_game.clone();
    let mut control_ai = pinned_advanced();
    let mut treatment_ai = pinned_advanced();
    let mut control_support = support_fleet(&control_game, focal, options.seed);
    let mut treatment_support = support_fleet(&treatment_game, focal, options.seed);
    let mut control_observer = Observer::default();
    let mut treatment_observer = Observer::default();
    control_observer.observe(&control_game, focal);
    treatment_observer.observe(&treatment_game, focal);
    let mut exact_prefix = true;
    let mut treatment_started = false;

    while control_game.winner.is_none()
        && treatment_game.winner.is_none()
        && control_game.turn <= observe_turns
        && treatment_game.turn <= observe_turns
        && !treatment_started
    {
        let control_json = serde_json::to_string(&control_game)
            .map_err(|why| format!("failed to serialize control world: {why}"))?;
        let treatment_json = serde_json::to_string(&treatment_game)
            .map_err(|why| format!("failed to serialize treatment world: {why}"))?;
        exact_prefix &= control_json == treatment_json;
        if !exact_prefix {
            return Err("arms diverged before the treatment trigger".to_string());
        }
        let control_triggered = advance_one(
            &mut control_game,
            focal,
            &mut control_ai,
            &mut control_support,
            &mut control_observer,
            false,
        )?;
        let treatment_triggered = advance_one(
            &mut treatment_game,
            focal,
            &mut treatment_ai,
            &mut treatment_support,
            &mut treatment_observer,
            treatment == Treatment::PostConquestDomination,
        )?;
        if control_triggered != treatment_triggered
            || control_observer.trigger != treatment_observer.trigger
        {
            return Err("arms disagreed on the shared conquest trigger".to_string());
        }
        treatment_started = treatment_triggered && treatment == Treatment::PostConquestDomination;
        if !treatment_started {
            let control_json = serde_json::to_string(&control_game)
                .map_err(|why| format!("failed to serialize control world: {why}"))?;
            let treatment_json = serde_json::to_string(&treatment_game)
                .map_err(|why| format!("failed to serialize treatment world: {why}"))?;
            exact_prefix &= control_json == treatment_json;
            if !exact_prefix {
                return Err("default-off arms diverged after an actor".to_string());
            }
        }
    }

    finish_arm(
        &mut control_game,
        focal,
        &mut control_ai,
        &mut control_support,
        &mut control_observer,
        false,
        observe_turns,
    )?;
    finish_arm(
        &mut treatment_game,
        focal,
        &mut treatment_ai,
        &mut treatment_support,
        &mut treatment_observer,
        treatment == Treatment::PostConquestDomination,
        observe_turns,
    )?;
    let exact_terminal = serde_json::to_string(&control_game)
        .map_err(|why| format!("failed to serialize terminal control: {why}"))?
        == serde_json::to_string(&treatment_game)
            .map_err(|why| format!("failed to serialize terminal treatment: {why}"))?;
    Ok(CellResult {
        exact_prefix,
        exact_terminal,
        control: result_from(&control_game, focal, control_observer, observe_turns),
        treatment: result_from(&treatment_game, focal, treatment_observer, observe_turns),
    })
}

#[derive(Clone, Debug)]
struct MapResult {
    cells: [CellResult; 2],
}

impl MapResult {
    fn eligible_cells(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| cell.control.eligible && cell.treatment.eligible)
            .count()
    }

    fn capital_gain_difference(&self) -> f64 {
        let eligible = self.eligible_cells();
        if eligible == 0 {
            return 0.0;
        }
        self.cells
            .iter()
            .filter(|cell| cell.control.eligible && cell.treatment.eligible)
            .map(|cell| {
                cell.treatment.post_trigger_capital_gain as f64
                    - cell.control.post_trigger_capital_gain as f64
            })
            .sum::<f64>()
            / eligible as f64
    }

    fn utility_pair(&self) -> (f64, f64) {
        (
            self.cells.iter().map(|cell| cell.control.utility).sum(),
            self.cells.iter().map(|cell| cell.treatment.utility).sum(),
        )
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

#[derive(Clone, Copy)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn index(&mut self, length: usize) -> usize {
        (self.next() as usize) % length
    }
}

fn percentile_interval(mut samples: Vec<f64>) -> (f64, f64) {
    if samples.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    samples.sort_by(f64::total_cmp);
    let last = samples.len() - 1;
    (samples[last * 25 / 1000], samples[last * 975 / 1000])
}

fn bootstrap_capital_difference(results: &[MapResult]) -> (f64, f64) {
    if results.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let mut rng = SplitMix64(BOOTSTRAP_SEED);
    let mut samples = Vec::with_capacity(BOOTSTRAP_SAMPLES);
    for _ in 0..BOOTSTRAP_SAMPLES {
        let mut difference = 0.0;
        let mut eligible_maps = 0usize;
        for _ in 0..results.len() {
            let result = &results[rng.index(results.len())];
            if result.eligible_cells() > 0 {
                difference += result.capital_gain_difference();
                eligible_maps += 1;
            }
        }
        if eligible_maps > 0 {
            samples.push(difference / eligible_maps as f64);
        }
    }
    percentile_interval(samples)
}

fn bootstrap_strength_share(results: &[MapResult]) -> (f64, f64) {
    if results.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let mut rng = SplitMix64(BOOTSTRAP_SEED ^ 0x5354_5245_4e47_5448);
    let mut samples = Vec::with_capacity(BOOTSTRAP_SAMPLES);
    for _ in 0..BOOTSTRAP_SAMPLES {
        let mut control = 0.0;
        let mut treatment = 0.0;
        for _ in 0..results.len() {
            let (old, new) = results[rng.index(results.len())].utility_pair();
            control += old;
            treatment += new;
        }
        let total = control + treatment;
        samples.push(if total > 0.0 { treatment / total } else { 0.5 });
    }
    percentile_interval(samples)
}

#[derive(Clone, Copy, Debug)]
struct GateFacts {
    cells: usize,
    eligible: usize,
    valid_prefixes: usize,
    exact_once: usize,
    control_capital_gains: usize,
    treatment_capital_gains: usize,
    control_multi_two: usize,
    treatment_multi_two: usize,
    capital_mean_difference: f64,
    capital_ci_low: f64,
    capital_favorable: usize,
    capital_adverse: usize,
    capital_sign_p: f64,
    strength_share: f64,
    strength_ci_low: f64,
    strength_favorable: usize,
    strength_adverse: usize,
    score_share: f64,
    control_wins: usize,
    treatment_wins: usize,
    control_survived: usize,
    treatment_survived: usize,
    control_own_capital: usize,
    treatment_own_capital: usize,
}

fn screen_passes(facts: GateFacts) -> bool {
    facts.cells == 60
        && facts.eligible >= 18
        && facts.valid_prefixes == facts.cells
        && facts.exact_once == facts.eligible
        && facts.treatment_capital_gains >= facts.control_capital_gains + 4
        && facts.treatment_multi_two >= facts.control_multi_two + 2
        && facts.strength_share + 1e-12 >= 0.52
        && facts.strength_favorable > facts.strength_adverse
        && facts.score_share + 1e-12 >= 0.50
        && facts.treatment_survived + 2 >= facts.control_survived
        && facts.treatment_own_capital + 2 >= facts.control_own_capital
}

fn holdout_passes(facts: GateFacts) -> bool {
    let control_multi_share = facts.control_multi_two as f64 / facts.eligible.max(1) as f64;
    let treatment_multi_share = facts.treatment_multi_two as f64 / facts.eligible.max(1) as f64;
    facts.cells == 240
        && facts.eligible >= 60
        && facts.valid_prefixes == facts.cells
        && facts.exact_once == facts.eligible
        && facts.capital_mean_difference + 1e-12 >= 0.15
        && facts.capital_ci_low > 0.0
        && facts.capital_favorable > facts.capital_adverse
        && facts.capital_sign_p < 0.05
        && treatment_multi_share + 1e-12 >= control_multi_share + 0.10
        && facts.strength_share + 1e-12 >= 0.52
        && facts.strength_ci_low > 0.50
        && facts.treatment_wins >= facts.control_wins
        && facts.score_share + 1e-12 >= 0.50
        && facts.strength_favorable > facts.strength_adverse
        && facts.treatment_survived as f64 / facts.cells as f64 + 0.05 + 1e-12
            >= facts.control_survived as f64 / facts.cells as f64
        && facts.treatment_own_capital as f64 / facts.cells as f64 + 0.05 + 1e-12
            >= facts.control_own_capital as f64 / facts.cells as f64
}

fn summarize(results: &[MapResult]) -> GateFacts {
    let cells = results.len() * 2;
    let mut eligible = 0;
    let mut valid_prefixes = 0;
    let mut exact_once = 0;
    let mut control_capital_gains = 0;
    let mut treatment_capital_gains = 0;
    let mut control_multi_two = 0;
    let mut treatment_multi_two = 0;
    let mut control_utility = 0.0;
    let mut treatment_utility = 0.0;
    let mut strength_favorable = 0;
    let mut strength_adverse = 0;
    let mut score_shares = 0.0;
    let mut control_wins = 0;
    let mut treatment_wins = 0;
    let mut control_survived = 0;
    let mut treatment_survived = 0;
    let mut control_own_capital = 0;
    let mut treatment_own_capital = 0;
    let mut capital_favorable = 0;
    let mut capital_adverse = 0;

    for result in results {
        let (old_utility, new_utility) = result.utility_pair();
        control_utility += old_utility;
        treatment_utility += new_utility;
        if new_utility > old_utility + 1e-12 {
            strength_favorable += 1;
        } else if new_utility + 1e-12 < old_utility {
            strength_adverse += 1;
        }
        let capital_difference = result.capital_gain_difference();
        if result.eligible_cells() > 0 {
            if capital_difference > 1e-12 {
                capital_favorable += 1;
            } else if capital_difference < -1e-12 {
                capital_adverse += 1;
            }
        }
        for cell in &result.cells {
            valid_prefixes += cell.exact_prefix as usize;
            let shared_eligible = cell.control.eligible && cell.treatment.eligible;
            eligible += shared_eligible as usize;
            exact_once += (shared_eligible
                && cell.control.retargets == 0
                && cell.treatment.retargets == 1) as usize;
            if shared_eligible {
                control_capital_gains += cell.control.post_trigger_capital_gain;
                treatment_capital_gains += cell.treatment.post_trigger_capital_gain;
                control_multi_two += cell.control.reached_two as usize;
                treatment_multi_two += cell.treatment.reached_two as usize;
            }
            let old_score = cell.control.score.max(0) as f64;
            let new_score = cell.treatment.score.max(0) as f64;
            score_shares += if old_score + new_score > 0.0 {
                new_score / (old_score + new_score)
            } else {
                0.5
            };
            control_wins += cell.control.won as usize;
            treatment_wins += cell.treatment.won as usize;
            control_survived += cell.control.survived as usize;
            treatment_survived += cell.treatment.survived as usize;
            control_own_capital += cell.control.retained_own_capital as usize;
            treatment_own_capital += cell.treatment.retained_own_capital as usize;
        }
    }
    let capital_map_differences = results
        .iter()
        .filter(|result| result.eligible_cells() > 0)
        .map(MapResult::capital_gain_difference)
        .collect::<Vec<_>>();
    let capital_mean_difference = if capital_map_differences.is_empty() {
        0.0
    } else {
        capital_map_differences.iter().sum::<f64>() / capital_map_differences.len() as f64
    };
    let (capital_ci_low, _) = bootstrap_capital_difference(results);
    let (strength_ci_low, _) = bootstrap_strength_share(results);
    let utility_total = control_utility + treatment_utility;
    GateFacts {
        cells,
        eligible,
        valid_prefixes,
        exact_once,
        control_capital_gains,
        treatment_capital_gains,
        control_multi_two,
        treatment_multi_two,
        capital_mean_difference,
        capital_ci_low,
        capital_favorable,
        capital_adverse,
        capital_sign_p: exact_two_sided(capital_favorable, capital_favorable + capital_adverse),
        strength_share: if utility_total > 0.0 {
            treatment_utility / utility_total
        } else {
            0.5
        },
        strength_ci_low,
        strength_favorable,
        strength_adverse,
        score_share: score_shares / cells.max(1) as f64,
        control_wins,
        treatment_wins,
        control_survived,
        treatment_survived,
        control_own_capital,
        treatment_own_capital,
    }
}

fn print_summary(results: &[MapResult], facts: GateFacts) {
    let mut control_terminal_capitals = 0;
    let mut treatment_terminal_capitals = 0;
    let mut control_peak_capitals = 0;
    let mut treatment_peak_capitals = 0;
    let mut control_reached_three = 0;
    let mut treatment_reached_three = 0;
    let mut control_captures = 0;
    let mut treatment_captures = 0;
    let mut control_wars = 0;
    let mut treatment_wars = 0;
    let mut control_victories = BTreeMap::<String, usize>::new();
    let mut treatment_victories = BTreeMap::<String, usize>::new();
    let mut trigger_turns = Vec::new();
    for result in results {
        for cell in &result.cells {
            control_terminal_capitals += cell.control.terminal_foreign_capitals;
            treatment_terminal_capitals += cell.treatment.terminal_foreign_capitals;
            control_peak_capitals += cell.control.peak_foreign_capitals;
            treatment_peak_capitals += cell.treatment.peak_foreign_capitals;
            control_reached_three += cell.control.reached_three as usize;
            treatment_reached_three += cell.treatment.reached_three as usize;
            control_captures += cell.control.captures_after_trigger;
            treatment_captures += cell.treatment.captures_after_trigger;
            control_wars += cell.control.wars_after_trigger;
            treatment_wars += cell.treatment.wars_after_trigger;
            if let Some(trigger) = &cell.treatment.trigger {
                trigger_turns.push(trigger.turn);
            }
            if cell.control.won {
                *control_victories
                    .entry(
                        cell.control
                            .victory
                            .clone()
                            .unwrap_or_else(|| "unknown".into()),
                    )
                    .or_default() += 1;
            }
            if cell.treatment.won {
                *treatment_victories
                    .entry(
                        cell.treatment
                            .victory
                            .clone()
                            .unwrap_or_else(|| "unknown".into()),
                    )
                    .or_default() += 1;
            }
        }
    }
    trigger_turns.sort_unstable();
    let trigger_median = if trigger_turns.is_empty() {
        None
    } else {
        Some(trigger_turns[trigger_turns.len() / 2])
    };
    let (_, capital_ci_high) = bootstrap_capital_difference(results);
    let (_, strength_ci_high) = bootstrap_strength_share(results);
    println!(
        "validity: {}/{} exact prefixes; {}/{} eligible cells retargeted exactly once; median trigger {:?}",
        facts.valid_prefixes, facts.cells, facts.exact_once, facts.eligible, trigger_median
    );
    println!(
        "capital mechanism: post-trigger gains {} -> {} (paired mean {:+.3}, map bootstrap 95% [{:+.3}, {:+.3}]); >=2 caps {} -> {}; >=3 {} -> {}",
        facts.control_capital_gains,
        facts.treatment_capital_gains,
        facts.capital_mean_difference,
        facts.capital_ci_low,
        capital_ci_high,
        facts.control_multi_two,
        facts.treatment_multi_two,
        control_reached_three,
        treatment_reached_three,
    );
    println!(
        "capital directions: favorable {}, neutral {}, adverse {}; exact two-sided p={:.4}",
        facts.capital_favorable,
        results.len() - facts.capital_favorable - facts.capital_adverse,
        facts.capital_adverse,
        facts.capital_sign_p,
    );
    println!(
        "peak/terminal foreign capitals total: control {control_peak_capitals}/{control_terminal_capitals}; treatment {treatment_peak_capitals}/{treatment_terminal_capitals}"
    );
    println!(
        "post-trigger captures/wars: control {control_captures}/{control_wars}; treatment {treatment_captures}/{treatment_wars}"
    );
    println!(
        "strength: paired 80/20 share {:.1}% (map bootstrap 95% [{:.1}, {:.1}]); directions {}-{}; wins {} -> {}; score share {:.1}%",
        100.0 * facts.strength_share,
        100.0 * facts.strength_ci_low,
        100.0 * strength_ci_high,
        facts.strength_favorable,
        facts.strength_adverse,
        facts.control_wins,
        facts.treatment_wins,
        100.0 * facts.score_share,
    );
    println!(
        "victories: control {:?}; treatment {:?}",
        control_victories, treatment_victories
    );
    println!(
        "survival/own capital: control {}/{}; treatment {}/{} ({} cells)",
        facts.control_survived,
        facts.control_own_capital,
        facts.treatment_survived,
        facts.treatment_own_capital,
        facts.cells,
    );
}

fn print_raw_rows(results: &[MapResult], config: &Config) {
    for (map, result) in results.iter().enumerate() {
        for (seat_index, cell) in result.cells.iter().enumerate() {
            for (arm, outcome) in [("control", &cell.control), ("treatment", &cell.treatment)] {
                let trigger = outcome.trigger.as_ref();
                let row = serde_json::json!({
                    "arm": arm,
                    "captures_after_trigger": outcome.captures_after_trigger,
                    "eligible": outcome.eligible,
                    "exact_prefix": cell.exact_prefix,
                    "exact_terminal": cell.exact_terminal,
                    "field_score": outcome.field_score,
                    "finish_turn": outcome.finish_turn,
                    "focal_seat": config.focal_seats[seat_index],
                    "map_seed": config.seed + map as u64,
                    "peak_foreign_capitals": outcome.peak_foreign_capitals,
                    "post_trigger_capital_gain": outcome.post_trigger_capital_gain,
                    "reached_three": outcome.reached_three,
                    "reached_two": outcome.reached_two,
                    "retained_own_capital": outcome.retained_own_capital,
                    "retargets": outcome.retargets,
                    "score": outcome.score,
                    "survived": outcome.survived,
                    "terminal_foreign_capitals": outcome.terminal_foreign_capitals,
                    "trigger_captures_before": trigger.map(|value| value.captures_before),
                    "trigger_city": trigger.map(|value| value.city),
                    "trigger_conquered_from": trigger.map(|value| value.conquered_from),
                    "trigger_foreign_capitals": trigger.map(|value| value.foreign_capitals),
                    "trigger_prior_plan": trigger.map(|value| value.prior_plan.as_str()),
                    "trigger_turn": trigger.map(|value| value.turn),
                    "trigger_world_fnv1a": trigger.map(|value| format!("{:016x}", value.world_fnv1a)),
                    "utility": outcome.utility,
                    "victory": outcome.victory,
                    "wars_after_trigger": outcome.wars_after_trigger,
                    "won": outcome.won,
                });
                println!("raw {row}");
            }
        }
    }
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let raw = RawArgs::parse(&args).unwrap_or_else(|why| {
        eprintln!("{why}");
        std::process::exit(2);
    });
    let config = Config::from_raw(&raw).unwrap_or_else(|why| {
        eprintln!("{why}");
        std::process::exit(2);
    });
    if raw.value("--ai", FROZEN_AI) != FROZEN_AI {
        eprintln!("this evaluator is frozen for --ai {FROZEN_AI}");
        std::process::exit(2);
    }
    let expected_victories =
        VictoryConditions::parse("science,culture,domination").expect("frozen victory set");
    if config.victories != expected_victories {
        eprintln!("this evaluator is frozen for science,culture,domination");
        std::process::exit(2);
    }
    let _ = frozen_champion_weights();
    println!(
        "Post-conquest Domination evaluator; embedded advanced_evolved generation {}, fnv1a:{:016x}",
        FROZEN_CHAMPION_GENERATION, FROZEN_CHAMPION_FNV1A
    );
    println!(
        "profile: {} maps x seats {},{} x matched arms = {} games; {}p {}x{}+{}cs; {} / {} / {}; nominal {} {}, observe through {}; seed {}; jobs {}; treatment {:?}",
        config.maps,
        config.focal_seats[0],
        config.focal_seats[1],
        config.maps * 4,
        config.players,
        config.width,
        config.height,
        config.city_states,
        config.map_script.id(),
        config.map_topology.id(),
        config.map_poles.id(),
        config.deployment_turns,
        config.speed,
        config.observe_turns,
        config.seed,
        config.jobs,
        config.treatment,
    );
    println!(
        "civilizations: {}; victories science,culture,domination; difficulty {}",
        if config.randomize_civs {
            "randomized"
        } else {
            "fixed"
        },
        config.difficulty,
    );

    let results = civvis::parallel::map_reporting(
        config.maps,
        config.jobs,
        |map| {
            let map_seed = config.seed + map as u64;
            let mut options = GameOptions::new(
                config.players,
                config.width,
                config.height,
                map_seed,
                config.deployment_turns,
                config.city_states,
            );
            options.speed = config.speed.clone();
            options.difficulty = config.difficulty.clone();
            options.map_script = config.map_script;
            options.map_topology = config.map_topology;
            options.map_poles = config.map_poles;
            options.randomize_civs = config.randomize_civs;
            let first = play_pair(
                options.clone(),
                config.focal_seats[0],
                config.treatment,
                config.victories.clone(),
                config.observe_turns,
            )
            .unwrap_or_else(|why| panic!("map {map_seed}, seat {}: {why}", config.focal_seats[0]));
            let second = play_pair(
                options,
                config.focal_seats[1],
                config.treatment,
                config.victories.clone(),
                config.observe_turns,
            )
            .unwrap_or_else(|why| panic!("map {map_seed}, seat {}: {why}", config.focal_seats[1]));
            MapResult {
                cells: [first, second],
            }
        },
        |completed, _| eprintln!("progress: {}/{} maps complete", completed + 1, config.maps),
    );
    let facts = summarize(&results);
    print_raw_rows(&results, &config);
    print_summary(&results, facts);

    let registered = registered_profile(&raw, config.phase);
    match config.phase {
        Phase::Null => {
            let identical = results.iter().flat_map(|result| &result.cells).all(|cell| {
                cell.exact_prefix
                    && cell.exact_terminal
                    && cell.control == cell.treatment
                    && cell.control.retargets == 0
            });
            if !identical {
                println!("serialized null: BROKEN — default-off arms differ");
                std::process::exit(3);
            }
            if registered && config.maps == NULL_MAPS && config.seed == NULL_SEED {
                println!("frozen serialized null: PASS — all 8 focal cells are exact");
            } else {
                println!("diagnostic serialized null: PASS — no registered gate spent");
            }
        }
        Phase::Screen if registered && config.maps == SCREEN_MAPS && config.seed == SCREEN_SEED => {
            println!(
                "screen gate: {}",
                if screen_passes(facts) {
                    "PASS — only the fixed seed-10720000 holdout is permitted"
                } else if facts.eligible < 18 {
                    "INERT — retain adaptive champion; do not tune or retry"
                } else {
                    "REJECT — retain adaptive champion; do not tune or inspect holdout"
                }
            );
        }
        Phase::Holdout
            if registered && config.maps == HOLDOUT_MAPS && config.seed == HOLDOUT_SEED =>
        {
            println!(
                "holdout gate: {}",
                if holdout_passes(facts) {
                    "PASS — a separate gameplay-integration PR is permitted"
                } else {
                    "RETAIN adaptive champion — no integration or rescue run"
                }
            );
        }
        _ => println!("decision: DIAGNOSTIC ONLY — no preregistered gate applies"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(command: &str) -> Vec<String> {
        command.split_whitespace().map(str::to_string).collect()
    }

    fn found_game() -> Game {
        let mut game = Game::new_full(3, 30, 18, 71_901, 100, 1, false);
        for pid in 0..3 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        game.current = 0;
        game
    }

    fn passing_facts(cells: usize) -> GateFacts {
        GateFacts {
            cells,
            eligible: if cells == 60 { 20 } else { 80 },
            valid_prefixes: cells,
            exact_once: if cells == 60 { 20 } else { 80 },
            control_capital_gains: 10,
            treatment_capital_gains: 25,
            control_multi_two: 5,
            treatment_multi_two: 15,
            capital_mean_difference: 0.20,
            capital_ci_low: 0.02,
            capital_favorable: 30,
            capital_adverse: 10,
            capital_sign_p: 0.004,
            strength_share: 0.53,
            strength_ci_low: 0.501,
            strength_favorable: 30,
            strength_adverse: 10,
            score_share: 0.51,
            control_wins: 10,
            treatment_wins: 12,
            control_survived: cells - 2,
            treatment_survived: cells - 3,
            control_own_capital: cells - 2,
            treatment_own_capital: cells - 3,
        }
    }

    fn fixture_result(eligible: bool, gain: usize, utility: f64, score: i64) -> GameResult {
        GameResult {
            eligible,
            trigger: eligible.then_some(Trigger {
                turn: 50,
                city: 1,
                conquered_from: 1,
                foreign_capitals: 0,
                captures_before: 1,
                world_fnv1a: 7,
                prior_plan: "adaptive".into(),
            }),
            retargets: 0,
            peak_foreign_capitals: gain,
            terminal_foreign_capitals: gain,
            post_trigger_capital_gain: gain,
            reached_two: gain >= 2,
            reached_three: gain >= 3,
            captures_after_trigger: gain as i64,
            wars_after_trigger: gain,
            survived: true,
            retained_own_capital: true,
            won: false,
            victory: None,
            finish_turn: 100,
            score,
            field_score: 1_000,
            utility,
        }
    }

    fn fixture_cell(control_gain: usize, treatment_gain: usize) -> CellResult {
        let mut treatment = fixture_result(true, treatment_gain, 0.12, 600);
        treatment.retargets = 1;
        CellResult {
            exact_prefix: true,
            exact_terminal: false,
            control: fixture_result(true, control_gain, 0.10, 500),
            treatment,
        }
    }

    #[test]
    fn controller_and_registered_commands_are_pinned() {
        assert_eq!(fnv1a(EMBEDDED_CHAMPION.as_bytes()), FROZEN_CHAMPION_FNV1A);
        let champion: Champion = serde_json::from_str(EMBEDDED_CHAMPION).unwrap();
        assert_eq!(champion.gen, FROZEN_CHAMPION_GENERATION);
        let _ = frozen_champion_weights();

        let command = strings(
            "--phase null --treatment none --ai advanced_evolved --maps 4 \
             --players 8 --width 84 --height 54 --city-states 12 \
             --deployment-turns 250 --observe-turns 320 --speed online \
             --difficulty prince \
             --map continents --shape planet --poles poles --randomize-civs \
             --victories science,culture,domination --focal-seats 0,7 \
             --seed 10700000 --jobs 6",
        );
        let raw = RawArgs::parse(&command).unwrap();
        assert!(registered_profile(&raw, Phase::Null));
        let mut noncanonical = command.clone();
        *noncanonical.iter_mut().find(|arg| *arg == "250").unwrap() = "0250".into();
        assert!(!registered_profile(
            &RawArgs::parse(&noncanonical).unwrap(),
            Phase::Null
        ));
        let mut missing = command.clone();
        let position = missing.iter().position(|arg| arg == "--jobs").unwrap();
        missing.drain(position..=position + 1);
        assert!(!registered_profile(
            &RawArgs::parse(&missing).unwrap(),
            Phase::Null
        ));
        let mut duplicate = command.clone();
        duplicate.extend(["--jobs".into(), "6".into()]);
        assert!(RawArgs::parse(&duplicate).is_err());
        let mut extra = command;
        extra.extend(["--difficulty-extra".into(), "prince".into()]);
        assert!(RawArgs::parse(&extra).is_err());
        assert!(RawArgs::parse(&strings("--maps --jobs 6")).is_err());
        assert!(RawArgs::parse(&strings("--maps nope")).is_ok());
        assert!(Config::from_raw(&RawArgs::parse(&strings("--maps nope")).unwrap()).is_err());
    }

    #[test]
    fn only_direct_foreign_major_conquest_qualifies() {
        let base = found_game();
        let city = base.player_city_ids(1)[0];

        let mut conquered = base.clone();
        conquered.cities.get_mut(&city).unwrap().owner = 0;
        conquered.cities.get_mut(&city).unwrap().occupied_from = Some(1);
        assert_eq!(qualifying_capture(&conquered, 0), Some((city, 1)));

        let mut peaceful = conquered.clone();
        peaceful.cities.get_mut(&city).unwrap().occupied_from = None;
        assert_eq!(qualifying_capture(&peaceful, 0), None);

        let mut recapture = conquered.clone();
        recapture.cities.get_mut(&city).unwrap().original_owner = 0;
        assert_eq!(qualifying_capture(&recapture, 0), None);

        let minor = base
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .unwrap()
            .id;
        let mut minor_capture = conquered;
        minor_capture.cities.get_mut(&city).unwrap().occupied_from = Some(minor);
        assert_eq!(qualifying_capture(&minor_capture, 0), None);

        let free = base
            .players
            .iter()
            .find(|player| player.is_free_city)
            .expect("full games carry the Free Cities player")
            .id;
        let mut free_capture = base.clone();
        free_capture.cities.get_mut(&city).unwrap().owner = 0;
        free_capture.cities.get_mut(&city).unwrap().occupied_from = Some(free);
        assert_eq!(qualifying_capture(&free_capture, 0), None);
        assert_eq!(
            qualifying_capture(&base, 0),
            None,
            "normally founded cities do not carry conquest provenance"
        );
    }

    #[test]
    fn treatment_commits_exactly_once_and_control_only_observes() {
        let mut game = found_game();
        game.victory_conditions = VictoryConditions::parse("science,culture,domination").unwrap();
        let city = game.player_city_ids(1)[0];
        game.cities.get_mut(&city).unwrap().owner = 0;
        game.cities.get_mut(&city).unwrap().occupied_from = Some(1);

        let mut treatment_ai = pinned_advanced();
        let mut treatment = Observer::default();
        assert!(treatment
            .maybe_trigger(&game, 0, &mut treatment_ai, true)
            .unwrap());
        assert_eq!(treatment.retargets, 1);
        assert_eq!(
            treatment_ai.victory_target(),
            Some(VictoryTarget::Domination)
        );
        let control_game = game.clone();
        game.cities.get_mut(&city).unwrap().owner = 1;
        assert!(!treatment
            .maybe_trigger(&game, 0, &mut treatment_ai, true)
            .unwrap());
        assert_eq!(treatment.retargets, 1);
        assert_eq!(
            treatment_ai.victory_target(),
            Some(VictoryTarget::Domination)
        );

        game.current = 0;
        game.turn += 5;
        treatment_ai.take_turn(&mut game, 0);
        assert_eq!(
            treatment_ai.victory_target(),
            Some(VictoryTarget::Domination),
            "peace and an ordinary reassessment cannot clear the commitment"
        );

        let mut control_ai = pinned_advanced();
        let mut control = Observer::default();
        assert!(control
            .maybe_trigger(&control_game, 0, &mut control_ai, false)
            .unwrap());
        assert_eq!(control.retargets, 0);
        assert_eq!(control_ai.victory_target(), None);
        assert_eq!(control.trigger, treatment.trigger);
    }

    #[test]
    fn default_off_pair_is_world_and_result_identical() {
        let options = GameOptions::new(2, 20, 14, 71_902, 1, 0);
        let result = play_pair(
            options,
            0,
            Treatment::None,
            VictoryConditions::parse("science,culture,domination").unwrap(),
            1,
        )
        .unwrap();
        assert!(result.exact_prefix);
        assert!(result.exact_terminal);
        assert_eq!(result.control, result.treatment);
        assert_eq!(result.control.retargets, 0);
    }

    #[test]
    fn capital_metrics_use_original_capitals_and_post_trigger_baseline() {
        let mut game = found_game();
        let second = game.player_city_ids(1)[0];
        let third = game.player_city_ids(2)[0];
        game.cities.get_mut(&second).unwrap().owner = 0;
        assert_eq!(foreign_capitals(&game, 0), 1);
        game.cities.get_mut(&third).unwrap().owner = 0;
        assert_eq!(foreign_capitals(&game, 0), 2);
        game.cities.get_mut(&second).unwrap().is_capital = false;
        assert_eq!(foreign_capitals(&game, 0), 1);

        let mut observer = Observer::default();
        observer.observe(&game, 0);
        game.cities.get_mut(&third).unwrap().owner = 2;
        game.cities.get_mut(&second).unwrap().occupied_from = Some(1);
        let mut ai = pinned_advanced();
        assert!(observer.maybe_trigger(&game, 0, &mut ai, false).unwrap());
        observer.observe(&game, 0);
        let result = result_from(&game, 0, observer, game.turn);
        assert_eq!(result.peak_foreign_capitals, 1);
        assert_eq!(result.post_trigger_capital_gain, 0);
    }

    #[test]
    fn map_clustering_sign_test_bootstrap_and_gates_are_frozen() {
        assert!((exact_two_sided(5, 5) - 0.0625).abs() < 1e-12);
        assert_eq!(exact_two_sided(0, 0), 1.0);
        assert_eq!(percentile_interval(vec![4.0, 1.0, 3.0, 2.0]), (1.0, 3.0));

        let positive = MapResult {
            cells: [fixture_cell(0, 2), fixture_cell(1, 1)],
        };
        assert_eq!(positive.eligible_cells(), 2);
        assert_eq!(positive.capital_gain_difference(), 1.0);
        assert_eq!(positive.utility_pair(), (0.20, 0.24));
        let mut ineligible = fixture_cell(0, 0);
        ineligible.control.eligible = false;
        ineligible.control.trigger = None;
        ineligible.treatment.eligible = false;
        ineligible.treatment.trigger = None;
        ineligible.treatment.retargets = 0;
        let sparse_positive = MapResult {
            cells: [fixture_cell(0, 2), ineligible],
        };
        let dense_neutral = MapResult {
            cells: [fixture_cell(0, 0), fixture_cell(0, 0)],
        };
        assert_eq!(
            summarize(&[sparse_positive, dense_neutral]).capital_mean_difference,
            1.0,
            "each map row has equal weight even when exposure differs by seat"
        );
        let neutral = MapResult {
            cells: [fixture_cell(1, 1), fixture_cell(0, 0)],
        };
        let negative = MapResult {
            cells: [fixture_cell(2, 0), fixture_cell(1, 1)],
        };
        let clustered = summarize(&[positive.clone(), neutral, negative]);
        assert_eq!(clustered.capital_favorable, 1);
        assert_eq!(clustered.capital_adverse, 1);
        assert_eq!(
            bootstrap_capital_difference(std::slice::from_ref(&positive)),
            bootstrap_capital_difference(&[positive])
        );

        let screen = passing_facts(60);
        assert!(screen_passes(screen));
        assert!(!screen_passes(GateFacts {
            eligible: 17,
            ..screen
        }));
        assert!(!screen_passes(GateFacts {
            treatment_capital_gains: 13,
            ..screen
        }));
        assert!(!screen_passes(GateFacts {
            strength_share: 0.519,
            ..screen
        }));
        assert!(!screen_passes(GateFacts {
            treatment_survived: screen.control_survived - 3,
            ..screen
        }));

        let holdout = passing_facts(240);
        assert!(holdout_passes(holdout));
        assert!(!holdout_passes(GateFacts {
            capital_mean_difference: 0.149,
            ..holdout
        }));
        assert!(!holdout_passes(GateFacts {
            capital_ci_low: 0.0,
            ..holdout
        }));
        assert!(!holdout_passes(GateFacts {
            capital_sign_p: 0.05,
            ..holdout
        }));
        assert!(!holdout_passes(GateFacts {
            strength_ci_low: 0.50,
            ..holdout
        }));
    }
}
