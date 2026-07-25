//! Zero-dependency local HTTP server for the human-vs-AI browser GUI.
//! Endpoints: GET / (page), GET /cinematic3d.js, GET /state, GET /save, GET /rules, GET /pedia,
//! POST /action, POST /step, POST /autoplay, POST /view,
//! POST /spectator-status, POST /next-game-settings, POST /new,
//! POST /supervisor-new.
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::ai::{AdvancedAi, Ai, BasicAi};
use crate::game::{Action, Game, GameOptions, VictoryConditions};
use crate::rules::Rules;
use crate::obs::{observation, observation_player_view, observation_spectator};
use crate::setup::{
    GameSpeed, MapScript, MapSize, CIV6_GAME_SPEEDS, CIV6_MAP_SCRIPTS, CIV6_MAP_SIZES,
};
use crate::Pos;

const EMBEDDED_INDEX: &str = include_str!("../web/index.html");
const EMBEDDED_CINEMATIC_3D: &str = include_str!("../web/cinematic3d.js");
const EMBEDDED_TERRAIN_ATLAS: &[u8] = include_bytes!("../web/assets/terrain-atlas.png");
const EMBEDDED_FEATURE_ATLAS: &[u8] = include_bytes!("../web/assets/feature-atlas.png");
const EMBEDDED_ENVIRONMENT_FEATURE_ATLAS: &[u8] =
    include_bytes!("../web/assets/environment-feature-atlas.png");
const EMBEDDED_NATURAL_WONDER_ATLAS: &[u8] =
    include_bytes!("../web/assets/natural-wonder-atlas.png");
const EMBEDDED_WORLD_WONDER_ATLAS: &[u8] =
    include_bytes!("../web/assets/world-wonder-atlas.png");
const EMBEDDED_MOUNTAIN_ATLAS: &[u8] = include_bytes!("../web/assets/mountain-atlas.png");

#[derive(Clone)]
pub struct Params {
    pub num_players: usize,
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    pub map_script: MapScript,
    pub game_speed: GameSpeed,
    pub max_turns: u32,
    pub victory_conditions: VictoryConditions,
    pub num_city_states: usize,
    /// All players AI-driven; the GUI just watches (auto-steps via /step).
    pub spectate: bool,
    pub difficulty: String,
    pub speed: String,
    pub teams: Vec<Option<usize>>,
    /// Civilizations for the leading major seats, in seat order — seat 0 is
    /// the person's own. Empty is the stock roster; see `Game::seat_civs`.
    pub civs: Vec<String>,
    /// A lifecycle supervisor, rather than the browser countdown, owns the
    /// transition after a completed spectator game.
    pub supervised: bool,
    /// Requested result-screen duration. Five seconds is the minimum; a
    /// supervisor may ask for longer when its handoff needs more time.
    pub restart_ms: u64,
    /// League directory to seat major players from (`civvis play --league`):
    /// each civ gets its best-rated strategies and the HUD shows per-player
    /// elo. `None` still annotates elo when a `league/` dir exists, because
    /// the default fleet below IS the league's "advanced" entrant — but the
    /// AIs themselves are unchanged.
    pub league_dir: Option<String>,
    /// Rate the finished game into `league_dir` (`--league-record`). Off by
    /// default because the shipped `data/league` roster is a committed
    /// snapshot: a run that seats from it must not rewrite it. Point this at
    /// a runtime copy and the table moves with every game played.
    pub league_record: bool,
}

pub struct Session {
    pub params: Params,
    pub game: Game,
    ais: Vec<Box<dyn Ai + Send>>,
    spectator_paused: bool,
    /// `None` is the omniscient spectator; `Some(pid)` is that major
    /// civilization's fog-of-war perspective. Only meaningful in spectate
    /// mode—the AI still controls every seat either way.
    view_player: Option<usize>,
    /// Irreversible event-log history and the running totals for active wars.
    /// Session scope prevents destroyed infrastructure or a temporarily lost
    /// high-population city from being announced as a first a second time.
    chronicle: ChronicleState,
    /// Manual new-game handoff consumed by the external spectator supervisor.
    /// The current process stays available until the requested runtime is ready.
    supervisor_request: Option<Value>,
    /// Setup selected while this world is running. It is inert until the next
    /// automatic or explicitly requested simulation boundary.
    next_game_params: Option<Params>,
    /// League roster used to label seats with player handles and elo (and,
    /// with `--league`, to choose who plays each civ).
    league: Option<crate::league::League>,
    /// Per-seat index into `league.strategies` for rated major seats.
    seat_strategy: Vec<Option<usize>>,
    /// Set once this game's result has been rated, so a winner that is
    /// stepped past more than once is only ever counted for one game.
    league_recorded: bool,
}

#[derive(Clone)]
struct ChronicleCity {
    name: String,
    owner: usize,
    pop: i32,
    occupied_from: Option<usize>,
}

#[derive(Clone)]
struct ChronicleDistrict {
    city: u32,
    district: String,
    owner: usize,
}

struct ChronicleSnapshot {
    turn: u32,
    cities: BTreeMap<u32, ChronicleCity>,
    districts: BTreeMap<Pos, ChronicleDistrict>,
    buildings: BTreeMap<(u32, String), usize>,
    wonders: BTreeMap<String, usize>,
    religions: Vec<Option<String>>,
    governments: Vec<Option<String>>,
    suzerains: BTreeMap<usize, Option<usize>>,
    tech_eras: Vec<usize>,
    civic_eras: Vec<usize>,
    majors: Vec<bool>,
    wars: BTreeSet<(usize, usize)>,
    military_units: BTreeMap<u32, usize>,
    combat_owners: BTreeMap<Pos, BTreeSet<usize>>,
}

#[derive(Clone, Default)]
struct WarLosses {
    units: u32,
    cities: u32,
}

#[derive(Clone)]
struct ChronicleWar {
    aggressor: usize,
    defender: usize,
    losses: BTreeMap<usize, WarLosses>,
}

impl ChronicleWar {
    fn new(aggressor: usize, defender: usize) -> Self {
        Self {
            aggressor,
            defender,
            losses: BTreeMap::new(),
        }
    }

    fn losses_for(&self, player: usize) -> WarLosses {
        self.losses.get(&player).cloned().unwrap_or_default()
    }
}

struct ChronicleState {
    districts: BTreeSet<String>,
    buildings: BTreeSet<String>,
    population_milestones: Vec<i32>,
    wars: BTreeMap<(usize, usize), ChronicleWar>,
}

pub struct SpectatorStep {
    pub player: usize,
    pub actions: Vec<Action>,
    pub world_events: Vec<Value>,
}

impl ChronicleSnapshot {
    fn capture(game: &Game) -> Self {
        let mut districts = BTreeMap::new();
        let mut buildings = BTreeMap::new();
        let mut wonders = BTreeMap::new();
        let mut combat_owners: BTreeMap<Pos, BTreeSet<usize>> = BTreeMap::new();
        for city in game.cities.values() {
            for (district, position) in &city.districts {
                districts.insert(
                    *position,
                    ChronicleDistrict {
                        city: city.id,
                        district: district.clone(),
                        owner: city.owner,
                    },
                );
            }
            for building in &city.buildings {
                if game
                    .rules
                    .buildings
                    .get(building)
                    .is_some_and(|spec| spec.buildable)
                {
                    buildings.insert((city.id, building.clone()), city.owner);
                }
            }
            for wonder in city.wonders.keys() {
                wonders.insert(wonder.clone(), city.owner);
            }
            combat_owners
                .entry(city.pos)
                .or_default()
                .insert(city.owner);
        }
        let military_units = game
            .units
            .values()
            .filter(|unit| game.rules.units[unit.kind.as_str()].class == "military")
            .map(|unit| {
                combat_owners
                    .entry(unit.pos)
                    .or_default()
                    .insert(unit.owner);
                (unit.id, unit.owner)
            })
            .collect();
        let tree_era = |nodes: &BTreeSet<String>, technology: bool| {
            nodes
                .iter()
                .filter_map(|node| {
                    if technology {
                        game.rules.techs.get(node).map(|spec| spec.era)
                    } else {
                        game.rules.civics.get(node).map(|spec| spec.era)
                    }
                })
                .max()
                .unwrap_or(0)
        };
        Self {
            turn: game.turn,
            cities: game
                .cities
                .values()
                .map(|city| {
                    (
                        city.id,
                        ChronicleCity {
                            name: city.name.clone(),
                            owner: city.owner,
                            pop: city.pop,
                            occupied_from: city.occupied_from,
                        },
                    )
                })
                .collect(),
            districts,
            buildings,
            wonders,
            religions: game
                .players
                .iter()
                .map(|player| player.religion.clone())
                .collect(),
            governments: game
                .players
                .iter()
                .map(|player| player.government.clone())
                .collect(),
            suzerains: game
                .players
                .iter()
                .filter(|player| player.is_minor && !player.is_barbarian)
                .map(|player| (player.id, game.suzerain_of(player.id)))
                .collect(),
            tech_eras: game
                .players
                .iter()
                .map(|player| tree_era(&player.techs, true))
                .collect(),
            civic_eras: game
                .players
                .iter()
                .map(|player| tree_era(&player.civics, false))
                .collect(),
            majors: game
                .players
                .iter()
                .map(|player| !player.is_minor && !player.is_barbarian)
                .collect(),
            wars: game.at_war.clone(),
            military_units,
            combat_owners,
        }
    }
}

fn completed_districts(game: &Game) -> BTreeSet<String> {
    game.cities
        .values()
        .flat_map(|city| city.districts.keys())
        .cloned()
        .collect()
}

fn completed_buildings(game: &Game) -> BTreeSet<String> {
    game.cities
        .values()
        .flat_map(|city| city.buildings.iter())
        .filter(|building| {
            game.rules
                .buildings
                .get(*building)
                .is_some_and(|spec| spec.buildable)
        })
        .cloned()
        .collect()
}

fn population_milestone(population: i32) -> i32 {
    if population < 4 {
        0
    } else {
        4 + ((population - 4) / 3) * 3
    }
}

impl ChronicleState {
    fn from_game(game: &Game) -> Self {
        let population_milestones = game
            .players
            .iter()
            .map(|player| {
                game.cities
                    .values()
                    .filter(|city| city.owner == player.id)
                    .map(|city| city.pop)
                    .max()
                    .map(population_milestone)
                    .unwrap_or(0)
            })
            .collect();
        let wars = game
            .at_war
            .iter()
            .map(|&(first, second)| ((first, second), ChronicleWar::new(first, second)))
            .collect();
        Self {
            districts: completed_districts(game),
            buildings: completed_buildings(game),
            population_milestones,
            wars,
        }
    }
}

fn chronicle_war_pair(first: usize, second: usize) -> (usize, usize) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn war_totals_event(event_type: &str, war: &ChronicleWar, turn: u32) -> Value {
    let aggressor = war.losses_for(war.aggressor);
    let defender = war.losses_for(war.defender);
    json!({
        "type": event_type,
        "aggressor": war.aggressor,
        "defender": war.defender,
        "aggressor_units_lost": aggressor.units,
        "aggressor_cities_lost": aggressor.cities,
        "defender_units_lost": defender.units,
        "defender_cities_lost": defender.cities,
        "turn": turn,
    })
}

fn chronicle_world_events(
    before: &ChronicleSnapshot,
    after: &ChronicleSnapshot,
    actor: usize,
    actions: &[Action],
    chronicle: &mut ChronicleState,
) -> Vec<Value> {
    let mut events = Vec::new();
    let turn = after.turn;

    for (wonder, owner) in &after.wonders {
        if !before.wonders.contains_key(wonder) {
            events.push(json!({
                "type": "wonder_built", "player": owner,
                "wonder": wonder, "turn": turn,
            }));
        }
    }

    for (player, religion) in after.religions.iter().enumerate() {
        if before.religions.get(player).is_some_and(Option::is_none) {
            if let Some(religion) = religion {
                events.push(json!({
                    "type": "religion_founded", "player": player,
                    "religion": religion, "turn": turn,
                }));
            }
        }
    }

    let mut new_districts: Vec<_> = after
        .districts
        .iter()
        .filter(|(position, _)| !before.districts.contains_key(position))
        .map(|(_, district)| district)
        .collect();
    new_districts.sort_by_key(|district| district.city);
    for district in new_districts {
        if chronicle.districts.insert(district.district.clone()) {
            let city = after
                .cities
                .get(&district.city)
                .map(|city| city.name.as_str());
            events.push(json!({
                "type": "district_first", "player": district.owner,
                "district": district.district, "city": city, "turn": turn,
            }));
        }
    }

    let mut new_buildings: Vec<_> = after
        .buildings
        .iter()
        .filter(|(key, _)| !before.buildings.contains_key(*key))
        .collect();
    new_buildings.sort_by_key(|((city, building), _)| (*city, building.as_str()));
    for ((city_id, building), owner) in new_buildings {
        if chronicle.buildings.insert(building.clone()) {
            let city = after.cities.get(city_id).map(|city| city.name.as_str());
            events.push(json!({
                "type": "building_first", "player": owner,
                "building": building, "city": city, "turn": turn,
            }));
        }
    }

    for (player, major) in after.majors.iter().copied().enumerate() {
        if !major {
            continue;
        }
        let Some(city) = after
            .cities
            .values()
            .filter(|city| city.owner == player)
            .max_by_key(|city| (city.pop, std::cmp::Reverse(city.name.as_str())))
        else {
            continue;
        };
        let milestone = population_milestone(city.pop);
        let seen = chronicle
            .population_milestones
            .get_mut(player)
            .expect("chronicle population ledger matches players");
        if milestone > *seen {
            // If conquest jumps over several thresholds, announce the current
            // one and retire the lower thresholds instead of flooding the log.
            *seen = milestone;
            events.push(json!({
                "type": "population_milestone", "player": player,
                "population": milestone, "city": city.name, "turn": turn,
            }));
        }
    }

    // Capture decisions are resolved before an AI can end its turn. Reading
    // those decisions catches kept, razed, and immediately liberated cities.
    let mut captured = BTreeSet::new();
    for action in actions {
        let city = match action {
            Action::KeepCity { city }
            | Action::RazeCity { city }
            | Action::LiberateCity { city } => Some(*city),
            _ => None,
        };
        let Some(city) = city else { continue };
        let Some(previous) = before.cities.get(&city) else {
            continue;
        };
        if captured.insert(city) {
            events.push(json!({
                "type": "city_captured", "player": actor,
                "former": previous.owner, "city": previous.name,
                "turn": turn,
            }));
        }
    }
    // Also cover a conquest that ended the match before its keep/raze choice
    // was logged.
    for (city, previous) in &before.cities {
        let Some(current) = after.cities.get(city) else {
            continue;
        };
        if current.owner != previous.owner
            && current.occupied_from == Some(previous.owner)
            && captured.insert(*city)
        {
            events.push(json!({
                "type": "city_captured", "player": current.owner,
                "former": previous.owner, "city": previous.name,
                "turn": turn,
            }));
        }
    }

    let active_wars: BTreeSet<_> = before.wars.union(&after.wars).copied().collect();
    for &(first, second) in after.wars.difference(&before.wars) {
        let (aggressor, defender) = if actor == first {
            (first, second)
        } else if actor == second {
            (second, first)
        } else {
            (first, second)
        };
        chronicle
            .wars
            .insert((first, second), ChronicleWar::new(aggressor, defender));
        events.push(json!({
            "type": "war_started", "aggressor": aggressor,
            "defender": defender, "turn": turn,
        }));
    }
    for &(first, second) in &active_wars {
        chronicle
            .wars
            .entry((first, second))
            .or_insert_with(|| ChronicleWar::new(first, second));
    }

    // Only vanished military units count as war losses. Corps/Army formation
    // consumes one constituent without a battle, so exclude both participants
    // and let the still-present one identify the survivor.
    let combined_units: BTreeSet<u32> = actions
        .iter()
        .flat_map(|action| match action {
            Action::CombineUnits { unit, with } => vec![*unit, *with],
            _ => Vec::new(),
        })
        .collect();
    let mut lost_units: BTreeMap<usize, u32> = BTreeMap::new();
    for (unit, owner) in &before.military_units {
        if !after.military_units.contains_key(unit) && !combined_units.contains(unit) {
            *lost_units.entry(*owner).or_default() += 1;
        }
    }

    let mut targeted_opponents = BTreeSet::new();
    for target in actions.iter().filter_map(|action| match action {
        Action::Attack { target, .. }
        | Action::Ranged { target, .. }
        | Action::AirStrike { target, .. }
        | Action::CityStrike { target, .. }
        | Action::EncampmentStrike { target, .. } => Some(*target),
        _ => None,
    }) {
        if let Some(owners) = before.combat_owners.get(&target) {
            targeted_opponents.extend(owners.iter().copied().filter(|owner| {
                *owner != actor && active_wars.contains(&chronicle_war_pair(actor, *owner))
            }));
        }
    }
    let enemy_losers: BTreeSet<_> = lost_units
        .keys()
        .copied()
        .filter(|owner| *owner != actor && active_wars.contains(&chronicle_war_pair(actor, *owner)))
        .collect();
    let actor_opponent = if targeted_opponents.len() == 1 {
        targeted_opponents.first().copied()
    } else if enemy_losers.len() == 1 {
        enemy_losers.first().copied()
    } else {
        let opponents: BTreeSet<_> = active_wars
            .iter()
            .filter_map(|&(first, second)| {
                if first == actor {
                    Some(second)
                } else if second == actor {
                    Some(first)
                } else {
                    None
                }
            })
            .collect();
        (opponents.len() == 1).then(|| *opponents.first().unwrap())
    };

    let mut changed_wars = BTreeSet::new();
    for (owner, losses) in lost_units {
        let opponent = if owner == actor {
            actor_opponent
        } else if active_wars.contains(&chronicle_war_pair(actor, owner)) {
            Some(actor)
        } else {
            None
        };
        let Some(opponent) = opponent else { continue };
        let pair = chronicle_war_pair(owner, opponent);
        let war = chronicle
            .wars
            .entry(pair)
            .or_insert_with(|| ChronicleWar::new(actor, opponent));
        war.losses.entry(owner).or_default().units += losses;
        changed_wars.insert(pair);
    }

    let mut lost_cities = BTreeSet::new();
    for (city_id, previous) in &before.cities {
        let conqueror = match after.cities.get(city_id) {
            Some(current) if current.owner != previous.owner => Some(current.owner),
            None if captured.contains(city_id) => Some(actor),
            _ => None,
        };
        let Some(conqueror) = conqueror else {
            continue;
        };
        let pair = chronicle_war_pair(previous.owner, conqueror);
        if previous.owner == conqueror
            || !active_wars.contains(&pair)
            || !lost_cities.insert(*city_id)
        {
            continue;
        }
        let war = chronicle
            .wars
            .entry(pair)
            .or_insert_with(|| ChronicleWar::new(conqueror, previous.owner));
        war.losses.entry(previous.owner).or_default().cities += 1;
        changed_wars.insert(pair);
    }

    for pair in changed_wars {
        if after.wars.contains(&pair) {
            if let Some(war) = chronicle.wars.get(&pair) {
                events.push(war_totals_event("war_progress", war, turn));
            }
        }
    }
    let ended_wars: Vec<_> = before.wars.difference(&after.wars).copied().collect();
    for pair in ended_wars {
        if let Some(war) = chronicle.wars.remove(&pair) {
            events.push(war_totals_event("war_ended", &war, turn));
        }
    }

    for (city_state, current) in &after.suzerains {
        let previous = before.suzerains.get(city_state).copied().flatten();
        if previous != *current {
            events.push(json!({
                "type": "suzerain_changed", "city_state": city_state,
                "from": previous, "to": current, "turn": turn,
            }));
        }
    }

    let first_era_events =
        |track: &str, before_eras: &[usize], after_eras: &[usize], events: &mut Vec<Value>| {
            let before_lead = before_eras
                .iter()
                .enumerate()
                .filter(|(player, _)| before.majors.get(*player) == Some(&true))
                .map(|(_, era)| *era)
                .max()
                .unwrap_or(0);
            let after_lead = after_eras
                .iter()
                .enumerate()
                .filter(|(player, _)| after.majors.get(*player) == Some(&true))
                .map(|(_, era)| *era)
                .max()
                .unwrap_or(0);
            for era in (before_lead + 1)..=after_lead {
                let Some(player) = after_eras
                    .iter()
                    .enumerate()
                    .find_map(|(player, after_era)| {
                        (after.majors.get(player) == Some(&true)
                            && *after_era >= era
                            && before_eras.get(player).copied().unwrap_or(0) < era)
                            .then_some(player)
                    })
                else {
                    continue;
                };
                events.push(json!({
                    "type": "era_first", "player": player,
                    "track": track, "era": era, "turn": turn,
                }));
            }
        };
    first_era_events(
        "technology",
        &before.tech_eras,
        &after.tech_eras,
        &mut events,
    );
    first_era_events("civics", &before.civic_eras, &after.civic_eras, &mut events);

    for (player, government) in after.governments.iter().enumerate() {
        if after.majors.get(player) != Some(&true) {
            continue;
        }
        let previous = before.governments.get(player).cloned().flatten();
        if previous != *government {
            events.push(json!({
                "type": "government_changed", "player": player,
                "from": previous, "to": government, "turn": turn,
            }));
        }
    }

    events
}

