//! Matched evaluation of one adaptive Conquest air wing.
//!
//! The focal controller is run on a clone and its successful actions are
//! replayed up to `EndTurn`. Treatment may then make one ordinary legal
//! production substitution: first one Aerodrome, then at most two aircraft.
//! Every cost and all later tactics remain in the shipped engine/controller.

use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::evolve::Champion;
use civvis::game::{default_difficulty, Action, Game, GameOptions, Item, VictoryConditions};
use civvis::name::Name;
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use civvis::Pos;
use std::cmp::Ordering;
use std::collections::BTreeMap;

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 10_050_000;
const SCREEN_MAPS: usize = 30;
const SCREEN_SEED: u64 = 10_051_000;
const HOLDOUT_MAPS: usize = 120;
const HOLDOUT_SEED: u64 = 10_052_000;
const NOMINAL_TURNS: u32 = 250;
const OBSERVE_THROUGH: u32 = 320;
const FROZEN_AI: &str = "advanced_evolved";
const FROZEN_CHAMPION_GENERATION: u32 = 14;
const FROZEN_CHAMPION_FNV1A: u64 = 0x40b1_fbb2_a5b8_8bc6;
const AIRCRAFT_TARGET: usize = 2;
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
    "--difficulty",
    "--victories",
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
        "data/evolved/best.json changed after the airpower preregistration"
    );
    let champion: Champion = serde_json::from_str(EMBEDDED_CHAMPION)
        .expect("the committed advanced_evolved champion must be valid JSON");
    assert_eq!(
        champion.gen, FROZEN_CHAMPION_GENERATION,
        "airpower evaluator champion generation changed"
    );
    champion
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
        } else if VALUE_OPTIONS.contains(&argument) || argument == "--ai" {
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
    for (index, arg) in args.iter().enumerate() {
        if arg != key {
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

#[allow(clippy::too_many_arguments)]
fn exact_common_profile(
    args: &[String],
    deployment_mix: bool,
    ai_name: &str,
    nominal_turns: u32,
    observe_through: u32,
    speed: &str,
    map_poles: MapPoles,
    randomize_civs: bool,
    victory_names: &str,
    jobs: usize,
) -> bool {
    deployment_mix
        && has_exact_flag(args, "--deployment-mix")
        && has_exact_value(args, "--ai", FROZEN_AI)
        && ai_name == FROZEN_AI
        && has_exact_number(args, "--turns", NOMINAL_TURNS as i64)
        && nominal_turns == NOMINAL_TURNS
        && has_exact_number(args, "--observe-through", OBSERVE_THROUGH as i64)
        && observe_through == OBSERVE_THROUGH
        && has_exact_value(args, "--speed", "online")
        && speed == "online"
        && has_exact_value(args, "--poles", "poles")
        && map_poles == MapPoles::Poles
        && has_exact_flag(args, "--randomize-civs")
        && randomize_civs
        && has_exact_value(args, "--victories", "science,culture,domination")
        && victory_names == "science,culture,domination"
        && has_exact_number(args, "--jobs", 6)
        && jobs == 6
        && !has_arg(args, "--difficulty")
}

fn district_is_family(rules: &Rules, district: Name, family: &str) -> bool {
    let mut current = district;
    loop {
        if current.as_str() == family {
            return true;
        }
        let Some(parent) = rules.districts.get(&current).and_then(|spec| spec.replaces) else {
            return false;
        };
        current = parent;
    }
}

fn is_air_unit(g: &Game, unit: Name) -> bool {
    g.rules
        .units
        .get(&unit)
        .is_some_and(|spec| spec.domain.as_deref() == Some("air"))
}

fn air_unit_unlocked_and_funded(g: &Game, pid: usize) -> bool {
    let player = &g.players[pid];
    g.rules.units.iter().any(|(name, spec)| {
        spec.domain.as_deref() == Some("air")
            && spec.buildable
            && spec
                .tech
                .as_ref()
                .is_none_or(|tech| player.techs.contains(tech))
            && spec
                .civic
                .as_ref()
                .is_none_or(|civic| player.civics.contains(civic))
            && spec
                .unique_to
                .as_deref()
                .is_none_or(|civilization| civilization == player.civ)
            && !g.rules.units.values().any(|candidate| {
                candidate.replaces == Some(*name)
                    && candidate.unique_to.as_deref() == Some(player.civ.as_str())
            })
            && !g.unit_is_obsolete(pid, *name)
            && spec.requires_resource.as_ref().is_none_or(|resource| {
                g.strategic_stockpile(pid, *resource) + f64::EPSILON >= spec.resource_cost
            })
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AerodromeCounts {
    built: usize,
    active: usize,
    queued: usize,
}

fn aerodrome_counts(g: &Game, pid: usize) -> AerodromeCounts {
    let mut counts = AerodromeCounts::default();
    for city in g.cities.values().filter(|city| city.owner == pid) {
        for (district, position) in &city.districts {
            if district_is_family(&g.rules, *district, "aerodrome") {
                counts.built += 1;
                counts.active +=
                    g.map.tiles.get(position).is_some_and(|tile| !tile.pillaged) as usize;
            }
        }
        counts.queued += matches!(
            city.queue.first(),
            Some(Item::District { district, .. })
                if district_is_family(&g.rules, *district, "aerodrome")
        ) as usize;
    }
    counts
}

fn aircraft_commitments(g: &Game, pid: usize) -> (usize, usize) {
    let living = g
        .units
        .values()
        .filter(|unit| unit.owner == pid && is_air_unit(g, unit.kind))
        .count();
    let queued = g
        .cities
        .values()
        .filter(|city| city.owner == pid)
        .filter(
            |city| matches!(city.queue.first(), Some(Item::Unit { unit }) if is_air_unit(g, *unit)),
        )
        .count();
    (living, queued)
}

fn item_progress_key(item: &Item) -> String {
    match item {
        Item::Formation { unit, formation } => format!("formation:{unit}:{formation}"),
        Item::Unit { unit } => format!("unit:{unit}"),
        Item::Building { building } => format!("building:{building}"),
        Item::District { district, pos } => {
            format!("district:{district}:{},{}", pos.0, pos.1)
        }
        Item::Wonder { wonder, pos } => format!("wonder:{wonder}:{},{}", pos.0, pos.1),
        Item::Repair { repair, pos } => format!("repair:{repair}:{},{}", pos.0, pos.1),
        Item::Project { project } => format!("project:{project}"),
        Item::Product { product } => format!("product:{product}"),
    }
}

fn remaining_turns(g: &Game, pid: usize, city: u32, item: &Item) -> f64 {
    let state = &g.cities[&city];
    let mut invested = state
        .production_progress
        .get(&item_progress_key(item))
        .copied()
        .unwrap_or(0.0);
    if state.queue.is_empty() || state.queue.first() == Some(item) {
        invested += state.production;
    }
    let remaining = (g.item_cost_for_city(pid, city, item) - invested).max(0.0);
    remaining / g.city_yields(city).production.max(1.0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OrderStage {
    Airbase,
    Aircraft(Name),
}

#[derive(Clone, Debug, PartialEq)]
struct AirpowerOrder {
    stage: OrderStage,
    action: Action,
}

#[derive(Clone, Copy, Debug)]
struct AirbaseCandidate {
    turns: f64,
    production: f64,
    city: u32,
    district: Name,
    pos: Pos,
}

fn airbase_better(candidate: AirbaseCandidate, old: AirbaseCandidate) -> bool {
    candidate
        .turns
        .total_cmp(&old.turns)
        .then_with(|| old.production.total_cmp(&candidate.production))
        .then_with(|| candidate.city.cmp(&old.city))
        .then_with(|| candidate.district.cmp(&old.district))
        .then_with(|| candidate.pos.cmp(&old.pos))
        == Ordering::Less
}

#[derive(Clone, Copy, Debug)]
struct AircraftCandidate {
    ranged: f64,
    defense: f64,
    turns: f64,
    city: u32,
    unit: Name,
}

fn aircraft_better(candidate: AircraftCandidate, old: AircraftCandidate) -> bool {
    old.ranged
        .total_cmp(&candidate.ranged)
        .then_with(|| old.defense.total_cmp(&candidate.defense))
        .then_with(|| candidate.turns.total_cmp(&old.turns))
        .then_with(|| candidate.city.cmp(&old.city))
        .then_with(|| candidate.unit.cmp(&old.unit))
        == Ordering::Less
}

fn airpower_order(
    g: &Game,
    pid: usize,
    strategy: Option<&str>,
    airbase_already_ordered: bool,
) -> Option<AirpowerOrder> {
    if strategy != Some("conquest") || !g.players[pid].techs.contains(&Name::new("flight")) {
        return None;
    }
    let bases = aerodrome_counts(g, pid);
    if bases.built + bases.queued == 0 {
        if airbase_already_ordered || !air_unit_unlocked_and_funded(g, pid) {
            return None;
        }
        let mut best: Option<AirbaseCandidate> = None;
        for city in g.player_city_ids(pid) {
            let production = g.city_yields(city).production;
            for item in g.producible_items(pid, city) {
                let Item::District { district, pos } = item else {
                    continue;
                };
                if !district_is_family(&g.rules, district, "aerodrome") {
                    continue;
                }
                let item = Item::District { district, pos };
                let candidate = AirbaseCandidate {
                    turns: remaining_turns(g, pid, city, &item),
                    production,
                    city,
                    district,
                    pos,
                };
                if best.is_none_or(|old| airbase_better(candidate, old)) {
                    best = Some(candidate);
                }
            }
        }
        let chosen = best?;
        return Some(AirpowerOrder {
            stage: OrderStage::Airbase,
            action: Action::Produce {
                city: chosen.city,
                item: Item::District {
                    district: chosen.district,
                    pos: chosen.pos,
                },
            },
        });
    }
    if bases.active == 0 {
        return None;
    }

    let (living, queued) = aircraft_commitments(g, pid);
    if living + queued >= AIRCRAFT_TARGET {
        return None;
    }
    let mut best: Option<AircraftCandidate> = None;
    for city in g.player_city_ids(pid) {
        if matches!(
            g.cities[&city].queue.first(),
            Some(Item::Unit { unit }) if is_air_unit(g, *unit)
        ) {
            continue;
        }
        for item in g.producible_items(pid, city) {
            let Item::Unit { unit } = item else {
                continue;
            };
            let spec = &g.rules.units[&unit];
            if spec.domain.as_deref() != Some("air") {
                continue;
            }
            let item = Item::Unit { unit };
            let candidate = AircraftCandidate {
                ranged: spec.ranged_attack_strength(),
                defense: spec.strength,
                turns: remaining_turns(g, pid, city, &item),
                city,
                unit,
            };
            if best.is_none_or(|old| aircraft_better(candidate, old)) {
                best = Some(candidate);
            }
        }
    }
    let chosen = best?;
    Some(AirpowerOrder {
        stage: OrderStage::Aircraft(chosen.unit),
        action: Action::Produce {
            city: chosen.city,
            item: Item::Unit { unit: chosen.unit },
        },
    })
}

/// Retain the shipped controller state while opening a pre-EndTurn treatment
/// seam. The action log is authoritative: every successful stock action is
/// replayed, in order, and any mismatch invalidates the harness.
fn replay_stock_actions_without_end(
    game: &mut Game,
    ai: &mut AdvancedAi,
    pid: usize,
) -> Result<(), String> {
    let mut observed = game.clone();
    let before = observed.log.len();
    let mut actor = ai.clone();
    actor.take_turn(&mut observed, pid);
    let mut actions: Vec<(usize, Action)> = observed.log.since(before).cloned().collect();
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
    for (owner, action) in actions {
        if owner != pid {
            return Err(format!(
                "stock seat {pid} logged an action for seat {owner}: {action:?}"
            ));
        }
        game.apply(owner, &action).map_err(|why| {
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

#[derive(Clone, Debug, Default, PartialEq)]
struct TreatmentCensus {
    focal_turns: u32,
    conquest_turns: u32,
    flight_turns: u32,
    resource_ready_turns: u32,
    airbase_opportunities: u32,
    aircraft_opportunities: u32,
    airbase_orders: u32,
    aircraft_orders: u32,
    strategic_material_committed: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Control,
    ReplayNull,
    Treatment,
}

#[derive(Clone, Debug, PartialEq)]
struct GameResult {
    won: bool,
    victory: Option<String>,
    reported_turn: u32,
    policy_max_turns: u32,
    score: i64,
    major_score_share: f64,
    fitness: f64,
    science_progress: f64,
    military_power: f64,
    cities: usize,
    kills: i64,
    captures: i64,
    territorial_index: i64,
    built_aerodromes: usize,
    queued_aerodromes: usize,
    trained_aircraft: i64,
    live_aircraft: usize,
    queued_aircraft: usize,
    offensive_air_actions: usize,
    air_rebases: usize,
    air_patrols: usize,
    trained_by_type: BTreeMap<String, i64>,
    live_by_type: BTreeMap<String, usize>,
    queued_by_type: BTreeMap<String, usize>,
    census: TreatmentCensus,
    serialized_world: Option<Vec<u8>>,
}

fn increment<K: Ord>(map: &mut BTreeMap<K, usize>, key: K) {
    *map.entry(key).or_default() += 1;
}

fn terminal_aircraft(
    g: &Game,
    pid: usize,
) -> (
    BTreeMap<String, i64>,
    BTreeMap<String, usize>,
    BTreeMap<String, usize>,
) {
    let mut trained = BTreeMap::new();
    for (name, spec) in &g.rules.units {
        if spec.domain.as_deref() != Some("air") {
            continue;
        }
        let count = g.players[pid]
            .counters
            .get(&format!("trained:{name}"))
            .copied()
            .unwrap_or(0);
        if count > 0 {
            trained.insert(name.to_string(), count);
        }
    }
    let mut living = BTreeMap::new();
    for unit in g
        .units
        .values()
        .filter(|unit| unit.owner == pid && is_air_unit(g, unit.kind))
    {
        increment(&mut living, unit.kind.to_string());
    }
    let mut queued = BTreeMap::new();
    for city in g.cities.values().filter(|city| city.owner == pid) {
        if let Some(Item::Unit { unit }) = city.queue.first() {
            if is_air_unit(g, *unit) {
                increment(&mut queued, unit.to_string());
            }
        }
    }
    (trained, living, queued)
}

fn territorial_index(g: &Game, pid: usize) -> i64 {
    let foreign_cities = g
        .cities
        .values()
        .filter(|city| city.owner == pid && city.original_owner != pid)
        .count() as i64;
    let foreign_major_capitals = g
        .cities
        .values()
        .filter(|city| {
            city.owner == pid
                && city.original_owner != pid
                && city.is_capital
                && g.players
                    .get(city.original_owner)
                    .is_some_and(|owner| !owner.is_minor && !owner.is_barbarian)
        })
        .count() as i64;
    foreign_cities + 9 * foreign_major_capitals
}

fn play(
    options: GameOptions,
    focal: usize,
    mode: Mode,
    observe_through: u32,
    weights: &Weights,
    capture_world: bool,
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
        if pid == focal && mode != Mode::Control {
            replay_stock_actions_without_end(&mut game, &mut ais[pid], pid)
                .unwrap_or_else(|why| panic!("turn {} seat {pid}: {why}", game.turn));
        } else {
            ais[pid].take_turn(&mut game, pid);
        }

        if mode == Mode::Treatment && pid == focal && game.winner.is_none() && game.current == pid {
            census.focal_turns += 1;
            let strategy = ais[pid].strategy_label();
            census.conquest_turns += (strategy == Some("conquest")) as u32;
            let flight = game.players[pid].techs.contains(&Name::new("flight"));
            census.flight_turns += (strategy == Some("conquest") && flight) as u32;
            let resource_ready = air_unit_unlocked_and_funded(&game, pid);
            census.resource_ready_turns +=
                (strategy == Some("conquest") && flight && resource_ready) as u32;
            if let Some(order) = airpower_order(&game, pid, strategy, census.airbase_orders > 0) {
                let resource_before = match order.stage {
                    OrderStage::Aircraft(unit) => game.rules.units[&unit]
                        .requires_resource
                        .map(|resource| (resource, game.strategic_stockpile(pid, resource))),
                    OrderStage::Airbase => None,
                };
                match order.stage {
                    OrderStage::Airbase => census.airbase_opportunities += 1,
                    OrderStage::Aircraft(_) => census.aircraft_opportunities += 1,
                }
                game.apply(pid, &order.action).unwrap_or_else(|why| {
                    panic!(
                        "turn {} seat {pid}: selected airpower order became illegal: {why}; {:?}",
                        game.turn, order.action
                    )
                });
                match order.stage {
                    OrderStage::Airbase => census.airbase_orders += 1,
                    OrderStage::Aircraft(_) => census.aircraft_orders += 1,
                }
                if let Some((resource, before)) = resource_before {
                    census.strategic_material_committed +=
                        (before - game.strategic_stockpile(pid, resource)).max(0.0);
                }
            }
        }
        if mode != Mode::Control && game.winner.is_none() && game.current == pid {
            game.apply(pid, &Action::EndTurn).unwrap_or_else(|why| {
                panic!("turn {} seat {pid}: EndTurn failed: {why}", game.turn)
            });
        }
    }
    assert_eq!(
        game.max_turns, policy_max_turns,
        "external continuation changed the policy-visible horizon"
    );

    let bases = aerodrome_counts(&game, focal);
    let (trained_by_type, live_by_type, queued_by_type) = terminal_aircraft(&game, focal);
    let trained_aircraft = trained_by_type.values().sum();
    let live_aircraft = live_by_type.values().sum();
    let queued_aircraft = queued_by_type.values().sum();
    let mut offensive_air_actions = 0usize;
    let mut air_rebases = 0usize;
    let mut air_patrols = 0usize;
    for (owner, action) in game.log.iter() {
        if *owner != focal {
            continue;
        }
        match action {
            Action::AirStrike { .. } | Action::AirPillage { .. } => {
                offensive_air_actions += 1;
            }
            Action::AirRebase { .. } => air_rebases += 1,
            Action::AirPatrol { .. } => air_patrols += 1,
            _ => {}
        }
    }
    let major_score_total = game
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| game.score(player.id).max(0) as f64)
        .sum::<f64>();
    let score = game.score(focal);
    let major_score_share = if major_score_total > 0.0 {
        score.max(0) as f64 / major_score_total
    } else {
        0.0
    };
    let won = game.winner == Some(focal);
    let result = GameResult {
        won,
        victory: won.then(|| game.victory_type.clone()).flatten(),
        reported_turn: if game.winner.is_some() {
            game.reported_turn()
        } else {
            observe_through
        },
        policy_max_turns,
        score,
        major_score_share,
        fitness: 80.0 * major_score_share + 20.0 * if won { 1.0 } else { 0.0 },
        science_progress: game.victory_races(focal, 0).science,
        military_power: game.military_power(focal),
        cities: game.player_city_ids(focal).len(),
        kills: game.players[focal]
            .counters
            .get("kills")
            .copied()
            .unwrap_or(0),
        captures: game.players[focal]
            .counters
            .get("captures")
            .copied()
            .unwrap_or(0),
        territorial_index: territorial_index(&game, focal),
        built_aerodromes: bases.built,
        queued_aerodromes: bases.queued,
        trained_aircraft,
        live_aircraft,
        queued_aircraft,
        offensive_air_actions,
        air_rebases,
        air_patrols,
        trained_by_type,
        live_by_type,
        queued_by_type,
        census,
        serialized_world: capture_world.then(|| {
            serde_json::to_vec(&game).expect("terminal Game must serialize for the replay null")
        }),
    };
    result
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
    if control + treatment > 0.0 {
        treatment / (control + treatment)
    } else {
        0.5
    }
}

fn map_fitness_delta(result: &MapResult) -> f64 {
    result
        .control
        .iter()
        .zip(&result.comparison)
        .map(|(old, new)| new.fitness - old.fitness)
        .sum::<f64>()
        / 2.0
}

fn map_territory_delta(result: &MapResult) -> f64 {
    result
        .control
        .iter()
        .zip(&result.comparison)
        .map(|(old, new)| (new.territorial_index - old.territorial_index) as f64)
        .sum::<f64>()
        / 2.0
}

fn map_terminal_share(result: &MapResult) -> f64 {
    result
        .control
        .iter()
        .zip(&result.comparison)
        .map(|(old, new)| terminal_share(old.score, new.score))
        .sum::<f64>()
        / 2.0
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
    score_share: f64,
    fitness: f64,
    science_progress: f64,
    military_power: f64,
    cities: usize,
    kills: i64,
    captures: i64,
    territorial_index: i64,
    built_aerodromes: usize,
    queued_aerodromes: usize,
    trained_aircraft: i64,
    live_aircraft: usize,
    queued_aircraft: usize,
    offensive_air_actions: usize,
    air_rebases: usize,
    air_patrols: usize,
    focal_turns: u64,
    conquest_turns: u64,
    flight_turns: u64,
    resource_ready_turns: u64,
    airbase_opportunities: u64,
    aircraft_opportunities: u64,
    airbase_orders: u64,
    aircraft_orders: u64,
    material_committed: f64,
    fired_games: usize,
    victories: BTreeMap<String, usize>,
    trained_by_type: BTreeMap<String, i64>,
}

impl ArmSummary {
    fn record(&mut self, result: &GameResult) {
        self.games += 1;
        self.wins += result.won as usize;
        self.turns += result.reported_turn as u64;
        self.score += result.score;
        self.score_share += result.major_score_share;
        self.fitness += result.fitness;
        self.science_progress += result.science_progress;
        self.military_power += result.military_power;
        self.cities += result.cities;
        self.kills += result.kills;
        self.captures += result.captures;
        self.territorial_index += result.territorial_index;
        self.built_aerodromes += result.built_aerodromes;
        self.queued_aerodromes += result.queued_aerodromes;
        self.trained_aircraft += result.trained_aircraft;
        self.live_aircraft += result.live_aircraft;
        self.queued_aircraft += result.queued_aircraft;
        self.offensive_air_actions += result.offensive_air_actions;
        self.air_rebases += result.air_rebases;
        self.air_patrols += result.air_patrols;
        self.focal_turns += result.census.focal_turns as u64;
        self.conquest_turns += result.census.conquest_turns as u64;
        self.flight_turns += result.census.flight_turns as u64;
        self.resource_ready_turns += result.census.resource_ready_turns as u64;
        self.airbase_opportunities += result.census.airbase_opportunities as u64;
        self.aircraft_opportunities += result.census.aircraft_opportunities as u64;
        self.airbase_orders += result.census.airbase_orders as u64;
        self.aircraft_orders += result.census.aircraft_orders as u64;
        self.material_committed += result.census.strategic_material_committed;
        self.fired_games +=
            (result.census.airbase_orders + result.census.aircraft_orders > 0) as usize;
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
        for (kind, count) in &result.trained_by_type {
            *self.trained_by_type.entry(kind.clone()).or_default() += *count;
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct GateInputs {
    coverage: f64,
    airbase_orders: u64,
    treatment_built_aerodromes: usize,
    control_built_aerodromes: usize,
    treatment_trained_aircraft: i64,
    control_trained_aircraft: i64,
    offensive_air_actions: usize,
    fitness_delta: f64,
    fitness_favorable: usize,
    fitness_adverse: usize,
    fitness_p: f64,
    treatment_wins: usize,
    control_wins: usize,
    paired_win_score: f64,
    terminal_score_share: f64,
    territory_delta: f64,
}

fn mechanism_passes(gate: GateInputs, minimum_aircraft: i64, minimum_actions: usize) -> bool {
    gate.coverage >= 0.10
        && gate.treatment_built_aerodromes > gate.control_built_aerodromes
        && gate.treatment_trained_aircraft > gate.control_trained_aircraft
        && gate.treatment_trained_aircraft >= minimum_aircraft
        && gate.offensive_air_actions >= minimum_actions
}

fn screen_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate, 6, 6)
        && gate.airbase_orders >= 6
        && gate.fitness_delta >= 0.25
        && gate.fitness_favorable > gate.fitness_adverse
        && gate.fitness_p <= 0.20
        && gate.treatment_wins >= gate.control_wins
        && gate.paired_win_score >= 0.50
        && gate.terminal_score_share >= 0.495
        && gate.territory_delta >= 0.0
}

fn holdout_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate, 20, 20)
        && gate.fitness_delta > 0.0
        && gate.fitness_favorable > gate.fitness_adverse
        && gate.fitness_p < 0.05
        && gate.treatment_wins >= gate.control_wins
        && gate.paired_win_score >= 0.50
        && gate.terminal_score_share >= 0.50
        && gate.territory_delta >= 0.0
}

#[derive(Default)]
struct StratumSummary {
    maps: usize,
    fitness_delta: f64,
    favorable: usize,
    adverse: usize,
    win_score: f64,
    terminal_share: f64,
    territory_delta: f64,
    orders: u64,
    trained_aircraft: i64,
    offensive_actions: usize,
}

impl StratumSummary {
    fn record(&mut self, result: &MapResult) {
        self.maps += 1;
        let delta = map_fitness_delta(result);
        self.fitness_delta += delta;
        if delta > 1e-9 {
            self.favorable += 1;
        } else if delta < -1e-9 {
            self.adverse += 1;
        }
        self.win_score += map_win_score(
            result.control.iter().filter(|game| game.won).count(),
            result.comparison.iter().filter(|game| game.won).count(),
        );
        self.terminal_share += map_terminal_share(result);
        self.territory_delta += map_territory_delta(result);
        for game in &result.comparison {
            self.orders += (game.census.airbase_orders + game.census.aircraft_orders) as u64;
            self.trained_aircraft += game.trained_aircraft;
            self.offensive_actions += game.offensive_air_actions;
        }
    }
}

fn summarize_stratum<'a>(results: impl IntoIterator<Item = &'a MapResult>) -> StratumSummary {
    let mut summary = StratumSummary::default();
    for result in results {
        summary.record(result);
    }
    summary
}

fn print_stratum(label: &str, summary: &StratumSummary) {
    let n = summary.maps.max(1) as f64;
    println!(
        "  {label:<28} maps {:>3}; fitness {:+.3}; dir {}/{}/{}; win {:>5.1}%; score {:>5.1}%; territory {:+.3}; orders {}; trained {}; offense {}",
        summary.maps,
        summary.fitness_delta / n,
        summary.favorable,
        summary.maps - summary.favorable - summary.adverse,
        summary.adverse,
        100.0 * summary.win_score / n,
        100.0 * summary.terminal_share / n,
        summary.territory_delta / n,
        summary.orders,
        summary.trained_aircraft,
        summary.offensive_actions,
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(why) = validate_args(&args) {
        eprintln!("{why}");
        std::process::exit(2);
    }
    let null_replay = has_arg(&args, "--null");
    let deployment_mix = has_arg(&args, "--deployment-mix");
    let ai_name = text(&args, "--ai", FROZEN_AI);
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
    let nominal_turns = number(&args, "--turns", NOMINAL_TURNS as i64).max(1) as u32;
    let observe_through = number(&args, "--observe-through", OBSERVE_THROUGH as i64).max(1) as u32;
    if observe_through < nominal_turns {
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
    let jobs = match number(&args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    }
    .clamp(1, 6);
    let speed = text(&args, "--speed", "online");
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
    let poles_name = text(&args, "--poles", "poles");
    let map_poles = MapPoles::from_id(&poles_name).unwrap_or_else(|| {
        eprintln!("unknown thermal distribution {poles_name:?}");
        std::process::exit(2);
    });
    let difficulty = text(&args, "--difficulty", &default_difficulty());
    if difficulty != default_difficulty() {
        eprintln!("this experiment resolves Prince difficulty; got {difficulty:?}");
        std::process::exit(2);
    }
    let victory_names = text(&args, "--victories", "science,culture,domination");
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

    println!("Adaptive Conquest airpower evaluator");
    println!(
        "controller: {ai_name}; embedded champion generation {}, FNV-1a {:#018x}",
        champion.gen,
        fnv1a(EMBEDDED_CHAMPION.as_bytes())
    );
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
        let stored = MapSize::from_dimensions(width, height)
            .map(|size| size.dimensions(map_topology))
            .unwrap_or((width, height));
        println!(
            "profile: diagnostic fixed cell; {players}p requested {width}x{height}, stored {}x{}, {city_states} city-states, map {}, shape {}",
            stored.0,
            stored.1,
            map_script.id(),
            map_topology.id(),
        );
    }
    println!(
        "rules: {nominal_turns} policy-visible {speed} turns, observe through {observe_through}, Prince, poles {}, civilizations {}, victories {victory_names}",
        map_poles.id(),
        if randomize_civs { "randomized" } else { "fixed" },
    );
    println!(
        "batch: {maps} maps x seats 0/final x control/comparison = {} games; seed {seed}; {jobs} jobs",
        maps * 4
    );
    println!(
        "comparison: {}",
        if null_replay {
            "NULL clone/action-log replay; intervention disabled"
        } else {
            "one legal Aerodrome then a two-aircraft Conquest wing"
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
                difficulty: difficulty.clone(),
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
                    null_replay,
                ),
                play(
                    options.clone(),
                    seats[1],
                    Mode::Control,
                    observe_through,
                    &champion.weights,
                    null_replay,
                ),
            ];
            let comparison_mode = if null_replay {
                Mode::ReplayNull
            } else {
                Mode::Treatment
            };
            let comparison = [
                play(
                    options.clone(),
                    seats[0],
                    comparison_mode,
                    observe_through,
                    &champion.weights,
                    null_replay,
                ),
                play(
                    options,
                    seats[1],
                    comparison_mode,
                    observe_through,
                    &champion.weights,
                    null_replay,
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

    let mut control = ArmSummary::default();
    let mut comparison = ArmSummary::default();
    let mut paired_win_score = 0.0;
    let mut terminal_score_share = 0.0;
    let mut fitness_delta = 0.0;
    let mut territory_delta = 0.0;
    let mut fitness_favorable = 0usize;
    let mut fitness_adverse = 0usize;
    let mut win_favorable = 0usize;
    let mut win_adverse = 0usize;
    let mut helped_cells = 0usize;
    let mut hurt_cells = 0usize;
    let mut exact_mismatches = 0usize;
    for result in &results {
        let control_wins = result.control.iter().filter(|game| game.won).count();
        let comparison_wins = result.comparison.iter().filter(|game| game.won).count();
        paired_win_score += map_win_score(control_wins, comparison_wins);
        match comparison_wins.cmp(&control_wins) {
            Ordering::Greater => win_favorable += 1,
            Ordering::Less => win_adverse += 1,
            Ordering::Equal => {}
        }
        terminal_score_share += map_terminal_share(result);
        let map_fitness = map_fitness_delta(result);
        fitness_delta += map_fitness;
        if map_fitness > 1e-9 {
            fitness_favorable += 1;
        } else if map_fitness < -1e-9 {
            fitness_adverse += 1;
        }
        territory_delta += map_territory_delta(result);
        for (old, new) in result.control.iter().zip(&result.comparison) {
            control.record(old);
            comparison.record(new);
            match (old.won, new.won) {
                (false, true) => helped_cells += 1,
                (true, false) => hurt_cells += 1,
                _ => {}
            }
            if null_replay {
                exact_mismatches += (old != new) as usize;
            }
        }
    }
    let n_maps = maps as f64;
    paired_win_score /= n_maps;
    terminal_score_share /= n_maps;
    fitness_delta /= n_maps;
    territory_delta /= n_maps;
    let fitness_p = exact_two_sided(fitness_favorable, fitness_favorable + fitness_adverse);
    let win_p = exact_two_sided(win_favorable, win_favorable + win_adverse);
    let coverage = comparison.fired_games as f64 / comparison.games.max(1) as f64;
    let gate = GateInputs {
        coverage,
        airbase_orders: comparison.airbase_orders,
        treatment_built_aerodromes: comparison.built_aerodromes,
        control_built_aerodromes: control.built_aerodromes,
        treatment_trained_aircraft: comparison.trained_aircraft,
        control_trained_aircraft: control.trained_aircraft,
        offensive_air_actions: comparison.offensive_air_actions,
        fitness_delta,
        fitness_favorable,
        fitness_adverse,
        fitness_p,
        treatment_wins: comparison.wins,
        control_wins: control.wins,
        paired_win_score,
        terminal_score_share,
        territory_delta,
    };

    println!();
    println!(
        "arm          wins  turns   score share fitness science military cities kills captures territory aero built+queue air trained/live/queue"
    );
    for (label, arm) in [("control", &control), ("comparison", &comparison)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{label:<12} {:>3}/{:<3} {:>6.1} {:>7.1} {:>5.1}% {:>7.2} {:>7.2} {:>8.1} {:>6.2} {:>6.1} {:>8.1} {:>9.2} {:>5}+{:<5} {:>5}/{:<4}/{:<5}",
            arm.wins,
            arm.games,
            arm.turns as f64 / n,
            arm.score as f64 / n,
            100.0 * arm.score_share / n,
            arm.fitness / n,
            arm.science_progress / n,
            arm.military_power / n,
            arm.cities as f64 / n,
            arm.kills as f64 / n,
            arm.captures as f64 / n,
            arm.territorial_index as f64 / n,
            arm.built_aerodromes,
            arm.queued_aerodromes,
            arm.trained_aircraft,
            arm.live_aircraft,
            arm.queued_aircraft,
        );
    }
    println!(
        "victory types: control {:?}; comparison {:?}",
        control.victories, comparison.victories
    );
    println!(
        "treatment mechanism: {}/{} games fired ({:.1}%); focal/conquest/Conquest+Flight/resource-ready turns {}/{}/{}/{}; base opportunities/orders {}/{}; aircraft opportunities/orders {}/{}; strategic material committed {:.1}",
        comparison.fired_games,
        comparison.games,
        100.0 * coverage,
        comparison.focal_turns,
        comparison.conquest_turns,
        comparison.flight_turns,
        comparison.resource_ready_turns,
        comparison.airbase_opportunities,
        comparison.airbase_orders,
        comparison.aircraft_opportunities,
        comparison.aircraft_orders,
        comparison.material_committed,
    );
    println!(
        "air operations: control offense/rebase/patrol {}/{}/{}; comparison {}/{}/{}; trained types {:?}",
        control.offensive_air_actions,
        control.air_rebases,
        control.air_patrols,
        comparison.offensive_air_actions,
        comparison.air_rebases,
        comparison.air_patrols,
        comparison.trained_by_type,
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
        "primary 80/20 fitness delta: {fitness_delta:+.3} points/map; favorable {fitness_favorable}, neutral {}, adverse {fitness_adverse}; exact p={fitness_p:.4}",
        maps - fitness_favorable - fitness_adverse,
    );
    println!(
        "paired terminal raw-score share: {:.2}%; territorial-index delta {territory_delta:+.3}/map",
        100.0 * terminal_score_share
    );

    println!("deployment-axis summaries (descriptive only; pooled gate decides):");
    for value in axis_values(&results, |profile| profile.players) {
        let summary = summarize_stratum(
            results
                .iter()
                .filter(|result| result.profile.players == value),
        );
        print_stratum(&format!("players={value}"), &summary);
    }
    for value in axis_values(&results, |profile| profile.map_script) {
        let summary = summarize_stratum(
            results
                .iter()
                .filter(|result| result.profile.map_script == value),
        );
        print_stratum(&format!("map={}", value.id()), &summary);
    }
    for value in axis_values(&results, |profile| profile.map_topology) {
        let summary = summarize_stratum(
            results
                .iter()
                .filter(|result| result.profile.map_topology == value),
        );
        print_stratum(&format!("shape={}", value.id()), &summary);
    }

    let exact_profile = exact_common_profile(
        &args,
        deployment_mix,
        &ai_name,
        nominal_turns,
        observe_through,
        &speed,
        map_poles,
        randomize_civs,
        &victory_names,
        jobs,
    );
    if null_replay {
        if exact_mismatches > 0 {
            println!(
                "null sanity: BROKEN — {exact_mismatches}/{} direct/replay matched focal cells differed",
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
                "frozen replay null: PASS — all {} direct/replay serialized worlds and results reproduced exactly",
                control.games
            );
        } else {
            println!(
                "diagnostic replay null: PASS — all {} matched cells exact; no registered gate spent",
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
            "development screen: {}",
            if screen_passes(gate) {
                "PASS — run only the fixed disjoint holdout"
            } else {
                "STOP — retain stock; do not tune, retry, or inspect the holdout"
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
                "PASS — a separate gameplay-integration PR is permitted"
            } else {
                "RETAIN stock AdvancedAi — no gameplay integration"
            }
        );
    } else {
        println!("decision: DIAGNOSTIC ONLY — not a preregistered treatment batch");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn supplied_values_fail_closed_including_malformed_duplicates() {
        assert_eq!(option_value(&[], "--speed").unwrap(), None);
        assert!(option_value(&strings(&["--speed"]), "--speed").is_err());
        assert!(option_value(&strings(&["--speed", "--maps"]), "--speed").is_err());
        assert_eq!(
            number_value(&strings(&["--turns", "250"]), "--turns").unwrap(),
            Some(250)
        );
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
    fn canonical_common_profile_requires_every_raw_flag_once() {
        let args = strings(&[
            "--deployment-mix",
            "--ai",
            "advanced_evolved",
            "--turns",
            "250",
            "--observe-through",
            "320",
            "--speed",
            "online",
            "--poles",
            "poles",
            "--randomize-civs",
            "--victories",
            "science,culture,domination",
            "--jobs",
            "6",
        ]);
        assert!(exact_common_profile(
            &args,
            true,
            FROZEN_AI,
            250,
            320,
            "online",
            MapPoles::Poles,
            true,
            "science,culture,domination",
            6,
        ));
        let mut padded = args.clone();
        let turns = padded.iter().position(|arg| arg == "250").unwrap();
        padded[turns] = "0250".to_string();
        assert!(!exact_common_profile(
            &padded,
            true,
            FROZEN_AI,
            250,
            320,
            "online",
            MapPoles::Poles,
            true,
            "science,culture,domination",
            6,
        ));
        let mut explicit_difficulty = args;
        explicit_difficulty.extend(strings(&["--difficulty", "prince"]));
        assert!(!exact_common_profile(
            &explicit_difficulty,
            true,
            FROZEN_AI,
            250,
            320,
            "online",
            MapPoles::Poles,
            true,
            "science,culture,domination",
            6,
        ));
    }

    #[test]
    fn deployment_cycle_and_frozen_batches_are_exact() {
        let expected = [
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
        ];
        for (index, expected) in expected.into_iter().enumerate() {
            let profile = deployment_profile(index);
            assert_eq!(
                (
                    profile.players,
                    profile.width,
                    profile.height,
                    profile.city_states,
                    profile.map_script,
                    profile.map_topology,
                ),
                expected
            );
        }
        let cycle = (0..126).map(deployment_profile).collect::<Vec<_>>();
        for (index, profile) in cycle.iter().enumerate() {
            assert!(!cycle[..index].contains(profile), "duplicate at {index}");
        }
        assert_eq!(deployment_profile(126), deployment_profile(0));
        assert_eq!(
            deployment_counts(SCREEN_MAPS, |profile| profile.map_topology),
            vec![(MapTopology::Flat, 15), (MapTopology::Planet, 15)]
        );
        assert_eq!(
            deployment_counts(HOLDOUT_MAPS, |profile| profile.map_topology),
            vec![(MapTopology::Flat, 60), (MapTopology::Planet, 60)]
        );
    }

    fn airpower_fixture() -> Game {
        let mut game = Game::new_full(1, 34, 20, 10_050_999, 40, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        game.players[0].techs = game.rules.techs.keys().copied().collect();
        game.players[0].civics = game.rules.civics.keys().copied().collect();
        game.players[0]
            .strategic_resources
            .insert(Name::new("oil"), 50.0);
        game.players[0]
            .strategic_resources
            .insert(Name::new("aluminum"), 50.0);
        let city = game.player_city_ids(0)[0];
        game.cities.get_mut(&city).unwrap().pop = 12;
        for position in game.cities[&city].owned_tiles.clone() {
            if position == game.cities[&city].pos {
                continue;
            }
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = Name::new("plains");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.district_foundation = None;
            tile.wonder = None;
        }
        game
    }

    #[test]
    fn treatment_bootstraps_one_real_airbase_then_a_sequential_two_unit_wing() {
        let mut game = airpower_fixture();
        assert!(airpower_order(&game, 0, Some("science"), false).is_none());
        let base = airpower_order(&game, 0, Some("conquest"), false).unwrap();
        assert_eq!(base.stage, OrderStage::Airbase);
        let Action::Produce {
            city,
            item: Item::District { district, pos },
        } = base.action
        else {
            panic!("expected an Aerodrome production order")
        };
        assert!(district_is_family(&game.rules, district, "aerodrome"));
        game.apply(
            0,
            &Action::Produce {
                city,
                item: Item::District { district, pos },
            },
        )
        .unwrap();
        assert!(
            airpower_order(&game, 0, Some("conquest"), false).is_none(),
            "a queued airbase must prevent a duplicate treatment base"
        );

        game.cities.get_mut(&city).unwrap().queue.clear();
        assert!(
            airpower_order(&game, 0, Some("conquest"), true).is_none(),
            "a displaced treatment queue must not authorize a second airbase"
        );
        game.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(district, pos);
        game.map.tiles.get_mut(&pos).unwrap().district = Some(district);
        game.map.tiles.get_mut(&pos).unwrap().district_foundation = None;
        let aircraft = airpower_order(&game, 0, Some("conquest"), true).unwrap();
        assert_eq!(
            aircraft.stage,
            OrderStage::Aircraft(Name::new("jet_bomber"))
        );
        game.apply(0, &aircraft.action).unwrap();
        assert!(
            airpower_order(&game, 0, Some("conquest"), true).is_none(),
            "one airbase cannot queue the second aircraft over the first"
        );

        let first_kind = match aircraft.stage {
            OrderStage::Aircraft(unit) => unit,
            OrderStage::Airbase => unreachable!(),
        };
        let first_key = item_progress_key(&Item::Unit { unit: first_kind });
        let existing = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "warrior")
            .unwrap();
        game.units.get_mut(&existing).unwrap().kind = first_kind;
        let city_state = game.cities.get_mut(&city).unwrap();
        city_state.queue.clear();
        city_state.strategic_resource_commitments.remove(&first_key);
        let second = airpower_order(&game, 0, Some("conquest"), true).unwrap();
        assert_eq!(second.stage, OrderStage::Aircraft(first_kind));
        game.apply(0, &second.action).unwrap();
        assert!(
            airpower_order(&game, 0, Some("conquest"), true).is_none(),
            "one living and one queued aircraft must close the two-unit cap"
        );
    }

    #[test]
    fn replay_defers_end_turn_and_reproduces_the_direct_world() {
        let game = Game::new(2, 20, 14, 10_050_998, 20, 0);
        let mut direct = game.clone();
        let mut direct_ai = AdvancedAi::new();
        direct_ai.take_turn(&mut direct, 0);

        let mut replay = game;
        let mut replay_ai = AdvancedAi::new();
        replay_stock_actions_without_end(&mut replay, &mut replay_ai, 0).unwrap();
        assert_eq!(replay.current, 0);
        replay.apply(0, &Action::EndTurn).unwrap();
        assert_eq!(
            serde_json::to_vec(&replay).unwrap(),
            serde_json::to_vec(&direct).unwrap()
        );
        assert_eq!(replay_ai.strategy_label(), direct_ai.strategy_label());
    }

    #[test]
    fn embedded_controller_is_the_frozen_nondefault_champion() {
        let champion = frozen_champion();
        let game = Game::new(2, 20, 14, 10_050_997, 1, 0);
        let ais = AdvancedAi::fleet_weighted(&game, &champion.weights);
        assert_eq!(champion.gen, 14);
        assert_eq!(ais[0].weights(), &champion.weights);
        assert_ne!(ais[0].weights(), &Weights::default());
    }

    fn passing_gate() -> GateInputs {
        GateInputs {
            coverage: 0.20,
            airbase_orders: 8,
            treatment_built_aerodromes: 10,
            control_built_aerodromes: 1,
            treatment_trained_aircraft: 24,
            control_trained_aircraft: 0,
            offensive_air_actions: 30,
            fitness_delta: 0.5,
            fitness_favorable: 20,
            fitness_adverse: 5,
            fitness_p: 0.004,
            treatment_wins: 8,
            control_wins: 7,
            paired_win_score: 0.52,
            terminal_score_share: 0.505,
            territory_delta: 0.1,
        }
    }

    #[test]
    fn frozen_gates_require_mechanism_inference_and_harm_guards() {
        let passing = passing_gate();
        assert!(screen_passes(passing));
        assert!(holdout_passes(passing));
        assert!(!screen_passes(GateInputs {
            offensive_air_actions: 5,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            fitness_p: 0.21,
            ..passing
        }));
        assert!(!holdout_passes(GateInputs {
            terminal_score_share: 0.499,
            ..passing
        }));
        assert!(!holdout_passes(GateInputs {
            territory_delta: -0.01,
            ..passing
        }));
    }
}
