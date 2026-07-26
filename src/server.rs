//! Zero-dependency local HTTP server for the human-vs-AI browser GUI.
//! Endpoints: GET / (page), GET /cinematic3d.js, GET /state, GET /save, GET /rules, GET /pedia,
//! POST /action, POST /step, POST /autoplay, POST /view,
//! POST /spectator-status, POST /next-game-settings, POST /new,
//! POST /supervisor-new.
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::ai::{AdvancedAi, Ai, BasicAi};
use crate::game::{Action, Game, GameOptions, PlayOnMode, VictoryConditions};
use crate::rules::Rules;
use crate::obs::{observation, observation_player_view, observation_spectator};
use crate::setup::{
    GameSpeed, MapPoles, MapScript, MapSize, MapTopology, CIV6_GAME_SPEEDS, CIV6_MAP_SCRIPTS,
    CIV6_MAP_SIZES, MAP_POLES, MAP_TOPOLOGIES,
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

/// The agents that exist in every build, whether or not a league snapshot is
/// on disk, with the handle the leaderboards give them. `make_send_ai`
/// resolves each id, and the auto-play control offers this list when there is
/// no roster to offer instead.
const BUILTIN_STRATEGIES: [(&str, &str); 4] = [
    ("advanced", "JackOfAllTrades"),
    ("advanced_evolved", "Evolved"),
    ("advanced_v1", "OldGuard"),
    ("basic", "TrainingWheels"),
];

#[derive(Clone)]
pub struct Params {
    pub num_players: usize,
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    pub map_script: MapScript,
    /// What shape the world is, chosen independently of what fills it.
    pub map_topology: MapTopology,
    /// Whether the world has cold ends.
    pub map_poles: MapPoles,
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
    /// The strategies auto-play can hand the human seat to. `league` above is
    /// the roster this game is *rated* against and is often absent; every
    /// build ships the committed snapshot under `data/league`, so the choice
    /// on offer is our bred strategies whether or not anything is being rated.
    /// Reading it is a labelling concern only: nothing here seats a rival.
    roster: Option<crate::league::League>,
    /// The strategy a player handed their own seat to, by roster name. Held
    /// separately from `seat_strategy[0]` because the roster it came from is
    /// not always the roster this game is rated against.
    autoplay_strategy: Option<String>,
    /// The last browser batch that borrowed the human seat, and how many
    /// turns it played. A client retries the same id after a dropped socket;
    /// remembering one completed batch makes that retry an acknowledgement,
    /// not a second run.
    last_autoplay_request: Option<(String, usize)>,
    /// Set once this game's result has been rated, so a winner that is
    /// stepped past more than once is only ever counted for one game.
    league_recorded: bool,
    /// Who is playing each human seat: a player registered when this game
    /// began, never one of the agents already in the roster.
    human_players: BTreeMap<usize, SeatPlayer>,
}

/// The identity of a person playing a seat.
///
/// A single player game registers a new player rather than lending the person
/// somebody else's name: the handle on screen is theirs, the rating beside it
/// is theirs, and the result is filed under it. `rated` says whether that
/// registration reached the roster on disk — without one there is nothing to
/// rate into, so the identity is the game's own and goes no further.
#[derive(Clone, Debug, PartialEq)]
struct SeatPlayer {
    name: String,
    username: String,
    rated: bool,
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
    frame_painted: Condvar,
    /// Serializes a displayed-state handoff with the start of an automatic AI
    /// step. If a new viewer arrives between the old frame check and the step,
    /// its first snapshot could otherwise be replaced before it paints. The
    /// request either wins and installs the paint obligation first, or the
    /// in-flight step wins and the viewer's first snapshot is the result.
    simulation_frame_gate: Mutex<()>,
    /// The most recent turn the stepper finished, and a bell rung when it
    /// changes. A page asks for whatever comes *after* the frame it is
    /// holding, and waits here until there is one.
    ///
    /// Without this, a page with no polling delay spins: `/state` answers at
    /// once with the turn it has already drawn, so it would rebuild a megabyte
    /// of observation over and over for a turn nobody needs again, competing
    /// with the simulation for the machine it is waiting on. With it, the last
    /// of the polling latency goes too — a finished turn is written to a
    /// socket the moment it exists rather than at the page's next tick.
    latest: Mutex<Option<SpectatorFrame>>,
    turn_ready: Condvar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SpectatorFrame {
    seed: u64,
    turn: u32,
}

/// One page, tracked apart from every other page.
///
/// Delivery used to be a single cursor for the whole server, which quietly
/// made the promise weaker the more people kept it: the stepper released a
/// turn as soon as *any* request had been handed it, so two tabs on one
/// exhibition took alternate turns and each saw half the game. The audit read
/// that same one cursor — the two tabs between them reported an unbroken run
/// of turns — so it reported nothing wrong. Every viewer is owed every turn,
/// so every viewer gets a seat of its own and the gate waits for all of them.
struct ViewerSeat {
    last_request: Instant,
    delivered: Option<SpectatorFrame>,
    /// The last frame this page reported having painted, and the turns it
    /// skipped getting there. A frame written to a socket is not yet a frame
    /// anybody saw, so the page says which turn it actually drew and the
    /// promise this gate exists to keep can be audited while it runs.
    painted: Option<SpectatorFrame>,
    missed: u64,
    /// A fingerprint of every tile this page was last sent, and the frame they
    /// belonged to. A spectator `/state` is about 1.4 MB and 1.2 MB of that is
    /// tiles, nearly all of which are the same terrain they were last turn, so
    /// what the page already holds is worth remembering rather than sending
    /// again.
    ///
    /// Eight bytes a tile rather than the tiles themselves. Keeping the parsed
    /// JSON would be about two kilobytes each — fine for the exhibition's 2252,
    /// a hundred megabytes on a large world, and that again for every tab
    /// watching. The walk that hashes a tile is the walk that would have
    /// compared it, so the bound is close to free.
    tiles: Option<(SpectatorFrame, Vec<u64>)>,
}

impl ViewerSeat {
    fn new(now: Instant) -> Self {
        ViewerSeat {
            last_request: now,
            delivered: None,
            painted: None,
            missed: 0,
            tiles: None,
        }
    }

    fn attached(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.last_request) <= VIEWER_ACTIVE
    }
}

#[derive(Default)]
struct FrameDelivery {
    seats: BTreeMap<String, ViewerSeat>,
    /// Every turn any viewer missed since this server started, kept apart from
    /// the seats so that closing the tab that missed them does not erase the
    /// record. A seat is retired six seconds after its page stops asking; the
    /// audit is for the whole run.
    missed: u64,
}

impl FrameDelivery {
    /// Forget the pages that stopped asking. A seat costs a cached copy of the
    /// world's tiles, and a closed tab must not go on holding turns open for a
    /// viewer that is not there to read them.
    fn retire_departed(&mut self, now: Instant) {
        self.seats.retain(|_, seat| seat.attached(now));
    }

    fn seat(&mut self, viewer: &str, now: Instant) -> &mut ViewerSeat {
        self.retire_departed(now);
        self.seats
            .entry(viewer.to_string())
            .or_insert_with(|| ViewerSeat::new(now))
    }

    fn frame_delivered(&mut self, viewer: &str, frame: SpectatorFrame, now: Instant) {
        let seat = self.seat(viewer, now);
        seat.last_request = now;
        seat.delivered = Some(frame);
    }

    /// A viewer's request, carrying the turn it says it painted since the last
    /// one. Turns between that and the previous report were simulated and
    /// never drawn — the exact failure the gate is here to prevent, counted
    /// rather than assumed away.
    ///
    /// Only counted against a viewer that never left. The promise is to a
    /// viewer that is *here*: an unattended exhibition runs flat out on
    /// purpose, so turns that went by while a tab was closed, reloading onto a
    /// swapped binary, or between two worlds are nobody's missed frames. A
    /// different world starts the count over for the same reason — seeds are
    /// unordered, and the turns before it were another game's.
    fn viewer_request(&mut self, viewer: &str, painted: Option<SpectatorFrame>, now: Instant) {
        // Read this before taking the seat: taking it retires the departed and
        // stamps the survivor with `now`, either of which would make a page
        // that has been away for a minute look like it never left.
        let attached = self.seats.get(viewer).is_some_and(|s| s.attached(now));
        let seat = self.seat(viewer, now);
        seat.last_request = now;
        // A viewer with nothing to report has painted nothing *yet*: a page
        // still booting, or one that just reloaded onto a swapped binary. The
        // turn it eventually draws does not follow whatever the last page
        // drew, so drop the baseline rather than score the gap between them.
        let Some(frame) = painted else {
            seat.painted = None;
            return;
        };
        // A query parameter is only an acknowledgement when this server
        // actually handed that exact snapshot to this exact page. Besides
        // rejecting fabricated/future acknowledgements, this keeps a request
        // issued before a slow render from releasing the turn it has not
        // finished painting yet.
        if seat.delivered != Some(frame) {
            return;
        }
        let mut lost = 0;
        if let Some(previous) = seat.painted {
            if attached && previous.seed == frame.seed && frame.turn > previous.turn {
                lost = u64::from(frame.turn - previous.turn - 1);
                seat.missed += lost;
            }
        }
        seat.painted = Some(frame);
        self.missed += lost;
    }

    /// How long the stepper must still hold this turn: the longest wait owed
    /// to any attached viewer that has not acknowledged painting its complete
    /// snapshot yet. Delivery alone is not enough: a socket is not a screen.
    /// `None` once every viewer present has painted it — or when nobody is
    /// watching at all.
    fn wait_remaining(&self, frame: SpectatorFrame, now: Instant) -> Option<Duration> {
        self.seats
            .values()
            .filter(|seat| seat.painted != Some(frame))
            .filter_map(|seat| {
                VIEWER_ACTIVE
                    .checked_sub(now.saturating_duration_since(seat.last_request))
                    .filter(|remaining| !remaining.is_zero())
            })
            .max()
    }
}

const MIN_RESTART_MS: u64 = 5_000;
/// How long after its last request a viewer is still considered present, and
/// so still owed a frame for every turn.
///
/// This has to outlast a whole slow paint. A page painting a megabyte of
/// observation is single-threaded and cannot say it is still there while it
/// works, so a viewer that is merely slow is indistinguishable from one that
/// closed the tab — and at two seconds a headless paint of about that length
/// was being read as a departure, dropping the turn it was in the middle of.
/// Six seconds covers a bad paint on a loaded machine with room to spare.
///
/// It costs almost nothing to be generous. A viewer that really has gone
/// delays exactly one turn: the next turn's wait is already past the window,
/// and the exhibition runs unattended at full speed from there.
const VIEWER_ACTIVE: Duration = Duration::from_secs(6);
const FRAME_WAIT_RECHECK: Duration = Duration::from_millis(100);
/// The longest a page's poll is held open waiting for the next turn before it
/// is answered with the one it already has. Short enough that a finished
/// game's restart countdown still ticks over once a second on screen.
const STATE_LONG_POLL: Duration = Duration::from_millis(1_000);
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
    /// Announce a finished turn to the pages parked waiting for one.
    fn note_turn_ready(&self, frame: SpectatorFrame) {
        *self.latest.lock().unwrap() = Some(frame);
        self.turn_ready.notify_all();
    }

    /// Park a page until the game is past the frame it says it holds.
    ///
    /// A page that holds nothing, or holds a turn this server has already left
    /// behind, is answered immediately — including every reader that is not a
    /// viewer at all, so a health check is never made to wait. The cap is what
    /// keeps a finished game's restart countdown ticking on screen while
    /// nothing is being simulated at all.
    fn wait_for_next_turn(&self, have: Option<SpectatorFrame>) {
        let Some(held) = have else { return };
        let deadline = Instant::now() + STATE_LONG_POLL;
        let mut latest = self.latest.lock().unwrap();
        loop {
            // What the game is on, rather than only what the stepper last
            // announced. A world replaced outright — a new game, a save loaded
            // — never completed a turn to announce, and a page holding the old
            // one would otherwise sit here until the cap ran out.
            //
            // The bell is held across this read so the answer cannot arrive
            // between looking and listening. Nothing holds the session lock
            // while ringing it, so taking them in this order is safe.
            let current = {
                let session = self.session.lock().unwrap();
                SpectatorFrame {
                    seed: session.game.seed,
                    turn: session.game.turn,
                }
            };
            if current != held {
                return;
            }
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return;
            };
            latest = self.turn_ready.wait_timeout(latest, left).unwrap().0;
        }
    }

    fn note_frame_delivered(&self, viewer: &str, frame: SpectatorFrame) {
        self.frame_delivery
            .lock()
            .unwrap()
            .frame_delivered(viewer, frame, Instant::now());
    }

    fn note_viewer_request(&self, viewer: &str, painted: Option<SpectatorFrame>) {
        self.frame_delivery
            .lock()
            .unwrap()
            .viewer_request(viewer, painted, Instant::now());
        // This is the only event that can satisfy Martin's complete-frame
        // simulation gate. Merely writing the state to a socket must never
        // advance the simulation.
        self.frame_painted.notify_all();
    }

    /// Turns this server simulated that some viewer never drew, the last turn
    /// one reported drawing, and how many pages are watching. The second reads
    /// the first: no painted turn at all means nobody was watching, which is a
    /// different thing from a promise being kept. The third says how many
    /// pages that "no turns missed" is a promise to.
    fn frame_audit(&self) -> (u64, Option<u32>, usize) {
        let mut delivery = self.frame_delivery.lock().unwrap();
        delivery.retire_departed(Instant::now());
        let missed = delivery.missed;
        let painted = delivery
            .seats
            .values()
            .filter_map(|seat| seat.painted.map(|frame| frame.turn))
            .max();
        (missed, painted, delivery.seats.len())
    }

    /// Replace the tile array in `o` with just the tiles that have changed
    /// since this viewer's last one.
    ///
    /// Tiles are 1.2 MB of a 1.4 MB spectator state and the overwhelming
    /// majority of them are the terrain they have been since the map was
    /// generated. Sending all of it every turn is what made a viewer cost the
    /// exhibition a quarter of a second per turn — serialising it here,
    /// pushing it through a socket, and parsing it there — and that quarter
    /// second was being paid out of the turn rate, because the gate holds each
    /// turn until the page has it.
    ///
    /// `have` is the turn the page says its own copy is built from, so the
    /// baseline is what the page *holds*, never what was last written at it: a
    /// response that never arrived leaves the two disagreeing, and disagreeing
    /// costs one full array rather than a silently wrong map. Indices are
    /// stable to compare against because the array is built from an explored
    /// set that only ever grows, so equal lengths mean equal membership in the
    /// same order — and a length that differs sends the whole thing.
    fn deliver_tiles(
        &self,
        viewer: &str,
        frame: SpectatorFrame,
        have: Option<SpectatorFrame>,
        o: &mut Value,
    ) {
        // Lift the array out of the map rather than blanking it in place: a
        // patched response carries no `tiles` key at all, and a null one would
        // read to the page as a world with no ground in it.
        let Some(Value::Object(map)) = o.get_mut("map") else {
            return;
        };
        let Some(Value::Array(tiles)) = map.remove("tiles") else {
            return;
        };
        let marks: Vec<u64> = tiles.iter().map(tile_mark).collect();
        let now = Instant::now();
        let mut delivery = self.frame_delivery.lock().unwrap();
        let seat = delivery.seat(viewer, now);
        let changed: Option<Vec<Value>> = seat
            .tiles
            .as_ref()
            .filter(|(held, cached)| {
                held.seed == frame.seed && Some(*held) == have && cached.len() == marks.len()
            })
            .map(|(_, cached)| {
                tiles
                    .iter()
                    .enumerate()
                    .filter(|(at, _)| cached[*at] != marks[*at])
                    .map(|(at, tile)| json!([at, tile]))
                    .collect()
            });
        match changed {
            Some(changed) => {
                // The map key carries the patch and no `tiles` at all: a page
                // that reported a baseline is holding one, and a full array
                // arriving anyway would be a megabyte saying nothing.
                o["map"]["tiles_from"] = json!(have.map(|held| held.turn));
                o["map"]["tiles_changed"] = Value::Array(changed);
            }
            None => o["map"]["tiles"] = Value::Array(tiles),
        }
        seat.tiles = Some((frame, marks));
    }

    /// Hold the stepper until every active viewer has painted `frame`.
    ///
    /// A turn budget is a floor on how long a turn takes. It was being relied
    /// on as something it never was — a promise that a browser could read the
    /// turn before it was replaced. A page that needs longer to paint a
    /// megabyte of observation than the budget allows loses turns outright,
    /// and loses them silently: five of twenty-eight on the default Blitz pace
    /// with a slow paint. Martin's simulation requirement is stricter than
    /// delivery: the updated map, HUD, victory tracker, and every other
    /// turn-bound surface must complete one shared-snapshot render before the
    /// next turn begins. With no viewer inside `VIEWER_ACTIVE` there is
    /// nothing to wait for and an unattended exhibition still runs flat out.
    fn wait_for_turn_frame(&self, frame: SpectatorFrame) {
        let mut delivery = self.frame_delivery.lock().unwrap();
        loop {
            let Some(remaining) = delivery.wait_remaining(frame, Instant::now()) else {
                return;
            };
            let result = self
                .frame_painted
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
    ///
    /// A seat somebody is playing is never seated from the roster. Whoever is
    /// at the keyboard is their own player — `register_human_players` gives
    /// them a new one — and an entrant that had this seat handed to it would
    /// wear a person's game as its own result.
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
                .filter(|p| !p.is_minor && !p.is_barbarian && !game.is_human_seat(p.id))
                .map(|p| p.id)
                .collect();
            if seat_from_roster && !l.active().is_empty() {
                let civs: Vec<String> =
                    majors.iter().map(|id| game.players[*id].civ.clone()).collect();
                for (id, pick) in majors.iter().zip(crate::league::seat_by_civ_seeded(
                    l,
                    &civs,
                    game.seed,
                    3,
                )) {
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

    /// The strategies auto-play may offer. Prefer whatever this game is
    /// already rated against, so the ratings shown are the ones in play; fall
    /// back to the snapshot every build ships, so the control still names our
    /// bred strategies in a game that is rating nothing.
    fn load_roster(league: Option<&crate::league::League>) -> Option<crate::league::League> {
        match league {
            Some(l) => Some(l.clone()),
            None => crate::league::load_league("data/league"),
        }
    }

    /// The roster named by `--league`, else a best-effort `league/` load
    /// purely for elo labels.
    fn load_params_league(params: &Params) -> (Option<crate::league::League>, bool) {
        match &params.league_dir {
            Some(dir) => (crate::league::load_league(dir), true),
            None => (crate::league::load_league("league"), false),
        }
    }

    /// Register a new player for every seat a person is at, and hand the seat
    /// that identity.
    ///
    /// Sitting down to play does not make you one of the agents on the
    /// leaderboard. When this game is being rated (`--league --league-record`)
    /// the new player is written into that roster, so `record_league_result`
    /// files the result under a name that is the person's own; otherwise
    /// there is nothing to rate into and the handle is minted against the
    /// roster in memory purely so the game can say who is playing. Either way
    /// no existing entrant is reused.
    fn register_human_players(
        params: &Params,
        game: &Game,
        league: &mut Option<crate::league::League>,
        seat_strategy: &mut [Option<usize>],
    ) -> BTreeMap<usize, SeatPlayer> {
        let mut players = BTreeMap::new();
        let rated_dir = params
            .league_record
            .then(|| params.league_dir.clone())
            .flatten();
        // Handles for an unrated game are drawn against this scratch roster,
        // so two seats in one game cannot mint the same one.
        let mut unrated: Option<crate::league::League> = None;
        for seat in game.human_seats.iter().copied() {
            if game.players.get(seat).is_none() {
                continue;
            }
            let registered = rated_dir
                .as_deref()
                .and_then(crate::league::register_player);
            let player = match registered {
                Some((updated, index)) => {
                    let entry = &updated.strategies[index];
                    let player = SeatPlayer {
                        name: entry.name.clone(),
                        username: entry.username.clone(),
                        rated: true,
                    };
                    seat_strategy[seat] = Some(index);
                    *league = Some(updated);
                    player
                }
                None => {
                    let table = unrated.get_or_insert_with(|| {
                        league.clone().unwrap_or(crate::league::League {
                            round: 0,
                            strategies: Vec::new(),
                            calibration: Default::default(),
                        })
                    });
                    let index = crate::league::register_new_player(table);
                    let entry = &table.strategies[index];
                    SeatPlayer {
                        name: entry.name.clone(),
                        username: entry.username.clone(),
                        rated: false,
                    }
                }
            };
            players.insert(seat, player);
        }
        players
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
            map_topology: params.map_topology,
            map_poles: params.map_poles,
            difficulty: params.difficulty.clone(),
            speed: params.speed.clone(),
            human_seats,
            teams: params.teams.clone(),
            civs: params.civs.clone(),
            randomize_civs: true,
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
        let (mut league, seat_from_roster) = Self::load_params_league(&params);
        let (ais, mut seat_strategy) = Self::ai_fleet(&game, league.as_ref(), seat_from_roster);
        let human_players =
            Self::register_human_players(&params, &game, &mut league, &mut seat_strategy);
        let chronicle = ChronicleState::from_game(&game);
        let roster = Self::load_roster(league.as_ref());
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
            roster,
            autoplay_strategy: None,
            last_autoplay_request: None,
            league_recorded: false,
            human_players,
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
        params.map_topology = if game.map.topology == crate::world::Topology::Cylinder {
            MapTopology::Flat
        } else {
            MapTopology::Planet
        };
        params.map_poles = game.map_poles;
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
        let (mut league, seat_from_roster) = Self::load_params_league(&params);
        let (ais, mut seat_strategy) = Self::ai_fleet(&game, league.as_ref(), seat_from_roster);
        let chronicle = ChronicleState::from_game(&game);
        // A match restored with its winner already decided was rated when it
        // finished; rating it again on the next step would count it twice.
        let league_recorded = game.winner.is_some();
        // A save carries the world, not the person: whoever reloads it is a
        // new player again, and a decided game has nothing left to rate, so
        // it registers nobody.
        let human_players = if league_recorded {
            BTreeMap::new()
        } else {
            Self::register_human_players(&params, &game, &mut league, &mut seat_strategy)
        };
        let roster = Self::load_roster(league.as_ref());
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
            roster,
            autoplay_strategy: None,
            last_autoplay_request: None,
            league_recorded,
            human_players,
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
        // The supervisor owns the exhibition: every AI-only world is a fresh
        // process on freshly built code, so this process may not replace one
        // in place. A game somebody sits down to play is not part of that
        // cycle — it takes this process over exactly as it would on a server
        // nobody is supervising, and the supervisor leaves it alone until it
        // is over.
        if self.params.supervised && request["spectate"].as_bool() != Some(false) {
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

    /// Say who is playing each human seat: the handle this game registered
    /// for them, and — once a rated roster is holding it — the rating they
    /// are defending. A player with no finished game is `player_rated` with
    /// zero games rather than an authoritative 1500, the same way the
    /// leaderboards mark a provisional entrant.
    fn name_human_players(&self, o: &mut Value) {
        if self.human_players.is_empty() {
            return;
        }
        let Some(players) = o["players"].as_array_mut() else {
            return;
        };
        for player in players {
            let Some(id) = player["id"].as_u64().map(|id| id as usize) else {
                continue;
            };
            let Some(seat) = self.human_players.get(&id) else {
                continue;
            };
            player["player_name"] = json!(seat.name);
            player["player_username"] = json!(seat.username);
            player["player_rated"] = json!(seat.rated);
            let rating = self
                .league
                .as_ref()
                .zip(self.seat_strategy.get(id).copied().flatten())
                .map(|(league, index)| &league.strategies[index]);
            if let Some(entry) = rating {
                let civ = &self.game.players[id].civ;
                let (elo, rd, civ_specific) = crate::league::display_elo(entry, civ);
                player["player_elo"] = json!(elo.round() as i64);
                player["player_elo_rd"] = json!(rd.round() as i64);
                player["player_elo_civ"] = json!(civ_specific);
                player["player_games"] = json!(entry.games);
            }
        }
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
                    // A perspective the observation has already withheld does
                    // not get its plan, its handle or its rating pinned back
                    // on here. Only the omniscient view annotates everyone.
                    if player["met"] == json!(false) {
                        continue;
                    }
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
        self.name_human_players(&mut o);
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

    /// Hand seat 0 to a named strategy, so auto-play runs *that* agent rather
    /// than whichever one the fleet happened to build for the seat.
    ///
    /// A name is matched against the league roster first — by entrant name or
    /// by the handle the leaderboards show — and then against the built-in
    /// agents, so a build with no roster on disk still has something to hand
    /// the seat to. An unknown name is an error rather than a silent fallback:
    /// a player who picked a strategy and got a different one has been lied to.
    pub fn seat_strategy_at(&mut self, seat: usize, name: &str) -> Result<(), String> {
        if name.is_empty() || self.autoplay_strategy.as_deref() == Some(name) {
            return Ok(());
        }
        let seed = self.game.seed.wrapping_add(seat as u64);
        let kind = self
            .roster
            .as_ref()
            .and_then(|roster| {
                roster
                    .strategies
                    .iter()
                    .find(|s| s.name == name || s.username == name)
                    .map(|s| s.kind.clone())
            })
            .or_else(|| {
                BUILTIN_STRATEGIES.iter().any(|(id, _)| *id == name).then(|| {
                    crate::league::StrategyKind::Builtin { ai: name.to_string() }
                })
            })
            .ok_or_else(|| format!("no strategy named {name}"))?;
        self.ais[seat] = crate::league::make_send_ai(&kind, seed);
        // The rated roster and the offered roster can be different rosters, so
        // only claim a rated identity for the seat when this name is in the
        // rated one; the name below is what the browser is told either way.
        self.seat_strategy[seat] = self
            .league
            .as_ref()
            .and_then(|l| l.strategies.iter().position(|s| s.name == name || s.username == name));
        self.autoplay_strategy = Some(name.to_string());
        Ok(())
    }

    /// The strategy currently playing `seat`, by roster name: the one a player
    /// handed the seat to, else whichever entrant the fleet seated there.
    pub fn seated_strategy_name(&self, seat: usize) -> Option<&str> {
        if seat == 0 {
            if let Some(name) = self.autoplay_strategy.as_deref() {
                return Some(name);
            }
        }
        // Nobody has been handed this seat and somebody is sitting in it. The
        // honest answer is that person, not the agent that would take over if
        // they got up.
        if let Some(player) = self.human_players.get(&seat) {
            return Some(&player.name);
        }
        if let (Some(Some(index)), Some(league)) = (self.seat_strategy.get(seat), self.league.as_ref())
        {
            return Some(league.strategies[*index].name.as_str());
        }
        // No rated identity for the seat. That does not make it nameless: the
        // fleet built the default agent there, and the roster's name for that
        // agent is "advanced" — the cheaper baseline for minors.
        let player = self.game.players.get(seat)?;
        Some(if player.is_minor || player.is_barbarian { "basic" } else { "advanced" })
    }

    /// Hand the player's own seat to the AI for `turns` turns.
    ///
    /// Unciv calls this AutoPlay, and it earns its keep in the same two
    /// places: skipping a stretch of a game that has already been decided,
    /// and watching how the agent would have played a position you are in.
    /// Seat 0 already has an agent built for it — in a human game it simply
    /// never gets asked — so this is a matter of asking it.
    ///
    /// "Play the rest of it" is bounded by the live turn limit. A continued
    /// game has no such limit, so a single HTTP request gets a generous finite
    /// batch instead; the browser can keep requesting batches until the next
    /// result or until the person stops an indefinite run.
    pub fn autoplay(&mut self, turns: u32) -> usize {
        let mut played = 0;
        let remaining = self.game.turn_limit().map_or_else(
            || turns.min(250),
            |limit| limit.saturating_sub(self.game.turn).saturating_add(1),
        );
        for _ in 0..turns.min(remaining) {
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
    /// "One more turn": put the decided world back into play.
    ///
    /// A victory can be declared in the middle of a round, which leaves the
    /// turn parked on whichever seat was up. A spectated world does not care —
    /// the stepper plays whoever is current — but a game somebody is playing
    /// would come back live on an AI seat, refusing every action the person
    /// tried to take. So the same catch-up `act` runs after an end-turn runs
    /// here, handing the round back to seat zero.
    pub fn play_on(&mut self, mode: PlayOnMode) -> bool {
        if !self.game.play_on(mode) {
            return false;
        }
        if !self.params.spectate {
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
        }
        true
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

/// Where a single-player game keeps its own saves, relative to the process's
/// working directory. Files are named `*.save.json`, which `.gitignore`
/// already covers, so a game played inside a checkout leaves the tree clean.
const SAVE_DIR: &str = "saves";
/// How many turn-stamped autosaves to keep. Civ 6 keeps a rolling handful for
/// the same reason: the useful save is rarely the newest one.
const AUTOSAVES: usize = 5;

/// A save name is used to build a path, so it is checked rather than trusted:
/// no separators, no traversal, nothing exotic. Returns the file path.
fn save_path(name: &str) -> Option<std::path::PathBuf> {
    let name = name.trim();
    if name.is_empty()
        || name.len() > 64
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some(std::path::Path::new(SAVE_DIR).join(format!("{name}.save.json")))
}

/// Write a save whole or not at all. A game interrupted mid-write is exactly
/// the game most likely to be reloaded, and a half-written save reads as a
/// corrupt one.
fn write_save(game: &Game, path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("writing");
    std::fs::write(&temporary, serde_json::to_vec(game)?)?;
    std::fs::rename(&temporary, path)
}

/// Every save this process can see, newest turn first, with enough of each to
/// choose between them without loading any.
fn list_saves() -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir(SAVE_DIR) else {
        return Vec::new();
    };
    let mut saves: Vec<Value> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.strip_suffix(".save.json")?.to_string();
            let raw = std::fs::read(&path).ok()?;
            let game: Game = serde_json::from_slice(&raw).ok()?;
            let leader = game
                .players
                .iter()
                .find(|player| !player.is_minor && !player.is_barbarian)
                .map(|player| player.civ.clone());
            Some(json!({
                "name": name,
                "turn": game.turn,
                "seed": game.seed,
                "civ": leader,
                "difficulty": game.difficulty,
                "speed": game.game_speed.id(),
                "winner": game.winner,
                "bytes": raw.len(),
            }))
        })
        .collect();
    saves.sort_by_key(|save| std::cmp::Reverse(save["turn"].as_u64().unwrap_or(0)));
    saves
}

/// Keep the newest `AUTOSAVES` turn-stamped autosaves and drop the rest.
fn prune_autosaves() {
    let Ok(entries) = std::fs::read_dir(SAVE_DIR) else {
        return;
    };
    let mut stamped: Vec<(u32, std::path::PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            let turn = name
                .strip_prefix("autosave-t")?
                .strip_suffix(".save.json")?
                .parse::<u32>()
                .ok()?;
            Some((turn, path))
        })
        .collect();
    stamped.sort_by_key(|(turn, _)| std::cmp::Reverse(*turn));
    for (_, path) in stamped.into_iter().skip(AUTOSAVES) {
        let _ = std::fs::remove_file(path);
    }
}

/// Write one complete HTTP response, returning whether it reached the socket.
/// Callers normally have nothing useful to do with a disconnected client, but
/// completed-turn delivery uses the result to record which exact snapshot the
/// page is later allowed to acknowledge painting.
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

/// One parameter out of a request target's query, or `None` if the request
/// did not carry the key at all. A key present with an empty value reads as
/// `Some("")`: the page announces itself as a viewer on its very first poll,
/// before it has painted anything to report.
fn query_value<'a>(target: &'a str, key: &str) -> Option<&'a str> {
    let (_, query) = target.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        (name == key).then_some(value)
    })
}

/// A structural fingerprint of one tile, for telling whether it has changed
/// since a viewer was last sent it without keeping a copy of it to compare.
///
/// Deterministic across turns because the map it walks is ordered — sorted by
/// key on the default `serde_json`, insertion-ordered under `preserve_order`,
/// and either way the same builder produces the same shape every turn. Kinds
/// are tagged so that `null`, `false` and `0` cannot agree by coincidence, and
/// numbers are tagged by how they read back so `1` and `1.0` do not either.
fn hash_json(value: &Value, into: &mut DefaultHasher) {
    match value {
        Value::Null => 0u8.hash(into),
        Value::Bool(flag) => {
            1u8.hash(into);
            flag.hash(into);
        }
        Value::Number(number) => {
            2u8.hash(into);
            match (number.as_i64(), number.as_u64(), number.as_f64()) {
                (Some(whole), _, _) => (0u8, whole).hash(into),
                (_, Some(whole), _) => (1u8, whole).hash(into),
                (_, _, Some(real)) => (2u8, real.to_bits()).hash(into),
                _ => 3u8.hash(into),
            }
        }
        Value::String(text) => {
            3u8.hash(into);
            text.hash(into);
        }
        Value::Array(items) => {
            4u8.hash(into);
            items.len().hash(into);
            for item in items {
                hash_json(item, into);
            }
        }
        Value::Object(fields) => {
            5u8.hash(into);
            fields.len().hash(into);
            for (key, item) in fields {
                key.hash(into);
                hash_json(item, into);
            }
        }
    }
}

fn tile_mark(tile: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_json(tile, &mut hasher);
    hasher.finish()
}

/// The frame a page says its own copy of the tiles is built from, written
/// `world:turn`. Both halves matter: a patch is only meaningful against the
/// exact array the server sent, and turn 5 of one world is not turn 5 of the
/// next. Anything unparseable simply means no baseline, and the page is sent
/// the whole map.
fn held_frame(token: &str) -> Option<SpectatorFrame> {
    let (seed, turn) = token.split_once(':')?;
    Some(SpectatorFrame {
        seed: seed.parse().ok()?,
        turn: turn.parse().ok()?,
    })
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
        "shape": params.map_topology.id(),
        "poles": params.map_poles.id(),
        "speed": params.game_speed.id(),
        "victories": victories,
    })
}