/// Server-side exhibition state: in spectate mode a background thread steps
/// the game at `pace_ms` per game turn and restarts 5s after a victory, so
/// games keep running with no browser attached.
///
/// `pace_ms` is the budget for a whole turn — every seat taking one step —
/// rather than for one seat, so the pace a viewer picks means the same wall
/// time whatever the player count. `0` means unlimited: no artificial wait at
/// all, the simulation runs as fast as the machine allows.
pub struct Shared {
    pub session: Mutex<Session>,
    pub pace_ms: AtomicU64,
    pub paused: AtomicBool,
    pub restart_in: AtomicU64, // ms until auto-restart; u64::MAX = not pending
    /// Measured wall time of a full game turn, including pacing sleeps.
    pub turn_us: AtomicU64,
    /// The same turn with the sleeps taken out: what the unlimited pace costs.
    pub turn_compute_us: AtomicU64,
    frame_delivery: Mutex<FrameDelivery>,
    frame_delivered: Condvar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SpectatorFrame {
    seed: u64,
    turn: u32,
}

#[derive(Default)]
struct FrameDelivery {
    last_request: Option<Instant>,
    delivered: Option<SpectatorFrame>,
}

impl FrameDelivery {
    fn request_started(&mut self, now: Instant) {
        self.last_request = Some(now);
    }

    fn frame_delivered(&mut self, frame: SpectatorFrame, now: Instant) {
        self.last_request = Some(now);
        self.delivered = Some(frame);
    }

    fn wait_remaining(&self, frame: SpectatorFrame, now: Instant) -> Option<Duration> {
        if self.delivered == Some(frame) {
            return None;
        }
        VIEWER_ACTIVE
            .checked_sub(now.saturating_duration_since(self.last_request?))
            .filter(|remaining| !remaining.is_zero())
    }
}

const MIN_RESTART_MS: u64 = 5_000;
/// A lone `/state` probe must not throttle an otherwise unattended exhibition
/// forever. The browser polls every 300ms at Lightning, so two seconds leaves
/// ample room for rendering jitter while releasing a disconnected viewer.
const VIEWER_ACTIVE: Duration = Duration::from_secs(2);
const FRAME_WAIT_RECHECK: Duration = Duration::from_millis(100);
/// The unlimited pace still hands the accept loop a slot this often, so the
/// page keeps loading state while the stepper saturates a core.
const UNLIMITED_BREATH_MS: u64 = 100;
/// Minor civilizations and barbarians take a quarter of a major's slice.
const MINOR_SHARE: f64 = 0.25;

fn final_countdown_ms(requested: u64) -> u64 {
    requested.max(MIN_RESTART_MS)
}

/// One seat's slice of the turn budget. Seats divide it in proportion to the
/// beat they are given, so a whole turn costs `pace_ms` whether it is two
/// empires or eight with a dozen city-states between them. The counts are of
/// seats that still take a turn — the eliminated are nobody's wait.
pub fn seat_delay_ms(pace_ms: u64, majors: usize, minors: usize, minor: bool) -> u64 {
    let weight = (majors as f64 + minors as f64 * MINOR_SHARE).max(1.0);
    let share = if minor { MINOR_SHARE } else { 1.0 };
    ((pace_ms as f64) * share / weight).round() as u64
}

/// Smooth a measurement so the reported figure does not flicker turn to turn.
fn blend(slot: &AtomicU64, sample: u64) {
    let prior = slot.load(Ordering::Relaxed);
    let next = if prior == 0 {
        sample
    } else {
        (prior * 3 + sample) / 4
    };
    slot.store(next, Ordering::Relaxed);
}

impl Shared {
    fn note_frame_request(&self) {
        self.frame_delivery
            .lock()
            .unwrap()
            .request_started(Instant::now());
    }

    fn note_frame_delivered(&self, frame: SpectatorFrame) {
        self.frame_delivery
            .lock()
            .unwrap()
            .frame_delivered(frame, Instant::now());
        self.frame_delivered.notify_all();
    }

    fn wait_for_lightning_frame(&self, frame: SpectatorFrame) {
        let mut delivery = self.frame_delivery.lock().unwrap();
        loop {
            if self.pace_ms.load(Ordering::Relaxed) != 0 || self.paused.load(Ordering::Relaxed) {
                return;
            }
            let Some(remaining) = delivery.wait_remaining(frame, Instant::now()) else {
                return;
            };
            let result = self
                .frame_delivered
                .wait_timeout(delivery, remaining.min(FRAME_WAIT_RECHECK))
                .unwrap();
            delivery = result.0;
        }
    }
}

impl Session {
    /// Seat AIs plus each seat's league identity. With a roster to seat
    /// from, every major civ is played by its best-rated available
    /// strategy (`league::seat_by_civ`); otherwise majors run the default
    /// hierarchical AI, which the league rates as its "advanced" entrant,
    /// so a loaded roster can still label those seats with an elo.
    fn ai_fleet(
        game: &Game,
        league: Option<&crate::league::League>,
        seat_from_roster: bool,
    ) -> (Vec<Box<dyn Ai + Send>>, Vec<Option<usize>>) {
        let mut seat_strategy: Vec<Option<usize>> = vec![None; game.players.len()];
        if let Some(l) = league {
            let majors: Vec<usize> = game
                .players
                .iter()
                .filter(|p| !p.is_minor && !p.is_barbarian)
                .map(|p| p.id)
                .collect();
            if seat_from_roster && !l.active().is_empty() {
                let civs: Vec<String> =
                    majors.iter().map(|id| game.players[*id].civ.clone()).collect();
                for (id, pick) in majors.iter().zip(crate::league::seat_by_civ(l, &civs)) {
                    seat_strategy[*id] = Some(pick);
                }
            } else if let Some(default_entrant) =
                l.strategies.iter().position(|s| s.name == "advanced")
            {
                for id in majors {
                    seat_strategy[id] = Some(default_entrant);
                }
            }
        }
        let ais = game
            .players
            .iter()
            .map(|p| -> Box<dyn Ai + Send> {
                if p.is_minor || p.is_barbarian {
                    return Box::new(BasicAi::new());
                }
                match (seat_from_roster, league, seat_strategy[p.id]) {
                    (true, Some(l), Some(si)) => crate::league::make_send_ai(
                        &l.strategies[si].kind,
                        game.seed.wrapping_add(p.id as u64),
                    ),
                    _ => Box::new(AdvancedAi::new()),
                }
            })
            .collect();
        (ais, seat_strategy)
    }

    /// The roster named by `--league`, else a best-effort `league/` load
    /// purely for elo labels.
    fn load_params_league(params: &Params) -> (Option<crate::league::League>, bool) {
        match &params.league_dir {
            Some(dir) => (crate::league::load_league(dir), true),
            None => (crate::league::load_league("league"), false),
        }
    }

    pub fn new(params: Params) -> Session {
        // Seat 0 is the person at the keyboard, which is what decides who the
        // difficulty hands its bonuses to. A spectated game has nobody there.
        let human_seats = if params.spectate {
            BTreeSet::new()
        } else {
            BTreeSet::from([0usize])
        };
        let mut game = Game::new_with(GameOptions {
            map_script: params.map_script,
            difficulty: params.difficulty.clone(),
            speed: params.speed.clone(),
            human_seats,
            teams: params.teams.clone(),
            civs: params.civs.clone(),
            ..GameOptions::new(
                params.num_players,
                params.width,
                params.height,
                params.seed,
                params.max_turns,
                params.num_city_states,
            )
        });
        game.victory_conditions = params.victory_conditions;
        // Paired and multiplayer evaluation make the hierarchical agent the
        // strongest built-in default. Minors/barbarians retain the cheaper
        // baseline because they do not need empire-level planning.
        let (league, seat_from_roster) = Self::load_params_league(&params);
        let (ais, seat_strategy) = Self::ai_fleet(&game, league.as_ref(), seat_from_roster);
        let chronicle = ChronicleState::from_game(&game);
        Session {
            params,
            game,
            ais,
            spectator_paused: false,
            view_player: None,
            chronicle,
            supervisor_request: None,
            next_game_params: None,
            league,
            seat_strategy,
            league_recorded: false,
        }
    }

    /// Restore an interrupted match and rebuild only the AIs' transient plans.
    /// The serialized game retains the authoritative RNG and world state.
    pub fn from_game(mut params: Params, game: Game) -> Session {
        // Launch flags may already carry setup selected for the next world.
        // Preserve that intent while the checkpoint below restores the active
        // world's authoritative parameters.
        let requested_next = params.clone();
        params.num_players = game
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .count();
        params.num_city_states = game
            .players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian)
            .count();
        params.width = game.map.width;
        params.height = game.map.height;
        params.seed = game.seed;
        params.map_script = game.map_script;
        params.game_speed = game.game_speed;
        params.max_turns = game.max_turns;
        params.difficulty = game.difficulty.clone();
        params.speed = game.speed.clone();
        params.victory_conditions = game.victory_conditions;
        params.teams = game
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .map(|player| player.team)
            .collect();
        let next_game_params = (simulation_settings(&requested_next)
            != simulation_settings(&params))
        .then_some(requested_next);
        let (league, seat_from_roster) = Self::load_params_league(&params);
        let (ais, seat_strategy) = Self::ai_fleet(&game, league.as_ref(), seat_from_roster);
        let chronicle = ChronicleState::from_game(&game);
        // A match restored with its winner already decided was rated when it
        // finished; rating it again on the next step would count it twice.
        let league_recorded = game.winner.is_some();
        Session {
            params,
            game,
            ais,
            spectator_paused: false,
            view_player: None,
            chronicle,
            supervisor_request: None,
            next_game_params,
            league,
            seat_strategy,
            league_recorded,
        }
    }

    fn set_view_player(&mut self, player: Option<usize>) -> Result<(), String> {
        if !self.params.spectate && player.is_none() {
            return Err("player views are only available in spectate mode".into());
        }
        if let Some(pid) = player {
            let Some(candidate) = self.game.players.get(pid) else {
                return Err(format!("unknown player {pid}"));
            };
            if candidate.is_minor || candidate.is_barbarian {
                return Err(format!("player {pid} is not a major civilization"));
            }
            // Selecting a civilization from the HUD is also the handoff from
            // an interactive match to AI-only observation. Keep the current
            // world intact; the already-created AI fleet can take over every
            // seat on the next spectator step.
            self.params.spectate = true;
        }
        self.view_player = player;
        Ok(())
    }

    /// Start a requested world, rejecting a delayed result-countdown request
    /// after the supervisor has already replaced the finished server.
    fn start_new_game(&mut self, request: &Value) -> Result<(), String> {
        if self.params.supervised {
            return Err("the spectator supervisor owns in-process game replacement".into());
        }
        if let Some(finished) = request.get("replace_finished") {
            let expected_seed = finished["seed"]
                .as_u64()
                .ok_or_else(|| "replace_finished.seed must be an integer".to_string())?;
            let expected_instance = finished["server_instance"]
                .as_u64()
                .ok_or_else(|| "replace_finished.server_instance must be an integer".to_string())?;
            if self.game.winner.is_none()
                || self.game.seed != expected_seed
                || expected_instance != std::process::id() as u64
            {
                return Err("finished game is no longer the active session".into());
            }
        } else if self.params.spectate
            && self.game.winner.is_none()
            && request["force"].as_bool() != Some(true)
        {
            // Old spectator pages used an unguarded result timer. If one
            // survives a process handoff, it must not reset a healthy game.
            // The visible setup button explicitly opts into a manual reset.
            return Err("active spectator game requires an explicit reset".into());
        }
        let previous_view = self.view_player;
        let params = new_game_params(&self.params, request);
        let mut next = Session::new(params);
        // Observation perspective is a display setting, not part of the
        // simulated world. Keep it when rolling into another spectator game
        // as long as that major-player seat still exists in the new setup.
        if next.params.spectate {
            next.view_player = previous_view.filter(|pid| {
                next.game
                    .players
                    .get(*pid)
                    .is_some_and(|player| !player.is_minor && !player.is_barbarian)
            });
        }
        *self = next;
        Ok(())
    }

    fn request_supervised_new_game(&mut self, request: &Value) -> Result<(), String> {
        if !self.params.supervised {
            return Err("fresh-code launches require the spectator supervisor".into());
        }
        let mode = request["mode"]
            .as_str()
            .ok_or_else(|| "mode must be restart or fresh_code".to_string())?;
        if mode != "restart" && mode != "fresh_code" {
            return Err("mode must be restart or fresh_code".into());
        }

        let paused = request["paused"].as_bool().unwrap_or(self.spectator_paused);
        let mut params = new_game_params(&self.params, request);
        params.spectate = true;
        self.supervisor_request = Some(json!({
            "mode": mode,
            "server_instance": std::process::id(),
            "paused": paused,
            "settings": simulation_settings(&params),
        }));
        self.spectator_paused = true;
        Ok(())
    }

