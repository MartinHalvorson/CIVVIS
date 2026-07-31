//! Matched evaluation of adaptive Science Spaceport parallelism.
//!
//! The treatment changes neither Production nor strategy. On a focal adaptive
//! Science turn after the Moon or Mars milestone, it orders at most one legal
//! Spaceport until the explicit-Science policy's two/three-site target is met.
//! Two focal seats are paired within every map; the map is the inference unit.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::evolve::Champion;
use civvis::game::{Action, Game, GameOptions, Item, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use civvis::Pos;
use std::collections::BTreeMap;

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 9_982_000;
const SCREEN_MAPS: usize = 30;
const SCREEN_SEED: u64 = 9_983_000;
const HOLDOUT_MAPS: usize = 120;
const HOLDOUT_SEED: u64 = 9_984_000;
const NOMINAL_TURNS: u32 = 250;
const OBSERVE_THROUGH: u32 = 320;
const FROZEN_AI: &str = "advanced_evolved";
const FROZEN_CHAMPION_GENERATION: u32 = 14;
/// Fingerprint of `data/evolved/best.json`, re-pinned 2026-07-31.
///
/// The Spaceport preregistration froze `advanced_evolved` so its screens
/// stayed interpretable, and it reached its own **STOP**: the registered
/// result retains stock `AdvancedAi`, the primary endpoint moved
/// +0.117 points per map against a required +0.5 at `p = 1.0000`, and seed
/// 9,984,000 and the whole 120-map holdout were left unopened by protocol.
/// Those numbers were measured on the gen-14 champion as it stood then and
/// this re-pin does not revise them.
///
/// The champion has since been replaced deliberately — the same genome with
/// `docs/GENOME.md`'s eleven economy and expansion genes reverted to
/// `Weights::default()`, promoted on three `ai_eval --matrix` runs at 300 maps
/// per profile, all `PASS`. See `docs/EVAL.md`.
///
/// ⚠ Any future run of this evaluator is therefore on a different agent than
/// the registered one, and a new number must say which champion it ran
/// against. The reserved holdout was closed by the STOP, not by this change.
const FROZEN_CHAMPION_FNV1A: u64 = 0x31cd_12c3_a1ba_5302;
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
const FLAG_OPTIONS: [&str; 3] = ["--null", "--deployment-mix", "--randomize-civs"];
const VALUE_OPTIONS: [&str; 16] = [
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
    "--shapes",
    "--poles",
    "--victories",
    "--ai",
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
        "data/evolved/best.json changed after the Spaceport preregistration"
    );
    let champion: Champion = serde_json::from_str(EMBEDDED_CHAMPION)
        .expect("the committed advanced_evolved champion must be valid JSON");
    assert_eq!(
        champion.gen, FROZEN_CHAMPION_GENERATION,
        "Spaceport evaluator champion generation changed"
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

fn topology_schedule(args: &[String]) -> Result<Vec<MapTopology>, String> {
    let has_shape = args.iter().any(|arg| arg == "--shape");
    let has_shapes = args.iter().any(|arg| arg == "--shapes");
    if has_shape && has_shapes {
        return Err("choose either --shape or --shapes, not both".to_string());
    }
    let names = if has_shapes {
        text(args, "--shapes", "")
    } else if has_shape {
        text(args, "--shape", "")
    } else {
        DEPLOYMENT_TOPOLOGIES
            .iter()
            .map(|topology| topology.id())
            .collect::<Vec<_>>()
            .join(",")
    };
    let topologies: Vec<MapTopology> = names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| MapTopology::from_id(name).ok_or_else(|| format!("unknown map shape {name:?}")))
        .collect::<Result<_, _>>()?;
    if topologies.is_empty() {
        return Err("--shapes must name at least one topology".to_string());
    }
    Ok(topologies)
}

fn topology_for(map: usize, schedule: &[MapTopology]) -> MapTopology {
    schedule[map % schedule.len()]
}

fn topology_counts(maps: usize, schedule: &[MapTopology]) -> Vec<(MapTopology, usize)> {
    let mut counts = Vec::new();
    for map in 0..maps {
        let topology = topology_for(map, schedule);
        if let Some((_, count)) = counts.iter_mut().find(|(seen, _)| *seen == topology) {
            *count += 1;
        } else {
            counts.push((topology, 1));
        }
    }
    counts
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Milestone {
    Moon,
    Mars,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SiteCandidate {
    production: f64,
    city: u32,
    pos: Pos,
}

fn choose_site(candidates: impl IntoIterator<Item = SiteCandidate>) -> Option<SiteCandidate> {
    candidates.into_iter().fold(None, |best, candidate| {
        if best.is_none_or(|old| {
            candidate.production > old.production
                || (candidate.production == old.production
                    && (candidate.city, candidate.pos) < (old.city, old.pos))
        }) {
            Some(candidate)
        } else {
            best
        }
    })
}

fn desired_spaceports(
    strategy: Option<&str>,
    completed: &std::collections::BTreeSet<String>,
) -> Option<(usize, Milestone)> {
    if strategy != Some("science") {
        return None;
    }
    if completed.contains("launch_mars_colony") {
        Some((3, Milestone::Mars))
    } else if completed.contains("launch_moon_landing") {
        Some((2, Milestone::Moon))
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
struct SpaceportOrder {
    milestone: Milestone,
    action: Action,
}

/// Select one actually producible site with the same city/site ordering as the
/// shipped explicit-Science branch. Existing and first-queued sites count once
/// per city, so a queue cannot manufacture fake parallelism.
fn spaceport_order(g: &Game, pid: usize, strategy: Option<&str>) -> Option<SpaceportOrder> {
    if !g.victory_conditions.science {
        return None;
    }
    let (desired, milestone) = desired_spaceports(strategy, &g.players[pid].science_projects)?;
    let city_ids = g.player_city_ids(pid);
    let built = city_ids
        .iter()
        .filter(|city| {
            g.cities[city]
                .districts
                .contains_key(civvis::name!("spaceport"))
        })
        .count();
    let queued = city_ids
        .iter()
        .filter(|city| {
            matches!(
                g.cities[city].queue.first(),
                Some(Item::District { district, .. }) if district == "spaceport"
            )
        })
        .count();
    if built + queued >= desired.min(city_ids.len()) {
        return None;
    }

    let candidates = city_ids.into_iter().flat_map(|city| {
        let ineligible = g.cities[&city]
            .districts
            .contains_key(civvis::name!("spaceport"))
            || matches!(
                g.cities[&city].queue.first(),
                Some(Item::District { district, .. }) if district == "spaceport"
            );
        let production = g.city_yields(city).production;
        g.producible_items(pid, city)
            .into_iter()
            .filter_map(move |item| {
                let Item::District { district, pos } = item else {
                    return None;
                };
                (!ineligible && district == "spaceport").then_some(SiteCandidate {
                    production,
                    city,
                    pos,
                })
            })
    });
    let site = choose_site(candidates)?;
    Some(SpaceportOrder {
        milestone,
        action: Action::Produce {
            city: site.city,
            item: Item::District {
                district: civvis::name!("spaceport"),
                pos: site.pos,
            },
        },
    })
}

/// Run the shipped controller, retain its internal state, and replay every
/// successful action except its final `EndTurn`. This opens a genuine
/// post-policy, pre-turn-boundary treatment point without changing the AI.
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TreatmentCensus {
    science_plan_turns: u32,
    milestone_turns: u32,
    opportunities: u32,
    orders: u32,
    moon_orders: u32,
    mars_orders: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    science_progress: f64,
    science_projects: usize,
    exoplanet_distance: f64,
    exoplanet_speed: f64,
    lasers: i64,
    built_spaceports: usize,
    queued_spaceports: usize,
    census: TreatmentCensus,
    serialized_world: Option<Vec<u8>>,
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
            let strategy = ais[pid].strategy_label();
            census.science_plan_turns += (strategy == Some("science")) as u32;
            if desired_spaceports(strategy, &game.players[pid].science_projects).is_some() {
                census.milestone_turns += 1;
            }
            if let Some(order) = spaceport_order(&game, pid, strategy) {
                census.opportunities += 1;
                game.apply(pid, &order.action).unwrap_or_else(|why| {
                    panic!(
                        "turn {} seat {pid}: selected Spaceport order became illegal: {why}; {:?}",
                        game.turn, order.action
                    )
                });
                census.orders += 1;
                match order.milestone {
                    Milestone::Moon => census.moon_orders += 1,
                    Milestone::Mars => census.mars_orders += 1,
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

    let city_ids = game.player_city_ids(focal);
    let built_spaceports = city_ids
        .iter()
        .filter(|city| {
            game.cities[city]
                .districts
                .contains_key(civvis::name!("spaceport"))
        })
        .count();
    let queued_spaceports = city_ids
        .iter()
        .filter(|city| {
            matches!(
                game.cities[city].queue.first(),
                Some(Item::District { district, .. }) if district == "spaceport"
            )
        })
        .count();
    let player = &game.players[focal];
    let lasers = player
        .counters
        .get("project:lagrange_laser_station")
        .copied()
        .unwrap_or(0)
        + player
            .counters
            .get("project:terrestrial_laser_station")
            .copied()
            .unwrap_or(0);
    let race = game.victory_races(focal, 0);

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
        science_progress: race.science,
        science_projects: race.science_projects,
        exoplanet_distance: player.exoplanet_distance,
        exoplanet_speed: game.exoplanet_speed(focal),
        lasers,
        built_spaceports,
        queued_spaceports,
        census,
        serialized_world: capture_world.then(|| {
            serde_json::to_vec(&game).expect("terminal Game must serialize for the replay null")
        }),
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
    science_wins: usize,
    turns: u64,
    score: i64,
    science_progress: f64,
    science_projects: usize,
    exoplanet_distance: f64,
    exoplanet_speed: f64,
    lasers: i64,
    built_spaceports: usize,
    queued_spaceports: usize,
    science_plan_turns: u64,
    milestone_turns: u64,
    opportunities: u64,
    orders: u64,
    moon_orders: u64,
    mars_orders: u64,
    fired_games: usize,
    victories: BTreeMap<String, usize>,
}

impl ArmSummary {
    fn record(&mut self, result: &GameResult) {
        self.games += 1;
        self.wins += result.won as usize;
        self.science_wins += (result.victory.as_deref() == Some("science")) as usize;
        self.turns += result.reported_turn as u64;
        self.score += result.score;
        self.science_progress += result.science_progress;
        self.science_projects += result.science_projects;
        self.exoplanet_distance += result.exoplanet_distance;
        self.exoplanet_speed += result.exoplanet_speed;
        self.lasers += result.lasers;
        self.built_spaceports += result.built_spaceports;
        self.queued_spaceports += result.queued_spaceports;
        self.science_plan_turns += result.census.science_plan_turns as u64;
        self.milestone_turns += result.census.milestone_turns as u64;
        self.opportunities += result.census.opportunities as u64;
        self.orders += result.census.orders as u64;
        self.moon_orders += result.census.moon_orders as u64;
        self.mars_orders += result.census.mars_orders as u64;
        self.fired_games += (result.census.orders > 0) as usize;
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

    fn spaceports(&self) -> usize {
        self.built_spaceports + self.queued_spaceports
    }
}

#[derive(Default)]
struct StratumSummary {
    maps: usize,
    control: ArmSummary,
    comparison: ArmSummary,
    science_delta: f64,
    science_favorable: usize,
    science_adverse: usize,
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
            .map(|(old, new)| new.science_progress - old.science_progress)
            .sum::<f64>()
            / 2.0;
        self.science_delta += delta;
        if delta > 1e-9 {
            self.science_favorable += 1;
        } else if delta < -1e-9 {
            self.science_adverse += 1;
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
        "  {label:<22} {:>3} maps; fired {}/{} ({:.1}%), {} orders; spaceports {}->{}, lasers {}->{}; Science delta {:+.3} (F/N/A {}/{}/{}); wins {}->{} (Science {}->{}); map win {:.1}%, score share {:.2}%",
        summary.maps,
        summary.comparison.fired_games,
        summary.comparison.games,
        100.0 * summary.comparison.fired_games as f64
            / summary.comparison.games.max(1) as f64,
        summary.comparison.orders,
        summary.control.spaceports(),
        summary.comparison.spaceports(),
        summary.control.lasers,
        summary.comparison.lasers,
        summary.science_delta / maps as f64,
        summary.science_favorable,
        maps - summary.science_favorable - summary.science_adverse,
        summary.science_adverse,
        summary.control.wins,
        summary.comparison.wins,
        summary.control.science_wins,
        summary.comparison.science_wins,
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
    coverage: f64,
    orders: u64,
    treatment_spaceports: usize,
    control_spaceports: usize,
    treatment_lasers: i64,
    control_lasers: i64,
    science_delta: f64,
    science_favorable: usize,
    science_adverse: usize,
    science_p: f64,
    treatment_science_wins: usize,
    control_science_wins: usize,
    treatment_wins: usize,
    control_wins: usize,
    paired_win_score: f64,
    terminal_score_share: f64,
}

fn mechanism_passes(gate: GateInputs) -> bool {
    gate.coverage >= 0.10
        && gate.orders >= 10
        && gate.treatment_spaceports > gate.control_spaceports
        && gate.treatment_lasers > gate.control_lasers
}

fn outcome_guards_pass(gate: GateInputs, score_floor: f64) -> bool {
    gate.treatment_science_wins >= gate.control_science_wins
        && gate.treatment_wins >= gate.control_wins
        && gate.paired_win_score >= 0.50
        && gate.terminal_score_share >= score_floor
}

fn screen_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate)
        && gate.science_delta >= 0.5
        && gate.science_favorable > gate.science_adverse
        && gate.science_p <= 0.20
        && outcome_guards_pass(gate, 0.495)
}

fn holdout_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate)
        && gate.science_delta > 0.0
        && gate.science_favorable > gate.science_adverse
        && gate.science_p < 0.05
        && outcome_guards_pass(gate, 0.50)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(why) = validate_args(&args) {
        eprintln!("{why}");
        std::process::exit(2);
    }
    let null_replay = has_arg(&args, "--null");
    let deployment_mix = has_arg(&args, "--deployment-mix");
    let explicit_frozen_ai = has_exact_value(&args, "--ai", FROZEN_AI);
    let ai_name = text(&args, "--ai", FROZEN_AI);
    if has_arg(&args, "--ai") && !explicit_frozen_ai {
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
    let map_topologies = if deployment_mix {
        DEPLOYMENT_TOPOLOGIES.to_vec()
    } else {
        topology_schedule(&args).unwrap_or_else(|why| {
            eprintln!("{why}");
            std::process::exit(2);
        })
    };
    let poles_name = text(&args, "--poles", "poles");
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

    let civilizations = if randomize_civs {
        "randomized"
    } else {
        "fixed"
    };
    let victory_profile = VictoryConditions::NAMES
        .into_iter()
        .filter(|name| victories.is_enabled(name))
        .collect::<Vec<_>>()
        .join(",");
    println!("Adaptive Science Spaceport parallelism evaluator");
    println!(
        "controller: {ai_name}; embedded champion generation {}, FNV-1a {:#018x}",
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
        let size_profile = DEPLOYMENT_PLAYERS
            .iter()
            .map(|players| {
                let size = MapSize::for_players(*players);
                let globe = size.dimensions(MapTopology::Planet);
                format!(
                    "{players}p={}x{}/{}x{}+{}cs",
                    size.width, size.height, globe.0, globe.1, size.default_city_states
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "profile: deployment mix; players {player_batch}; scripts {script_batch}; topologies {topology_batch}"
        );
        println!("derived Flat/Planet size rows: {size_profile}");
    } else {
        let topology_profile = map_topologies
            .iter()
            .map(|topology| {
                let stored = MapSize::from_dimensions(width, height)
                    .map(|size| size.dimensions(*topology))
                    .unwrap_or((width, height));
                format!("{}={}x{}", topology.id(), stored.0, stored.1)
            })
            .collect::<Vec<_>>()
            .join(",");
        let topology_batch = topology_counts(maps, &map_topologies)
            .into_iter()
            .map(|(topology, count)| format!("{}={count}", topology.id()))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "profile: diagnostic fixed cell: {players}p requested {width}x{height}, stored {topology_profile}, \
             {city_states} city-states, map {}, topology schedule {topology_batch}",
            map_script.id(),
        );
    }
    println!(
        "rules: {nominal_turns} policy-visible {speed} turns, observe through {observe_through}, poles {}, \
         civilizations {civilizations}, victories {victory_profile}",
        map_poles.id(),
    );
    println!(
        "batch: {maps} independent maps x seats 0/final x control/comparison = {} games; seed {seed}; {jobs} jobs",
        maps * 4
    );
    println!(
        "comparison: {}",
        if null_replay {
            "NULL action-log replay with no added order"
        } else {
            "adaptive Science plan uses the explicit target's 2/3-Spaceport schedule"
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
    let mut paired_win_scores = Vec::with_capacity(maps);
    let mut paired_terminal_scores = Vec::with_capacity(maps);
    let mut science_deltas = Vec::with_capacity(maps);
    let mut science_favorable = 0usize;
    let mut science_adverse = 0usize;
    let mut win_favorable = 0usize;
    let mut win_adverse = 0usize;
    let mut helped_cells = 0usize;
    let mut hurt_cells = 0usize;
    let mut exact_mismatches = 0usize;

    for result in &results {
        let control_wins = result.control.iter().filter(|game| game.won).count();
        let comparison_wins = result.comparison.iter().filter(|game| game.won).count();
        paired_win_scores.push(map_win_score(control_wins, comparison_wins));
        match comparison_wins.cmp(&control_wins) {
            std::cmp::Ordering::Greater => win_favorable += 1,
            std::cmp::Ordering::Less => win_adverse += 1,
            std::cmp::Ordering::Equal => {}
        }

        paired_terminal_scores.push(
            result
                .control
                .iter()
                .zip(&result.comparison)
                .map(|(old, new)| terminal_share(old.score, new.score))
                .sum::<f64>()
                / 2.0,
        );
        let science_delta = result
            .control
            .iter()
            .zip(&result.comparison)
            .map(|(old, new)| new.science_progress - old.science_progress)
            .sum::<f64>()
            / 2.0;
        science_deltas.push(science_delta);
        if science_delta > 1e-9 {
            science_favorable += 1;
        } else if science_delta < -1e-9 {
            science_adverse += 1;
        }

        for (old, new) in result.control.iter().zip(&result.comparison) {
            control.record(old);
            comparison.record(new);
            match (old.won, new.won) {
                (false, true) => helped_cells += 1,
                (true, false) => hurt_cells += 1,
                _ => {}
            }
            exact_mismatches += (old != new) as usize;
        }
    }

    let paired_win_score = paired_win_scores.iter().sum::<f64>() / maps as f64;
    let terminal_score_share = paired_terminal_scores.iter().sum::<f64>() / maps as f64;
    let science_delta = science_deltas.iter().sum::<f64>() / maps as f64;
    let science_p = exact_two_sided(science_favorable, science_favorable + science_adverse);
    let win_p = exact_two_sided(win_favorable, win_favorable + win_adverse);
    let coverage = comparison.fired_games as f64 / comparison.games.max(1) as f64;
    let gate = GateInputs {
        coverage,
        orders: comparison.orders,
        treatment_spaceports: comparison.spaceports(),
        control_spaceports: control.spaceports(),
        treatment_lasers: comparison.lasers,
        control_lasers: control.lasers,
        science_delta,
        science_favorable,
        science_adverse,
        science_p,
        treatment_science_wins: comparison.science_wins,
        control_science_wins: control.science_wins,
        treatment_wins: comparison.wins,
        control_wins: control.wins,
        paired_win_score,
        terminal_score_share,
    };

    println!();
    println!(
        "arm         wins  sci-wins  turns  score  science  projects  distance  speed  lasers  spaceports"
    );
    for (name, arm) in [("control", &control), ("comparison", &comparison)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{name:<11} {:>3}/{:<3} {:>3}/{:<3} {:>6.1} {:>6.1} {:>8.2} {:>9.2} {:>9.2} {:>6.2} {:>7} {:>5}+{:<5}",
            arm.wins,
            arm.games,
            arm.science_wins,
            arm.games,
            arm.turns as f64 / n,
            arm.score as f64 / n,
            arm.science_progress / n,
            arm.science_projects as f64 / n,
            arm.exoplanet_distance / n,
            arm.exoplanet_speed / n,
            arm.lasers,
            arm.built_spaceports,
            arm.queued_spaceports,
        );
    }
    println!(
        "victory types: control {:?}; comparison {:?}",
        control.victories, comparison.victories
    );
    println!(
        "treatment mechanism: {}/{} focal games fired ({:.1}%); {} opportunities, {} successful orders ({} after Moon, {} after Mars); {} Science-plan turns, {} milestone turns",
        comparison.fired_games,
        comparison.games,
        100.0 * coverage,
        comparison.opportunities,
        comparison.orders,
        comparison.moon_orders,
        comparison.mars_orders,
        comparison.science_plan_turns,
        comparison.milestone_turns,
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
        "primary Science delta: {science_delta:+.3} points/map; favorable {science_favorable}, neutral {}, adverse {science_adverse}; exact p={science_p:.4}",
        maps - science_favorable - science_adverse,
    );
    println!(
        "paired terminal-score share: {:.2}%",
        100.0 * terminal_score_share
    );

    println!("deployment-axis summaries (descriptive only; the decision gate is pooled):");
    for players in axis_values(&results, |profile| profile.players) {
        let summary = summarize_stratum(
            results
                .iter()
                .filter(|result| result.profile.players == players),
        );
        print_stratum(&format!("players={players}"), &summary);
    }
    for script in axis_values(&results, |profile| profile.map_script) {
        let summary = summarize_stratum(
            results
                .iter()
                .filter(|result| result.profile.map_script == script),
        );
        print_stratum(&format!("map={}", script.id()), &summary);
    }
    for topology in axis_values(&results, |profile| profile.map_topology) {
        let summary = summarize_stratum(
            results
                .iter()
                .filter(|result| result.profile.map_topology == topology),
        );
        print_stratum(&format!("shape={}", topology.id()), &summary);
    }

    let exact_profile = deployment_mix
        && has_exact_flag(&args, "--deployment-mix")
        && explicit_frozen_ai
        && ai_name == FROZEN_AI
        && has_exact_number(&args, "--turns", NOMINAL_TURNS as i64)
        && nominal_turns == NOMINAL_TURNS
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
                "frozen null gate: PASS — all {} direct/replay serialized worlds and results reproduced exactly",
                control.games
            );
        } else {
            println!(
                "diagnostic null sanity: PASS — all {} direct/replay serialized worlds and results reproduced exactly",
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
                "STOP — retain AdvancedAi; do not tune, retry, or inspect the holdout"
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

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
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
    fn formal_flags_require_one_canonical_raw_value() {
        let args = [
            "--ai".to_string(),
            FROZEN_AI.to_string(),
            "--turns".to_string(),
            "250".to_string(),
            "--deployment-mix".to_string(),
        ];
        assert!(has_exact_value(&args, "--ai", FROZEN_AI));
        assert!(has_exact_number(&args, "--turns", 250));
        assert!(has_exact_flag(&args, "--deployment-mix"));
        assert!(!has_exact_number(
            &["--turns".to_string(), "0250".to_string()],
            "--turns",
            250
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
        assert!(!has_exact_flag(
            &["--null".to_string(), "--null".to_string()],
            "--null"
        ));
    }

    fn science_fixture() -> Game {
        let mut game = Game::new_full(1, 34, 20, 79_200, 320, 0, false);
        game.victory_conditions = VictoryConditions {
            science: true,
            culture: true,
            religious: false,
            diplomatic: false,
            domination: true,
            score: false,
        };
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let cities = game.player_city_ids(0);
        game.players[0].techs = game.rules.techs.keys().cloned().collect();
        game.players[0].civics = game.rules.civics.keys().cloned().collect();
        game.players[0].science_projects.extend([
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
        ]);
        for city in &cities {
            game.cities.get_mut(city).unwrap().pop = 12;
            for position in game.cities[city].owned_tiles.clone() {
                if position == game.cities[city].pos {
                    continue;
                }
                let tile = game.map.tiles.get_mut(&position).unwrap();
                tile.terrain = civvis::name!("plains");
                tile.feature = None;
                tile.hills = false;
                tile.resource = None;
                tile.improvement = None;
                tile.district = None;
                tile.district_foundation = None;
                tile.wonder = None;
            }
        }
        game
    }

    #[test]
    fn deployment_cycle_is_factorial_and_balances_every_frozen_batch() {
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
                (6, 74, 46, 9, MapScript::WaterWorld, MapTopology::Planet,),
                (8, 84, 54, 12, MapScript::Continents, MapTopology::Flat,),
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
                "deployment profile repeated before the full cycle at offset {index}: {profile:?}"
            );
        }
        assert_eq!(deployment_profile(126), deployment_profile(0));

        assert_eq!(
            deployment_counts(SCREEN_MAPS, |profile| profile.players),
            vec![(4, 5), (6, 5), (8, 4), (10, 4), (5, 4), (7, 4), (9, 4)]
        );
        assert_eq!(
            deployment_counts(HOLDOUT_MAPS, |profile| profile.players),
            vec![
                (4, 18),
                (6, 17),
                (8, 17),
                (10, 17),
                (5, 17),
                (7, 17),
                (9, 17),
            ]
        );
        assert_eq!(
            deployment_counts(SCREEN_MAPS, |profile| profile.map_script),
            vec![
                (MapScript::LandOnly, 4),
                (MapScript::WaterWorld, 4),
                (MapScript::Continents, 4),
                (MapScript::TrueStartEarth, 3),
                (MapScript::Lakes, 3),
                (MapScript::InlandSea, 3),
                (MapScript::Pangaea, 3),
                (MapScript::SmallContinents, 3),
                (MapScript::Islands, 3),
            ]
        );
        assert_eq!(
            deployment_counts(HOLDOUT_MAPS, |profile| profile.map_script),
            vec![
                (MapScript::LandOnly, 14),
                (MapScript::WaterWorld, 14),
                (MapScript::Continents, 14),
                (MapScript::TrueStartEarth, 13),
                (MapScript::Lakes, 13),
                (MapScript::InlandSea, 13),
                (MapScript::Pangaea, 13),
                (MapScript::SmallContinents, 13),
                (MapScript::Islands, 13),
            ]
        );
        assert_eq!(
            deployment_counts(NULL_MAPS, |profile| profile.map_topology),
            vec![(MapTopology::Flat, 2), (MapTopology::Planet, 2)]
        );
        assert_eq!(
            deployment_counts(SCREEN_MAPS, |profile| profile.map_topology),
            vec![(MapTopology::Flat, 15), (MapTopology::Planet, 15)]
        );
        assert_eq!(
            deployment_counts(HOLDOUT_MAPS, |profile| profile.map_topology),
            vec![(MapTopology::Flat, 60), (MapTopology::Planet, 60)]
        );
    }

    #[test]
    fn diagnostic_topology_flags_remain_explicit() {
        let schedule = topology_schedule(&[]).unwrap();
        assert_eq!(schedule, DEPLOYMENT_TOPOLOGIES);
        assert_eq!(topology_for(0, &schedule), MapTopology::Flat);
        assert_eq!(topology_for(1, &schedule), MapTopology::Planet);
        assert_eq!(topology_for(2, &schedule), MapTopology::Flat);
        let single = ["--shape".to_string(), "planet".to_string()];
        assert_eq!(topology_schedule(&single).unwrap(), [MapTopology::Planet]);
        let conflicting = [
            "--shape".to_string(),
            "planet".to_string(),
            "--shapes".to_string(),
            "flat,planet".to_string(),
        ];
        assert!(topology_schedule(&conflicting).is_err());
    }

    #[test]
    fn site_choice_prefers_production_then_stable_low_coordinates() {
        let chosen = choose_site([
            SiteCandidate {
                production: 12.0,
                city: 8,
                pos: (3, 3),
            },
            SiteCandidate {
                production: 15.0,
                city: 9,
                pos: (4, 4),
            },
            SiteCandidate {
                production: 15.0,
                city: 2,
                pos: (5, 5),
            },
            SiteCandidate {
                production: 15.0,
                city: 2,
                pos: (1, 1),
            },
        ])
        .unwrap();
        assert_eq!(chosen.city, 2);
        assert_eq!(chosen.pos, (1, 1));
    }

    #[test]
    fn adaptive_schedule_is_science_only_and_orders_one_legal_site_at_a_time() {
        let mut projects = std::collections::BTreeSet::new();
        assert_eq!(desired_spaceports(Some("science"), &projects), None);
        projects.insert("launch_moon_landing".to_string());
        assert_eq!(
            desired_spaceports(Some("culture"), &projects),
            None,
            "a non-Science plan must never receive the treatment"
        );
        assert_eq!(
            desired_spaceports(Some("science"), &projects),
            Some((2, Milestone::Moon))
        );
        projects.insert("launch_mars_colony".to_string());
        assert_eq!(
            desired_spaceports(Some("science"), &projects),
            Some((3, Milestone::Mars))
        );

        let mut game = science_fixture();
        assert!(spaceport_order(&game, 0, Some("culture")).is_none());
        let moon = spaceport_order(&game, 0, Some("science")).unwrap();
        assert_eq!(moon.milestone, Milestone::Moon);
        game.apply(0, &moon.action).unwrap();
        assert!(spaceport_order(&game, 0, Some("science")).is_none());
    }

    #[test]
    fn replay_defers_end_turn_and_reproduces_the_direct_world() {
        let mut game = Game::new(2, 20, 14, 79_201, 20, 0);
        game.set_fog_memory(false);
        let mut direct = game.clone();
        let mut direct_ai = AdvancedAi::new();
        direct_ai.take_turn(&mut direct, 0);

        let mut replay_ai = AdvancedAi::new();
        replay_stock_actions_without_end(&mut game, &mut replay_ai, 0).unwrap();
        assert_eq!(game.current, 0);
        game.apply(0, &Action::EndTurn).unwrap();
        assert_eq!(
            serde_json::to_vec(&game).unwrap(),
            serde_json::to_vec(&direct).unwrap()
        );
        assert_eq!(replay_ai.strategy_label(), direct_ai.strategy_label());
    }

    #[test]
    fn external_observation_preserves_the_policy_horizon() {
        let options = GameOptions::new(2, 20, 14, 79_202, 1, 0);
        let result = play(options, 0, Mode::Control, 3, &Weights::default(), false);
        assert_eq!(result.policy_max_turns, 1);
        assert_eq!(result.reported_turn, 3);
    }

    #[test]
    fn frozen_controller_uses_the_committed_champion_weights() {
        let champion = frozen_champion();
        let game = Game::new(2, 20, 14, 79_203, 1, 0);
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
    fn frozen_gates_require_mechanism_progress_and_harm_guards() {
        let passing = GateInputs {
            coverage: 0.20,
            orders: 12,
            treatment_spaceports: 20,
            control_spaceports: 10,
            treatment_lasers: 5,
            control_lasers: 2,
            science_delta: 1.0,
            science_favorable: 8,
            science_adverse: 2,
            science_p: 0.109,
            treatment_science_wins: 3,
            control_science_wins: 2,
            treatment_wins: 4,
            control_wins: 3,
            paired_win_score: 0.52,
            terminal_score_share: 0.501,
        };
        assert!(screen_passes(passing));
        assert!(holdout_passes(GateInputs {
            science_p: 0.01,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_lasers: 2,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_wins: 2,
            ..passing
        }));
    }
}
