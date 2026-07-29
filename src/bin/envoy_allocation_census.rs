//! Observer-only census of Envoy allocation under a conserved raw stock.
//!
//! The focal champion is run on a clone, its successful actions are replayed,
//! and every `SendEnvoy` is classified at its real before/after boundary. A
//! separate dynamic program asks what the same stored raw Envoy stock could
//! achieve across city-states the focal empire has actually met.
use civvis::ai::{AdvancedAi, Ai, PlanReport, StrategyCensus, Weights};
use civvis::evolve::Champion;
use civvis::game::{Action, Game, GameOptions, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use std::collections::BTreeMap;

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 10_032_999;
const CENSUS_MAPS: usize = 30;
const CENSUS_SEED: u64 = 10_033_000;
const NOMINAL_TURNS: u32 = 250;
const OBSERVE_THROUGH: u32 = 320;
const FROZEN_AI: &str = "advanced_evolved";
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
const PROFILE_OVERRIDE_FLAGS: [&str; 7] = [
    "--players",
    "--width",
    "--height",
    "--city-states",
    "--map",
    "--shape",
    "--shapes",
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
    let fingerprint = fnv1a(EMBEDDED_CHAMPION.as_bytes());
    assert_eq!(
        fingerprint, FROZEN_CHAMPION_FNV1A,
        "data/evolved/best.json changed after Envoy census preregistration"
    );
    let champion: Champion = serde_json::from_str(EMBEDDED_CHAMPION)
        .expect("the committed advanced_evolved champion must be valid JSON");
    assert_eq!(
        champion.gen, FROZEN_CHAMPION_GENERATION,
        "Envoy census champion generation changed"
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

/// The diagnostic parser is intentionally forgiving. Registered labels are
/// not: every frozen token must occur exactly once with the literal committed
/// value, and no extra token may be present.
fn registered_profile(args: &[String], null: bool) -> bool {
    let expected_values = [
        ("--maps", if null { "4" } else { "30" }),
        ("--turns", "250"),
        ("--observe-through", "320"),
        ("--speed", "online"),
        ("--poles", "poles"),
        ("--victories", "science,culture,domination"),
        ("--ai", FROZEN_AI),
        ("--seed", if null { "10032999" } else { "10033000" }),
        ("--jobs", "6"),
    ];
    let expected_len = expected_values.len() * 2 + 2 + usize::from(null);
    args.len() == expected_len
        && flag_once(args, "--deployment-mix")
        && flag_once(args, "--randomize-civs")
        && if null {
            flag_once(args, "--null")
        } else {
            !has_arg(args, "--null")
        }
        && expected_values
            .iter()
            .all(|(key, value)| value_once(args, key, value))
}

fn number(args: &[String], key: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], key: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn raw_envoys_at(game: &Game, pid: usize, minor: usize) -> i64 {
    game.players[pid]
        .envoys
        .iter()
        .find(|(target, _)| *target == minor)
        .map(|(_, raw)| (*raw).max(0))
        .unwrap_or(0)
}

fn raw_stock(game: &Game, pid: usize) -> usize {
    game.players[pid]
        .envoys
        .iter()
        .map(|(_, raw)| (*raw).max(0) as usize)
        .sum()
}

fn threshold_levels(effective: i64) -> usize {
    usize::from(effective >= 1) + usize::from(effective >= 3) + usize::from(effective >= 6)
}

type PayoffOptions = Vec<(usize, usize)>;

fn eligible_city_states(game: &Game, pid: usize) -> Vec<usize> {
    game.players
        .iter()
        .filter(|minor| minor.alive && minor.is_minor && !minor.is_barbarian)
        .filter(|minor| game.has_met(pid, minor.id) && !game.is_at_war(pid, minor.id))
        .map(|minor| minor.id)
        .collect()
}

fn unseen_raw_stock(game: &Game, pid: usize) -> usize {
    game.players[pid]
        .envoys
        .iter()
        .filter(|(target, raw)| {
            *raw > 0
                && game
                    .players
                    .get(*target)
                    .is_some_and(|player| player.alive && player.is_minor && !player.is_barbarian)
                && !game.has_met(pid, *target)
        })
        .map(|(_, raw)| *raw as usize)
        .sum()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct AllocationBound {
    raw_budget: usize,
    unseen_raw: usize,
    eligible_states: usize,
    actual_suzerainties: usize,
    maximum_suzerainties: usize,
    actual_thresholds: usize,
    maximum_thresholds: usize,
}

/// Minimum raw cost for every distinct payoff available at one city-state.
/// Engine methods, rather than copied Amani arithmetic, resolve each option.
fn state_options(
    probe: &mut Game,
    pid: usize,
    minor: usize,
    budget: usize,
) -> (PayoffOptions, PayoffOptions) {
    let mut cheapest_suzerainty = BTreeMap::<usize, usize>::new();
    let mut cheapest_thresholds = BTreeMap::<usize, usize>::new();
    for raw in 0..=budget {
        // The checkpoint created this isolated probe with an empty QueryCache,
        // and this sweep never opens a query_memo guard. Engine queries thus
        // recompute after each deliberately mutated candidate allocation.
        probe.players[pid].envoys.clear();
        if raw > 0 {
            probe.players[pid].envoys.push((minor, raw as i64));
        }
        let suzerainty = usize::from(probe.suzerain_of(minor) == Some(pid));
        let thresholds = threshold_levels(probe.envoys_at(pid, minor));
        cheapest_suzerainty.entry(suzerainty).or_insert(raw);
        cheapest_thresholds.entry(thresholds).or_insert(raw);
    }
    let by_cost = |cheapest: BTreeMap<usize, usize>| {
        cheapest
            .into_iter()
            .map(|(payoff, cost)| (cost, payoff))
            .collect::<Vec<_>>()
    };
    (by_cost(cheapest_suzerainty), by_cost(cheapest_thresholds))
}

fn optimize_options(options: &[Vec<(usize, usize)>], budget: usize) -> usize {
    let mut dp = vec![None; budget + 1];
    dp[0] = Some(0usize);
    for state in options {
        let mut next: Vec<Option<usize>> = vec![None; budget + 1];
        for (used, value) in dp.iter().enumerate() {
            let Some(value) = value else { continue };
            for (cost, payoff) in state {
                let total = used + cost;
                if total <= budget {
                    let candidate = value + payoff;
                    next[total] = Some(next[total].map_or(candidate, |old| old.max(candidate)));
                }
            }
        }
        dp = next;
    }
    dp.into_iter().flatten().max().unwrap_or(0)
}

fn conserved_stock_bound(game: &Game, pid: usize) -> Option<AllocationBound> {
    let eligible = eligible_city_states(game, pid);
    if eligible.is_empty() {
        return None;
    }
    let budget = raw_stock(game, pid);
    let actual_suzerainties = eligible
        .iter()
        .filter(|minor| game.suzerain_of(**minor) == Some(pid))
        .count();
    let actual_thresholds = eligible
        .iter()
        .map(|minor| threshold_levels(game.envoys_at(pid, *minor)))
        .sum();
    let mut probe = game.clone();
    let mut suzerain_options = Vec::with_capacity(eligible.len());
    let mut threshold_options = Vec::with_capacity(eligible.len());
    for minor in &eligible {
        let (suzerainty, thresholds) = state_options(&mut probe, pid, *minor, budget);
        suzerain_options.push(suzerainty);
        threshold_options.push(thresholds);
    }
    Some(AllocationBound {
        raw_budget: budget,
        unseen_raw: unseen_raw_stock(game, pid),
        eligible_states: eligible.len(),
        actual_suzerainties,
        maximum_suzerainties: optimize_options(&suzerain_options, budget),
        actual_thresholds,
        maximum_thresholds: optimize_options(&threshold_options, budget),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TargetState {
    met: bool,
    free: i64,
    raw: i64,
    effective: i64,
    best_rival: i64,
    suzerain: Option<usize>,
}

fn target_state(game: &Game, pid: usize, minor: usize) -> TargetState {
    let best_rival = game
        .players
        .iter()
        .filter(|player| {
            player.alive && !player.is_minor && !player.is_barbarian && player.id != pid
        })
        .map(|player| game.envoys_at(player.id, minor))
        .max()
        .unwrap_or(0);
    TargetState {
        met: game.has_met(pid, minor),
        free: game.players[pid].envoys_free.max(0),
        raw: raw_envoys_at(game, pid, minor),
        effective: game.envoys_at(pid, minor),
        best_rival,
        suzerain: game.suzerain_of(minor),
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct EnvoyCensus {
    focal_turns: u32,
    sends: u32,
    met_sends: u32,
    unseen_sends: u32,
    threshold_crossings: u32,
    threshold_sends: u32,
    suzerainties_acquired: u32,
    secure_extensions: u32,
    no_immediate_effect: u32,
    met_no_immediate_effect: u32,
    raw_added: i64,
    effective_added: i64,
    free_spent: i64,
    rival_effective_sum: i64,
    checkpoints: u32,
    positive_gap_checkpoints: u32,
    resource_limited_checkpoints: u32,
    raw_budget_sum: u64,
    unseen_raw_sum: u64,
    eligible_state_sum: u64,
    actual_suzerainty_sum: u64,
    maximum_suzerainty_sum: u64,
    actual_threshold_sum: u64,
    maximum_threshold_sum: u64,
    free_envoy_sum: i64,
}

fn record_send(census: &mut EnvoyCensus, pid: usize, before: TargetState, after: TargetState) {
    census.sends += 1;
    if before.met {
        census.met_sends += 1;
    } else {
        census.unseen_sends += 1;
    }
    let crossed =
        threshold_levels(after.effective).saturating_sub(threshold_levels(before.effective));
    census.threshold_crossings += crossed as u32;
    census.threshold_sends += (crossed > 0) as u32;
    let focal_acquired = before.suzerain != Some(pid) && after.suzerain == Some(pid);
    census.suzerainties_acquired += focal_acquired as u32;
    census.secure_extensions += (before.suzerain == Some(pid)
        && after.suzerain == Some(pid)
        && after.effective > before.effective) as u32;
    census.no_immediate_effect += (crossed == 0 && !focal_acquired) as u32;
    census.met_no_immediate_effect += (before.met && crossed == 0 && !focal_acquired) as u32;
    census.raw_added += (after.raw - before.raw).max(0);
    census.effective_added += (after.effective - before.effective).max(0);
    census.free_spent += (before.free - after.free).max(0);
    census.rival_effective_sum += before.best_rival.max(0);
}

fn record_checkpoint(game: &Game, pid: usize, census: &mut EnvoyCensus) {
    let Some(bound) = conserved_stock_bound(game, pid) else {
        return;
    };
    census.checkpoints += 1;
    census.positive_gap_checkpoints += (bound.maximum_suzerainties > bound.actual_suzerainties
        || bound.maximum_thresholds > bound.actual_thresholds)
        as u32;
    census.resource_limited_checkpoints += (bound.maximum_suzerainties == 0) as u32;
    census.raw_budget_sum += bound.raw_budget as u64;
    census.unseen_raw_sum += bound.unseen_raw as u64;
    census.eligible_state_sum += bound.eligible_states as u64;
    census.actual_suzerainty_sum += bound.actual_suzerainties as u64;
    census.maximum_suzerainty_sum += bound.maximum_suzerainties as u64;
    census.actual_threshold_sum += bound.actual_thresholds as u64;
    census.maximum_threshold_sum += bound.maximum_thresholds as u64;
    census.free_envoy_sum += game.players[pid].envoys_free.max(0);
}

/// Run the champion on a clone, retain its state, and replay every successful
/// action except the final EndTurn. Classification observes the actual replay.
fn replay_champion_turn(
    game: &mut Game,
    ai: &mut AdvancedAi,
    pid: usize,
    census: Option<&mut EnvoyCensus>,
) -> Result<(), String> {
    let mut observed = game.clone();
    let before_log = observed.log.len();
    let mut actor = ai.clone();
    actor.take_turn(&mut observed, pid);
    let mut actions: Vec<(usize, Action)> = observed.log.since(before_log).cloned().collect();
    let ended = actions
        .last()
        .is_some_and(|(owner, action)| *owner == pid && matches!(action, Action::EndTurn));
    if ended {
        actions.pop();
    }
    let mut census = census;
    for (owner, action) in actions {
        if owner != pid {
            return Err(format!(
                "champion seat {pid} logged an action for seat {owner}: {action:?}"
            ));
        }
        let before = match &action {
            Action::SendEnvoy { player } => Some(target_state(game, pid, *player)),
            _ => None,
        };
        game.apply(owner, &action).map_err(|why| {
            format!("champion action replay failed for seat {pid}: {why}; {action:?}")
        })?;
        if let (Some(census), Some(before), Action::SendEnvoy { player }) =
            (census.as_deref_mut(), before, &action)
        {
            let after = target_state(game, pid, *player);
            record_send(census, pid, before, after);
        }
    }
    *ai = actor;
    if ended && game.winner.is_none() && game.current != pid {
        return Err(format!(
            "champion replay advanced from seat {pid} before deferred EndTurn"
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Direct,
    ReplayNull,
    Census,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalResult {
    won: bool,
    winner: Option<usize>,
    victory: Option<String>,
    reported_turn: u32,
    score: i64,
}

struct Played {
    terminal: TerminalResult,
    census: EnvoyCensus,
    serialized: Option<String>,
    focal_plan: Option<PlanReport>,
    focal_strategy_census: StrategyCensus,
}

fn play(
    options: GameOptions,
    focal: usize,
    mode: Mode,
    observe_through: u32,
    weights: &Weights,
    serialize: bool,
) -> Played {
    let mut game = Game::new_with(options);
    let policy_max_turns = game.max_turns;
    assert!(observe_through >= policy_max_turns);
    game.set_fog_memory(false);
    game.victory_conditions = VictoryConditions::parse("science,culture,domination").unwrap();
    let mut ais = AdvancedAi::fleet_weighted(&game, weights);
    let mut census = EnvoyCensus::default();
    while game.winner.is_none() && game.turn <= observe_through {
        assert_eq!(game.max_turns, policy_max_turns);
        let pid = game.current;
        if pid == focal && mode != Mode::Direct {
            replay_champion_turn(
                &mut game,
                &mut ais[pid],
                pid,
                (mode == Mode::Census).then_some(&mut census),
            )
            .unwrap_or_else(|why| panic!("turn {} seat {pid}: {why}", game.turn));
            if mode == Mode::Census {
                census.focal_turns += 1;
                record_checkpoint(&game, pid, &mut census);
            }
        } else {
            ais[pid].take_turn(&mut game, pid);
        }
        if mode != Mode::Direct && game.winner.is_none() && game.current == pid {
            game.apply(pid, &Action::EndTurn).unwrap_or_else(|why| {
                panic!(
                    "turn {} seat {pid}: deferred EndTurn failed: {why}",
                    game.turn
                )
            });
        }
    }
    assert_eq!(game.max_turns, policy_max_turns);
    let terminal = TerminalResult {
        won: game.winner == Some(focal),
        winner: game.winner,
        victory: game.victory_type.clone(),
        reported_turn: if game.winner.is_some() {
            game.reported_turn()
        } else {
            observe_through
        },
        score: game.score(focal),
    };
    let serialized = serialize
        .then(|| serde_json::to_string(&game).expect("terminal Game must remain serializable"));
    let focal_plan = ais[focal].plan_report();
    let focal_strategy_census = ais[focal].strategy_census();
    Played {
        terminal,
        census,
        serialized,
        focal_plan,
        focal_strategy_census,
    }
}

#[derive(Clone)]
struct SeatResult {
    terminal: TerminalResult,
    census: EnvoyCensus,
}

#[derive(Clone)]
struct MapResult {
    profile: DeploymentProfile,
    seats: [SeatResult; 2],
    exact: [bool; 2],
}

#[derive(Default)]
struct Summary {
    maps: usize,
    games: usize,
    wins: usize,
    sends: u64,
    met_sends: u64,
    unseen_sends: u64,
    threshold_crossings: u64,
    threshold_sends: u64,
    acquired: u64,
    secure_extensions: u64,
    no_immediate: u64,
    met_no_immediate: u64,
    raw_added: i64,
    effective_added: i64,
    free_spent: i64,
    rival_effective_sum: i64,
    checkpoints: u64,
    positive_gap_checkpoints: u64,
    resource_limited_checkpoints: u64,
    raw_budget_sum: u64,
    unseen_raw_sum: u64,
    eligible_state_sum: u64,
    actual_suzerainty_sum: u64,
    maximum_suzerainty_sum: u64,
    actual_threshold_sum: u64,
    maximum_threshold_sum: u64,
    free_envoy_sum: i64,
    allocation_gap_games: usize,
    no_immediate_games: usize,
    eligible_games: usize,
    allocation_gap_maps: usize,
    no_immediate_maps: usize,
    met_send_maps: usize,
    eligible_maps: usize,
    map_no_immediate_share_sum: f64,
    map_resource_limited_share_sum: f64,
}

impl Summary {
    fn record_map(&mut self, result: &MapResult) {
        self.maps += 1;
        self.allocation_gap_maps += result
            .seats
            .iter()
            .any(|seat| seat.census.positive_gap_checkpoints >= 5)
            as usize;
        let map_met_sends = result
            .seats
            .iter()
            .map(|seat| seat.census.met_sends as u64)
            .sum::<u64>();
        let map_met_no_immediate = result
            .seats
            .iter()
            .map(|seat| seat.census.met_no_immediate_effect as u64)
            .sum::<u64>();
        if map_met_sends > 0 {
            self.met_send_maps += 1;
            self.map_no_immediate_share_sum += map_met_no_immediate as f64 / map_met_sends as f64;
        }
        self.no_immediate_maps += (map_met_no_immediate > 0) as usize;
        let map_checkpoints = result
            .seats
            .iter()
            .map(|seat| seat.census.checkpoints as u64)
            .sum::<u64>();
        let map_resource_limited = result
            .seats
            .iter()
            .map(|seat| seat.census.resource_limited_checkpoints as u64)
            .sum::<u64>();
        if map_checkpoints > 0 {
            self.eligible_maps += 1;
            self.map_resource_limited_share_sum +=
                map_resource_limited as f64 / map_checkpoints as f64;
        }
        for seat in &result.seats {
            self.games += 1;
            self.wins += seat.terminal.won as usize;
            let census = &seat.census;
            self.sends += census.sends as u64;
            self.met_sends += census.met_sends as u64;
            self.unseen_sends += census.unseen_sends as u64;
            self.threshold_crossings += census.threshold_crossings as u64;
            self.threshold_sends += census.threshold_sends as u64;
            self.acquired += census.suzerainties_acquired as u64;
            self.secure_extensions += census.secure_extensions as u64;
            self.no_immediate += census.no_immediate_effect as u64;
            self.met_no_immediate += census.met_no_immediate_effect as u64;
            self.raw_added += census.raw_added;
            self.effective_added += census.effective_added;
            self.free_spent += census.free_spent;
            self.rival_effective_sum += census.rival_effective_sum;
            self.checkpoints += census.checkpoints as u64;
            self.positive_gap_checkpoints += census.positive_gap_checkpoints as u64;
            self.resource_limited_checkpoints += census.resource_limited_checkpoints as u64;
            self.raw_budget_sum += census.raw_budget_sum;
            self.unseen_raw_sum += census.unseen_raw_sum;
            self.eligible_state_sum += census.eligible_state_sum;
            self.actual_suzerainty_sum += census.actual_suzerainty_sum;
            self.maximum_suzerainty_sum += census.maximum_suzerainty_sum;
            self.actual_threshold_sum += census.actual_threshold_sum;
            self.maximum_threshold_sum += census.maximum_threshold_sum;
            self.free_envoy_sum += census.free_envoy_sum;
            self.allocation_gap_games += (census.positive_gap_checkpoints >= 5) as usize;
            self.no_immediate_games += (census.met_no_immediate_effect > 0) as usize;
            self.eligible_games += (census.checkpoints > 0) as usize;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    Allocation,
    Acquisition,
    Mixed,
    NoMechanism,
}

fn route(summary: &Summary) -> Route {
    let resource_limited_share =
        summary.map_resource_limited_share_sum / summary.eligible_maps.max(1) as f64;
    let allocation = summary.allocation_gap_maps >= 10;
    let acquisition = summary.eligible_maps >= 20 && resource_limited_share + 1e-12 >= 0.25;
    match (allocation, acquisition) {
        (true, true) => Route::Mixed,
        (true, false) => Route::Allocation,
        (false, true) => Route::Acquisition,
        (false, false) => Route::NoMechanism,
    }
}

fn print_summary(label: &str, results: impl Iterator<Item = MapResult>) {
    let mut summary = Summary::default();
    for result in results {
        summary.record_map(&result);
    }
    let checkpoints = summary.checkpoints.max(1) as f64;
    let map_no_immediate_share =
        summary.map_no_immediate_share_sum / summary.met_send_maps.max(1) as f64;
    let map_resource_limited_share =
        summary.map_resource_limited_share_sum / summary.eligible_maps.max(1) as f64;
    println!(
        "  {label:<24} {:>3} maps/{:>3} seat-games; wins {}; sends {} (met {}, unseen {}), threshold sends/crossings {}/{}, acquisitions {}, secure extensions {}, no-immediate met/all {}/{}",
        summary.maps,
        summary.games,
        summary.wins,
        summary.sends,
        summary.met_sends,
        summary.unseen_sends,
        summary.threshold_sends,
        summary.threshold_crossings,
        summary.acquired,
        summary.secure_extensions,
        summary.met_no_immediate,
        summary.no_immediate,
    );
    println!(
        "  {label:<24} send deltas raw +{}, effective +{}, free spent {}; mean rival effective {:.2}",
        summary.raw_added,
        summary.effective_added,
        summary.free_spent,
        summary.rival_effective_sum as f64 / summary.sends.max(1) as f64,
    );
    println!(
        "  {label:<24} checkpoints {}; raw {:.2} (unseen {:.2}), free {:.2}, eligible states {:.2}, suzerainties {:.2}->{:.2}, thresholds {:.2}->{:.2}, gap games {}, resource-limited {:.1}%",
        summary.checkpoints,
        summary.raw_budget_sum as f64 / checkpoints,
        summary.unseen_raw_sum as f64 / checkpoints,
        summary.free_envoy_sum as f64 / checkpoints,
        summary.eligible_state_sum as f64 / checkpoints,
        summary.actual_suzerainty_sum as f64 / checkpoints,
        summary.maximum_suzerainty_sum as f64 / checkpoints,
        summary.actual_threshold_sum as f64 / checkpoints,
        summary.maximum_threshold_sum as f64 / checkpoints,
        summary.allocation_gap_games,
        100.0 * summary.resource_limited_checkpoints as f64 / checkpoints,
    );
    println!(
        "  {label:<24} route units: gap maps {}, no-immediate maps {}/{} sending ({:.1}% equal-map mean), eligible maps {}, resource-limited {:.1}% equal-map mean",
        summary.allocation_gap_maps,
        summary.no_immediate_maps,
        summary.met_send_maps,
        100.0 * map_no_immediate_share,
        summary.eligible_maps,
        100.0 * map_resource_limited_share,
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let null = has_arg(&args, "--null");
    let deployment_mix = has_arg(&args, "--deployment-mix");
    let ai_name = text(&args, "--ai", FROZEN_AI);
    if ai_name != FROZEN_AI {
        eprintln!("this census is frozen for {FROZEN_AI}; got controller {ai_name:?}");
        std::process::exit(2);
    }
    if deployment_mix {
        let conflicts = PROFILE_OVERRIDE_FLAGS
            .iter()
            .filter(|flag| has_arg(&args, flag))
            .copied()
            .collect::<Vec<_>>();
        if !conflicts.is_empty() {
            eprintln!(
                "--deployment-mix derives every profile; remove conflicting flags: {}",
                conflicts.join(", ")
            );
            std::process::exit(2);
        }
    }
    let champion = frozen_champion();
    let maps = number(
        &args,
        "--maps",
        if null {
            NULL_MAPS as i64
        } else {
            CENSUS_MAPS as i64
        },
    )
    .max(1) as usize;
    let players = number(&args, "--players", 8).max(2) as usize;
    let size = MapSize::for_players(players);
    let width = number(&args, "--width", size.width as i64).max(8) as i32;
    let height = number(&args, "--height", size.height as i64).max(8) as i32;
    let city_states =
        number(&args, "--city-states", size.default_city_states as i64).max(0) as usize;
    let turns = number(&args, "--turns", NOMINAL_TURNS as i64).max(1) as u32;
    let observe_through = number(&args, "--observe-through", OBSERVE_THROUGH as i64).max(1) as u32;
    if observe_through < turns {
        eprintln!("--observe-through must be at least --turns");
        std::process::exit(2);
    }
    let seed = number(
        &args,
        "--seed",
        if null {
            NULL_SEED as i64
        } else {
            CENSUS_SEED as i64
        },
    )
    .max(0) as u64;
    let requested_jobs = number(&args, "--jobs", 0);
    let jobs = match requested_jobs {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    }
    .clamp(1, 6);
    let speed = text(&args, "--speed", "online");
    let map_script = MapScript::from_id(&text(&args, "--map", "continents")).unwrap_or_else(|| {
        eprintln!("unknown map script");
        std::process::exit(2);
    });
    let map_topology =
        MapTopology::from_id(&text(&args, "--shape", "planet")).unwrap_or_else(|| {
            eprintln!("unknown map topology");
            std::process::exit(2);
        });
    let map_poles = MapPoles::from_id(&text(&args, "--poles", "poles")).unwrap_or_else(|| {
        eprintln!("unknown poles profile");
        std::process::exit(2);
    });
    let victory_names = text(&args, "--victories", "science,culture,domination");
    let victories = VictoryConditions::parse(&victory_names).unwrap_or_else(|why| {
        eprintln!("--victories: {why}");
        std::process::exit(2);
    });
    let expected_victories = VictoryConditions::parse("science,culture,domination").unwrap();
    if victories != expected_victories {
        eprintln!("this census is frozen for science,culture,domination");
        std::process::exit(2);
    }
    let randomize_civs = has_arg(&args, "--randomize-civs");
    if !Rules::embedded().speeds.contains_key(&speed) {
        eprintln!("unknown game speed {speed:?}");
        std::process::exit(2);
    }

    println!("Conserved-stock Envoy allocation census");
    println!(
        "controller: {ai_name}; embedded champion generation {}",
        champion.gen
    );
    if deployment_mix {
        let player_rows = deployment_counts(maps, |profile| profile.players)
            .into_iter()
            .map(|(value, count)| format!("{value}p={count}"))
            .collect::<Vec<_>>()
            .join(",");
        let scripts = deployment_counts(maps, |profile| profile.map_script)
            .into_iter()
            .map(|(value, count)| format!("{}={count}", value.id()))
            .collect::<Vec<_>>()
            .join(",");
        let topologies = deployment_counts(maps, |profile| profile.map_topology)
            .into_iter()
            .map(|(value, count)| format!("{}={count}", value.id()))
            .collect::<Vec<_>>()
            .join(",");
        println!("profile: deployment mix; players {player_rows}; scripts {scripts}; topologies {topologies}");
    } else {
        println!(
            "profile: diagnostic {}p {}x{}+{}cs {} {}",
            players,
            width,
            height,
            city_states,
            map_script.id(),
            map_topology.id()
        );
    }
    println!(
        "rules: {turns} policy-visible {speed} turns, observe through {observe_through}; poles {}; civilizations {}; victories {victory_names}; seed {seed}; {jobs} jobs",
        map_poles.id(),
        if randomize_civs { "randomized" } else { "fixed" }
    );
    println!(
        "batch: {maps} maps x seats 0/final = {} focal games{}",
        maps * 2,
        if null {
            "; plus matched direct games for exact null"
        } else {
            ""
        }
    );

    let results = civvis::parallel::map_reporting(
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
            let seats = [0, profile.players - 1];
            let mut observations = Vec::new();
            let mut exact = [true; 2];
            for (index, seat) in seats.into_iter().enumerate() {
                if null {
                    let direct = play(
                        options.clone(),
                        seat,
                        Mode::Direct,
                        observe_through,
                        &champion.weights,
                        true,
                    );
                    let replay = play(
                        options.clone(),
                        seat,
                        Mode::ReplayNull,
                        observe_through,
                        &champion.weights,
                        true,
                    );
                    exact[index] = direct.terminal == replay.terminal
                        && direct.serialized == replay.serialized
                        && direct.focal_plan == replay.focal_plan
                        && direct.focal_strategy_census == replay.focal_strategy_census;
                    observations.push(SeatResult {
                        terminal: replay.terminal,
                        census: replay.census,
                    });
                } else {
                    let observed = play(
                        options.clone(),
                        seat,
                        Mode::Census,
                        observe_through,
                        &champion.weights,
                        false,
                    );
                    observations.push(SeatResult {
                        terminal: observed.terminal,
                        census: observed.census,
                    });
                }
            }
            MapResult {
                profile,
                seats: observations.try_into().ok().expect("two focal seats"),
                exact,
            }
        },
        |completed, _| eprintln!("progress: {}/{} maps complete", completed + 1, maps),
    );

    let exact_profile = registered_profile(&args, null);

    if null {
        let mismatches = results
            .iter()
            .flat_map(|result| result.exact)
            .filter(|exact| !exact)
            .count();
        if mismatches > 0 {
            println!(
                "null sanity: BROKEN — {mismatches}/{} matched cells differed",
                maps * 2
            );
            std::process::exit(3);
        }
        if exact_profile && maps == NULL_MAPS && seed == NULL_SEED {
            println!(
                "registered null: PASS — all {} matched cells reproduced exactly",
                maps * 2
            );
        } else {
            println!("diagnostic null: PASS — no registered gate applies");
        }
        return;
    }

    let mut summary = Summary::default();
    for result in &results {
        summary.record_map(result);
    }
    println!();
    print_summary("pooled", results.clone().into_iter());
    for players in deployment_counts(maps, |profile| profile.players)
        .into_iter()
        .map(|(value, _)| value)
    {
        print_summary(
            &format!("players={players}"),
            results
                .iter()
                .filter(|result| result.profile.players == players)
                .cloned(),
        );
    }
    for script in deployment_counts(maps, |profile| profile.map_script)
        .into_iter()
        .map(|(value, _)| value)
    {
        print_summary(
            &format!("script={}", script.id()),
            results
                .iter()
                .filter(|result| result.profile.map_script == script)
                .cloned(),
        );
    }
    for topology in deployment_counts(maps, |profile| profile.map_topology)
        .into_iter()
        .map(|(value, _)| value)
    {
        print_summary(
            &format!("topology={}", topology.id()),
            results
                .iter()
                .filter(|result| result.profile.map_topology == topology)
                .cloned(),
        );
    }
    let no_immediate_share =
        summary.map_no_immediate_share_sum / summary.met_send_maps.max(1) as f64;
    let resource_limited_share =
        summary.map_resource_limited_share_sum / summary.eligible_maps.max(1) as f64;
    println!(
        "routing facts: allocation-gap maps {}/{}, no-immediate maps {}/{} sending, equal-map met-send share {:.1}%, eligible maps {}, equal-map resource-limited share {:.1}%",
        summary.allocation_gap_maps,
        summary.maps,
        summary.no_immediate_maps,
        summary.met_send_maps,
        100.0 * no_immediate_share,
        summary.eligible_maps,
        100.0 * resource_limited_share,
    );
    if summary.unseen_sends > 0 {
        println!(
            "HIDDEN-STATE LEGALITY BUG — {} sends targeted unmet city-states",
            summary.unseen_sends
        );
    }
    if exact_profile && maps == CENSUS_MAPS && seed == CENSUS_SEED {
        println!(
            "registered route: {}",
            match route(&summary) {
                Route::Allocation => "ALLOCATION LEAD",
                Route::Acquisition => "ACQUISITION LEAD",
                Route::Mixed => "MIXED",
                Route::NoMechanism => "NO MECHANISM",
            }
        );
    } else {
        println!("decision: DIAGNOSTIC ONLY — no registered route applies");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use civvis::game::GovernorState;
    use std::collections::BTreeSet;

    fn envoy_fixture(raw: &[(usize, i64)]) -> (Game, Vec<usize>) {
        let mut game = Game::new_full(2, 26, 16, 74_001, 100, 2, false);
        let minors = game
            .players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect::<Vec<_>>();
        assert_eq!(minors.len(), 2);
        for minor in &minors {
            game.players[0].met.insert(*minor);
            game.players[*minor].met.insert(0);
        }
        game.players[0].envoys = raw.to_vec();
        (game, minors)
    }

    #[test]
    fn deployment_cycle_is_unique_and_batches_are_frozen() {
        let profiles = (0..126).map(deployment_profile).collect::<Vec<_>>();
        for (index, profile) in profiles.iter().enumerate() {
            assert!(
                !profiles[..index].contains(profile),
                "duplicate profile {index}"
            );
        }
        assert_eq!(deployment_profile(126), deployment_profile(0));
        assert_eq!(
            deployment_counts(NULL_MAPS, |profile| profile.players).len(),
            4
        );
        assert_eq!(
            deployment_counts(CENSUS_MAPS, |profile| profile.map_topology),
            vec![(MapTopology::Flat, 15), (MapTopology::Planet, 15)]
        );
    }

    #[test]
    fn registered_labels_require_every_exact_frozen_token() {
        let args = |null: bool| {
            format!(
                "{}--deployment-mix --maps {} --turns 250 --observe-through 320 --speed online --poles poles --randomize-civs --victories science,culture,domination --ai advanced_evolved --seed {} --jobs 6",
                if null { "--null " } else { "" },
                if null { 4 } else { 30 },
                if null { 10_032_999 } else { 10_033_000 },
            )
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>()
        };
        let null_args = args(true);
        assert!(registered_profile(&null_args, true));
        assert!(registered_profile(&args(false), false));

        let mut malformed = null_args.clone();
        let maps = malformed.iter().position(|arg| arg == "--maps").unwrap();
        malformed[maps + 1] = "not-a-number".to_string();
        assert!(!registered_profile(&malformed, true));

        let mut duplicate = null_args;
        duplicate.push("--null".to_string());
        assert!(!registered_profile(&duplicate, true));
    }

    #[test]
    fn conserved_stock_finds_waste_without_inventing_raw_envoys() {
        let (mut wasteful, minors) = envoy_fixture(&[]);
        wasteful.players[0].envoys = vec![(minors[0], 6)];
        let wasteful_bound = conserved_stock_bound(&wasteful, 0).unwrap();
        assert_eq!(wasteful_bound.raw_budget, 6);
        assert_eq!(wasteful_bound.actual_suzerainties, 1);
        assert_eq!(wasteful_bound.maximum_suzerainties, 2);
        assert_eq!(wasteful_bound.actual_thresholds, 3);
        assert_eq!(wasteful_bound.maximum_thresholds, 4);

        wasteful.players[0].envoys = vec![(minors[0], 3), (minors[1], 3)];
        let efficient = conserved_stock_bound(&wasteful, 0).unwrap();
        assert_eq!(efficient.raw_budget, 6);
        assert_eq!(
            efficient.actual_suzerainties,
            efficient.maximum_suzerainties
        );
        assert_eq!(efficient.actual_thresholds, efficient.maximum_thresholds);
    }

    #[test]
    fn unseen_stock_is_conserved_but_only_met_states_receive_options() {
        let (mut game, minors) = envoy_fixture(&[]);
        game.players[0].met.remove(&minors[1]);
        game.players[0].envoys = vec![(minors[1], 3)];
        let bound = conserved_stock_bound(&game, 0).unwrap();
        assert_eq!(bound.raw_budget, 3);
        assert_eq!(bound.unseen_raw, 3);
        assert_eq!(bound.eligible_states, 1);
        assert_eq!(bound.maximum_suzerainties, 1);
    }

    #[test]
    fn send_classification_uses_true_before_and_after_states() {
        let (mut game, minors) = envoy_fixture(&[]);
        game.players[0].envoys_free = 4;
        let mut census = EnvoyCensus::default();
        for _ in 0..4 {
            let before = target_state(&game, 0, minors[0]);
            game.apply(0, &Action::SendEnvoy { player: minors[0] })
                .unwrap();
            let after = target_state(&game, 0, minors[0]);
            record_send(&mut census, 0, before, after);
        }
        assert_eq!(census.sends, 4);
        assert_eq!(census.met_sends, 4);
        assert_eq!(census.unseen_sends, 0);
        assert!(census.threshold_crossings >= 2);
        assert_eq!(census.suzerainties_acquired, 1);
        assert_eq!(census.secure_extensions, 1);
        assert_eq!(census.no_immediate_effect, 2);
        assert_eq!(census.met_no_immediate_effect, 2);
        assert_eq!(census.raw_added, 4);
        assert_eq!(census.effective_added, 4);
        assert_eq!(census.free_spent, 4);
        assert_eq!(census.rival_effective_sum, 0);
    }

    #[test]
    fn conserved_stock_uses_engine_amani_effective_envoys() {
        let (mut game, minors) = envoy_fixture(&[]);
        game.turn = 100;
        let city_state = game.player_city_ids(minors[0])[0];
        game.players[0].governor_roster.insert(
            "amani".to_string(),
            GovernorState {
                city: Some(city_state),
                assigned_turn: 0,
                disabled_until: 0,
                promotions: BTreeSet::from(["puppeteer".to_string()]),
            },
        );
        game.players[0].envoys = vec![(minors[0], 1)];

        assert_eq!(raw_stock(&game, 0), 1);
        assert_eq!(game.envoys_at(0, minors[0]), 6);
        let bound = conserved_stock_bound(&game, 0).unwrap();
        assert_eq!(bound.raw_budget, 1);
        assert_eq!(bound.actual_suzerainties, 1);
        assert_eq!(bound.maximum_suzerainties, 1);
        assert_eq!(bound.actual_thresholds, 3);
        assert_eq!(bound.maximum_thresholds, 3);
    }

    #[test]
    fn routing_requires_every_preregistered_prevalence_term() {
        let mut summary = Summary {
            maps: 30,
            allocation_gap_maps: 10,
            eligible_maps: 20,
            map_resource_limited_share_sum: 5.0,
            ..Summary::default()
        };
        assert_eq!(route(&summary), Route::Mixed);
        summary.allocation_gap_maps = 9;
        assert_eq!(route(&summary), Route::Acquisition);
        summary.map_resource_limited_share_sum = 4.8;
        assert_eq!(route(&summary), Route::NoMechanism);
        summary.no_immediate_maps = 30;
        summary.met_send_maps = 30;
        summary.map_no_immediate_share_sum = 30.0;
        assert_eq!(
            route(&summary),
            Route::NoMechanism,
            "setup or defensive-margin sends remain descriptive"
        );
        summary.allocation_gap_maps = 10;
        assert_eq!(route(&summary), Route::Allocation);
    }

    #[test]
    fn routing_shares_give_each_map_equal_weight() {
        let terminal = TerminalResult {
            won: false,
            winner: None,
            victory: None,
            reported_turn: 320,
            score: 0,
        };
        let map = |met_sends, no_immediate, checkpoints, resource_limited| MapResult {
            profile: deployment_profile(0),
            seats: [
                SeatResult {
                    terminal: terminal.clone(),
                    census: EnvoyCensus {
                        met_sends,
                        met_no_immediate_effect: no_immediate,
                        checkpoints,
                        resource_limited_checkpoints: resource_limited,
                        ..EnvoyCensus::default()
                    },
                },
                SeatResult {
                    terminal: terminal.clone(),
                    census: EnvoyCensus::default(),
                },
            ],
            exact: [true; 2],
        };
        let mut summary = Summary::default();
        summary.record_map(&map(100, 0, 100, 0));
        summary.record_map(&map(1, 1, 1, 1));
        assert_eq!(summary.met_send_maps, 2);
        assert_eq!(summary.eligible_maps, 2);
        assert!((summary.map_no_immediate_share_sum / 2.0 - 0.5).abs() < 1e-12);
        assert!((summary.map_resource_limited_share_sum / 2.0 - 0.5).abs() < 1e-12);
    }

    #[test]
    fn frozen_controller_is_the_committed_champion() {
        let champion = frozen_champion();
        let game = Game::new(2, 20, 14, 74_002, 1, 0);
        let ais = AdvancedAi::fleet_weighted(&game, &champion.weights);
        assert!(champion.gen > 0);
        assert_eq!(ais[0].weights(), &champion.weights);
        assert_ne!(ais[0].weights(), &Weights::default());
    }

    #[test]
    fn replay_preserves_world_and_controller_on_a_small_fixture() {
        let champion = frozen_champion();
        let mut direct = Game::new(2, 20, 14, 74_003, 3, 0);
        let mut replay = direct.clone();
        let mut direct_ai = AdvancedAi::with_weights(champion.weights.clone());
        let mut replay_ai = direct_ai.clone();
        direct_ai.take_turn(&mut direct, 0);
        replay_champion_turn(&mut replay, &mut replay_ai, 0, None).unwrap();
        if replay.winner.is_none() && replay.current == 0 {
            replay.apply(0, &Action::EndTurn).unwrap();
        }
        assert_eq!(
            serde_json::to_string(&direct).unwrap(),
            serde_json::to_string(&replay).unwrap()
        );
        assert_eq!(direct_ai.plan_report(), replay_ai.plan_report());
        assert_eq!(direct_ai.strategy_census(), replay_ai.strategy_census());
    }

    #[test]
    fn replay_matches_direct_across_the_disabled_score_rollover() {
        let champion = frozen_champion();
        let options = GameOptions::new(2, 20, 14, 74_004, 3, 0);
        let direct = play(options.clone(), 0, Mode::Direct, 3, &champion.weights, true);
        let replay = play(options, 0, Mode::ReplayNull, 3, &champion.weights, true);
        assert_eq!(direct.terminal.reported_turn, 3);
        assert_eq!(direct.terminal.winner, None);
        assert_eq!(direct.terminal.victory, None);
        let crossed: Game = serde_json::from_str(direct.serialized.as_deref().unwrap()).unwrap();
        assert_eq!(crossed.max_turns, 3);
        assert_eq!(crossed.turn, 4);
        assert_eq!(crossed.winner, None);
        assert_eq!(direct.terminal, replay.terminal);
        assert_eq!(direct.serialized, replay.serialized);
        assert_eq!(direct.focal_plan, replay.focal_plan);
        assert_eq!(direct.focal_strategy_census, replay.focal_strategy_census);
    }
}