    /// Queue setup controls for the next world without changing this one.
    fn stage_next_game_settings(&mut self, request: &Value) {
        let mut params = new_game_params(&self.params, request);
        params.spectate = self.params.spectate;
        self.next_game_params = Some(params);
    }

    fn start_automatic_next_game(&mut self) {
        let next_seed = self
            .params
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let mut params = self
            .next_game_params
            .take()
            .unwrap_or_else(|| self.params.clone());
        params.seed = next_seed;
        *self = Session::new(params);
    }

    /// An agent's plan as the spectator sees it. City ids mean nothing to a
    /// browser, so each one is resolved here into the name and owner the HUD
    /// can actually print.
    fn plan_json(&self, plan: &crate::ai::PlanReport) -> Value {
        let city = |id: Option<u32>| {
            id.and_then(|id| self.game.cities.get(&id)).map(|city| {
                json!({
                    "id": city.id,
                    "name": city.name,
                    "owner": city.owner,
                    "owner_civ": self.game.players[city.owner].civ,
                    "pos": [city.pos.0, city.pos.1],
                })
            })
        };
        json!({
            "strategy": plan.strategy,
            "victory_target": plan.victory_target,
            "target_player": plan.target_player,
            "target_civ": plan
                .target_player
                .and_then(|pid| self.game.players.get(pid))
                .map(|player| player.civ.clone()),
            "target_city": city(plan.target_city),
            "threatened_city": city(plan.threatened_city),
            "desired_cities": plan.desired_cities,
            "assessed_turn": plan.assessed_turn,
            "forces": plan.forces.iter().map(|force| json!({
                "domain": force.domain,
                "posture": force.posture,
                "units": force.units,
                "objective": [force.objective.0, force.objective.1],
                "readiness": (force.readiness * 100.0).round() / 100.0,
                "strength_ratio": (force.strength_ratio * 100.0).round() / 100.0,
            })).collect::<Vec<_>>(),
        })
    }

    pub fn state(&self) -> Value {
        if self.params.spectate {
            let g = &self.game;
            // The omniscient view still needs an empire perspective for the
            // side-panel summary. Follow the acting major, falling back when
            // a city-state or barbarian is up.
            let summary_pid = if g.players[g.current].is_minor || g.players[g.current].is_barbarian
            {
                g.players
                    .iter()
                    .find(|p| !p.is_minor && !p.is_barbarian && p.alive)
                    .map(|p| p.id)
                    .unwrap_or(0)
            } else {
                g.current
            };
            let mut o = match self.view_player {
                Some(pid) => observation_player_view(g, pid),
                None => observation_spectator(g, summary_pid),
            };
            // Each rated seat's display elo (civ table when settled, else
            // global), gathered up front so a seat's expected win share can
            // be computed against the rest of the table.
            let seat_elo: std::collections::BTreeMap<usize, f64> = self
                .seat_strategy
                .iter()
                .enumerate()
                .filter_map(|(pid, si)| {
                    let (si, league) = ((*si)?, self.league.as_ref()?);
                    let p = &g.players[pid];
                    if !p.alive || p.is_minor || p.is_barbarian {
                        return None;
                    }
                    Some((
                        pid,
                        crate::league::display_elo(&league.strategies[si], &p.civ).0,
                    ))
                })
                .collect();
            // One table, one winner: the seats share out a single win rather
            // than each answering a separate two-player question.
            let seat_expected: std::collections::BTreeMap<usize, f64> = if seat_elo.len() > 1 {
                let ratings: Vec<f64> = seat_elo.values().copied().collect();
                seat_elo
                    .keys()
                    .copied()
                    .zip(crate::elo::win_shares(&ratings))
                    .collect()
            } else {
                std::collections::BTreeMap::new()
            };
            if let Some(players) = o["players"].as_array_mut() {
                for player in players {
                    let Some(id) = player["id"].as_u64().map(|id| id as usize) else {
                        continue;
                    };
                    if let Some(strategy) = self.ais.get(id).and_then(|ai| ai.strategy_label()) {
                        player["ai_strategy"] = json!(strategy);
                    }
                    // The expanded HUD card explains a civilization's whole
                    // medium-term plan, not just its one-word label, so the
                    // spectator frame carries the agent's own read-out.
                    if let Some(plan) = self.ais.get(id).and_then(|ai| ai.plan_report()) {
                        player["ai_plan"] = self.plan_json(&plan);
                    }
                    // League identity: who is playing this seat and how
                    // strong the league currently believes they are on this
                    // civ. `ai_expected` is the elo-implied chance of winning
                    // this table outright, so the seats sum to 1 and the
                    // number can be checked against winners over time.
                    if let (Some(league), Some(Some(si))) =
                        (self.league.as_ref(), self.seat_strategy.get(id))
                    {
                        let s = &league.strategies[*si];
                        let civ = &g.players[id].civ;
                        let (elo, rd, civ_specific) = crate::league::display_elo(s, civ);
                        player["ai_username"] = json!(s.username);
                        player["ai_strat_label"] = json!(s.label());
                        player["ai_elo"] = json!(elo.round() as i64);
                        player["ai_elo_rd"] = json!(rd.round() as i64);
                        player["ai_elo_civ"] = json!(civ_specific);
                        if let Some(share) = seat_expected.get(&id) {
                            player["ai_expected"] = json!((share * 100.0).round() / 100.0);
                        }
                    }
                }
            }
            o["spectate"] = json!(true);
            o["supervised"] = json!(self.params.supervised);
            o["spectator_paused"] = json!(self.spectator_paused);
            o["view_player"] = json!(self.view_player);
            o["victory_conditions"] = json!(self.game.victory_conditions);
            o["supervisor_request"] = json!(self.supervisor_request);
            o["next_game_settings"] = self
                .next_game_params
                .as_ref()
                .map(simulation_settings)
                .unwrap_or(Value::Null);
            o["legal_actions"] = json!([]);
            // Lets a long-running spectator notice that its server was
            // rebuilt/restarted between games and reload the latest UI.
            o["server_instance"] = json!(std::process::id());
            return o;
        }
        let mut o = observation(&self.game, 0);
        o["spectate"] = json!(false);
        o["supervised"] = json!(self.params.supervised);
        o["view_player"] = json!(0);
        o["victory_conditions"] = json!(self.game.victory_conditions);
        o["supervisor_request"] = json!(self.supervisor_request);
        o["next_game_settings"] = self
            .next_game_params
            .as_ref()
            .map(simulation_settings)
            .unwrap_or(Value::Null);
        o["legal_actions"] = serde_json::to_value(self.game.legal_actions(0)).unwrap();
        o["server_instance"] = json!(std::process::id());
        o
    }

    /// Spectator mode: play out exactly one player's turn with its AI.
    /// Returns the pid and successful actions so the observer UI can explain
    /// the AI's decisions instead of showing only their eventual outcomes.
    pub fn step(&mut self) -> (usize, Vec<Action>) {
        let g = &mut self.game;
        let pid = g.current;
        let log_start = g.log.len();
        if g.winner.is_some() {
            return (pid, vec![]);
        }
        self.ais[pid].take_turn(g, pid);
        if g.current == pid && g.winner.is_none() {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        let actions = g
            .log
            .since(log_start)
            .map(|(_, action)| action.clone())
            .collect();
        // Every way of advancing the world funnels through here — the browser
        // stepping a batch, the headless pacer running an unattended
        // exhibition, autoplay — so this is the one place a result cannot be
        // missed.
        self.record_league_result();
        (pid, actions)
    }

    /// Advance a bounded batch while retaining each civilization's action
    /// trace. The HTTP layer can then serialize the large world observation
    /// once per browser paint instead of once per AI turn.
    fn spectator_step(&mut self) -> SpectatorStep {
        let before = ChronicleSnapshot::capture(&self.game);
        let (player, actions) = self.step();
        let after = ChronicleSnapshot::capture(&self.game);
        let world_events =
            chronicle_world_events(&before, &after, player, &actions, &mut self.chronicle);
        SpectatorStep {
            player,
            actions,
            world_events,
        }
    }

    /// Rate a just-decided game into the roster it was seated from. Without
    /// this a rated exhibition plays hundreds of games against a frozen
    /// table: the elo on screen is whatever the last offline league run left
    /// behind, no matter who keeps winning.
    fn record_league_result(&mut self) {
        if self.league_recorded || self.game.winner.is_none() || !self.params.league_record {
            return;
        }
        self.league_recorded = true;
        let (Some(dir), Some(league)) = (self.params.league_dir.clone(), self.league.as_ref())
        else {
            return;
        };
        // Name every rated seat up front so the roster can be replaced below.
        let seat_names: Vec<Option<String>> = self
            .seat_strategy
            .iter()
            .enumerate()
            .map(|(pid, si)| {
                let p = &self.game.players[pid];
                match (si, p.is_minor || p.is_barbarian) {
                    (Some(si), false) => Some(league.strategies[*si].name.clone()),
                    _ => None,
                }
            })
            .collect();
        let winner = self.game.winner.unwrap();
        // Same ordering the league itself uses: winner first, then by score.
        let mut rated: Vec<usize> = (0..seat_names.len())
            .filter(|pid| seat_names[*pid].is_some())
            .collect();
        rated.sort_by_key(|pid| (*pid != winner, -self.game.score(*pid), *pid));
        let placements: Vec<(String, String)> = rated
            .iter()
            .map(|pid| {
                (
                    seat_names[*pid].clone().unwrap(),
                    self.game.players[*pid].civ.clone(),
                )
            })
            .collect();
        let victory = self.game.victory_type.clone().unwrap_or_default();
        let Some(updated) = crate::league::record_game(
            &dir,
            &placements,
            self.game.seed,
            self.game.turn,
            &victory,
        ) else {
            eprintln!("[league] could not rate this game into {dir}");
            return;
        };
        // Show the new numbers for the rest of the results screen, and let the
        // next game seat from them.
        for (pid, slot) in self.seat_strategy.iter_mut().enumerate() {
            let Some(name) = &seat_names[pid] else {
                *slot = None;
                continue;
            };
            *slot = updated.strategies.iter().position(|s| &s.name == name);
        }
        self.league = Some(updated);
    }

    pub fn step_many(&mut self, count: usize) -> Vec<SpectatorStep> {
        let mut steps = Vec::new();
        for _ in 0..count.clamp(1, 12) {
            steps.push(self.spectator_step());
            if self.game.winner.is_some() {
                break;
            }
        }
        steps
    }

    /// Hand the player's own seat to the AI for `turns` turns.
    ///
    /// Unciv calls this AutoPlay, and it earns its keep in the same two
    /// places: skipping a stretch of a game that has already been decided,
    /// and watching how the agent would have played a position you are in.
    /// Seat 0 already has an agent built for it — in a human game it simply
    /// never gets asked — so this is a matter of asking it.
    pub fn autoplay(&mut self, turns: u32) -> usize {
        let mut played = 0;
        for _ in 0..turns.min(500) {
            if self.game.winner.is_some() || !self.game.players[0].alive {
                break;
            }
            self.ais[0].take_turn(&mut self.game, 0);
            if self.game.current == 0 && self.game.winner.is_none() {
                let _ = self.game.apply(0, &Action::EndTurn);
            }
            let g = &mut self.game;
            let mut guard = 0;
            while g.winner.is_none()
                && g.current != 0
                && g.players[0].alive
                && guard < 2 * g.players.len()
            {
                let pid = g.current;
                self.ais[pid].take_turn(g, pid);
                if g.current == pid && g.winner.is_none() {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
                guard += 1;
            }
            self.record_league_result();
            played += 1;
        }
        played
    }
    pub fn act(&mut self, v: &Value) -> Option<String> {
        let action: Action = match serde_json::from_value(v.clone()) {
            Ok(a) => a,
            Err(e) => return Some(format!("bad action: {e}")),
        };
        if let Err(e) = self.game.apply(0, &action) {
            return Some(e);
        }
        if matches!(action, Action::EndTurn) {
            let g = &mut self.game;
            let mut guard = 0;
            while g.winner.is_none()
                && g.current != 0
                && g.players[0].alive
                && guard < 2 * g.players.len()
            {
                let pid = g.current;
                self.ais[pid].take_turn(g, pid);
                if g.current == pid && g.winner.is_none() {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
                guard += 1;
            }
            self.record_league_result();
        }
        None
    }
}

fn index_html() -> Vec<u8> {
    for p in ["web/index.html"] {
        if let Ok(b) = std::fs::read(p) {
            return b;
        }
    }
    EMBEDDED_INDEX.as_bytes().to_vec()
}

fn cinematic_3d_js() -> Vec<u8> {
    std::fs::read("web/cinematic3d.js")
        .unwrap_or_else(|_| EMBEDDED_CINEMATIC_3D.as_bytes().to_vec())
}

fn terrain_atlas() -> Vec<u8> {
    std::fs::read("web/assets/terrain-atlas.png")
        .unwrap_or_else(|_| EMBEDDED_TERRAIN_ATLAS.to_vec())
}

fn feature_atlas() -> Vec<u8> {
    std::fs::read("web/assets/feature-atlas.png")
        .unwrap_or_else(|_| EMBEDDED_FEATURE_ATLAS.to_vec())
}

fn environment_feature_atlas() -> Vec<u8> {
    std::fs::read("web/assets/environment-feature-atlas.png")
        .unwrap_or_else(|_| EMBEDDED_ENVIRONMENT_FEATURE_ATLAS.to_vec())
}

fn natural_wonder_atlas() -> Vec<u8> {
    std::fs::read("web/assets/natural-wonder-atlas.png")
        .unwrap_or_else(|_| EMBEDDED_NATURAL_WONDER_ATLAS.to_vec())
}

fn world_wonder_atlas() -> Vec<u8> {
    std::fs::read("web/assets/world-wonder-atlas.png")
        .unwrap_or_else(|_| EMBEDDED_WORLD_WONDER_ATLAS.to_vec())
}

fn mountain_atlas() -> Vec<u8> {
    std::fs::read("web/assets/mountain-atlas.png")
        .unwrap_or_else(|_| EMBEDDED_MOUNTAIN_ATLAS.to_vec())
}

/// Write one complete HTTP response, returning whether it reached the socket.
/// Callers normally have nothing useful to do with a disconnected client, but
/// completed-turn delivery uses the result as its release acknowledgement.
fn respond(stream: &mut TcpStream, code: &str, ctype: &str, body: &[u8]) -> bool {
    // Nothing this server sends is worth reusing from a cache. The page and
    // its art are compiled into the binary, so a build swap changes them
    // underneath an open tab - and with no cache headers at all a browser was
    // free to keep serving the copy it already had, which made a new engine
    // look like it was still running yesterday's GUI. The state feeds change
    // every turn by definition.
    let head = format!(
        "HTTP/1.1 {code}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store, must-revalidate\r\nPragma: no-cache\r\n\
         Connection: close\r\n\r\n",
        body.len());
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body))
        .and_then(|()| stream.flush())
        .is_ok()
}

fn respond_json(stream: &mut TcpStream, v: &Value) -> bool {
    respond(
        stream,
        "200 OK",
        "application/json",
        v.to_string().as_bytes(),
    )
}

fn request_path(target: &str) -> &str {
    target.split_once('?').map_or(target, |(path, _)| path)
}

fn simulation_settings(params: &Params) -> Value {
    let victories = [
        (params.victory_conditions.science, "science"),
        (params.victory_conditions.culture, "culture"),
        (params.victory_conditions.religious, "religious"),
        (params.victory_conditions.diplomatic, "diplomatic"),
        (params.victory_conditions.domination, "domination"),
        (params.victory_conditions.score, "score"),
    ]
    .into_iter()
    .filter_map(|(enabled, name)| enabled.then_some(name))
    .collect::<Vec<_>>();
    json!({
        "players": params.num_players,
        "width": params.width,
        "height": params.height,
        "city_states": params.num_city_states,
        "turns": params.max_turns,
        "map": params.map_script.id(),
        "speed": params.game_speed.id(),
        "victories": victories,
    })
}

fn new_game_params(current: &Params, request: &Value) -> Params {
    let mut p = current.clone();
    if let Some(v) = request["num_players"].as_u64() {
        p.num_players = v as usize;
        p.teams.clear();
        let size = MapSize::for_players(p.num_players);
        p.width = size.width;
        p.height = size.height;
        p.num_city_states = size.default_city_states;
    }
    if let Some(v) = request["seed"].as_u64() {
        p.seed = v;
    }
    if let Some(v) = request["map_script"].as_str().and_then(MapScript::from_id) {
        p.map_script = v;
    }
    if let Some(v) = request["game_speed"].as_str().and_then(GameSpeed::from_id) {
        p.game_speed = v;
        p.speed = v.id().to_string();
        p.max_turns = v.turn_limit();
    }
    if let Some(v) = request["max_turns"].as_u64() {
        p.max_turns = v as u32;
    }
    // The two settings a Civ 6 lobby asks for that this protocol could not
    // carry: how hard the rivals play, and who the player is. Both are
    // validated against the live ruleset rather than trusted, because the
    // constructor asserts on an unknown difficulty and would take the server
    // down with it.
    if let Some(difficulty) = request["difficulty"].as_str() {
        if Rules::shared().difficulties.contains_key(difficulty) {
            p.difficulty = difficulty.to_string();
        }
    }
    if let Some(civs) = request["civs"].as_array() {
        let rules = Rules::shared();
        p.civs = civs
            .iter()
            .filter_map(|civ| civ.as_str())
            .filter(|civ| rules.civs.contains_key(*civ))
            .map(str::to_string)
            .collect();
    }
    if let Some(victories) = request["victory_conditions"].as_object() {
        for (name, enabled) in victories {
            let Some(enabled) = enabled.as_bool() else {
                continue;
            };
            match name.as_str() {
                "science" => p.victory_conditions.science = enabled,
                "culture" => p.victory_conditions.culture = enabled,
                "religious" => p.victory_conditions.religious = enabled,
                "diplomatic" => p.victory_conditions.diplomatic = enabled,
                "domination" => p.victory_conditions.domination = enabled,
                "score" => p.victory_conditions.score = enabled,
                _ => {}
            }
        }
    }
    // Advanced clients can still deliberately override individual stock
    // settings by sending them alongside num_players.
    if let Some(v) = request["width"].as_i64() {
        p.width = v as i32;
    }
    if let Some(v) = request["height"].as_i64() {
        p.height = v as i32;
    }
    if let Some(v) = request["num_city_states"].as_u64() {
        p.num_city_states = v as usize;
    }
    if let Some(v) = request["spectate"].as_bool() {
        p.spectate = v;
    }
    if let Some(teams) = request["teams"].as_array() {
        let parsed = teams
            .iter()
            .map(|team| team.as_u64().map(|team| team as usize))
            .collect::<Vec<_>>();
        if parsed.len() == p.num_players {
            p.teams = parsed;
        }
    }
    let rules = Rules::embedded();
    if let Some(v) = request["difficulty"].as_str() {
        if rules.difficulties.contains_key(v) {
            p.difficulty = v.to_string();
        }
    }
    if let Some(v) = request["speed"].as_str() {
        if let Some(spec) = rules.speeds.get(v) {
            p.speed = v.to_string();
            p.game_speed = GameSpeed::from_id(v).unwrap_or(GameSpeed::Standard);
            // A speed carries its own turn budget; adopt it unless the client
            // asked for a specific one in the same request.
            p.max_turns = request["max_turns"].as_u64().unwrap_or(spec.turns as u64) as u32;
        }
    }
    p
}

fn auto_step_loop(sh: Arc<Shared>) {
    let mut over_since: Option<Instant> = None;
    let mut watched_turn: Option<u32> = None;
    let mut turn_mark = Instant::now();
    let mut turn_compute_us: u64 = 0;
    let mut unlimited_since = Instant::now();
    let mut timed_pace = u64::MAX;
    loop {
        let pace = sh.pace_ms.load(Ordering::Relaxed).min(60_000);
        if pace != timed_pace {
            // The turn in flight was paced two ways; time the next one whole,
            // or the readout spends a dozen turns crawling toward the truth.
            timed_pace = pace;
            watched_turn = None;
        }
        if sh.paused.load(Ordering::Relaxed) {
            over_since = None; // pausing resets the restart countdown
            watched_turn = None; // and voids the half-timed turn
            std::thread::sleep(Duration::from_millis(150));
            continue;
        }
        let cadence_started = Instant::now();
        let delay; // this seat's slice of the turn budget
        let mut waiting = false; // between games nothing is being simulated
        let mut completed_frame = None;
        {
            let mut s = sh.session.lock().unwrap();
            if !s.params.spectate {
                drop(s);
                std::thread::sleep(Duration::from_millis(300));
                continue;
            }
            if s.game.winner.is_some() {
                let t0 = *over_since.get_or_insert_with(Instant::now);
                let left = final_countdown_ms(s.params.restart_ms)
                    .saturating_sub(t0.elapsed().as_millis() as u64);
                sh.restart_in.store(left, Ordering::Relaxed);
                if left == 0 {
                    s.start_automatic_next_game();
                    over_since = None;
                    watched_turn = None;
                    sh.restart_in.store(u64::MAX, Ordering::Relaxed);
                }
                delay = 200;
                waiting = true;
            } else {
                over_since = None;
                sh.restart_in.store(u64::MAX, Ordering::Relaxed);
                let step_started = Instant::now();
                let turn_before = s.game.turn;
                let (pid, _) = s.step();
                turn_compute_us += step_started.elapsed().as_micros() as u64;
                if s.game.turn != turn_before {
                    completed_frame = Some(SpectatorFrame {
                        seed: s.game.seed,
                        turn: s.game.turn,
                    });
                }
                // A turn is one step per seat, so a seat waits for its own
                // share of the turn budget and the round adds up to the pace.
                // Only the living take a step: counting the eliminated made a
                // late game outrun its own pace as the city-states fell.
                let living: Vec<_> = s.game.players.iter().filter(|p| p.alive).collect();
                let minors = living
                    .iter()
                    .filter(|p| p.is_minor || p.is_barbarian)
                    .count();
                let majors = living.len() - minors;
                let p = &s.game.players[pid];
                delay = seat_delay_ms(pace, majors, minors, p.is_minor || p.is_barbarian);
                // The seat that ends the round closes the turn being timed.
                let turn = s.game.turn;
                if watched_turn != Some(turn) {
                    if watched_turn.is_some() {
                        blend(&sh.turn_us, turn_mark.elapsed().as_micros() as u64);
                        blend(&sh.turn_compute_us, turn_compute_us);
                    }
                    watched_turn = Some(turn);
                    turn_mark = Instant::now();
                    turn_compute_us = 0;
                }
            }
        }
        if pace == 0 && !waiting {
            // An active browser consumes exactly one completed-turn state
            // before the next round starts. This makes Lightning as fast as
            // the viewer can paint without letting it skip whole turns. With
            // no recent viewer, the exhibition remains truly unlimited.
            if let Some(frame) = completed_frame {
                sh.wait_for_lightning_frame(frame);
            }
            // Unlimited: no wait between steps. Yield anyway, and give the
            // single-threaded accept loop a real slot a few times a second,
            // or /state would starve behind the session lock.
            if unlimited_since.elapsed() >= Duration::from_millis(UNLIMITED_BREATH_MS) {
                unlimited_since = Instant::now();
                std::thread::sleep(Duration::from_millis(1));
            } else {
                std::thread::yield_now();
            }
            continue;
        }
        unlimited_since = Instant::now();
        // Pace is a start-to-start cadence. Sleeping the full interval after
        // AI computation made the fast paces visibly slower as empires grew.
        // Spend only the remaining frame budget instead.
        let elapsed_ms = cadence_started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        std::thread::sleep(Duration::from_millis(delay.saturating_sub(elapsed_ms).max(1)));
    }
}

/// Attach exhibition metadata (restart countdown, pace, paused) to a state.
fn decorate(o: &mut Value, sh: &Shared) {
    let r = sh.restart_in.load(Ordering::Relaxed);
    if r != u64::MAX {
        o["restart_in"] = json!(r.div_ceil(1000));
    }
    o["pace"] = json!(sh.pace_ms.load(Ordering::Relaxed));
    o["paused"] = json!(sh.paused.load(Ordering::Relaxed));
    // Both in milliseconds per game turn: what the current pace is actually
    // delivering, and what it would cost with every wait removed.
    let measured = sh.turn_us.load(Ordering::Relaxed);
    let compute = sh.turn_compute_us.load(Ordering::Relaxed);
    if measured > 0 {
        o["turn_ms"] = json!(measured as f64 / 1000.0);
    }
    if compute > 0 {
        o["turn_compute_ms"] = json!(compute as f64 / 1000.0);
    }
}

fn handle(stream: &mut TcpStream, sh: &Shared) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.is_empty() {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    // Route on the URL path, not its cache-busting/query component. The
    // supervised spectator tags each successor URL with its server instance
    // so a long-lived tab loads fresh embedded assets after a binary swap.
    let request_target = parts.next().unwrap_or("/");
    let path = request_path(request_target).to_string();
    let mut content_len = 0usize;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).is_err() || h == "\r\n" || h == "\n" || h.is_empty() {
            break;
        }
        let hl = h.to_ascii_lowercase();
        if let Some(v) = hl.strip_prefix("content-length:") {
            content_len = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; content_len];
    if content_len > 0 {
        let _ = reader.read_exact(&mut body);
    }
    let parsed: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

