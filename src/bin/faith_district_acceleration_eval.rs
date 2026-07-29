//! Matched evaluation of Faith acceleration for stock-chosen districts.
//!
//! An untreated one-turn oracle names only districts stock would start now.
//! Treatment may complete one exact match with a legal, real-Faith
//! `BuyDistrict`, then lets the actual stateful controller replan normally.
use civvis::ai::{AdvancedAi, Ai, PlanReport};
use civvis::game::{Action, ActionFamilies, Game, GameOptions, Item, VictoryConditions};
use civvis::name::Name;
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use civvis::Pos;
use std::collections::{BTreeMap, BTreeSet};

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 10_039_999;
const SCREEN_MAPS: usize = 30;
const SCREEN_SEED: u64 = 10_040_000;
const HOLDOUT_MAPS: usize = 63;
const HOLDOUT_SEED: u64 = 10_041_000;
const NOMINAL_TURNS: u32 = 250;
const OBSERVE_THROUGH: u32 = 320;
const FINAL_VALUE_WINDOW: u32 = 20;
const CADENCE: u32 = 6;
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
    NullOracle,
    Acceleration,
}

#[derive(Clone, Debug)]
struct OracleTurn {
    actions: Vec<(usize, Action)>,
    plan: Option<PlanReport>,
    won: bool,
}