/// The agents a person can hand their seat to, strongest first.
///
/// With a league roster on disk this is every entrant still competing, with
/// the rating it is defending, so the choice is between *our* strategies and
/// not between adjectives. An entrant that has not played a rated game yet is
/// marked provisional rather than shown as an authoritative 1500. Without a
/// roster the list falls back to the built-in agents, because a control with
/// nothing in it is worse than one with four honest entries.
fn strategy_roster(session: &Session) -> Value {
    let mut rows: Vec<Value> = Vec::new();
    if let Some(roster) = session.roster.as_ref() {
        // Agents only. A person registered in this roster is a player in it,
        // but a seat cannot be handed to somebody who is not at a keyboard.
        let mut active: Vec<&crate::league::Strategy> = roster
            .strategies
            .iter()
            .filter(|s| !s.retired && !s.human)
            .collect();
        active.sort_by(|a, b| b.rating.total_cmp(&a.rating));
        rows.extend(active.into_iter().map(|s| {
            json!({
                "name": s.name,
                "username": s.username,
                "label": s.label(),
                "rating": s.rating.round(),
                "games": s.games,
                "wins": s.wins,
                "provisional": s.games == 0,
            })
        }));
    }
    if rows.is_empty() {
        rows.extend(BUILTIN_STRATEGIES.iter().map(|(name, username)| {
            json!({
                "name": name,
                "username": username,
                "label": name,
                "provisional": true,
            })
        }));
    }
    json!(rows)
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
        // `planet` used to name a world type; it now names a shape. A client
        // still asking for it by the old name means both halves of what it
        // used to mean, so the shape comes along with the type.
        if request["map_script"].as_str() == Some("planet") {
            p.map_topology = MapTopology::Planet;
        }
    }
    if let Some(v) = request["map_topology"].as_str().and_then(MapTopology::from_id) {
        p.map_topology = v;
    }
    if let Some(v) = request["map_poles"].as_str().and_then(MapPoles::from_id) {
        p.map_poles = v;
    }
    if let Some(v) = request["map_poles"].as_bool() {
        p.map_poles = if v { MapPoles::Poles } else { MapPoles::NoPoles };
    }
    // Earth is drawn from real longitudes and latitudes and closes on itself,
    // so it is always a globe whatever shape the lobby asked for.
    if p.map_script.is_fixed_geography() {
        p.map_topology = MapTopology::Planet;
    }
    // A globe is stored in a rectangle of its own shape, so the chosen size is
    // re-expressed whenever either the size or the shape moves, and the lobby
    // always names the world it is about to build.
    if p.map_topology.is_globe() {
        let frequency = crate::mapgen::globe_frequency(p.width, p.height);
        p.width = crate::sphere::Sphere::width_for(frequency);
        p.height = crate::sphere::Sphere::height_for(frequency);
    } else if let Some(size) = MapSize::from_dimensions(p.width, p.height) {
        p.width = size.width;
        p.height = size.height;
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
        // Close the first-viewer race as well as the steady-state one. A page
        // attaching to the current turn must either finish registering and
        // receive that snapshot before this step begins, or wait until the
        // step completes and receive the next snapshot as its first frame.
        // Once registered, its current frame must be painted before any more
        // simulation work starts.
        let simulation_frame_gate = sh.simulation_frame_gate.lock().unwrap();
        let current_frame = {
            let s = sh.session.lock().unwrap();
            SpectatorFrame {
                seed: s.game.seed,
                turn: s.game.turn,
            }
        };
        sh.wait_for_turn_frame(current_frame);
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
                    // A world's opening turn is a turn, and it is the one turn
                    // no seat has to complete for it to exist. Gate it like
                    // any other or the stepper plays straight through the
                    // starting position — settlers before their capitals —
                    // and the first thing a viewer ever sees of a new world is
                    // already several turns into it.
                    completed_frame = Some(SpectatorFrame {
                        seed: s.game.seed,
                        turn: s.game.turn,
                    });
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
                // The step that ends a game has to hand the viewer its
                // countdown in the same breath. Arming it on the next pass
                // instead left `/state` reporting no countdown at all for a
                // beat, so the result screen opened on "preparing the next
                // world" and only then began counting down from five — and the
                // window in which "one more turn" can be pressed is exactly
                // the window the countdown describes.
                if s.game.winner.is_some() {
                    over_since = Some(Instant::now());
                    sh.restart_in.store(
                        final_countdown_ms(s.params.restart_ms),
                        Ordering::Relaxed,
                    );
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
        // An active browser paints exactly one complete, same-snapshot frame
        // before the next round starts, at every pace. This makes Lightning as
        // fast as the viewer can paint without letting it skip whole turns,
        // and it stops the paced settings from skipping them too — a turn
        // budget says how long a turn lasts, not that anyone saw its updated
        // HUD, victory tracker, map, and remaining turn-bound surfaces.
        //
        // The wait comes before the cadence sleep on purpose. `elapsed_ms`
        // below measures from the top of the step, so a viewer who answers
        // inside the seat's own slice costs the exhibition nothing at all;
        // only one slower than the pace slows the pace down. With no recent
        // viewer the wait returns at once and nothing is throttled.
        if let Some(frame) = completed_frame {
            // Wake the pages parked on "whatever comes after what I hold"
            // before waiting on them to take it, or the two would deadlock on
            // each other for the length of the poll cap, every single turn.
            sh.note_turn_ready(frame);
            sh.wait_for_turn_frame(frame);
        }
        drop(simulation_frame_gate);
        if pace == 0 && !waiting {
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
    let request_target = parts.next().unwrap_or("/").to_string();
    let path = request_path(&request_target).to_string();
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
            // A Planet world is a globe, and a client cannot draw one from tile
            // coordinates alone. It asks for the sphere's geometry the first
            // time it sees one; the ordinary poll never carries it.
            let wants_planet = query_value(&request_target, "planet") == Some("1");
            // Only a page that paints frames holds the simulation to a turn.
            // The keeper's refresh check reads `/state` too, as does any
            // curl, and a poller that draws nothing must not drag the
            // exhibition down to its own cadence. A viewer identifies itself
            // by reporting the turn it last painted; everyone else reads the
            // same state and is not counted.
            let painting_viewer = query_value(&request_target, "painted");
            // Which page is asking. Every viewer is owed every turn, so they
            // are counted and waited for one at a time; a page that names
            // itself gets a seat of its own, and one too old to know to (a tab
            // open across a binary swap) shares the unnamed seat, which is the
            // single-cursor behaviour it was written against.
            let viewer = query_value(&request_target, "viewer").unwrap_or("").to_string();
            // The frame the page's own tile array is built from, which is not
            // the frame it painted: a state can arrive, patch the tiles and
            // still fail to draw. It names its world as well as its turn — a
            // page holding turn 5 of the world before this one must not be
            // handed a patch against turn 5 of this one.
            let have = query_value(&request_target, "have").and_then(held_frame);
            // A first snapshot and the start of an automatic step are atomic
            // with respect to one another. Holding this only for a page that
            // reports it has painted nothing yet avoids blocking ordinary
            // long polls and full-map resyncs: those must reach the server
            // carrying the painted acknowledgement that releases it.
            let _first_frame = if painting_viewer == Some("") && have.is_none() {
                Some(sh.simulation_frame_gate.lock().unwrap())
            } else {
                None
            };
            if let Some(reported) = painting_viewer {
                let painted = match (
                    reported.parse::<u32>(),
                    query_value(&request_target, "world").map(str::parse::<u64>),
                ) {
                    (Ok(turn), Some(Ok(seed))) => Some(SpectatorFrame { seed, turn }),
                    _ => None, // a page that has painted nothing yet
                };
                sh.note_viewer_request(&viewer, painted);
            }
            // A page that says what it holds is asking for the next turn, not
            // this one again, so it waits here instead of spinning on a clock
            // of its own. A reader that names nothing is answered at once.
            sh.wait_for_next_turn(have);
            let (mut o, frame) = {
                let session = sh.session.lock().unwrap();
                let frame = SpectatorFrame {
                    seed: session.game.seed,
                    turn: session.game.turn,
                };
                let mut observed = session.state();
                if wants_planet {
                    if let Some(geometry) = crate::obs::planet_geometry(&session.game) {
                        observed["map"]["planet"] = geometry;
                    }
                }
                (observed, frame)
            };
            decorate(&mut o, sh);
            if painting_viewer.is_some() {
                sh.deliver_tiles(&viewer, frame, have, &mut o);
            }
            if respond_json(stream, &o) && painting_viewer.is_some() {
                // Remember which exact snapshot this page is allowed to
                // acknowledge. Delivery does not release the stepper: only
                // the page's next request, after its synchronous map + HUD +
                // victory-tracker render completes, can do that.
                sh.note_frame_delivered(&viewer, frame);
            }
        }
        // Everything a supervisor needs to know - is there a game, is it over -
        // without building the whole observation. /state runs close to a
        // megabyte of JSON on a standard map, and something polling it every
        // few seconds to read one field spends the server's time on rendering
        // a view nobody looks at.
        ("GET", "/status") => {
            let (frames_missed, frames_painted, viewers) = sh.frame_audit();
            let session = sh.session.lock().unwrap();
            let game = &session.game;
            respond_json(
                stream,
                &json!({
                    "turn": game.turn,
                    "winner": game.winner,
                    "victory_type": game.victory_type,
                    "spectate": session.params.spectate,
                    // Turns this server simulated that no viewer ever drew.
                    // Every turn is supposed to reach the page as one whole
                    // frame, and the page reports the turns it paints, so the
                    // promise is measured rather than assumed. A healthy
                    // exhibition holds this at zero.
                    "frames_missed": frames_missed,
                    // The last turn a viewer reported drawing; null when
                    // nobody is watching, which is why zero misses on its own
                    // is not yet good news.
                    "frames_painted": frames_painted,
                    // How many pages that promise is being kept to. Each is
                    // waited for separately, so this is also the number of
                    // paints a turn now costs before the next one starts.
                    "viewers": viewers,
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
        // The saves this process can see, newest turn first.
        // Where a unit would step next on its way somewhere far. `path_to`
        // only searches this turn's movement, so a click on a distant tile is
        // "unreachable" and the client has no way to offer Civ 6's "go there".
        // `route_step` is the router the AI already uses: it plans across
        // future turns, around mountains, coastlines and choke points, and
        // returns the first step. Read-only — the client still sends a normal
        // Move for the step it is given, so the engine remains the authority
        // on whether that move is legal now.
        ("POST", "/route") => {
            let session = sh.session.lock().unwrap();
            let unit = parsed["unit"].as_u64().map(|unit| unit as u32);
            let to = parsed["to"]
                .as_array()
                .and_then(|pos| Some((pos.first()?.as_i64()? as i32, pos.get(1)?.as_i64()? as i32)));
            let answer = match (unit, to) {
                (Some(unit), Some(to)) => {
                    let owned = session
                        .game
                        .units
                        .get(&unit)
                        .is_some_and(|held| held.owner == 0);
                    if !owned {
                        json!({"error": "not your unit"})
                    } else {
                        match session.game.route_step(unit, to, 0) {
                            Some(step) => json!({"step": [step.0, step.1], "error": Value::Null}),
                            None => json!({"step": Value::Null, "error": Value::Null}),
                        }
                    }
                }
                _ => json!({"error": "route needs a unit and a destination"}),
            };
            drop(session);
            respond_json(stream, &answer);
        }
        ("GET", "/saves") => {
            respond_json(stream, &json!({"saves": list_saves()}));
        }
        // Name a save and it is written to disk; the browser can then offer
        // it back later instead of asking the player to keep a JSON file.
        ("POST", "/save") => {
            let name = parsed["name"].as_str().unwrap_or("").to_string();
            let Some(path) = save_path(&name) else {
                respond_json(stream, &json!({"error": "a save name is letters, digits, - and _"}));
                return;
            };
            let session = sh.session.lock().unwrap();
            let result = write_save(&session.game, &path);
            let turn = session.game.turn;
            drop(session);
            respond_json(
                stream,
                &match result {
                    Ok(()) => json!({"error": Value::Null, "name": name, "turn": turn}),
                    Err(error) => json!({"error": format!("cannot write {name}: {error}")}),
                },
            );
        }
        // Restore a game: `{"name": "…"}` for one of this process's saves, or
        // `{"game": {…}}` for a save the player uploaded from somewhere else.
        // The AIs' transient plans are rebuilt; the serialized game keeps the
        // authoritative RNG and world state.
        ("POST", "/load") => {
            let loaded: Result<Game, String> = if let Some(name) = parsed["name"].as_str() {
                save_path(name)
                    .ok_or_else(|| "a save name is letters, digits, - and _".to_string())
                    .and_then(|path| {
                        std::fs::read(&path).map_err(|error| format!("cannot read {name}: {error}"))
                    })
                    .and_then(|raw| {
                        serde_json::from_slice(&raw)
                            .map_err(|error| format!("{name} is not a save: {error}"))
                    })
            } else if !parsed["game"].is_null() {
                serde_json::from_value(parsed["game"].clone())
                    .map_err(|error| format!("that is not a save: {error}"))
            } else {
                Err("load needs a save name or a game".to_string())
            };
            let mut out = match loaded {
                Ok(game) => {
                    // A save records the mods it was played under. Loading it
                    // under a different set silently changes the rules
                    // mid-game, so refuse rather than pretend otherwise.
                    let active = crate::mods::active_names();
                    if game.mods != active {
                        let session = sh.session.lock().unwrap();
                        let mut out = session.state();
                        out["error"] = json!(format!(
                            "that save was played with mods {:?}, this server has {:?}",
                            game.mods, active
                        ));
                        drop(session);
                        decorate(&mut out, sh);
                        respond_json(stream, &out);
                        return;
                    }
                    let mut session = sh.session.lock().unwrap();
                    let params = session.params.clone();
                    *session = Session::from_game(params, game);
                    let mut out = session.state();
                    out["error"] = Value::Null;
                    drop(session);
                    out
                }
                Err(error) => {
                    let session = sh.session.lock().unwrap();
                    let mut out = session.state();
                    out["error"] = json!(error);
                    drop(session);
                    out
                }
            };
            decorate(&mut out, sh);
            respond_json(stream, &out);
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
                    "map_topologies": MAP_TOPOLOGIES,
                    "map_poles": MAP_POLES,
                    "game_speeds": CIV6_GAME_SPEEDS,
                    "strategies": strategy_roster(&session),
                    "seat_strategy": session.seated_strategy_name(0),
                }),
            );
        }
        // Hand your seat to one of our agents for a stretch of turns. `turns`
        // is a count or the string "all"; `strategy` names who plays, and is
        // remembered on the seat so a run continued in chunks stays one agent.
        ("POST", "/autoplay") => {
            let mut session = sh.session.lock().unwrap();
            if session.params.spectate {
                drop(session);
                respond_json(stream, &json!({"error": "a spectated game is already playing itself"}));
                return;
            }
            // A stale page must never hand a seat in the successor world to an
            // agent. The identifiers are optional for old clients, but every
            // current browser sends both with each retryable batch.
            if parsed["seed"]
                .as_u64()
                .is_some_and(|seed| seed != session.game.seed)
                || parsed["server_instance"]
                    .as_u64()
                    .is_some_and(|instance| instance != std::process::id() as u64)
            {
                drop(session);
                respond_json(stream, &json!({"error": "the game changed before auto-play began"}));
                return;
            }
            let request_id = parsed["request_id"]
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= 128);
            if let Some((_, played)) = request_id.and_then(|id| {
                session
                    .last_autoplay_request
                    .as_ref()
                    .filter(|(completed, _)| completed == id)
            }) {
                let played = *played;
                let mut out = session.state();
                out["autoplayed"] = json!(played);
                out["autoplay_strategy"] = json!(session.seated_strategy_name(0));
                drop(session);
                decorate(&mut out, sh);
                respond_json(stream, &out);
                return;
            }
            if let Some(name) = parsed["strategy"].as_str() {
                if let Err(error) = session.seat_strategy_at(0, name) {
                    drop(session);
                    respond_json(stream, &json!({"error": error}));
                    return;
                }
            }
            let turns = match parsed["turns"].as_str() {
                Some("all") => u32::MAX,
                _ => parsed["turns"].as_u64().unwrap_or(1).clamp(1, u32::MAX as u64) as u32,
            };
            let played = session.autoplay(turns);
            if let Some(request_id) = request_id {
                session.last_autoplay_request = Some((request_id.to_string(), played));
            }
            let mut out = session.state();
            out["autoplayed"] = json!(played);
            out["autoplay_strategy"] = json!(session.seated_strategy_name(0));
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
            let ending_turn = parsed["action"]["type"].as_str() == Some("end_turn");
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
            let refused = err.is_some();
            out["error"] = match err {
                Some(e) => Value::String(e),
                None => Value::Null,
            };
            // Civ 6 autosaves at the top of every turn, and the reason is the
            // same here: a single-player game that only exists in one
            // process's memory is one crash away from never having happened.
            // Spectated games are the supervisor's business, not this.
            if ending_turn && !refused && !session.params.spectate {
                let turn = session.game.turn;
                let path =
                    std::path::Path::new(SAVE_DIR).join(format!("autosave-t{turn}.save.json"));
                if write_save(&session.game, &path).is_ok() {
                    prune_autosaves();
                    out["autosaved"] = json!(turn);
                }
            }
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
        // "One more turn": carry the decided world on instead of retiring it.
        // The countdown is cleared here rather than left to the stepper, so a
        // state read between the press and the stepper's next pass never
        // reports a restart that is no longer coming.
        ("POST", "/play-on") => {
            let mode_name = parsed["mode"].as_str().unwrap_or("until_next_victory");
            let Some(mode) = PlayOnMode::parse(mode_name) else {
                respond_json(
                    stream,
                    &json!({"error": format!("unknown play-on mode {mode_name:?}")}),
                );
                return;
            };
            let mut session = sh.session.lock().unwrap();
            let played_on = session.play_on(mode);
            let mut out = session.state();
            out["error"] = if played_on {
                Value::Null
            } else {
                json!("this game has no result to play on past")
            };
            drop(session);
            if played_on {
                sh.restart_in.store(u64::MAX, Ordering::Relaxed);
            }
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
        chronicle_world_events, final_countdown_ms, new_game_params, query_value, request_path,
        save_path, seat_delay_ms, strategy_roster, tile_mark, ChronicleSnapshot, ChronicleState,
        FrameDelivery, Params,
        Session, SpectatorFrame, EMBEDDED_CINEMATIC_3D, EMBEDDED_INDEX, MIN_RESTART_MS,
        EMBEDDED_WORLD_WONDER_ATLAS, SAVE_DIR, STATE_LONG_POLL, VIEWER_ACTIVE,
    };
    use crate::game::{Action, Game, VictoryConditions};
    use crate::setup::{GameSpeed, MapPoles, MapScript, MapTopology};
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
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
    fn every_turn_waits_for_its_frame_only_while_a_viewer_is_active() {
        let now = Instant::now();
        let turn_7 = SpectatorFrame { seed: 41, turn: 7 };
        let turn_8 = SpectatorFrame { seed: 41, turn: 8 };
        let next_world = SpectatorFrame { seed: 42, turn: 7 };
        let mut delivery = FrameDelivery::default();

        assert_eq!(delivery.wait_remaining(turn_7, now), None);

        delivery.viewer_request("one", None, now);
        assert_eq!(delivery.wait_remaining(turn_7, now), Some(VIEWER_ACTIVE));

        delivery.frame_delivered("one", turn_7, now + Duration::from_millis(20));
        assert!(
            delivery.wait_remaining(turn_7, now).is_some(),
            "delivery to a socket is not a painted frame"
        );
        delivery.viewer_request(
            "one",
            Some(turn_7),
            now + Duration::from_millis(40),
        );
        assert_eq!(delivery.wait_remaining(turn_7, now), None);
        assert!(delivery.wait_remaining(turn_8, now).is_some());
        assert!(delivery.wait_remaining(next_world, now).is_some());

        assert_eq!(
            delivery.wait_remaining(turn_8, now + Duration::from_millis(40) + VIEWER_ACTIVE),
            None
        );
        assert_eq!(
            delivery.wait_remaining(turn_8, now + VIEWER_ACTIVE + Duration::from_millis(41)),
            None
        );
    }

    /// Two tabs on one exhibition are two promises, not one. The gate used to
    /// keep a single delivery cursor, so either page satisfying it released the
    /// turn and they took alternate ones — each seeing half the game while the
    /// audit, reading that same cursor, called it perfect.
    #[test]
    fn every_viewer_is_owed_the_turn_not_whichever_asks_first() {
        let now = Instant::now();
        let turn_7 = SpectatorFrame { seed: 41, turn: 7 };
        let mut delivery = FrameDelivery::default();

        delivery.viewer_request("one", None, now);
        delivery.viewer_request("two", None, now);

        delivery.frame_delivered("one", turn_7, now);
        assert!(
            delivery.wait_remaining(turn_7, now).is_some(),
            "neither delivered snapshot has been painted yet"
        );
        delivery.frame_delivered("two", turn_7, now);
        assert!(
            delivery.wait_remaining(turn_7, now).is_some(),
            "both sockets have the turn, but neither screen has acknowledged it"
        );
        delivery.viewer_request("one", Some(turn_7), now);
        assert!(
            delivery.wait_remaining(turn_7, now).is_some(),
            "the second tab has not painted this turn yet"
        );
        delivery.viewer_request("two", Some(turn_7), now);
        assert_eq!(delivery.wait_remaining(turn_7, now), None);

        // And a tab that closes stops holding turns open once it goes stale,
        // rather than costing the exhibition a wait for a page nobody has.
        let later = now + VIEWER_ACTIVE + Duration::from_millis(1);
        let turn_8 = SpectatorFrame { seed: 41, turn: 8 };
        delivery.viewer_request("one", None, later);
        delivery.frame_delivered("one", turn_8, later);
        delivery.viewer_request("one", Some(turn_8), later);
        assert_eq!(delivery.wait_remaining(turn_8, later), None);
        assert_eq!(delivery.seats.len(), 1, "the departed tab was retired");
    }

    #[test]
    fn only_the_exact_delivered_snapshot_can_acknowledge_a_complete_frame() {
        let now = Instant::now();
        let turn_7 = SpectatorFrame { seed: 41, turn: 7 };
        let turn_8 = SpectatorFrame { seed: 41, turn: 8 };
        let other_world = SpectatorFrame { seed: 42, turn: 7 };
        let mut delivery = FrameDelivery::default();

        delivery.viewer_request("one", None, now);
        delivery.frame_delivered("one", turn_7, now);

        delivery.viewer_request("one", Some(turn_8), now);
        delivery.viewer_request("one", Some(other_world), now);
        assert_eq!(delivery.seats["one"].painted, None);
        assert!(delivery.wait_remaining(turn_7, now).is_some());

        delivery.viewer_request("one", Some(turn_7), now);
        assert_eq!(delivery.seats["one"].painted, Some(turn_7));
        assert_eq!(delivery.wait_remaining(turn_7, now), None);
    }

    /// Each viewer's misses are its own. One page catching every turn does not
    /// cover for another that is dropping them.
    #[test]
    fn misses_are_counted_against_the_viewer_that_missed_them() {
        let world = |turn| Some(SpectatorFrame { seed: 41, turn });
        let mut now = Instant::now();
        let beat = Duration::from_millis(50);
        let mut delivery = FrameDelivery::default();

        for turn in 7..=10 {
            now += beat;
            let steady = world(turn);
            let skipping = world(7 + (turn - 7) * 3);
            delivery.frame_delivered("steady", steady.unwrap(), now);
            delivery.viewer_request("steady", steady, now);
            delivery.frame_delivered("skipping", skipping.unwrap(), now);
            delivery.viewer_request("skipping", skipping, now);
        }
        let seat = |id: &str| delivery.seats[id].missed;
        assert_eq!(seat("steady"), 0);
        assert_eq!(seat("skipping"), 6); // three turns lost, three times over
    }

    /// A frame written to a socket is not yet a frame anybody saw. The page
    /// reports the turn it painted, so turns that went by undrawn are counted
    /// rather than assumed not to exist.
    #[test]
    fn painting_reports_count_the_turns_no_viewer_ever_drew() {
        let world = |turn| Some(SpectatorFrame { seed: 41, turn });
        let mut now = Instant::now();
        let mut poll = |delivery: &mut FrameDelivery, painted, after: Duration| {
            now += after;
            if let Some(frame) = painted {
                delivery.frame_delivered("tab", frame, now);
            }
            delivery.viewer_request("tab", painted, now);
        };
        let mut delivery = FrameDelivery::default();
        let beat = Duration::from_millis(300);
        let missed = |delivery: &FrameDelivery| delivery.missed;

        poll(&mut delivery, world(7), beat);
        assert_eq!(missed(&delivery), 0); // nothing to compare the first against

        poll(&mut delivery, world(8), beat);
        assert_eq!(missed(&delivery), 0);

        poll(&mut delivery, world(12), beat);
        assert_eq!(missed(&delivery), 3); // 9, 10 and 11 were simulated unseen

        // A viewer that left is owed nothing while it is gone. The exhibition
        // is meant to run flat out unattended, and a tab that closes, reloads
        // onto a swapped binary, or sits through a game boundary comes back to
        // a later turn through no fault of the gate.
        poll(&mut delivery, world(400), VIEWER_ACTIVE + beat);
        assert_eq!(missed(&delivery), 3);
        poll(&mut delivery, world(401), beat);
        assert_eq!(missed(&delivery), 3);

        // A different world starts the count over too: seeds are unordered and
        // the turns before it belonged to another game. Nor is a repeated turn
        // a miss — the page redraws the same turn whenever it polls twice
        // inside one.
        poll(&mut delivery, Some(SpectatorFrame { seed: 42, turn: 40 }), beat);
        poll(&mut delivery, Some(SpectatorFrame { seed: 42, turn: 40 }), beat);
        poll(&mut delivery, Some(SpectatorFrame { seed: 42, turn: 41 }), beat);
        assert_eq!(missed(&delivery), 3);
    }

    /// Only a page that paints holds the simulation to a turn. The keeper's
    /// refresh check reads `/state` as well, and a reader that draws nothing
    /// must not drag the exhibition down to its own polling cadence.
    #[test]
    fn only_a_request_that_reports_painting_is_a_viewer() {
        assert_eq!(query_value("/state", "painted"), None);
        assert_eq!(query_value("/state?instance=9232", "painted"), None);
        // A page that has painted nothing yet is still a viewer.
        assert_eq!(query_value("/state?painted=", "painted"), Some(""));
        assert_eq!(
            query_value("/state?painted=17&world=41", "painted"),
            Some("17")
        );
        assert_eq!(
            query_value("/state?painted=17&world=41", "world"),
            Some("41")
        );
        assert_eq!(query_value("/state?painted=17", "world"), None);
        // A key is a whole key, not a prefix of the next one along.
        assert_eq!(query_value("/state?painted_at=17", "painted"), None);
    }

    fn http(port: u16, request: &str) -> Option<String> {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .ok()?;
        stream.write_all(request.as_bytes()).ok()?;
        let mut response = String::new();
        stream.read_to_string(&mut response).ok()?;
        response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
    }

    fn http_get(port: u16, target: &str) -> Option<String> {
        http(
            port,
            &format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"),
        )
    }

    fn http_post(port: u16, target: &str, body: &str) -> Option<String> {
        http(
            port,
            &format!(
                "POST {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        )
    }

    /// The promise itself, end to end, against a real server over a real
    /// socket. A page that paints slower than the turn budget is the case that
    /// used to lose turns silently — five of twenty-eight on the default pace
    /// when the paint took 1.2s — so the viewer here is deliberately slower
    /// than the pace it asks for. Every turn the server simulated has to have
    /// arrived in some response, and the server has to agree that it did.
    #[test]
    fn a_viewer_slower_than_the_pace_still_sees_every_turn() {
        let port = TcpListener::bind(("127.0.0.1", 0))
            .expect("a free port")
            .local_addr()
            .unwrap()
            .port();
        let mut params = current();
        params.spectate = true;
        params.num_players = 3;
        params.num_city_states = 1;
        params.width = 24;
        params.height = 16;
        params.seed = 20_260_725;
        std::thread::spawn(move || super::serve_with_game(port, false, params, None, false));

        let deadline = Instant::now() + Duration::from_secs(60);
        while http_get(port, "/status").is_none() {
            assert!(Instant::now() < deadline, "spectator server never came up");
            std::thread::sleep(Duration::from_millis(50));
        }
        http_post(port, "/pace", "{\"ms\":120}").expect("set the turn pace");

        // The browser's loop at its worst: one request in flight at a time,
        // and a paint that costs twice what the whole turn was budgeted.
        let mut seen: Vec<u32> = Vec::new();
        let mut painted: Option<(u64, u32)> = None;
        for _ in 0..24 {
            let target = match painted {
                None => "/state?painted=".to_string(),
                Some((seed, turn)) => format!("/state?painted={turn}&world={seed}"),
            };
            let body = http_get(port, &target).expect("a state to draw");
            let state: Value = serde_json::from_str(&body).expect("state is JSON");
            let turn = state["turn"].as_u64().expect("a turn") as u32;
            let seed = state["seed"].as_u64().expect("a world");
            std::thread::sleep(Duration::from_millis(250)); // the paint
            seen.push(turn);
            painted = Some((seed, turn));
        }
        if let Some((seed, turn)) = painted {
            http_get(port, &format!("/state?painted={turn}&world={seed}"));
        }
        http_post(port, "/pace", "{\"paused\":true}"); // stop stepping this game

        let (first, last) = (seen[0], *seen.last().unwrap());
        assert!(
            last >= first + 4,
            "the exhibition never moved, so nothing was tested: {seen:?}"
        );
        let missed: Vec<u32> = (first..=last).filter(|turn| !seen.contains(turn)).collect();
        assert!(
            missed.is_empty(),
            "turns simulated but never sent to the viewer: {missed:?} out of {seen:?}"
        );

        let status: Value = serde_json::from_str(&http_get(port, "/status").expect("status"))
            .expect("status is JSON");
        assert_eq!(status["frames_missed"], json!(0));
        assert_eq!(status["frames_painted"], json!(last));
    }

    /// Martin's requirement is a simulation gate, not merely an audit after
    /// the fact. Once a turn has reached the socket, the game must remain on
    /// that turn until the viewer reports that the complete frame rendered.
    #[test]
    fn simulation_cannot_advance_from_delivery_without_a_paint_acknowledgement() {
        let port = exhibition(20_260_729);
        http_post(port, "/pace", "{\"paused\":true}").expect("pause before attaching");
        std::thread::sleep(Duration::from_millis(300));

        let first: Value = serde_json::from_str(
            &http_get(port, "/state?painted=&viewer=paint-gate").expect("a state to draw"),
        )
        .expect("state is JSON");
        let seed = first["seed"].as_u64().expect("a world");
        let turn = first["turn"].as_u64().expect("a turn") as u32;
        http_post(port, "/pace", "{\"ms\":0,\"paused\":false}").expect("set unlimited pace");

        // The response has been delivered, but the test viewer deliberately
        // has not claimed to paint it. Unlimited pace must still stay put.
        std::thread::sleep(Duration::from_millis(300));
        let waiting: Value =
            serde_json::from_str(&http_get(port, "/status").expect("status")).expect("status JSON");
        assert_eq!(
            waiting["turn"],
            json!(turn),
            "the simulation advanced on socket delivery instead of complete paint"
        );

        // The next browser poll is the acknowledgement. It also asks for the
        // next turn, which must now be exactly one turn later because that new
        // turn is itself gated until another complete-frame acknowledgement.
        let next: Value = serde_json::from_str(
            &http_get(
                port,
                &format!("/state?painted={turn}&world={seed}&viewer=paint-gate&have={seed}:{turn}"),
            )
            .expect("the next state"),
        )
        .expect("state is JSON");
        assert_eq!(next["turn"], json!(turn + 1));
        http_post(port, "/pace", "{\"paused\":true}");
    }

    /// Start a spectator on its own port and wait for it to answer.
    fn exhibition(seed: u64) -> u16 {
        let port = TcpListener::bind(("127.0.0.1", 0))
            .expect("a free port")
            .local_addr()
            .unwrap()
            .port();
        let mut params = current();
        params.spectate = true;
        params.num_players = 3;
        params.num_city_states = 1;
        params.width = 24;
        params.height = 16;
        params.seed = seed;
        std::thread::spawn(move || super::serve_with_game(port, false, params, None, false));
        let deadline = Instant::now() + Duration::from_secs(60);
        while http_get(port, "/status").is_none() {
            assert!(Instant::now() < deadline, "spectator server never came up");
            std::thread::sleep(Duration::from_millis(50));
        }
        port
    }

    /// A result has to arrive with the window it promises, and that window has
    /// to be answerable.
    ///
    /// The countdown used to be armed on the stepper's *next* pass, so for a
    /// beat `/state` carried a winner and no `restart_in` at all and the
    /// result screen opened on "preparing the next world" before it started
    /// counting. That beat is the difference between five seconds to press
    /// "one more turn" and however much of five seconds is left over.
    #[test]
    fn a_result_arrives_with_its_countdown_and_can_be_played_past() {
        let port = TcpListener::bind(("127.0.0.1", 0))
            .expect("a free port")
            .local_addr()
            .unwrap()
            .port();
        let mut params = current();
        params.spectate = true;
        params.num_players = 2;
        params.num_city_states = 1;
        params.width = 24;
        params.height = 16;
        params.seed = 20_260_727;
        // Short enough that the turn limit lands within the test's patience.
        params.max_turns = 3;
        std::thread::spawn(move || super::serve_with_game(port, false, params, None, false));
        let deadline = Instant::now() + Duration::from_secs(60);
        while http_get(port, "/status").is_none() {
            assert!(Instant::now() < deadline, "spectator server never came up");
            std::thread::sleep(Duration::from_millis(50));
        }

        let read = |target: &str| -> Value {
            serde_json::from_str(&http_get(port, target).expect("a state")).expect("state is JSON")
        };
        let deadline = Instant::now() + Duration::from_secs(60);
        let decided = loop {
            let state = read("/state");
            if !state["winner"].is_null() {
                break state;
            }
            assert!(Instant::now() < deadline, "the short game never ended");
            std::thread::sleep(Duration::from_millis(20));
        };
        // Every state that carries a winner carries the countdown with it.
        assert_eq!(
            decided["restart_in"],
            json!(MIN_RESTART_MS / 1_000),
            "a result was published without the five seconds it is owed"
        );

        let played_on: Value =
            serde_json::from_str(&http_post(port, "/play-on", "{}").expect("play on"))
                .expect("play-on answers JSON");
        assert!(played_on["error"].is_null());
        assert!(played_on["winner"].is_null(), "the world is live again");
        // The verdict survives the extension, and the countdown that was
        // running for it does not.
        assert_eq!(played_on["decided"]["turn"], decided["turn"]);
        assert_eq!(
            played_on["decided"]["victory_type"],
            decided["victory_type"]
        );
        assert!(played_on["restart_in"].is_null());
        assert_eq!(
            played_on["max_turns"].as_u64().expect("a raised limit"),
            decided["turn"].as_u64().unwrap() + u64::from(crate::game::PLAY_ON_TURNS)
        );
        assert_eq!(played_on["seed"], decided["seed"], "the same world");

        http_post(port, "/pace", "{\"paused\":true}");
    }

    /// Two tabs on one exhibition are two promises, not one.
    ///
    /// Delivery used to be a single cursor for the whole server, so a turn was
    /// released as soon as *either* page had been handed it and the two of
    /// them took alternate turns — each seeing half the game. The audit read
    /// that same cursor, and between them they had reported an unbroken run of
    /// turns, so it called it perfect. Both of these viewers paint slower than
    /// the pace they ask for, and both are owed all of it.
    #[test]
    fn two_viewers_each_see_every_turn() {
        let port = exhibition(20_260_726);
        http_post(port, "/pace", "{\"ms\":60}").expect("set the turn pace");

        // The two run side by side for the same stretch of wall clock rather
        // than for the same number of polls, because the whole point is that
        // they read at different rates: one an order of magnitude slower than
        // the other, which is what a big map on a loaded machine looks like
        // next to a small one.
        let until = Instant::now() + Duration::from_secs(6);
        let watch = |name: &'static str, paint: u64| {
            std::thread::spawn(move || {
                let mut seen: Vec<u32> = Vec::new();
                let mut painted: Option<(u64, u32)> = None;
                while Instant::now() < until {
                    let target = match painted {
                        None => format!("/state?painted=&viewer={name}"),
                        Some((seed, turn)) => {
                            format!("/state?painted={turn}&world={seed}&viewer={name}")
                        }
                    };
                    let Some(body) = http_get(port, &target) else {
                        continue;
                    };
                    let state: Value = serde_json::from_str(&body).expect("state is JSON");
                    let turn = state["turn"].as_u64().expect("a turn") as u32;
                    let seed = state["seed"].as_u64().expect("a world");
                    std::thread::sleep(Duration::from_millis(paint)); // the paint
                    seen.push(turn);
                    painted = Some((seed, turn));
                }
                seen
            })
        };
        let slow = watch("slow", 400);
        let quick = watch("quick", 40);
        let (slow, quick) = (slow.join().unwrap(), quick.join().unwrap());
        http_post(port, "/pace", "{\"paused\":true}");

        for (name, seen) in [("slow", &slow), ("quick", &quick)] {
            let (first, last) = (seen[0], *seen.last().unwrap());
            assert!(
                last >= first + 3,
                "the exhibition never moved for {name}, so nothing was tested: {seen:?}"
            );
            let missed: Vec<u32> = (first..=last).filter(|turn| !seen.contains(turn)).collect();
            assert!(
                missed.is_empty(),
                "{name} was never sent {missed:?}, out of {seen:?}"
            );
        }
        let status: Value = serde_json::from_str(&http_get(port, "/status").expect("status"))
            .expect("status is JSON");
        assert_eq!(status["frames_missed"], json!(0));
        assert_eq!(status["viewers"], json!(2));
    }

    /// A tile's fingerprint stands in for the tile when deciding whether a
    /// viewer needs to be sent it again, so anything that changes on a tile has
    /// to change the mark. A false match is a hex that stays wrong on somebody's
    /// map until the next resync — silently, and only in the corner nobody is
    /// looking at.
    #[test]
    fn a_tile_that_changed_does_not_keep_its_fingerprint() {
        let tile = json!({
            "pos": [-15, 30], "terrain": "ocean", "hills": false, "road": 0,
            "resource": null, "river_edges": [false, false, false, false, false, false],
            "disaster_yields": {"faith": 0.0, "food": 0.0, "production": 0.0},
        });
        let same = tile_mark(&tile);
        assert_eq!(same, tile_mark(&tile.clone()), "the same tile, twice");

        let mut changed = |mutate: &dyn Fn(&mut Value)| {
            let mut other = tile.clone();
            mutate(&mut other);
            assert_ne!(tile_mark(&other), same, "unnoticed change: {other}");
        };
        changed(&|t| t["terrain"] = json!("grass"));
        changed(&|t| t["hills"] = json!(true));
        changed(&|t| t["road"] = json!(1));
        changed(&|t| t["resource"] = json!("iron"));
        changed(&|t| t["pos"] = json!([-15, 31]));
        changed(&|t| t["river_edges"][2] = json!(true));
        changed(&|t| t["disaster_yields"]["food"] = json!(2.0));
        changed(&|t| t["owner"] = json!(0)); // a field appearing at all
        // The kinds that would otherwise all hash as "empty", and the numbers
        // that would otherwise hash as each other.
        changed(&|t| t["resource"] = json!(false));
        changed(&|t| t["resource"] = json!(0));
        changed(&|t| t["resource"] = json!(""));
        changed(&|t| t["road"] = json!(0.5));
        assert_ne!(tile_mark(&json!(0)), tile_mark(&json!(0.5)));
        assert_ne!(tile_mark(&json!(null)), tile_mark(&json!(false)));
        assert_ne!(tile_mark(&json!([])), tile_mark(&json!({})));
    }

    /// A page that says what it is holding is asking for the turn *after* it.
    ///
    /// It waits on the server rather than on a clock of its own. That is what
    /// lets the page ask again the instant it has finished drawing without
    /// spinning: `/state` answers immediately by nature, so a loop with no
    /// delay in it would rebuild a megabyte of observation over and over for a
    /// turn already on the screen, competing with the simulation for the
    /// machine. Readers that hold nothing — every health check there is — are
    /// still answered at once.
    #[test]
    fn a_page_holding_the_current_turn_waits_for_the_next_one() {
        let port = exhibition(20_260_728);
        http_post(port, "/pace", "{\"paused\":true}").expect("pause the exhibition");
        let read = |target: &str| -> Value {
            serde_json::from_str(&http_get(port, target).expect("a state")).expect("state is JSON")
        };
        let now = read("/state?painted=&viewer=one");
        let seed = now["seed"].as_u64().expect("a world");
        let turn = now["turn"].as_u64().expect("a turn") as u32;
        let holding =
            format!("/state?painted={turn}&world={seed}&viewer=one&have={seed}:{turn}");

        // Nothing is being simulated, so a page holding the current turn is
        // held until the cap and then answered with what it already had.
        let began = Instant::now();
        let same = read(&holding);
        let waited = began.elapsed();
        assert_eq!(same["turn"], json!(turn));
        assert!(
            waited >= STATE_LONG_POLL - Duration::from_millis(50),
            "answered a page that had nothing to be told, after {waited:?}"
        );
        assert!(waited < STATE_LONG_POLL * 4, "held far past the cap: {waited:?}");

        // A reader that names no baseline is never made to wait for one.
        let began = Instant::now();
        read("/state");
        assert!(began.elapsed() < Duration::from_millis(500));

        // And once the game is moving, the wait ends when the turn does rather
        // than when the cap runs out.
        http_post(port, "/pace", "{\"ms\":0,\"paused\":false}").expect("let it run");
        let began = Instant::now();
        let next = read(&holding);
        let woken = began.elapsed();
        http_post(port, "/pace", "{\"paused\":true}");
        assert!(
            next["turn"].as_u64().expect("a turn") as u32 > turn,
            "the wait ended on the same turn it started on"
        );
        assert!(woken < STATE_LONG_POLL, "timed out rather than woken: {woken:?}");
    }

    /// The map is 1.2 MB of a 1.4 MB state and hardly any of it differs from
    /// one turn to the next, so a page that says which array it is holding is
    /// sent only what changed. What it rebuilds from that has to be exactly the
    /// map the server would have sent it whole — the failure this guards
    /// against is not a crash but a world that is quietly a few turns stale in
    /// the corners nobody is looking at.
    #[test]
    fn a_viewer_is_sent_only_the_tiles_that_changed() {
        let port = exhibition(20_260_727);
        // Hold the turn still, so the whole map and the patched one can be
        // compared as of the same moment.
        http_post(port, "/pace", "{\"paused\":true}").expect("pause the exhibition");

        let read = |target: &str| -> Value {
            serde_json::from_str(&http_get(port, target).expect("a state")).expect("state is JSON")
        };
        let first = read("/state?painted=&viewer=one");
        let seed = first["seed"].as_u64().expect("a world");
        let base = first["turn"].as_u64().expect("a turn") as u32;
        let mut held: Vec<Value> = first["map"]["tiles"]
            .as_array()
            .expect("the whole map, the first time")
            .clone();
        assert!(held.len() > 300, "a map of {} tiles", held.len());

        // Play on far enough that the map itself has moved on: capitals get
        // founded, borders claim their tiles, improvements appear.
        for _ in 0..12 {
            http_post(port, "/step", "{\"count\":8}").expect("step the game on");
        }

        let patched = read(&format!(
            "/state?painted={base}&world={seed}&viewer=one&have={seed}:{base}"
        ));
        assert!(
            patched["map"]["tiles"].is_null(),
            "a page that is holding the map must not be sent it again"
        );
        assert_eq!(patched["map"]["tiles_from"], json!(base));
        let changed = patched["map"]["tiles_changed"]
            .as_array()
            .expect("a patch")
            .clone();
        assert!(!changed.is_empty(), "a dozen turns changed nothing at all");
        assert!(
            changed.len() < held.len() / 2,
            "{} of {} tiles is not worth calling a patch",
            changed.len(),
            held.len()
        );
        for entry in &changed {
            let at = entry[0].as_u64().expect("a tile index") as usize;
            held[at] = entry[1].clone();
        }

        // What a reader with no baseline is handed, at the same still turn.
        let whole = read("/state");
        assert_eq!(whole["turn"], patched["turn"], "the game moved mid-test");
        assert_eq!(
            whole["map"]["tiles"].as_array().expect("a whole map"),
            &held,
            "the patched map is not the map"
        );

        // And a page whose baseline the server does not share gets the map
        // back whole rather than a patch it cannot apply.
        let stale = read(&format!(
            "/state?painted={base}&world={seed}&viewer=one&have={seed}:{}",
            base + 9_000
        ));
        assert!(stale["map"]["tiles"].is_array());
        assert!(stale["map"]["tiles_changed"].is_null());
    }

    #[test]
    fn browser_renders_each_delivered_state_as_one_complete_frame() {
        let requirement = include_str!("../docs/SPECTATOR_DEPLOY.md");
        assert!(
            requirement.contains("**Martin-requested simulation requirement:**")
                && requirement.contains("must be shown in at least one complete frame")
                && requirement.contains("HUD, player")
                && requirement.contains("victory tracker, world map, minimap"),
            "the named complete-frame simulation requirement must remain explicit"
        );

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
        let painted = render
            .find("paintedFrame = {seed:st.seed, turn:st.turn};")
            .expect("complete-frame acknowledgement");
        assert!(
            full_frame < painted,
            "the frame cannot be acknowledged before its turn-bound surfaces draw"
        );

        let victory_hud = EMBEDDED_INDEX
            .split_once("function playerHudOverview() {")
            .expect("victory tracker renderer")
            .1
            .split_once("\nfunction spectatorIdentity(player)")
            .expect("end of victory tracker renderer")
            .0;
        assert!(victory_hud.contains("victoryMetric(player, track.id)"));

        // The turn plate is the player HUD's left cell, not the tracker's, so
        // the turn count is rendered by the plate and must not linger in the
        // tracker's markup.
        let turn_plate = EMBEDDED_INDEX
            .split_once("function hudTurnPlate() {")
            .expect("turn plate renderer")
            .1
            .split_once("\nfunction playerHudOverview()")
            .expect("end of turn plate renderer")
            .0;
        assert!(turn_plate.contains("<strong>${state.turn}</strong>"));
        assert!(!victory_hud.contains("<strong>${state.turn}</strong>"));

        let player_hud = EMBEDDED_INDEX
            .split_once("function drawPlayerHud() {")
            .expect("player HUD renderer")
            .1
            .split_once("\n// CSS mode changes")
            .expect("end of player HUD renderer")
            .0;
        assert!(player_hud.contains("const overview = playerHudOverview();"));
        assert!(player_hud.contains("hudTurnPlate()"));
        assert!(player_hud.contains("state.players"));
        assert!(player_hud.contains("playerHudStats(p,"));
        assert!(player_hud.contains("victoryHud.innerHTML = overview;"));
        assert!(player_hud.contains("hud.innerHTML = html;"));
        // A seat somebody is playing is named after the player this game
        // registered for them, and it is preferred over any agent handle: a
        // person is never one of the entrants on the leaderboard.
        assert!(player_hud.contains("p.player_username || p.ai_username || \"AI player\""));
        // And a player with nothing behind them reads unrated rather than
        // wearing the 1500 every unrated player would have.
        assert!(player_hud.contains("(playedGames ? `${p.player_elo} ELO` : \"Unrated\")"));

        // The side panel is the one part of a frame that is allowed to skip a
        // repaint, because below a second per turn it changes faster than
        // anyone can read it. That budget may never swallow a turn's own
        // frame: research, civics and government belonging to the previous
        // turn is exactly the stale corner this promise rules out.
        let side = EMBEDDED_INDEX
            .split_once("function drawSide(force = true) {")
            .expect("side panel renderer")
            .1;
        let throttle = side
            .split_once("return;")
            .expect("side panel repaint budget")
            .0;
        assert!(
            throttle.contains("turn === lastSideTurn"),
            "a new turn must repaint the side panel whatever the clock says"
        );

        // And the page tells the server which turn it painted, both so that
        // only a painting page holds the simulation to a turn and so that
        // turns nobody drew can be counted instead of assumed away.
        assert!(EMBEDDED_INDEX.contains("paintedFrame = {seed:st.seed, turn:st.turn};"));
        assert!(EMBEDDED_INDEX
            .contains("`?painted=${paintedFrame.turn}&world=${paintedFrame.seed}`"));
        assert!(EMBEDDED_INDEX.contains("fetchJSON(\"/state\" + paintedQuery())"));
        // Two tabs are two promises, so a page says which one it is, and what
        // it holds is asked separately from what it drew — a state can arrive,
        // patch the tiles and still fail to paint.
        assert!(EMBEDDED_INDEX.contains("&viewer=${VIEWER_ID}"));
        assert!(EMBEDDED_INDEX.contains("&have=${tileStore.seed}:${tileStore.turn}"));

        // And it draws one turn per animation frame, on the display's clock
        // rather than a timer of its own. Two turns painted inside one refresh
        // are composited into one, so a turn drawn faster than the screen can
        // show it is still a turn nobody saw — and a fixed delay between polls
        // is a ceiling on the whole exhibition, because the simulation is held
        // to whatever rate this loop reads.
        let frame_loop = EMBEDDED_INDEX
            .split_once("(function specFrame() {")
            .expect("the spectator's frame loop")
            .1
            .split_once("\n})();")
            .expect("the end of the frame loop")
            .0;
        assert!(frame_loop.contains("requestAnimationFrame(specFrame)"));
        assert!(
            !frame_loop.contains("setTimeout"),
            "the frame loop keeps no clock of its own"
        );
        assert!(
            frame_loop.contains("render(st);"),
            "every state taken off the queue is drawn"
        );
        let render_done = frame_loop.find("render(st);").unwrap();
        let acknowledge = frame_loop
            .find("specFetch(); // acknowledge this complete frame")
            .expect("the next request acknowledges the completed render");
        assert!(
            render_done < acknowledge,
            "delivery must not be acknowledged before map, HUD, victory tracker, and controls render"
        );
        // Nothing may be dropped: the gate released that turn on the strength
        // of this page drawing it, so a state already in hand has to be
        // painted before another one is asked for.
        let fetching = EMBEDDED_INDEX
            .split_once("function specFetch() {")
            .expect("the spectator's fetch")
            .1;
        assert!(fetching.contains(
            "if (!SPEC || specFetching || specPending || worldTransitionPending()) return;"
        ));
        assert!(fetching.contains("generation === specFetchGeneration"));
    }

    fn current() -> Params {
        Params {
            map_topology: MapTopology::Flat,
            map_poles: MapPoles::Poles,
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

    /// A save name becomes a path, so it is checked rather than trusted.
    /// Everything a browser might send that is not a plain name is refused
    /// before it can reach the filesystem.
    #[test]
    fn a_save_name_cannot_escape_the_save_directory() {
        for good in ["autosave-t12", "my_game", "Rome_1", "a"] {
            let path = save_path(good).expect("{good} is a plain name");
            assert_eq!(path.parent().unwrap(), std::path::Path::new(SAVE_DIR));
            assert_eq!(
                path.file_name().unwrap().to_str().unwrap(),
                format!("{good}.save.json")
            );
        }
        for bad in [
            "",
            "   ",
            "..",
            "../secrets",
            "a/b",
            "a\\b",
            "/etc/passwd",
            "game.save.json",
            "spaced name",
            "n\u{0000}ull",
            &"x".repeat(65),
        ] {
            assert!(save_path(bad).is_none(), "{bad:?} should not be a save name");
        }
    }

    /// A save written and read back is the same game, and the session that
    /// comes out of it can still be played. `Session::from_game` rebuilds the
    /// agents; the serialized game keeps the authoritative RNG.
    #[test]
    fn a_saved_game_reloads_onto_the_same_turn() {
        let mut session = Session::new(current());
        for _ in 0..3 {
            session.act(&json!({"type": "end_turn"}));
        }
        let turn = session.game.turn;
        assert!(turn > 1, "the game should have advanced");

        let round_tripped: Game =
            serde_json::from_value(serde_json::to_value(&session.game).unwrap()).unwrap();
        assert_eq!(round_tripped.turn, turn);
        assert_eq!(round_tripped.seed, session.game.seed);

        let mut restored = Session::from_game(session.params.clone(), round_tripped);
        assert_eq!(restored.game.turn, turn);
        assert!(restored.act(&json!({"type": "end_turn"})).is_none());
        assert!(restored.game.turn > turn, "a loaded game plays on");
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

    /// Planet is drawn from geometry the client cannot derive, so the
    /// protocol has to carry it — but only when asked, because the ordinary
    /// observation is polled every turn and is already large.
    #[test]
    fn a_globe_hands_the_browser_its_shape_only_when_asked() {
        let size = crate::setup::MapSize::for_players(2);
        let (width, height) = size.dimensions(MapTopology::Planet);
        let game = Game::new_with(crate::game::GameOptions {
            map_topology: MapTopology::Planet,
            ..crate::game::GameOptions::new(2, width, height, 6_031, 30, 2)
        });
        let plain = crate::obs::observation_spectator(&game, 0);
        assert!(plain["map"]["planet"].is_null(), "the poll never carries geometry");
        assert_eq!(plain["map"]["shape"], "planet");

        let geometry = crate::obs::planet_geometry(&game).expect("a globe has geometry");
        assert_eq!(geometry["frequency"], size.globe_frequency);
        let cells = geometry["cells"].as_array().unwrap();
        assert_eq!(cells.len(), game.map.tiles.len());
        let corners = geometry["corners"].as_array().unwrap();
        assert_eq!(corners.len() % 3, 0);
        // Each corner is shared by the three tiles meeting there, and is sent
        // once: a frequency-n globe has 20n² of them.
        let frequency = size.globe_frequency as usize;
        assert_eq!(corners.len() / 3, 20 * frequency * frequency);
        let mut pentagons = 0;
        for cell in cells {
            let entry = cell.as_array().unwrap();
            let pos = (entry[0].as_i64().unwrap() as i32, entry[1].as_i64().unwrap() as i32);
            assert!(game.map.tiles.contains_key(&pos));
            match entry.len() - 2 {
                5 => pentagons += 1,
                6 => {}
                other => panic!("{pos:?} was sent {other} corners"),
            }
            for index in &entry[2..] {
                assert!((index.as_i64().unwrap() as usize) < corners.len() / 3);
            }
        }
        assert_eq!(pentagons, 12, "a globe closes with twelve pentagons");

        // A flat map has no geometry to send.
        let flat = Game::new(2, 44, 26, 6_031, 30, 2);
        assert!(crate::obs::planet_geometry(&flat).is_none());
    }

    /// Picking the globe re-expresses the chosen size in the rectangle a globe
    /// is stored in, so the lobby and the world it builds agree.
    #[test]
    fn choosing_the_globe_resizes_the_world_it_builds() {
        let current = current();
        let planet = new_game_params(&current, &json!({"map_topology": "planet"}));
        let size = crate::setup::MapSize::from_dimensions(current.width, current.height)
            .unwrap_or_else(|| crate::setup::MapSize::for_players(current.num_players));
        assert_eq!(planet.map_topology, MapTopology::Planet);
        assert_eq!(
            (planet.width, planet.height),
            (
                crate::sphere::Sphere::width_for(crate::mapgen::globe_frequency(
                    current.width,
                    current.height
                )),
                crate::sphere::Sphere::height_for(crate::mapgen::globe_frequency(
                    current.width,
                    current.height
                ))
            )
        );
        // A stock size keeps its own globe.
        let stock = new_game_params(
            &Params { width: size.width, height: size.height, ..current.clone() },
            &json!({"map_topology": "planet"}),
        );
        assert_eq!((stock.width, stock.height), (size.globe_width(), size.globe_height()));
        assert_eq!(
            crate::setup::MapSize::from_dimensions(stock.width, stock.height).map(|found| found.id),
            Some(size.id),
            "the globe still reports the size it was chosen at"
        );
        // Changing what fills the world does not change its shape: a globe
        // asked for Continents is a globe of continents, and keeps its
        // rectangle.
        let still_round = new_game_params(&stock, &json!({"map_script": "continents"}));
        assert_eq!(still_round.map_topology, MapTopology::Planet);
        assert_eq!(
            (still_round.width, still_round.height),
            (size.globe_width(), size.globe_height())
        );
        // Asking for the flat shape is what flattens it, and back comes the
        // size's own rectangle.
        let flat = new_game_params(&stock, &json!({"map_topology": "flat"}));
        assert_eq!((flat.width, flat.height), (size.width, size.height));
        // Earth is the exception, and overrules the shape it is handed.
        let earth = new_game_params(
            &flat,
            &json!({"map_script": "true_start_earth", "map_topology": "flat"}),
        );
        assert_eq!(earth.map_topology, MapTopology::Planet);
        assert_eq!((earth.width, earth.height), (size.globe_width(), size.globe_height()));
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
        // The globe has its own renderer, and it is the only one: both globe
        // scripts are drawn by it, so neither needs a projection of its own.
        assert!(EMBEDDED_INDEX.contains("function drawPlanetMap()"));
        // A world faces the way it was found until north is discovered, so
        // nothing in the viewer may go back to a bare north-up reset: the
        // camera paths, the compass and the minimap all read one bearing.
        assert!(EMBEDDED_INDEX.contains("function restingRot()"));
        assert!(EMBEDDED_INDEX.contains("function worldFacing(seed)"));
        assert!(EMBEDDED_INDEX.contains("function adoptWorldFacing(st)"));
        assert!(EMBEDDED_INDEX.contains("found_north !== false"));
        // A world's shape and its bearing are earned by going round it: until
        // then the chart is unrolled about one fixed place instead of about the
        // camera, so panning east does not hand back the coasts you started
        // from, and the thumbnail frames the ground that is known rather than
        // the whole rectangle.
        assert!(EMBEDDED_INDEX.contains("function wentAround(st = state)"));
        assert!(EMBEDDED_INDEX.contains("went_around !== false"));
        assert!(EMBEDDED_INDEX.contains("function chartAnchorX()"));
        assert!(EMBEDDED_INDEX.contains("function chartCovers(worldX)"));
        assert!(EMBEDDED_INDEX.contains("function miniBounds()"));
        assert!(EMBEDDED_INDEX.contains("function axisRot()"));
        assert!(!EMBEDDED_INDEX.contains("Math.round((cam.x - x) / WW()) * WW()"));
        // The same rule one step out: a world is drawn as its own people draw
        // it. Until they have proved it round the viewer must keep the chart
        // projection, keep the zoom short of anything that would show them an
        // object, and keep the sky empty — and the world chart in the corner
        // has to obey the same limit, or it hands back what the map withheld.
        assert!(EMBEDDED_INDEX.contains("knows_globe !== false"));
        assert!(EMBEDDED_INDEX.contains("sees_exoplanet !== false"));
        assert!(EMBEDDED_INDEX.contains("function visibleSkyBodies(st = state)"));
        assert!(EMBEDDED_INDEX.contains("chart:!knowsGlobe()"));
        assert!(EMBEDDED_INDEX.contains("function planetChartFloor(centerX, centerY)"));
        assert!(EMBEDDED_INDEX.contains("function planetScaleClamp(scale)"));
        assert!(EMBEDDED_INDEX.contains("function planetMiniScale(width, height)"));
        assert!(EMBEDDED_INDEX.contains("id=\"compass\""));
        assert!(EMBEDDED_INDEX.contains("id=\"compass-needle\""));
        assert!(EMBEDDED_INDEX.contains("resetMapFacing(DEFAULT_CINEMA_YS - cinematicYS)"));
        assert!(!EMBEDDED_INDEX.contains("orbitCamera(-cam.rot, DEFAULT_CINEMA_YS"));
        // The globe's yaw is a bearing, not a second way to spin it eastward.
        assert!(EMBEDDED_INDEX.contains("roll:cam.rot"));
        // A globe is turned, not slid. Longitude and latitude cannot express a
        // drag — near a pole the parallels are a few pixels long, so spending a
        // sideways drag on longitude spins the world about the point under the
        // pointer, and the pole is a wall latitude stops at. So the camera's own
        // basis is rotated bodily and read back into cam.x/cam.y/cam.rot, which
        // makes a pixel of drag the same arc anywhere on the globe and carries
        // the view straight over a pole and down the far side. Every way of
        // moving the map shares that one turn: pointer, touch and the arrows.
        assert!(EMBEDDED_INDEX.contains("function planetViewBasis(camera)"));
        assert!(EMBEDDED_INDEX.contains("function planetBasisCamera(basis)"));
        assert!(EMBEDDED_INDEX.contains("function planetTurnAxis(basis, dx, dy)"));
        assert!(EMBEDDED_INDEX.contains("function applyPlanetBasis(basis)"));
        assert!(EMBEDDED_INDEX.contains("applyPlanetBasis(planetTurn(dragState.basis, dx, dy))"));
        assert!(EMBEDDED_INDEX.contains("applyPlanetBasis(planetTurn(touchGesture.basis, dx, dy))"));
        assert!(EMBEDDED_INDEX.contains("applyPlanetBasis(planetTurn(basis, -screenX, -screenY))"));
        assert!(EMBEDDED_INDEX.contains("spin:planetGlide(released.vpx, released.vpy)"));
        assert!(EMBEDDED_INDEX.contains("<option value=\"planet\">Planet</option>"));
        assert!(EMBEDDED_INDEX
            .contains("<option value=\"true_start_earth\">True Start Earth</option>"));
        // The world's shape and its poles are settings of their own, and the
        // renderer picks its projection from the shape the world reports
        // rather than from the world type it was filled with.
        assert!(EMBEDDED_INDEX.contains("id=\"mapshape\""));
        assert!(EMBEDDED_INDEX.contains("id=\"mappoles\""));
        assert!(EMBEDDED_INDEX.contains("return state.map.shape === \"planet\""));
        assert!(EMBEDDED_INDEX.contains("<option value=\"land_only\">Land Only</option>"));
        assert!(EMBEDDED_INDEX.contains("<option value=\"water_world\">Water World</option>"));
        assert!(EMBEDDED_INDEX.contains("RULES.map_topologies"));
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
        // The modes in the order they are offered: the AI-only simulation this
        // engine exists for, then the human seat, then the one that is still
        // "later". Single player is no longer "later" and is the only mode
        // that offers a leader and a difficulty.
        assert!(
            EMBEDDED_INDEX.contains("<option value=\"ai_sim\" selected>AI-only simulation</option>")
        );
        assert!(EMBEDDED_INDEX.contains("<option value=\"single\">Single player</option>"));
        assert!(!EMBEDDED_INDEX.contains("Single player · later"));
        assert!(EMBEDDED_INDEX.contains("Multiplayer · later"));
        let ai_sim_mode = EMBEDDED_INDEX.find("AI-only simulation").expect("ai sim mode");
        let single_mode = EMBEDDED_INDEX.find(">Single player<").expect("single player mode");
        let multiplayer_mode = EMBEDDED_INDEX.find("Multiplayer · later").expect("multiplayer mode");
        assert!(ai_sim_mode < single_mode && single_mode < multiplayer_mode);
        // A world already on screen sets the mode select, so the panel beside a
        // human game never offers to replace it with a simulation by default.
        assert!(EMBEDDED_INDEX.contains("select.value = SPEC ? \"ai_sim\" : \"single\""));
        assert!(EMBEDDED_INDEX.contains(
            "id=\"restart-sim\" title=\"Restart with the same settings\"><span class=\"lbl\">Restart sim</span><span class=\"sub\">same settings</span>"
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
            "sessionStorage.setItem(\"civvis-restart-paused-v1\", handoff.paused ? \"1\" : \"0\")"
        ));
        assert!(EMBEDDED_INDEX.contains("<html class=\"world-loading\">"));
        assert!(EMBEDDED_INDEX.contains("id=\"world-transition\""));
        assert!(EMBEDDED_INDEX.contains("sessionStorage.setItem(\"civvis-world-transition-v1\""));
        assert!(EMBEDDED_INDEX.contains("await settingsStageChain.catch(() => {})"));
        assert!(EMBEDDED_INDEX.contains("specFetching || specPending || worldTransitionPending()"));
        assert!(EMBEDDED_INDEX.contains("specFetchAbort?.abort()"));
        assert!(EMBEDDED_INDEX.contains("worldTransitionHandoff.supervised"));
        assert!(EMBEDDED_INDEX
            .contains("String(st?.seed) === String(worldTransitionHandoff.targetSeed)"));
        assert!(EMBEDDED_INDEX.contains("finishWorldTransition(st);"));
        assert!(EMBEDDED_INDEX.contains("setTimeout(startFade, 500)"));
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
        let map_setting = EMBEDDED_INDEX.find("id=\"maptype\"").expect("map setting");
        let world_setting = EMBEDDED_INDEX
            .find("id=\"np\"")
            .expect("world size setting");
        let speed_setting = EMBEDDED_INDEX
            .find("id=\"gamespeed\"")
            .expect("game speed setting");
        assert!(
            mode_setting < map_setting
                && map_setting < world_setting
                && world_setting < speed_setting
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
                && display_settings < event_log
                && event_log < war_log
                && war_log < strategy,
            "left panel should show game settings, display settings, and the two logs first"
        );
        assert!(EMBEDDED_INDEX.contains("<span>Display settings</span>"));
        for overlay in ["players", "victory", "minimap", "controls"] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("data-overlay-close=\"{overlay}\"")),
                "map overlay {overlay} should have a close control"
            );
        }
        assert!(
            EMBEDDED_INDEX.contains(
                r#"body.sidebar-hidden .overlay-close[data-overlay-close="controls"] { display: none; }"#
            ),
            "map controls should not offer dismissal while their restore switch is hidden"
        );
        // The switches are a two-column grid, and the order they are written in
        // follows the map: the rail read top to bottom — standings, victory
        // tracker, world minimap — and then the map controls, which are the one
        // instrument still standing in the opposite corner.
        assert!(EMBEDDED_INDEX
            .contains(".overlay-option-grid { display: grid; grid-template-columns: 1fr 1fr;"));
        let switches = EMBEDDED_INDEX
            .split_once("<div class=\"overlay-option-grid\">")
            .expect("map overlay switches")
            .1
            .split_once("</div>")
            .expect("end of map overlay switches")
            .0;
        let corners: Vec<&str> = switches
            .match_indices("data-overlay=\"")
            .map(|(at, marker)| {
                let rest = &switches[at + marker.len()..];
                &rest[..rest.find('"').expect("overlay name is quoted")]
            })
            .collect();
        assert_eq!(
            corners,
            ["players", "victory", "minimap", "controls"],
            "the switches read down the rail, then the map controls in the far corner"
        );
        // One instrument, one name. The switch, the title bar it is dragged by
        // and the label that follows it across the map all say "World minimap",
        // so nothing in the interface reads as a second, separate world map —
        // the world map is the thing filling the screen behind all of them.
        for name in [
            "<span>World minimap</span>",
            "data-overlay=\"minimap\" checked>World minimap",
            "minimap:\"World minimap\"",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(name),
                "the corner map should be named the same everywhere: {name}"
            );
        }
        assert!(
            !EMBEDDED_INDEX.contains("World map"),
            "\"World map\" names the map itself, not the corner instrument showing it"
        );
        // Any row in the standings can be locked so it stays in view while the
        // rest of the table scrolls past it. The choice belongs to the viewer,
        // and it names a *seat* — the civilization plus which of that
        // civilization's seats it is — because an exhibition table routinely
        // seats two Romes, and a name alone locked and unlocked both at once.
        // The name is still the durable half, so a lock carries into the next
        // game after every id has been reassigned.
        assert!(EMBEDDED_INDEX.contains("civvis-hud-locked-seats-v1"));
        assert!(EMBEDDED_INDEX.contains("function toggleSeatLock(id)"));
        assert!(EMBEDDED_INDEX.contains("function seatKeysById(majors)"));
        assert!(EMBEDDED_INDEX.contains("function lockedSeats()"));
        assert!(EMBEDDED_INDEX.contains("function syncPlayerLockPins()"));
        assert!(EMBEDDED_INDEX.contains("data-hud-action=\"lock\""));
        assert!(EMBEDDED_INDEX.contains("if (target.dataset.hudAction === \"lock\") toggleSeatLock(id);"));
        // Nothing but the viewer's own clicking may write the lock set. A
        // default synthesized from whichever civilization was being watched
        // moved the mark from row to row on its own, and the first real click
        // then persisted it as a lock the viewer had never made.
        assert!(
            !EMBEDDED_INDEX.contains("function viewerCivName()"),
            "a lock is the viewer's stored choice, never derived from the watched civilization"
        );
        assert!(EMBEDDED_INDEX.contains("function seedOwnSeatLock(seats)"));
        // A locked row holds at whichever edge it was about to leave, so it
        // needs both offsets, staggered by one row per row held above it.
        assert!(EMBEDDED_INDEX.contains("top: calc(var(--pin-head, 0) * var(--hud-row-pitch));"));
        assert!(EMBEDDED_INDEX.contains("bottom: calc(var(--pin-tail, 0) * var(--hud-row-pitch));"));
        // The standings grow from one consolidated row through eight readable
        // rows. A twelve-player exhibition then scrolls even on a tall screen
        // instead of continuing to consume the world below it.
        assert!(EMBEDDED_INDEX.contains("--player-hud-max-height: min(34vh, 244px);"));
        assert!(EMBEDDED_INDEX.contains("maxHeightRatio:.34"));
        assert!(EMBEDDED_INDEX
            .contains("const requestedWidth = 760 + Math.max(0, rows - 1) * 100;"));
        assert!(EMBEDDED_INDEX.contains(
            "mapArea.style.setProperty(\"--player-hud-width\", `${requestedWidth}px`);"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "const playerScroll = hud.querySelector(\".diplomacy-ribbon\")?.scrollTop || 0;"
        ));
        assert!(EMBEDDED_INDEX.contains("playerRibbon.scrollTop = playerScroll;"));
        // The masthead grows toward the right rail but never beneath it. The
        // tracker grows by the contender rows it shows but never through the
        // minimap seam.
        assert!(EMBEDDED_INDEX.contains("--hud-rail-width:"));
        assert!(EMBEDDED_INDEX.contains(
            "width: min(var(--player-hud-width, 100%),\n      \
             calc(100% - min(var(--hud-rail-width), var(--hud-rail-share)) - 32px));"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "height: min(var(--victory-hud-height, 100%),\n      \
             calc(100% - var(--minimap-height) - 32px));"
        ));
        // Every path keeps its top three plus the player's own civilization
        // when that row is lower in this particular victory race.
        assert!(EMBEDDED_INDEX.contains("const focusId = SPEC ? state.view_player : state.player;"));
        assert!(EMBEDDED_INDEX.contains("focusRank >= DEFAULT_VICTORY_LEADERS ? 1 : 0"));
        assert!(EMBEDDED_INDEX.contains(
            "entry.hidden = index >= capacity && entry.dataset.victoryFocus !== \"true\";"
        ));
        assert!(EMBEDDED_INDEX.contains("data-victory-focus=\"${isFocus}\""));
        assert!(EMBEDDED_INDEX.contains("grid-auto-rows: var(--hud-row-height);"));
        // A masthead row is one line: identity and the ten values side by side,
        // under one set of column heads. Stacking them was what the rail needed
        // and it costs the map 12px of height per civilization here.
        assert!(EMBEDDED_INDEX.contains(
            "grid-template-columns: var(--hud-lock-column, 0px) var(--hud-medallion-column) \
             var(--hud-identity-column) minmax(0, 1fr);"
        ));
        assert!(EMBEDDED_INDEX.contains("--hud-row-height: 23px;"));
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
        // Collapsing the command deck collapses the deck alone. Every map
        // overlay is switched from the deck's display settings instead, so the
        // two controls stay independent and the deck's width can be handed to
        // the map without losing the instruments on it.
        for (overlay, element) in [
            ("players", "#playerhud"),
            ("victory", "#victoryhud"),
            ("minimap", ".minimap-frame"),
            ("controls", "#zoomctl"),
        ] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("body.overlay-{overlay}-hidden {element}")),
                "display settings should hide the {element} map overlay"
            );
        }
        for element in [
            "#playerhud",
            "#victoryhud",
            ".minimap-frame",
            "#zoomctl",
            "#ubar",
            "#modeline",
            "#tip",
        ] {
            assert!(
                !EMBEDDED_INDEX.contains(&format!("body.sidebar-hidden {element}")),
                "collapsing the deck should leave the {element} map overlay alone"
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
        // The atlas view returns to whatever "up" means in this world, which is
        // north only for a civilization that has found it.
        assert!(EMBEDDED_INDEX.contains("takeCameraControl();\n  setRot(restingRot());"));
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
        // Default camera moves compose inside the rectangle the chrome leaves
        // the map, measured rather than guessed: below whichever top
        // instrument hangs lower, down to the real bottom edge, right of both
        // the command deck and the world map's own midline, and left of the
        // victory rail. Every instrument is draggable, so each edge comes off
        // the live boxes.
        assert!(EMBEDDED_INDEX.contains("function mapOverlayVisible(name)"));
        assert!(EMBEDDED_INDEX.contains(
            "document.body.classList.contains(\"sidebar-hidden\")"
        ));
        assert!(EMBEDDED_INDEX.contains("function mapWidgetBox(name, areaRect)"));
        assert!(EMBEDDED_INDEX.contains("function mapFocusBounds()"));
        assert!(EMBEDDED_INDEX.contains("function mapFocusPoint()"));
        // The world map sits in the lower-left corner, so it takes width off the
        // left and gives up only half of it. The victory rail is not a corner
        // widget — it stands the whole right edge — so the band ends where the
        // rail begins, and the standings alone hang over the top.
        assert!(EMBEDDED_INDEX
            .contains("if (minimap) left = Math.max(left, (minimap.left + minimap.right) / 2);"));
        assert!(EMBEDDED_INDEX.contains("if (victory) right = Math.min(right, victory.left);"));
        assert!(EMBEDDED_INDEX.contains("if (players) top = Math.max(top, players.bottom);"));
        assert!(EMBEDDED_INDEX.contains(
            "return {x:(bounds.left + bounds.right) / 2, y:(bounds.top + bounds.bottom) / 2};"
        ));
        // A widget parked over a whole axis must hand that axis back rather
        // than aiming the camera off-screen.
        assert!(EMBEDDED_INDEX.contains("const MIN_MAP_FOCUS_BAND = 120;"));
        assert!(EMBEDDED_INDEX
            .contains("if (right - left < MIN_MAP_FOCUS_BAND) { left = 0; right = width; }"));
        assert!(EMBEDDED_INDEX
            .contains("if (bottom - top < MIN_MAP_FOCUS_BAND) { top = 0; bottom = height; }"));
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
        assert!(EMBEDDED_INDEX.contains(
            "[\"Start mil\", \"Peak mil\", \"Saw action\"]"
        ));
        assert!(EMBEDDED_INDEX.contains(
            "[\"Saw action\", \"Peak mil\", \"Start mil\"]"
        ));
        assert!(EMBEDDED_INDEX.contains("overflow-wrap: break-word"));
        assert!(EMBEDDED_INDEX.contains("height: 4px"));
        assert!(EMBEDDED_INDEX.contains("width: var(--war-effort, 0%)"));
        assert!(EMBEDDED_INDEX.contains(
            ".war-side.aggressor .war-belligerent-bar { margin-left: auto; }"
        ));
        assert!(EMBEDDED_INDEX.contains(
            ".war-side.defender .war-belligerent-bar { margin-right: auto; }"
        ));
        assert!(EMBEDDED_INDEX.contains("const effort = maxSawAction > 0 ? 100 * sawAction / maxSawAction : 0"));
        assert!(!EMBEDDED_INDEX.contains("strength_total"));
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

    /// The viewer's controls are Civilization VI's, read out of the game's own
    /// `InputConfiguration.xml` (`InputActionDefaultGestures`, plus the rows
    /// the two expansions add). Every pair below is one row of that table, so
    /// a binding cannot be quietly moved or dropped without this failing.
    /// `docs/CIV6_KEYBINDINGS.md` carries the same table in prose, including
    /// the Civ 6 actions this build has nothing to bind to.
    #[test]
    fn browser_key_bindings_are_civ6s_own() {
        for (action, key) in [
            // UI
            ("ToggleTechTree", "t"),
            ("ToggleCivicsTree", "c"),
            ("ToggleGovernment", "F7"),
            ("ToggleReligion", "l"),
            ("ToggleGreatPeople", "o"),
            ("ToggleCityStates", "F2"),
            ("ToggleEspionage", "F3"),
            ("ToggleTradeRoutes", "F4"),
            ("ToggleGovernors", "F10"),
            ("OpenCivilopedia", "F9"),
            ("ToggleFSMap", "End"),
            // Units
            ("FoundCity", "b"),
            ("MoveTo", "m"),
            ("Fortify", "f"),
            ("FortifyUntilHeal", "h"),
            ("Attack", "a"),
            ("RangedAttack", "r"),
            ("AutoExplore", "e"),
            ("SkipTurn", " "),
            ("Sleep", "z"),
            ("Alert", "v"),
            // Global
            ("EndTurn", "Enter"),
            ("ToggleGrid", "g"),
            ("ToggleResources", "q"),
            ("ToggleYield", "y"),
            ("Toggle2DView", "+"),
            ("PauseMenu", "Home"),
            ("QuickSave", "F5"),
            ("QuickLoad", "F6"),
            ("OnlinePause", "p"),
            ("PrevUnit", ","),
            ("NextUnit", "."),
            ("PrevCity", "["),
            ("NextCity", "]"),
            ("CapitalCity", "\\\\"),
            // Camera
            ("CameraPanLeft", "ArrowLeft"),
            ("CameraPanRight", "ArrowRight"),
            ("CameraPanUp", "ArrowUp"),
            ("CameraPanDown", "ArrowDown"),
            ("ZoomIn", "NumpadAdd"),
            ("ZoomOut", "NumpadSubtract"),
        ] {
            let row = format!("{{id: \"{action}\", key: \"{key}\"");
            assert!(
                EMBEDDED_INDEX.contains(&row),
                "Civ 6's {action} must stay on {key}: no `{row}` in the viewer"
            );
        }
        // The pedia's history is Civ 6's only chorded pair.
        assert!(EMBEDDED_INDEX
            .contains("{id: \"CivilopediaBack\", key: \",\", ctrl: true"));
        assert!(EMBEDDED_INDEX
            .contains("{id: \"CivilopediaForward\", key: \".\", ctrl: true"));

        // Three keys are the operator's deliberate overrides and keep their
        // CIVVIS meaning; Civ 6 spends them on lenses that do not exist here.
        for (action, key) in [("NextAction", "1"), ("SettlerLens", "2"), ("PlaceTack", "3")] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("{{id: \"{action}\", key: \"{key}\"")),
                "the {key} override must stay {action}"
            );
        }
        // Everything CIVVIS adds sits on a chord or a key Civ 6 leaves free,
        // so arriving from Civ 6 cannot find a shadowed binding.
        for chord in [
            "{id: \"AutoPlay\", key: \"a\", ctrl: true",
            "{id: \"ResetFacing\", key: \"r\", ctrl: true",
            "{id: \"HidePanel\", key: \"u\", ctrl: true",
            "{id: \"QuickDeals\", key: \"d\", ctrl: true",
        ] {
            assert!(EMBEDDED_INDEX.contains(chord), "missing CIVVIS chord: {chord}");
        }
        assert!(EMBEDDED_INDEX.contains("{id: \"Diplomacy\", key: \"F8\""));

        // Movement: Civ 6 pans with a left drag, moves with the right button,
        // centres with the middle one, and walks the camera at the map's edge.
        assert!(EMBEDDED_INDEX.contains("function updateEdgePan(clientX, clientY)"));
        assert!(EMBEDDED_INDEX.contains("id=\"edgepanchk\""));
        assert!(EMBEDDED_INDEX.contains("else if (ev.button === 1) {"));
        // Command belongs to the browser on a Mac; only Control chords are ours.
        assert!(EMBEDDED_INDEX.contains("if (ev.metaKey) return undefined;"));
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
            // The default is `null`, not `cam.x`: a chart that has not been
            // round its world is unrolled about that civilization's own ground
            // rather than about the camera, and only a caller chaining a path
            // together names the point it wants a leg drawn beside.
            "function unitMapPoint(p, nearX = null)",
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
                "shape": "flat",
                "poles": "poles",
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
                "shape": "flat",
                "poles": "poles",
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

    /// The supervisor replaces the AI exhibition process by process, so this
    /// server may not swap one simulation for another in place. Sitting down
    /// to play is the exception the rule exists around: a single-player game
    /// is not part of that cycle, so choosing it in the setup panel and
    /// starting it takes this process over at once, and the way back to the
    /// exhibition is the supervised request it has always been.
    #[test]
    fn a_supervised_exhibition_hands_its_process_to_a_single_player_game() {
        let mut params = current();
        params.spectate = true;
        params.supervised = true;
        let mut session = Session::new(params);
        let watched = session.game.seed;

        assert!(session
            .start_new_game(&json!({"seed": 7, "spectate": true, "force": true}))
            .is_err());
        assert_eq!(session.game.seed, watched);

        session
            .start_new_game(&json!({"seed": 8, "spectate": false, "force": true}))
            .unwrap();
        assert_eq!(session.game.seed, 8);
        let state = session.state();
        assert_eq!(state["spectate"], json!(false));
        assert_eq!(state["supervised"], json!(true));
        assert!(!state["legal_actions"].as_array().unwrap().is_empty());

        assert!(session
            .request_supervised_new_game(&json!({"mode": "restart", "paused": false}))
            .is_ok());
    }

    /// "One more turn" on a game somebody is playing. The victory that ended
    /// it can be declared on any seat's turn, so the round is usually parked
    /// on an agent when the result appears; coming back live there would hand
    /// the person a board that refuses every action they take.
    #[test]
    fn playing_on_returns_the_round_to_the_person_at_the_keyboard() {
        let mut params = current();
        params.max_turns = 40;
        let mut session = Session::new(params);
        session.game.turn = 12;
        session.game.current = 1;
        session.game.winner = Some(1);
        session.game.victory_type = Some("science".to_string());

        assert!(session.play_on());
        assert_eq!(session.game.current, 0);
        assert!(session.game.winner.is_none());
        assert_eq!(session.game.max_turns, 12 + crate::game::PLAY_ON_TURNS);
        let state = session.state();
        assert!(state["winner"].is_null());
        assert_eq!(state["decided"]["victory_type"], json!("science"));
        assert_eq!(state["decided"]["turn"], json!(12));
        // A live game has nothing to play on past, and says so rather than
        // quietly granting turns nobody won.
        assert!(!session.play_on());
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

    /// Choosing single player and pressing the one start control on screen
    /// must open that game — on the supervised exhibition too, where every
    /// simulation is a fresh process but a human game takes this one over.
    /// Which control that is follows the world on screen, never the pending
    /// selection: keying the sidebar button to the mode select left a player
    /// who picked AI-only with no way to launch anything at all.
    #[test]
    fn browser_enters_single_player_from_whichever_start_control_is_showing() {
        assert!(EMBEDDED_INDEX
            .contains("const supervised = !!(state && state.supervised) && payload.spectate;"));
        assert!(EMBEDDED_INDEX.contains("const human = !selectedSimulationSettings().spectate;"));
        // Choosing single player renames that control after the game it opens,
        // rather than leaving "Restart sim" over a single-player subtitle.
        assert!(EMBEDDED_INDEX.contains("<span class=\"lbl\">Restart sim</span>"));
        assert!(EMBEDDED_INDEX.contains("button.classList.toggle(\"human-start\", human);"));
        assert!(EMBEDDED_INDEX.contains("? \"Start Single Player Game\""));
        assert!(EMBEDDED_INDEX
            .contains(".spec-controls #restart-sim.human-start::before { content: \"▶\";"));
        // It shares the row with Pause/Resume rather than displacing it: keep
        // watching or leave for your own game is one decision, so the two read
        // as a pair. The row goes uneven instead — Pause keeps just enough for
        // its own label and the start takes the rest — and the start stays the
        // only gold button on it, since two would leave neither reading as the
        // one to press.
        assert!(EMBEDDED_INDEX.contains(
            ".spec-controls:has(#restart-sim.human-start) { grid-template-columns: 96px minmax(0, 1fr); }"
        ));
        assert!(EMBEDDED_INDEX
            .contains(".spec-controls:has(#restart-sim.human-start) #specpause.primary {"));
        assert!(EMBEDDED_INDEX.contains("body.watching-sim #startgame { display: none; }"));
        assert!(EMBEDDED_INDEX.contains("document.body.classList.toggle(\"watching-sim\", SPEC);"));
        // The start button belongs to the game being played, not to the mode
        // the sidebar is staging for the next one.
        assert!(!EMBEDDED_INDEX.contains("human-setting\" id=\"startgame\""));
        // Leader and difficulty still do follow the selection.
        assert!(EMBEDDED_INDEX.contains("body.spectating .human-setting { display: none; }"));
        assert!(EMBEDDED_INDEX.contains("class=\"small human-setting\">Leader"));
        assert!(EMBEDDED_INDEX.contains("class=\"small human-setting\">Difficulty"));
        // Settings staged for the next simulation describe a spectated world,
        // so they may only adopt that mode while one is on screen.
        assert!(EMBEDDED_INDEX
            .contains("if (SPEC) document.getElementById(\"gamemode\").value = \"ai_sim\";"));
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

    /// A treasury that can buy a Warrior and nothing else is not a treasury.
    /// `buy_building` and `buy_district` were legal for seat 0 and had no
    /// control anywhere, and a district's tile — which is most of what a
    /// district is worth — could only be picked out of a flat dropdown. The
    /// city screen is where all of that lives.
    #[test]
    fn browser_has_a_city_screen_that_can_spend() {
        for piece in [
            "id=\"cityscreen\"",
            "function drawCityScreen()",
            "function openCityScreen(id)",
            "function sendFromCity(action)",
            "function itemNote(item)",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(piece),
                "the city screen is missing {piece}"
            );
        }
        // Both purchases the client never offered, and production itself.
        for action in [
            "\"buy\"",
            "\"buy_building\"",
            "\"buy_district\"",
            "\"buy_plot\"",
            "\"produce\"",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("a.type === {action}")),
                "the city screen does not offer {action}"
            );
        }
        // A district with more than one candidate tile names the tiles.
        assert!(EMBEDDED_INDEX.contains("entry.sites.length > 1"));
        // Actions are posted back exactly as the engine handed them over.
        assert!(EMBEDDED_INDEX
            .contains("onclick='sendFromCity(${JSON.stringify(action)})'>${label}</button>"));
        // An idle city is a turn blocker; it must open the screen that
        // answers it rather than merely scrolling a sidebar.
        assert!(EMBEDDED_INDEX.contains("openCityScreen(city.id);"));
    }

    /// Clicking a distant tile is Civ 6's "go there", and it cannot be built
    /// on `move_to`: `path_to` seeds its search with the unit's remaining
    /// movement, so anything further is `"unreachable"`. `/route` exposes the
    /// long-range router the AI already uses, one step at a time, and the
    /// client still sends a normal Move for that step — so the engine stays
    /// the authority on whether the move is legal now.
    #[test]
    fn route_offers_one_step_of_a_journey_the_current_turn_cannot_finish() {
        let session = Session::new(current());
        let unit = *session
            .game
            .units
            .iter()
            .find(|(_, held)| held.owner == 0)
            .expect("seat 0 starts with a unit")
            .0;
        let start = session.game.units[&unit].pos;

        // A destination beyond this turn's movement: `path_to` refuses it,
        // which is exactly the case the client could not express before.
        let far = session
            .game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| session.game.path_to(unit, *pos).is_none())
            .max_by_key(|pos| session.game.wdist(start, *pos))
            .expect("a map has somewhere out of reach");
        assert!(session.game.path_to(unit, far).is_none());

        match session.game.route_step(unit, far, 0) {
            Some(step) => {
                assert_ne!(step, start, "a route step must leave where it started");
                assert_eq!(
                    session.game.wdist(start, step),
                    1,
                    "a route step is one tile, validated by the caller's Move"
                );
            }
            // An island start can legitimately have no land route; the client
            // treats that the same way — the order ends rather than retrying.
            None => {}
        }

        // A refused step must not end the journey: a unit with one movement
        // point cannot enter a two-cost forest, and next turn it can. The
        // first draft dropped the order on the first refusal and stranded
        // units one tile short of where they were sent.
        assert!(EMBEDDED_INDEX.contains("const TRAVEL_PATIENCE = 3;"));
        assert!(EMBEDDED_INDEX.contains("break; // too little movement for that step"));

        // The browser has the order and re-issues it each turn.
        for piece in [
            "async function resumeTravel(unitId)",
            "async function resumeAllTravel()",
            "async function orderTravel(unitId, to)",
            "fetchJSON(\"/route\"",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(piece),
                "the browser cannot travel: missing {piece}"
            );
        }
    }

    /// A finished game has no next turn. Leaving End Turn live meant `Enter`
    /// posted an `end_turn` the engine refused, and the player got a red
    /// error toast for pressing the only lit control on the screen. The
    /// finale offers the one thing still useful instead: another game.
    #[test]
    fn browser_stops_asking_for_turns_once_somebody_has_won() {
        // The winner test's `over` became `won`, because elimination now
        // disables the button on the same path — same contract, wider reason.
        // Auto-play is the third reason: the seat is on loan while it runs.
        assert!(EMBEDDED_INDEX
            .contains("const won = state.winner !== null && state.winner !== undefined;"));
        assert!(EMBEDDED_INDEX.contains("button.disabled = won || eliminated || autoplaying;"));
        assert!(EMBEDDED_INDEX.contains("The game is over<span class=\"endturn-hint\">"));
        // The keys agree with the button.
        assert!(EMBEDDED_INDEX
            .contains("if (state.winner !== null && state.winner !== undefined) return;"));
        // And a human finale offers a way on; a spectated one keeps its
        // countdown, because the supervisor owns that handoff.
        assert!(EMBEDDED_INDEX.contains("class=\"primary winner-again\" onclick=\"startNewSimulation()\""));
        assert!(EMBEDDED_INDEX.contains("id=\"respawn\" role=\"timer\""));
        // Both finales also offer the other answer: keep this world. It is
        // the reason the countdown has to be long enough to read — a button
        // nobody can reach before the next world loads is not an offer.
        assert!(EMBEDDED_INDEX.contains("id=\"play-on\" onclick=\"playOnPastVictory()\""));
        assert!(EMBEDDED_INDEX.contains("async function playOnPastVictory()"));
        assert!(EMBEDDED_INDEX.contains("cancelSupervisedSuccessorWatch();"));
    }

    /// Auto-play used to be one button that ran whichever agent the fleet
    /// happened to build for the seat, for one turn or ten. Both of those are
    /// decisions a person should make: *which* of our strategies plays, and
    /// for how long.
    #[test]
    fn a_player_can_hand_their_seat_to_a_named_strategy() {
        let mut session = Session::new(current());
        // The roster is the one every build ships, so the choice exists in a
        // game that is rating nothing.
        let roster = strategy_roster(&session);
        let names: Vec<&str> = roster
            .as_array()
            .expect("a roster")
            .iter()
            .filter_map(|entry| entry["name"].as_str())
            .collect();
        assert!(names.contains(&"advanced"), "the default agent is offerable");
        assert!(
            names.len() >= 4,
            "a roster with nothing in it is not a choice: {names:?}"
        );
        // Ratings are shown as ratings, and an entrant that has never played a
        // rated game is marked rather than shown as an authoritative 1500.
        for entry in roster.as_array().expect("a roster") {
            assert!(entry["username"].as_str().is_some_and(|name| !name.is_empty()));
            assert!(entry["provisional"].is_boolean());
        }

        // Nobody is offered a person's seat: the roster on offer is agents.
        assert!(
            !names.contains(&"player"),
            "a seat cannot be handed to somebody who is not at a keyboard: {names:?}"
        );

        // The seat starts as the person's own. Nothing is seated until
        // somebody asks, and then it stays seated.
        assert_eq!(session.seated_strategy_name(0), Some("player"));
        session
            .seat_strategy_at(0, "basic")
            .expect("a built-in agent is always available");
        assert_eq!(session.seated_strategy_name(0), Some("basic"));
        assert_eq!(
            session.seat_strategy_at(0, "no-such-strategy"),
            Err("no strategy named no-such-strategy".to_string()),
            "a player who picked a strategy must not silently get another one"
        );
        assert_eq!(session.seated_strategy_name(0), Some("basic"));

        // And it plays: turns pass, and the seat is still the player's after.
        let before = session.game.turn;
        assert_eq!(session.autoplay(3), 3);
        assert_eq!(session.game.turn, before + 3);
    }

    /// Sitting down to play registers a *new* player.
    ///
    /// The seat used to be dealt an entrant off the league table like any
    /// other major, so a person wore an agent's handle and rating, and the
    /// game they finished was filed as that agent's win. Both halves are the
    /// same mistake: an identity nobody at the keyboard earned.
    #[test]
    fn a_single_player_game_registers_a_new_player() {
        let dir = std::env::temp_dir().join(format!(
            "civvis-server-register-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let dir = dir.to_str().unwrap().to_string();
        let _ = std::fs::remove_dir_all(&dir);
        let entrant = |ai: &str| {
            crate::league::Strategy::new(
                ai,
                crate::league::StrategyKind::Builtin { ai: ai.to_string() },
                0,
            )
        };
        crate::league::save_league(
            &dir,
            &crate::league::League {
                round: 2,
                strategies: vec![entrant("advanced"), entrant("basic")],
                calibration: Default::default(),
            },
        );

        let mut params = current();
        params.league_dir = Some(dir.clone());
        params.league_record = true;
        let mut session = Session::new(params);

        // Seat 0 is the person: a row of their own, provisional, and not one
        // of the two agents that were already here.
        let seated = session.seat_strategy[0].expect("the person is rated");
        let league = session.league.clone().expect("a rated roster");
        assert!(league.strategies[seated].human);
        assert_eq!(league.strategies[seated].username, "Player");
        assert_eq!(session.seated_strategy_name(0), Some("player"));
        // The rival is still seated from the roster, and is still an agent.
        let rival = session.seat_strategy[1].expect("the rival is rated");
        assert!(!league.strategies[rival].human);

        // The registration reached the roster on disk, so the result has a
        // name to be filed under.
        let saved = crate::league::load_league(&dir).expect("roster on disk");
        assert_eq!(saved.strategies.len(), 3);
        assert_eq!(saved.humans().len(), 1);

        // And the game says who is playing it.
        let state = session.state();
        let me = &state["players"][0];
        assert_eq!(me["player_username"], json!("Player"));
        assert_eq!(me["player_rated"], json!(true));
        assert_eq!(me["player_games"], json!(0));
        assert!(
            state["players"][1]["player_username"].is_null(),
            "only a seat somebody is playing carries a person"
        );

        // A decided game rates the person, and rates nobody in their place.
        session.game.winner = Some(0);
        session.game.victory_type = Some("score".to_string());
        session.record_league_result();
        let rated = crate::league::load_league(&dir).expect("roster on disk");
        let person = rated.strategies.iter().find(|s| s.human).expect("the person");
        assert_eq!((person.games, person.wins), (1, 1));
        assert!(person.rating > 1500.0);
        for agent in rated.strategies.iter().filter(|s| !s.human) {
            assert_eq!(agent.wins, 0, "{} was credited a person's win", agent.name);
        }
        assert_eq!(
            rated.strategies.iter().filter(|s| s.games > 0).count(),
            2,
            "the person and the rival they beat, nobody else"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Most games are rated against nothing at all. The person is still not
    /// an existing player: they get a handle for this game, and it goes no
    /// further than this game.
    #[test]
    fn an_unrated_single_player_game_still_names_the_person() {
        let session = Session::new(current());
        assert_eq!(session.seated_strategy_name(0), Some("player"));
        assert_eq!(session.seat_strategy[0], None, "there is nothing to rate into");
        let state = session.state();
        assert_eq!(state["players"][0]["player_username"], json!("Player"));
        assert_eq!(state["players"][0]["player_rated"], json!(false));
        assert!(state["players"][0]["player_elo"].is_null());

        // A spectated world has nobody at a keyboard and registers nobody.
        let mut params = current();
        params.spectate = true;
        let spectated = Session::new(params);
        assert!(spectated.human_players.is_empty());
        assert!(spectated.state()["players"][0]["player_username"].is_null());
    }

    /// "All" is a turn count like any other, bounded by the turns this game
    /// has left rather than by a fixed 500 — a marathon game is 1500 turns
    /// long, and a request for the rest of it must not stop two thirds of the
    /// way through.
    #[test]
    fn autoplay_of_everything_is_bounded_by_the_turns_that_remain() {
        let mut params = current();
        params.max_turns = 12;
        let mut session = Session::new(params);
        let played = session.autoplay(u32::MAX);
        assert!(played <= 13, "played {played} turns of a 12-turn game");
        assert!(played >= 12, "only played {played} turns of a 12-turn game");
    }

    /// A browser can lose the response after the agent has already played the
    /// turns. Retrying that POST must acknowledge the completed batch rather
    /// than silently playing it twice.
    #[test]
    fn an_autoplay_batch_is_idempotent_across_a_dropped_response() {
        let port = TcpListener::bind(("127.0.0.1", 0))
            .expect("a free port")
            .local_addr()
            .unwrap()
            .port();
        let mut params = current();
        params.num_players = 3;
        params.num_city_states = 0;
        params.width = 24;
        params.height = 16;
        params.seed = 20_260_726;
        std::thread::spawn(move || super::serve_with_game(port, false, params, None, false));

        let deadline = Instant::now() + Duration::from_secs(60);
        while http_get(port, "/status").is_none() {
            assert!(Instant::now() < deadline, "single-player server never came up");
            std::thread::sleep(Duration::from_millis(50));
        }
        let stale = json!({
            "turns": 3,
            "strategy": "basic",
            "request_id": "viewer-1-stale",
            "seed": 20_260_726,
            "server_instance": u64::from(std::process::id()) + 1,
        })
        .to_string();
        let refused: Value =
            serde_json::from_str(&http_post(port, "/autoplay", &stale).expect("stale response"))
                .expect("stale response is JSON");
        assert_eq!(
            refused["error"],
            json!("the game changed before auto-play began")
        );

        let body = json!({
            "turns": 3,
            "strategy": "basic",
            "request_id": "viewer-1-autoplay-1",
            "seed": 20_260_726,
            "server_instance": std::process::id(),
        })
        .to_string();
        let first: Value =
            serde_json::from_str(&http_post(port, "/autoplay", &body).expect("first response"))
                .expect("first response is JSON");
        let retry: Value =
            serde_json::from_str(&http_post(port, "/autoplay", &body).expect("retry response"))
                .expect("retry response is JSON");

        assert_eq!(first["autoplayed"], json!(3));
        assert_eq!(retry["autoplayed"], json!(3));
        assert_eq!(
            retry["turn"], first["turn"],
            "the retry played the completed batch a second time"
        );
    }

    /// The control that drives the two decisions above. The turn counts are
    /// the ones offered, and the loop that runs them has to be interruptible:
    /// a full game is over a minute of engine work, and a person watching it
    /// wants to be able to stop.
    #[test]
    fn browser_offers_a_strategy_and_a_turn_count_to_auto_play() {
        for piece in [
            "id=\"autoplaystrategy\"",
            "id=\"autoplayturns\"",
            "id=\"autoplaybtn\"",
            "function fillStrategies(rules)",
            "function autoplayRequest()",
            "async function autoplay(turns)",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(piece),
                "the auto-play control is missing {piece}"
            );
        }
        for turns in [
            "1", "2", "3", "4", "5", "10", "20", "30", "40", "50", "100", "150", "200", "250",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(&format!("<option value=\"{turns}\"")),
                "the auto-play turn counts are missing {turns}"
            );
        }
        assert!(EMBEDDED_INDEX.contains("<option value=\"all\">All</option>"));
        // The picker is filled from the server's roster, never a hardcoded list.
        assert!(EMBEDDED_INDEX
            .contains("const roster = Array.isArray(rules.strategies) ? rules.strategies : []"));
        assert!(EMBEDDED_INDEX.contains("fillStrategies(RULES)"));
        // The choice rides on every request, so a run continued in batches
        // cannot change agent halfway through.
        assert!(EMBEDDED_INDEX.contains("async function autoplayBatch(turns, strategy)"));
        assert!(EMBEDDED_INDEX.contains("request_id: requestId"));
        assert!(EMBEDDED_INDEX.contains("AUTOPLAY_BATCH_TIMEOUT_MS = 120000"));
        assert!(EMBEDDED_INDEX.contains("const next = await autoplayBatch(ask, strategy)"));
        // Pressing it again stops, rather than queueing a second run.
        assert!(EMBEDDED_INDEX.contains("if (autoplaying) { autoplayStop = true; return; }"));
        assert!(EMBEDDED_INDEX.contains("while (left > 0 && !autoplayStop)"));
        // Short of the turns asked for means the game ended under it.
        assert!(EMBEDDED_INDEX.contains("if (played < ask) break;"));
    }

    /// Which named Great Person a kind is offering is a world fact — it
    /// depends on who every civilization has retired — so the client cannot
    /// derive it and used to say "a Great Merchant" where Civ 6 says "Marco
    /// Polo, 60 Faith". And enough points is not enough on its own: a Great
    /// Scientist wants a Campus, a Great Writer wants an open Great Work
    /// slot. A card with the points and no Recruit button reads as broken
    /// unless it says which.
    #[test]
    fn browser_names_the_great_person_the_points_are_buying() {
        let mut session = Session::new(current());
        for _ in 0..2 {
            session.act(&json!({"type": "end_turn"}));
        }
        let state = session.state();
        let offers = &state["me"]["great_person_offers"];
        assert!(offers.is_object(), "the observation must carry the offers");
        for (kind, offer) in offers.as_object().unwrap() {
            assert!(offer["name"].is_string(), "{kind} offer has no name");
            assert!(offer["points"].is_number(), "{kind} offer has no threshold");
            assert!(
                offer["blocked"].is_string() || offer["blocked"].is_null(),
                "{kind} blocker must be a reason or nothing"
            );
        }
        // And the screen shows all three.
        assert!(EMBEDDED_INDEX.contains("const offered = me.great_person_offers || {};"));
        assert!(EMBEDDED_INDEX.contains("offer ? offer.name : `Great ${titleCase(kind)}`"));
        assert!(EMBEDDED_INDEX.contains("offer && offer.blocked"));
    }

    /// Past three or four cities, clicking each one on the map to find out
    /// whether it is building anything stops being navigation and becomes a
    /// chore — which is why Civ 6 has a report for it. The Cities screen is
    /// that report: one row per city, the ones waiting on an order first,
    /// because that is the only reason to open it in a hurry.
    #[test]
    fn browser_lists_the_whole_empire() {
        assert!(EMBEDDED_INDEX.contains("function empireCities()"));
        assert!(EMBEDDED_INDEX.contains("{id: \"cities\", icon: \"⌂\", name: \"Cities\"},"));
        assert!(EMBEDDED_INDEX.contains("cities: empireCities,"));
        // It opens on Cities: a wide empire wants the list before the panels.
        assert!(EMBEDDED_INDEX.contains("let empireTab = \"cities\";"));
        // Idle cities sort first and badge the tab, so the screen says how
        // much is waiting without being opened.
        assert!(EMBEDDED_INDEX.contains("Number(idle(second)) - Number(idle(first))"));
        assert!(EMBEDDED_INDEX.contains("case \"cities\":"));
        // Each row goes somewhere: the city screen, or the city itself.
        assert!(EMBEDDED_INDEX.contains("closeEmpire();openCityScreen("));
        assert!(EMBEDDED_INDEX.contains("closeEmpire();centerOn("));
    }

    /// Losing your last city ends the game for the person at the keyboard
    /// even though the world plays on, and the engine answers their
    /// `end_turn` with "not your turn". Before this, an eliminated player
    /// kept a live End Turn button on a map they could not touch, and Enter
    /// earned them a red error toast. Same shape as the winner case, found
    /// by losing an Emperor game rather than winning one.
    #[test]
    fn browser_tells_the_player_when_they_have_been_eliminated() {
        assert!(EMBEDDED_INDEX
            .contains("const eliminated = state.players[0] && state.players[0].alive === false;"));
        assert!(EMBEDDED_INDEX.contains("button.disabled = won || eliminated || autoplaying;"));
        assert!(EMBEDDED_INDEX.contains("Your civilization has fallen<span class=\"endturn-hint\">"));
        // The keys agree with the button.
        assert!(EMBEDDED_INDEX
            .contains("if (state.players[0] && state.players[0].alive === false) return;"));
        // A defeat draws the finale card, and the victory path must not wipe
        // a card it did not draw.
        assert!(EMBEDDED_INDEX.contains("st.players[0].alive === false;"));
        assert!(EMBEDDED_INDEX.contains("} else if (!fallen) {"));
        // A spectated world has nobody to eliminate.
        assert!(EMBEDDED_INDEX.contains("const fallen = !SPEC && !hasWinner"));
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

    /// Hovering a tile reports it, the way Civ 6's plot tooltip does — and it
    /// keeps doing so after the map has been panned.
    ///
    /// `dragMoved` outlives its gesture: the click that follows clears it, and
    /// a drag released off the canvas never produces one. A hover guard that
    /// reads it therefore goes permanently quiet after the first pan, which is
    /// exactly how the tooltip died. The guard has to ask whether a gesture is
    /// in flight *now*.
    #[test]
    fn hovering_a_tile_reports_it_and_survives_a_pan() {
        assert!(
            EMBEDDED_INDEX.contains("if (!state || dragState || mapTouches.size || rdrag) {"),
            "the hover guard must test a live gesture, never the stale dragMoved flag"
        );
        for piece in [
            "function tileMoveCost(t)",
            "function tileDefense(t)",
            "function tileGroundLevel(t)",
            "function tileCoverLevel(t)",
            "function sightTipLine(t)",
            "function appealBand(appeal)",
            // The four lines the panel gains: cost to cross, worth defending,
            // how it looks, and what it lets a unit see past.
            "🥾 \" + (mp % 1 ? mp.toFixed(1) : mp) + \" MP\"",
            "\" defense\"",
            "\"🌸 appeal \"",
            "lines.push(sightTipLine(t));",
        ] {
            assert!(
                EMBEDDED_INDEX.contains(piece),
                "the tile tooltip is missing {piece}"
            );
        }
        // Ground level is terrain alone; only the cover a tile offers others
        // picks up the feature on top of it.
        assert!(EMBEDDED_INDEX
            .contains("t.terrain === \"mountain\" ? 2 : (t.hills ? 1 : 0)"));
        assert!(EMBEDDED_INDEX.contains("sight_through"));
    }

    /// The map's own overlays must be siblings, not each other's children.
    ///
    /// `<section>` does not self-close on a nested `<section>`, so one missing
    /// `</section>` silently reparents everything after it. That is how the
    /// tooltip, Diplomacy, the city screen, Quick Deals and the capture choice
    /// all ended up inside `#empire`, which is `display: none` until the
    /// Government screen is open — every one of them invisible, with no error
    /// anywhere. Nothing in the CSS or the script can find this; only the
    /// markup can.
    #[test]
    fn the_map_overlays_are_siblings_and_not_nested_dialogs() {
        let start = EMBEDDED_INDEX
            .find("<div id=\"maparea\">")
            .expect("the map area is declared");
        let end = EMBEDDED_INDEX
            .find("<div id=\"side\">")
            .expect("the side panel follows it");
        // Pure markup: the inline script is far below this window.
        let markup = &EMBEDDED_INDEX[start..end];
        let mut depth = 0i32;
        let mut at = 0usize;
        while let Some(next) = markup[at..].find("<section") {
            let open = at + next;
            let close = markup[at..open].matches("</section>").count() as i32;
            depth -= close;
            assert!(depth >= 0, "a stray </section> closes past the map area");
            depth += 1;
            at = open + "<section".len();
        }
        depth -= markup[at..].matches("</section>").count() as i32;
        assert_eq!(
            depth, 0,
            "every map overlay must close its own <section>; an unclosed one \
             hides the tooltip and every dialog after it inside #empire"
        );
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
        frame_painted: Condvar::new(),
        simulation_frame_gate: Mutex::new(()),
        latest: Mutex::new(None),
        turn_ready: Condvar::new(),
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