    match (method.as_str(), path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => {
            respond(stream, "200 OK", "text/html; charset=utf-8", &index_html());
        }
        ("GET", "/cinematic3d.js") => {
            respond(
                stream,
                "200 OK",
                "text/javascript; charset=utf-8",
                &cinematic_3d_js(),
            );
        }
        ("GET", "/assets/terrain-atlas.png") => {
            respond(stream, "200 OK", "image/png", &terrain_atlas());
        }
        ("GET", "/assets/feature-atlas.png") => {
            respond(stream, "200 OK", "image/png", &feature_atlas());
        }
        ("GET", "/assets/environment-feature-atlas.png") => {
            respond(stream, "200 OK", "image/png", &environment_feature_atlas());
        }
        ("GET", "/assets/natural-wonder-atlas.png") => {
            respond(stream, "200 OK", "image/png", &natural_wonder_atlas());
        }
        ("GET", "/assets/world-wonder-atlas.png") => {
            respond(stream, "200 OK", "image/png", &world_wonder_atlas());
        }
        ("GET", "/assets/mountain-atlas.png") => {
            respond(stream, "200 OK", "image/png", &mountain_atlas());
        }
        ("GET", "/state") => {
            sh.note_frame_request();
            let (mut o, frame) = {
                let session = sh.session.lock().unwrap();
                let frame = SpectatorFrame {
                    seed: session.game.seed,
                    turn: session.game.turn,
                };
                (session.state(), frame)
            };
            decorate(&mut o, sh);
            if respond_json(stream, &o) {
                // Release Lightning only after the completed-turn snapshot is
                // on the wire. The browser renders every successful `/state`
                // response as one synchronous map + HUD update.
                sh.note_frame_delivered(frame);
            }
        }
        // Everything a supervisor needs to know - is there a game, is it over -
        // without building the whole observation. /state runs close to a
        // megabyte of JSON on a standard map, and something polling it every
        // few seconds to read one field spends the server's time on rendering
        // a view nobody looks at.
        ("GET", "/status") => {
            let session = sh.session.lock().unwrap();
            let game = &session.game;
            respond_json(
                stream,
                &json!({
                    "turn": game.turn,
                    "winner": game.winner,
                    "victory_type": game.victory_type,
                    "spectate": session.params.spectate,
                    // Which code is actually playing. A binary swap only
                    // happens between games, so a running server is always
                    // somewhat behind origin/main and there was no way to see
                    // by how much - "is it running old code" could only be
                    // guessed at from file timestamps. The build stamps this
                    // in; an unstamped build reports unknown.
                    "commit": option_env!("CIVVIS_COMMIT").unwrap_or("unknown"),
                }),
            );
        }
        ("POST", "/pace") => {
            if let Some(v) = parsed["ms"].as_u64() {
                // 0 is the unlimited pace; anything else is a turn budget.
                sh.pace_ms.store(v.min(60_000), Ordering::Relaxed);
                sh.turn_us.store(0, Ordering::Relaxed); // re-measure at the new pace
            }
            if let Some(v) = parsed["paused"].as_bool() {
                sh.paused.store(v, Ordering::Relaxed);
            }
            let mut session = sh.session.lock().unwrap();
            if let Some(v) = parsed["paused"].as_bool() {
                session.spectator_paused = v;
            }
            let mut o = session.state();
            drop(session);
            decorate(&mut o, sh);
            respond_json(stream, &o);
        }
        ("GET", "/save") => {
            let session = sh.session.lock().unwrap();
            let save = serde_json::to_value(&session.game).unwrap();
            respond_json(stream, &save);
        }
        ("GET", "/rules") => {
            let session = sh.session.lock().unwrap();
            let r = &session.game.rules;
            respond_json(
                stream,
                &json!({
                    "techs": r.techs, "civics": r.civics,
                    "terrains": r.terrains, "features": r.features,
                    "resources": r.resources, "improvements": r.improvements,
                    "governments": r.governments, "units": r.units,
                    "promotions": r.promotions,
                    "buildings": r.buildings, "districts": r.districts,
                    "wonders": r.wonders,
                    "projects": r.projects,
                    "policies": r.policies, "beliefs": r.beliefs, "civs": r.civs,
                    "great_people": r.great_people, "governors": r.governors,
                    "map_sizes": CIV6_MAP_SIZES,
                    "difficulties": r.difficulties, "speeds": r.speeds,
                    "map_scripts": CIV6_MAP_SCRIPTS,
                    "game_speeds": CIV6_GAME_SPEEDS,
                }),
            );
        }
        ("POST", "/autoplay") => {
            let mut session = sh.session.lock().unwrap();
            if session.params.spectate {
                drop(session);
                respond_json(stream, &json!({"error": "a spectated game is already playing itself"}));
                return;
            }
            let turns = parsed["turns"].as_u64().unwrap_or(1).clamp(1, 500) as u32;
            let played = session.autoplay(turns);
            let mut out = session.state();
            out["autoplayed"] = json!(played);
            drop(session);
            decorate(&mut out, sh);
            respond_json(stream, &out);
        }
        ("GET", "/pedia") => {
            // Generated from the ruleset in play, mods included, so the GUI
            // reference never disagrees with the game it is attached to.
            let session = sh.session.lock().unwrap();
            let entries = crate::pedia::entries(&session.game.rules);
            drop(session);
            respond_json(stream, &json!({ "entries": entries }));
        }
        ("POST", "/action") => {
            let mut session = sh.session.lock().unwrap();
            let movement_path = serde_json::from_value::<Action>(parsed["action"].clone())
                .ok()
                .and_then(|action| match action {
                    Action::MoveTo { unit, to } => {
                        let start = session.game.units.get(&unit)?.pos;
                        let mut path = session.game.path_to(unit, to)?;
                        path.insert(0, start);
                        Some((unit, path))
                    }
                    _ => None,
                });
            let err = session.act(&parsed["action"]);
            let mut out = session.state();
            if err.is_none() {
                if let Some((unit, mut path)) = movement_path {
                    if let Some(actual) = session.game.units.get(&unit).map(|unit| unit.pos) {
                        if let Some(end) = path.iter().position(|position| *position == actual) {
                            path.truncate(end + 1);
                        } else if let Some(start) = path.first().copied() {
                            path = vec![start, actual];
                        }
                    }
                    if path.len() > 1 {
                        out["movement_paths"] = json!({unit.to_string(): path});
                    }
                }
            }
            out["error"] = match err {
                Some(e) => Value::String(e),
                None => Value::Null,
            };
            respond_json(stream, &out);
        }
        ("POST", "/step") => {
            let mut session = sh.session.lock().unwrap();
            let mut out;
            if session.params.spectate {
                let count = parsed["count"].as_u64().unwrap_or(1) as usize;
                let steps = session.step_many(count);
                out = session.state();
                // An omniscient observer can narrate every AI decision. A
                // civilization view only receives that civilization's own
                // traces; otherwise hidden movement and combat would bypass
                // the map fog through the event chronicle.
                let visible_steps: Vec<_> = steps
                    .iter()
                    .filter(|step| {
                        session
                            .view_player
                            .is_none_or(|viewer| step.player == viewer)
                    })
                    .collect();
                if let Some(step) = visible_steps.last() {
                    // Preserve the original single-step response fields for
                    // existing clients and supervisor recovery nudges.
                    out["stepped"] = json!(step.player);
                    out["actions_taken"] = serde_json::to_value(&step.actions).unwrap();
                }
                out["step_batches"] = Value::Array(
                    visible_steps
                        .iter()
                        .map(|step| {
                            json!({
                                "stepped": step.player,
                                "actions_taken": step.actions,
                                "world_events": if session.view_player.is_none() {
                                    step.world_events.clone()
                                } else {
                                    Vec::new()
                                },
                            })
                        })
                        .collect(),
                );
            } else {
                out = session.state();
                out["error"] = json!("not in spectate mode");
            }
            drop(session);
            decorate(&mut out, sh);
            respond_json(stream, &out);
        }
        ("POST", "/view") => {
            let mut session = sh.session.lock().unwrap();
            let result = match parsed.get("player") {
                Some(Value::Null) => session.set_view_player(None),
                Some(value) => value
                    .as_u64()
                    .ok_or_else(|| "player must be a non-negative integer or null".to_string())
                    .and_then(|pid| session.set_view_player(Some(pid as usize))),
                None => Err("missing player".to_string()),
            };
            let mut out = session.state();
            out["error"] = match result {
                Ok(()) => Value::Null,
                Err(error) => Value::String(error),
            };
            respond_json(stream, &out);
        }
        ("POST", "/spectator-status") => {
            let mut session = sh.session.lock().unwrap();
            if session.params.spectate {
                if let Some(paused) = parsed["paused"].as_bool() {
                    session.spectator_paused = paused;
                }
                respond_json(stream, &json!({"ok": true}));
            } else {
                respond_json(stream, &json!({"error": "not in spectate mode"}));
            }
        }
        ("POST", "/next-game-settings") => {
            let mut session = sh.session.lock().unwrap();
            session.stage_next_game_settings(&parsed);
            respond_json(
                stream,
                &json!({
                    "ok": true,
                    "next_game_settings": session
                        .next_game_params
                        .as_ref()
                        .map(simulation_settings)
                        .unwrap_or(Value::Null),
                }),
            );
        }
        ("POST", "/new") => {
            let mut session = sh.session.lock().unwrap();
            let result = session.start_new_game(&parsed);
            if result.is_ok() {
                let paused = parsed["paused"]
                    .as_bool()
                    .unwrap_or_else(|| sh.paused.load(Ordering::Relaxed));
                sh.paused.store(paused, Ordering::Relaxed);
                session.spectator_paused = paused;
            }
            let mut o = session.state();
            o["error"] = match result {
                Ok(()) => Value::Null,
                Err(error) => Value::String(error),
            };
            drop(session);
            decorate(&mut o, sh);
            respond_json(stream, &o);
        }
        ("POST", "/supervisor-new") => {
            let mut session = sh.session.lock().unwrap();
            let result = session.request_supervised_new_game(&parsed);
            let mut out = session.state();
            out["error"] = match result {
                Ok(()) => Value::Null,
                Err(error) => Value::String(error),
            };
            drop(session);
            decorate(&mut out, sh);
            respond_json(stream, &out);
        }
        _ => {
            respond(
                stream,
                "404 Not Found",
                "application/json",
                b"{\"error\":\"not found\"}",
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{
        chronicle_world_events, final_countdown_ms, new_game_params, request_path, seat_delay_ms,
        ChronicleSnapshot, ChronicleState, FrameDelivery, Params, Session, SpectatorFrame,
        EMBEDDED_CINEMATIC_3D, EMBEDDED_INDEX, EMBEDDED_WORLD_WONDER_ATLAS, VIEWER_ACTIVE,
    };
    use crate::game::{Action, VictoryConditions};
    use crate::setup::{GameSpeed, MapScript};
    use serde_json::json;
    use std::time::{Duration, Instant};

    /// The pace a viewer picks is what a turn costs, so the seats' waits have
    /// to add back up to it — at any player count, with minors on their
    /// quarter beat. A per-seat pace made big games crawl at the same label.
    #[test]
    fn seat_waits_add_up_to_the_chosen_turn_pace() {
        for (majors, minors) in [(2, 0), (4, 4), (8, 12), (6, 3)] {
            for pace in [100, 1_000, 4_000, 10_000] {
                let round = majors as u64 * seat_delay_ms(pace, majors, minors, false)
                    + minors as u64 * seat_delay_ms(pace, majors, minors, true);
                // Each seat rounds to whole milliseconds; nothing beyond that.
                let allowed = (majors + minors) as u64 / 2 + pace / 100 + 1;
                let drift = round.abs_diff(pace);
                assert!(
                    drift <= allowed,
                    "{majors}+{minors} seats at {pace}ms spent {round}ms on a turn"
                );
            }
        }
        // Minors take a quarter of a major's slice, and unlimited never waits.
        assert_eq!(seat_delay_ms(1_000, 4, 4, false) / 4, seat_delay_ms(1_000, 4, 4, true));
        assert_eq!(seat_delay_ms(0, 8, 12, false), 0);
    }

    #[test]
    fn final_countdown_is_five_seconds_unless_longer_is_requested() {
        assert_eq!(final_countdown_ms(0), 5_000);
        assert_eq!(final_countdown_ms(4_999), 5_000);
        assert_eq!(final_countdown_ms(5_000), 5_000);
        assert_eq!(final_countdown_ms(12_500), 12_500);
    }

    #[test]
    fn lightning_waits_for_each_turn_only_while_a_viewer_is_active() {
        let now = Instant::now();
        let turn_7 = SpectatorFrame { seed: 41, turn: 7 };
        let turn_8 = SpectatorFrame { seed: 41, turn: 8 };
        let next_world = SpectatorFrame { seed: 42, turn: 7 };
        let mut delivery = FrameDelivery::default();

        assert_eq!(delivery.wait_remaining(turn_7, now), None);

        delivery.request_started(now);
        assert_eq!(delivery.wait_remaining(turn_7, now), Some(VIEWER_ACTIVE));

        delivery.frame_delivered(turn_7, now + Duration::from_millis(20));
        assert_eq!(delivery.wait_remaining(turn_7, now), None);
        assert!(delivery.wait_remaining(turn_8, now).is_some());
        assert!(delivery.wait_remaining(next_world, now).is_some());

        assert_eq!(
            delivery.wait_remaining(turn_8, now + Duration::from_millis(20) + VIEWER_ACTIVE),
            None
        );
        assert_eq!(
            delivery.wait_remaining(turn_8, now + VIEWER_ACTIVE + Duration::from_millis(21)),
            None
        );
    }

    #[test]
    fn browser_renders_each_delivered_state_as_one_complete_frame() {
        let render = EMBEDDED_INDEX
            .split_once("function render(st, recordChronicle = true) {")
            .expect("browser render function")
            .1
            .split_once("\nfunction drawCaptureChoice()")
            .expect("end of browser render function")
            .0;
        let state_assignment = render.find("state = st;").expect("install delivered state");
        let full_frame = render
            .find(
                "draw(); drawSide(newWorld); drawMini(); drawPlayerHud(); drawUbar(); drawQuickDeals(); drawCaptureChoice();",
            )
            .expect("map, minimap, HUDs, and controls must repaint together");
        assert!(state_assignment < full_frame);

        let victory_hud = EMBEDDED_INDEX
            .split_once("function playerHudOverview() {")
            .expect("victory tracker renderer")
            .1
            .split_once("\nfunction spectatorIdentity(player)")
            .expect("end of victory tracker renderer")
            .0;
        assert!(victory_hud.contains("victoryMetric(player, track.id)"));
        assert!(victory_hud.contains("<strong>${state.turn}</strong>"));

        let player_hud = EMBEDDED_INDEX
            .split_once("function drawPlayerHud() {")
            .expect("player HUD renderer")
            .1
            .split_once("\n// CSS mode changes")
            .expect("end of player HUD renderer")
            .0;
        assert!(player_hud.contains("const overview = playerHudOverview();"));
        assert!(player_hud.contains("state.players"));
        assert!(player_hud.contains("playerHudStats(p,"));
        assert!(player_hud.contains("victoryHud.innerHTML = overview;"));
        assert!(player_hud.contains("hud.innerHTML = html;"));
    }

    fn current() -> Params {
        Params {
            num_players: 2,
            width: 20,
            height: 14,
            seed: 1,
            map_script: MapScript::Pangaea,
            game_speed: GameSpeed::Standard,
            max_turns: 500,
            victory_conditions: VictoryConditions::default(),
            num_city_states: 1,
            spectate: false,
            difficulty: crate::game::default_difficulty(),
            speed: crate::game::default_speed(),
            teams: Vec::new(),
            civs: Vec::new(),
            supervised: false,
            restart_ms: 5_000,
            league_dir: None,
            league_record: false,
        }
    }

    /// A Civ 6 lobby asks two things this protocol could not carry: how hard
    /// the rivals play, and who the player is. Both are validated against the
    /// live ruleset — `Game::new_with` asserts on an unknown difficulty, and
    /// a request is not a trusted caller.
    #[test]
    fn new_game_takes_a_difficulty_and_a_leader_and_refuses_nonsense() {
        let current = current();
        let next = new_game_params(&current, &json!({"difficulty": "deity"}));
        assert_eq!(next.difficulty, "deity");

        let ignored = new_game_params(&current, &json!({"difficulty": "impossible"}));
        assert_eq!(ignored.difficulty, current.difficulty);

        let seated = new_game_params(&current, &json!({"civs": ["Egypt", "Nowhere", "Greece"]}));
        assert_eq!(seated.civs, vec!["Egypt".to_string(), "Greece".to_string()]);

        // The chosen civilization reaches the seat, and nobody else is given
        // the same one.
        let mut params = current;
        params.num_players = 4;
        params.civs = vec!["Egypt".to_string()];
        let session = Session::new(params);
        assert_eq!(session.game.players[0].civ, "Egypt");
        let majors: Vec<&str> = session
            .game
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .map(|player| player.civ.as_str())
            .collect();
        assert_eq!(majors.len(), 4);
        let unique: std::collections::BTreeSet<&str> = majors.iter().copied().collect();
        assert_eq!(unique.len(), 4, "two majors were seated as {majors:?}");
    }

    #[test]
    fn new_game_player_count_applies_the_whole_civ6_size_profile() {
        let expected = [
            (2, 44, 26, 3),
            (4, 60, 38, 6),
            (6, 74, 46, 9),
            (8, 84, 54, 12),
            (10, 96, 60, 15),
            (12, 106, 66, 18),
        ];
        let mut params = current();
        for (players, width, height, city_states) in expected {
            params = new_game_params(&params, &json!({"num_players": players}));
            assert_eq!(params.num_players, players);
            assert_eq!(
                (params.width, params.height, params.num_city_states),
                (width, height, city_states)
            );
        }
    }

    #[test]
    fn explicit_advanced_overrides_win_over_the_profile() {
        let p = new_game_params(
            &current(),
            &json!({
                "num_players": 6,
                "width": 80,
                "height": 50,
                "num_city_states": 2
            }),
        );
        assert_eq!((p.width, p.height, p.num_city_states), (80, 50, 2));
    }

    #[test]
    fn map_and_speed_choices_update_the_complete_setup() {
        let p = new_game_params(
            &current(),
            &json!({"map_script": "inland_sea", "game_speed": "online"}),
        );
        assert_eq!(p.map_script, MapScript::InlandSea);
        assert_eq!(p.game_speed, GameSpeed::Online);
        assert_eq!(p.max_turns, 250);

        let custom = new_game_params(
            &current(),
            &json!({"game_speed": "marathon", "max_turns": 99}),
        );
        assert_eq!(custom.game_speed, GameSpeed::Marathon);
        assert_eq!(custom.max_turns, 99);
    }

    #[test]
    fn new_game_applies_each_victory_condition_setting() {
        let disabled = json!({
            "science": false,
            "culture": false,
            "religious": false,
            "diplomatic": false,
            "domination": false,
            "score": false
        });
        let params = new_game_params(&current(), &json!({"victory_conditions": disabled.clone()}));
        assert_eq!(
            params.victory_conditions,
            VictoryConditions {
                science: false,
                culture: false,
                religious: false,
                diplomatic: false,
                domination: false,
                score: false,
            }
        );

        let session = Session::new(params.clone());
        assert_eq!(session.game.victory_conditions, params.victory_conditions);
        assert_eq!(session.state()["victory_conditions"], disabled);
    }

    #[test]
    fn omitted_victory_settings_preserve_the_current_selection() {
        let mut current = current();
        current.victory_conditions.culture = false;
        current.victory_conditions.score = false;
        let next = new_game_params(&current, &json!({"seed": 2}));
        assert!(!next.victory_conditions.culture);
        assert!(!next.victory_conditions.score);
        assert!(next.victory_conditions.science);
    }

    /// The client is one top-level script, so a single lookup of an element
    /// that no longer exists does not fail locally — it throws, and every
    /// statement after it, including `boot()`, never runs. The page then loads
    /// as an empty map with dead buttons. That is exactly what happened when a
    /// button was removed from the markup and its `onclick` binding was left
    /// behind, and it takes the spectator down with the human game.
    ///
    /// A lookup that is immediately used (`.onclick`, `.value`, `[0]`) must
    /// therefore name an id the page actually declares. Lookups stored first
    /// and guarded with `if (element)` are deliberately optional and exempt.
    #[test]
    fn browser_never_binds_an_element_that_does_not_exist() {
        let declared: std::collections::HashSet<&str> = EMBEDDED_INDEX
            .match_indices("id=\"")
            .filter_map(|(at, marker)| {
                let rest = &EMBEDDED_INDEX[at + marker.len()..];
                rest.find('"').map(|end| &rest[..end])
            })
            .collect();
        let lookup = "getElementById(\"";
        for (at, marker) in EMBEDDED_INDEX.match_indices(lookup) {
            let rest = &EMBEDDED_INDEX[at + marker.len()..];
            let Some(end) = rest.find('"') else { continue };
            let id = &rest[..end];
            // `")` closes the call; what follows decides whether the result is
            // used on the spot or bound to a name that can be checked first.
            let used_now = rest[end..]
                .strip_prefix("\")")
                .map(|after| after.starts_with('.') || after.starts_with('['))
                .unwrap_or(false);
            assert!(
                !used_now || declared.contains(id),
                "the browser binds #{id} directly, but no element declares that id — \
                 the whole client script dies at that line"
            );
        }
    }

    #[test]
    fn browser_orders_settings_event_log_and_strategy() {
        for players in [2, 4, 6, 8, 10, 12] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("<option value=\"{players}\"")),
                "browser setup is missing the {players}-player map size"
            );
        }
        assert!(EMBEDDED_INDEX.contains("RULES.map_sizes.map(size =>"));
        assert!(EMBEDDED_INDEX.contains("RULES.map_scripts.map(script =>"));
        assert!(EMBEDDED_INDEX.contains("RULES.game_speeds.map(speed =>"));
        assert!(EMBEDDED_INDEX.contains("id=\"gamemode\""));
        assert!(EMBEDDED_INDEX.contains("id=\"maptype\""));
        assert!(EMBEDDED_INDEX.contains("id=\"gamespeed\""));
        for victory in [
            "science",
            "culture",
            "religious",
            "diplomatic",
            "domination",
            "score",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("id=\"victory-{victory}\"")),
                "browser setup is missing the {victory} victory checkbox"
            );
        }
        assert!(EMBEDDED_INDEX.contains("victory_conditions: victoryConditions"));
        assert!(EMBEDDED_INDEX.contains("AI-only simulation"));
        // Single player is no longer "later": it is the default mode, and it
        // is the only one that offers a leader and a difficulty.
        assert!(EMBEDDED_INDEX.contains("<option value=\"single\" selected>Single player</option>"));
        assert!(!EMBEDDED_INDEX.contains("Single player · later"));
        assert!(EMBEDDED_INDEX.contains("Multiplayer · later"));
        assert!(EMBEDDED_INDEX.contains(
            "id=\"restart-sim\" title=\"Restart with the same settings\">Restart sim<span class=\"sub\">same settings</span>"
        ));
        assert!(!EMBEDDED_INDEX.contains("id=\"fresh-sim\""));
        assert!(!EMBEDDED_INDEX.contains("id=\"default-settings\""));
        assert!(!EMBEDDED_INDEX.contains("id=\"specstep\""));
        assert!(!EMBEDDED_INDEX.contains("id=\"specdirector\""));
        assert!(!EMBEDDED_INDEX.contains("id=\"speccinema\""));
        assert!(EMBEDDED_INDEX.contains("async function startNewSimulation()"));
        assert!(EMBEDDED_INDEX
            .contains("const payload = {...newSimulationPayload(), paused: wasPaused}"));
        assert!(EMBEDDED_INDEX.contains("setPace({paused: wasPaused})"));
        assert!(EMBEDDED_INDEX.contains(
            "sessionStorage.setItem(\"civvis-restart-paused-v1\", wasPaused ? \"1\" : \"0\")"
        ));
        assert!(EMBEDDED_INDEX.contains("specPaused = restartPaused === \"1\""));
        assert!(EMBEDDED_INDEX.contains("fetchJSON(\"/next-game-settings\""));
        assert!(EMBEDDED_INDEX.contains("with selected settings"));
        assert!(EMBEDDED_INDEX.contains("fetchJSON(\"/supervisor-new\""));
        assert!(EMBEDDED_INDEX.contains(
            "function supervisedSuccessorChanged(successor, finishedInstance, finishedSeed)"
        ));
        assert!(
            EMBEDDED_INDEX.contains("waitForSupervisedSuccessor(finishedInstance, finishedSeed)")
        );
        assert!(EMBEDDED_INDEX.contains(
            "waitForSupervisedSuccessor(st.server_instance, st.seed)"
        ));
        assert!(EMBEDDED_INDEX.contains("fetchJSON(\"/state\", {cache: \"no-store\"}, 3000)"));
        assert!(EMBEDDED_INDEX.contains("st.seed !== state.seed"));
        assert!(!EMBEDDED_INDEX.contains("id=\"head-newgame\""));
        assert!(EMBEDDED_INDEX.contains("spectate: gameMode === \"ai_sim\""));
        assert!(!EMBEDDED_INDEX.contains("id=\"specchk\""));
        assert!(!EMBEDDED_INDEX.contains("RULES.map_sizes.filter"));

        let mode_setting = EMBEDDED_INDEX
            .find("id=\"gamemode\"")
            .expect("game mode setting");
        let world_setting = EMBEDDED_INDEX
            .find("id=\"np\"")
            .expect("world size setting");
        let map_setting = EMBEDDED_INDEX.find("id=\"maptype\"").expect("map setting");
        let speed_setting = EMBEDDED_INDEX
            .find("id=\"gamespeed\"")
            .expect("game speed setting");
        assert!(
            mode_setting < world_setting
                && world_setting < map_setting
                && map_setting < speed_setting
        );

        let game_settings = EMBEDDED_INDEX
            .find("id=\"game-settings\"")
            .expect("game settings panel");
        let display_settings = EMBEDDED_INDEX
            .find("id=\"display-settings\"")
            .expect("display settings panel");
        let event_log = EMBEDDED_INDEX
            .find("<span>Game event log</span>")
            .expect("game event log");
        let war_log = EMBEDDED_INDEX
            .find("<span>War log</span>")
            .expect("war log");
        let strategy = EMBEDDED_INDEX
            .find("<span>Active strategy</span>")
            .expect("active strategy section");
        assert!(
            game_settings < display_settings
                && display_settings < war_log
                && war_log < event_log
                && event_log < strategy,
            "left panel should show game settings, display settings, and the two logs first"
        );
        assert!(EMBEDDED_INDEX.contains("<span>Display settings</span>"));
        for overlay in ["players", "victory", "minimap", "controls"] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("data-overlay-close=\"{overlay}\"")),
                "map overlay {overlay} should have a close control"
            );
        }
        assert!(EMBEDDED_INDEX.contains("function dismissOverlay(name, source)"));
        assert!(EMBEDDED_INDEX.contains("addEventListener(\"pointerdown\", event =>"));
        assert!(EMBEDDED_INDEX.contains("overlay-return-flash .24s ease-in-out 3"));
        assert!(EMBEDDED_INDEX.contains("restore in Display settings"));
        assert_eq!(
            EMBEDDED_INDEX
                .matches("class=\"sidebar-section\"")
                .count(),
            7,
            "every top-level left-panel section should be collapsible"
        );
        assert!(EMBEDDED_INDEX.contains("function initSidebarSections()"));
        assert!(EMBEDDED_INDEX.contains("civvis-sidebar-sections-v1"));
        for overlay in [
            "#playerhud",
            "#victoryhud",
            ".minimap-frame",
            "#zoomctl > :not(#paneltoggle)",
            "#ubar",
            "#modeline",
            "#tip",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("body.sidebar-hidden {overlay}")),
                "focus mode should hide the {overlay} map overlay"
            );
        }
        assert!(EMBEDDED_INDEX.contains("function civilizationEventText(text, next)"));
        assert!(!EMBEDDED_INDEX.contains("Simulator settings"));
        assert!(EMBEDDED_INDEX.contains("Quick Deals"));
        assert!(EMBEDDED_INDEX.contains("function drawQuickDeals()"));
        assert!(EMBEDDED_INDEX.contains("type:\"trade\""));
        assert!(EMBEDDED_INDEX.contains("function spectatorIdentity(player)"));
        assert!(EMBEDDED_INDEX.contains("function warLossLedger(war)"));
        let loss_categories = [
            "[\"civilian\", \"Civilian\"]",
            "[\"light_cavalry\", \"Light cavalry\"]",
            "[\"heavy_cavalry\", \"Heavy cavalry\"]",
            "[\"melee\", \"Melee\"]",
            "[\"anti_cavalry\", \"Anti-cavalry\"]",
            "[\"ranged\", \"Ranged\"]",
            "[\"siege\", \"Siege\"]",
            "[\"support\", \"Support\"]",
        ];
        for pair in loss_categories.windows(2) {
            assert!(
                EMBEDDED_INDEX.find(pair[0]).unwrap() < EMBEDDED_INDEX.find(pair[1]).unwrap(),
                "war-loss categories should preserve the requested order"
            );
        }
        assert!(EMBEDDED_INDEX.contains("WAR_LOSS_CIVILIAN_ORDER"));
        assert!(EMBEDDED_INDEX.contains("return a.unique ? -1 : 1"));
        assert!(EMBEDDED_INDEX.contains("${loss.total} x ${titleCase(info.kind)}"));
        assert!(EMBEDDED_INDEX.contains("onclick=\"spectatePlayer(${id})\""));
        assert!(EMBEDDED_INDEX.contains("state.players[state.player] || actor"));
        assert!(EMBEDDED_INDEX.contains("Global lifetime carbon emissions"));
        assert!(EMBEDDED_INDEX.contains("Alliance · Level"));
        assert!(EMBEDDED_INDEX.contains("p.ai_strategy"));
        // The ribbon is the consolidated view; one civilization at a time can
        // be opened into the dossier from its name.
        assert!(EMBEDDED_INDEX.contains("function civDossier(p, rank, relation)"));
        assert!(EMBEDDED_INDEX.contains("function toggleCivDossier(id)"));
        assert!(EMBEDDED_INDEX.contains("p.ai_plan"));
        assert!(EMBEDDED_INDEX.contains(".civ-dossier {"));
        assert!(EMBEDDED_INDEX.contains("changed its grand strategy from"));
        // The log never reorders: an overflowing log retires its least
        // valuable entry instead of holding important ones frozen at the top.
        assert!(!EMBEDDED_INDEX.contains("e.important && now - e.at < 6000"));
        assert!(EMBEDDED_INDEX.contains("const CAP = 60, FRESH = 12"));
        assert!(EMBEDDED_INDEX.contains("SERVER_EVENT_VALUES"));
        assert!(EMBEDDED_INDEX.contains("const floor = active ? (SPEC ? 32 : 16) : MODE.idle"));
        // The repaint rate answers to what a frame actually costs, so the
        // expensive style degrades to a slower picture rather than a stalled one
        // on a box that is also running the game.
        assert!(EMBEDDED_INDEX.contains("Math.max(floor, drawCost * 1.15)"));
        // Three map styles, and the browser must be able to name each of them.
        // The idle repaint rate is a property of the style rather than a
        // constant now: strategic never repaints on its own, balanced ticks
        // slowly for the pulsing markers, cinematic runs its weather.
        for style in ["strategic", "balanced", "cinematic"] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("<option value=\"{style}\"")),
                "map style {style} missing from the view selector"
            );
            assert!(
                EMBEDDED_INDEX.contains(&format!("  {style}:")),
                "map style {style} missing from VIEW_MODES"
            );
        }
        assert!(EMBEDDED_INDEX.contains("const VIEW_MODES = {"));
        assert!(EMBEDDED_INDEX.contains(
            "const VIEW_LEVELS = [\"strategic\", \"balanced\", \"cinematic\"]"
        ));
        let strategic_option = EMBEDDED_INDEX
            .find("<option value=\"strategic\">Strategic")
            .unwrap();
        let painted_option = EMBEDDED_INDEX
            .find("<option value=\"balanced\">Painted")
            .unwrap();
        let cinematic_option = EMBEDDED_INDEX
            .find("<option value=\"cinematic\">Cinematic")
            .unwrap();
        assert!(strategic_option < painted_option && painted_option < cinematic_option);
        assert!(EMBEDDED_INDEX.contains("if (old === \"cinematic\") return \"balanced\";"));
        assert!(EMBEDDED_INDEX.contains("return \"strategic\";"));
        // Painted and Cinematic are different geometries, not the same raised
        // board with a post-process pass: the former is a top-down painted
        // plane, while the latter lowers the camera and extrudes the terrain.
        assert!(
            EMBEDDED_INDEX.contains("balanced:  { relief:0,   projection:1.00, skirt:0")
        );
        assert!(
            EMBEDDED_INDEX.contains("cinematic: { relief:1.8, projection:0.70, skirt:9")
        );
        assert!(EMBEDDED_INDEX.contains(
            "YS = VIEW === \"cinematic\" ? cinematicYS : MODE.projection"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "else setRot(cam.rot, false);  // recompute screen-space light"
        ));
        // Reducing visual complexity is a deliberate return to the atlas, not
        // merely a material swap on whatever close-up the cinematic director
        // happened to leave behind.
        assert!(EMBEDDED_INDEX.contains(
            "const movingDown = VIEW_LEVELS.indexOf(v) < VIEW_LEVELS.indexOf(VIEW);"
        ));
        assert!(EMBEDDED_INDEX.contains("if (movingDown) setFullWorldView();"));
        assert!(EMBEDDED_INDEX.contains("function setFullWorldView()"));
        assert!(EMBEDDED_INDEX.contains("takeCameraControl();\n  setRot(0);"));
        assert!(EMBEDDED_INDEX.contains("const centers = state.map.tiles.map"));
        assert!(
            EMBEDDED_INDEX.contains("if (beau && MODE.relief > 0 && !water) drawWalls(")
        );
        assert!(EMBEDDED_INDEX.contains("localStorage.setItem(\"civvis-view-v3\", v)"));
        // Preserve the old Cinematic-to-Painted rename for returning browsers;
        // a browser with no saved preference starts in Strategic instead.
        assert!(EMBEDDED_INDEX.contains("localStorage.getItem(\"civvis-view\")"));
        // Ground is baked once per style/terrain/relief/variant and blitted
        // through the world plane; the per-frame clip-and-gradient path it
        // replaced is what made a full-size map cost eighty milliseconds a
        // frame. Both halves have to ship together to mean anything.
        assert!(EMBEDDED_INDEX.contains("function bakeTileArt("));
        assert!(EMBEDDED_INDEX.contains("function tileArt("));
        assert!(EMBEDDED_INDEX.contains("cx.drawImage(art, -artR, -artR, artR * 2, artR * 2)"));
        assert!(EMBEDDED_INDEX.contains("TERRAIN_ATLAS.onload"));
        assert!(EMBEDDED_INDEX.contains("ATLAS_READY = true; TILE_ART.clear()"));
        // Only what the camera can reach is drawn.
        assert!(EMBEDDED_INDEX.contains("const onscreen = []"));
        // Combat is staged rather than marked: a weapon, a flight, an impact.
        assert!(EMBEDDED_INDEX.contains("function stageAttack("));
        assert!(EMBEDDED_INDEX.contains("const SHOT_KIND = {"));
        assert!(EMBEDDED_INDEX.contains("const SHOT_STYLE = {"));
        assert!(EMBEDDED_INDEX.contains("function drawAtmosphere("));
        assert!(EMBEDDED_INDEX.contains(".diplomacy-card.allied"));
        assert!(EMBEDDED_INDEX.contains("function cameraYBounds"));
        assert!(EMBEDDED_INDEX.contains("cam.y = clampCameraY(cam.y)"));
        // Default camera moves use the center of the map canvas horizontally.
        // The command deck narrows that canvas and therefore shifts the focus
        // right on the full screen. Top HUDs move it 42% up from the bottom;
        // focus mode hides them and restores the exact 50/50 center.
        assert!(EMBEDDED_INDEX.contains("const DEFAULT_MAP_FOCUS_FROM_BOTTOM = .42;"));
        assert!(EMBEDDED_INDEX.contains("function mapOverlayVisible(name)"));
        assert!(EMBEDDED_INDEX.contains(
            "document.body.classList.contains(\"sidebar-hidden\")"
        ));
        assert!(EMBEDDED_INDEX.contains("function mapFocusPoint()"));
        assert!(EMBEDDED_INDEX.contains(
            "topHudVisible ? 1 - DEFAULT_MAP_FOCUS_FROM_BOTTOM : .5"
        ));
        assert!(EMBEDDED_INDEX.contains("function cameraCenterForWorld("));
        assert!(EMBEDDED_INDEX.contains("function currentMapFocusWorld()"));
        assert!(EMBEDDED_INDEX.contains("function reframeCurrentMapFocus(world)"));
        assert!(EMBEDDED_INDEX.contains(
            "const {x:desiredX, y:desiredY} = mapFocusPoint();"
        ));
        assert!(EMBEDDED_INDEX.contains("View as"));
        assert!(EMBEDDED_INDEX.contains("id=\"viewplayer\""));
        assert!(EMBEDDED_INDEX.contains("fetchJSON(\"/view\""));
        // The ribbon repaints under the cursor, so its buttons declare their
        // action as data and one delegated listener dispatches it.
        assert!(EMBEDDED_INDEX.contains("data-hud-action=\"watch\" data-hud-civ=\"${p.id}\""));
        assert!(EMBEDDED_INDEX.contains("data-hud-action=\"dossier\" data-hud-civ=\"${p.id}\""));
        assert!(EMBEDDED_INDEX.contains("else spectatePlayer(id);"));
        assert!(EMBEDDED_INDEX.contains("async function spectatePlayer(player)"));
        // Watching one civilization is a persistent empire portrait, not a
        // one-time jump to its capital. Borders/cities define the durable
        // frame; strategic, grouped, promoted, and war-front units may widen
        // it, while a lone recon unit cannot continually zoom the map out.
        assert!(EMBEDDED_INDEX.contains("function watchedEmpireSubjects(player)"));
        assert!(EMBEDDED_INDEX.contains("function observedViewGoal(anchors)"));
        assert!(EMBEDDED_INDEX.contains("watchedEmpireAutoFrame"));
        assert!(EMBEDDED_INDEX.contains("const EMPIRE_RECON_UNITS"));
        assert!(EMBEDDED_INDEX.contains("const atWarFront"));
        assert!(EMBEDDED_INDEX.contains("Number(unit.formation) > 0"));
        assert!(EMBEDDED_INDEX.contains("Number(unit.level) >= 3"));
        assert!(EMBEDDED_INDEX.contains("directorGoal.kind === \"empire\""));
        assert!(EMBEDDED_INDEX.contains("player log"));
        assert!(EMBEDDED_INDEX.contains("Spectator · combined summary"));
        assert!(EMBEDDED_INDEX.contains("let eventLogs = new Map()"));
        assert!(EMBEDDED_INDEX.contains("function chronicleWorldEvents(next)"));
        // The war log reads the engine's ledger straight out of the
        // observation, so the panel and its source must ship together.
        assert!(EMBEDDED_INDEX.contains("function drawWarLog()"));
        assert!(EMBEDDED_INDEX.contains("function warsForLog(wars)"));
        assert!(EMBEDDED_INDEX.contains("id=\"warsec\""));
        assert!(EMBEDDED_INDEX.contains("function warBelligerentRows("));
        assert!(EMBEDDED_INDEX.contains("function warPartyIsCityState("));
        assert!(EMBEDDED_INDEX.contains("war-row-label\">Belligerents"));
        assert!(EMBEDDED_INDEX.contains("<span>Start</span><span>Peak</span><span>Total</span>"));
        assert!(EMBEDDED_INDEX.contains("overflow-wrap: break-word"));
        assert!(EMBEDDED_INDEX.contains(
            ".war-belligerent.city-state .war-belligerent-bar { width: 70%; }"
        ));
        assert!(!EMBEDDED_INDEX.contains("Military strength at entry"));
        assert!(EMBEDDED_INDEX.contains("war-row-label\">Chronology"));
        assert!(EMBEDDED_INDEX.contains("war-row-label\">Losses"));
        assert!(EMBEDDED_INDEX.contains("Peace deal terms"));
        let belligerents = EMBEDDED_INDEX.find("war-row-label\">Belligerents").unwrap();
        let losses = EMBEDDED_INDEX.find("war-row-label\">Losses").unwrap();
        let chronology = EMBEDDED_INDEX.find("war-row-label\">Chronology").unwrap();
        assert!(belligerents < losses && losses < chronology);
        assert!(EMBEDDED_INDEX.contains("entered Turn ${party.entered}"));
        assert!(EMBEDDED_INDEX.contains("peaced out Turn ${party.exited}"));
        assert!(EMBEDDED_INDEX.contains("sort((a, b) => a.turn - b.turn)"));
        assert!(EMBEDDED_INDEX.contains("built the world's first"));
        assert!(EMBEDDED_INDEX.contains("changed government from"));
        assert!(!EMBEDDED_INDEX.contains("completed its turn"));
        assert!(!EMBEDDED_INDEX
            .contains("civilization${summaries.length === 1 ? \"\" : \"s\"} completed"));
        assert!(EMBEDDED_INDEX.contains("id=\"strategysec\""));
        assert!(EMBEDDED_INDEX
            .contains("document.getElementById(\"strategysec\").style.display = fullMapSpectator"));
        assert!(EMBEDDED_INDEX.contains("if (!fullMapSpectator && (SPEC || govs.length"));
        assert!(EMBEDDED_INDEX.contains(".sort((a, b) => b.score - a.score || a.id - b.id)"));
        assert!(EMBEDDED_INDEX.contains("class=\"diplomacy-rank\">#${rank}"));
        // The sidebar sits left of the map. Match the declaration rather than
        // its formatting, so restyling the block cannot fail the rule.
        let side_rule = EMBEDDED_INDEX
            .split_once("#side {")
            .and_then(|(_, rest)| rest.split_once('}'))
            .map(|(rule, _)| rule)
            .unwrap_or_default();
        assert!(side_rule.contains("order: -1"));
        assert!(EMBEDDED_INDEX.contains("<strong>${state.turn}</strong>"));
        assert!(!EMBEDDED_INDEX.contains("${state.turn}/${maxTurns}"));
    }

    #[test]
    fn browser_includes_the_cinematic_spectator_director() {
        for id in [
            "cinemachk",
            "cinema-atmosphere",
            "cinema-lighting",
            "cinema-frame",
            "cinema-transition",
            "cinema-prologue",
            "cinema-story",
            "cinema-audio",
            "cinema-follow",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("id=\"{id}\"")),
                "cinematic spectator element {id} is missing"
            );
        }
        for function in [
            "function applyCinemaMode()",
            "function showCinemaPrologue(st)",
            "function showCinemaChapter(cue)",
            "function createCinemaAudioGraph()",
            "function playCinemaCue(cue = {})",
            "function showDirectorStory(cue)",
            "function cinematicDisasterStory(disaster, next)",
            "function cinematicWarContext(st)",
            "function cinematicWarFront(st, war)",
            "function drawCinematicDisasterField(sceneTime)",
            "function recentCombatActions(st)",
            "const lethalBattle = MODE.fx && lethalTrace",
            "const deathAt = lethalBattle?.impactAt || now",
            "function directorCue(prev, next)",
            "function cinematicShotGoal(goal, variation = 0)",
            "function directorSurveyGoal()",
            "function directorAmbientCue()",
            "function advanceDirector(now = performance.now())",
            "function advanceUserCameraMotion(now = performance.now())",
            "function advanceCameraFollow(now = performance.now())",
            "function startCameraFollow(unitId)",
            "function unitMapPoint(p, nearX = cam.x)",
            "function sampleUnitMove(mv, now = performance.now())",
            "function cinematicUnitMapPoint(unit, now = performance.now())",
            "function unitMoveDuration(unitId, steps)",
            "function drawCinematicSubjectMarker(u, x, y, now)",
            "function drawCinematicSubjectBrackets(u, x, y, now)",
            "function drawUnitCompany(u, x, y, now, moving)",
            "function beginTouchTransform()",
            "function cinematicSurveyUnits(units)",
            "function drawWalls(t, x, ytop, baseColor, tileElevation)",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(function),
                "cinematic spectator behavior {function} is missing"
            );
        }
        for story in [
            "A new world awakens",
            "Nature unleashed",
            "The world enters the ${eraName} Era",
            "A civilization falls",
            "History has a victor",
            "City captured",
            "War declared",
            "Battlefield losses",
            "The war continues",
            "Wonder completed",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(story),
                "cinematic spectator story cue {story} is missing"
            );
        }
        assert!(EMBEDDED_INDEX.contains("civvis-cinema-audio-v1"));
        assert!(EMBEDDED_INDEX.contains("REDUCED_MOTION_QUERY.matches"));
        assert!(EMBEDDED_INDEX.contains("touch-action: none"));
        assert!(EMBEDDED_INDEX
            .contains("kind:battle ? \"battle\" : (tracksUnit ? \"character\" : \"event\")"));
        assert!(EMBEDDED_INDEX.contains("side.units_lost"));
        assert!(EMBEDDED_INDEX.contains("action.type === \"theological_attack\""));
        assert!(EMBEDDED_INDEX.contains("front:\"war front\""));
        assert!(EMBEDDED_INDEX.contains("kind:cue.kind || \"portrait\""));
        assert!(!EMBEDDED_INDEX.contains("kicker:\"Casualty of war\""));
        assert!(EMBEDDED_INDEX.contains("class=\"winner-content\""));
        assert!(EMBEDDED_INDEX.contains("cinema-finale"));
        assert!(
            EMBEDDED_INDEX.contains("DEFAULT_CINEMA_YS = VIEW_MODES.cinematic.projection")
        );
        assert!(EMBEDDED_INDEX.contains("function drawUnitFormationBadge"));
        assert!(EMBEDDED_INDEX.contains("<script src=\"/cinematic3d.js\"></script>"));
        assert!(EMBEDDED_INDEX.contains("globalThis.Cinematic3D?.supports(family)"));
        assert!(EMBEDDED_INDEX.contains("Cinematic3D.draw({"));
        assert!(EMBEDDED_INDEX.contains("specular glints travel"));
        assert!(EMBEDDED_INDEX.contains("cx.lineDashOffset = dash.length"));
    }

    #[test]
    fn cinematic_world_wonders_use_a_complete_sprite_atlas() {
        assert!(EMBEDDED_WORLD_WONDER_ATLAS.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(EMBEDDED_WORLD_WONDER_ATLAS.len() > 1_000_000);
        assert!(
            EMBEDDED_INDEX.contains("WORLD_WONDER_ATLAS.src = \"/assets/world-wonder-atlas.png\"")
        );

        let ids = EMBEDDED_INDEX
            .split("const WORLD_WONDER_IDS = [")
            .nth(1)
            .and_then(|tail| tail.split("];\nconst WORLD_WONDER_CELL").next())
            .expect("ordered World Wonder sprite IDs");
        let rules = crate::rules::Rules::embedded();
        assert_eq!(ids.matches('"').count() / 2, rules.wonders.len());
        for wonder in rules.wonders.keys() {
            assert!(
                ids.contains(&format!("\"{wonder}\"")),
                "World Wonder {wonder} has no sprite cell"
            );
        }

        let renderer = EMBEDDED_INDEX
            .split("function drawWorldWonderSprite")
            .nth(1)
            .and_then(|tail| tail.split("function drawWonder").next())
            .expect("World Wonder sprite renderer");
        assert!(renderer.contains("!MODE.atmosphere"));
        assert!(renderer.contains("SHX * S * .42"));
        assert!(EMBEDDED_INDEX.contains("t.wonder && !drawWorldWonderSprite(t.wonder, x, y)"));
    }

    #[test]
    fn cinematic_3d_module_covers_every_unit_renderer_family() {
        for family in [
            "embarked",
            "naval",
            "air",
            "rotor",
            "balloon",
            "drone",
            "robot",
            "armor",
            "gun",
            "siege",
            "mounted",
            "religious",
            "civilian",
            "infantry",
        ] {
            assert!(
                EMBEDDED_CINEMATIC_3D.contains(&format!("\"{family}\"")),
                "cinematic 3D model family {family} is missing"
            );
        }
        for behavior in [
            "class Scene",
            "this.items.sort((a, b) => a.depth - b.depth)",
            "const direct = Math.max(0, dot(normal, this.light))",
            "function human(scene, options",
            "function drawMounted(scene, o)",
            "function drawChariot(scene, o)",
            "function drawArmor(scene, o)",
            "function drawRobot(scene, o)",
            "function drawGun(scene, o)",
            "function drawSiege(scene, o)",
            "function drawNaval(scene, o, embarked = false)",
            "function drawAir(scene, o)",
            "function drawRotor(scene, o)",
            "function drawBalloon(scene, o)",
            "function drawDrone(scene, o)",
            "function drawConvoy(scene, o)",
            "type === \"slinger\"",
            "type === \"scout\"",
            "global.Cinematic3D = Object.freeze",
        ] {
            assert!(
                EMBEDDED_CINEMATIC_3D.contains(behavior),
                "cinematic 3D behavior {behavior} is missing"
            );
        }
    }

    #[test]
    fn browser_blends_atlas_art_only_across_compatible_tile_footprints() {
        let atlas_drawer = EMBEDDED_INDEX
            .split("function drawAtlasFeatureCell")
            .nth(1)
            .and_then(|tail| tail.split("function drawFeatureSprite").next())
            .expect("shared atlas feature renderer");
        assert!(atlas_drawer.contains("tileArtPath(footprint"));
        assert!(atlas_drawer.contains("cx.clip()"));

        let blend_footprint = EMBEDDED_INDEX
            .split("function blendedTileFootprint")
            .nth(1)
            .and_then(|tail| tail.split("function featureBlendKind").next())
            .expect("compatible-neighbor footprint builder");
        assert!(blend_footprint.contains("if (!neighbor || !accepts(neighbor)) continue"));
        assert!(EMBEDDED_INDEX.contains("featureBlendKind(neighbor.feature) === baseFeature"));
        assert!(EMBEDDED_INDEX.contains("neighbor.terrain === \"mountain\""));

        let terrain_texture = EMBEDDED_INDEX
            .split("function drawTerrainTexture")
            .nth(1)
            .and_then(|tail| tail.split("function drawTerrainBlend").next())
            .expect("terrain material renderer");
        assert!(terrain_texture.contains("drawContinuousTerrain(t, x, y"));
        assert!(!terrain_texture.contains("hexPath(x, y"));
        let terrain_blend = EMBEDDED_INDEX
            .split("function drawTerrainBlend")
            .nth(1)
            .and_then(|tail| tail.split("function drawAtlasFeatureCell").next())
            .expect("terrain transition renderer");
        assert!(terrain_blend.contains("drawFeathered(t, x, y"));
        assert!(EMBEDDED_INDEX.contains("function drawContinuousTerrain(t, x, y, alpha)"));
        assert!(EMBEDDED_INDEX.contains("cx.createPattern(c, \"repeat\")"));

        let mountain_drawer = EMBEDDED_INDEX
            .split("function drawMountainSprite")
            .nth(1)
            .and_then(|tail| tail.split("function tri(").next())
            .expect("mountain sprite renderer");
        assert!(mountain_drawer.contains("tileArtPath(footprint, true"));
        assert!(mountain_drawer.contains("cx.clip()"));

        let wonder_placement = EMBEDDED_INDEX
            .split("function buildNaturalWonderPlacements")
            .nth(1)
            .and_then(|tail| tail.split("function drawTileYields").next())
            .expect("natural wonder footprint builder");
        assert!(wonder_placement.contains("footprint:points.map"));
        assert!(EMBEDDED_INDEX.contains("placement.footprint"));

        assert!(!EMBEDDED_INDEX.contains("const w = S * 2.55"));
        assert!(!EMBEDDED_INDEX.contains("width: volcano ? 2.32"));
    }

    #[test]
    fn instance_tagged_spectator_url_routes_to_the_embedded_page() {
        assert_eq!(request_path("/"), "/");
        assert_eq!(request_path("/?instance=9232"), "/");
        assert_eq!(request_path("/?instance=9232&game=17"), "/");
        assert_eq!(request_path("/state?instance=9232"), "/state");
    }

    #[test]
    fn next_spectator_game_preserves_settings_and_watched_player() {
        let mut params = current();
        params.spectate = true;
        let mut session = Session::new(params);
        session.set_view_player(Some(1)).unwrap();
        let previous_settings = (
            session.params.num_players,
            session.params.width,
            session.params.height,
            session.params.num_city_states,
            session.params.map_script,
            session.params.game_speed,
            session.params.spectate,
        );

        session
            .start_new_game(&json!({"seed": 2, "force": true}))
            .unwrap();

        assert_eq!(session.params.seed, 2);
        assert_eq!(
            (
                session.params.num_players,
                session.params.width,
                session.params.height,
                session.params.num_city_states,
                session.params.map_script,
                session.params.game_speed,
                session.params.spectate,
            ),
            previous_settings
        );
        assert_eq!(session.state()["view_player"].as_u64(), Some(1));
    }

    #[test]
    fn selected_settings_wait_for_the_next_automatic_game() {
        let mut params = current();
        params.spectate = true;
        let mut session = Session::new(params);
        let original_seed = session.game.seed;
        let original_script = session.game.map_script;
        let original_speed = session.game.game_speed;

        session.stage_next_game_settings(&json!({
            "num_players": 6,
            "map_script": "continents",
            "game_speed": "quick",
            "victory_conditions": {"culture": false, "score": false},
        }));

        assert_eq!(session.game.seed, original_seed);
        assert_eq!(session.game.map_script, original_script);
        assert_eq!(session.game.game_speed, original_speed);
        assert_eq!(
            session.state()["next_game_settings"],
            json!({
                "players": 6,
                "width": 74,
                "height": 46,
                "city_states": 9,
                "turns": 330,
                "map": "continents",
                "speed": "quick",
                "victories": ["science", "religious", "diplomatic", "domination"],
            })
        );

        session.start_automatic_next_game();

        assert_ne!(session.game.seed, original_seed);
        assert_eq!(session.params.num_players, 6);
        assert_eq!(session.params.map_script, MapScript::Continents);
        assert_eq!(session.params.game_speed, GameSpeed::Quick);
        assert!(!session.game.victory_conditions.culture);
        assert!(!session.game.victory_conditions.score);
        assert!(session.state()["next_game_settings"].is_null());
    }

    #[test]
    fn supervised_new_game_request_normalizes_settings_without_replacing_the_live_game() {
        let mut params = current();
        params.spectate = true;
        params.supervised = true;
        let mut session = Session::new(params);
        let original_seed = session.game.seed;

        session
            .request_supervised_new_game(&json!({
                "mode": "fresh_code",
                "paused": false,
                "num_players": 4,
                "map_script": "continents",
                "game_speed": "quick",
                "victory_conditions": {"culture": false, "score": false},
            }))
            .unwrap();

        let state = session.state();
        assert_eq!(session.game.seed, original_seed);
        assert!(session.spectator_paused);
        assert_eq!(state["supervisor_request"]["mode"], "fresh_code");
        assert_eq!(state["supervisor_request"]["paused"], false);
        assert_eq!(
            state["supervisor_request"]["server_instance"].as_u64(),
            Some(std::process::id() as u64)
        );
        assert_eq!(
            state["supervisor_request"]["settings"],
            json!({
                "players": 4,
                "width": 60,
                "height": 38,
                "city_states": 6,
                "turns": 330,
                "map": "continents",
                "speed": "quick",
                "victories": ["science", "religious", "diplomatic", "domination"],
            })
        );
    }

    #[test]
    fn unsupervised_server_rejects_supervisor_new_game_requests() {
        let mut session = Session::new(current());
        assert!(session
            .request_supervised_new_game(&json!({"mode": "fresh_code"}))
            .is_err());
        assert!(session.state()["supervisor_request"].is_null());
    }

    #[test]
    fn next_game_drops_a_watched_player_that_is_not_in_the_new_world() {
        let mut params = current();
        params.num_players = 4;
        params.width = 30;
        params.height = 20;
        params.spectate = true;
        let mut session = Session::new(params);
        session.set_view_player(Some(3)).unwrap();

        session
            .start_new_game(&json!({"num_players": 2, "seed": 2, "force": true}))
            .unwrap();

        assert!(session.state()["view_player"].is_null());
    }

    #[test]
    fn state_identifies_the_running_server_instance() {
        let state = Session::new(current()).state();
        assert_eq!(
            state["server_instance"].as_u64(),
            Some(std::process::id() as u64)
        );
        assert!(state["quick_deals"].is_array());
        assert!(state["active_trade_deals"].is_array());
        assert!(state["me"]["resources"].is_array());
    }

    #[test]
    fn spectator_state_reports_the_pause_liveness_signal() {
        let mut params = current();
        params.spectate = true;
        let mut session = Session::new(params);
        let state = session.state();
        assert_eq!(state["spectator_paused"].as_bool(), Some(false));
        assert!(state["view_player"].is_null());
        assert_eq!(
            state["visible"].as_array().unwrap().len(),
            state["map"]["tiles"].as_array().unwrap().len()
        );
        assert!(state["units"]
            .as_array()
            .unwrap()
            .iter()
            .all(|unit| unit.get("reachable").is_none()));
        assert!(state["players"][0]["ai_strategy"].is_null());
        assert!(state["players"][0]["ai_plan"].is_null());
        session.step();
        let stepped = session.state();
        assert_eq!(stepped["players"][0]["ai_strategy"], "expansion");
        // The expanded HUD card reads the whole plan, not just its label.
        let plan = &stepped["players"][0]["ai_plan"];
        assert_eq!(plan["strategy"], "expansion");
        assert!(plan["desired_cities"].as_u64().is_some());
        assert!(plan["assessed_turn"].as_u64().is_some());
        assert!(plan["forces"].is_array());
    }

    #[test]
    fn spectator_can_view_any_major_through_that_players_fog() {
        let mut params = current();
        params.spectate = true;
        let mut session = Session::new(params);
        let omniscient = session.state();

        session.set_view_player(Some(1)).unwrap();
        let player_view = session.state();
        assert_eq!(player_view["player"].as_u64(), Some(1));
        assert_eq!(player_view["view_player"].as_u64(), Some(1));
        assert!(
            player_view["visible"].as_array().unwrap().len()
                < omniscient["visible"].as_array().unwrap().len()
        );
        assert!(
            player_view["map"]["tiles"].as_array().unwrap().len()
                < omniscient["map"]["tiles"].as_array().unwrap().len()
        );
        assert!(player_view["units"]
            .as_array()
            .unwrap()
            .iter()
            .all(|unit| unit.get("reachable").is_none()));

        session.set_view_player(None).unwrap();
        assert!(session.state()["view_player"].is_null());
    }

    #[test]
    fn selecting_any_ranked_player_promotes_the_live_match_to_spectator_mode() {
        for pid in 0..current().num_players {
            let mut session = Session::new(current());
            assert!(!session.params.spectate);
            let omniscient_tile_count = session.game.map.tiles.len();

            session.set_view_player(Some(pid)).unwrap();
            let player_view = session.state();

            assert!(session.params.spectate);
            assert_eq!(player_view["spectate"].as_bool(), Some(true));
            assert_eq!(player_view["player"].as_u64(), Some(pid as u64));
            assert_eq!(player_view["view_player"].as_u64(), Some(pid as u64));
            assert!(player_view["map"]["tiles"].as_array().unwrap().len() < omniscient_tile_count);
        }
    }

    #[test]
    fn spectator_view_rejects_non_major_and_unknown_players() {
        let mut params = current();
        params.spectate = true;
        let mut session = Session::new(params);
        let minor = session
            .game
            .players
            .iter()
            .find(|player| player.is_minor || player.is_barbarian)
            .unwrap()
            .id;

        assert!(session.set_view_player(Some(minor)).is_err());
        assert!(session.set_view_player(Some(usize::MAX)).is_err());
        assert!(session.state()["view_player"].is_null());
    }

    #[test]
    fn result_countdown_cannot_replace_an_active_successor() {
        let mut params = current();
        params.spectate = true;
        let mut session = Session::new(params);
        let original_seed = session.game.seed;
        let guarded = json!({
            "seed": 2,
            "spectate": true,
            "replace_finished": {
                "seed": original_seed,
                "server_instance": std::process::id()
            }
        });

        assert!(session.start_new_game(&guarded).is_err());
        assert_eq!(session.game.seed, original_seed);
        assert!(session
            .start_new_game(&json!({"seed": 4, "spectate": true}))
            .is_err());
        assert_eq!(session.game.seed, original_seed);

        assert!(session
            .start_new_game(&json!({"seed": 5, "spectate": true, "force": true}))
            .is_ok());
        assert_eq!(session.game.seed, 5);

        session.game.winner = Some(0);
        let guarded = json!({
            "seed": 2,
            "spectate": true,
            "replace_finished": {
                "seed": 5,
                "server_instance": std::process::id()
            }
        });
        session.params.supervised = true;
        assert!(session.start_new_game(&guarded).is_err());
        assert_eq!(session.game.seed, 5);
        assert!(session
            .start_new_game(&json!({"seed": 6, "spectate": true, "force": true}))
            .is_err());
        assert_eq!(session.game.seed, 5);
        session.params.supervised = false;
        assert!(session.start_new_game(&guarded).is_ok());
        assert_eq!(session.game.seed, 2);

        session.game.winner = Some(0);
        let stale = json!({
            "seed": 3,
            "spectate": true,
            "replace_finished": {
                "seed": 2,
                "server_instance": u64::from(std::process::id()) + 1
            }
        });
        assert!(session.start_new_game(&stale).is_err());
        assert_eq!(session.game.seed, 2);
    }

    #[test]
    fn spectator_state_uses_a_major_viewpoint_during_barbarian_turns() {
        let mut params = current();
        params.spectate = true;
        let mut session = Session::new(params);
        let barbarian = session
            .game
            .players
            .iter()
            .find(|player| player.is_barbarian)
            .unwrap()
            .id;
        session.game.current = barbarian;

        let state = session.state();
        let viewer = state["player"].as_u64().unwrap() as usize;
        assert!(!session.game.players[viewer].is_minor);
        assert!(!session.game.players[viewer].is_barbarian);
        assert!(session.game.players[viewer].alive);
    }

    #[test]
    fn spectator_chronicle_reports_world_milestones_once() {
        let mut params = current();
        params.spectate = true;
        let mut session = Session::new(params);
        let game = &mut session.game;
        let first_pos = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .map(|unit| game.units[&unit].pos)
            .unwrap();
        let second_pos = game
            .player_unit_ids(1)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .map(|unit| game.units[&unit].pos)
            .unwrap();
        let first_city = game.found_city_for(0, first_pos, Some("Alpha".to_string()));
        let captured_city = game.found_city_for(1, second_pos, Some("Beta".to_string()));
        let before = ChronicleSnapshot::capture(game);
        let mut chronicle = ChronicleState::from_game(game);

        let district_pos = game.cities[&first_city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != first_pos)
            .unwrap();
        game.cities
            .get_mut(&first_city)
            .unwrap()
            .districts
            .insert("campus".to_string(), district_pos);
        game.cities
            .get_mut(&first_city)
            .unwrap()
            .wonders
            .insert("pyramids".to_string(), district_pos);
        game.cities
            .get_mut(&first_city)
            .unwrap()
            .buildings
            .push("granary".to_string());
        game.cities.get_mut(&first_city).unwrap().pop = 4;
        game.players[0].religion = Some("Test Faith".to_string());
        game.players[0].government = Some("classical_republic".to_string());
        game.players[0].techs.insert("horseback_riding".to_string());
        game.players[0].civics.insert("drama_poetry".to_string());
        let city_state = game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .unwrap();
        game.players[0].envoys.push((city_state, 3));
        {
            let city = game.cities.get_mut(&captured_city).unwrap();
            city.owner = 0;
            city.occupied_from = Some(1);
        }

        let after = ChronicleSnapshot::capture(game);
        let events = chronicle_world_events(
            &before,
            &after,
            0,
            &[Action::KeepCity {
                city: captured_city,
            }],
            &mut chronicle,
        );
        let event_types: Vec<_> = events
            .iter()
            .filter_map(|event| event["type"].as_str())
            .collect();
        for expected in [
            "wonder_built",
            "religion_founded",
            "district_first",
            "building_first",
            "population_milestone",
            "city_captured",
            "suzerain_changed",
            "government_changed",
        ] {
            assert!(
                event_types.contains(&expected),
                "missing {expected}: {events:?}"
            );
        }
        assert_eq!(
            events
                .iter()
                .filter(|event| event["type"] == "era_first")
                .count(),
            2,
            "technology and civics should each announce their Classical leader"
        );

        let later = ChronicleSnapshot::capture(game);
        let repeat = chronicle_world_events(&after, &later, 0, &[], &mut chronicle);
        assert!(
            repeat.is_empty(),
            "unchanged milestones repeated: {repeat:?}"
        );
    }

    #[test]
    fn spectator_chronicle_tracks_war_declarations_losses_and_peace() {
        let mut game = Session::new(current()).game;
        let defeated = game
            .units
            .values()
            .find(|unit| {
                unit.owner == 1 && game.rules.units[unit.kind.as_str()].class == "military"
            })
            .map(|unit| unit.id)
            .expect("player two starts with a military unit");
        let before = ChronicleSnapshot::capture(&game);
        let mut chronicle = ChronicleState::from_game(&game);

        game.at_war.insert((0, 1));
        game.remove_unit(defeated);
        let after_battle = ChronicleSnapshot::capture(&game);
        let events = chronicle_world_events(
            &before,
            &after_battle,
            0,
            &[Action::DeclareWar { player: 1 }],
            &mut chronicle,
        );
        assert!(events.iter().any(|event| event["type"] == "war_started"));
        let progress = events
            .iter()
            .find(|event| event["type"] == "war_progress")
            .expect("a destroyed military unit advances the war chronicle");
        assert_eq!(progress["defender_units_lost"], 1);
        assert_eq!(progress["aggressor_units_lost"], 0);

        game.at_war.remove(&(0, 1));
        let after_peace = ChronicleSnapshot::capture(&game);
        let peace = chronicle_world_events(
            &after_battle,
            &after_peace,
            0,
            &[Action::MakePeace { player: 1 }],
            &mut chronicle,
        );
        let ended = peace
            .iter()
            .find(|event| event["type"] == "war_ended")
            .expect("peace concludes the running war chronicle");
        assert_eq!(ended["defender_units_lost"], 1);
    }

    #[test]
    fn restored_session_preserves_progress_and_derives_its_world_settings() {
        let mut game = Session::new(current()).game;
        game.turn = 37;
        game.current = 1;
        let mut wrong = current();
        wrong.num_players = 12;
        wrong.width = 106;
        wrong.height = 66;
        wrong.num_city_states = 18;

        let restored = Session::from_game(wrong, game);
        assert_eq!((restored.game.turn, restored.game.current), (37, 1));
        assert_eq!(restored.params.num_players, 2);
        assert_eq!((restored.params.width, restored.params.height), (20, 14));
        assert_eq!(restored.params.num_city_states, 1);
    }

    /// The single-player turn loop is a promise to the player, not an
    /// implementation detail: the End Turn button says what the game is
    /// waiting on, `Enter` walks those blockers in a fixed order and only
    /// ends the turn once none are left, and a unit under a standing order
    /// stops being counted. `docs/SINGLE_PLAYER.md` states the contract.
    /// A Civ 6 lobby asks who you are and how hard the rivals play, and it
    /// can open a game you saved. The browser could do none of those: single
    /// player was disabled in the mode select, and `/new`'s `difficulty` and
    /// `civs` and the save endpoints had no control anywhere.
    #[test]
    fn browser_sets_up_and_reopens_a_single_player_game() {
        for piece in [
            "id=\"leader\"",
            "id=\"difficulty\"",
            "id=\"startgame\"",
            "id=\"saves-group\"",
            "function syncSetupMode()",
            "async function refreshSaves()",
            "async function loadSave(name)",
            "async function writeSave()",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(piece),
                "the setup screen is missing {piece}"
            );
        }
        // Both selects are filled from the live ruleset, never a hardcoded list.
        assert!(EMBEDDED_INDEX.contains("RULES.civs && typeof RULES.civs === \"object\""));
        assert!(EMBEDDED_INDEX
            .contains("RULES.difficulties && typeof RULES.difficulties === \"object\""));
        // A spectated world has nobody to hand a leader or a handicap to.
        assert!(EMBEDDED_INDEX.contains(
            "...(gameMode === \"ai_sim\" ? {} : {civs: leader ? [leader] : [], difficulty})"
        ));
        // A build without the save endpoints hides the group rather than
        // offering one that cannot work.
        assert!(EMBEDDED_INDEX.contains("catch (error) { group.style.display = \"none\";"));
    }

    /// War, peace and denouncement have been in `legal_actions(0)` since v0.6
    /// with nothing on the page that would send one, which closed the
    /// domination path to a person while leaving it open to every agent. The
    /// diplomacy screen is where those live now, and it must keep covering
    /// them — including for city-states, or a war with one can be started and
    /// never ended.
    #[test]
    fn browser_lets_the_player_conduct_diplomacy() {
        for piece in [
            "id=\"diplomacy\"",
            "function drawDiplomacy()",
            "function openDiplomacy()",
            "function sendFromDiplomacy(action)",
            "id=\"diplomacybtn\"",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(piece),
                "the diplomacy screen is missing {piece}"
            );
        }
        for action in [
            "declare_war",
            "declare_war_with_casus_belli",
            "make_peace",
            "denounce",
            "propose_deal",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("byPlayer(\"{action}\")")),
                "the diplomacy screen does not offer {action}"
            );
        }
        // Incoming proposals are answered from the same screen.
        assert!(EMBEDDED_INDEX.contains("a.type === \"accept_deal\" || a.type === \"reject_deal\""));
        // City-states are listed, so peace with one is reachable.
        assert!(EMBEDDED_INDEX.contains("Number(first.is_minor) - Number(second.is_minor)"));
        // Barbarians are permanently at war and must not be counted as a power.
        assert!(EMBEDDED_INDEX
            .contains("player.at_war_with_me && player.alive && !player.is_barbarian"));
        // Actions are posted back exactly as the engine handed them over.
        assert!(EMBEDDED_INDEX
            .contains("onclick='sendFromDiplomacy(${JSON.stringify(action)})'>${label}</button>"));
    }

    #[test]
    fn browser_runs_a_civ_six_turn_loop() {
        for piece in [
            "function turnBlockers()",
            "function standingNotices()",
            "function drawTurnLoop()",
            "function advanceTurn(force = false)",
            "function openTurnIfNew()",
            "function advanceToNextUnit(force = false)",
            "function unitNeedsOrders(unit)",
            "id=\"notify\"",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(piece),
                "the browser turn loop is missing {piece}"
            );
        }
        // Blockers are announced in priority order, highest first.
        let order = [
            "kind: \"capture\"",
            "kind: \"deal\"",
            "kind: \"congress\"",
            "kind: \"dedication\"",
            "kind: \"research\"",
            "kind: \"civic\"",
            "kind: `produce:${city.id}`",
            "kind: \"units\"",
        ];
        let mut previous = 0;
        for blocker in order {
            let at = EMBEDDED_INDEX
                .find(blocker)
                .unwrap_or_else(|| panic!("turn blocker {blocker} is missing"));
            assert!(
                at > previous,
                "turn blockers must be enumerated in priority order; {blocker} is out of place"
            );
            previous = at;
        }
        // Shift overrides the blockers; without that a disagreement with the
        // priority order becomes a trap the player cannot leave.
        assert!(EMBEDDED_INDEX.contains("advanceTurn(ev.shiftKey)"));
        assert!(EMBEDDED_INDEX.contains("if (next && !force) { next.act(); drawTurnLoop(); return; }"));
        // Standing orders are the client's own; they must never masquerade as
        // engine state, and a skip must expire with the turn that set it.
        assert!(EMBEDDED_INDEX.contains("if (held.order === \"skip\") return held.turn === state.turn ? \"skip\" : null;"));
        assert!(EMBEDDED_INDEX.contains("function wakeSleepers()"));
    }

    /// Every empire decision the engine offers seat 0 has a screen behind the
    /// launch bar, and each screen speaks only the JSON protocol: it labels
    /// the legal actions it was given and posts them back unchanged. The
    /// action kinds below are the ledger — a screen that stops covering one
    /// of them is a decision the player silently loses.
    #[test]
    fn browser_covers_every_empire_decision() {
        for piece in [
            "id=\"launchbar\"",
            "id=\"empire\"",
            "function drawLaunchBar()",
            "function openEmpire(tab)",
            "function empireBadge(tab)",
            "function empireGovernment()",
            "function empireReligion()",
            "function empireGreatPeople()",
            "function empireGovernors()",
            "function empireCityStates()",
            "function empireTrade()",
            "function empireSpies()",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(piece),
                "the empire panel is missing {piece}"
            );
        }
        for action in [
            "government",
            "slot_policy",
            "unslot_policy",
            "choose_pantheon",
            "found_religion",
            "evangelize_belief",
            "recruit_great_person",
            "patronize_great_person",
            "appoint_governor",
            "assign_governor",
            "reassign_governor",
            "promote_governor",
            "send_envoy",
            "levy_military",
            "trade_route",
            "found_corporation",
            "assign_spy",
            "spy_mission",
            "promote_spy",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("legalFor(\"{action}\")")),
                "no empire screen offers the {action} action to the player"
            );
        }
        // Actions are posted back exactly as the engine handed them over. A
        // screen that rebuilt one by hand would be inventing protocol.
        assert!(EMBEDDED_INDEX
            .contains("onclick='sendFromEmpire(${JSON.stringify(action)})'>${label}</button>"));
    }
}