fn observe_stock_turn(game: &Game, ai: &AdvancedAi, pid: usize) -> OracleTurn {
    let mut observed = game.clone();
    let before = observed.log.len();
    let mut actor = ai.clone();
    actor.take_turn(&mut observed, pid);
    OracleTurn {
        actions: observed.log.since(before).cloned().collect(),
        plan: actor.plan_report(),
        won: observed.winner.is_some(),
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PurchaseChoice {
    action: Action,
    city: u32,
    district: Name,
    pos: Pos,
    family: String,
    faith_cost: f64,
    turns_saved: f64,
}

fn faith_reserve_from(
    strategy: &str,
    has_national_park_site: bool,
    naturalist_cost: f64,
    has_cold_war: bool,
) -> f64 {
    match strategy {
        "religion" => 180.0,
        "culture" if has_national_park_site => naturalist_cost,
        "culture" if has_cold_war => 700.0,
        _ => 80.0,
    }
}

fn faith_reserve(game: &Game, pid: usize, strategy: &str) -> f64 {
    let naturalist_cost = game.game_speed.scale(
        game.rules.units["naturalist"].cost
            + 100.0
                * game.players[pid]
                    .counters
                    .get("purchased:naturalist")
                    .copied()
                    .unwrap_or(0) as f64,
    );
    faith_reserve_from(
        strategy,
        !game.national_park_sites(pid).is_empty(),
        naturalist_cost,
        game.players[pid].civics.contains(&Name::new("cold_war")),
    )
}

fn district_remaining_cost(game: &Game, pid: usize, city: u32, item: &Item) -> f64 {
    let Item::District { district, pos } = item else {
        unreachable!("Faith acceleration only ranks districts")
    };
    let city_state = &game.cities[&city];
    let key = format!("district:{district}:{},{}", pos.0, pos.1);
    let mut invested = city_state
        .production_progress
        .get(&key)
        .copied()
        .unwrap_or(0.0);
    if city_state.queue.is_empty() || city_state.queue.first() == Some(item) {
        invested += city_state.production;
    }
    (game.item_cost_for_city(pid, city, item) - invested).max(0.0)
}

fn district_family(game: &Game, district: Name) -> Name {
    let mut current = district;
    for _ in 0..game.rules.districts.len() {
        let Some(parent) = game
            .rules
            .districts
            .get(&current)
            .and_then(|spec| spec.replaces)
        else {
            break;
        };
        current = parent;
    }
    current
}

fn exact_faith_purchase(legal: &[Action], city: u32, district: Name, pos: Pos) -> Option<Action> {
    legal
        .iter()
        .find(|action| {
            matches!(
                action,
                Action::BuyDistrict {
                    city: candidate_city,
                    district: candidate_district,
                    pos: candidate_pos,
                    currency,
                } if *candidate_city == city
                    && *candidate_district == district
                    && *candidate_pos == pos
                    && currency == "faith"
            )
        })
        .cloned()
}

fn choice_order(left: &PurchaseChoice, right: &PurchaseChoice) -> std::cmp::Ordering {
    left.turns_saved
        .total_cmp(&right.turns_saved)
        .then_with(|| right.city.cmp(&left.city))
        .then_with(|| right.district.cmp(&left.district))
        .then_with(|| right.pos.cmp(&left.pos))
}

#[derive(Clone, Debug, Default, PartialEq)]
struct AccelerationCensus {
    cadence_checks: u32,
    stock_district_intentions: u32,
    exact_legal_matches: u32,
    affordable_matches: u32,
    purchases: u32,
    failed_applications: u32,
    controller_turns_after_purchase: u32,
    controller_actions_after_purchase: u32,
    new_production_in_purchase_city: u32,
    subsequent_faith_actions: u32,
    faith_spent: f64,
    projected_turns_saved: f64,
    purchased_surviving: u32,
    families: BTreeMap<String, u32>,
}

#[derive(Clone, Debug)]
struct PurchaseRecord {
    city: u32,
    district: Name,
    pos: Pos,
}

#[derive(Default)]
struct AccelerationState {
    census: AccelerationCensus,
    attempted_turns: BTreeSet<u32>,
    records: Vec<PurchaseRecord>,
}

fn cadence_open(turn: u32, pid: usize, observe_through: u32) -> bool {
    turn.saturating_add(FINAL_VALUE_WINDOW) <= observe_through
        && turn % CADENCE == pid as u32 % CADENCE
}

fn purchase_candidates(
    game: &Game,
    pid: usize,
    oracle: &OracleTurn,
    census: &mut AccelerationCensus,
) -> Vec<PurchaseChoice> {
    if oracle.won {
        return Vec::new();
    }
    let Some(plan) = oracle.plan.as_ref() else {
        return Vec::new();
    };
    let legal = game.legal_actions_within(pid, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE);
    let bank = game.players[pid].faith;
    let reserve = faith_reserve(game, pid, plan.strategy);
    let mut choices = Vec::new();
    for (owner, action) in &oracle.actions {
        let Action::Produce {
            city,
            item: Item::District { district, pos },
        } = action
        else {
            continue;
        };
        if *owner != pid {
            continue;
        }
        census.stock_district_intentions += 1;
        let Some(purchase) = exact_faith_purchase(&legal, *city, *district, *pos) else {
            continue;
        };
        census.exact_legal_matches += 1;
        let item = Item::District {
            district: *district,
            pos: *pos,
        };
        let production = game.city_yields(*city).production.max(1.0);
        let turns_saved = district_remaining_cost(game, pid, *city, &item) / production;
        let mut priced = game.clone();
        if priced.apply(pid, &purchase).is_err() {
            continue;
        }
        let faith_cost = (bank - priced.players[pid].faith).max(0.0);
        if priced.players[pid].faith + f64::EPSILON < reserve {
            continue;
        }
        census.affordable_matches += 1;
        choices.push(PurchaseChoice {
            action: purchase,
            city: *city,
            district: *district,
            pos: *pos,
            family: district_family(game, *district).to_string(),
            faith_cost,
            turns_saved,
        });
    }
    choices
}

fn accelerate_one(
    game: &mut Game,
    pid: usize,
    observe_through: u32,
    oracle: &OracleTurn,
    state: &mut AccelerationState,
) -> Option<PurchaseRecord> {
    if !cadence_open(game.turn, pid, observe_through) || !state.attempted_turns.insert(game.turn) {
        return None;
    }
    state.census.cadence_checks += 1;
    let choice = purchase_candidates(game, pid, oracle, &mut state.census)
        .into_iter()
        .max_by(choice_order)?;
    match game.apply(pid, &choice.action) {
        Ok(()) => {
            state.census.purchases += 1;
            state.census.faith_spent += choice.faith_cost;
            state.census.projected_turns_saved += choice.turns_saved;
            *state.census.families.entry(choice.family).or_default() += 1;
            let record = PurchaseRecord {
                city: choice.city,
                district: choice.district,
                pos: choice.pos,
            };
            state.records.push(record.clone());
            Some(record)
        }
        Err(_) => {
            state.census.failed_applications += 1;
            None
        }
    }
}

fn is_faith_action(action: &Action) -> bool {
    match action {
        Action::Buy { currency, .. }
        | Action::BuyBuilding { currency, .. }
        | Action::BuyDistrict { currency, .. }
        | Action::PatronizeGreatPerson { currency, .. } => currency == "faith",
        _ => false,
    }
}

fn record_controller_followthrough(
    actions: &[(usize, Action)],
    pid: usize,
    purchase: &PurchaseRecord,
    state: &mut AccelerationState,
) {
    state.census.controller_turns_after_purchase += 1;
    for (owner, action) in actions {
        if *owner != pid || matches!(action, Action::EndTurn) {
            continue;
        }
        state.census.controller_actions_after_purchase += 1;
        if matches!(action, Action::Produce { city, .. } if *city == purchase.city) {
            state.census.new_production_in_purchase_city += 1;
        }
        state.census.subsequent_faith_actions += is_faith_action(action) as u32;
    }
}

fn count_surviving_purchases(game: &Game, pid: usize, records: &[PurchaseRecord]) -> u32 {
    records
        .iter()
        .filter(|record| {
            game.cities.get(&record.city).is_some_and(|city| {
                city.owner == pid
                    && city
                        .districts
                        .iter()
                        .any(|(district, pos)| *district == record.district && *pos == record.pos)
            })
        })
        .count() as u32
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
    faith: f64,
    cities: usize,
    districts: usize,
    buildings: usize,
    techs: usize,
    civics: usize,
    science_projects: usize,
    science_progress: f64,
    culture_lifetime: f64,
    tourism_lifetime: f64,
    military_power: f64,
    city_science: f64,
    city_culture: f64,
    city_production: f64,
    great_person_points: f64,
    great_people_claimed: i64,
    low_loyalty_cities: usize,
    lost_capital: bool,
    controller_plan: Option<String>,
    census: AccelerationCensus,
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
    let mut state = AccelerationState::default();

    while game.winner.is_none() && game.turn <= observe_through {
        assert_eq!(
            game.max_turns, policy_max_turns,
            "external continuation changed the policy-visible horizon"
        );
        let pid = game.current;
        if pid == focal && mode != Mode::Stock {
            let oracle = observe_stock_turn(&game, &ais[pid], pid);
            let purchase = (mode == Mode::Acceleration)
                .then(|| accelerate_one(&mut game, pid, observe_through, &oracle, &mut state))
                .flatten();
            let before = game.log.len();
            ais[pid].take_turn(&mut game, pid);
            if let Some(purchase) = purchase.as_ref() {
                let actions = game.log.since(before).cloned().collect::<Vec<_>>();
                record_controller_followthrough(&actions, pid, purchase, &mut state);
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
    state.census.purchased_surviving = count_surviving_purchases(&game, focal, &state.records);

    let player = &game.players[focal];
    let city_ids = game.player_city_ids(focal);
    let _memo = game.query_memo();
    let (city_science, city_culture, city_production) =
        city_ids.iter().fold((0.0, 0.0, 0.0), |total, city| {
            let yields = game.city_yields(*city);
            (
                total.0 + yields.science,
                total.1 + yields.culture,
                total.2 + yields.production,
            )
        });
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
        faith: player.faith,
        cities: city_ids.len(),
        districts: city_ids
            .iter()
            .map(|city| game.cities[city].districts.len())
            .sum(),
        buildings: city_ids
            .iter()
            .map(|city| game.cities[city].buildings.len())
            .sum(),
        techs: player.techs.len(),
        civics: player.civics.len(),
        science_projects,
        science_progress,
        culture_lifetime: player.culture_lifetime,
        tourism_lifetime: player.tourism_lifetime,
        military_power: game.military_power(focal),
        city_science,
        city_culture,
        city_production,
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
        controller_plan: ais[focal].plan_report().map(|plan| format!("{plan:?}")),
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
    faith: f64,
    cities: usize,
    districts: usize,
    buildings: usize,
    techs: usize,
    civics: usize,
    science_projects: usize,
    science_progress: f64,
    culture_lifetime: f64,
    tourism_lifetime: f64,
    military_power: f64,
    city_science: f64,
    city_culture: f64,
    city_production: f64,
    great_person_points: f64,
    great_people_claimed: i64,
    low_loyalty_cities: usize,
    lost_capitals: usize,
    cadence_checks: u64,
    stock_district_intentions: u64,
    exact_legal_matches: u64,
    affordable_matches: u64,
    purchases: u64,
    failed_applications: u64,
    controller_turns_after_purchase: u64,
    controller_actions_after_purchase: u64,
    new_production_in_purchase_city: u64,
    subsequent_faith_actions: u64,
    faith_spent: f64,
    projected_turns_saved: f64,
    purchased_surviving: u64,
    purchase_games: usize,
    families: BTreeMap<String, u64>,
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
        self.districts += result.districts;
        self.buildings += result.buildings;
        self.techs += result.techs;
        self.civics += result.civics;
        self.science_projects += result.science_projects;
        self.science_progress += result.science_progress;
        self.culture_lifetime += result.culture_lifetime;
        self.tourism_lifetime += result.tourism_lifetime;
        self.military_power += result.military_power;
        self.city_science += result.city_science;
        self.city_culture += result.city_culture;
        self.city_production += result.city_production;
        self.great_person_points += result.great_person_points;
        self.great_people_claimed += result.great_people_claimed;
        self.low_loyalty_cities += result.low_loyalty_cities;
        self.lost_capitals += result.lost_capital as usize;
        let census = &result.census;
        self.cadence_checks += census.cadence_checks as u64;
        self.stock_district_intentions += census.stock_district_intentions as u64;
        self.exact_legal_matches += census.exact_legal_matches as u64;
        self.affordable_matches += census.affordable_matches as u64;
        self.purchases += census.purchases as u64;
        self.failed_applications += census.failed_applications as u64;
        self.controller_turns_after_purchase += census.controller_turns_after_purchase as u64;
        self.controller_actions_after_purchase += census.controller_actions_after_purchase as u64;
        self.new_production_in_purchase_city += census.new_production_in_purchase_city as u64;
        self.subsequent_faith_actions += census.subsequent_faith_actions as u64;
        self.faith_spent += census.faith_spent;
        self.projected_turns_saved += census.projected_turns_saved;
        self.purchased_surviving += census.purchased_surviving as u64;
        self.purchase_games += (census.purchases > 0) as usize;
        for (family, count) in &census.families {
            *self.families.entry(family.clone()).or_default() += u64::from(*count);
        }
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
    purchase_games: usize,
    purchases: u64,
    failed_applications: u64,
    controller_turns_after_purchase: u64,
    purchased_surviving: u64,
    control_districts: usize,
    treatment_districts: usize,
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
    gate.purchases >= 8
        && gate.purchase_games >= 6
        && gate.failed_applications == 0
        && gate.controller_turns_after_purchase >= gate.purchases
        && gate.purchased_surviving >= 6
}

fn safety_passes(gate: GateInputs) -> bool {
    gate.treatment_districts >= gate.control_districts
        && gate.treatment_low_loyalty <= gate.control_low_loyalty
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

    println!("Adaptive Faith district acceleration evaluator");
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
            "NULL untreated one-turn oracle observation with acceleration disabled"
        } else {
            "one exact legal Divine Architect purchase for a stock-chosen district before normal controller play"
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
                Mode::NullOracle
            } else {
                Mode::Acceleration
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
        purchase_games: treatment.purchase_games,
        purchases: treatment.purchases,
        failed_applications: treatment.failed_applications,
        controller_turns_after_purchase: treatment.controller_turns_after_purchase,
        purchased_surviving: treatment.purchased_surviving,
        control_districts: control.districts,
        treatment_districts: treatment.districts,
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
        "arm        wins/games turns score faith cities districts buildings tech civic projects progress city-sci city-cult city-prod GPP claimed low-loy lost-cap military"
    );
    for (name, arm) in [("control", control), ("treatment", treatment)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{name:<10} {:>3}/{:<3} {:>5.1} {:>5.1} {:>7.1} {:>6.2} {:>9.2} {:>9.2} {:>4.1} {:>5.1} {:>8.2} {:>8.3} {:>8.1} {:>9.1} {:>9.1} {:>5.1} {:>7.1} {:>7} {:>8} {:>8.1}",
            arm.wins,
            arm.games,
            arm.turns as f64 / n,
            arm.score as f64 / n,
            arm.faith / n,
            arm.cities as f64 / n,
            arm.districts as f64 / n,
            arm.buildings as f64 / n,
            arm.techs as f64 / n,
            arm.civics as f64 / n,
            arm.science_projects as f64 / n,
            arm.science_progress / n,
            arm.city_science / n,
            arm.city_culture / n,
            arm.city_production / n,
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
        "terminal construction: districts {}->{}, buildings {}->{}, Faith {:.1}->{:.1}",
        control.districts,
        treatment.districts,
        control.buildings,
        treatment.buildings,
        control.faith / control.games.max(1) as f64,
        treatment.faith / treatment.games.max(1) as f64,
    );
    println!(
        "mechanism: cadence {}, stock district intentions {}, exact legal matches {}, affordable matches {}; purchases {} in {}/{} games, failures {}",
        treatment.cadence_checks,
        treatment.stock_district_intentions,
        treatment.exact_legal_matches,
        treatment.affordable_matches,
        treatment.purchases,
        treatment.purchase_games,
        treatment.games,
        treatment.failed_applications,
    );
    println!(
        "purchase accounting: families {:?}; Faith spent {:.1}, projected production turns saved {:.1}; controller turns/actions afterward {}/{}, new production in purchase city {}, subsequent Faith actions {}, purchased districts surviving {}",
        treatment.families,
        treatment.faith_spent,
        treatment.projected_turns_saved,
        treatment.controller_turns_after_purchase,
        treatment.controller_actions_after_purchase,
        treatment.new_production_in_purchase_city,
        treatment.subsequent_faith_actions,
        treatment.purchased_surviving,
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
    use civvis::game::GovernorState;

    fn plan(strategy: &'static str) -> PlanReport {
        PlanReport {
            strategy,
            victory_target: None,
            rush: false,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: 102,
            forces: Vec::new(),
        }
    }

    fn live_purchase_fixture() -> (Game, u32, Action) {
        let mut game = Game::new_full(1, 30, 20, 100_400, 200, 0, false);
        let settler_id = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("fixture needs its starting Settler");
        game.apply(0, &Action::FoundCity { unit: settler_id })
            .expect("starting position must found a capital");
        let city = game.player_city_ids(0)[0];
        game.turn = 102;
        game.cities.get_mut(&city).unwrap().pop = 20;
        game.players[0].techs = game.rules.techs.keys().copied().collect();
        game.players[0].civics = game.rules.civics.keys().copied().collect();
        game.players[0].faith = 100_000.0;
        game.players[0].governor_roster.insert(
            "moksha".to_string(),
            GovernorState {
                city: Some(city),
                assigned_turn: 0,
                disabled_until: 0,
                promotions: BTreeSet::from(["divine_architect".to_string()]),
            },
        );
        game.players[0].governors = vec![city];
        let queued = game
            .legal_actions_within(0, ActionFamilies::PURCHASES)
            .into_iter()
            .find(|action| {
                matches!(
                    action,
                    Action::Produce {
                        city: candidate,
                        item: Item::District { .. },
                    } if *candidate == city
                )
            })
            .expect("fixture needs one legal district production item");
        let Action::Produce {
            item:
                Item::District {
                    district: queued_district,
                    pos: queued_pos,
                },
            ..
        } = &queued
        else {
            unreachable!()
        };
        game.apply(0, &queued)
            .expect("fixture should begin with a legal district queue");
        let purchase = game
            .legal_actions_within(0, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE)
            .into_iter()
            .find(|action| {
                matches!(
                    action,
                    Action::BuyDistrict {
                        district,
                        pos,
                        currency,
                        ..
                    } if currency == "faith"
                        && (*district != *queued_district || *pos != *queued_pos)
                )
            })
            .expect("established Divine Architect must expose a real Faith district action");
        (game, city, purchase)
    }

    fn oracle_for(purchase: &Action, strategy: &'static str) -> OracleTurn {
        let Action::BuyDistrict {
            city,
            district,
            pos,
            ..
        } = purchase
        else {
            panic!("fixture action must be a district purchase")
        };
        OracleTurn {
            actions: vec![(
                0,
                Action::Produce {
                    city: *city,
                    item: Item::District {
                        district: *district,
                        pos: *pos,
                    },
                },
            )],
            plan: Some(plan(strategy)),
            won: false,
        }
    }

    fn passing_gate() -> GateInputs {
        GateInputs {
            purchase_games: 6,
            purchases: 8,
            failed_applications: 0,
            controller_turns_after_purchase: 8,
            purchased_surviving: 6,
            control_districts: 40,
            treatment_districts: 40,
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
    fn reserves_and_candidate_order_are_exact() {
        assert_eq!(faith_reserve_from("religion", true, 900.0, true), 180.0);
        assert_eq!(faith_reserve_from("culture", true, 900.0, true), 900.0);
        assert_eq!(faith_reserve_from("culture", false, 900.0, true), 700.0);
        assert_eq!(faith_reserve_from("science", true, 900.0, true), 80.0);

        let choice = |city, turns_saved| PurchaseChoice {
            action: Action::EndTurn,
            city,
            district: Name::new("campus"),
            pos: (city as i32, 0),
            family: "campus".to_string(),
            faith_cost: 100.0,
            turns_saved,
        };
        assert!(choice_order(&choice(9, 11.0), &choice(1, 10.0)).is_gt());
        assert!(choice_order(&choice(3, 10.0), &choice(4, 10.0)).is_gt());
        assert!(choice_order(&choice(4, 10.0), &choice(3, 10.0)).is_lt());
    }

    #[test]
    fn cadence_and_final_value_window_are_exact() {
        assert!(cadence_open(102, 0, 320));
        assert!(!cadence_open(103, 0, 320));
        assert!(cadence_open(103, 1, 320));
        assert!(cadence_open(300, 0, 320));
        assert!(!cadence_open(301, 0, 320));
        assert!(!cadence_open(306, 0, 320));
    }

    #[test]
    fn eligibility_requires_live_divine_architect_and_exact_stock_produce() {
        let (game, _, purchase) = live_purchase_fixture();
        let oracle = oracle_for(&purchase, "science");
        let mut census = AccelerationCensus::default();
        assert_eq!(purchase_candidates(&game, 0, &oracle, &mut census).len(), 1);
        assert_eq!(census.stock_district_intentions, 1);
        assert_eq!(census.exact_legal_matches, 1);
        assert_eq!(census.affordable_matches, 1);

        let mut no_moksha = game.clone();
        no_moksha.players[0].governor_roster.clear();
        no_moksha.players[0].governors.clear();
        assert!(!no_moksha
            .legal_actions_within(0, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE)
            .iter()
            .any(|action| matches!(action, Action::BuyDistrict { currency, .. } if currency == "faith")));

        let synthetic_purchase = OracleTurn {
            actions: vec![(0, purchase.clone())],
            plan: Some(plan("science")),
            won: false,
        };
        assert!(purchase_candidates(
            &game,
            0,
            &synthetic_purchase,
            &mut AccelerationCensus::default()
        )
        .is_empty());

        let mut mismatch = oracle.clone();
        mismatch.actions = vec![(
            0,
            Action::Produce {
                city: game.player_city_ids(0)[0],
                item: Item::District {
                    district: Name::new("not_a_district"),
                    pos: (0, 0),
                },
            },
        )];
        assert!(
            purchase_candidates(&game, 0, &mismatch, &mut AccelerationCensus::default()).is_empty()
        );
        let mut won = oracle;
        won.won = true;
        assert!(purchase_candidates(&game, 0, &won, &mut AccelerationCensus::default()).is_empty());
    }

    #[test]
    fn exact_match_rejects_gold_and_every_mismatched_field() {
        let (_, _, purchase) = live_purchase_fixture();
        let Action::BuyDistrict {
            city,
            district,
            pos,
            ..
        } = purchase
        else {
            unreachable!()
        };
        let action = |city, district, pos, currency: &str| Action::BuyDistrict {
            city,
            district,
            pos,
            currency: currency.to_string(),
        };
        assert!(
            exact_faith_purchase(&[action(city, district, pos, "faith")], city, district, pos)
                .is_some()
        );
        for mismatch in [
            action(city, district, pos, "gold"),
            action(city + 1, district, pos, "faith"),
            action(city, Name::new("not_the_district"), pos, "faith"),
            action(city, district, (pos.0 + 1, pos.1), "faith"),
        ] {
            assert!(exact_faith_purchase(&[mismatch], city, district, pos).is_none());
        }
    }

    #[test]
    fn treatment_applies_one_real_faith_purchase_without_touching_queue() {
        let (mut game, city, purchase) = live_purchase_fixture();
        let oracle = oracle_for(&purchase, "science");
        let faith_before = game.players[0].faith;
        let queue_before = game.cities[&city].queue.clone();
        let mut state = AccelerationState::default();
        let record = accelerate_one(&mut game, 0, 320, &oracle, &mut state)
            .expect("exact live treatment must buy the district");
        assert!(accelerate_one(&mut game, 0, 320, &oracle, &mut state).is_none());
        assert_eq!(state.census.purchases, 1);
        assert_eq!(state.census.failed_applications, 0);
        assert!(state.census.faith_spent > 0.0);
        assert_eq!(
            game.players[0].faith,
            faith_before - state.census.faith_spent
        );
        assert_eq!(game.cities[&city].queue, queue_before);
        assert!(game.cities[&city]
            .districts
            .iter()
            .any(|(district, pos)| *district == record.district && *pos == record.pos));
        assert_eq!(count_surviving_purchases(&game, 0, &state.records), 1);

        let mut controller = AdvancedAi::new();
        let before = game.log.len();
        controller.take_turn(&mut game, 0);
        let actions = game.log.since(before).cloned().collect::<Vec<_>>();
        record_controller_followthrough(&actions, 0, &record, &mut state);
        assert_eq!(state.census.controller_turns_after_purchase, 1);
    }

    #[test]
    fn followthrough_counts_only_actual_controller_actions() {
        let record = PurchaseRecord {
            city: 7,
            district: Name::new("campus"),
            pos: (1, 2),
        };
        let actions = vec![
            (
                0,
                Action::Produce {
                    city: 7,
                    item: Item::Building {
                        building: Name::new("monument"),
                    },
                },
            ),
            (
                0,
                Action::BuyBuilding {
                    city: 7,
                    building: Name::new("granary"),
                    currency: "faith".to_string(),
                },
            ),
            (
                0,
                Action::BuyDistrict {
                    city: 7,
                    district: Name::new("holy_site"),
                    pos: (2, 2),
                    currency: "gold".to_string(),
                },
            ),
            (
                1,
                Action::BuyBuilding {
                    city: 8,
                    building: Name::new("granary"),
                    currency: "faith".to_string(),
                },
            ),
            (0, Action::EndTurn),
        ];
        let mut state = AccelerationState::default();
        record_controller_followthrough(&actions, 0, &record, &mut state);
        assert_eq!(state.census.controller_turns_after_purchase, 1);
        assert_eq!(state.census.controller_actions_after_purchase, 3);
        assert_eq!(state.census.new_production_in_purchase_city, 1);
        assert_eq!(state.census.subsequent_faith_actions, 1);
    }

    #[test]
    fn null_oracle_observation_preserves_stock_world_and_controller() {
        let mut stock = Game::new(2, 20, 14, 100_401, 20, 0);
        stock.set_fog_memory(false);
        let mut observed = stock.clone();
        let mut stock_ai = AdvancedAi::new();
        let mut observed_ai = stock_ai.clone();
        let before_observation = serde_json::to_string(&observed).unwrap();
        let oracle = observe_stock_turn(&observed, &observed_ai, 0);
        assert_eq!(
            serde_json::to_string(&observed).unwrap(),
            before_observation
        );

        let stock_before = stock.log.len();
        stock_ai.take_turn(&mut stock, 0);
        observed_ai.take_turn(&mut observed, 0);
        let stock_turn_actions = stock.log.since(stock_before).cloned().collect::<Vec<_>>();
        if stock.winner.is_none() && stock.current == 0 {
            stock.apply(0, &Action::EndTurn).unwrap();
        }
        if observed.winner.is_none() && observed.current == 0 {
            observed.apply(0, &Action::EndTurn).unwrap();
        }
        assert_eq!(oracle.actions, stock_turn_actions);
        assert_eq!(
            serde_json::to_string(&observed).unwrap(),
            serde_json::to_string(&stock).unwrap()
        );
        assert_eq!(observed_ai.plan_report(), stock_ai.plan_report());
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
            purchases: 7,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            purchase_games: 5,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            failed_applications: 1,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            controller_turns_after_purchase: 7,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            purchased_surviving: 5,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_districts: 39,
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