pub fn serve_with_game(
    port: u16,
    open_browser: bool,
    params: Params,
    game: Option<Game>,
    initially_paused: bool,
) {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .unwrap_or_else(|e| panic!("cannot bind port {port}: {e}"));
    let actual = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{actual}/");
    let mut session = match game {
        Some(game) => Session::from_game(params, game),
        None => Session::new(params),
    };
    session.spectator_paused = initially_paused;
    println!("Martin Halvorson's Civilization VIS — playing at {url}");
    if session.params.spectate {
        println!(
            "Spectator mode: all {} players are AI-driven. Ctrl+C to quit.",
            session.params.num_players
        );
    } else {
        println!("You are player 0. Ctrl+C to quit.");
    }
    let shared = Arc::new(Shared {
        session: Mutex::new(session),
        pace_ms: AtomicU64::new(1_000), // one second per turn by default
        paused: AtomicBool::new(initially_paused),
        restart_in: AtomicU64::new(u64::MAX),
        turn_us: AtomicU64::new(0),
        turn_compute_us: AtomicU64::new(0),
        frame_delivery: Mutex::new(FrameDelivery::default()),
        frame_delivered: Condvar::new(),
    });
    let stepper = shared.clone();
    std::thread::spawn(move || auto_step_loop(stepper));
    if open_browser {
        open_url(&url);
    }
    // One connection at a time meant one slow request stopped the server
    // dead for everyone. /state builds close to a megabyte of observation and
    // the browser asks for it continuously, so on a loaded machine the
    // supervisor's health and game-over checks queued behind it - measured at
    // twenty-one seconds once and fifty-five another, with the game running
    // fine behind the stall. Each connection gets its own thread; the session
    // mutex still serialises the state itself, but only for as long as the
    // snapshot takes, not for the serialisation and the socket write too.
    for stream in listener.incoming() {
        if let Ok(mut s) = stream {
            let shared = shared.clone();
            std::thread::spawn(move || handle(&mut s, &shared));
        }
    }
}

pub fn serve(port: u16, open_browser: bool, params: Params) {
    serve_with_game(port, open_browser, params, None, false);
}

fn open_url(url: &str) {
    #[cfg(windows)]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
